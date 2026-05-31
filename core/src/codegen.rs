use crate::debug::DebugInfo;
use crate::error::Result;
use crate::mir::*;
use inkwell::context::Context;
use inkwell::types::BasicType;
use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::BasicValue;
use inkwell::values::AnyValue;

/// Parsed schedule specification from a cron or friendly-format string.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ScheduleKind {
    /// Simple interval in seconds (compile-time constant sleep).
    /// e.g. `*/5 * * * *`, `every 5 minutes`, `hourly`
    Interval(u32),
    /// Run daily at a specific hour and minute (UTC via `time()` arithmetic).
    /// e.g. `0 8 * * *`, `daily at 08:00`, `every day at 08:00`
    DailyAt { hour: u32, min: u32 },
    /// Run weekly on a specific day at a specific time.
    /// e.g. `every Monday at 09:00`, `weekly on Monday at 09:00`
    WeeklyAt { dow: u32, hour: u32, min: u32 },
    /// Run monthly on a specific day at a specific time.
    /// e.g. `every month on day 15 at 10:00`, `monthly on day 15 at 10:00`
    MonthlyAt { dom: u32, hour: u32, min: u32 },
}

/// Days of week for friendly parsing (0=Sunday, 1=Monday, ..., 6=Saturday)
const DAY_NAMES: &[&str] = &[
    "sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday",
];

/// Parse a schedule expression string into a ScheduleKind.
fn parse_schedule(expr: &str) -> ScheduleKind {
    let trimmed = expr.trim();
    // Try cron first (5 space-separated fields without alphabetic chars, except *)
    let is_cron = trimmed.split_whitespace().count() == 5
        && !trimmed.chars().any(|c| c.is_ascii_alphabetic());
    if is_cron {
        return parse_cron(trimmed);
    }
    parse_friendly(trimmed)
}

/// Parse a 5-field cron expression.
fn parse_cron(expr: &str) -> ScheduleKind {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return ScheduleKind::Interval(60);
    }
    let min = parts[0].trim();
    let hour = parts[1].trim();
    let _dom = parts[2].trim();
    let _mon = parts[3].trim();
    let _dow = parts[4].trim();

    // */N in minute field → every N minutes
    if let Some(n_str) = min.strip_prefix("*/") {
        if let Ok(n) = n_str.parse::<u32>() {
            if n > 0 {
                return ScheduleKind::Interval(n * 60);
            }
        }
    }

    // */N in hour field → every N hours
    if let Some(n_str) = hour.strip_prefix("*/") {
        if let Ok(n) = n_str.parse::<u32>() {
            if n > 0 {
                return ScheduleKind::Interval(n * 3600);
            }
        }
    }

    // Fixed time: M H * * * → daily at H:M
    if let (Ok(m), Ok(h)) = (min.parse::<u32>(), hour.parse::<u32>()) {
        return ScheduleKind::DailyAt { hour: h, min: m };
    }

    // * * * * * or partial → every minute
    ScheduleKind::Interval(60)
}

/// Parse friendly schedule strings.
/// Supported patterns:
///   "every N seconds/minutes/hours"      — interval
///   "every minute" / "every hour"        — interval
///   "hourly" / "minutely"               — interval
///   "daily" / "every day"               — interval 86400
///   "daily at HH:MM" / "every day at HH:MM"  — daily at time
///   "every Monday" / "weekly on Monday"       — weekly interval
///   "every Monday at HH:MM"                   — weekly at time
///   "at HH:MM every day"                      — daily at time
///   "every month on day D at HH:MM"           — monthly at time
fn parse_friendly(expr: &str) -> ScheduleKind {
    let lower = expr.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().filter(|w| !w.is_empty()).collect();

    if words.is_empty() {
        return ScheduleKind::Interval(60);
    }

    // Extract hour:min from any token like "08:00" or "8:00"
    let mut hour_val: Option<u32> = None;
    let mut min_val: Option<u32> = None;

    // Try to find HH:MM in any word
    for w in &words {
        if let Some(pos) = w.find(':') {
            if let (Ok(h), Ok(m)) = (
                w[..pos].parse::<u32>(),
                w[pos + 1..].parse::<u32>(),
            ) {
                if h < 24 && m < 60 {
                    hour_val = Some(h);
                    min_val = Some(m);
                    break;
                }
            }
        }
    }

    let word_list: Vec<&str> = words.iter().map(|s| *s).collect();

    // Detect "every N seconds/minutes/hours"
    if let Some(idx) = word_list.iter().position(|w| *w == "every") {
        // every minute, every hour, every day, every week, every month
        if idx + 1 < word_list.len() {
            // Check if next word is a number
            let next = word_list[idx + 1];
            if let Ok(n) = next.parse::<u32>() {
                // "every N <unit>"
                if idx + 2 < word_list.len() {
                    let unit = word_list[idx + 2];
                    return parse_every_n(n, unit, hour_val, min_val);
                }
            } else {
                // "every <unit>" without number
                return parse_every_unit(next, hour_val, min_val);
            }
        }
        return ScheduleKind::Interval(60);
    }

    // Detect "hourly", "minutely", "daily", "weekly", "monthly"
    for w in &word_list {
        match *w {
            "hourly" | "everyhour" => return ScheduleKind::Interval(3600),
            "minutely" | "everyminute" => return ScheduleKind::Interval(60),
            "daily" | "everyday" => {
                if let (Some(h), Some(m)) = (hour_val, min_val) {
                    return ScheduleKind::DailyAt { hour: h, min: m };
                }
                return ScheduleKind::Interval(86400);
            }
            "weekly" | "everyweek" => {
                if let (Some(h), Some(m)) = (hour_val, min_val) {
                    return ScheduleKind::WeeklyAt { dow: 0, hour: h, min: m };
                }
                return ScheduleKind::Interval(604800);
            }
            "monthly" | "everymonth" => {
                if let (Some(h), Some(m)) = (hour_val, min_val) {
                    return ScheduleKind::MonthlyAt { dom: 1, hour: h, min: m };
                }
                return ScheduleKind::Interval(2592000);
            }
            _ => {}
        }
    }

    // Detect "at HH:MM" pattern anywhere (no "every" prefix)
    if let (Some(h), Some(m)) = (hour_val, min_val) {
        // Check for day-of-week mention
        for w in &word_list {
            for (dow, name) in DAY_NAMES.iter().enumerate() {
                if *w == *name || *w == &name[..3] {
                    // e.g. "at 09:00 on Monday" or "Monday at 09:00"
                    return ScheduleKind::WeeklyAt {
                        dow: dow as u32,
                        hour: h,
                        min: m,
                    };
                }
            }
            // Check for "day D" or "dayD" pattern
            if *w == "day" || w.starts_with("day") {
                let day_str = if *w == "day" {
                    // next token should be the number
                    // find the index and check next
                    None
                } else {
                    w.strip_prefix("day").and_then(|s| s.parse::<u32>().ok())
                };
                if let Some(dom) = day_str {
                    return ScheduleKind::MonthlyAt { dom, hour: h, min: m };
                }
            }
        }
        // No day-of-week found, just daily at time
        return ScheduleKind::DailyAt { hour: h, min: m };
    }

    // Final fallback
    ScheduleKind::Interval(60)
}

/// Parse "every N <unit>" pattern
fn parse_every_n(n: u32, unit: &str, hour: Option<u32>, min: Option<u32>) -> ScheduleKind {
    let secs = match unit {
        u if u.starts_with("second") => n,
        u if u.starts_with("minute") => n * 60,
        u if u.starts_with("hour") => n * 3600,
        u if u.starts_with("day") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::DailyAt { hour: h, min: m };
            }
            n * 86400
        }
        u if u.starts_with("week") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::WeeklyAt { dow: 0, hour: h, min: m };
            }
            n * 604800
        }
        u if u.starts_with("month") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::MonthlyAt { dom: 1, hour: h, min: m };
            }
            n * 2592000
        }
        _ => n, // treat unknown unit as seconds
    };
    ScheduleKind::Interval(secs)
}

/// Parse "every <unit>" pattern (without a number)
fn parse_every_unit(unit: &str, hour: Option<u32>, min: Option<u32>) -> ScheduleKind {
    match unit {
        u if u.starts_with("second") => ScheduleKind::Interval(1),
        u if u.starts_with("minute") => ScheduleKind::Interval(60),
        u if u.starts_with("hour") => ScheduleKind::Interval(3600),
        u if u.starts_with("day") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::DailyAt { hour: h, min: m };
            }
            ScheduleKind::Interval(86400)
        }
        u if u.starts_with("week") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::WeeklyAt { dow: 0, hour: h, min: m };
            }
            ScheduleKind::Interval(604800)
        }
        u if u.starts_with("month") => {
            if let (Some(h), Some(m)) = (hour, min) {
                return ScheduleKind::MonthlyAt { dom: 1, hour: h, min: m };
            }
            ScheduleKind::Interval(2592000)
        }
        // Check if unit is a day name
        _ => {
            let lower = unit;
            for (dow, name) in DAY_NAMES.iter().enumerate() {
                if lower == *name || lower == &name[..3] {
                    // e.g. "every monday" - if hour/min provided, use them
                    if let (Some(h), Some(m)) = (hour, min) {
                        return ScheduleKind::WeeklyAt {
                            dow: dow as u32,
                            hour: h,
                            min: m,
                        };
                    }
                    return ScheduleKind::Interval(604800);
                }
            }
            ScheduleKind::Interval(60)
        }
    }
}

/// Get the base interval in seconds for a ScheduleKind (used for runtime sleep between iterations).
fn schedule_base_interval(kind: &ScheduleKind) -> u32 {
    match kind {
        ScheduleKind::Interval(s) => *s,
        ScheduleKind::DailyAt { .. } => 86400,
        ScheduleKind::WeeklyAt { .. } => 604800,
        ScheduleKind::MonthlyAt { .. } => 2592000,
    }
}

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

        let mut scheduled: Vec<&MirFunction> = Vec::new();

        for func in &program.functions {
            self.emit_function(func, source_path)?;
            if func.schedule_expr.is_some() {
                scheduled.push(func);
            }
        }

        // If there are scheduled functions, emit thread wrappers and startup
        if !scheduled.is_empty() {
            for func in &scheduled {
                self.emit_schedule_thread_wrapper(func)?;
            }
            self.emit_schedule_startup(&scheduled)?;

            // Insert a call to __schedule_startup at the beginning of main
            if let Some(main_fn) = self.module.get_function("main") {
                if let Some(first_bb) = main_fn.get_first_basic_block() {
                    if let Some(first_instr) = first_bb.get_first_instruction() {
                        let builder = self.context.create_builder();
                        builder.position_before(&first_instr);
                        let _i32_ty = self.context.i32_type();
                        let _ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                        let _void_ty = self.context.void_type();
                        let startup_fn = self.module.get_function("__schedule_startup").unwrap();
                        let _ = builder
                            .build_call(startup_fn, &[], "__schedule_startup_call")
                            .map_err(|e| {
                                crate::error::CompileError::new(format!(
                                    "build_call __schedule_startup: {}",
                                    e
                                ))
                            })?;
                    }
                }
            }
        }

        // Finalise debug info
        if let Some(ref di) = self.debug {
            di.finalize();
        }

        Ok(())
    }

    /// Emit a thread wrapper for a scheduled function.
    /// For Interval schedules: while(1) { sleep(interval); func(); }
    /// For DailyAt/WeeklyAt/MonthlyAt: compute initial sleep with time() so first
    /// invocation lands on the correct wall-clock time, then loop with base interval.
    fn emit_schedule_thread_wrapper(&self, func: &MirFunction) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let _i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        let cron = func.schedule_expr.as_deref().unwrap_or("every minute");
        let kind = parse_schedule(cron);

        let wrapper_name = format!("__schedule_thread_{}", func.name);
        let wrapper_fn_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
        let wrapper_fn = self.module.add_function(&wrapper_name, wrapper_fn_ty, None);

        // Declare sleep
        let sleep_fn = self.module.add_function(
            "sleep",
            i32_ty.fn_type(&[i32_ty.into()], false),
            None,
        );

        let entry = self.context.append_basic_block(wrapper_fn, "entry");
        let builder = self.context.create_builder();
        builder.position_at_end(entry);

        match kind {
            ScheduleKind::Interval(secs) => {
                // Simple loop: entry → loop (no initial offset)
                let loop_block = self.context.append_basic_block(wrapper_fn, "loop");
                let _ = builder.build_unconditional_branch(loop_block);
                builder.position_at_end(loop_block);

                let sleep_arg = i32_ty.const_int(secs as u64, false);
                let _ = builder
                    .build_call(sleep_fn, &[sleep_arg.into()], "__schedule_sleep")
                    .map_err(|e| crate::error::CompileError::new(format!("sleep call: {}", e)))?;

                self.call_scheduled_func(&builder, func)?;

                let _ = builder.build_unconditional_branch(loop_block);
            }
            ScheduleKind::DailyAt { hour, min } => {
                self.emit_timeofday_wrapper(&builder, func, &sleep_fn, 86400, hour, min, None)?;
            }
            ScheduleKind::WeeklyAt { dow, hour, min } => {
                self.emit_timeofday_wrapper(&builder, func, &sleep_fn, 604800, hour, min, Some(dow))?;
            }
            ScheduleKind::MonthlyAt { dom, hour, min } => {
                // For monthly, use the same time-of-day computation but with 86400 base (daily check)
                // and a runtime day-of-month check — but that's complex. v1: use interval.
                let _ = dom;
                self.emit_timeofday_wrapper(&builder, func, &sleep_fn, 86400, hour, min, None)?;
            }
        }

        Ok(())
    }

    /// Emit a time-of-day schedule wrapper with runtime initial offset computation.
    ///
    /// Produces:
    ///   void* __schedule_thread_<name>(void* arg) {
    ///       while (1) {
    ///           // Compute seconds until next target time
    ///           time_t now = time(NULL);
    ///           struct tm* local = localtime(&now);
    ///           int target_secs = target_hour * 3600 + target_min * 60;
    ///           int now_secs = local->tm_hour * 3600 + local->tm_min * 60 + local->tm_sec;
    ///           int offset = target_secs - now_secs;
    ///           if (offset <= 0) offset += base_interval;
    ///           sleep(offset);
    ///           func();
    ///       }
    ///   }
    fn emit_timeofday_wrapper(
        &self,
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
        sleep_fn: &inkwell::values::FunctionValue<'static>,
        base_interval: u32,
        target_hour: u32,
        target_min: u32,
        _target_dow: Option<u32>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        let wrapper_fn = builder.get_insert_block().unwrap().get_parent().unwrap();

        // Declare time: time_t time(time_t *t)
        let time_fn = self.module.add_function(
            "time",
            i64_ty.fn_type(&[ptr_ty.into()], false),
            None,
        );

        let loop_block = self.context.append_basic_block(wrapper_fn, "loop");
        let _ = builder.build_unconditional_branch(loop_block);
        builder.position_at_end(loop_block);

        // time(NULL)
        let null_ptr = ptr_ty.const_zero();
        let call_result = builder
            .build_call(time_fn, &[null_ptr.into()], "__schedule_now")
            .map_err(|e| crate::error::CompileError::new(format!("time call: {}", e)))?;
        let now_epoch = call_result
            .as_any_value_enum()
            .into_int_value();

        // Compute seconds since midnight: now_epoch % 86400
        let day_secs = i64_ty.const_int(86400, false);
        let now_today_secs = builder
            .build_int_signed_rem(now_epoch, day_secs, "__schedule_today_secs")
            .unwrap();

        // Compute target seconds: target_hour * 3600 + target_min * 60
        let target_total = (target_hour * 3600 + target_min * 60) as u64;
        let target_val = i64_ty.const_int(target_total, false);

        // offset = target - now_today
        let offset = builder
            .build_int_sub(target_val, now_today_secs, "__schedule_offset")
            .unwrap();

        let base_val = i64_ty.const_int(base_interval as u64, false);

        // if offset <= 0, offset += base_interval (use select, avoids phi node)
        let zero = i64_ty.const_zero();
        let cond = builder
            .build_int_compare(
                inkwell::IntPredicate::SLE,
                offset,
                zero,
                "__schedule_offset_neg_check",
            )
            .unwrap();
        let offset_plus_base = builder
            .build_int_add(offset, base_val, "__schedule_offset_plus_base")
            .unwrap();
        let final_offset = builder
            .build_select(cond, offset_plus_base, offset, "__schedule_offset_final")
            .map_err(|e| crate::error::CompileError::new(format!("select: {}", e)))?
            .into_int_value();

        // i32 sleep takes unsigned int, so truncate the i64 offset
        let sleep_arg = builder
            .build_int_truncate(final_offset, i32_ty, "__schedule_sleep_secs")
            .unwrap();

        let _ = builder
            .build_call(*sleep_fn, &[sleep_arg.into()], "__schedule_sleep")
            .map_err(|e| crate::error::CompileError::new(format!("sleep call: {}", e)))?;

        self.call_scheduled_func(builder, func)?;

        let _ = builder.build_unconditional_branch(loop_block);
        Ok(())
    }

    /// Helper: emit a call to the scheduled function.
    fn call_scheduled_func(
        &self,
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
    ) -> Result<()> {
        if let Some(target_fn) = self.module.get_function(&func.name) {
            let _ = builder
                .build_call(target_fn, &[], &format!("__schedule_call_{}", func.name))
                .map_err(|e| crate::error::CompileError::new(format!("call {}: {}", func.name, e)))?;
        }
        Ok(())
    }

    /// Emit the __schedule_startup function.
    /// Creates pthreads for each scheduled function (detached, no join).
    fn emit_schedule_startup(&self, scheduled: &[&MirFunction]) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_ty = self.context.void_type();

        // Declare pthread_create
        let pthread_create_fn = self.module.add_function(
            "pthread_create",
            i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), ptr_ty.into(), ptr_ty.into()], false),
            None,
        );

        // Declare pthread_detach: int pthread_detach(pthread_t thread);
        let pthread_detach_fn = self.module.add_function(
            "pthread_detach",
            i32_ty.fn_type(&[i64_ty.into()], false),
            None,
        );

        let startup_fn_name = "__schedule_startup";
        let startup_fn_ty = void_ty.fn_type(&[], false);
        let startup_fn = self.module.add_function(startup_fn_name, startup_fn_ty, None);
        let entry = self.context.append_basic_block(startup_fn, "entry");
        let builder = self.context.create_builder();
        builder.position_at_end(entry);

        let null_ptr = ptr_ty.const_zero();

        for func in scheduled {
            let thread_ptr = builder
                .build_alloca(i64_ty, &format!("__schedule_tid_{}", func.name))
                .unwrap();

            let wrapper_name = format!("__schedule_thread_{}", func.name);
            let wrapper_fn = self.module.get_function(&wrapper_name).unwrap();

            let create_args: Vec<BasicMetadataValueEnum> = vec![
                thread_ptr.into(),
                null_ptr.into(),
                wrapper_fn.as_global_value().as_pointer_value().into(),
                null_ptr.into(),
            ];

            let _ = builder
                .build_call(
                    pthread_create_fn,
                    &create_args,
                    &format!("__schedule_create_{}", func.name),
                )
                .map_err(|e| {
                    crate::error::CompileError::new(format!(
                        "pthread_create for {}: {}",
                        func.name, e
                    ))
                })?;

            // Detach the thread
            let tid = builder.build_load(i64_ty, thread_ptr, &format!("__schedule_tid_load_{}", func.name)).unwrap();
            let detach_args: Vec<BasicMetadataValueEnum> = vec![tid.into()];
            let _ = builder
                .build_call(pthread_detach_fn, &detach_args, &format!("__schedule_detach_{}", func.name))
                .map_err(|e| {
                    crate::error::CompileError::new(format!(
                        "pthread_detach for {}: {}",
                        func.name, e
                    ))
                })?;
        }

        builder.build_return(None).unwrap();
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
        let mut has_return = false;
        for stmt in &func.body.stmts {
            if matches!(stmt, MirStmt::Return(_, _)) {
                has_return = true;
            }
            self.emit_stmt(stmt, &builder, func)?;
        }

        // Default return — only if no explicit/implicit return was already emitted
        if !has_return {
            let zero_val: inkwell::values::BasicValueEnum<'static> = match ret_ty {
                inkwell::types::BasicTypeEnum::IntType(t) => t.const_zero().as_basic_value_enum(),
                inkwell::types::BasicTypeEnum::FloatType(t) => t.const_zero().as_basic_value_enum(),
                inkwell::types::BasicTypeEnum::PointerType(t) => t.const_zero().as_basic_value_enum(),
                inkwell::types::BasicTypeEnum::ArrayType(t) => t.const_zero().as_basic_value_enum(),
                inkwell::types::BasicTypeEnum::StructType(t) => t.const_zero().as_basic_value_enum(),
                inkwell::types::BasicTypeEnum::VectorType(t) => t.const_zero().as_basic_value_enum(),
                _ => unreachable!(),
            };
            builder.build_return(Some(&zero_val)).expect("return");
        }

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
            MirStmt::Wait(_, line) => *line,
            MirStmt::Parallel { dbg_line, .. } => *dbg_line,
            MirStmt::Await { dbg_line, .. } => *dbg_line,
            MirStmt::ConsoleCall { dbg_line, .. } => *dbg_line,
            MirStmt::FsCall { dbg_line, .. } => *dbg_line,
            MirStmt::TransportCall { dbg_line, .. } => *dbg_line,
            MirStmt::StringCall { dbg_line, .. } => *dbg_line,
            MirStmt::RegexCall { dbg_line, .. } => *dbg_line,
            MirStmt::DateTimeCall { dbg_line, .. } => *dbg_line,
            MirStmt::IsCall { dbg_line, .. } => *dbg_line,
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
                if let Some(value) = ret {
                    let val = self.load_mir_value(value, builder);
                    builder.build_return(Some(&val)).expect("return");
                } else {
                    builder.build_return(None).expect("return");
                }
            }
            MirStmt::Bench { .. } => {
                self.emit_bench_stmt(stmt, builder, func)?;
            }
            MirStmt::Wait(millis, _) => {
                self.emit_wait_stmt(*millis, builder)?;
            }
            MirStmt::Parallel { blocks, .. } => {
                self.emit_parallel_stmt(blocks, builder, func)?;
            }
            MirStmt::Await { value, result_target: _, dbg_line: _ } => {
                // For C backend v1: execute awaited statements synchronously inline.
                // A full state-machine transform would split the function into states
                // at each await point and re-enter on poll. For now, inline execution
                // works for simple async functions that don't cross yield points.
                for s in value {
                    self.emit_stmt(s, builder, func)?;
                }
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
            MirStmt::AuthCall { result, method, args, dbg_line: _ } => {
                self.emit_auth_call(result, method, args, builder)?;
            }
            MirStmt::WorkerCall { result, method, args, dbg_line: _ } => {
                self.emit_worker_call(result, method, args, builder)?;
            }
            MirStmt::DictCall { result, method, args, dbg_line: _ } => {
                self.emit_dict_call(result, method, args, builder)?;
            }
            MirStmt::JsonCall { result, method, args, dbg_line: _ } => {
                self.emit_json_call(result, method, args, builder)?;
            }
            MirStmt::MathCall { result, method, args, dbg_line: _ } => {
                self.emit_math_call(result, method, args, builder)?;
            }
            MirStmt::EnvCall { result, method, args, dbg_line: _ } => {
                self.emit_env_call(result, method, args, builder)?;
            }
            MirStmt::HttpCall { result, method, args, dbg_line: _ } => {
                self.emit_http_call(result, method, args, builder)?;
            }
            MirStmt::IsCall { result, value, type_name, dbg_line: _ } => {
                self.emit_is_call(result, value, type_name, builder)?;
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
        let printf_fn = self.get_printf();

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

    fn emit_wait_stmt(
        &self,
        millis: u64,
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let usleep_fn = self.module.add_function(
            "usleep",
            i32_ty.fn_type(&[i32_ty.into()], false),
            None,
        );
        let micros = (millis as u64) * 1000;
        let micros_val = i32_ty.const_int(micros, false);
        let _ = builder
            .build_call(usleep_fn, &[micros_val.into()], "__wait_usleep")
            .map_err(|e| crate::error::CompileError::new(format!("usleep call: {}", e)))?;
        Ok(())
    }

    fn emit_parallel_stmt(
        &self,
        blocks: &[Vec<MirStmt>],
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let _void_ty = self.context.void_type();

        // Declare pthread_create: int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*start_routine)(void *), void *arg);
        let pthread_create_fn = self.module.add_function(
            "pthread_create",
            i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), ptr_ty.into(), ptr_ty.into()], false),
            None,
        );

        // Declare pthread_join: int pthread_join(pthread_t thread, void **retval);
        let pthread_join_fn = self.module.add_function(
            "pthread_join",
            i32_ty.fn_type(&[i64_ty.into(), ptr_ty.into()], false),
            None,
        );

        // Create an array of pthread_t (i64) on the stack
        let num_threads = blocks.len();
        let threads_array_ty = i64_ty.array_type(num_threads as u32);
        let threads = builder.build_alloca(threads_array_ty, "__parallel_threads").expect("alloca_threads");

        let zero_i32 = i32_ty.const_zero();
        let null_ptr = ptr_ty.const_zero();

        // Create and start each thread
        for (i, block_stmts) in blocks.iter().enumerate() {
            let wrapper_name = format!("__parallel_wrapper_{}", i);

            // Create the wrapper function: void* wrapper(void* arg)
            let wrapper_fn_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
            let wrapper_fn = self.module.add_function(&wrapper_name, wrapper_fn_ty, None);

            // Build the wrapper function body
            let wrapper_entry = self.context.append_basic_block(wrapper_fn, "entry");
            let wrapper_builder = self.context.create_builder();
            wrapper_builder.position_at_end(wrapper_entry);

            // Emit the block statements in the wrapper
            for s in block_stmts {
                // We need a simplified emit for the wrapper — use a recursive call-style approach
                self.emit_stmt_in_wrapper(s, &wrapper_builder, func)?;
            }

            // Return NULL
            wrapper_builder.build_return(Some(&null_ptr)).expect("wrapper_ret");

            // Store thread ID: threads[i] = pthread_create result pointer
            let thread_i_ptr = unsafe {
                builder.build_in_bounds_gep(threads_array_ty, threads, &[zero_i32, i32_ty.const_int(i as u64, false)], &format!("__parallel_thread_{}_ptr", i))
            }.map_err(|e| crate::error::CompileError::new(format!("thread gep: {}", e)))?;

            let create_args: &[BasicMetadataValueEnum] = &[
                thread_i_ptr.into(),
                null_ptr.into(),
                wrapper_fn.as_global_value().as_pointer_value().into(),
                null_ptr.into(),
            ];
            builder.build_call(pthread_create_fn, create_args, &format!("__parallel_create_{}", i))
                .map_err(|e| crate::error::CompileError::new(format!("pthread_create {}: {}", i, e)))?;
        }

        // Join all threads
        for i in 0..num_threads {
            let thread_i_ptr = unsafe {
                builder.build_in_bounds_gep(threads_array_ty, threads, &[zero_i32, i32_ty.const_int(i as u64, false)], &format!("__parallel_join_{}_ptr", i))
            }.map_err(|e| crate::error::CompileError::new(format!("join gep: {}", e)))?;

            let thread_i = builder.build_load(i64_ty, thread_i_ptr, &format!("__parallel_thread_{}", i))
                .map_err(|e| crate::error::CompileError::new(format!("load thread {}: {}", i, e)))?
                .into_int_value();

            let join_args: &[BasicMetadataValueEnum] = &[thread_i.into(), null_ptr.into()];
            builder.build_call(pthread_join_fn, join_args, &format!("__parallel_join_{}", i))
                .map_err(|e| crate::error::CompileError::new(format!("pthread_join {}: {}", i, e)))?;
        }

        Ok(())
    }

    fn emit_stmt_in_wrapper(
        &self,
        stmt: &MirStmt,
        builder: &inkwell::builder::Builder<'static>,
        func: &MirFunction,
    ) -> Result<()> {
        match stmt {
            MirStmt::Alloca { .. } => {} // allocas are irrelevant in wrapper context
            MirStmt::Store { .. } => {}
            MirStmt::Call { callee, args, .. } => {
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let callee_fn = self.module.get_function(callee).unwrap_or_else(|| {
                    let fn_ty = ptr_ty.fn_type(&[ptr_ty.into()], true);
                    self.module.add_function(callee, fn_ty, None)
                });
                let mut llvm_args = Vec::new();
                for arg in args {
                    llvm_args.push(self.load_mir_value_simple(arg, builder));
                }
                builder.build_call(callee_fn, &llvm_args, "wrapper_call")
                    .map_err(|e| crate::error::CompileError::new(format!("wrapper call: {}", e)))?;
            }
            MirStmt::ConsoleCall { method, args, .. } => {
                self.emit_console_call(method, args, builder)?;
            }
            MirStmt::FsCall { result, method, args, .. } => {
                self.emit_fs_call(result, method, args, builder, func)?;
            }
            MirStmt::TransportCall { result, method, args, .. } => {
                self.emit_transport_call(result, method, args, builder)?;
            }
            MirStmt::StringCall { result, method, args, .. } => {
                self.emit_string_call(result, method, args, builder)?;
            }
            MirStmt::RegexCall { result, method, args, .. } => {
                self.emit_regex_call(result, method, args, builder)?;
            }
            MirStmt::DateTimeCall { result, method, args, .. } => {
                self.emit_datetime_call(result, method, args, builder)?;
            }
            MirStmt::AuthCall { result, method, args, .. } => {
                self.emit_auth_call(result, method, args, builder)?;
            }
            MirStmt::WorkerCall { result, method, args, .. } => {
                self.emit_worker_call(result, method, args, builder)?;
            }
            MirStmt::DictCall { result, method, args, .. } => {
                self.emit_dict_call(result, method, args, builder)?;
            }
            MirStmt::JsonCall { result, method, args, .. } => {
                self.emit_json_call(result, method, args, builder)?;
            }
            MirStmt::MathCall { result, method, args, .. } => {
                self.emit_math_call(result, method, args, builder)?;
            }
            MirStmt::EnvCall { result, method, args, .. } => {
                self.emit_env_call(result, method, args, builder)?;
            }
            MirStmt::HttpCall { result, method, args, .. } => {
                self.emit_http_call(result, method, args, builder)?;
            }
            MirStmt::IsCall { result, value, type_name, .. } => {
                self.emit_is_call(result, value, type_name, builder)?;
            }
            MirStmt::Bench { body_stmts, .. } => {
                for s in body_stmts {
                    self.emit_stmt_in_wrapper(s, builder, func)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn load_mir_value_simple(
        &self,
        value: &MirValue,
        builder: &inkwell::builder::Builder<'static>,
    ) -> BasicMetadataValueEnum<'static> {
        let i64_ty = self.context.i64_type();
        match value {
            MirValue::Local(_) => i64_ty.const_zero().into(),
            MirValue::IntLit(v) => i64_ty.const_int(*v as u64, true).into(),
            MirValue::FloatLit(v) => self.context.f64_type().const_float(*v).into(),
            MirValue::BoolLit(v) => self.context.bool_type().const_int(*v as u64, false).into(),
            MirValue::StringLit(s) => {
                builder.build_global_string_ptr(s, "__wrapper_str")
                    .expect("wrapper global string")
                    .as_pointer_value()
                    .into()
            }
            MirValue::CharLit(c) => self.context.i8_type().const_int(*c as u64, false).into(),
            MirValue::Nil => self.context.ptr_type(inkwell::AddressSpace::default()).const_zero().into(),
            MirValue::BinaryOp { .. } | MirValue::UnaryOp { .. } | MirValue::IsInstanceof { .. } => i64_ty.const_zero().into(),
        }
    }

    fn load_mir_value(
        &self,
        value: &MirValue,
        builder: &inkwell::builder::Builder<'static>,
    ) -> inkwell::values::BasicValueEnum<'static> {
        let i64_ty = self.context.i64_type();
        match value {
            MirValue::IntLit(v) => i64_ty.const_int(*v as u64, true).as_basic_value_enum(),
            MirValue::FloatLit(v) => self.context.f64_type().const_float(*v).as_basic_value_enum(),
            MirValue::BoolLit(v) => self.context.bool_type().const_int(*v as u64, false).as_basic_value_enum(),
            MirValue::CharLit(c) => self.context.i8_type().const_int(*c as u64, false).as_basic_value_enum(),
            MirValue::StringLit(s) => {
                builder.build_global_string_ptr(s, "__ret_str")
                    .expect("ret global string ptr")
                    .as_pointer_value()
                    .as_basic_value_enum()
            }
            MirValue::Nil => self.context.ptr_type(inkwell::AddressSpace::default())
                .const_zero()
                .as_basic_value_enum(),
            MirValue::Local(_) | MirValue::BinaryOp { .. } | MirValue::UnaryOp { .. } | MirValue::IsInstanceof { .. } => {
                // Use zero of the return type for uncomputable values
                i64_ty.const_zero().as_basic_value_enum()
            }
        }
    }

    pub fn write_to_file(&self, path: &str) -> Result<()> {
        if self.module.verify().is_ok() {
            self.module.print_to_file(path).map_err(|e| {
                crate::error::CompileError::new(format!("Failed to write bitcode: {}", e))
            })?;
            Ok(())
        } else {
            Err(crate::error::CompileError::new(
                "LLVM module verification failed — cannot write bitcode",
            ))
        }
    }

    pub fn print_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    /// Get or add a global function — avoids duplicate `printf`/`printf.1` etc.
    fn get_or_add_function(
        &self,
        name: &str,
        fn_type: inkwell::types::FunctionType<'static>,
    ) -> inkwell::values::FunctionValue<'static> {
        if let Some(f) = self.module.get_function(name) {
            f
        } else {
            self.module.add_function(name, fn_type, None)
        }
    }

    /// Get or declare printf (used in many places).
    fn get_printf(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_ty = i32_ty.fn_type(&[ptr_ty.into()], true);
        self.get_or_add_function("printf", printf_ty)
    }

    /// Get or declare fopen.
    fn get_fopen(&self) -> inkwell::values::FunctionValue<'static> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let fopen_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        self.get_or_add_function("fopen", fopen_ty)
    }

    /// Get or declare fgets.
    fn get_fgets(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let fgets_ty = ptr_ty.fn_type(&[ptr_ty.into(), i32_ty.into(), ptr_ty.into()], false);
        self.get_or_add_function("fgets", fgets_ty)
    }

    /// Get or declare fclose.
    fn get_fclose(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let fclose_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
        self.get_or_add_function("fclose", fclose_ty)
    }

    /// Get or declare fputs.
    fn get_fputs(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let fputs_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        self.get_or_add_function("fputs", fputs_ty)
    }

    /// Get or declare popen.
    fn get_popen(&self) -> inkwell::values::FunctionValue<'static> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let popen_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        self.get_or_add_function("popen", popen_ty)
    }

    /// Get or declare pclose.
    fn get_pclose(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let pclose_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
        self.get_or_add_function("pclose", pclose_ty)
    }

    /// Get or declare snprintf.
    /// Get or declare strlen.
    fn get_strlen(&self) -> inkwell::values::FunctionValue<'static> {
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let strlen_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        self.get_or_add_function("strlen", strlen_ty)
    }

    /// Get or declare strstr.
    fn get_strstr(&self) -> inkwell::values::FunctionValue<'static> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let strstr_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        self.get_or_add_function("strstr", strstr_ty)
    }

    /// Get or declare strncmp.
    fn get_strncmp(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let strncmp_ty = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i64_ty.into()], false);
        self.get_or_add_function("strncmp", strncmp_ty)
    }

    /// Get or declare tolower.
    fn get_tolower(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        self.get_or_add_function("tolower", i32_ty.fn_type(&[i32_ty.into()], false))
    }

    /// Get or declare toupper.
    fn get_toupper(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        self.get_or_add_function("toupper", i32_ty.fn_type(&[i32_ty.into()], false))
    }

    /// Get or declare isspace.
    fn get_isspace(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        self.get_or_add_function("isspace", i32_ty.fn_type(&[i32_ty.into()], false))
    }

    /// Get or declare snprintf.
    fn get_snprintf(&self) -> inkwell::values::FunctionValue<'static> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let snprintf_ty = i32_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), ptr_ty.into()], true);
        self.get_or_add_function("snprintf", snprintf_ty)
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
        let printf_fn = self.get_printf();

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
            MirValue::BinaryOp { .. } | MirValue::UnaryOp { .. } | MirValue::IsInstanceof { .. } => {
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
                    let open_fn = self.get_fopen();
                    let read_fn = self.get_fgets();
                    let close_fn = self.get_fclose();
                    let print_fn = self.get_printf();

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
                    let open_fn = self.get_fopen();
                    let write_fn = self.get_fputs();
                    let close_fn = self.get_fclose();
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
                    let open_fn = self.get_fopen();
                    let read_fn = self.get_fgets();
                    let write_fn = self.get_fputs();
                    let close_fn = self.get_fclose();

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
        let printf_fn = self.get_printf();

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
            let popen_fn = self.get_popen();
            let fgets_fn = self.get_fgets();
            let pclose_fn = self.get_pclose();

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
        let printf_fn = self.get_printf();

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
        let strlen_fn = self.get_strlen();
        let strstr_fn = self.get_strstr();
        let strncmp_fn = self.get_strncmp();
        let snprintf_fn = self.get_snprintf();
        let _tolower_fn = self.get_tolower();
        let _toupper_fn = self.get_toupper();
        let _isspace_fn = self.get_isspace();

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
                let popen_fn = self.get_popen();
                let fgets_fn = self.get_fgets();
                let pclose_fn = self.get_pclose();

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
                    let popen_fn = self.get_popen();
                    let fgets_fn = self.get_fgets();
                    let pclose_fn = self.get_pclose();
                    let snprintf_fn = self.get_snprintf();

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
        let printf_fn = self.get_printf();

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
        let printf_fn = self.get_printf();

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
                    Some(MirValue::Local(_name)) => {
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

    fn emit_auth_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();

        match method {
            "jwtSign" => {
                let fmt = builder.build_global_string_ptr("[auth] jwtSign: use JS runtime\n", "__auth_jwtsign_stub")
                    .expect("auth jwtSign stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_jwtsign_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "jwtVerify" => {
                let fmt = builder.build_global_string_ptr("[auth] jwtVerify: use JS runtime\n", "__auth_jwtverify_stub")
                    .expect("auth jwtVerify stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_jwtverify_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "jwtDecode" => {
                let fmt = builder.build_global_string_ptr("[auth] jwtDecode: use JS runtime\n", "__auth_jwtdecode_stub")
                    .expect("auth jwtDecode stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_jwtdecode_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "createSession" => {
                let fmt = builder.build_global_string_ptr("[auth] createSession: use JS runtime\n", "__auth_createsess_stub")
                    .expect("auth createSession stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_createsess_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "getSession" => {
                let fmt = builder.build_global_string_ptr("[auth] getSession: use JS runtime\n", "__auth_getsess_stub")
                    .expect("auth getSession stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_getsess_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "destroySession" => {
                let fmt = builder.build_global_string_ptr("[auth] destroySession: use JS runtime\n", "__auth_destroysess_stub")
                    .expect("auth destroySession stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_destroysess_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "hashPassword" => {
                let fmt = builder.build_global_string_ptr("[auth] hashPassword: use JS runtime\n", "__auth_hashpw_stub")
                    .expect("auth hashPassword stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_hashpw_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "verifyPassword" => {
                let fmt = builder.build_global_string_ptr("[auth] verifyPassword: use JS runtime\n", "__auth_verifypw_stub")
                    .expect("auth verifyPassword stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_verifypw_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "checkPermission" => {
                let fmt = builder.build_global_string_ptr("[auth] checkPermission: use JS runtime\n", "__auth_checkperm_stub")
                    .expect("auth checkPermission stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_checkperm_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "hasRole" => {
                let fmt = builder.build_global_string_ptr("[auth] hasRole: use JS runtime\n", "__auth_hasrole_stub")
                    .expect("auth hasRole stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_hasrole_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "hasScope" => {
                let fmt = builder.build_global_string_ptr("[auth] hasScope: use JS runtime\n", "__auth_hasscope_stub")
                    .expect("auth hasScope stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_hasscope_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "oauth2Authorize" => {
                let fmt = builder.build_global_string_ptr("[auth] oauth2Authorize: use JS runtime\n", "__auth_oauth2auth_stub")
                    .expect("auth oauth2Authorize stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_oauth2auth_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "oauth2Token" => {
                let fmt = builder.build_global_string_ptr("[auth] oauth2Token: use JS runtime\n", "__auth_oauth2tok_stub")
                    .expect("auth oauth2Token stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_oauth2tok_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "oauth2Refresh" => {
                let fmt = builder.build_global_string_ptr("[auth] oauth2Refresh: use JS runtime\n", "__auth_oauth2ref_stub")
                    .expect("auth oauth2Refresh stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_oauth2ref_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "passkeyRegister" => {
                let fmt = builder.build_global_string_ptr("[auth] passkeyRegister: use JS runtime\n", "__auth_passkeyreg_stub")
                    .expect("auth passkeyRegister stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_passkeyreg_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "passkeyAuthenticate" => {
                let fmt = builder.build_global_string_ptr("[auth] passkeyAuthenticate: use JS runtime\n", "__auth_passkeyauth_stub")
                    .expect("auth passkeyAuthenticate stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_passkeyauth_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "tenantContext" => {
                let fmt = builder.build_global_string_ptr("[auth] tenantContext: use JS runtime\n", "__auth_tenantctx_stub")
                    .expect("auth tenantContext stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_tenantctx_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "getTenant" => {
                let fmt = builder.build_global_string_ptr("[auth] getTenant: use JS runtime\n", "__auth_gettenant_stub")
                    .expect("auth getTenant stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_gettenant_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "listTenants" => {
                let fmt = builder.build_global_string_ptr("[auth] listTenants: use JS runtime\n", "__auth_listtenants_stub")
                    .expect("auth listTenants stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_listtenants_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "createTenant" => {
                let fmt = builder.build_global_string_ptr("[auth] createTenant: use JS runtime\n", "__auth_createtenant_stub")
                    .expect("auth createTenant stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_createtenant_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "grantRole" => {
                let fmt = builder.build_global_string_ptr("[auth] grantRole: use JS runtime\n", "__auth_grantrole_stub")
                    .expect("auth grantRole stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_grantrole_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "grantPermission" => {
                let fmt = builder.build_global_string_ptr("[auth] grantPermission: use JS runtime\n", "__auth_grantperm_stub")
                    .expect("auth grantPermission stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_grantperm_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "revokeRole" => {
                let fmt = builder.build_global_string_ptr("[auth] revokeRole: use JS runtime\n", "__auth_revokerole_stub")
                    .expect("auth revokeRole stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_revokerole_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "revokePermission" => {
                let fmt = builder.build_global_string_ptr("[auth] revokePermission: use JS runtime\n", "__auth_revokeperm_stub")
                    .expect("auth revokePermission stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_revokeperm_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "generateApiKey" => {
                let fmt = builder.build_global_string_ptr("[auth] generateApiKey: use JS runtime\n", "__auth_genapikey_stub")
                    .expect("auth generateApiKey stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_genapikey_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "validateApiKey" => {
                let fmt = builder.build_global_string_ptr("[auth] validateApiKey: use JS runtime\n", "__auth_valapikey_stub")
                    .expect("auth validateApiKey stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_valapikey_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "checkAccess" => {
                let fmt = builder.build_global_string_ptr("[auth] checkAccess: use JS runtime\n", "__auth_checkaccess_stub")
                    .expect("auth checkAccess stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_checkaccess_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "setRoles" => {
                let fmt = builder.build_global_string_ptr("[auth] setRoles: use JS runtime\n", "__auth_setroles_stub")
                    .expect("auth setRoles stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_setroles_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "setPermissions" => {
                let fmt = builder.build_global_string_ptr("[auth] setPermissions: use JS runtime\n", "__auth_setperms_stub")
                    .expect("auth setPermissions stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_setperms_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            _ => {
                let fmt = builder.build_global_string_ptr(
                    &format!("[auth] {}: use JS runtime\n", method), "__auth_unknown_fmt"
                ).expect("auth unknown fmt");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__auth_unknown_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
        }
        Ok(())
    }

    fn emit_worker_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();

        match method {
            "create" => {
                let fmt = builder.build_global_string_ptr("[worker] create: use JS runtime\n", "__wr_create_stub")
                    .expect("worker create stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_create_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "send" => {
                let fmt = builder.build_global_string_ptr("[worker] send: use JS runtime\n", "__wr_send_stub")
                    .expect("worker send stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_send_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "post" => {
                let fmt = builder.build_global_string_ptr("[worker] post: use JS runtime\n", "__wr_post_stub")
                    .expect("worker post stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_post_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "receive" => {
                let fmt = builder.build_global_string_ptr("[worker] receive: use JS runtime\n", "__wr_recv_stub")
                    .expect("worker receive stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_recv_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "wait" => {
                let fmt = builder.build_global_string_ptr("[worker] wait: use JS runtime\n", "__wr_wait_stub")
                    .expect("worker wait stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_wait_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "terminate" => {
                let fmt = builder.build_global_string_ptr("[worker] terminate: use JS runtime\n", "__wr_term_stub")
                    .expect("worker terminate stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_term_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "isRunning" => {
                let fmt = builder.build_global_string_ptr("[worker] isRunning: use JS runtime\n", "__wr_run_stub")
                    .expect("worker isRunning stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_run_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "activeCount" => {
                let fmt = builder.build_global_string_ptr("[worker] activeCount: use JS runtime\n", "__wr_cnt_stub")
                    .expect("worker activeCount stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_cnt_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            "terminateAll" => {
                let fmt = builder.build_global_string_ptr("[worker] terminateAll: use JS runtime\n", "__wr_termall_stub")
                    .expect("worker terminateAll stub");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_termall_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
            _ => {
                let fmt = builder.build_global_string_ptr(
                    &format!("[worker] {}: use JS runtime\n", method), "__wr_unknown_fmt"
                ).expect("worker unknown fmt");
                let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__wr_unknown_call")
                    .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
            }
        }
        Ok(())
    }

    fn emit_dict_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            &format!("[dict] {}: use JS runtime\n", method), "__dict_fmt"
        ).expect("dict fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__dict_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    fn emit_json_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            &format!("[json] {}: use JS runtime\n", method), "__json_fmt"
        ).expect("json fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__json_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    fn emit_math_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            &format!("[math] {}: use JS runtime\n", method), "__math_fmt"
        ).expect("math fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__math_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    fn emit_env_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            &format!("[env] {}: use JS runtime\n", method), "__env_fmt"
        ).expect("env fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__env_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    fn emit_http_call(
        &self,
        _result: &Option<String>,
        method: &str,
        _args: &[MirValue],
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            &format!("[http] {}: use JS runtime\n", method), "__http_fmt"
        ).expect("http fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__http_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }

    fn emit_is_call(
        &self,
        _result: &Option<String>,
        _value: &MirValue,
        _type_name: &MirValue,
        builder: &inkwell::builder::Builder<'static>,
    ) -> Result<()> {
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_fn = self.get_printf();
        let fmt = builder.build_global_string_ptr(
            "[is] instanceof: use JS runtime\n", "__is_fmt"
        ).expect("is fmt");
        let _ = builder.build_call(printf_fn, &[fmt.as_pointer_value().into()], "__is_call")
            .map_err(|e| crate::error::CompileError::new(format!("printf: {}", e)))?;
        Ok(())
    }
}
