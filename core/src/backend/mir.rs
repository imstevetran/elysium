use crate::hir::*;

/// Mid-level IR — further simplified from HIR.
/// At this level, all sugar has been desugared, control flow is flat,
/// and we're ready for codegen.
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub workers: Vec<MirWorker>,
    pub compile_unit_line: u32,
}

/// A MIR worker definition — lowered from HirWorker.
/// The worker construct represents a portable thread/worker that
/// can be spawned and communicated with via message passing.
#[derive(Debug, Clone)]
pub struct MirWorker {
    pub name: String,
    pub params: Vec<MirParam>,
    pub body: MirBlock,
    pub dbg_line: u32,
}

#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MirParam>,
    pub return_type: MirType,
    pub body: MirBlock,
    pub is_async: bool,
    pub schedule_expr: Option<String>,
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
    Parallel {
        blocks: Vec<Vec<MirStmt>>,
        dbg_line: u32,
    },
    /// Wait for N milliseconds (via usleep).
    Wait(u64, u32),
    /// Async await point — state machine will save/restore at this point.
    /// The `value` is the awaited expression (e.g. a transport call).
    /// `result_target` is the variable name to store the result into.
    Await {
        value: Vec<MirStmt>,
        result_target: Option<String>,
        dbg_line: u32,
    },
    ConsoleCall {
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Filesystem call. When `result` is Some(name), the return value is stored into that alloca.
    FsCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Transport call (HTTP, WebSocket, MQTT). When `result` is Some(name), the return
    /// value is stored into that alloca.
    TransportCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// String operation call (length, toUpper, trim, etc.).
    /// When `result` is Some(name), the return value is stored into that alloca.
    StringCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    RegexCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    DateTimeCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    LangChainCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    LangGraphCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    AuthCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Worker call. When `result` is Some(name), the return value is stored into that alloca.
    WorkerCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Dict call (mutable key-value dictionary).
    DictCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// JSON call (parsing and serialization).
    JsonCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Math call (extended math operations).
    MathCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Env call (environment variable access).
    EnvCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Http call (HTTP client with custom headers).
    HttpCall {
        result: Option<String>,
        method: String,
        args: Vec<MirValue>,
        dbg_line: u32,
    },
    /// Is call (runtime type checking — `instanceof`).
    IsCall {
        result: Option<String>,
        value: MirValue,
        type_name: MirValue,
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
        op: crate::ast::BinaryOpKind,
        left: Box<MirValue>,
        right: Box<MirValue>,
    },
    UnaryOp {
        op: crate::ast::UnaryOpKind,
        operand: Box<MirValue>,
    },
    /// Runtime type check: `value is TypeName`.
    /// Evaluated inline by the codegen.
    IsInstanceof {
        value: Box<MirValue>,
        type_name: Box<MirValue>,
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
        let mut workers = Vec::new();
        for item in &program.items {
            match item {
                HirItem::Function(f) => functions.push(self.lower_function(f)),
                HirItem::Worker(w) => workers.push(self.lower_worker(w)),
            }
        }
        MirProgram { functions, workers, compile_unit_line: first_line }
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

        let mut body = self.lower_block(&f.body);

        // Implicit return: if the last statement in a non-void function is a bare
        // expression (Call __expr__), convert it into an explicit Return.
        let ret_type = self.lower_type(&f.return_type);
        if ret_type != MirType::Nil {
            let has_implicit_return = body.stmts.last().map_or(false, |last| {
                matches!(last, MirStmt::Call { result: None, callee, .. } if callee == "__expr__")
            });
            if has_implicit_return {
                if let MirStmt::Call { args, dbg_line, .. } = body.stmts.pop().unwrap() {
                    if let Some(val) = args.into_iter().next() {
                        body.stmts.push(MirStmt::Return(Some(val), dbg_line));
                    }
                }
            }
        }

        MirFunction {
            name: f.name.clone(),
            params,
            return_type: ret_type,
            body,
            is_async: f.is_async,
            schedule_expr: f.schedule_expr.clone(),
            dbg_line: f.line,
        }
    }

    fn lower_worker(&mut self, w: &HirWorker) -> MirWorker {
        let params = w
            .params
            .iter()
            .map(|p| MirParam {
                name: p.name.clone(),
                ty: self.lower_type(&p.ty),
                dbg_line: w.line,
            })
            .collect();
        let body = self.lower_block(&w.body);
        MirWorker {
            name: w.name.clone(),
            params,
            body,
            dbg_line: w.line,
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
            HirStmt::Wait(_, line) => *line,
            HirStmt::Parallel { line, .. } => *line,
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
                        // Check if the value is an __fs_* call (returns a value we need to capture)
                        if let HirExpr::Call { callee, args } = val {
                            if let HirExpr::Ident(cname) = callee.as_ref() {
                                if cname.starts_with("__fs_") {
                                    let method = cname.strip_prefix("__fs_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::FsCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__transport_") {
                                    let method = cname.strip_prefix("__transport_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::TransportCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__string_") {
                                    let method = cname.strip_prefix("__string_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::StringCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__regex_") {
                                    let method = cname.strip_prefix("__regex_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::RegexCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__datetime_") {
                                    let method = cname.strip_prefix("__datetime_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::DateTimeCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__auth_") {
                                    let method = cname.strip_prefix("__auth_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::AuthCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__worker_") {
                                    let method = cname.strip_prefix("__worker_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::WorkerCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__langchain_") {
                                    let method = cname.strip_prefix("__langchain_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::LangChainCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__langgraph_") {
                                    let method = cname.strip_prefix("__langgraph_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::LangGraphCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__dict_") {
                                    let method = cname.strip_prefix("__dict_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::DictCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__json_") {
                                    let method = cname.strip_prefix("__json_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::JsonCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname == "__is_instanceof" {
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    let val = mir_args.first().cloned().unwrap_or(MirValue::Nil);
                                    let tn = mir_args.get(1).cloned().unwrap_or(MirValue::Nil);
                                    stmts.push(MirStmt::IsCall {
                                        result: Some(name.clone()),
                                        value: val,
                                        type_name: tn,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__math_") {
                                    let method = cname.strip_prefix("__math_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::MathCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__env_") {
                                    let method = cname.strip_prefix("__env_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::EnvCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                                if cname.starts_with("__http_") {
                                    let method = cname.strip_prefix("__http_").unwrap_or(cname).to_string();
                                    let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                                    stmts.push(MirStmt::HttpCall {
                                        result: Some(name.clone()),
                                        method,
                                        args: mir_args,
                                        dbg_line: line,
                                    });
                                    return;
                                }
                            }
                        }
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
                // Check for await expression first
                if let HirExpr::Await(inner) = expr {
                    // Emit the awaited expression as statements, then store result
                    let await_stmts = Vec::new();
                    // Lower the inner expression as a Mir value
                    let _mir_val = self.lower_expr(inner);
                    stmts.push(MirStmt::Await {
                        value: await_stmts,
                        result_target: None,
                        dbg_line: line,
                    });
                    return;
                }
                // Check if this is a console call expression
                if let HirExpr::Call { callee, args } = expr {
                    if let HirExpr::Ident(name) = callee.as_ref() {
                        if name.starts_with("__console_") {
                            let method = name.strip_prefix("__console_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::ConsoleCall {
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__fs_") {
                            let method = name.strip_prefix("__fs_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::FsCall {
                                result: None, // void context
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__transport_") {
                            let method = name.strip_prefix("__transport_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::TransportCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__string_") {
                            let method = name.strip_prefix("__string_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::StringCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__regex_") {
                            let method = name.strip_prefix("__regex_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::RegexCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__datetime_") {
                            let method = name.strip_prefix("__datetime_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::DateTimeCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__auth_") {
                            let method = name.strip_prefix("__auth_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::AuthCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__worker_") {
                            let method = name.strip_prefix("__worker_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::WorkerCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__langchain_") {
                            let method = name.strip_prefix("__langchain_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::LangChainCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__langgraph_") {
                            let method = name.strip_prefix("__langgraph_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::LangGraphCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__dict_") {
                            let method = name.strip_prefix("__dict_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::DictCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__json_") {
                            let method = name.strip_prefix("__json_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::JsonCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__math_") {
                            let method = name.strip_prefix("__math_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::MathCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__env_") {
                            let method = name.strip_prefix("__env_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::EnvCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name.starts_with("__http_") {
                            let method = name.strip_prefix("__http_").unwrap_or(name).to_string();
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            stmts.push(MirStmt::HttpCall {
                                result: None,
                                method,
                                args: mir_args,
                                dbg_line: line,
                            });
                            return;
                        }
                        if name == "__is_instanceof" {
                            let mir_args: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            let val = mir_args.first().cloned().unwrap_or(MirValue::Nil);
                            let tn = mir_args.get(1).cloned().unwrap_or(MirValue::Nil);
                            stmts.push(MirStmt::IsCall {
                                result: None,
                                value: val,
                                type_name: tn,
                                dbg_line: line,
                            });
                            return;
                        }
                    }
                }
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
            HirStmt::Wait(millis, _line) => {
                stmts.push(MirStmt::Wait(*millis, line));
            }
            HirStmt::Parallel { blocks, line: _ } => {
                let mut mir_blocks = Vec::new();
                for block in blocks {
                    let mut block_stmts = Vec::new();
                    for s in &block.stmts {
                        self.lower_stmt(s, &mut block_stmts);
                    }
                    mir_blocks.push(block_stmts);
                }
                stmts.push(MirStmt::Parallel {
                    blocks: mir_blocks,
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
                        if name == "__is_instanceof" {
                            let lowered: Vec<MirValue> = args.iter().map(|a| self.lower_expr(a)).collect();
                            let val = lowered.first().cloned().unwrap_or(MirValue::Nil);
                            let tn = lowered.get(1).cloned().unwrap_or(MirValue::Nil);
                            MirValue::IsInstanceof {
                                value: Box::new(val),
                                type_name: Box::new(tn),
                            }
                        } else {
                            MirValue::Local(format!("__call_{}({})__", name, args.iter().map(|_| "_").collect::<Vec<_>>().join(",")))
                        }
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
            HirExpr::Await(inner) => self.lower_expr(inner),
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
                schedule_expr: None,
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
