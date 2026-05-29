use crate::ast as A;
use crate::error::SourceSpan;

/// High-level IR — a typed, simplified version of the AST.
/// This is produced after type-checking and ownership analysis.
#[derive(Debug, Clone)]
pub struct HirProgram {
    pub items: Vec<HirItem>,
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Function(HirFunction),
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: HirType,
    pub body: HirBlock,
    pub is_async: bool,
    pub is_lazy: bool,
    pub is_private: bool,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: HirType,
    pub is_rest: bool,
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub first_line: u32,
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let {
        name: String,
        ty: HirType,
        value: Option<HirExpr>,
        is_mutable: bool,
        is_only: bool,
        is_lazy: bool,
        line: u32,
    },
    Assign {
        target: HirExpr,
        value: HirExpr,
        line: u32,
    },
    Expr(HirExpr, u32),
    Return(Option<HirExpr>, u32),
    If {
        condition: HirExpr,
        then_block: HirBlock,
        else_block: Option<HirBlock>,
        line: u32,
    },
    For {
        variable: String,
        iterable: HirExpr,
        body: HirBlock,
        line: u32,
    },
    While {
        condition: HirExpr,
        body: HirBlock,
        line: u32,
    },
    Match {
        value: HirExpr,
        arms: Vec<HirMatchArm>,
        line: u32,
    },
    Bench(HirBlock, u32),
}

#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub body: HirBlock,
}

#[derive(Debug, Clone)]
pub enum HirExpr {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    CharLit(char),
    NilLit,
    Ident(String),
    BinaryOp {
        op: A::BinaryOpKind,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    UnaryOp {
        op: A::UnaryOpKind,
        operand: Box<HirExpr>,
    },
    Call {
        callee: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    MethodCall {
        object: Box<HirExpr>,
        method: String,
        args: Vec<HirExpr>,
    },
    Lambda {
        params: Vec<HirParam>,
        body: Box<HirExpr>,
    },
    Block(HirBlock),
    Array(Vec<HirExpr>),
    Tuple(Vec<HirExpr>),
    IfThenElse {
        condition: Box<HirExpr>,
        then_expr: Box<HirExpr>,
        else_expr: Option<Box<HirExpr>>,
    },
    Range {
        start: Box<HirExpr>,
        end: Box<HirExpr>,
        inclusive: bool,
    },
    Spread(Box<HirExpr>),
    ErrorPropagate(Box<HirExpr>),
    BcAnnotation {
        expr: Box<HirExpr>,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Int,
    Float,
    Bool,
    String,
    Char,
    Nil,
    Array(Box<HirType>),
    Option(Box<HirType>),
    Result(Box<HirType>, Box<HirType>),
    Function(Vec<HirType>, Box<HirType>),
    Tuple(Vec<HirType>),
    Named(String),
}

/// Lower AST to HIR.
pub fn lower(program: &A::Program, source: &str) -> HirProgram {
    let mut lowerer = Lowerer { source };
    lowerer.lower_program(program)
}

struct Lowerer<'a> {
    source: &'a str,
}

impl<'a> Lowerer<'a> {
    fn line_of(&self, span: &SourceSpan) -> u32 {
        self.source[..span.offset].lines().count() as u32 + 1
    }
    fn lower_program(&mut self, program: &A::Program) -> HirProgram {
        let mut items = Vec::new();
        for item in &program.items {
            match &item.value {
                A::Item::Function(f) => {
                    items.push(HirItem::Function(self.lower_function(f)));
                }
                A::Item::Class(c) => {
                    // Lower class methods as standalone functions
                    for method in &c.methods {
                        items.push(HirItem::Function(self.lower_function(method)));
                    }
                }
                _ => {}
            }
        }
        HirProgram { items }
    }

    fn lower_function(&mut self, f: &A::Function) -> HirFunction {
        let line = f.body.statements.first()
            .map(|s| self.line_of(&s.span))
            .unwrap_or(1);
        HirFunction {
            name: f.name.clone(),
            params: f.params.iter().map(|p| self.lower_param(p)).collect(),
            return_type: f
                .return_type
                .as_ref()
                .map(|t| self.lower_type_expr(t))
                .unwrap_or(HirType::Nil),
            body: self.lower_block(&f.body),
            is_async: f.is_async,
            is_lazy: f.is_lazy,
            is_private: f.is_private,
            line,
        }
    }

    fn lower_param(&mut self, p: &A::Param) -> HirParam {
        HirParam {
            name: p.name.clone(),
            ty: p
                .type_ann
                .as_ref()
                .map(|t| self.lower_type_expr(t))
                .unwrap_or(HirType::Nil),
            is_rest: p.is_rest,
        }
    }

    fn lower_type_expr(&mut self, t: &A::TypeExpr) -> HirType {
        match t {
            A::TypeExpr::Named(name) => HirType::Named(name.clone()),
            A::TypeExpr::Generic(_, _) => HirType::Named("generic".into()),
            A::TypeExpr::Array(inner) => HirType::Array(Box::new(self.lower_type_expr(inner))),
            A::TypeExpr::Option(inner) => HirType::Option(Box::new(self.lower_type_expr(inner))),
            A::TypeExpr::Result(ok, err) => HirType::Result(
                Box::new(self.lower_type_expr(ok)),
                Box::new(self.lower_type_expr(err)),
            ),
            A::TypeExpr::Union(_) => HirType::Named("union".into()),
            A::TypeExpr::Function(params, ret) => HirType::Function(
                params.iter().map(|p| self.lower_type_expr(p)).collect(),
                Box::new(self.lower_type_expr(ret)),
            ),
            A::TypeExpr::Tuple(ts) => {
                HirType::Tuple(ts.iter().map(|t| self.lower_type_expr(t)).collect())
            }
            A::TypeExpr::Record(_) => HirType::Named("record".into()),
            A::TypeExpr::Infer => HirType::Named("infer".into()),
        }
    }

    fn lower_block(&mut self, block: &A::Block) -> HirBlock {
        let first_line = block.statements.first()
            .map(|s| self.line_of(&s.span))
            .unwrap_or(1);
        HirBlock {
            stmts: block.statements.iter().map(|s| self.lower_stmt(s)).collect(),
            first_line,
        }
    }

    fn lower_stmt(&mut self, stmt: &A::Node<A::Stmt>) -> HirStmt {
        let line = self.line_of(&stmt.span);
        match &stmt.value {
            A::Stmt::Let(boxed) => {
                let ls = &boxed.value;
                HirStmt::Let {
                name: ls.name.clone(),
                ty: ls
                    .type_ann
                    .as_ref()
                    .map(|t| self.lower_type_expr(t))
                    .unwrap_or(HirType::Named("infer".into())),
                value: ls.value.as_ref().map(|v| self.lower_expr(v)),
                is_mutable: ls.is_mutable,
                is_only: ls.is_only,
                is_lazy: ls.is_lazy,
                line,
            }},
            A::Stmt::Expr(boxed) => HirStmt::Expr(self.lower_expr_from_node(boxed), line),
            A::Stmt::Return(ret) => {
                HirStmt::Return(ret.as_ref().map(|e| self.lower_expr_from_node(e)), line)
            }
            A::Stmt::Assign(boxed) => {
                let a = &boxed.value;
                HirStmt::Assign {
                    target: self.lower_expr_from_node(&a.target),
                    value: self.lower_expr_from_node(&a.value),
                    line,
                }
            }
            A::Stmt::BcAssert(boxed) => {
                let ba = &boxed.value;
                HirStmt::Expr(HirExpr::BcAnnotation {
                    expr: Box::new(self.lower_expr_from_node(&ba.condition)),
                    reason: ba.message.clone(),
                }, line)
            }
            A::Stmt::If(boxed) => {
                let ifs = &boxed.value;
                HirStmt::If {
                    condition: self.lower_expr_from_node(&ifs.condition),
                    then_block: self.lower_block(&ifs.then_block),
                    else_block: ifs.else_block.as_ref().map(|b| self.lower_block(b)),
                    line,
                }
            }
            A::Stmt::For(boxed) => {
                let fs = &boxed.value;
                HirStmt::For {
                    variable: fs.variable.clone(),
                    iterable: self.lower_expr_from_node(&fs.iterable),
                    body: self.lower_block(&fs.body),
                    line,
                }
            }
            A::Stmt::While(boxed) => {
                let ws = &boxed.value;
                HirStmt::While {
                    condition: self.lower_expr_from_node(&ws.condition),
                    body: self.lower_block(&ws.body),
                    line,
                }
            }
            A::Stmt::Match(boxed) => {
                let ms = &boxed.value;
                HirStmt::Match {
                    value: self.lower_expr_from_node(&ms.value),
                    arms: ms
                        .arms
                        .iter()
                        .map(|arm| HirMatchArm {
                            body: self.lower_block(&arm.body),
                        })
                        .collect(),
                    line,
                }
            }
            A::Stmt::TryCatch(boxed) => {
                HirStmt::Expr(self.lower_expr_to_expr_block(&boxed.value.try_block), line)
            }
            A::Stmt::OnlyGuard(boxed) => {
                let og = &boxed.value;
                HirStmt::If {
                    condition: self.lower_expr_from_node(&og.condition),
                    then_block: self.lower_block(&og.body),
                    else_block: None,
                    line,
                }
            }
            A::Stmt::UnsafeBlock(boxed) => {
                HirStmt::Expr(self.lower_expr_to_expr_block(&boxed.value.body), line)
            }
            A::Stmt::Expect(boxed) => {
                HirStmt::Expr(self.lower_expr_from_node(&boxed.value.expr), line)
            }
            A::Stmt::Todo(_) => HirStmt::Expr(HirExpr::NilLit, line),
            A::Stmt::Question(_) => HirStmt::Expr(HirExpr::NilLit, line),
            A::Stmt::Bench(boxed) => HirStmt::Bench(self.lower_block(&boxed.value.body), line),
        }
    }

    fn lower_expr_from_node(&mut self, node: &A::Node<A::Expr>) -> HirExpr {
        self.lower_expr(&node.value)
    }

    fn lower_expr_to_expr_block(&mut self, block: &A::Block) -> HirExpr {
        let hir_block = self.lower_block(block);
        if hir_block.stmts.len() == 1 {
            match &hir_block.stmts[0] {
                HirStmt::Expr(e, _) => return e.clone(),
                _ => {}
            }
        }
        HirExpr::Block(hir_block)
    }

    fn lower_expr(&mut self, expr: &A::Expr) -> HirExpr {
        match expr {
            A::Expr::Literal(lit) => match &lit.value {
                A::Literal::Int(v) => HirExpr::IntLit(*v),
                A::Literal::Float(v) => HirExpr::FloatLit(*v),
                A::Literal::Bool(v) => HirExpr::BoolLit(*v),
                A::Literal::String(v) => HirExpr::StringLit(v.clone()),
                A::Literal::Char(v) => HirExpr::CharLit(*v),
                A::Literal::Nil => HirExpr::NilLit,
            },
            A::Expr::Identifier(name) => HirExpr::Ident(name.clone()),
            A::Expr::BinaryOp { op, left, right } => HirExpr::BinaryOp {
                op: *op,
                left: Box::new(self.lower_expr(&left.value)),
                right: Box::new(self.lower_expr(&right.value)),
            },
            A::Expr::UnaryOp { op, operand } => HirExpr::UnaryOp {
                op: *op,
                operand: Box::new(self.lower_expr(&operand.value)),
            },
            A::Expr::Call { callee, args } => HirExpr::Call {
                callee: Box::new(self.lower_expr(&callee.value)),
                args: args.iter().map(|a| self.lower_expr(&a.value)).collect(),
            },
            A::Expr::MethodCall { object, method, args } => HirExpr::MethodCall {
                object: Box::new(self.lower_expr(&object.value)),
                method: method.clone(),
                args: args.iter().map(|a| self.lower_expr(&a.value)).collect(),
            },
            A::Expr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => HirExpr::IfThenElse {
                condition: Box::new(self.lower_expr(&condition.value)),
                then_expr: Box::new(self.lower_expr(&then_expr.value)),
                else_expr: else_expr.as_ref().map(|e| Box::new(self.lower_expr(&e.value))),
            },
            A::Expr::Lambda { params, body } => HirExpr::Lambda {
                params: params.iter().map(|p| self.lower_param(p)).collect(),
                body: Box::new(self.lower_expr(&body.value)),
            },
            A::Expr::Block(block) => HirExpr::Block(self.lower_block(block)),
            A::Expr::Array(items) => {
                HirExpr::Array(items.iter().map(|i| self.lower_expr(&i.value)).collect())
            }
            A::Expr::Tuple(items) => {
                HirExpr::Tuple(items.iter().map(|i| self.lower_expr(&i.value)).collect())
            }
            A::Expr::Record(fields) => HirExpr::Array(
                fields.iter().map(|(_, e)| self.lower_expr(&e.value)).collect(),
            ),
            A::Expr::Index { target, index } => HirExpr::Call {
                callee: Box::new(HirExpr::Ident("__index__".into())),
                args: vec![self.lower_expr(&target.value), self.lower_expr(&index.value)],
            },
            A::Expr::MemberAccess { target, field } => HirExpr::MethodCall {
                object: Box::new(self.lower_expr(&target.value)),
                method: field.clone(),
                args: vec![],
            },
            A::Expr::Range {
                start,
                end,
                inclusive,
            } => HirExpr::Range {
                start: Box::new(self.lower_expr(&start.value)),
                end: Box::new(self.lower_expr(&end.value)),
                inclusive: *inclusive,
            },
            A::Expr::Spread(inner) => HirExpr::Spread(Box::new(self.lower_expr(&inner.value))),
            A::Expr::BcAnnotation { expr, reason } => HirExpr::BcAnnotation {
                expr: Box::new(self.lower_expr(&expr.value)),
                reason: reason.clone(),
            },
            A::Expr::ErrorPropagate(inner) => {
                HirExpr::ErrorPropagate(Box::new(self.lower_expr(&inner.value)))
            }
            A::Expr::MatchExpression { value, arms } => HirExpr::Block(HirBlock {
                stmts: vec![HirStmt::Match {
                    value: self.lower_expr_from_node(value),
                    arms: arms
                        .iter()
                        .map(|arm| HirMatchArm {
                            body: self.lower_block(&arm.body),
                        })
                        .collect(),
                    line: self.line_of(&value.span),
                }],
                first_line: self.line_of(&value.span),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn parse_and_lower(src: &str) -> HirProgram {
        let mut p = crate::parser::Parser::new(src);
        let program = p.parse_program().expect("parse failed");
        lower(&program, src)
    }

    fn last_func(prog: &HirProgram) -> &HirFunction {
        prog.items.last().map(|i| match i {
            HirItem::Function(f) => f,
        }).expect("no functions")
    }

    // ----- Expect lowering -----

    #[test]
    fn test_hir_expect_lowers_to_expr() {
        let prog = parse_and_lower("func f() { expect 1 + 2 }");
        let func = last_func(&prog);
        match &func.body.stmts[0] {
            HirStmt::Expr(HirExpr::BinaryOp { op, .. }, _) => {
                assert_eq!(*op, ast::BinaryOpKind::Add);
            }
            other => panic!("expected Expr(BinaryOp), got {:?}", other),
        }
    }

    // ----- Todo lowering -----

    #[test]
    fn test_hir_todo_lowers_to_nil() {
        let prog = parse_and_lower("func f() { todo }");
        let func = last_func(&prog);
        match &func.body.stmts[0] {
            HirStmt::Expr(HirExpr::NilLit, _) => {} // ok
            other => panic!("expected NilLit, got {:?}", other),
        }
    }

    #[test]
    fn test_hir_todo_with_message_lowers_to_nil() {
        let prog = parse_and_lower(r##"func f() { todo "fix me" }"##);
        let func = last_func(&prog);
        match &func.body.stmts[0] {
            HirStmt::Expr(HirExpr::NilLit, _) => {}
            other => panic!("expected NilLit, got {:?}", other),
        }
    }

    // ----- Question lowering -----

    #[test]
    fn test_hir_question_lowers_to_nil() {
        let prog = parse_and_lower("func f() { question }");
        let func = last_func(&prog);
        match &func.body.stmts[0] {
            HirStmt::Expr(HirExpr::NilLit, _) => {}
            other => panic!("expected NilLit, got {:?}", other),
        }
    }

    // ----- Bench lowering -----

    #[test]
    fn test_hir_bench_lowers_to_bench_block() {
        let prog = parse_and_lower("func f() { bench { let x = 1 } }");
        let func = last_func(&prog);
        match &func.body.stmts[0] {
            HirStmt::Bench(body, _) => {
                // The bench body should contain the lowered let statement
                assert!(!body.stmts.is_empty());
                match &body.stmts[0] {
                    HirStmt::Let { name, .. } => assert_eq!(name, "x"),
                    other => panic!("expected Let, got {:?}", other),
                }
            }
            other => panic!("expected Bench, got {:?}", other),
        }
    }

    #[test]
    fn test_hir_bm_shorthand() {
        let prog = parse_and_lower("func f() { bm { } }");
        let func = last_func(&prog);
        assert!(matches!(&func.body.stmts[0], HirStmt::Bench(_, _)));
    }
}
