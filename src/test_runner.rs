use crate::ast::*;
use crate::error;
use crate::parser;
use crate::type_checker;

/// Discover and validate spec-driven tests in Elysium files.
///
/// Elysium's `spec`/`feat`/`expect` are compile-time constructs validated by
/// the type checker. The `test` command:
///   1. Type-checks each spec and its feats
///   2. Reports pass/fail per spec
///
/// `expect <expr>` validates that `<expr>` is well-typed.
/// Returns `true` if all tests pass.
pub fn run_tests_in_file(source: &str, program: &mut Program) -> error::Result<bool> {
    let specs = collect_specs(&program.items);
    if specs.is_empty() {
        println!("  (no specs found)");
        return Ok(true);
    }

    let total_feats: usize = specs.iter().map(|s| s.1.len()).sum();
    println!(
        "  Specs: {} | Tests: {}",
        specs.len(),
        total_feats
    );

    // Type-check (validates all expect statements)
    let type_result = {
        let mut tc = type_checker::TypeChecker::new();
        tc.check_program(program)
    };

    match type_result {
        Ok(()) => {
            for (name, feats) in &specs {
                println!("    ✓ \"{}\"", name);
                for feat in feats {
                    println!("      ✓ {}", feat);
                }
            }
            println!("  All tests passed. ✓");
            Ok(true)
        }
        Err(e) => {
            eprintln!("  ✗ Type check failed: {}", e.message);
            Ok(false)
        }
    }
}

/// List specs and feats without type-checking (--dry-run).
pub fn list_tests(source: &str) -> error::Result<String> {
    let mut parser = parser::Parser::new(source);
    let program = parser.parse_program()?;
    let specs = collect_specs(&program.items);

    if specs.is_empty() {
        return Ok("  (no specs found)".to_string());
    }

    let mut out = String::new();
    let total: usize = specs.iter().map(|s| s.1.len()).sum();
    out.push_str(&format!(
        "  Specs: {} | Tests: {}\n",
        specs.len(),
        total
    ));
    for (name, feats) in &specs {
        out.push_str(&format!("    \"{}\"\n", name));
        for feat in feats {
            out.push_str(&format!("      - {}\n", feat));
        }
    }
    Ok(out)
}

fn collect_specs(items: &[Node<Item>]) -> Vec<(String, Vec<String>)> {
    let mut specs = Vec::new();
    for item in items {
        if let Item::Spec(s) = &item.value {
            let feats: Vec<String> = s.feats.iter().map(|f| f.name.clone()).collect();
            specs.push((s.name.clone(), feats));
        }
    }
    specs
}

/// Find `.ely` files in a directory (non-recursive, sorted).
pub fn find_test_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "ely").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
