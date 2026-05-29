use crate::hir::*;

/// Mid-level IR — further simplified from HIR.
/// At this level, all sugar has been desugared, control flow is flat,
/// and we're ready for codegen.
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub compile_unit_line: u32,
}

#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MirParam>,
    pub return_type: MirType,
    pub body: MirBlock,
    pub is_async: bool,
    pub dbg_line: u32,
}

#[derive(Debug, Clone)]
pub struct MirParam {
    pub name: String,
    pub ty: MirType,
    pub dbg_line: u32,
}

#[derive(Debug, Clone)]
pub struct MirBlock {
    pub stmts: Vec<MirStmt>,
}

#[derive(Debug, Clone)]
pub enum MirStmt {
    Alloca {
        name: String,
        ty: MirType,
        is_mutable: bool,
        is_lazy: bool,
        dbg_line: u32,
    },
    Store {
        target: String,
        value: MirValue,
        dbg_line: u32,
    },
    Call {
        result: Option<String>,
        callee: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    Return(Option<MirValue>, u32),
    CondBranch {
        condition: MirValue,
        then_block: usize,
        else_block: usize,
        dbg_line: u32,
    },
    Jump(usize),
    BcAssert {
        condition: MirValue,
        message: String,
        dbg_line: u32,
    },
    UnsafeBlock(Vec<MirStmt>),
    Bench {
        body_stmts: Vec<MirStmt>,
        dbg_line: u32,
    },
}

#[derive(Debug, Clone)]
pub enum MirValue {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    CharLit(char),
    Nil,
    Local(String),
    BinaryOp {
        op: super::ast::BinaryOpKind,
        left: Box<MirValue>,
        right: Box<MirValue>,
    },
    UnaryOp {
        op: super::ast::UnaryOpKind,
        operand: Box<MirValue>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirType {
    Int,
    Float,
    Bool,
    String,
    Char,
    Nil,
    Ptr(Box<MirType>),
    Array(Box<MirType>),
}

/// Lower HIR to MIR.
pub fn lower(program: &HirProgram, first_line: u32) -> MirProgram {
    let mut lowerer = MirLowerer::new();
    lowerer.lower_program(program, first_line)
}

struct MirLowerer {
    next_block: usize,
}

impl MirLowerer {
    fn new() -> Self {
        Self { next_block: 1 }
    }

    fn fresh_block(&mut self) -> usize {
        let b = self.next_block;
        self.next_block += 1;
        b
    }

    fn lower_program(&mut self, program: &HirProgram, first_line: u32) -> MirProgram {
        let mut functions = Vec::new();
        for item in &program.items {
            match item {
                HirItem::Function(f) => functions.push(self.lower_function(f)),
            }
        }
        MirProgram { functions, compile_unit_line: first_line }
    }

    fn lower_function(&mut self, f: &HirFunction) -> MirFunction {
        let params = f
            .params
            .iter()
            .map(|p| MirParam {
                name: p.name.clone(),
                ty: self.lower_type(&p.ty),
                dbg_line: f.line,
            })
            .collect();

        let body = self.lower_block(&f.body);

        MirFunction {
            name: f.name.clone(),
            params,
            return_type: self.lower_type(&f.return_type),
            body,
            is_async: f.is_async,
            dbg_line: f.line,
        }
    }

    fn lower_type(&self, ty: &HirType) -> MirType {
        match ty {
            HirType::Int => MirType::Int,
            HirType::Float => MirType::Float,
            HirType::Bool => MirType::Bool,
            HirType::String => MirType::String,
            HirType::Char => MirType::Char,
            HirType::Nil => MirType::Nil,
            HirType::Array(inner) => MirType::Array(Box::new(self.lower_type(inner))),
            HirType::Option(_) => MirType::Ptr(Box::new(MirType::Nil)),
            HirType::Result(_, _) => MirType::Ptr(Box::new(MirType::Nil)),
            _ => MirType::Nil,
        }
    }

    fn lower_block(&mut self, block: &HirBlock) -> MirBlock {
        let mut stmts = Vec::new();
        for stmt in &block.stmts {
            self.lower_stmt(stmt, &mut stmts);
        }
        MirBlock { stmts }
    }

    fn stmt_line(&self, stmt: &HirStmt) -> u32 {
        match stmt {
            HirStmt::Let { line, .. } => *line,
            HirStmt::Assign { line, .. } => *line,
            HirStmt::Expr(_, line) => *line,
            HirStmt::Return(_, line) => *line,
            HirStmt::If { line, .. } => *line,
            HirStmt::For { line, .. } => *line,
            HirStmt::While { line, .. } => *line,
            HirStmt::Match { line, .. } => *line,
            HirStmt::Bench(_, line) => *line,
        }
    }

    fn lower_stmt(&mut self, stmt: &HirStmt, stmts: &mut Vec<MirStmt>) {
        let line = self.stmt_line(stmt);
        match stmt {
            HirStmt::Let {
                name,
                ty,
                value,
                is_mutable,
                is_lazy,
                is_only: _,
                line: _,
            } => {
                stmts.push(MirStmt::Alloca {
                    name: name.clone(),
                    ty: self.lower_type(ty),
                    is_mutable: *is_mutable,
                    is_lazy: *is_lazy,
                    dbg_line: line,
                });
                // For non-lazy lets with a value, store immediately
                if let Some(val) = value {
                    if !is_lazy {
                        stmts.push(MirStmt::Store {
                            target: name.clone(),
                            value: self.lower_expr(val),
                            dbg_line: line,
                        });
                    }
                }
            }
            HirStmt::Assign { target, value, line: _ } => {
                if let HirExpr::Ident(name) = target {
                    stmts.push(MirStmt::Store {
                        target: name.clone(),
                        value: self.lower_expr(value),
                        dbg_line: line,
                    });
                }
            }
            HirStmt::Expr(expr, _) => {
                let val = self.lower_expr(expr);
                stmts.push(MirStmt::Call {
                    result: None,
                    callee: "__expr__".into(),
                    args: vec![val],
                    dbg_line: line,
                });
            }
            HirStmt::Return(ret, _) => {
                stmts.push(MirStmt::Return(ret.as_ref().map(|e| self.lower_expr(e)), line));
            }
            HirStmt::If {
                condition,
                then_block,
                else_block,
                line: _,
            } => {
                let cond = self.lower_expr(condition);
                let then_idx = self.fresh_block();
                let mut then_stmts = Vec::new();
                self.lower_block_stmts(then_block, &mut then_stmts);
                let end_idx = self.fresh_block();
                then_stmts.push(MirStmt::Jump(end_idx));

                match else_block {
                    Some(eb) => {
                        let else_idx = self.fresh_block();
                        let mut else_stmts = Vec::new();
                        self.lower_block_stmts(eb, &mut else_stmts);
                        else_stmts.push(MirStmt::Jump(end_idx));
                        stmts.push(MirStmt::CondBranch {
                            condition: cond,
                            then_block: then_idx,
                            else_block: else_idx,
                            dbg_line: line,
                        });
                        stmts.append(&mut then_stmts);
                        stmts.append(&mut else_stmts);
                    }
                    None => {
                        stmts.push(MirStmt::CondBranch {
                            condition: cond,
                            then_block: then_idx,
                            else_block: end_idx,
                            dbg_line: line,
                        });
                        stmts.append(&mut then_stmts);
                    }
                }
                stmts.push(MirStmt::Jump(end_idx));
            }
            HirStmt::For {
                variable,
                iterable,
                body,
                line: _,
            } => {
                stmts.push(MirStmt::Alloca {
                    name: variable.clone(),
                    ty: MirType::Int,
                    is_mutable: true,
                    is_lazy: false,
                    dbg_line: line,
                });
                let iter_val = self.lower_expr(iterable);
                stmts.push(MirStmt::Store {
                    target: variable.clone(),
                    value: iter_val,
                    dbg_line: line,
                });
                self.lower_block_stmts(body, stmts);
            }
            HirStmt::While { condition, body, line: _ } => {
                let cond_idx = self.fresh_block();
                let body_idx = self.fresh_block();
                let end_idx = self.fresh_block();

                stmts.push(MirStmt::Jump(cond_idx));
                stmts.push(MirStmt::Jump(cond_idx));

                let cond = self.lower_expr(condition);
                stmts.push(MirStmt::CondBranch {
                    condition: cond,
                    then_block: body_idx,
                    else_block: end_idx,
                    dbg_line: line,
                });

                let mut body_stmts = Vec::new();
                self.lower_block_stmts(body, &mut body_stmts);
                body_stmts.push(MirStmt::Jump(cond_idx));
                stmts.append(&mut body_stmts);
                stmts.push(MirStmt::Jump(end_idx));
            }
            HirStmt::Match { value, arms, line: _ } => {
                let _match_val = self.lower_expr(value);
                for arm in arms {
                    self.lower_block_stmts(&arm.body, stmts);
                }
            }
            HirStmt::Bench(body, _line) => {
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    self.lower_stmt(s, &mut body_stmts);
                }
                stmts.push(MirStmt::Bench {
                    body_stmts,
                    dbg_line: line,
                });
            }
        }
    }

    fn lower_block_stmts(&mut self, block: &HirBlock, stmts: &mut Vec<MirStmt>) {
        for stmt in &block.stmts {
            self.lower_stmt(stmt, stmts);
        }
    }

    fn lower_expr(&self, expr: &HirExpr) -> MirValue {
        match expr {
            HirExpr::IntLit(v) => MirValue::IntLit(*v),
            HirExpr::FloatLit(v) => MirValue::FloatLit(*v),
            HirExpr::BoolLit(v) => MirValue::BoolLit(*v),
            HirExpr::StringLit(v) => MirValue::StringLit(v.clone()),
            HirExpr::CharLit(v) => MirValue::CharLit(*v),
            HirExpr::NilLit => MirValue::Nil,
            HirExpr::Ident(name) => MirValue::Local(name.clone()),
            HirExpr::BinaryOp { op, left, right } => MirValue::BinaryOp {
                op: *op,
                left: Box::new(self.lower_expr(left)),
                right: Box::new(self.lower_expr(right)),
            },
            HirExpr::UnaryOp { op, operand } => MirValue::UnaryOp {
                op: *op,
                operand: Box::new(self.lower_expr(operand)),
            },
            HirExpr::Call { callee, args } => {
                match callee.as_ref() {
                    HirExpr::Ident(name) => {
                        MirValue::Local(format!("__call_{}({})__", name, args.iter().map(|_| "_").collect::<Vec<_>>().join(",")))
                    }
                    _ => MirValue::Nil,
                }
            }
            HirExpr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                let _c = self.lower_expr(condition);
                let t = self.lower_expr(then_expr);
                else_expr.as_ref().map(|e| self.lower_expr(e));
                t
            }
            HirExpr::Block(_) => MirValue::Nil,
            HirExpr::Array(_) => MirValue::Nil,
            HirExpr::Tuple(_) => MirValue::Nil,
            HirExpr::Range { .. } => MirValue::Nil,
            HirExpr::Spread(inner) => self.lower_expr(inner),
            HirExpr::BcAnnotation { expr, .. } => self.lower_expr(expr),
            HirExpr::ErrorPropagate(inner) => self.lower_expr(inner),
            HirExpr::Lambda { .. } => MirValue::Nil,
            HirExpr::MethodCall { .. } => MirValue::Nil,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_hir_stmts(stmts: Vec<HirStmt>) -> MirProgram {
        let hir_prog = HirProgram {
            items: vec![HirItem::Function(HirFunction {
                name: "test".to_string(),
                params: vec![],
                return_type: HirType::Nil,
                body: HirBlock {
                    stmts,
                    first_line: 1,
                },
                is_async: false,
                is_lazy: false,
                is_private: false,
                line: 1,
            })],
        };
        lower(&hir_prog, 1)
    }

    fn last_func(prog: &MirProgram) -> &MirFunction {
        prog.functions.last().expect("no functions")
    }

    // ----- Expect -----

    #[test]
    fn test_mir_expect_from_hir() {
        let mir_prog = lower_hir_stmts(vec![
            HirStmt::Expr(HirExpr::IntLit(42), 1),
        ]);
        let func = last_func(&mir_prog);
        // Expect gets lowered to just an Expr, which becomes a Call
        assert!(!func.body.stmts.is_empty());
    }

    // ----- Todo -> Nil -----

    #[test]
    fn test_mir_todo_becomes_call() {
        let mir_prog = lower_hir_stmts(vec![
            HirStmt::Expr(HirExpr::NilLit, 1),
        ]);
        let func = last_func(&mir_prog);
        // NilLit becomes a Call with Literal(Nil) — the key is no panic
        assert!(!func.body.stmts.is_empty());
    }

    // ----- Bench -----

    #[test]
    fn test_mir_bench_contains_body_stmts() {
        let mir_prog = lower_hir_stmts(vec![
            HirStmt::Bench(
                HirBlock {
                    stmts: vec![
                        HirStmt::Expr(HirExpr::IntLit(99), 1),
                    ],
                    first_line: 1,
                },
                1,
            ),
        ]);
        let func = last_func(&mir_prog);
        match &func.body.stmts[0] {
            MirStmt::Bench { body_stmts, .. } => {
                assert!(!body_stmts.is_empty(), "bench body should contain stmts");
            }
            other => panic!("expected Bench, got {:?}", other),
        }
    }

    #[test]
    fn test_mir_bench_empty_body() {
        let mir_prog = lower_hir_stmts(vec![
            HirStmt::Bench(
                HirBlock { stmts: vec![], first_line: 1 },
                1,
            ),
        ]);
        let func = last_func(&mir_prog);
        match &func.body.stmts[0] {
            MirStmt::Bench { body_stmts, .. } => {
                assert!(body_stmts.is_empty(), "empty bench body should have no stmts");
            }
            other => panic!("expected Bench, got {:?}", other),
        }
    }
}
