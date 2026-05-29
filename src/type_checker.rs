use crate::ast::*;
use crate::error::{CompileError, Result};
use std::collections::HashMap;

pub struct TypeChecker {
    types: HashMap<String, Type>,
    scopes: Vec<HashMap<String, Type>>,
    functions: HashMap<String, FunctionSignature>,
    errors: Vec<CompileError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Nil,
    Array(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Named(String, Vec<Type>),
    Infer,
    Error,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub param_types: Vec<Type>,
    pub return_type: Box<Type>,
    pub is_async: bool,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = Self {
            types: HashMap::new(),
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            errors: Vec::new(),
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        self.types.insert("Int".into(), Type::Int);
        self.types.insert("Float".into(), Type::Float);
        self.types.insert("Bool".into(), Type::Bool);
        self.types.insert("String".into(), Type::String);
        self.types.insert("Char".into(), Type::Char);
        self.types.insert("Nil".into(), Type::Nil);

        self.functions.insert(
            "print".into(),
            FunctionSignature {
                param_types: vec![Type::Infer],
                return_type: Box::new(Type::Nil),
                is_async: false,
            },
        );
        self.functions.insert(
            "sum".into(),
            FunctionSignature {
                param_types: vec![Type::Array(Box::new(Type::Float))],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
        self.functions.insert(
            "min".into(),
            FunctionSignature {
                param_types: vec![Type::Float, Type::Float],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
        self.functions.insert(
            "max".into(),
            FunctionSignature {
                param_types: vec![Type::Float, Type::Float],
                return_type: Box::new(Type::Float),
                is_async: false,
            },
        );
    }

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // First pass: collect all function signatures (including class methods)
        for item in &program.items {
            match &item.value {
                Item::Function(f) => {
                    let sig = self.infer_function_signature(f);
                    self.functions.insert(f.name.clone(), sig);
                }
                Item::Class(c) => {
                    for method in &c.methods {
                        let sig = self.infer_function_signature(method);
                        self.functions.insert(method.name.clone(), sig);
                    }
                }
                _ => {}
            }
        }

        // Second pass: check bodies
        for item in &program.items {
            match &item.value {
                Item::Function(f) => {
                    self.check_func_body(f)?;
                }
                Item::Class(c) => {
                    for method in &c.methods {
                        self.check_func_body(method)?;
                    }
                }
                _ => {}
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.remove(0))
        }
    }

    fn check_func_body(&mut self, f: &Function) -> Result<()> {
        // Skip stub functions — no body to type-check
        if f.stub_envs.is_some() {
            return Ok(());
        }
        self.scopes.push(HashMap::new());
        for param in &f.params {
            let ty = param
                .type_ann
                .as_ref()
                .map(|t| self.resolve_type_expr(t))
                .unwrap_or(Type::Infer);
            self.scopes.last_mut().unwrap().insert(param.name.clone(), ty);
        }

        if let Some(_ret_type) = &f.return_type {
            // checked by resolution
        }

        let _ = self.check_block(&f.body);
        self.scopes.pop();
        Ok(())
    }

    fn infer_function_signature(&mut self, f: &Function) -> FunctionSignature {
        let param_types = f
            .params
            .iter()
            .map(|p| {
                p.type_ann
                    .as_ref()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(Type::Infer)
            })
            .collect();
        let return_type = f
            .return_type
            .as_ref()
            .map(|t| Box::new(self.resolve_type_expr(t)))
            .unwrap_or(Box::new(Type::Infer));
        FunctionSignature {
            param_types,
            return_type,
            is_async: f.is_async,
        }
    }

    fn resolve_type_expr(&self, texpr: &TypeExpr) -> Type {
        match texpr {
            TypeExpr::Named(name) => {
                self.types.get(name).cloned().unwrap_or(Type::Named(name.clone(), vec![]))
            }
            TypeExpr::Generic(name, params) => {
                let resolved: Vec<Type> = params.iter().map(|p| self.resolve_type_expr(p)).collect();
                Type::Named(name.clone(), resolved)
            }
            TypeExpr::Array(inner) => Type::Array(Box::new(self.resolve_type_expr(inner))),
            TypeExpr::Option(inner) => Type::Option(Box::new(self.resolve_type_expr(inner))),
            TypeExpr::Result(ok, err) => Type::Result(
                Box::new(self.resolve_type_expr(ok)),
                Box::new(self.resolve_type_expr(err)),
            ),
            TypeExpr::Union(ts) => {
                ts.first()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(Type::Error)
            }
            TypeExpr::Function(params, ret) => Type::Function(
                params.iter().map(|p| self.resolve_type_expr(p)).collect(),
                Box::new(self.resolve_type_expr(ret)),
            ),
            TypeExpr::Tuple(ts) => {
                Type::Tuple(ts.iter().map(|t| self.resolve_type_expr(t)).collect())
            }
            TypeExpr::Record(fields) => Type::Named(
                "Record".into(),
                fields.iter().map(|(_, t)| self.resolve_type_expr(t)).collect(),
            ),
            TypeExpr::Infer => Type::Infer,
        }
    }

    fn check_block(&mut self, block: &Block) -> Option<Type> {
        let mut last_type = None;
        for stmt in &block.statements {
            last_type = Some(self.check_stmt(&stmt.value));
        }
        last_type
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Type {
        match stmt {
            Stmt::Let(boxed_node) => {
                let let_stmt = &boxed_node.value;
                let ty = if let Some(val) = &let_stmt.value {
                    self.check_expr(val)
                } else {
                    Type::Infer
                };
                self.scopes.last_mut().unwrap().insert(let_stmt.name.clone(), ty.clone());
                ty
            }
            Stmt::Expr(boxed) => self.check_expr(&boxed.value),
            Stmt::Return(ret) => {
                ret.as_ref()
                    .map(|e| self.check_expr(&e.value))
                    .unwrap_or(Type::Nil)
            }
            Stmt::Assign(assign) => {
                let a = &assign.value;
                let val_ty = self.check_expr(&a.value.value);
                let _target_ty = self.check_expr(&a.target.value);
                val_ty
            }
            Stmt::BcAssert(assert) => {
                let _cond = self.check_expr(&assert.value.condition.value);
                Type::Nil
            }
            Stmt::If(boxed) => {
                let ifs = &boxed.value;
                let _ = self.check_expr(&ifs.condition.value);
                let then_ty = self.check_block(&ifs.then_block);
                let else_ty = ifs
                    .else_block
                    .as_ref()
                    .map(|b| self.check_block(b))
                    .unwrap_or(None);
                then_ty.unwrap_or(Type::Nil)
            }
            Stmt::For(boxed) => {
                let fs = &boxed.value;
                let _ = self.check_expr(&fs.iterable.value);
                self.scopes.last_mut().unwrap().insert(fs.variable.clone(), Type::Infer);
                self.check_block(&fs.body);
                Type::Nil
            }
            Stmt::While(boxed) => {
                let ws = &boxed.value;
                let _ = self.check_expr(&ws.condition.value);
                self.check_block(&ws.body);
                Type::Nil
            }
            Stmt::Match(boxed) => {
                let ms = &boxed.value;
                let _ = self.check_expr(&ms.value.value);
                for arm in &ms.arms {
                    self.check_block(&arm.body);
                }
                Type::Infer
            }
            Stmt::TryCatch(boxed) => {
                let tc = &boxed.value;
                let try_ty = self.check_block(&tc.try_block);
                let catch_ty = self.check_block(&tc.catch_block);
                catch_ty.unwrap_or(try_ty.unwrap_or(Type::Nil))
            }
            Stmt::OnlyGuard(boxed) => {
                let og = &boxed.value;
                let _ = self.check_expr(&og.condition.value);
                self.check_block(&og.body);
                Type::Nil
            }
            Stmt::UnsafeBlock(boxed) => {
                self.check_block(&boxed.value.body);
                Type::Infer
            }
            Stmt::Expect(boxed) => {
                let _ = self.check_expr(&boxed.value.expr.value);
                Type::Nil
            }
            Stmt::Todo(_) | Stmt::Question(_) => Type::Nil,
            Stmt::Bench(boxed) => {
                self.check_block(&boxed.value.body);
                Type::Nil
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::String,
                Literal::Char(_) => Type::Char,
                Literal::Nil => Type::Nil,
            },
            Expr::Identifier(name) => {
                for scope in self.scopes.iter().rev() {
                    if let Some(ty) = scope.get(name) {
                        return ty.clone();
                    }
                }
                if let Some(sig) = self.functions.get(name) {
                    return Type::Function(sig.param_types.clone(), sig.return_type.clone());
                }
                Type::Infer
            }
            Expr::BinaryOp { op: _, left, right } => {
                let l = self.check_expr(&left.value);
                let r = self.check_expr(&right.value);
                match (l, r) {
                    (Type::Int, Type::Int) => Type::Int,
                    (Type::Float, _) | (_, Type::Float) => Type::Float,
                    (Type::Bool, Type::Bool) => Type::Bool,
                    (Type::String, _) | (_, Type::String) => Type::String,
                    _ => Type::Infer,
                }
            }
            Expr::UnaryOp { op: _, operand } => self.check_expr(&operand.value),
            Expr::Call { callee, args } => {
                let _callee_ty = self.check_expr(&callee.value);
                for arg in args {
                    self.check_expr(&arg.value);
                }
                if let Expr::Identifier(name) = &callee.value {
                    if let Some(sig) = self.functions.get(name) {
                        return *sig.return_type.clone();
                    }
                }
                Type::Infer
            }
            Expr::MethodCall { object, method: _, args } => {
                let _obj_ty = self.check_expr(&object.value);
                for arg in args {
                    self.check_expr(&arg.value);
                }
                Type::Infer
            }
            Expr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                let _ = self.check_expr(&condition.value);
                let then_ty = self.check_expr(&then_expr.value);
                let else_ty = else_expr.as_ref().map(|e| self.check_expr(&e.value)).unwrap_or(Type::Nil);
                if then_ty == else_ty { then_ty } else { Type::Infer }
            }
            Expr::Lambda { params, body } => {
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|p| {
                        p.type_ann
                            .as_ref()
                            .map(|t| self.resolve_type_expr(t))
                            .unwrap_or(Type::Infer)
                    })
                    .collect();
                let ret_ty = self.check_expr(&body.value);
                Type::Function(param_types, Box::new(ret_ty))
            }
            Expr::Block(block) => self.check_block(block).unwrap_or(Type::Nil),
            Expr::Array(items) => {
                if items.is_empty() {
                    Type::Array(Box::new(Type::Infer))
                } else {
                    Type::Array(Box::new(self.check_expr(&items[0].value)))
                }
            }
            Expr::Tuple(items) => Type::Tuple(items.iter().map(|i| self.check_expr(&i.value)).collect()),
            Expr::Record(fields) => {
                for (_, expr) in fields { self.check_expr(&expr.value); }
                Type::Infer
            }
            Expr::Index { target, index } => {
                let _ = self.check_expr(&target.value);
                let _ = self.check_expr(&index.value);
                Type::Infer
            }
            Expr::MemberAccess { target, field: _ } => self.check_expr(&target.value),
            Expr::Range { start, end, .. } => {
                self.check_expr(&start.value);
                self.check_expr(&end.value);
                Type::Array(Box::new(Type::Int))
            }
            Expr::Spread(inner) => self.check_expr(&inner.value),
            Expr::BcAnnotation { expr, .. } => self.check_expr(&expr.value),
            Expr::ErrorPropagate(inner) => {
                let ty = self.check_expr(&inner.value);
                if let Type::Result(ok, _) = ty { *ok } else { Type::Infer }
            }
            Expr::MatchExpression { value, arms } => {
                let _ = self.check_expr(&value.value);
                for arm in arms { self.check_block(&arm.body); }
                Type::Infer
            }
        }
    }

    pub fn into_errors(self) -> Vec<CompileError> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn check_src(src: &str) -> std::result::Result<(), CompileError> {
        let mut p = crate::parser::Parser::new(src);
        let program = p.parse_program().expect("parse failed");
        let mut tc = TypeChecker::new();
        tc.check_program(&program)
    }

    fn assert_ok(src: &str) {
        let result = check_src(src);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    // ----- Spec items are ignored -----

    #[test]
    fn test_type_check_spec_is_ok() {
        assert_ok(r##"spec "math" { feat "add" { expect 1 + 1 } }"##);
    }

    #[test]
    fn test_type_check_describe_is_ok() {
        assert_ok(r##"describe "suite" { it "pass" { expect true } }"##);
    }

    #[test]
    fn test_type_check_todo_is_ok() {
        assert_ok("func f() { todo }");
    }

    #[test]
    fn test_type_check_todo_with_message_is_ok() {
        assert_ok(r##"func f() { todo "fix this" }"##);
    }

    #[test]
    fn test_type_check_question_is_ok() {
        assert_ok("func f() { question }");
    }

    #[test]
    fn test_type_check_bench_is_ok() {
        assert_ok("func f() { bench { let x = 1 } }");
    }

    #[test]
    fn test_type_check_bm_is_ok() {
        assert_ok("func f() { bm { let x = 42 } }");
    }

    #[test]
    fn test_type_check_expect_is_ok() {
        assert_ok("func f() { expect 1 + 2 }");
    }

    #[test]
    fn test_type_check_import_is_ok() {
        assert_ok(r##"import "foo.ely""##);
    }

    #[test]
    fn test_type_check_import_as_is_ok() {
        assert_ok(r##"import "foo.ely" as mymod"##);
    }

    // ----- Combined -----

    #[test]
    fn test_type_check_spec_with_bench() {
        assert_ok(r##"
            spec "perf" {
                feat "fib" { bench { let x = 1 } }
            }
        "##);
    }
}
