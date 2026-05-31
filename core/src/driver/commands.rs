//! CLI helpers: test, highlight, lint, doc, dep-graph, gen-test, port.

use std::fs;
use std::path::PathBuf;

use crate::ast;
use crate::codegen_tools;
use crate::error;
use crate::highlighter;
use crate::linter;
use crate::parser;
use crate::port;
use crate::test_runner;

use super::desugar::desugar_builtin_calls;
use super::imports::load_with_imports;
use super::source::{has_imports, read_source};
use super::stubs::{filter_stubs, filter_stubs_raw};

pub fn cmd_test(path: &Option<PathBuf>, dry_run: bool, env: &str) -> error::Result<()> {
    let test_path = path.as_ref().cloned().unwrap_or_else(|| PathBuf::from("core/examples/"));

    // Collect files to test
    let files: Vec<PathBuf> = if test_path.is_dir() {
        test_runner::find_test_files(&test_path)
    } else if test_path.exists() {
        vec![test_path]
    } else {
        let default_dir = PathBuf::from("core/examples/");
        if default_dir.is_dir() {
            test_runner::find_test_files(&default_dir)
        } else {
            println!("No test files found at {:?}.", test_path);
            return Ok(());
        }
    };

    if files.is_empty() {
        println!("No test files found.");
        return Ok(());
    }

    if dry_run {
        for file in &files {
            println!("[{}]", file.display());
            let source = read_source(file)?;
            let report = test_runner::list_tests(&source)?;
            println!("{}", report);
        }
        return Ok(());
    }

    let mut all_passed = true;

    for file in &files {
        println!("[{}]", file.display());
        let source = read_source(file)?;

        // Parse with imports
        let (source, mut program) = if has_imports(file)? {
            let (src, p, aliases) = load_with_imports(file)?;
            (src, filter_stubs(p, &aliases, env))
        } else {
            let mut parser = parser::Parser::new(&source);
            let p = parser.parse_program()?;
            let p = filter_stubs_raw(p, env);
            (source, p)
        };
        desugar_builtin_calls(&mut program);

        // Validate all specs via type-checking
        match test_runner::run_tests_in_file(&source, &mut program) {
            Ok(passed) => {
                if !passed {
                    all_passed = false;
                }
            }
            Err(e) => {
                eprintln!("  Error checking tests: {}", e.message);
                all_passed = false;
            }
        }
    }

    println!(
        "\nSummary: {} across {} file(s)",
        if all_passed { "ALL TESTS PASSED ✓" } else { "SOME TESTS FAILED ✗" },
        files.len()
    );

    if all_passed {
        Ok(())
    } else {
        Err(error::CompileError::new("Some tests failed"))
    }
}
pub fn highlight_file(
    file: &PathBuf,
    format: &str,
    output: &Option<PathBuf>,
) -> error::Result<()> {
    let source = read_source(file)?;

    match format {
        "ansi" => {
            highlighter::print_ansi(&source)
                .map_err(|e| error::CompileError::new(format!("Failed to write output: {}", e)))
        }
        "html" => {
            let html = highlighter::to_html(&source);
            if let Some(out_path) = output {
                fs::write(out_path, html)
                    .map_err(|e| error::CompileError::new(format!("Failed to write file: {}", e)))
            } else {
                println!("{}", html);
                Ok(())
            }
        }
        other => Err(error::CompileError::new(format!(
            "Unknown format '{}'. Use 'ansi' or 'html'.",
            other
        ))),
    }
}

pub fn lint_file(file: &PathBuf, format: &str) -> error::Result<()> {
    let source = read_source(file)?;

    match format {
        "text" => {
            let mut parser = parser::Parser::new(&source);
            match parser.parse_program() {
                Ok(program) => {
                    let result = linter::lint(&source, &program);
                    if result.diagnostics.is_empty() {
                        println!("No lint issues found.");
                    } else {
                        for diag in &result.diagnostics {
                            let line = source[..diag.offset].lines().count();
                            let prefix = diag.severity.prefix();
                            let help = diag
                                .help
                                .as_ref()
                                .map(|h| format!("\n  help: {}", h))
                                .unwrap_or_default();
                            println!(
                                "  {}[{}]: {} (line {}){}",
                                prefix, diag.rule_id, diag.message, line, help
                            );
                        }
                        println!("\nFound {} issue(s).", result.diagnostics.len());
                    }
                }
                Err(parse_err) => {
                    eprintln!("Parse error: {}", parse_err.message);
                    let result = linter::lint(&source, &ast::Program { items: vec![] });
                    for diag in &result.diagnostics {
                        let line = source[..diag.offset].lines().count();
                        let prefix = diag.severity.prefix();
                        println!("  {}[{}]: {} (line {})", prefix, diag.rule_id, diag.message, line);
                    }
                }
            }
            Ok(())
        }
        "json" => {
            let mut parser = parser::Parser::new(&source);
            match parser.parse_program() {
                Ok(program) => {
                    let result = linter::lint(&source, &program);
                    println!("[");
                    for (i, diag) in result.diagnostics.iter().enumerate() {
                        if i > 0 {
                            println!(",");
                        }
                        println!(
                            r#"  {{"severity":"{}","rule_id":"{}","message":"{}","offset":{},"length":{}}}"#,
                            diag.severity.prefix(),
                            diag.rule_id,
                            diag.message.replace('"', r#"\""#),
                            diag.offset,
                            diag.length
                        );
                    }
                    println!("]");
                }
                Err(e) => {
                    println!(
                        r#"[{{"severity":"error","rule_id":"parse-error","message":"{}"}}]"#,
                        e.message.replace('"', r#"\""#)
                    );
                }
            }
            Ok(())
        }
        other => Err(error::CompileError::new(format!(
            "Unknown format '{}'. Use 'text' or 'json'.",
            other
        ))),
    }
}

// ============================================================================
// Doc, Dep-Graph, Gen-Test
// ============================================================================

pub fn doc_file(file: &PathBuf, output: &Option<PathBuf>) -> error::Result<()> {
    let source = read_source(file)?;
    let program = codegen_tools::parse_source(&source)?;
    let md = codegen_tools::generate_doc(&source, &program);

    if let Some(out_path) = output {
        fs::write(out_path, md)
            .map_err(|e| error::CompileError::new(format!("Failed to write doc: {}", e)))
    } else {
        println!("{}", md);
        Ok(())
    }
}

pub fn dep_graph_file(file: &PathBuf, format: &str, output: &Option<PathBuf>) -> error::Result<()> {
    let source = read_source(file)?;
    let file_name = file.to_string_lossy().to_string();
    let program = codegen_tools::parse_source(&source)?;

    match format {
        "dot" => {
            let dot = codegen_tools::render_dot(&file_name, &program);
            if let Some(out_path) = output {
                fs::write(out_path, dot)
                    .map_err(|e| error::CompileError::new(format!("Failed to write DOT: {}", e)))
            } else {
                println!("{}", dot);
                Ok(())
            }
        }
        "json" => {
            let json = codegen_tools::render_json(&file_name, &program);
            if let Some(out_path) = output {
                fs::write(out_path, json)
                    .map_err(|e| error::CompileError::new(format!("Failed to write JSON: {}", e)))
            } else {
                println!("{}", json);
                Ok(())
            }
        }
        other => Err(error::CompileError::new(format!(
            "Unknown format '{}'. Use 'dot' or 'json'.",
            other
        ))),
    }
}

pub fn gen_test_file(file: &PathBuf, output: &Option<PathBuf>) -> error::Result<()> {
    let source = read_source(file)?;
    let file_name = file.to_string_lossy().to_string();
    let program = codegen_tools::parse_source(&source)?;
    let tests = codegen_tools::generate_tests(&file_name, &source, &program);

    if let Some(out_path) = output {
        fs::write(out_path, tests)
            .map_err(|e| error::CompileError::new(format!("Failed to write tests: {}", e)))
    } else {
        println!("{}", tests);
        Ok(())
    }
}

pub fn port_file(file: &PathBuf, output: &Option<PathBuf>, lang: &Option<String>) -> error::Result<()> {
    port::port_file(file.as_path(), output.as_ref().map(|p| p.as_path()), lang)
}
