use crate::ast::*;
use crate::error::Result;

/// Ownership analysis for `only let` annotations.
pub struct OwnershipChecker;

impl OwnershipChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        for item in &program.items {
            match &item.value {
                Item::Function(f) => {
                    if f.stub_envs.is_some() {
                        continue;
                    }
                    self.check_block(&f.body)?;
                }
                Item::Class(c) => {
                    for method in &c.methods {
                        if method.stub_envs.is_some() {
                            continue;
                        }
                        self.check_block(&method.body)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<()> {
        for stmt in &block.statements {
            self.check_stmt(&stmt.value)?;
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let(boxed) => {
                let ls = &boxed.value;
                if ls.is_only && ls.value.is_none() {
                    return Err(crate::error::CompileError::new(
                        "`only let` requires an initializer expression",
                    ));
                }
                if let Some(val) = &ls.value {
                    self.check_expr(val)?;
                }
                Ok(())
            }
            Stmt::Expr(boxed) => self.check_expr(&boxed.value),
            Stmt::Return(ret) => {
                if let Some(boxed) = ret {
                    self.check_expr(&boxed.value)?;
                }
                Ok(())
            }
            Stmt::Assign(boxed) => {
                let a = &boxed.value;
                self.check_expr(&a.target.value)?;
                self.check_expr(&a.value.value)?;
                Ok(())
            }
            Stmt::BcAssert(boxed) => {
                let ba = &boxed.value;
                self.check_expr(&ba.condition.value)?;
                Ok(())
            }
            Stmt::If(boxed) => {
                let ifs = &boxed.value;
                self.check_expr(&ifs.condition.value)?;
                self.check_block(&ifs.then_block)?;
                if let Some(else_block) = &ifs.else_block {
                    self.check_block(else_block)?;
                }
                Ok(())
            }
            Stmt::For(boxed) => {
                let fs = &boxed.value;
                self.check_expr(&fs.iterable.value)?;
                self.check_block(&fs.body)?;
                Ok(())
            }
            Stmt::While(boxed) => {
                let ws = &boxed.value;
                self.check_expr(&ws.condition.value)?;
                self.check_block(&ws.body)?;
                Ok(())
            }
            Stmt::Match(boxed) => {
                let ms = &boxed.value;
                self.check_expr(&ms.value.value)?;
                for arm in &ms.arms {
                    self.check_block(&arm.body)?;
                }
                Ok(())
            }
            Stmt::TryCatch(boxed) => {
                let tc = &boxed.value;
                self.check_block(&tc.try_block)?;
                self.check_block(&tc.catch_block)?;
                if let Some(finally) = &tc.finally_block {
                    self.check_block(finally)?;
                }
                Ok(())
            }
            Stmt::OnlyGuard(boxed) => {
                let og = &boxed.value;
                self.check_expr(&og.condition.value)?;
                self.check_block(&og.body)?;
                Ok(())
            }
            Stmt::UnsafeBlock(boxed) => {
                self.check_block(&boxed.value.body)?;
                Ok(())
            }
            Stmt::Expect(boxed) => {
                self.check_expr(&boxed.value.expr.value)?;
                Ok(())
            }
            Stmt::Todo(_) | Stmt::Question(_) => Ok(()),
            Stmt::Wait(_) => Ok(()),
            Stmt::Bench(boxed) => {
                self.check_block(&boxed.value.body)?;
                Ok(())
            }
            Stmt::Parallel(boxed) => {
                for item in &boxed.value.items {
                    self.check_stmt(&item.value)?;
                }
                Ok(())
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Literal(_) => Ok(()),
            Expr::Identifier(_) => Ok(()),
            Expr::BinaryOp { left, right, .. } => {
                self.check_expr(&left.value)?;
                self.check_expr(&right.value)?;
                Ok(())
            }
            Expr::UnaryOp { operand, .. } => self.check_expr(&operand.value),
            Expr::Call { callee, args } => {
                self.check_expr(&callee.value)?;
                for arg in args { self.check_expr(&arg.value)?; }
                Ok(())
            }
            Expr::MethodCall { object, args, .. } => {
                self.check_expr(&object.value)?;
                for arg in args { self.check_expr(&arg.value)?; }
                Ok(())
            }
            Expr::IfThenElse { condition, then_expr, else_expr } => {
                self.check_expr(&condition.value)?;
                self.check_expr(&then_expr.value)?;
                if let Some(e) = else_expr { self.check_expr(&e.value)?; }
                Ok(())
            }
            Expr::Lambda { body, .. } => self.check_expr(&body.value),
            Expr::Block(block) => self.check_block(block),
            Expr::Array(items) => {
                for item in items { self.check_expr(&item.value)?; }
                Ok(())
            }
            Expr::Tuple(items) => {
                for item in items { self.check_expr(&item.value)?; }
                Ok(())
            }
            Expr::Record(fields) => {
                for (_, expr) in fields { self.check_expr(&expr.value)?; }
                Ok(())
            }
            Expr::Index { target, index } => {
                self.check_expr(&target.value)?;
                self.check_expr(&index.value)?;
                Ok(())
            }
            Expr::MemberAccess { target, .. } => self.check_expr(&target.value),
            Expr::Range { start, end, .. } => {
                self.check_expr(&start.value)?;
                self.check_expr(&end.value)?;
                Ok(())
            }
            Expr::Spread(inner) => self.check_expr(&inner.value),
            Expr::BcAnnotation { expr, .. } => self.check_expr(&expr.value),
            Expr::ErrorPropagate(inner) => self.check_expr(&inner.value),
            Expr::Await(inner) => self.check_expr(&inner.value),
            Expr::MatchExpression { value, arms } => {
                self.check_expr(&value.value)?;
                for arm in arms { self.check_block(&arm.body)?; }
                Ok(())
            }
            Expr::Is { value, .. } => self.check_expr(&value.value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_src(src: &str) -> std::result::Result<(), crate::error::CompileError> {
        let mut p = crate::parser::Parser::new(src);
        let program = p.parse_program().expect("parse failed");
        let mut checker = OwnershipChecker::new();
        checker.check_program(&program)
    }

    fn assert_ownership_ok(src: &str) {
        assert!(check_src(src).is_ok(), "expected Ok, got error");
    }

    #[test]
    fn test_ownership_todo() {
        assert_ownership_ok("func f() { todo }");
    }

    #[test]
    fn test_ownership_question() {
        assert_ownership_ok("func f() { question }");
    }

    #[test]
    fn test_ownership_expect() {
        assert_ownership_ok("func f() { expect 1 + 2 }");
    }

    #[test]
    fn test_ownership_bench() {
        assert_ownership_ok("func f() { bench { let x = 1 } }");
    }

    #[test]
    fn test_ownership_bm() {
        assert_ownership_ok("func f() { bm { let x = 42 } }");
    }

    #[test]
    fn test_ownership_spec() {
        assert_ownership_ok(r##"spec "tests" { feat "add" { expect 1 + 1 } }"##);
    }
}
