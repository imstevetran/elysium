use crate::ast::*;
use crate::error::Result;
use crate::lexer::{Token, TokenStream};

/// Recursive descent parser for Elysium 2.0
pub struct Parser<'a> {
    tokens: Vec<(usize, Token, usize)>,
    pos: usize,
    source: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let tokens: Vec<_> = TokenStream::new(input).collect();
        Self {
            tokens,
            pos: 0,
            source: input,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        let mut top_stmts = Vec::new();
        while self.pos < self.tokens.len() {
            if self.peek_is(&Token::Let)
                || self.peek_is(&Token::Var)
                || self.peek_is(&Token::Func)
                || self.peek_is(&Token::Async)
                || self.peek_is(&Token::Class)
                || self.peek_is(&Token::Enum)
                || self.peek_is(&Token::Component)
                || self.peek_is(&Token::Typealias)
                || self.peek_is(&Token::Import)
                || self.peek_is(&Token::Spec)
                || self.peek_is(&Token::Describe)
            {
                // Check if we have a function/class/etc — these are items
                if self.peek_is(&Token::Func) || self.peek_is(&Token::Async)
                    || self.peek_is(&Token::Class) || self.peek_is(&Token::Enum)
                    || self.peek_is(&Token::Component) || self.peek_is(&Token::Typealias)
                    || self.peek_is(&Token::Import)
                    || self.peek_is(&Token::Spec)
                    || self.peek_is(&Token::Describe)
                {
                    items.push(self.parse_item()?);
                } else {
                    // let/var at top level: collect into synthetic main
                    top_stmts.push(self.parse_stmt()?);
                }
            } else {
                // Expression statement at top level
                top_stmts.push(self.parse_stmt()?);
            }
        }

        // If there are top-level statements, wrap them in a synthetic main function
        if !top_stmts.is_empty() {
            let saved_span = top_stmts.first().unwrap().span.clone();
            items.push(Node::new(
                Item::Function(Function {
                    name: "main".to_string(),
                    params: vec![],
                    return_type: None,
                    body: Block { statements: top_stmts },
                    is_async: false,
                    doc_comment: None,
                    bc_reason: None,
                }),
                saved_span,
            ));
        }

        Ok(Program { items })
    }

    // ==================== ITEMS ====================

    fn parse_item(&mut self) -> Result<Node<Item>> {
        if self.peek_is(&Token::Func) {
            self.parse_func_def()
        } else if self.peek_is(&Token::Async) {
            self.parse_async_func_def()
        } else if self.peek_is(&Token::Class) {
            self.parse_class_def()
        } else if self.peek_is(&Token::Enum) {
            self.parse_enum_def()
        } else if self.peek_is(&Token::Component) {
            self.parse_component_def()
        } else if self.peek_is(&Token::Typealias) {
            self.parse_typealias_def()
        } else if self.peek_is(&Token::Import) {
            self.parse_import()
        } else if self.peek_is(&Token::Spec) || self.peek_is(&Token::Describe) {
            self.parse_spec()
        } else {
            Err(self.error("expected a top-level definition"))
        }
    }

    fn parse_func_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "func"
        let name = self.expect_identifier()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = self.parse_return_type()?;
        let bc_reason = self.parse_bc_annotation()?;
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Function(Function {
                name,
                params,
                return_type,
                body,
                is_async: false,
                doc_comment: None,
                bc_reason,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_async_func_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "async"
        self.expect(&Token::Func)?;
        let name = self.expect_identifier()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = self.parse_return_type()?;
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Function(Function {
                name,
                params,
                return_type,
                body,
                is_async: true,
                doc_comment: None,
                bc_reason: None,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        if !self.peek_is(&Token::RParen) {
            params.push(self.parse_param()?);
            while self.peek_is(&Token::Comma) {
                self.advance(); // ","
                if self.peek_is(&Token::RParen) {
                    break;
                }
                params.push(self.parse_param()?);
            }
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param> {
        let is_rest = self.peek_is(&Token::Ellipsis);
        if is_rest {
            self.advance(); // "..."
        }
        let name = self.expect_identifier()?;
        let type_ann = if self.peek_is(&Token::Colon) {
            self.advance(); // ":"
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        Ok(Param {
            name,
            type_ann,
            is_rest,
        })
    }

    fn parse_return_type(&mut self) -> Result<Option<TypeExpr>> {
        if self.peek_is(&Token::Arrow) {
            self.advance(); // "->"
            Ok(Some(self.parse_type_expr()?))
        } else {
            Ok(None)
        }
    }

    fn parse_bc_annotation(&mut self) -> Result<Option<String>> {
        if self.peek_is(&Token::Bc) || self.peek_is(&Token::Because) {
            self.advance(); // "bc" or "because"
            let reason = if self.is_string_lit() {
                let (_, val) = self.expect_string()?;
                Some(val)
            } else {
                None
            };
            Ok(reason)
        } else {
            Ok(None)
        }
    }

    // ==================== TYPES ====================

    fn parse_type_expr(&mut self) -> Result<TypeExpr> {
        if self.peek_is(&Token::LParen) {
            self.advance(); // "("
            let mut types = Vec::new();
            types.push(self.parse_type_expr()?);
            while self.peek_is(&Token::Comma) {
                self.advance(); // ","
                types.push(self.parse_type_expr()?);
            }
            self.expect(&Token::RParen)?;
            if types.len() == 1 {
                Ok(types.into_iter().next().unwrap())
            } else {
                Ok(TypeExpr::Tuple(types))
            }
        } else if self.peek_is(&Token::LBracket) {
            self.advance(); // "["
            let inner = self.parse_type_expr()?;
            self.expect(&Token::RBracket)?;
            Ok(TypeExpr::Array(Box::new(inner)))
        } else if self.is_identifier() {
            let name = self.expect_identifier()?;
            self.parse_named_type(name)
        } else {
            Err(self.error("expected a type"))
        }
    }

    fn parse_named_type(&mut self, name: String) -> Result<TypeExpr> {
        // Handle generic types: Option<T>, Result<T, E>
        if self.peek_is(&Token::Lt) {
            // This is "<" which we also parse, but our tokenizer uses it for less-than
            // For simplicity, handle known generic types
            match name.as_str() {
                "Result" | "Option" | "Array" => {
                    self.advance(); // "<"
                    let mut params = Vec::new();
                    params.push(self.parse_type_expr()?);
                    while self.peek_is(&Token::Comma) {
                        self.advance(); // ","
                        params.push(self.parse_type_expr()?);
                    }
                    self.expect(&Token::Gt)?;
                    Ok(TypeExpr::Generic(name, params))
                }
                _ => Ok(TypeExpr::Named(name)),
            }
        } else if self.peek_is(&Token::Question) {
            self.advance(); // "?"
            Ok(TypeExpr::Option(Box::new(TypeExpr::Named(name))))
        } else {
            Ok(TypeExpr::Named(name))
        }
    }

    // ==================== CLASSES ====================

    fn parse_class_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "class"
        let name = self.expect_identifier()?;
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.peek_is(&Token::RBrace) && self.pos < self.tokens.len() {
            if self.peek_is(&Token::Func) {
                if let Item::Function(f) = self.parse_func_in_class()?.value {
                    methods.push(f);
                }
            } else {
                fields.push(self.parse_class_field()?);
            }
        }
        self.expect(&Token::RBrace)?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Class(Class {
                name,
                fields,
                methods,
                doc_comment: None,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_class_field(&mut self) -> Result<ClassField> {
        let is_mutable = if self.peek_is(&Token::Var) {
            self.advance(); // "var"
            true
        } else {
            self.expect(&Token::Let)?;
            false
        };
        let name = self.expect_identifier()?;
        let type_ann = if self.peek_is(&Token::Colon) {
            self.advance(); // ":"
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        Ok(ClassField {
            name,
            is_mutable,
            type_ann,
        })
    }

    fn parse_func_in_class(&mut self) -> Result<Node<Item>> {
        self.parse_func_def()
    }

    // ==================== ENUMS ====================

    fn parse_enum_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "enum"
        let name = self.expect_identifier()?;
        self.expect(&Token::LBrace)?;
        let mut variants = Vec::new();
        while !self.peek_is(&Token::RBrace) && self.pos < self.tokens.len() {
            variants.push(self.parse_enum_variant()?);
            if self.peek_is(&Token::Comma) {
                self.advance(); // ","
            }
        }
        self.expect(&Token::RBrace)?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Enum(Enum {
                name,
                variants,
                doc_comment: None,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant> {
        let name = self.expect_identifier()?;
        let fields = if self.peek_is(&Token::LParen) {
            self.advance(); // "("
            let mut fields = Vec::new();
            if !self.peek_is(&Token::RParen) {
                fields.push(self.parse_enum_field()?);
                while self.peek_is(&Token::Comma) {
                    self.advance(); // ","
                    fields.push(self.parse_enum_field()?);
                }
            }
            self.expect(&Token::RParen)?;
            fields
        } else {
            vec![]
        };
        Ok(EnumVariant { name, fields })
    }

    fn parse_enum_field(&mut self) -> Result<EnumField> {
        let name = self.expect_identifier()?;
        let type_ann = if self.peek_is(&Token::Colon) {
            self.advance(); // ":"
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        Ok(EnumField { name, type_ann })
    }

    // ==================== COMPONENTS ====================

    fn parse_component_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "component"
        let name = self.expect_identifier()?;
        self.expect(&Token::LBrace)?;
        let mut state_vars = Vec::new();
        while self.peek_is(&Token::State) {
            state_vars.push(self.parse_state_var()?);
        }
        let body = if self.peek_is(&Token::Let)
            || self.peek_is(&Token::Var)
            || self.peek_is(&Token::If)
            || self.peek_is(&Token::For)
            || self.peek_is(&Token::While)
            || self.peek_is(&Token::Match)
            || self.peek_is(&Token::Try)
            || self.peek_is(&Token::Bc)
            || self.peek_is(&Token::Only)
            || self.peek_is(&Token::Unsafe)
            || self.peek_is(&Token::Return)
            || self.is_identifier()
            || self.is_int_lit()
            || self.is_float_lit()
            || self.is_string_lit()
            || self.is_char_lit()
            || self.peek_is(&Token::True)
            || self.peek_is(&Token::False)
            || self.peek_is(&Token::Nil)
            || self.peek_is(&Token::LParen)
            || self.peek_is(&Token::LBracket)
            || self.peek_is(&Token::LBrace)
        {
            // Component bodies use implicit block (no extra braces)
            let stmts = self.parse_statements_until_rbrace()?;
            Block { statements: stmts }
        } else {
            Block {
                statements: vec![],
            }
        };
        self.expect(&Token::RBrace)?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Component(Component {
                name,
                params: vec![],
                state_vars,
                body,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_state_var(&mut self) -> Result<StateVar> {
        self.expect(&Token::State)?;
        let name = self.expect_identifier()?;
        self.expect(&Token::Assign)?;
        let initial_value = self.parse_expr()?;
        Ok(StateVar {
            name,
            initial_value: Some(initial_value),
        })
    }

    // ==================== TYPE ALIAS ====================

    fn parse_typealias_def(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "typealias"
        let name = self.expect_identifier()?;
        self.expect(&Token::Assign)?;
        let type_expr = self.parse_type_expr()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::TypeAlias(TypeAlias { name, type_expr }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    // ==================== IMPORT ====================

    fn parse_import(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "import"
        let (_, path) = self.expect_string()?;

        // Optional: `as Alias`
        let alias = if self.peek_is(&Token::As) {
            self.advance(); // consume "as"
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Import(path, alias),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    // ==================== SPEC ====================

    /// Parse: `spec "name" { feat "desc" { ... } ... }`
    fn parse_spec(&mut self) -> Result<Node<Item>> {
        let start = self.advance_span_start(); // "spec" or "describe"
        let (_, name) = self.expect_string()?;
        self.expect(&Token::LBrace)?;
        let mut feats = Vec::new();
        while !self.peek_is(&Token::RBrace) && self.pos < self.tokens.len() {
            if self.peek_is(&Token::Feat) || self.peek_is(&Token::It) {
                feats.push(self.parse_feat()?);
            } else {
                return Err(self.error("expected `feat` or `it` inside spec"));
            }
        }
        self.expect(&Token::RBrace)?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Item::Spec(Spec {
                name,
                feats,
            }),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    /// Parse: `feat "description" { ... }` or `it "description" { ... }`
    fn parse_feat(&mut self) -> Result<Feat> {
        let _start = self.advance_span_start(); // "feat" or "it"
        let (_, name) = self.expect_string()?;
        let body = self.parse_block()?;
        Ok(Feat {
            name,
            body,
        })
    }

    // ==================== BLOCKS & STATEMENTS ====================

    fn parse_block(&mut self) -> Result<Block> {
        self.expect(&Token::LBrace)?;
        let stmts = self.parse_statements_until_rbrace()?;
        self.expect(&Token::RBrace)?;
        Ok(Block { statements: stmts })
    }

    fn parse_statements_until_rbrace(&mut self) -> Result<Vec<Node<Stmt>>> {
        let mut stmts = Vec::new();
        while !self.peek_is(&Token::RBrace) && self.pos < self.tokens.len() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Node<Stmt>> {
        if self.peek_is(&Token::Let) {
            self.parse_let_stmt()
        } else if self.peek_is(&Token::Var) {
            self.parse_var_stmt()
        } else if self.peek_is(&Token::If) {
            self.parse_if_stmt()
        } else if self.peek_is(&Token::For) {
            self.parse_for_stmt()
        } else if self.peek_is(&Token::While) {
            self.parse_while_stmt()
        } else if self.peek_is(&Token::Return) {
            self.parse_return_stmt()
        } else if self.peek_is(&Token::Match) {
            self.parse_match_stmt()
        } else if self.peek_is(&Token::Try) {
            self.parse_try_catch_stmt()
        } else if self.peek_is(&Token::Bc) || self.peek_is(&Token::Because) {
            self.parse_bc_assert_stmt()
        } else if self.peek_is(&Token::Only) {
            self.parse_only_guard_stmt()
        } else if self.peek_is(&Token::Unsafe) {
            self.parse_unsafe_stmt()
        } else if self.peek_is(&Token::Expect) {
            self.parse_expect_stmt()
        } else if self.peek_is(&Token::Todo) {
            self.parse_todo_stmt()
        } else if self.peek_is(&Token::KwQuestion) {
            self.parse_question_stmt()
        } else if self.peek_is(&Token::Bench) || self.peek_is(&Token::Bm) {
            self.parse_bench_stmt()
        } else {
            self.parse_expr_stmt()
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "let"
        let is_only = false;
        let name = self.expect_identifier()?;
        let type_ann = if self.peek_is(&Token::Colon) {
            self.advance(); // ":"
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let value = if self.peek_is(&Token::Assign) {
            self.advance(); // "="
            Some(self.parse_expr()?)
        } else {
            None
        };
        let bc_reason = self.parse_bc_suffix()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Let(Box::new(Node::new(
                Let {
                    name,
                    type_ann,
                    value,
                    is_mutable: false,
                    is_only,
                    bc_reason,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_var_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "var"
        let name = self.expect_identifier()?;
        let type_ann = if self.peek_is(&Token::Colon) {
            self.advance(); // ":"
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let value = if self.peek_is(&Token::Assign) {
            self.advance(); // "="
            Some(self.parse_expr()?)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Let(Box::new(Node::new(
                Let {
                    name,
                    type_ann,
                    value,
                    is_mutable: true,
                    is_only: false,
                    bc_reason: None,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_bc_suffix(&mut self) -> Result<Option<String>> {
        if self.peek_is(&Token::Bc) || self.peek_is(&Token::Because) {
            self.advance(); // "bc" or "because"
            let (_, val) = self.expect_string()?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    fn parse_expr_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = if self.pos < self.tokens.len() {
            self.tokens[self.pos].0
        } else {
            return Err(self.error("expected expression"));
        };
        let expr = self.parse_expr()?;
        let (_, end) = self.last_span();

        // Check for assignment (ident = expr)
        if self.peek_is(&Token::Assign) {
            self.advance(); // "="
            let value = self.parse_expr()?;
            let (_, end2) = self.last_span();
            return Ok(Node::new(
                Stmt::Assign(Box::new(Node::new(
                    Assign {
                        target: Box::new(Node::new(expr, crate::error::SourceSpan::new(start, start))),
                        value: Box::new(Node::new(value, crate::error::SourceSpan::new(start, end2 - start))),
                    },
                    crate::error::SourceSpan::new(start, end2 - start),
                ))),
                crate::error::SourceSpan::new(start, end2 - start),
            ));
        }

        Ok(Node::new(
            Stmt::Expr(Box::new(Node::new(expr, crate::error::SourceSpan::new(start, end - start)))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_return_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "return"
        let value = if self.pos < self.tokens.len()
            && !self.peek_is(&Token::RBrace)
        {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Return(value.map(|e| Box::new(Node::new(e, crate::error::SourceSpan::new(start, end - start))))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_if_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "if"
        let condition = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.peek_is(&Token::Else) {
            self.advance(); // "else"
            Some(self.parse_block()?)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::If(Box::new(Node::new(
                If {
                    condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                    then_block,
                    else_block,
                    is_expression: false,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_for_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "for"
        let variable = self.expect_identifier()?;
        self.expect(&Token::In)?; // TODO: need "in" keyword token
        // Actually `for item in ...` — we parse "in" as an identifier for now
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::For(Box::new(Node::new(
                For {
                    variable,
                    iterable: Box::new(Node::new(iterable, crate::error::SourceSpan::new(start, end - start))),
                    body,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_while_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "while"
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::While(Box::new(Node::new(
                While {
                    condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                    body,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_match_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "match"
        let value = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.peek_is(&Token::RBrace) && self.pos < self.tokens.len() {
            self.expect(&Token::Case)?;
            let pattern = self.parse_pattern()?;
            self.expect(&Token::Arrow)?;
            let body = self.parse_block()?;
            arms.push(MatchArm { pattern, body });
        }
        self.expect(&Token::RBrace)?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Match(Box::new(Node::new(
                Match {
                    value: Box::new(Node::new(value, crate::error::SourceSpan::new(start, end - start))),
                    arms,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        if self.peek_is(&Token::Only) {
            self.advance(); // "only"
            let name = self.expect_identifier()?;
            Ok(Pattern::OnlyType(name))
        } else if self.is_identifier() {
            let name = self.expect_identifier()?;
            if self.peek_is(&Token::LParen) {
                self.advance(); // "("
                let mut bindings = Vec::new();
                if !self.peek_is(&Token::RParen) {
                    bindings.push(self.expect_identifier()?);
                    while self.peek_is(&Token::Comma) {
                        self.advance(); // ","
                        bindings.push(self.expect_identifier()?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(Pattern::EnumVariant { name, bindings })
            } else {
                Ok(Pattern::Binding(name))
            }
        } else {
            Err(self.error("expected pattern"))
        }
    }

    fn parse_try_catch_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "try"
        let try_block = self.parse_block()?;
        self.expect(&Token::Catch)?;
        let catch_pattern = if self.is_identifier() || self.peek_is(&Token::Only) || self.peek_is(&Token::LBrace) {
            if !self.peek_is(&Token::LBrace) {
                Some(self.parse_pattern()?)
            } else {
                None
            }
        } else {
            None
        };
        let catch_block = self.parse_block()?;
        let finally_block = if self.peek_is(&Token::Finally) {
            self.advance(); // "finally"
            Some(self.parse_block()?)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::TryCatch(Box::new(Node::new(
                TryCatch {
                    try_block,
                    catch_pattern,
                    catch_block,
                    finally_block,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_bc_assert_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "bc" or "because"
        let condition = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let (_, message) = self.expect_string()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::BcAssert(Box::new(Node::new(
                BcAssert {
                    condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                    message,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_only_guard_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "only"
        let condition = self.parse_expr()?;
        self.expect(&Token::Do)?;
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::OnlyGuard(Box::new(Node::new(
                OnlyGuard {
                    condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                    body,
                },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_unsafe_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "unsafe"
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::UnsafeBlock(Box::new(Node::new(
                UnsafeBlock { body },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_expect_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "expect"
        let expr = self.parse_expr()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Expect(Box::new(Node::new(
                Expect { expr: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))) },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_todo_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "todo"
        let message = if self.is_string_lit() {
            let (_, msg) = self.expect_string()?;
            Some(msg)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Todo(Box::new(Node::new(
                Todo { message },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_question_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "question"
        let message = if self.is_string_lit() {
            let (_, msg) = self.expect_string()?;
            Some(msg)
        } else {
            None
        };
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Question(Box::new(Node::new(
                Question { message },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    fn parse_bench_stmt(&mut self) -> Result<Node<Stmt>> {
        let start = self.advance_span_start(); // "bench" or "bm"
        let body = self.parse_block()?;
        let (_, end) = self.last_span();
        Ok(Node::new(
            Stmt::Bench(Box::new(Node::new(
                Bench { body },
                crate::error::SourceSpan::new(start, end - start),
            ))),
            crate::error::SourceSpan::new(start, end - start),
        ))
    }

    // ==================== EXPRESSIONS ====================

    pub fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_if_then_else()
    }

    fn parse_if_then_else(&mut self) -> Result<Expr> {
        if self.peek_is(&Token::If) {
            let start = self.advance_span_start(); // "if"
            let condition = self.parse_or_expr()?;
            if self.peek_is(&Token::Then) {
                self.advance(); // "then"
                let then_expr = self.parse_or_expr()?;
                let else_expr = if self.peek_is(&Token::Else) {
                    self.advance(); // "else"
                    Some(Box::new(Node::new(self.parse_or_expr()?, crate::error::SourceSpan::new(0,0))))
                } else {
                    None
                };
                let (_, end) = self.last_span();
                return Ok(Expr::IfThenElse {
                    condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                    then_expr: Box::new(Node::new(then_expr, crate::error::SourceSpan::new(start, end - start))),
                    else_expr,
                });
            }
            // Not an if-then-else, rewind? For now, fall through to or_expr
            // Actually we consumed "if" already. If no "then" follows, it's an if-stmt not expression.
            // We need to backtrack. Since we already consumed "if", handle this:
            // Reconstruct the condition and parse the rest
            let (_, end) = self.last_span();
            return Ok(Expr::IfThenElse {
                condition: Box::new(Node::new(condition, crate::error::SourceSpan::new(start, end - start))),
                then_expr: Box::new(Node::new(Expr::Literal(Node::new(Literal::Nil, crate::error::SourceSpan::new(start, end - start))), crate::error::SourceSpan::new(start, end - start))),
                else_expr: None,
            });
        }
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr> {
        let mut left = self.parse_and_expr()?;
        while self.peek_is(&Token::OrOr) {
            let op = BinaryOpKind::Or;
            self.advance(); // "||"
            let right = self.parse_and_expr()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr> {
        let mut left = self.parse_cmp_expr()?;
        while self.peek_is(&Token::AndAnd) {
            let op = BinaryOpKind::And;
            self.advance(); // "&&"
            let right = self.parse_cmp_expr()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            };
        }
        Ok(left)
    }

    fn parse_cmp_expr(&mut self) -> Result<Expr> {
        let left = self.parse_add_expr()?;
        if self.peek_is(&Token::EqEq) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Eq,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::NotEq) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Ne,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::Lt) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Lt,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::Gt) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Gt,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::LtEq) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Le,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::GtEq) {
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr::BinaryOp {
                op: BinaryOpKind::Ge,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_add_expr(&mut self) -> Result<Expr> {
        let mut left = self.parse_mul_expr()?;
        while self.peek_is(&Token::Plus) || self.peek_is(&Token::Minus) {
            let op = if self.peek_is(&Token::Plus) {
                self.advance();
                BinaryOpKind::Add
            } else {
                self.advance();
                BinaryOpKind::Sub
            };
            let right = self.parse_mul_expr()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            };
        }
        Ok(left)
    }

    fn parse_mul_expr(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary_expr()?;
        while self.peek_is(&Token::Star) || self.peek_is(&Token::Slash) || self.peek_is(&Token::Percent) {
            let op = if self.peek_is(&Token::Star) {
                self.advance();
                BinaryOpKind::Mul
            } else if self.peek_is(&Token::Slash) {
                self.advance();
                BinaryOpKind::Div
            } else {
                self.advance();
                BinaryOpKind::Mod
            };
            let right = self.parse_unary_expr()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(Node::new(left, crate::error::SourceSpan::new(0, 0))),
                right: Box::new(Node::new(right, crate::error::SourceSpan::new(0, 0))),
            };
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr> {
        if self.peek_is(&Token::Minus) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            Ok(Expr::UnaryOp {
                op: UnaryOpKind::Neg,
                operand: Box::new(Node::new(operand, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::Bang) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            Ok(Expr::UnaryOp {
                op: UnaryOpKind::Not,
                operand: Box::new(Node::new(operand, crate::error::SourceSpan::new(0, 0))),
            })
        } else if self.peek_is(&Token::Ellipsis) || self.peek_is(&Token::Ellipsis2) {
            self.advance(); // "..." or "…"
            let operand = self.parse_postfix_expr()?;
            Ok(Expr::Spread(Box::new(Node::new(operand, crate::error::SourceSpan::new(0, 0)))))
        } else {
            self.parse_postfix_expr()
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.peek_is(&Token::LParen) {
                // Function call
                self.advance(); // "("
                let mut args = Vec::new();
                if !self.peek_is(&Token::RParen) {
                    args.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                    while self.peek_is(&Token::Comma) {
                        self.advance(); // ","
                        if self.peek_is(&Token::RParen) {
                            break;
                        }
                        args.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                    }
                }
                self.expect(&Token::RParen)?;
                expr = Expr::Call {
                    callee: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                    args,
                };
            } else if self.peek_is(&Token::Dot) {
                self.advance(); // "."
                let method = self.expect_identifier()?;
                if self.peek_is(&Token::LParen) {
                    // Method call
                    self.advance(); // "("
                    let mut args = Vec::new();
                    if !self.peek_is(&Token::RParen) {
                        args.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                        while self.peek_is(&Token::Comma) {
                            self.advance(); // ","
                            if self.peek_is(&Token::RParen) {
                                break;
                            }
                            args.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                        }
                    }
                    self.expect(&Token::RParen)?;
                    expr = Expr::MethodCall {
                        object: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                        method,
                        args,
                    };
                } else {
                    expr = Expr::MemberAccess {
                        target: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                        field: method,
                    };
                }
            } else if self.peek_is(&Token::LBracket) {
                self.advance(); // "["
                let index = self.parse_expr()?;
                self.expect(&Token::RBracket)?;
                expr = Expr::Index {
                    target: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                    index: Box::new(Node::new(index, crate::error::SourceSpan::new(0, 0))),
                };
            } else if self.peek_is(&Token::Question) {
                self.advance(); // "?"
                expr = Expr::ErrorPropagate(Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))));
            } else {
                break;
            }
        }

        // Check for bc annotation
        if self.peek_is(&Token::Bc) || self.peek_is(&Token::Because) {
            self.advance();
            let (_, reason) = self.expect_string()?;
            expr = Expr::BcAnnotation {
                expr: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                reason,
            };
        }

        // Check for range
        if self.peek_is(&Token::Ellipsis) {
            let start_tok = self.tokens[self.pos].1.clone();
            if let Token::Ellipsis = start_tok {
                self.advance(); // "..."
                let end = self.parse_postfix_expr()?;
                expr = Expr::Range {
                    start: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                    end: Box::new(Node::new(end, crate::error::SourceSpan::new(0, 0))),
                    inclusive: true,
                };
            }
        } else if self.peek_is(&Token::DotDot) {
            self.advance(); // ".."
            let end = self.parse_postfix_expr()?;
            expr = Expr::Range {
                start: Box::new(Node::new(expr, crate::error::SourceSpan::new(0, 0))),
                end: Box::new(Node::new(end, crate::error::SourceSpan::new(0, 0))),
                inclusive: false,
            };
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr> {
        if self.is_int_lit() {
            let (start, val) = self.expect_int()?;
            let (_, end) = self.last_span();
            Ok(Expr::Literal(Node::new(Literal::Int(val), crate::error::SourceSpan::new(start, 1))))
        } else if self.is_float_lit() {
            let (start, val) = self.expect_float()?;
            let (_, end) = self.last_span();
            Ok(Expr::Literal(Node::new(Literal::Float(val), crate::error::SourceSpan::new(start, 1))))
        } else if self.is_string_lit() {
            let (start, val) = self.expect_string()?;
            let (_, end) = self.last_span();
            Ok(Expr::Literal(Node::new(Literal::String(val), crate::error::SourceSpan::new(start, 1))))
        } else if self.is_char_lit() {
            let (start, val) = self.expect_char()?;
            let (_, end) = self.last_span();
            Ok(Expr::Literal(Node::new(Literal::Char(val), crate::error::SourceSpan::new(start, 1))))
        } else if self.peek_is(&Token::True) {
            let (start, _) = self.expect_token(&Token::True)?;
            Ok(Expr::Literal(Node::new(Literal::Bool(true), crate::error::SourceSpan::new(start, 4))))
        } else if self.peek_is(&Token::False) {
            let (start, _) = self.expect_token(&Token::False)?;
            Ok(Expr::Literal(Node::new(Literal::Bool(false), crate::error::SourceSpan::new(start, 5))))
        } else if self.peek_is(&Token::Nil) {
            let (start, _) = self.expect_token(&Token::Nil)?;
            Ok(Expr::Literal(Node::new(Literal::Nil, crate::error::SourceSpan::new(start, 3))))
        } else if self.is_identifier() {
            let name = self.expect_identifier()?;
            // Check for lambda
            if self.peek_is(&Token::Arrow) {
                self.advance(); // "->"
                let body = self.parse_expr()?;
                Ok(Expr::Lambda {
                    params: vec![Param {
                        name,
                        type_ann: None,
                        is_rest: false,
                    }],
                    body: Box::new(Node::new(body, crate::error::SourceSpan::new(0, 0))),
                })
            } else {
                Ok(Expr::Identifier(name))
            }
        } else if self.peek_is(&Token::LParen) {
            self.advance(); // "("
            // Check for lambda with param list
            if self.peek_at(1).map(|t| matches!(t, Token::Identifier(_))) == Some(true)
                && self.peek_at(2).map(|t| matches!(t, Token::Comma | Token::Colon | Token::Arrow)) == Some(true)
            {
                // Could be lambda params: (x) -> ...
                // But simpler: just parse expression
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                if self.peek_is(&Token::Arrow) {
                    self.advance(); // "->"
                    let body = self.parse_expr()?;
                    // Simple single-param lambda
                    if let Expr::Identifier(name) = &expr {
                        Ok(Expr::Lambda {
                            params: vec![Param {
                                name: name.clone(),
                                type_ann: None,
                                is_rest: false,
                            }],
                            body: Box::new(Node::new(body, crate::error::SourceSpan::new(0, 0))),
                        })
                    } else {
                        Ok(Expr::Lambda {
                            params: vec![],
                            body: Box::new(Node::new(body, crate::error::SourceSpan::new(0, 0))),
                        })
                    }
                } else {
                    // Check for range expression or just parenthesized expr
                    if self.peek_is(&Token::Ellipsis) || self.peek_is(&Token::DotDot) {
                        // Rewind? This is tricky with our simple approach
                    }
                    Ok(expr)
                }
            } else {
                // Tuple or parenthesized expression
                let expr = self.parse_expr()?;
                let mut items = vec![Node::new(expr, crate::error::SourceSpan::new(0, 0))];
                while self.peek_is(&Token::Comma) {
                    self.advance(); // ","
                    items.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                }
                self.expect(&Token::RParen)?;
                if items.len() == 1 {
                    Ok(items.into_iter().next().unwrap().value)
                } else {
                    Ok(Expr::Tuple(items))
                }
            }
        } else if self.peek_is(&Token::LBracket) {
            self.advance(); // "["
            let mut items = Vec::new();
            if !self.peek_is(&Token::RBracket) {
                items.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                while self.peek_is(&Token::Comma) {
                    self.advance(); // ","
                    if self.peek_is(&Token::RBracket) {
                        break;
                    }
                    items.push(Node::new(self.parse_expr()?, crate::error::SourceSpan::new(0, 0)));
                }
            }
            self.expect(&Token::RBracket)?;
            Ok(Expr::Array(items))
        } else if self.peek_is(&Token::LBrace) {
            self.parse_block_expr()
        } else {
            Err(self.error("expected expression"))
        }
    }

    fn parse_block_expr(&mut self) -> Result<Expr> {
        let block = self.parse_block()?;
        Ok(Expr::Block(block))
    }

    // ==================== TOKEN HELPERS ====================

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn advance_span_start(&mut self) -> usize {
        let start = if self.pos < self.tokens.len() {
            self.tokens[self.pos].0
        } else {
            0
        };
        self.pos += 1;
        start
    }

    fn peek_is(&self, token: &Token) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        let current = &self.tokens[self.pos].1;
        match (current, token) {
            (Token::Let, Token::Let) => true,
            (Token::Var, Token::Var) => true,
            (Token::Func, Token::Func) => true,
            (Token::If, Token::If) => true,
            (Token::Else, Token::Else) => true,
            (Token::Then, Token::Then) => true,
            (Token::For, Token::For) => true,
            (Token::In, Token::In) => true,
            (Token::While, Token::While) => true,
            (Token::Return, Token::Return) => true,
            (Token::Match, Token::Match) => true,
            (Token::Case, Token::Case) => true,
            (Token::Try, Token::Try) => true,
            (Token::Catch, Token::Catch) => true,
            (Token::Finally, Token::Finally) => true,
            (Token::Async, Token::Async) => true,
            (Token::Await, Token::Await) => true,
            (Token::Class, Token::Class) => true,
            (Token::Init, Token::Init) => true,
            (Token::Enum, Token::Enum) => true,
            (Token::Component, Token::Component) => true,
            (Token::State, Token::State) => true,
            (Token::Bc, Token::Bc) => true,
            (Token::Because, Token::Because) => true,
            (Token::Only, Token::Only) => true,
            (Token::Unsafe, Token::Unsafe) => true,
            (Token::Weak, Token::Weak) => true,
            (Token::Unowned, Token::Unowned) => true,
            (Token::Typealias, Token::Typealias) => true,
            (Token::Import, Token::Import) => true,
            (Token::As, Token::As) => true,
            (Token::True, Token::True) => true,
            (Token::False, Token::False) => true,
            (Token::Nil, Token::Nil) => true,
            (Token::Do, Token::Do) => true,
            (Token::Render, Token::Render) => true,
            (Token::Spec, Token::Spec) => true,
            (Token::Describe, Token::Describe) => true,
            (Token::Feat, Token::Feat) => true,
            (Token::It, Token::It) => true,
            (Token::Expect, Token::Expect) => true,
            (Token::Todo, Token::Todo) => true,
            (Token::KwQuestion, Token::KwQuestion) => true,
            (Token::Bench, Token::Bench) => true,
            (Token::Bm, Token::Bm) => true,
            (Token::Assign, Token::Assign) => true,
            (Token::Plus, Token::Plus) => true,
            (Token::Minus, Token::Minus) => true,
            (Token::Star, Token::Star) => true,
            (Token::Slash, Token::Slash) => true,
            (Token::Percent, Token::Percent) => true,
            (Token::EqEq, Token::EqEq) => true,
            (Token::NotEq, Token::NotEq) => true,
            (Token::Lt, Token::Lt) => true,
            (Token::Gt, Token::Gt) => true,
            (Token::LtEq, Token::LtEq) => true,
            (Token::GtEq, Token::GtEq) => true,
            (Token::AndAnd, Token::AndAnd) => true,
            (Token::OrOr, Token::OrOr) => true,
            (Token::Bang, Token::Bang) => true,
            (Token::Arrow, Token::Arrow) => true,
            (Token::Dot, Token::Dot) => true,
            (Token::Comma, Token::Comma) => true,
            (Token::Colon, Token::Colon) => true,
            (Token::Semicolon, Token::Semicolon) => true,
            (Token::LParen, Token::LParen) => true,
            (Token::RParen, Token::RParen) => true,
            (Token::LBracket, Token::LBracket) => true,
            (Token::RBracket, Token::RBracket) => true,
            (Token::LBrace, Token::LBrace) => true,
            (Token::RBrace, Token::RBrace) => true,
            (Token::Pipe, Token::Pipe) => true,
            (Token::Question, Token::Question) => true,
            (Token::Ellipsis, Token::Ellipsis) => true,
            (Token::Ellipsis2, Token::Ellipsis2) => true,
            (Token::DotDot, Token::DotDot) => true,
            (Token::LeftArrow, Token::LeftArrow) => true,
            (Token::Error, Token::Error) => true,
            _ => false,
        }
    }

    fn is_identifier(&self) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        matches!(self.tokens[self.pos].1, Token::Identifier(_))
    }

    fn is_int_lit(&self) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        matches!(self.tokens[self.pos].1, Token::IntLiteral(_))
    }

    fn is_float_lit(&self) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        matches!(self.tokens[self.pos].1, Token::FloatLiteral(_))
    }

    fn is_string_lit(&self) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        matches!(self.tokens[self.pos].1, Token::StringLiteral(_))
    }

    fn is_char_lit(&self) -> bool {
        if self.pos >= self.tokens.len() { return false; }
        matches!(self.tokens[self.pos].1, Token::CharLiteral(_))
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        let idx = self.pos + offset;
        if idx < self.tokens.len() {
            Some(&self.tokens[idx].1)
        } else {
            None
        }
    }

    fn last_span(&self) -> (usize, usize) {
        if self.pos > 0 && self.pos - 1 < self.tokens.len() {
            let tok = &self.tokens[self.pos - 1];
            (tok.0, tok.2)
        } else {
            (0, 0)
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(usize, String)> {
        if self.pos < self.tokens.len() {
            let (start, ref tok, end) = self.tokens[self.pos];
            if self.token_eq(tok, expected) {
                self.pos += 1;
                let repr = format!("{:?}", tok);
                Ok((start, repr))
            } else {
                Err(self.error(&format!("expected {:?}, got {:?}", expected, tok)))
            }
        } else {
            Err(self.error(&format!("expected {:?}, got end of input", expected)))
        }
    }

    fn token_eq(&self, a: &Token, b: &Token) -> bool {
        match (a, b) {
            (Token::Let, Token::Let) | (Token::Var, Token::Var) | (Token::Func, Token::Func)
            | (Token::If, Token::If) | (Token::Else, Token::Else) | (Token::Then, Token::Then)
            | (Token::For, Token::For) | (Token::In, Token::In) | (Token::While, Token::While)
            | (Token::Return, Token::Return) | (Token::Match, Token::Match)
            | (Token::Case, Token::Case) | (Token::Try, Token::Try) | (Token::Catch, Token::Catch)
            | (Token::Finally, Token::Finally) | (Token::Async, Token::Async)
            | (Token::Await, Token::Await) | (Token::Class, Token::Class)
            | (Token::Init, Token::Init) | (Token::Enum, Token::Enum)
            | (Token::Component, Token::Component) | (Token::State, Token::State)
            | (Token::Bc, Token::Bc) | (Token::Because, Token::Because)
            | (Token::Only, Token::Only) | (Token::Unsafe, Token::Unsafe)
            | (Token::Weak, Token::Weak) | (Token::Unowned, Token::Unowned)
            | (Token::Typealias, Token::Typealias) | (Token::Import, Token::Import) | (Token::As, Token::As)
            | (Token::True, Token::True) | (Token::False, Token::False)
            | (Token::Nil, Token::Nil) | (Token::Do, Token::Do)
            | (Token::Render, Token::Render)
            | (Token::Spec, Token::Spec) | (Token::Describe, Token::Describe)
            | (Token::Feat, Token::Feat) | (Token::It, Token::It)
            | (Token::Expect, Token::Expect)
            | (Token::Todo, Token::Todo) | (Token::KwQuestion, Token::KwQuestion)
            | (Token::Bench, Token::Bench) | (Token::Bm, Token::Bm)
            | (Token::Assign, Token::Assign) | (Token::Plus, Token::Plus)
            | (Token::Minus, Token::Minus) | (Token::Star, Token::Star)
            | (Token::Slash, Token::Slash) | (Token::Percent, Token::Percent)
            | (Token::EqEq, Token::EqEq) | (Token::NotEq, Token::NotEq)
            | (Token::Lt, Token::Lt) | (Token::Gt, Token::Gt)
            | (Token::LtEq, Token::LtEq) | (Token::GtEq, Token::GtEq)
            | (Token::AndAnd, Token::AndAnd) | (Token::OrOr, Token::OrOr)
            | (Token::Bang, Token::Bang) | (Token::Arrow, Token::Arrow)
            | (Token::Dot, Token::Dot) | (Token::Comma, Token::Comma)
            | (Token::Colon, Token::Colon) | (Token::Semicolon, Token::Semicolon)
            | (Token::LParen, Token::LParen) | (Token::RParen, Token::RParen)
            | (Token::LBracket, Token::LBracket) | (Token::RBracket, Token::RBracket)
            | (Token::LBrace, Token::LBrace) | (Token::RBrace, Token::RBrace)
            | (Token::Pipe, Token::Pipe) | (Token::Question, Token::Question)
            | (Token::Ellipsis, Token::Ellipsis) | (Token::Ellipsis2, Token::Ellipsis2) | (Token::DotDot, Token::DotDot)
            | (Token::LeftArrow, Token::LeftArrow) | (Token::Error, Token::Error) => true,
            (Token::Identifier(_), Token::Identifier(_))
            | (Token::IntLiteral(_), Token::IntLiteral(_))
            | (Token::FloatLiteral(_), Token::FloatLiteral(_))
            | (Token::StringLiteral(_), Token::StringLiteral(_))
            | (Token::CharLiteral(_), Token::CharLiteral(_)) => true,
            _ => false,
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        self.expect_token(expected)?;
        Ok(())
    }

    fn expect_identifier(&mut self) -> Result<String> {
        if self.pos < self.tokens.len() {
            match &self.tokens[self.pos].1 {
                Token::Identifier(name) => {
                    let name = name.clone();
                    self.pos += 1;
                    Ok(name)
                }
                other => Err(self.error(&format!("expected identifier, got {:?}", other))),
            }
        } else {
            Err(self.error("expected identifier, got end of input"))
        }
    }

    fn expect_int(&mut self) -> Result<(usize, i64)> {
        if self.pos < self.tokens.len() {
            match &self.tokens[self.pos].1 {
                Token::IntLiteral(val) => {
                    let start = self.tokens[self.pos].0;
                    let val = *val;
                    self.pos += 1;
                    Ok((start, val))
                }
                other => Err(self.error(&format!("expected integer literal, got {:?}", other))),
            }
        } else {
            Err(self.error("expected integer literal, got end of input"))
        }
    }

    fn expect_float(&mut self) -> Result<(usize, f64)> {
        if self.pos < self.tokens.len() {
            match &self.tokens[self.pos].1 {
                Token::FloatLiteral(val) => {
                    let start = self.tokens[self.pos].0;
                    let val = *val;
                    self.pos += 1;
                    Ok((start, val))
                }
                other => Err(self.error(&format!("expected float literal, got {:?}", other))),
            }
        } else {
            Err(self.error("expected float literal, got end of input"))
        }
    }

    fn expect_string(&mut self) -> Result<(usize, String)> {
        if self.pos < self.tokens.len() {
            match &self.tokens[self.pos].1 {
                Token::StringLiteral(val) => {
                    let start = self.tokens[self.pos].0;
                    let val = val.clone();
                    self.pos += 1;
                    Ok((start, val))
                }
                other => Err(self.error(&format!("expected string literal, got {:?}", other))),
            }
        } else {
            Err(self.error("expected string literal, got end of input"))
        }
    }

    fn expect_char(&mut self) -> Result<(usize, char)> {
        if self.pos < self.tokens.len() {
            match &self.tokens[self.pos].1 {
                Token::CharLiteral(val) => {
                    let start = self.tokens[self.pos].0;
                    let val = *val;
                    self.pos += 1;
                    Ok((start, val))
                }
                other => Err(self.error(&format!("expected char literal, got {:?}", other))),
            }
        } else {
            Err(self.error("expected char literal, got end of input"))
        }
    }

    fn error(&self, msg: &str) -> crate::error::CompileError {
        let span = if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            Some(crate::error::SourceSpan::new(tok.0, tok.2 - tok.0))
        } else {
            None
        };
        let mut err = crate::error::CompileError::new(msg);
        err.span = span;
        err
    }
}
