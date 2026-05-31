// Debug information for Elysium.
//
// 1. DWARF metadata emission via inkwell's DIBuilder so lldb/gdb
//    can set breakpoints and show source locations.
// 2. REPL: interactive Read-Eval-Print-Loop.

use crate::error::Result;
use crate::mir::*;
use inkwell::debug_info::{AsDIScope, DebugInfoBuilder, DWARFEmissionKind, DWARFSourceLanguage};
use inkwell::module::Module;
use inkwell::values::FunctionValue;

/// DebugInfo wraps an LLVM DIBuilder and emits DWARF metadata.
///
/// Note: In inkwell 0.9, `Module::create_debug_info_builder` takes all
/// compile-unit parameters directly and returns (DebugInfoBuilder, DICompileUnit).
pub struct DebugInfo<'ctx> {
    builder: Option<DebugInfoBuilder<'ctx>>,
    file: Option<inkwell::debug_info::DIFile<'ctx>>,
    current_subprogram: Option<inkwell::debug_info::DISubprogram<'ctx>>,
    pub enabled: bool,
}

impl<'ctx> DebugInfo<'ctx> {
    pub fn new() -> Self {
        DebugInfo {
            builder: None,
            file: None,
            current_subprogram: None,
            enabled: true,
        }
    }

    /// Initialise the DIBuilder and compile unit.
    pub fn init(&mut self, module: &Module<'ctx>, source_path: &str) {
        if !self.enabled {
            return;
        }
        let (builder, _cu) = module.create_debug_info_builder(
            false,               // allow_unresolved
            DWARFSourceLanguage::C,
            source_path,
            ".",
            "Elysium 2.0",
            false,              // is_optimized
            "",                 // flags
            0,                  // runtime_version
            "",                 // split_name
            DWARFEmissionKind::Full,
            0,                  // dwo_id
            false,              // split_debug_inlining
            false,              // debug_info_for_profiling
            "",                 // sysroot
            "",                 // sdk
        );
        let file = builder.create_file(source_path, ".");
        self.builder = Some(builder);
        self.file = Some(file);
    }

    /// Create a DISubprogram for a function value.
    pub fn create_function(
        &mut self,
        fn_val: &FunctionValue<'ctx>,
        func: &MirFunction,
    ) {
        if !self.enabled {
            return;
        }
        let (builder, file) = match (&self.builder, &self.file) {
            (Some(b), Some(f)) => (b, f),
            _ => return,
        };
        let scope = file.as_debug_info_scope();
        let fn_type = builder.create_subroutine_type(
            *file,
            None,
            &[],
            0, // DIFlags
        );
        let sp = builder.create_function(
            scope,
            &func.name,
            None, // linkage_name
            *file,
            func.dbg_line,
            fn_type,
            true,  // is_local_to_unit
            true,  // is_definition
            func.dbg_line,
            0, // DIFlags
            false, // is_optimized
        );
        fn_val.set_subprogram(sp);
        self.current_subprogram = Some(sp);
    }

    /// Set the debug location on a builder.
    pub fn set_location(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
        module: &Module<'ctx>,
        line: u32,
    ) {
        if !self.enabled || line == 0 {
            return;
        }
        let db = match &self.builder {
            Some(b) => b,
            None => return,
        };
        if let Some(ref sp) = self.current_subprogram {
            let loc = db.create_debug_location(
                module.get_context(),
                line,
                0,
                sp.as_debug_info_scope(),
                None,
            );
            builder.set_current_debug_location(loc);
        }
    }

    /// Finalise debug metadata. Call after all IR is emitted.
    pub fn finalize(&self) {
        if let Some(ref builder) = self.builder {
            builder.finalize();
        }
    }
}

/// Elysium REPL (Read-Eval-Print-Loop) for interactive exploration.
pub struct Repl;

impl Repl {
    pub fn new() -> Self {
        Repl
    }

    pub fn run(&mut self) -> Result<()> {
        println!("Elysium 2.0 REPL");
        println!("Type expressions or statements. Press Ctrl+C to exit.");
        println!("  :help     Show help");
        println!("  :exit     Exit the REPL");
        println!();

        let mut code_buffer = String::new();

        loop {
            let line = match read_line("elys> ") {
                Some(l) => l,
                None => break,
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if matches!(trimmed, "exit" | ":q" | ":quit") {
                break;
            }
            if matches!(trimmed, ":help" | ":h") {
                println!("Commands:");
                println!("  :help  :h     Show this help");
                println!("  :exit  :q     Exit the REPL");
                println!("  :reset        Clear the code buffer");
                continue;
            }
            if trimmed == ":reset" {
                code_buffer.clear();
                println!("Buffer cleared.");
                continue;
            }

            code_buffer.push_str(&line);
            code_buffer.push('\n');

            match eval_repl(&code_buffer) {
                Ok(output) => {
                    if !output.is_empty() {
                        println!("=> {}", output);
                    }
                }
                Err(e) => {
                    eprintln!("  {}", e.message);
                }
            }
        }

        println!("\nGoodbye!");
        Ok(())
    }
}

fn read_line(prompt: &str) -> Option<String> {
    use std::io::{self, Write};
    print!("{}", prompt);
    io::stdout().flush().ok()?;
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line),
        Err(_) => None,
    }
}

fn eval_repl(code: &str) -> std::result::Result<String, crate::error::CompileError> {
    let mut parser = crate::parser::Parser::new(code);
    let program = parser.parse_program()?;
    let mut type_checker = crate::type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;
    let mut ownership = crate::ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;
    let item_count = program.items.len();
    Ok(format!(
        "OK ({} top-level item{})",
        item_count,
        if item_count == 1 { "" } else { "s" }
    ))
}
