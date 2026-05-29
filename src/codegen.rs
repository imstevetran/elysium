use crate::debug::DebugInfo;
use crate::error::Result;
use crate::mir::*;
use inkwell::context::Context;
use inkwell::types::BasicType;
use inkwell::values::BasicMetadataValueEnum;

/// LLVM IR code generator for Elysium.
pub struct Codegen {
    context: &'static Context,
    module: inkwell::module::Module<'static>,
    debug: Option<DebugInfo<'static>>,
}

impl Codegen {
    pub fn new(module_name: &str) -> Result<Self> {
        let context = Box::leak(Box::new(Context::create()));
        let module = context.create_module(module_name);

        Ok(Self {
            context,
            module,
            debug: None,
        })
    }

    /// Attach a DebugInfo instance.
    pub fn set_debug_info(&mut self, debug: DebugInfo<'static>) {
        self.debug = Some(debug);
    }

    pub fn compile(&mut self, program: &MirProgram, source_path: &str) -> Result<()> {
        // Initialise debug info
        if let Some(ref mut di) = self.debug {
            di.init(&self.module, source_path);
        }

        for func in &program.functions {
            self.emit_function(func, source_path)?;
        }

        // Finalise debug info
        if let Some(ref di) = self.debug {
            di.finalize();
        }

        Ok(())
    }

    fn emit_function(&mut self, func: &MirFunction, _source_path: &str) -> Result<()> {
        let ret_ty = self.mir_type(&func.return_type);

        let mut param_meta = Vec::new();
        for param in &func.params {
            let ty = self.mir_type(&param.ty);
            let meta_ty = match ty {
                inkwell::types::BasicTypeEnum::IntType(i) => i.into(),
                inkwell::types::BasicTypeEnum::FloatType(f) => f.into(),
                inkwell::types::BasicTypeEnum::PointerType(p) => p.into(),
                _ => {
                    let ptr = self.context.ptr_type(inkwell::AddressSpace::default());
                    ptr.into()
                }
            };
            param_meta.push(meta_ty);
        }

        let fn_type = ret_ty.fn_type(&param_meta, false);
        let fn_val = self.module.add_function(&func.name, fn_type, None);

        // Create DISubprogram for this function
        if let Some(ref mut di) = self.debug {
            di.create_function(&fn_val, func);
        }

        let entry = self.context.append_basic_block(fn_val, "entry");
        let builder = self.context.create_builder();
        builder.position_at_end(entry);

        // Set debug location for function entry
        if let Some(ref di) = self.debug {
            di.set_location(&builder, &self.module, func.dbg_line);
        }

        // Store params into allocas
        for (i, param) in func.params.iter().enumerate() {
            let param_llvm = fn_val.get_nth_param(i as u32).unwrap();
            let ty = self.mir_type(&param.ty);
            // Set debug location for parameter
            if let Some(ref di) = self.debug {
                di.set_location(&builder, &self.module, param.dbg_line);
            }
            let alloca = builder.build_alloca(ty, &param.name).expect("alloca");
            builder.build_store(alloca, param_llvm).expect("store");
        }

        // Emit body stmts
        for stmt in &func.body.stmts {
            self.emit_stmt(stmt, &builder, func)?;
        }

        // Default return 0
        let ret_val = self.context.i64_type().const_zero();
        builder.build_return(Some(&ret_val)).expect("return");

        Ok(())
    }

    fn emit_stmt(
        &self,
        stmt: &MirStmt,
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
    ) -> Result<()> {
        let line = match stmt {
            MirStmt::Alloca { dbg_line, .. } => *dbg_line,
            MirStmt::Store { dbg_line, .. } => *dbg_line,
            MirStmt::Call { dbg_line, .. } => *dbg_line,
            MirStmt::Return(_, line) => *line,
            MirStmt::CondBranch { dbg_line, .. } => *dbg_line,
            MirStmt::UnsafeBlock(_) => func.dbg_line,
            MirStmt::Bench { dbg_line, .. } => *dbg_line,
            MirStmt::ConsoleCall { dbg_line, .. } => *dbg_line,
            _ => func.dbg_line,
        };

        if let Some(ref di) = self.debug {
            di.set_location(builder, &self.module, line);
        }

        match stmt {
            MirStmt::Alloca { name, ty, is_mutable: _, is_lazy: _, dbg_line: _ } => {
                let llvm_ty = self.mir_type(ty);
                builder.build_alloca(llvm_ty, name).expect("alloca");
            }
            MirStmt::Store { .. } => {}
            MirStmt::Return(ret, _) => {
                if let Some(_value) = ret {
                    let ret_val = self.context.i64_type().const_zero();
                    builder.build_return(Some(&ret_val)).expect("return");
                } else {
                    builder.build_return(None).expect("return");
                }
            }
            MirStmt::Bench { .. } => {
                self.emit_bench_stmt(stmt, builder, func)?;
            }
            MirStmt::ConsoleCall { method, args, dbg_line: _ } => {
                self.emit_console_call(method, args, builder)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn emit_bench_stmt(
        &self,
        bench: &MirStmt,
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
    ) -> Result<()> {
        let body_stmts = match bench {
            MirStmt::Bench { body_stmts, dbg_line: _ } => body_stmts,
            _ => return Ok(()),
        };

        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let f64_ty = self.context.f64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        // Declare external functions
        let clock_gettime = self.module.add_function(
            "clock_gettime",
            i32_ty.fn_type(&[i32_ty.into(), ptr_ty.into()], false),
            None,
        );
        let printf_fn = self.module.add_function(
            "printf",
            i32_ty.fn_type(&[ptr_ty.into()], true),
            None,
        );

        // Build format string: "bench: %.6f s\n"
        let fmt_global_ptr = builder.build_global_string_ptr("bench: %.6f s\n", "__bench_fmt")
            .map_err(|e| crate::error::CompileError::new(format!("global string: {}", e)))?;
        let fmt_ptr = fmt_global_ptr.as_pointer_value();

        // Allocate a struct timespec (two i64s)
        let ts_ty = i64_ty.array_type(2);
        let ts_alloc = builder.build_alloca(ts_ty, "__bench_ts").expect("ts alloc");

        // CLOCK_MONOTONIC = 1
        let zero_i32 = i32_ty.const_zero();
        let one_i32 = i32_ty.const_int(1, false);
        let clock_id = one_i32;

        // --- Start time: clock_gettime(1, &ts) ---
        let cgt_args: &[BasicMetadataValueEnum] = &[clock_id.into(), ts_alloc.into()];
        builder.build_call(clock_gettime, cgt_args, "__bench_start_call")
            .map_err(|e| crate::error::CompileError::new(format!("clock_gettime start: {}", e)))?;

        // Load start tv_sec (index 0)
        let start_sec_ptr = unsafe {
            builder.build_in_bounds_gep(ts_ty, ts_alloc, &[zero_i32, zero_i32], "__bench_start_sec_ptr")
        }.map_err(|e| crate::error::CompileError::new(format!("start_sec gep: {}", e)))?;
        let start_sec = builder.build_load(i64_ty, start_sec_ptr, "__bench_start_sec")
            .map_err(|e| crate::error::CompileError::new(format!("load start_sec: {}", e)))?
            .into_int_value();

        // Load start tv_nsec (index 1)
        let start_nsec_ptr = unsafe {
            builder.build_in_bounds_gep(ts_ty, ts_alloc, &[zero_i32, one_i32], "__bench_start_nsec_ptr")
        }.map_err(|e| crate::error::CompileError::new(format!("start_nsec gep: {}", e)))?;
        let start_nsec = builder.build_load(i64_ty, start_nsec_ptr, "__bench_start_nsec")
            .map_err(|e| crate::error::CompileError::new(format!("load start_nsec: {}", e)))?
            .into_int_value();

        // --- Benchmark body ---
        for s in body_stmts {
            self.emit_stmt(s, builder, func)?;
        }

        // --- End time ---
        builder.build_call(clock_gettime, cgt_args, "__bench_end_call")
            .map_err(|e| crate::error::CompileError::new(format!("clock_gettime end: {}", e)))?;

        // Load end tv_sec
        let end_sec = builder.build_load(i64_ty, start_sec_ptr, "__bench_end_sec")
            .map_err(|e| crate::error::CompileError::new(format!("load end_sec: {}", e)))?
            .into_int_value();

        // Load end tv_nsec
        let end_nsec = builder.build_load(i64_ty, start_nsec_ptr, "__bench_end_nsec")
            .map_err(|e| crate::error::CompileError::new(format!("load end_nsec: {}", e)))?
            .into_int_value();

        // elapsed = (end_sec - start_sec) * 1_000_000_000 + (end_nsec - start_nsec)
        let sec_diff = builder.build_int_sub(end_sec, start_sec, "__bench_sec_diff")
            .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
        let nsec_diff = builder.build_int_sub(end_nsec, start_nsec, "__bench_nsec_diff")
            .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
        let sec_to_nsec = builder.build_int_mul(sec_diff, i64_ty.const_int(1_000_000_000, false), "__bench_sec_to_ns")
            .map_err(|e| crate::error::CompileError::new(format!("mul: {}", e)))?;
        let total_ns = builder.build_int_add(sec_to_nsec, nsec_diff, "__bench_total_ns")
            .map_err(|e| crate::error::CompileError::new(format!("add: {}", e)))?;

        // Convert to double seconds
        let ns_double = builder.build_signed_int_to_float(total_ns, f64_ty, "__bench_ns_double")
            .map_err(|e| crate::error::CompileError::new(format!("sitofp: {}", e)))?;
        let one_billion = f64_ty.const_float(1_000_000_000.0);
        let seconds = builder.build_float_div(ns_double, one_billion, "__bench_seconds")
            .map_err(|e| crate::error::CompileError::new(format!("fdiv: {}", e)))?;

        // printf(fmt_ptr, seconds)
        let printf_args: &[BasicMetadataValueEnum] = &[fmt_ptr.into(), seconds.into()];
        builder.build_call(printf_fn, printf_args, "__bench_printf_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf bench: {}", e)))?;

        Ok(())
    }

    pub fn write_to_file(&self, path: &str) -> Result<()> {
        if self.module.verify().is_ok() {
            self.module.print_to_file(path).map_err(|e| {
                crate::error::CompileError::new(format!("Failed to write bitcode: {}", e))
            })?;
        }
        Ok(())
    }

    pub fn print_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    fn mir_type(&self, ty: &MirType) -> inkwell::types::BasicTypeEnum<'static> {
        match ty {
            MirType::Int => self.context.i64_type().as_basic_type_enum(),
            MirType::Float => self.context.f64_type().as_basic_type_enum(),
            MirType::Bool => self.context.bool_type().as_basic_type_enum(),
            MirType::String | MirType::Char | MirType::Nil | MirType::Ptr(_) | MirType::Array(_) => {
                self.context.i8_type().as_basic_type_enum()
            }
        }
    }

    /// Emit a printf-based console call.
    /// The format string embeds the log level prefix and formats all args.
    fn emit_console_call(
        &self,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i32_ty = self.context.i32_type();

        // Declare printf once
        let printf_fn = self.module.add_function(
            "printf",
            i32_ty.fn_type(&[ptr_ty.into()], true),
            None,
        );

        // Determine prefix based on method
        let prefix = match method.as_ref() {
            "debug" => "[DEBUG] ",
            "info" | "log" => "[INFO] ",
            "warn" => "[WARN] ",
            "error" => "[ERROR] ",
            "print" => "",
            "println" => "",
            _ => "[LOG] ",
        };

        // Build format string: prefix + specifiers for each arg + newline for println
        let mut fmt = prefix.to_string();
        for _ in args {
            fmt.push_str("%s");
            fmt.push(' '); // space between args
        }
        if method == "println" || method == "debug" || method == "info"
            || method == "log" || method == "warn" || method == "error" {
            fmt.push('\n');
        }

        // Create the format string as a global
        let fmt_name = format!("__console_{}_fmt", method);
        let fmt_global = builder.build_global_string_ptr(&fmt, &fmt_name)
            .map_err(|e| crate::error::CompileError::new(format!("console fmt: {}", e)))?;
        let fmt_ptr = fmt_global.as_pointer_value();

        // Convert each arg to a string pointer representation
        let mut printf_args: Vec<BasicMetadataValueEnum> = vec![fmt_ptr.into()];
        for arg in args {
            let str_val = self.console_arg_to_string(arg, builder);
            printf_args.push(str_val.into());
        }

        builder.build_call(printf_fn, &printf_args, &format!("__console_{}_call", method))
            .map_err(|e| crate::error::CompileError::new(format!("printf console: {}", e)))?;

        Ok(())
    }

    /// Convert a single MirValue argument into a `char*` string pointer for printf("%s", ...).
    fn console_arg_to_string(
        &self,
        val: &MirValue,
        builder: &inkwell::builder::Builder<'static>,
    ) -> inkwell::values::PointerValue<'static> {
        match val {
            MirValue::IntLit(v) => {
                // Build a formatted string like "42" as a global
                let s = format!("{}", v);
                builder.build_global_string_ptr(&s, "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::FloatLit(v) => {
                let s = format!("{}", v);
                builder.build_global_string_ptr(&s, "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::BoolLit(v) => {
                let s = if *v { "true" } else { "false" };
                builder.build_global_string_ptr(s, "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::StringLit(s) => {
                builder.build_global_string_ptr(s, "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::CharLit(c) => {
                let s = format!("{}", c);
                builder.build_global_string_ptr(&s, "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::Nil => {
                builder.build_global_string_ptr("nil", "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::Local(name) => {
                // Try to load from alloca — but for now emit "?" placeholder
                builder.build_global_string_ptr(&format!("<{}>", name), "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
            MirValue::BinaryOp { .. } | MirValue::UnaryOp { .. } => {
                builder.build_global_string_ptr("<expr>", "__console_arg_str")
                    .expect("global arg str")
                    .as_pointer_value()
            }
        }
    }
}
