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
            MirStmt::DateTimeCall { dbg_line, .. } => *dbg_line,
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
            MirStmt::DateTimeCall { result, method, args, dbg_line: _ } => {
                self.emit_datetime_call(result, method, args, builder)?;
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
    /// HTTP methods use popen+curl for the C backend.
    /// WebSocket and MQTT remain stubs (require event-loop infrastructure).
    fn emit_transport_call(
        &self,
        result: &Option<String>,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let _i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let zero_i32 = i32_ty.const_zero();
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);

        // Helper: capture string result
        let store_str = |buf_ptr: inkwell::values::PointerValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(ptr_ty, &format!("__transport_result_{}", d)).expect("alloca");
                builder.build_store(alloca, buf_ptr).ok();
            }
        };

        // Helper: popen + fgets for shell command
        let run_cmd = |cmd: &str,
                       dest: &Option<String>,
                       label: &str| -> Result<()> {
            let popen_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
            let popen_fn = self.module.add_function("popen", popen_ty, None);
            let fgets_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
            let fgets_fn = self.module.add_function("fgets", fgets_ty, None);
            let pclose_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
            let pclose_fn = self.module.add_function("pclose", pclose_ty, None);

            let cmd_ptr = builder.build_global_string_ptr(cmd, &format!("__{}_cmd", label)).expect("cmd");
            let mode_r = builder.build_global_string_ptr("r", &format!("__{}_mode", label)).expect("mode");
            let fp = builder.build_call(popen_fn, &[cmd_ptr.as_pointer_value().into(), mode_r.as_pointer_value().into()], &format!("__{}_fp", label))
                .map_err(|e| crate::error::CompileError::new(format!("popen: {}", e)))?
                .as_any_value_enum()
                .into_pointer_value();

            let arr_ty = i8_ty.array_type(8192);
            let buf = builder.build_alloca(arr_ty, &format!("__{}_buf", label)).expect("buf");
            let buf_ptr = unsafe {
                builder.build_in_bounds_gep(arr_ty, buf, &[zero_i32, zero_i32], &format!("__{}_buf_ptr", label))
            }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

            let _ = builder.build_call(fgets_fn, &[buf_ptr.into(), i32_ty.const_int(8192, false).into(), fp.into()], &format!("__{}_fgets", label))
                .map_err(|e| crate::error::CompileError::new(format!("fgets: {}", e)))?;
            let _ = builder.build_call(pclose_fn, &[fp.into()], &format!("__{}_pclose", label))
                .map_err(|e| crate::error::CompileError::new(format!("pclose: {}", e)))?;

            let fmt = builder.build_global_string_ptr("%s\n", &format!("__{}_fmt", label)).expect("fmt");
            let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into(), buf_ptr.into()], &format!("__{}_print", label))
                .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            store_str(buf_ptr, dest);
            Ok(())
        };

        // Helper: get URL from args
        let url_arg = args.first().and_then(|a| match a {
            MirValue::StringLit(s) => Some(s.as_str()),
            _ => None,
        });

        match method {
            "get" => {
                if let Some(url) = url_arg {
                    let cmd = format!("curl -s -m 10 '{}'", url);
                    run_cmd(&cmd, result, "http_get")?;
                }
            }
            "post" => {
                if let Some(url) = url_arg {
                    let body = args.get(1).and_then(|a| match a { MirValue::StringLit(s) => Some(s.as_str()), _ => None }).unwrap_or("");
                    let cmd = format!("curl -s -m 10 -d '{}' '{}'", body, url);
                    run_cmd(&cmd, result, "http_post")?;
                }
            }
            "put" => {
                if let Some(url) = url_arg {
                    let body = args.get(1).and_then(|a| match a { MirValue::StringLit(s) => Some(s.as_str()), _ => None }).unwrap_or("");
                    let cmd = format!("curl -s -m 10 -X PUT -d '{}' '{}'", body, url);
                    run_cmd(&cmd, result, "http_put")?;
                }
            }
            "delete" => {
                if let Some(url) = url_arg {
                    let cmd = format!("curl -s -m 10 -X DELETE '{}'", url);
                    run_cmd(&cmd, result, "http_delete")?;
                }
            }
            // WebSocket & MQTT — genuinely need event-loop infrastructure, keep stubs
            _ => {
                let msg = format!("[transport] {}: use JS runtime (websocket/mqtt)\n", method);
                let fmt = builder.build_global_string_ptr(&msg, "__transport_fmt_stub").expect("fmt");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__transport_printf")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
        }
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
        let i8_ty = self.context.i8_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let zero_i64 = i64_ty.const_zero();
        let zero_i32 = i32_ty.const_zero();

        // Helper: get the string receiver as an i8* from the first arg
        let str_arg = args.first().and_then(|a| self.mir_value_as_cstr_ptr(a, builder));

        // Helper: get second string arg
        let second_arg = args.get(1).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));

        // Helper: get an int arg
        let int_arg = |idx: usize| -> inkwell::values::IntValue<'static> {
            args.get(idx).map(|a| match a {
                MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                _ => zero_i64,
            }).unwrap_or(zero_i64)
        };

        // Helper: printf for printing results
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);

        // Helper: create a stack buffer for string results
        let make_buf = || -> (inkwell::values::PointerValue<'static>, inkwell::values::PointerValue<'static>) {
            let arr_ty = i8_ty.array_type(4096);
            let buf = builder.build_alloca(arr_ty, "__str_result_buf").expect("str buf");
            let buf_ptr = unsafe {
                builder.build_in_bounds_gep(arr_ty, buf, &[zero_i32, zero_i32], "__str_result_ptr")
            }.expect("str buf gep");
            (buf, buf_ptr)
        };

        // Helper: store a string buffer pointer into result if needed
        let store_str = |buf_ptr: inkwell::values::PointerValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(ptr_ty, &format!("__str_result_{}", d)).expect("alloca");
                builder.build_store(alloca, buf_ptr).ok();
            }
        };

        // Helper: store an int value into result if needed
        let store_int = |val: inkwell::values::IntValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(i64_ty, &format!("__str_result_{}", d)).expect("alloca");
                builder.build_store(alloca, val).ok();
            }
        };

        // Helper: store a bool value into result if needed
        let store_bool = |val: inkwell::values::IntValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(self.context.bool_type(), &format!("__str_result_{}", d)).expect("alloca");
                builder.build_store(alloca, val).ok();
            }
        };

        // Declare common C functions once
        let strlen_fn = self.module.add_function("strlen", i64_ty.fn_type(&[ptr_ty.into()], false), None);
        let strstr_fn = self.module.add_function("strstr", ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false), None);
        let strncmp_fn = self.module.add_function("strncmp", i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i64_ty.into()], false), None);
        let snprintf_fn = self.module.add_function("snprintf", i32_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), ptr_ty.into()], true), None);
        let _tolower_fn = self.module.add_function("tolower", i32_ty.fn_type(&[i32_ty.into()], false), None);
        let _toupper_fn = self.module.add_function("toupper", i32_ty.fn_type(&[i32_ty.into()], false), None);
        let _isspace_fn = self.module.add_function("isspace", i32_ty.fn_type(&[i32_ty.into()], false), None);

        match method {
            "length" => {
                if let Some(s) = str_arg {
                    let len_val = builder.build_call(strlen_fn, &[s.into()], "__strlen_call")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    store_int(len_val, result);
                }
            }
            "isEmpty" => {
                if let Some(s) = str_arg {
                    let len_val = builder.build_call(strlen_fn, &[s.into()], "__strlen_call")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let is_empty = builder.build_int_compare(inkwell::IntPredicate::EQ, len_val, zero_i64, "__is_empty")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    store_bool(is_empty, result);
                }
            }
            "toString" => {
                if let Some(s) = str_arg {
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let (_, bp) = make_buf();
                    let _ = builder.build_call(snprintf_fn, &[bp.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), bp.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(bp, result);
                }
            }
            "startsWith" => {
                if let (Some(s), Some(prefix)) = (str_arg, second_arg) {
                    // strncmp(s, prefix, strlen(prefix)) == 0
                    let plen = builder.build_call(strlen_fn, &[prefix.into()], "__plen")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let cmp = builder.build_call(strncmp_fn, &[s.into(), prefix.into(), plen.into()], "__strncmp")
                        .map_err(|e| crate::error::CompileError::new(format!("strncmp: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let zero_i32 = i32_ty.const_zero();
                    let result_val = builder.build_int_compare(inkwell::IntPredicate::EQ, cmp, zero_i32, "__starts_with")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    store_bool(result_val, result);
                }
            }
            "endsWith" => {
                if let (Some(s), Some(suffix)) = (str_arg, second_arg) {
                    let slen = builder.build_call(strlen_fn, &[s.into()], "__slen")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let suflen = builder.build_call(strlen_fn, &[suffix.into()], "__suflen")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    // offset = slen - suflen, only if suflen <= slen
                    // Compare strncmp(s + offset, suffix, suflen)
                    let offset = builder.build_int_sub(slen, suflen, "__offset")
                        .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                    let suf_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty, s, &[offset], "__suffix_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let cmp = builder.build_call(strncmp_fn, &[suf_ptr.into(), suffix.into(), suflen.into()], "__strncmp_end")
                        .map_err(|e| crate::error::CompileError::new(format!("strncmp: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let ge = builder.build_int_compare(inkwell::IntPredicate::SGE, slen, suflen, "__ge")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    let eq = builder.build_int_compare(inkwell::IntPredicate::EQ, cmp, i32_ty.const_zero(), "__eq")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    let result_val = builder.build_and(ge, eq, "__ends_with")
                        .map_err(|e| crate::error::CompileError::new(format!("and: {}", e)))?;
                    store_bool(result_val, result);
                }
            }
            "contains" | "includes" => {
                if let (Some(s), Some(sub)) = (str_arg, second_arg) {
                    let found = builder.build_call(strstr_fn, &[s.into(), sub.into()], "__strstr")
                        .map_err(|e| crate::error::CompileError::new(format!("strstr: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let _null_ptr = ptr_ty.const_null();
                    let cmp = builder.build_is_null(found, "__is_null")
                        .map_err(|e| crate::error::CompileError::new(format!("isnull: {}", e)))?;
                    let result_val = builder.build_not(cmp, "__contains")
                        .map_err(|e| crate::error::CompileError::new(format!("not: {}", e)))?;
                    store_bool(result_val, result);
                }
            }
            "indexOf" => {
                if let (Some(s), Some(sub)) = (str_arg, second_arg) {
                    let found = builder.build_call(strstr_fn, &[s.into(), sub.into()], "__strstr_idx")
                        .map_err(|e| crate::error::CompileError::new(format!("strstr: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let _null_ptr = ptr_ty.const_null();
                    let is_null = builder.build_is_null(found, "__is_null")
                        .map_err(|e| crate::error::CompileError::new(format!("isnull: {}", e)))?;
                    // index = found - s (ptr diff)
                    let s_int = builder.build_ptr_to_int(s, i64_ty, "__s_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let f_int = builder.build_ptr_to_int(found, i64_ty, "__f_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let diff = builder.build_int_sub(f_int, s_int, "__index")
                        .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                    // if is_null, return -1
                    let neg_one = i64_ty.const_int(-1i64 as u64, true);
                    let result_val = builder.build_select(is_null, neg_one, diff, "__index_result")
                        .map_err(|e| crate::error::CompileError::new(format!("select: {}", e)))?
                        .into_int_value();
                    store_int(result_val, result);
                }
            }
            "lastIndexOf" => {
                if let (Some(s), Some(sub)) = (str_arg, second_arg) {
                    let found = builder.build_call(strstr_fn, &[s.into(), sub.into()], "__strstr_last")
                        .map_err(|e| crate::error::CompileError::new(format!("strstr: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let is_null = builder.build_is_null(found, "__is_null")
                        .map_err(|e| crate::error::CompileError::new(format!("isnull: {}", e)))?;
                    let s_int = builder.build_ptr_to_int(s, i64_ty, "__s_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let f_int = builder.build_ptr_to_int(found, i64_ty, "__f_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let diff = builder.build_int_sub(f_int, s_int, "__index")
                        .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                    let neg_one = i64_ty.const_int(-1i64 as u64, true);
                    let result_val = builder.build_select(is_null, neg_one, diff, "__index_result")
                        .map_err(|e| crate::error::CompileError::new(format!("select: {}", e)))?
                        .into_int_value();
                    store_int(result_val, result);
                }
            }
            "search" => {
                if let (Some(s), Some(pattern)) = (str_arg, second_arg) {
                    let found = builder.build_call(strstr_fn, &[s.into(), pattern.into()], "__strstr_search")
                        .map_err(|e| crate::error::CompileError::new(format!("strstr: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let is_null = builder.build_is_null(found, "__is_null")
                        .map_err(|e| crate::error::CompileError::new(format!("isnull: {}", e)))?;
                    let s_int = builder.build_ptr_to_int(s, i64_ty, "__s_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let f_int = builder.build_ptr_to_int(found, i64_ty, "__f_int")
                        .map_err(|e| crate::error::CompileError::new(format!("ptoint: {}", e)))?;
                    let diff = builder.build_int_sub(f_int, s_int, "__index")
                        .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                    let neg_one = i64_ty.const_int(-1i64 as u64, true);
                    let result_val = builder.build_select(is_null, neg_one, diff, "__search_result")
                        .map_err(|e| crate::error::CompileError::new(format!("select: {}", e)))?
                        .into_int_value();
                    store_int(result_val, result);
                }
            }
            "charCodeAt" => {
                if let Some(s) = str_arg {
                    let idx = int_arg(1);
                    let char_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty, s, &[idx], "__char_at_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let ch = builder.build_load(i8_ty, char_ptr, "__char_val")
                        .map_err(|e| crate::error::CompileError::new(format!("load: {}", e)))?
                        .into_int_value();
                    let ch_ext = builder.build_int_cast(ch, i64_ty, "__char_ext")
                        .map_err(|e| crate::error::CompileError::new(format!("zext: {}", e)))?;
                    store_int(ch_ext, result);
                }
            }
            "charAt" => {
                if let Some(s) = str_arg {
                    let idx = int_arg(1);
                    let char_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty, s, &[idx], "__char_at_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let ch = builder.build_load(i8_ty, char_ptr, "__char_val")
                        .map_err(|e| crate::error::CompileError::new(format!("load: {}", e)))?
                        .into_int_value();
                    // Build a 2-byte buffer: char + null
                    let buf = builder.build_alloca(i8_ty.array_type(2), "__char_buf").expect("char buf");
                    let zero = i32_ty.const_zero();
                    let buf_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty.array_type(2), buf, &[zero, zero], "__char_buf_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    builder.build_store(buf_ptr, ch)
                        .map_err(|e| crate::error::CompileError::new(format!("store char: {}", e)))?;
                    let one = i32_ty.const_int(1, false);
                    let null_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty.array_type(2), buf, &[zero, one], "__null_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    builder.build_store(null_ptr, i8_ty.const_zero())
                        .map_err(|e| crate::error::CompileError::new(format!("store null: {}", e)))?;
                    let fmt_s = builder.build_global_string_ptr("%s\n", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__char_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "toUpper" => {
                if let Some(s) = str_arg {
                    let _slen = builder.build_call(strlen_fn, &[s.into()], "__slen")
                        .map_err(|e| crate::error::CompileError::new(format!("strlen: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();
                    let (_, buf_ptr) = make_buf();
                    // Loop: for each char, toupper, store to buffer
                    let _ = builder.build_call(printf_fn, &[builder.build_global_string_ptr("[string] toUpper: char-by-char in C\n", "__str_toupper_stub").expect("stub").as_pointer_value().into()], "__stub")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    // Simplified: just copy the string using snprintf
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "toLower" => {
                if let Some(s) = str_arg {
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let (_, buf_ptr) = make_buf();
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "trim" | "trimStart" | "trimEnd" => {
                if let Some(s) = str_arg {
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let (_, buf_ptr) = make_buf();
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "concat" => {
                if let (Some(s), Some(other)) = (str_arg, second_arg) {
                    let fmt_ss = builder.build_global_string_ptr("%s%s", "__str_fmt_ss").expect("fmt");
                    let (_, buf_ptr) = make_buf();
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_ss.as_pointer_value().into(), s.into(), other.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_ss.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "slice" | "substring" => {
                if let Some(s) = str_arg {
                    let start = int_arg(1);
                    let end = int_arg(2);
                    let fmt_sp = builder.build_global_string_ptr("%.*s", "__str_fmt_sp").expect("fmt");
                    let (_, buf_ptr) = make_buf();
                    let len = builder.build_int_sub(end, start, "__slice_len")
                        .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                    let start_ptr = unsafe {
                        builder.build_in_bounds_gep(i8_ty, s, &[start], "__slice_start")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_sp.as_pointer_value().into(), len.into(), start_ptr.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_sp.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "replace" => {
                if let (Some(s), Some(search_str)) = (str_arg, second_arg) {
                    let _replacement = args.get(2).and_then(|a| self.mir_value_as_cstr_ptr(a, builder));
                    let found = builder.build_call(strstr_fn, &[s.into(), search_str.into()], "__strstr_replace")
                        .map_err(|e| crate::error::CompileError::new(format!("strstr: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();
                    let _is_null = builder.build_is_null(found, "__is_null")
                        .map_err(|e| crate::error::CompileError::new(format!("isnull: {}", e)))?;

                    let _null_ptr = ptr_ty.const_null();
                    let (_, buf_ptr) = make_buf();

                    // if not found, just copy original; else build before+replacement+after
                    // In C backend we only do the simple case (no match = original)
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "padStart" | "padEnd" => {
                if let Some(s) = str_arg {
                    let _target_len = int_arg(1);
                    let _pad_str = args.get(2).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)).unwrap_or(s);
                    let (_, buf_ptr) = make_buf();
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "repeat" => {
                if let Some(s) = str_arg {
                    let _count = int_arg(1);
                    let (_, buf_ptr) = make_buf();
                    let fmt_s = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_s.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), buf_ptr.into()], "__str_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "split" => {
                if let (Some(s), Some(_sep)) = (str_arg, second_arg) {
                    // Simple: just print the original string
                    let (_, buf_ptr) = make_buf();
                    let fmt_s = builder.build_global_string_ptr("[split] %s\n", "__str_split_fmt").expect("fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_s.as_pointer_value().into(), s.into()], "__str_split_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    let fmt_plain = builder.build_global_string_ptr("%s", "__str_fmt_s").expect("fmt");
                    let _ = builder.build_call(snprintf_fn, &[buf_ptr.into(), i64_ty.const_int(4096, false).into(), fmt_plain.as_pointer_value().into(), s.into()], "__snprintf")
                        .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    store_str(buf_ptr, result);
                }
            }
            "match" => {
                if let Some(_s) = str_arg {
                    let fmt_stub = builder.build_global_string_ptr("[string] match requires JS runtime (regex)\n", "__str_match_stub").expect("stub");
                    let _ = builder.build_call(printf_fn, &[fmt_stub.as_pointer_value().into()], "__stub")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                }
            }
            "uuid" => {
                let popen_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                let popen_fn = self.module.add_function("popen", popen_ty, None);
                let fgets_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
                let fgets_fn = self.module.add_function("fgets", fgets_ty, None);
                let pclose_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                let pclose_fn = self.module.add_function("pclose", pclose_ty, None);

                let cmd_uuidgen = builder.build_global_string_ptr("uuidgen", "__uuid_cmd").expect("cmd");
                let mode_r = builder.build_global_string_ptr("r", "__uuid_mode").expect("mode");
                let fp = builder.build_call(popen_fn, &[cmd_uuidgen.as_pointer_value().into(), mode_r.as_pointer_value().into()], "__uuid_fp")
                    .map_err(|e| crate::error::CompileError::new(format!("popen: {}", e)))?
                    .as_any_value_enum()
                    .into_pointer_value();

                let arr_ty = i8_ty.array_type(64);
                let buf = builder.build_alloca(arr_ty, "__uuid_buf").expect("buf");
                let buf_ptr = unsafe {
                    builder.build_in_bounds_gep(arr_ty, buf, &[zero_i32, zero_i32], "__uuid_buf_ptr")
                }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                let _ = builder.build_call(fgets_fn, &[buf_ptr.into(), i32_ty.const_int(64, false).into(), fp.into()], "__uuid_fgets")
                    .map_err(|e| crate::error::CompileError::new(format!("fgets: {}", e)))?;
                let _ = builder.build_call(pclose_fn, &[fp.into()], "__uuid_pclose")
                    .map_err(|e| crate::error::CompileError::new(format!("pclose: {}", e)))?;

                let fmt_uuid = builder.build_global_string_ptr("uuid: %s\n", "__uuid_print_fmt").expect("fmt");
                let _ = builder.build_call(printf_fn, &[fmt_uuid.as_pointer_value().into(), buf_ptr.into()], "__uuid_print")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                store_str(buf_ptr, result);
            }
            // Crypto: build shell command at runtime via snprintf, then popen
            "sha256" | "md5" | "base64Encode" | "base64Decode" | "hexEncode" | "hexDecode" | "hmac" => {
                if let Some(s) = str_arg {
                    let popen_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    let popen_fn = self.module.add_function("popen", popen_ty, None);
                    let fgets_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
                    let fgets_fn = self.module.add_function("fgets", fgets_ty, None);
                    let pclose_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
                    let pclose_fn = self.module.add_function("pclose", pclose_ty, None);

                    // snprintf(cmd_buf, 4096, cmd_fmt, s [, key])
                    let snprintf_ty = i32_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), ptr_ty.into()], true);
                    let snprintf_fn = self.module.add_function("snprintf", snprintf_ty, None);

                    let cmd_fmt_str = match method {
                        "sha256" => "echo -n '%s' | openssl dgst -sha256 2>/dev/null | awk '{print $2}'",
                        "md5" => "echo -n '%s' | openssl dgst -md5 2>/dev/null | awk '{print $2}'",
                        "base64Encode" => "echo -n '%s' | openssl base64 -A 2>/dev/null",
                        "base64Decode" => "echo -n '%s' | openssl base64 -d -A 2>/dev/null",
                        "hexEncode" => "echo -n '%s' | xxd -p 2>/dev/null | tr -d '\\n'",
                        "hexDecode" => "echo -n '%s' | xxd -r -p 2>/dev/null | tr -d '\\n'",
                        "hmac" => "echo -n '%s' | openssl dgst -sha256 -hmac '%s' 2>/dev/null | awk '{print $2}'",
                        _ => unreachable!(),
                    };

                    let cmd_arr_ty = i8_ty.array_type(4096);
                    let cmd_buf = builder.build_alloca(cmd_arr_ty, &format!("__crypto_{}_cmd", method)).expect("cmd buf");
                    let cmd_buf_ptr = unsafe {
                        builder.build_in_bounds_gep(cmd_arr_ty, cmd_buf, &[zero_i32, zero_i32], &format!("__crypto_{}_cmd_ptr", method))
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let fmt_cmd = builder.build_global_string_ptr(cmd_fmt_str, &format!("__crypto_{}_fmt", method)).expect("fmt");
                    let cmd_size = i64_ty.const_int(4096, false);

                    // Build snprintf args: buf, size, fmt, s [, key]
                    if method == "hmac" {
                        let key = match args.get(1).and_then(|a| self.mir_value_as_cstr_ptr(a, builder)) {
                            Some(k) => k,
                            None => builder.build_global_string_ptr("", "__crypto_hmac_key_empty").expect("key").as_pointer_value(),
                        };
                        let _ = builder.build_call(snprintf_fn, &[cmd_buf_ptr.into(), cmd_size.into(), fmt_cmd.as_pointer_value().into(), s.into(), key.into()], "__crypto_hmac_snprintf")
                            .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    } else {
                        let _ = builder.build_call(snprintf_fn, &[cmd_buf_ptr.into(), cmd_size.into(), fmt_cmd.as_pointer_value().into(), s.into()], &format!("__crypto_{}_snprintf", method))
                            .map_err(|e| crate::error::CompileError::new(format!("snprintf: {}", e)))?;
                    }

                    // popen(cmd_buf, "r")
                    let mode_r = builder.build_global_string_ptr("r", &format!("__crypto_{}_mode", method)).expect("mode");
                    let fp = builder.build_call(popen_fn, &[cmd_buf_ptr.into(), mode_r.as_pointer_value().into()], &format!("__crypto_{}_fp", method))
                        .map_err(|e| crate::error::CompileError::new(format!("popen: {}", e)))?
                        .as_any_value_enum()
                        .into_pointer_value();

                    // fgets(buf, 8192, fp)
                    let out_arr_ty = i8_ty.array_type(8192);
                    let out_buf = builder.build_alloca(out_arr_ty, &format!("__crypto_{}_out", method)).expect("out buf");
                    let out_buf_ptr = unsafe {
                        builder.build_in_bounds_gep(out_arr_ty, out_buf, &[zero_i32, zero_i32], &format!("__crypto_{}_out_ptr", method))
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let _ = builder.build_call(fgets_fn, &[out_buf_ptr.into(), i32_ty.const_int(8192, false).into(), fp.into()], &format!("__crypto_{}_fgets", method))
                        .map_err(|e| crate::error::CompileError::new(format!("fgets: {}", e)))?;
                    let _ = builder.build_call(pclose_fn, &[fp.into()], &format!("__crypto_{}_pclose", method))
                        .map_err(|e| crate::error::CompileError::new(format!("pclose: {}", e)))?;

                    // printf("%s\n", result)
                    let fmt_print = builder.build_global_string_ptr("%s\n", &format!("__crypto_{}_out_fmt", method)).expect("out fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_print.as_pointer_value().into(), out_buf_ptr.into()], &format!("__crypto_{}_print", method))
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                    store_str(out_buf_ptr, result);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Emit a regex call using POSIX regex (regcomp/regexec).
    fn emit_regex_call(
        &self,
        result: &Option<String>,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let zero_i32 = i32_ty.const_zero();
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);

        // Helpers: get pattern and string from args
        let pattern = args.first().and_then(|a| match a {
            MirValue::StringLit(s) => Some(s.as_str()),
            _ => None,
        });
        let subject = args.get(1).and_then(|a| match a {
            MirValue::StringLit(s) => Some(s.as_str()),
            _ => None,
        });
        let replacement = args.get(2).and_then(|a| match a {
            MirValue::StringLit(s) => Some(s.as_str()),
            _ => None,
        });

        // Store helpers
        let _store_str = |buf_ptr: inkwell::values::PointerValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(ptr_ty, &format!("__regex_result_{}", d)).expect("alloca");
                builder.build_store(alloca, buf_ptr).ok();
            }
        };
        let store_int = |val: inkwell::values::IntValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(i64_ty, &format!("__regex_result_{}", d)).expect("alloca");
                builder.build_store(alloca, val).ok();
            }
        };
        let store_bool = |val: inkwell::values::IntValue<'static>, dest: &Option<String>| {
            if let Some(d) = dest {
                let alloca = builder.build_alloca(self.context.bool_type(), &format!("__regex_result_{}", d)).expect("alloca");
                builder.build_store(alloca, val).ok();
            }
        };

        // Declare POSIX regex functions
        let regcomp_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i32_ty.into()], false);
        let regcomp_fn = self.module.add_function("regcomp", regcomp_ty, None);
        let regexec_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i64_ty.into(), ptr_ty.into(), i32_ty.into()], false);
        let regexec_fn = self.module.add_function("regexec", regexec_ty, None);
        let regfree_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
        let regfree_fn = self.module.add_function("regfree", regfree_ty, None);
        let regerror_ty = i64_ty.fn_type(&[i32_ty.into(), ptr_ty.into(), ptr_ty.into(), i64_ty.into()], false);
        let _regerror_fn = self.module.add_function("regerror", regerror_ty, None);

        let _regex_t_size = i64_ty.const_int(72, false); // approximate sizeof(regex_t), enough for stack storage

        let _ = (pattern, subject, replacement); // suppress unused warnings

        match method {
            "test" => {
                if let (Some(pat), Some(sub)) = (pattern, subject) {
                    // Allocate regex_t on stack (72 bytes)
                    let regex_t_arr = i8_ty.array_type(72);
                    let regex_t_buf = builder.build_alloca(regex_t_arr, "__regex_t").expect("regex_t");
                    let regex_t_ptr = unsafe {
                        builder.build_in_bounds_gep(regex_t_arr, regex_t_buf, &[zero_i32, zero_i32], "__regex_t_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let pat_ptr = builder.build_global_string_ptr(pat, "__regex_pat").expect("pat").as_pointer_value();
                    let cflags = i32_ty.const_int(1, false); // REG_EXTENDED

                    let _ = builder.build_call(regcomp_fn, &[regex_t_ptr.into(), pat_ptr.into(), cflags.into()], "__regcomp")
                        .map_err(|e| crate::error::CompileError::new(format!("regcomp: {}", e)))?;

                    let sub_ptr = builder.build_global_string_ptr(sub, "__regex_sub").expect("sub").as_pointer_value();
                    let no_match = i64_ty.const_zero();
                    let null_ptr = ptr_ty.const_null();
                    let efags = i32_ty.const_zero();

                    let rc = builder.build_call(regexec_fn, &[regex_t_ptr.into(), sub_ptr.into(), no_match.into(), null_ptr.into(), efags.into()], "__regexec_test")
                        .map_err(|e| crate::error::CompileError::new(format!("regexec: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();

                    let _ = builder.build_call(regfree_fn, &[regex_t_ptr.into()], "__regfree")
                        .map_err(|e| crate::error::CompileError::new(format!("regfree: {}", e)))?;

                    let z = i32_ty.const_zero();
                    let matched = builder.build_int_compare(inkwell::IntPredicate::EQ, rc, z, "__regex_matched")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;
                    store_bool(matched, result);
                }
            }
            "search" => {
                if let (Some(pat), Some(sub)) = (pattern, subject) {
                    let regex_t_arr = i8_ty.array_type(72);
                    let regex_t_buf = builder.build_alloca(regex_t_arr, "__regex_t").expect("regex_t");
                    let regex_t_ptr = unsafe {
                        builder.build_in_bounds_gep(regex_t_arr, regex_t_buf, &[zero_i32, zero_i32], "__regex_t_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let pat_ptr = builder.build_global_string_ptr(pat, "__regex_pat").expect("pat").as_pointer_value();
                    let cflags = i32_ty.const_int(1, false); // REG_EXTENDED
                    let _ = builder.build_call(regcomp_fn, &[regex_t_ptr.into(), pat_ptr.into(), cflags.into()], "__regcomp")
                        .map_err(|e| crate::error::CompileError::new(format!("regcomp: {}", e)))?;

                    let sub_ptr = builder.build_global_string_ptr(sub, "__regex_sub").expect("sub").as_pointer_value();

                    // regmatch_t array for 1 match
                    let rm_arr_ty = i64_ty.array_type(2); // two i64s: rm_so, rm_eo
                    let rm_buf = builder.build_alloca(rm_arr_ty, "__rm_buf").expect("rm_buf");
                    let rm_ptr = rm_buf;

                    let efags = i32_ty.const_zero();
                    let nmatch = i64_ty.const_int(1, false);

                    let rc = builder.build_call(regexec_fn, &[regex_t_ptr.into(), sub_ptr.into(), nmatch.into(), rm_ptr.into(), efags.into()], "__regexec_search")
                        .map_err(|e| crate::error::CompileError::new(format!("regexec: {}", e)))?
                        .as_any_value_enum()
                        .into_int_value();

                    let _ = builder.build_call(regfree_fn, &[regex_t_ptr.into()], "__regfree")
                        .map_err(|e| crate::error::CompileError::new(format!("regfree: {}", e)))?;

                    let z = i32_ty.const_zero();
                    let matched = builder.build_int_compare(inkwell::IntPredicate::EQ, rc, z, "__regex_matched")
                        .map_err(|e| crate::error::CompileError::new(format!("icmp: {}", e)))?;

                    // rm_so is first element of rm_buf
                    let rm_so_ptr = unsafe {
                        builder.build_in_bounds_gep(i64_ty, rm_ptr, &[z.into(), z.into()], "__rm_so")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;
                    let rm_so = builder.build_load(i64_ty, rm_so_ptr, "__rm_so_val")
                        .map_err(|e| crate::error::CompileError::new(format!("load: {}", e)))?
                        .into_int_value();
                    let neg_one = i64_ty.const_int(-1i64 as u64, true);
                    let result_val = builder.build_select(matched, rm_so, neg_one, "__regex_search_result")
                        .map_err(|e| crate::error::CompileError::new(format!("select: {}", e)))?
                        .into_int_value();
                    store_int(result_val, result);
                }
            }
            "match" => {
                if let (Some(pat), Some(sub)) = (pattern, subject) {
                    let regex_t_arr = i8_ty.array_type(72);
                    let regex_t_buf = builder.build_alloca(regex_t_arr, "__regex_t").expect("regex_t");
                    let regex_t_ptr = unsafe {
                        builder.build_in_bounds_gep(regex_t_arr, regex_t_buf, &[zero_i32, zero_i32], "__regex_t_ptr")
                    }.map_err(|e| crate::error::CompileError::new(format!("gep: {}", e)))?;

                    let pat_ptr = builder.build_global_string_ptr(pat, "__regex_pat").expect("pat").as_pointer_value();
                    let cflags = i32_ty.const_int(1, false);
                    let _ = builder.build_call(regcomp_fn, &[regex_t_ptr.into(), pat_ptr.into(), cflags.into()], "__regcomp")
                        .map_err(|e| crate::error::CompileError::new(format!("regcomp: {}", e)))?;

                    let sub_ptr = builder.build_global_string_ptr(sub, "__regex_sub").expect("sub").as_pointer_value();
                    let rm_arr_ty = i64_ty.array_type(2);
                    let rm_buf = builder.build_alloca(rm_arr_ty, "__rm_buf").expect("rm_buf");
                    let efags = i32_ty.const_zero();
                    let nmatch = i64_ty.const_int(1, false);

                    let _ = builder.build_call(regexec_fn, &[regex_t_ptr.into(), sub_ptr.into(), nmatch.into(), rm_buf.into(), efags.into()], "__regexec_match")
                        .map_err(|e| crate::error::CompileError::new(format!("regexec: {}", e)))?;
                    let _ = builder.build_call(regfree_fn, &[regex_t_ptr.into()], "__regfree")
                        .map_err(|e| crate::error::CompileError::new(format!("regfree: {}", e)))?;

                    // Print matched substring
                    let fmt_str = builder.build_global_string_ptr("[regex match] %s with pattern %s -> check C output\n", "__regex_match_fmt").expect("fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_str.as_pointer_value().into(), sub_ptr.into(), pat_ptr.into()], "__regex_match_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                }
            }
            "replace" => {
                if let (Some(pat), Some(sub)) = (pattern, subject) {
                    let sub_ptr = builder.build_global_string_ptr(sub, "__regex_sub").expect("sub").as_pointer_value();
                    let pat_ptr = builder.build_global_string_ptr(pat, "__regex_pat").expect("pat").as_pointer_value();
                    let repl = replacement.unwrap_or("");
                    let repl_ptr = builder.build_global_string_ptr(repl, "__regex_repl").expect("repl").as_pointer_value();
                    let fmt_str = builder.build_global_string_ptr("[regex replace] %s pattern=%s replacement=%s\n", "__regex_replace_fmt").expect("fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_str.as_pointer_value().into(), sub_ptr.into(), pat_ptr.into(), repl_ptr.into()], "__regex_replace_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;

                    let fmt_plain = builder.build_global_string_ptr("%s\n", "__regex_plain").expect("fmt");
                    let _ = builder.build_call(printf_fn, &[fmt_plain.as_pointer_value().into(), sub_ptr.into()], "__regex_repl_out")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                }
            }
            "split" => {
                if let (Some(pat), Some(sub)) = (pattern, subject) {
                    let fmt_str = builder.build_global_string_ptr("[regex split] %s pattern=%s\n", "__regex_split_fmt").expect("fmt");
                    let sub_ptr = builder.build_global_string_ptr(sub, "__regex_sub").expect("sub").as_pointer_value();
                    let pat_ptr = builder.build_global_string_ptr(pat, "__regex_pat").expect("pat").as_pointer_value();
                    let _ = builder.build_call(printf_fn, &[fmt_str.as_pointer_value().into(), sub_ptr.into(), pat_ptr.into()], "__regex_split_print")
                        .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Emit a datetime call — emits real C time.h calls on the backend.
    /// Uses unix timestamps (i64) as the universal bridge between C and JS.
    fn emit_datetime_call(
        &self,
        result: &Option<String>,
        method: &str,
        args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let zero_i32 = i32_ty.const_zero();
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        let printf_fn = self.module.add_function("printf", printf_ty, None);

        match method {
            "now" => {
                let time_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
                let time_fn = self.module.add_function("time", time_ty, None);
                let null_ptr = ptr_ty.const_null();
                let ts_val = builder.build_call(time_fn, &[null_ptr.into()], "__dt_now")
                    .map_err(|e| crate::error::CompileError::new(format!("time: {}", e)))?
                    .as_any_value_enum()
                    .into_int_value();
                if let Some(dest) = result {
                    let alloca = builder.build_alloca(i64_ty, &format!("__dt_result_{}", dest))
                        .expect("dt alloca");
                    builder.build_store(alloca, ts_val)
                        .map_err(|e| crate::error::CompileError::new(format!("store dt: {}", e)))?;
                }
            }
            "fromTimestamp" | "format" => {
                let ts_val = args.first().map(|a| {
                    match a {
                        MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                        _ => i64_ty.const_zero(),
                    }
                }).unwrap_or_else(|| i64_ty.const_zero());
                let ts_alloca = builder.build_alloca(i64_ty, "__dt_ts").expect("dt ts alloca");
                builder.build_store(ts_alloca, ts_val)
                    .map_err(|e| crate::error::CompileError::new(format!("store ts: {}", e)))?;

                let ctime_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
                let ctime_fn = self.module.add_function("ctime", ctime_ty, None);
                let str_ptr = builder.build_call(ctime_fn, &[ts_alloca.into()], "__dt_ctime")
                    .map_err(|e| crate::error::CompileError::new(format!("ctime: {}", e)))?
                    .as_any_value_enum()
                    .into_pointer_value();

                // Print the result
                let fmt_str = builder.build_global_string_ptr("%s", "__dt_fmt").expect("fmt");
                let _ = builder.build_call(printf_fn, &[fmt_str.as_pointer_value().into(), str_ptr.into()], "__dt_print")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;

                if let Some(dest) = result {
                    let alloca = builder.build_alloca(ptr_ty, &format!("__dt_result_{}", dest))
                        .expect("dt alloca");
                    builder.build_store(alloca, str_ptr)
                        .map_err(|e| crate::error::CompileError::new(format!("store dt: {}", e)))?;
                }
            }
            // Component extraction: localtime_r then GEP into struct tm fields
            "year" | "month" | "day" | "hour" | "minute" | "second" | "weekday" => {
                let ts_val = match args.first() {
                    Some(MirValue::IntLit(v)) => i64_ty.const_int(*v as u64, false),
                    Some(MirValue::Local(name)) => {
                        let msg = format!("[datetime] component {} from variable not supported in C codegen\n", method);
                        let f = builder.build_global_string_ptr(&msg, "__dt_warn").expect("warn");
                        let _ = builder.build_call(printf_fn, &[f.as_pointer_value().into()], "__dt_warn_call")
                            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
                        i64_ty.const_zero()
                    }
                    _ => i64_ty.const_zero(),
                };
                let ts_alloca = builder.build_alloca(i64_ty, "__dt_ts").expect("dt ts alloca");
                builder.build_store(ts_alloca, ts_val)
                    .map_err(|e| crate::error::CompileError::new(format!("store ts: {}", e)))?;

                let localtime_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
                let localtime_fn = self.module.add_function("localtime", localtime_ty, None);
                let tm_ptr = builder.build_call(localtime_fn, &[ts_alloca.into()], "__dt_localtime")
                    .map_err(|e| crate::error::CompileError::new(format!("localtime: {}", e)))?
                    .as_any_value_enum()
                    .into_pointer_value();

                // struct tm: tm_sec(0), tm_min(1), tm_hour(2), tm_mday(3), tm_mon(4), tm_year(5), tm_wday(6)
                let offset = match method {
                    "second" => 0u32,
                    "minute" => 1,
                    "hour" => 2,
                    "day" => 3,
                    "month" => 4,
                    "year" => 5,
                    "weekday" => 6,
                    _ => 0,
                };
                let tm_array_ty = i32_ty.array_type(9);
                let idx = i32_ty.const_int(offset as u64, false);
                let field_ptr = unsafe {
                    builder.build_in_bounds_gep(tm_array_ty, tm_ptr, &[zero_i32, idx], "__dt_field")
                }.map_err(|e| crate::error::CompileError::new(format!("gep tm: {}", e)))?;

                let field_val = builder.build_load(i32_ty, field_ptr, "__dt_field_val")
                    .map_err(|e| crate::error::CompileError::new(format!("load tm: {}", e)))?
                    .into_int_value();

                // Adjust year (tm_year + 1900) and month (tm_mon + 1)
                let adjusted = if method == "year" {
                    builder.build_int_add(field_val, i32_ty.const_int(1900, false), "__dt_year_adj")
                        .map_err(|e| crate::error::CompileError::new(format!("add: {}", e)))?
                } else if method == "month" {
                    builder.build_int_add(field_val, i32_ty.const_int(1, false), "__dt_mon_adj")
                        .map_err(|e| crate::error::CompileError::new(format!("add: {}", e)))?
                } else {
                    field_val
                };

                if let Some(dest) = result {
                    let alloca = builder.build_alloca(i32_ty, &format!("__dt_result_{}", dest))
                        .expect("dt alloca");
                    builder.build_store(alloca, adjusted)
                        .map_err(|e| crate::error::CompileError::new(format!("store dt: {}", e)))?;
                }
            }
            "addDays" | "addHours" => {
                let multiplier = if method == "addDays" { 86400i64 } else { 3600 };
                let ts = args.get(0).map(|a| match a {
                    MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                    MirValue::Local(_) => i64_ty.const_zero(),
                    _ => i64_ty.const_zero(),
                }).unwrap_or_else(|| i64_ty.const_zero());
                let amount = args.get(1).map(|a| match a {
                    MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                    MirValue::Local(_) => i64_ty.const_zero(),
                    _ => i64_ty.const_zero(),
                }).unwrap_or_else(|| i64_ty.const_zero());
                let secs = i64_ty.const_int(multiplier as u64, false);
                let delta = builder.build_int_mul(amount, secs, "__dt_mul")
                    .map_err(|e| crate::error::CompileError::new(format!("mul: {}", e)))?;
                let result_val = builder.build_int_add(ts, delta, "__dt_add")
                    .map_err(|e| crate::error::CompileError::new(format!("add: {}", e)))?;
                if let Some(dest) = result {
                    let alloca = builder.build_alloca(i64_ty, &format!("__dt_result_{}", dest))
                        .expect("dt alloca");
                    builder.build_store(alloca, result_val)
                        .map_err(|e| crate::error::CompileError::new(format!("store dt: {}", e)))?;
                }
            }
            "diffSeconds" => {
                let ts1 = args.get(0).map(|a| match a {
                    MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                    _ => i64_ty.const_zero(),
                }).unwrap_or_else(|| i64_ty.const_zero());
                let ts2 = args.get(1).map(|a| match a {
                    MirValue::IntLit(v) => i64_ty.const_int(*v as u64, false),
                    _ => i64_ty.const_zero(),
                }).unwrap_or_else(|| i64_ty.const_zero());
                let result_val = builder.build_int_sub(ts1, ts2, "__dt_diff")
                    .map_err(|e| crate::error::CompileError::new(format!("sub: {}", e)))?;
                if let Some(dest) = result {
                    let alloca = builder.build_alloca(i64_ty, &format!("__dt_result_{}", dest))
                        .expect("dt alloca");
                    builder.build_store(alloca, result_val)
                        .map_err(|e| crate::error::CompileError::new(format!("store dt: {}", e)))?;
                }
            }
            "parse" => {
                // strptime is POSIX, not always available — emit stub
                let msg = builder.build_global_string_ptr(
                    "[datetime] parse: use JS runtime\n", "__dt_parse_stub"
                ).expect("stub");
                let _ = builder.build_call(printf_fn, &[msg.as_pointer_value().into()], "__dt_stub")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            _ => {
                let msg = format!("[datetime] {}: use JS runtime\n", method);
                let f = builder.build_global_string_ptr(&msg, "__dt_stub").expect("stub");
                let _ = builder.build_call(printf_fn, &[f.as_pointer_value().into()], "__dt_stub_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
        }
        Ok(())
    }
}
