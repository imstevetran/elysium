//! Compile pipeline: type-check → MIR → LLVM → link.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast;
use crate::backend::codegen;
use crate::debug;
use crate::error;
use crate::extension;
use crate::hir;
use crate::mir;
use crate::ownership;
use crate::parser;
use crate::type_checker;

use super::desugar::desugar_builtin_calls;
use super::imports::{find_project_root, load_with_imports};
use super::source::{has_imports, read_source};
use super::stubs::{filter_stubs, filter_stubs_raw};

/// Compile a C source file to a .o file (no caching; extensions change infrequently).
pub fn compile_c_file(c_path: &Path) -> error::Result<PathBuf> {
    let cache_dir = std::env::temp_dir().join("elysium_ext_cache");
    let _ = fs::create_dir_all(&cache_dir);
    let stem = c_path.file_stem().unwrap_or_default();
    let o_path = cache_dir.join(format!("{}.o", stem.to_string_lossy()));
    let status = Command::new("clang")
        .arg("-c")
        .arg(c_path)
        .arg("-o")
        .arg(&o_path)
        .status()
        .map_err(|e| error::CompileError::new(format!("failed to compile extension C file: {}", e)))?;
    if !status.success() {
        return Err(error::CompileError::new(
            format!("clang compilation of `{}` failed", c_path.display())
        ));
    }
    Ok(o_path)
}

/// Compile the merged program (root + all imports) through the full pipeline.
pub fn compile_merged(
    source: &str,
    program: &ast::Program,
    file_path: &str,
    debug_enabled: bool,
    emit_ir: bool,
    output: Option<PathBuf>,
) -> error::Result<()> {
    // Type check
    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(program)?;

    // Ownership check
    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(program)?;

    // Lower: AST → HIR → MIR
    let hir_program = hir::lower(program, source);
    let mir_program = mir::lower(&hir_program, 1);

    // Codegen
    let mut codegen = codegen::Codegen::new("elysium_program")?;

    if debug_enabled {
        let debug_info = debug::DebugInfo::new();
        codegen.set_debug_info(debug_info);
    }

    codegen.compile(&mir_program, file_path)?;

    if emit_ir {
        println!("{}", codegen.print_ir());
    } else {
        let out_path = output.unwrap_or_else(|| PathBuf::from("a.out"));

        // Load extension runtime files
        let project_root = find_project_root(Path::new(".")).unwrap_or_else(|| PathBuf::from("."));
        let extensions = extension::load_extensions(&project_root)?;
        let mut ext_o_files: Vec<PathBuf> = Vec::new();
        for ext in &extensions {
            for runtime_path in &ext.runtime_files {
                let o_path = compile_c_file(runtime_path)?;
                ext_o_files.push(o_path);
            }
        }

        // Write LLVM bitcode
        let bc_path = out_path.with_extension("bc");
        codegen.write_to_file(bc_path.to_str().unwrap())?;

        // Compile .bc to .o
        let obj_path = out_path.with_extension("o");
        let compile_status = Command::new("clang")
            .arg("-c")
            .arg(&bc_path)
            .arg("-o")
            .arg(&obj_path)
            .status()
            .map_err(|e| error::CompileError::new(format!("failed to compile .bc: {}", e)))?;

        if !compile_status.success() {
            let _ = fs::remove_file(&bc_path);
            return Err(error::CompileError::new("clang compilation of .bc failed"));
        }

        // Link: .o + extension .o files -> executable
        let mut link_cmd = Command::new("clang");
        link_cmd.arg("-o").arg(&out_path)
            .arg(&obj_path);
        for ext_o in &ext_o_files {
            link_cmd.arg(ext_o);
        }
        link_cmd.arg("-lm")
            .arg("-framework").arg("CoreFoundation");

        let status = link_cmd.status()
            .map_err(|e| error::CompileError::new(format!("failed to invoke clang: {}", e)))?;

        if !status.success() {
            let _ = fs::remove_file(&bc_path);
            let _ = fs::remove_file(&obj_path);
            return Err(error::CompileError::new("linking with clang failed"));
        }

        // Clean up intermediate files
        let _ = fs::remove_file(&bc_path);
        let _ = fs::remove_file(&obj_path);

        println!("Compiled to {}", out_path.display());
    }

    Ok(())
}

pub fn compile_file(file: &PathBuf, output: Option<PathBuf>, emit_ir: bool, debug_enabled: bool, env: &str) -> error::Result<()> {
    let file_path = file.to_string_lossy().to_string();

    let (source, mut program) = if has_imports(file)? {
        let (src, p, aliases) = load_with_imports(file)?;
        (src, filter_stubs(p, &aliases, env))
    } else {
        let source = read_source(file)?;
        let mut parser = parser::Parser::new(&source);
        let program = parser.parse_program()?;
        let program = filter_stubs_raw(program, env);
        (source, program)
    };
    desugar_builtin_calls(&mut program);

    compile_merged(&source, &program, &file_path, debug_enabled, emit_ir, output)
}

pub fn compile_and_run(file: &PathBuf, debug_enabled: bool, emit_ir: bool, env: &str) -> error::Result<()> {
    let file_path = file.to_string_lossy().to_string();

    let (source, mut program) = if has_imports(file)? {
        let (src, p, aliases) = load_with_imports(file)?;
        (src, filter_stubs(p, &aliases, env))
    } else {
        let source = read_source(file)?;
        let mut parser = parser::Parser::new(&source);
        let program = parser.parse_program()?;
        let program = filter_stubs_raw(program, env);
        (source, program)
    };
    desugar_builtin_calls(&mut program);

    compile_merged(&source, &program, &file_path, debug_enabled, emit_ir, Some(PathBuf::from("output.bc")))?;
    println!("Compiled successfully.");
    Ok(())
}

pub fn check_file(file: &PathBuf, env: &str) -> error::Result<()> {
    let (_source, mut program) = if has_imports(file)? {
        let (src, p, aliases) = load_with_imports(file)?;
        (src, filter_stubs(p, &aliases, env))
    } else {
        let source = read_source(file)?;
        let mut parser = parser::Parser::new(&source);
        let program = parser.parse_program()?;
        let program = filter_stubs_raw(program, env);
        (source, program)
    };
    desugar_builtin_calls(&mut program);

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed.");
    Ok(())
}
