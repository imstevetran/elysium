use crate::debug::DebugInfo;
use crate::error::Result;
use crate::mir::*;
use inkwell::context::Context;
use inkwell::types::BasicType;
use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::AnyValue;

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
            MirStmt::FsCall { dbg_line, .. } => *dbg_line,
            MirStmt::TransportCall { dbg_line, .. } => *dbg_line,
            MirStmt::StringCall { dbg_line, .. } => *dbg_line,
            MirStmt::RegexCall { dbg_line, .. } => *dbg_line,
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
            MirStmt::FsCall { result, method, args, dbg_line: _ } => {
                self.emit_fs_call(result, method, args, builder, func)?;
            }
            MirStmt::TransportCall { result, method, args, dbg_line: _ } => {
                self.emit_transport_call(result, method, args, builder)?;
            }
            MirStmt::StringCall { result, method, args, dbg_line: _ } => {
                self.emit_string_call(result, method, args, builder)?;
            }
            MirStmt::RegexCall { result, method, args, dbg_line: _ } => {
                self.emit_regex_call(result, method, args, builder)?;
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
            _ => "[LOG] ",
        };

        // Build format string: prefix + specifiers for each arg + newline for println
        let mut fmt = prefix.to_string();
        for _ in args {
            fmt.push_str("%s");
            fmt.push(' '); // space between args
        }
        if method == "debug" || method == "info"
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

    /// Get an i8* from a MirValue argument (for passing to C functions).
    fn mir_value_as_cstr_ptr(
        &self,
        val: &MirValue,
        builder: &inkwell::builder::Builder<'static>,
    ) -> Option<inkwell::values::PointerValue<'static>> {
        match val {
            MirValue::StringLit(s) => {
                let gv = builder.build_global_string_ptr(s, "__fs_arg_str")
                    .expect("fs arg str");
                Some(gv.as_pointer_value())
            }
            _ => None,
        }
    }

    /// Emit a filesystem call using C stdlib functions.
    fn emit_fs_call(
        &self,
        _result: &Option<String>,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
        _func: &MirFunction,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i8_ty = self.context.i8_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        match method {
            "readFile" | "readFileSync" => {
                if let Some(path) = args.first().and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                    let open_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let open_fn = self.module.add_function("fopen", open_ty, None);
                    let read_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
                    let read_fn = self.module.add_function("fgets", read_ty, None);
                    let close_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let close_fn = self.module.add_function("fclose", close_ty, None);
                    let print_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
                    let print_fn = self.module.add_function("printf", print_ty, None);

                    let mode_r = builder.build_global_string_ptr("r", "__fs_mode_r").expect("mode r");
                    let fp_val = builder.build_call(open_fn, &[path.into(), mode_r.as_pointer_value().into()], "__fs_fp")
                        .map_err(|e| crate::error::CompileError::new(format!("fopen: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();

                    let buf = builder.build_alloca(i8_ty.array_type(4096), "__fs_buf").expect("buf");
                    let zero = i32_ty.const_zero();
                    let buf_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty.array_type(4096), buf, &[zero, zero], "__fs_buf_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let _ = builder.build_call(read_fn, &[buf_ptr.into(), i32_ty.const_int(4096, false).into(), fp_val.into()], "__fs_fgets")
                        .map_err(|e| crate::error::CompileError::new(format!("fgets: {}", e)))?;
                    let _ = builder.build_call(close_fn, &[fp_val.into()], "__fs_fclose")
                        .map_err(|e| crate::error::CompileError::new(format!("fclose: {}", e)))?;

                    let fmt = builder.build_global_string_ptr("%s\n", "__fs_fmt").expect("fmt");
                    let _ = builder.build_call(print_fn, &[fmt.as_pointer_value().into(), buf_ptr.into()], "__fs_printf")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                }
            }
            "writeFile" | "appendFile" => {
                let mode = if method == "writeFile" { "w" } else { "a" };
                let path = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                let content = args.get(1).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                if let (Some(p), Some(c)) = (path, content) {
                    let open_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let open_fn = self.module.add_function("fopen", open_ty, None);
                    let write_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let write_fn = self.module.add_function("fputs", write_ty, None);
                    let close_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let close_fn = self.module.add_function("fclose", close_ty, None);
                    let mode_ptr = builder.build_global_string_ptr(mode, &format!("__fs_mode_{}", mode)).expect("mode");
                    let fp_val = builder.build_call(open_fn, &[p.into(), mode_ptr.as_pointer_value().into()], "__fs_fp")
                        .map_err(|e| crate::error::CompileError::new(format!("fopen: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let _ = builder.build_call(write_fn, &[c.into(), fp_val.into()], "__fs_fputs")
                        .map_err(|e| crate::error::CompileError::new(format!("fputs: {}", e)))?;
                    let _ = builder.build_call(close_fn, &[fp_val.into()], "__fs_fclose")
                        .map_err(|e| crate::error::CompileError::new(format!("fclose: {}", e)))?;
                }
            }
            "removeFile" => {
                if let Some(path) = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                    let remove_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let remove_fn = self.module.add_function("remove", remove_ty, None);
                    let _ = builder.build_call(remove_fn, &[path.into()], "__fs_remove")
                        .map_err(|e| crate::error::CompileError::new(format!("remove: {}", e)))?;
                }
            }
            "exists" | "isFile" | "isDir" => {
                if let Some(path) = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                    let access_ty = i32_ty.fn_type(&[ptr_ty.into(), i32_ty.into()], false);
                    let access_fn = self.module.add_function("access", access_ty, None);
                    let _ = builder.build_call(access_fn, &[path.into(), i32_ty.const_zero().into()], "__fs_access")
                        .map_err(|e| crate::error::CompileError::new(format!("access: {}", e)))?;
                }
            }
            "createDir" => {
                if let Some(path) = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                    let mkdir_ty = i32_ty.fn_type(&[ptr_ty.into(), i32_ty.into()], false);
                    let mkdir_fn = self.module.add_function("mkdir", mkdir_ty, None);
                    let _ = builder.build_call(mkdir_fn, &[path.into(), i32_ty.const_int(0o755, false).into()], "__fs_mkdir")
                        .map_err(|e| crate::error::CompileError::new(format!("mkdir: {}", e)))?;
                }
            }
            "removeDir" => {
                if let Some(path) = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                    let rmdir_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let rmdir_fn = self.module.add_function("rmdir", rmdir_ty, None);
                    let _ = builder.build_call(rmdir_fn, &[path.into()], "__fs_rmdir")
                        .map_err(|e| crate::error::CompileError::new(format!("rmdir: {}", e)))?;
                }
            }
            "copyFile" => {
                let src = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                let dst = args.get(1).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                if let (Some(s), Some(d)) = (src, dst) {
                    let open_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let open_fn = self.module.add_function("fopen", open_ty, None);
                    let read_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
                    let read_fn = self.module.add_function("fgets", read_ty, None);
                    let write_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let write_fn = self.module.add_function("fputs", write_ty, None);
                    let close_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let close_fn = self.module.add_function("fclose", close_ty, None);

                    let mode_r = builder.build_global_string_ptr("r", "__fs_mode_r").expect("mode r");
                    let mode_w = builder.build_global_string_ptr("w", "__fs_mode_w").expect("mode w");
                    let src_fp = builder.build_call(open_fn, &[s.into(), mode_r.as_pointer_value().into()], "__fs_open_src")
                        .map_err(|e| crate::error::CompileError::new(format!("fopen src: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let dst_fp = builder.build_call(open_fn, &[d.into(), mode_w.as_pointer_value().into()], "__fs_open_dst")
                        .map_err(|e| crate::error::CompileError::new(format!("fopen dst: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();

                    let buf = builder.build_alloca(i8_ty.array_type(4096), "__fs_copy_buf").expect("buf");
                    let zero = i32_ty.const_zero();
                    let buf_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty.array_type(4096), buf, &[zero, zero], "__fs_buf_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let _ = builder.build_call(read_fn, &[buf_ptr.into(), i32_ty.const_int(4096, false).into(), src_fp.into()], "__fs_fgets")
                        .map_err(|e| crate::error::CompileError::new(format!("fgets: {}", e)))?;
                    let _ = builder.build_call(write_fn, &[buf_ptr.into(), dst_fp.into()], "__fs_fputs")
                        .map_err(|e| crate::error::CompileError::new(format!("fputs: {}", e)))?;

                    let _ = builder.build_call(close_fn, &[src_fp.into()], "__fs_fclose_src")
                        .map_err(|e| crate::error::CompileError::new(format!("fclose: {}", e)))?;
                    let _ = builder.build_call(close_fn, &[dst_fp.into()], "__fs_fclose_dst")
                        .map_err(|e| crate::error::CompileError::new(format!("fclose: {}", e)))?;
                }
            }
            "rename" => {
                let old = args.get(0).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                let new = args.get(1).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                if let (Some(o), Some(n)) = (old, new) {
                    let rename_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let rename_fn = self.module.add_function("rename", rename_ty, None);
                    let _ = builder.build_call(rename_fn, &[o.into(), n.into()], "__fs_rename")
                        .map_err(|e| crate::error::CompileError::new(format!("rename: {}", e)))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Emit a transport call (HTTP, WebSocket, MQTT).
    /// Since these operations don't map to C stdlib, emit a runtime stub
    /// that prints a message. The real implementation lives in the JS runtime.
    fn emit_transport_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);
        let msg = format!("[transport] {}: use JS runtime\n", method);
        let fmt = builder.build_global_string_ptr(&msg, "__transport_fmt").expect("fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__transport_printf")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    /// Emit a string operation call.
    /// For `length`, uses C `strlen`. For everything else, emits a runtime stub
    /// since C codegen can't allocate/manipulate strings easily.
    fn emit_string_call(
        &self,
        result: &Option<String>,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        // Helper: get the string receiver as an i8* from the first arg
        let str_arg = args.first().and_then(|a| self.mir_value_as_cstr_ptr(a, builder));

        match method {
            "length" => {
                if let Some(s) = str_arg {
                    let strlen_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
                    let strlen_fn = self.module.add_function("strlen", strlen_ty, None);
                    let len_val = builder.build_call(strlen_fn, &[s.into()], "__strlen_call")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    if let Some(dest) = result {
                        let dest_alloca = builder.build_alloca(i64_ty, &format!("__str_result_{}", dest))
                            .expect("str result alloca");
                        builder.build_store(dest_alloca, len_val)
                            .map_err(|e| crate::error::CompileError::new(format!("store strlen: {}", e)))?;
                    }
                }
            }
            "isEmpty" => {
                if let Some(s) = str_arg {
                    let strlen_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
                    let strlen_fn = self.module.add_function("strlen", strlen_ty, None);
                    let len_val = builder.build_call(strlen_fn, &[s.into()], "__strlen_call")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let zero = i64_ty.const_zero();
                    let is_empty = builder.build_int_compare(
                        inkwell::IntPredicate::EQ, len_val, zero, "__str_is_empty",
                    ).map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    if let Some(dest) = result {
                        let bool_ty = self.context.bool_type();
                        let dest_alloca = builder.build_alloca(bool_ty, &format!("__str_result_{}", dest))
                            .expect("str result alloca");
                        builder.build_store(dest_alloca, is_empty)
                            .map_err(|e| crate::error::CompileError::new(format!("store is_empty: {}", e)))?;
                    }
                }
            }
            _ => {
                let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
                let printf_fn = self.module.add_function("printf", printf_ty, None);
                let msg = format!("[string] {}: use JS runtime\n", method);
                let fmt = builder.build_global_string_ptr(&msg, "__str_fmt").expect("fmt");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__str_printf")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Emit a regex call.
    /// Regex operations don't map to C stdlib — emit a runtime stub.
    fn emit_regex_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);
        let msg = format!("[regex] {}: use JS runtime\n", method);
        let fmt = builder.build_global_string_ptr(&msg, "__regex_fmt").expect("fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__regex_printf")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }
}
