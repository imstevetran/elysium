mod ast;
mod cli;
mod codegen;
mod codegen_tools;
mod debug;
mod elyx;
mod error;
mod highlighter;
mod hir;
mod lexer;
mod linter;
mod mir;
mod module;
mod ownership;
mod parser;
mod type_checker;

use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let cli = cli::Cli::parse();

    let result = match &cli.command {
        cli::Commands::Build { file, output, emit_ir, debug, env } => {
            let env = resolve_env_alias(file, env);
            if is_elyx_file(file) {
                build_elyx(file, output.clone(), *emit_ir)
            } else {
                compile_file(file, output.clone(), *emit_ir, *debug, &env)
            }
        }
        cli::Commands::Run { file, debug, emit_ir, env } => {
            let env = resolve_env_alias(file, env);
            if is_elyx_file(file) {
                build_elyx(file, None, false)
            } else {
                compile_and_run(file, *debug, *emit_ir, &env)
            }
        }
        cli::Commands::Check { file, env } => {
            let env = resolve_env_alias(file, env);
            if is_elyx_file(file) {
                check_elyx(file)
            } else {
                check_file(file, &env)
            }
        }
        cli::Commands::Highlight { file, format, output } => {
            highlight_file(file, format, output)
        }
        cli::Commands::Lint { file, format } => {
            lint_file(file, format)
        }
        cli::Commands::HighlightCss => {
            println!("{}", highlighter::css());
            Ok(())
        }
        cli::Commands::Repl => {
            let mut repl = debug::Repl::new();
            repl.run()
        }
        cli::Commands::Doc { file, output } => {
            doc_file(file, output)
        }
        cli::Commands::DepGraph { file, format, output } => {
            dep_graph_file(file, format, output)
        }
        cli::Commands::GenTest { file, output } => {
            gen_test_file(file, output)
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e.message);
        std::process::exit(1);
    }
}

fn is_elyx_file(file: &PathBuf) -> bool {
    file.extension().map(|ext| ext == "elyx").unwrap_or(false)
}

fn read_source(file: &PathBuf) -> error::Result<String> {
    fs::read_to_string(file)
        .map_err(|e| error::CompileError::new(format!("Failed to read file: {}", e)))
}

/// Load a source file and resolve all its imports into a single merged Program.
/// Returns the source, program, and a set of known module aliases for desugaring.
fn load_with_imports(file: &PathBuf) -> error::Result<(String, ast::Program, std::collections::HashSet<String>)> {
    let source = read_source(file)?;

    // Parse the root file
    let mut parser = parser::Parser::new(&source);
    let root_program = parser.parse_program()?;

    // Resolve imports by loading referenced files
    let mut all_items: Vec<ast::Node<ast::Item>> = Vec::new();
    let mut module_aliases = std::collections::HashSet::new();

    for item in &root_program.items {
        match &item.value {
            ast::Item::Import(import_path, alias) => {
                // Resolve relative to the importing file's directory
                let from_dir = file.parent().unwrap_or(Path::new("."));
                let resolved = find_import_file(from_dir, import_path)
                    .map_err(|e| error::CompileError::new(e))?;

                // Read and parse the imported file
                let imported_source = read_source(&resolved)?;
                let imported_program = if resolved.extension().map(|e| e == "elyx").unwrap_or(false) {
                    let elyx_file = elyx::parse_elyx(&imported_source)?;
                    ast::Program {
                        items: vec![ast::Node::new(
                            ast::Item::Component(elyx_file.component.value.clone()),
                            elyx_file.component.span.clone(),
                        )],
                    }
                } else {
                    let mut p = parser::Parser::new(&imported_source);
                    p.parse_program()?
                };

                // Add all items from the imported file (skip its own imports for now)
                for imported_item in &imported_program.items {
                    if !matches!(imported_item.value, ast::Item::Import(..)) {
                        let mut renamed = imported_item.clone();
                        // If alias is present, prefix all item names with `{alias}_`
                        if let Some(alias_name) = alias {
                            module_aliases.insert(alias_name.clone());
                            match &mut renamed.value {
                                ast::Item::Function(f) => {
                                    f.name = format!("{}_{}", alias_name, f.name);
                                }
                                ast::Item::Class(c) => {
                                    c.name = format!("{}_{}", alias_name, c.name);
                                }
                                ast::Item::Enum(e) => {
                                    e.name = format!("{}_{}", alias_name, e.name);
                                }
                                ast::Item::TypeAlias(ta) => {
                                    ta.name = format!("{}_{}", alias_name, ta.name);
                                }
                                ast::Item::Component(c) => {
                                    c.name = format!("{}_{}", alias_name, c.name);
                                }
                                _ => {}
                            }
                        }
                        all_items.push(renamed);
                    }
                }
            }
            _ => {
                all_items.push(item.clone());
            }
        }
    }

    Ok((source, ast::Program { items: all_items }, module_aliases))
}

/// Desugar module-aliased calls like `math.add(1, 2)` into `math_add(1, 2)`
/// and property access like `math.PI` into `math_PI`.
/// This must be called after import resolution and before type-checking.
fn desugar_module_calls(program: &mut ast::Program, aliases: &std::collections::HashSet<String>) {
    for item in &mut program.items {
        if let ast::Item::Function(f) = &mut item.value {
            desugar_module_calls_in_block(&mut f.body, aliases);
        }
    }
}

fn desugar_module_calls_in_block(block: &mut ast::Block, aliases: &std::collections::HashSet<String>) {
    for stmt in &mut block.statements {
        desugar_module_calls_in_stmt(stmt, aliases);
    }
}

fn desugar_module_calls_in_stmt(stmt: &mut ast::Node<ast::Stmt>, aliases: &std::collections::HashSet<String>) {
    match &mut stmt.value {
        ast::Stmt::Let(boxed) => {
            if let Some(val) = &mut boxed.value.value {
                desugar_module_calls_in_expr(val, aliases);
            }
        }
        ast::Stmt::Expr(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value, aliases);
        }
        ast::Stmt::Return(ret) => {
            if let Some(e) = ret {
                desugar_module_calls_in_expr(&mut e.value, aliases);
            }
        }
        ast::Stmt::Assign(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.target.value, aliases);
            desugar_module_calls_in_expr(&mut boxed.value.value.value, aliases);
        }
        ast::Stmt::If(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.condition.value, aliases);
            desugar_module_calls_in_block(&mut boxed.value.then_block, aliases);
            if let Some(eb) = &mut boxed.value.else_block {
                desugar_module_calls_in_block(eb, aliases);
            }
        }
        ast::Stmt::For(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.iterable.value, aliases);
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
        ast::Stmt::While(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.condition.value, aliases);
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
        ast::Stmt::Match(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.value.value, aliases);
            for arm in &mut boxed.value.arms {
                desugar_module_calls_in_block(&mut arm.body, aliases);
            }
        }
        ast::Stmt::TryCatch(boxed) => {
            desugar_module_calls_in_block(&mut boxed.value.try_block, aliases);
            desugar_module_calls_in_block(&mut boxed.value.catch_block, aliases);
        }
        ast::Stmt::OnlyGuard(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.condition.value, aliases);
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
        ast::Stmt::UnsafeBlock(boxed) => {
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
        ast::Stmt::BcAssert(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.condition.value, aliases);
        }
        ast::Stmt::Expect(boxed) => {
            desugar_module_calls_in_expr(&mut boxed.value.expr.value, aliases);
        }
        ast::Stmt::Todo(_) | ast::Stmt::Question(_) => {},
        ast::Stmt::Bench(boxed) => {
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
    }
}

fn desugar_module_calls_in_expr(expr: &mut ast::Expr, aliases: &std::collections::HashSet<String>) {
    match expr {
        ast::Expr::Literal(_) | ast::Expr::Identifier(_) => {}
        ast::Expr::BinaryOp { left, right, .. } => {
            desugar_module_calls_in_expr(&mut left.value, aliases);
            desugar_module_calls_in_expr(&mut right.value, aliases);
        }
        ast::Expr::UnaryOp { operand, .. } => {
            desugar_module_calls_in_expr(&mut operand.value, aliases);
        }
        ast::Expr::Call { callee, args } => {
            desugar_module_calls_in_expr(&mut callee.value, aliases);
            for arg in args {
                desugar_module_calls_in_expr(&mut arg.value, aliases);
            }
        }
        ast::Expr::MethodCall { object, method, args } => {
            // Check if object is an alias identifier → desugar to normal call
            if let ast::Expr::Identifier(alias_name) = &object.value {
                if aliases.contains(alias_name) {
                    let aliased_name = format!("{}_{}", alias_name, method);
                    let new_callee = ast::Expr::Identifier(aliased_name);
                    let mut new_args = Vec::new();
                    std::mem::swap(args, &mut new_args);
                    *expr = ast::Expr::Call {
                        callee: Box::new(ast::Node::new(new_callee, object.span.clone())),
                        args: new_args,
                    };
                    return;
                }
            }
            desugar_module_calls_in_expr(&mut object.value, aliases);
            for arg in args {
                desugar_module_calls_in_expr(&mut arg.value, aliases);
            }
        }
        ast::Expr::MemberAccess { target, field } => {
            // Check if target is an alias identifier → desugar to a simple identifier
            if let ast::Expr::Identifier(alias_name) = &target.value {
                if aliases.contains(alias_name) {
                    let aliased_name = format!("{}_{}", alias_name, field);
                    *expr = ast::Expr::Identifier(aliased_name);
                    return;
                }
            }
            desugar_module_calls_in_expr(&mut target.value, aliases);
        }
        ast::Expr::IfThenElse { condition, then_expr, else_expr } => {
            desugar_module_calls_in_expr(&mut condition.value, aliases);
            desugar_module_calls_in_expr(&mut then_expr.value, aliases);
            if let Some(e) = else_expr {
                desugar_module_calls_in_expr(&mut e.value, aliases);
            }
        }
        ast::Expr::Lambda { body, .. } => {
            desugar_module_calls_in_expr(&mut body.value, aliases);
        }
        ast::Expr::Block(block) => {
            desugar_module_calls_in_block(block, aliases);
        }
        ast::Expr::Array(items) => {
            for item in items {
                desugar_module_calls_in_expr(&mut item.value, aliases);
            }
        }
        ast::Expr::Tuple(items) => {
            for item in items {
                desugar_module_calls_in_expr(&mut item.value, aliases);
            }
        }
        ast::Expr::Record(fields) => {
            for (_, e) in fields {
                desugar_module_calls_in_expr(&mut e.value, aliases);
            }
        }
        ast::Expr::Index { target, index } => {
            desugar_module_calls_in_expr(&mut target.value, aliases);
            desugar_module_calls_in_expr(&mut index.value, aliases);
        }
        ast::Expr::Range { start, end, .. } => {
            desugar_module_calls_in_expr(&mut start.value, aliases);
            desugar_module_calls_in_expr(&mut end.value, aliases);
        }
        ast::Expr::Spread(inner) => {
            desugar_module_calls_in_expr(&mut inner.value, aliases);
        }
        ast::Expr::BcAnnotation { expr: inner, .. } => {
            desugar_module_calls_in_expr(&mut inner.value, aliases);
        }
        ast::Expr::ErrorPropagate(inner) => {
            desugar_module_calls_in_expr(&mut inner.value, aliases);
        }
        ast::Expr::MatchExpression { value, arms } => {
            desugar_module_calls_in_expr(&mut value.value, aliases);
            for arm in arms {
                desugar_module_calls_in_block(&mut arm.body, aliases);
            }
        }
    }
}

/// Try to resolve an import path relative to the source directory.
/// Supports:
///   import "./foo.ely"
///   import "./foo"
///   import "./sub/bar"
fn find_import_file(from_dir: &std::path::Path, import_path: &str) -> std::result::Result<PathBuf, String> {
    let clean = import_path.trim().trim_matches('"');
    let candidate = from_dir.join(clean);

    // Try exact path
    if candidate.exists() {
        return Ok(candidate);
    }

    // Try with .ely extension
    let with_ely = from_dir.join(format!("{}.ely", clean.trim_end_matches(".elyx")));
    if with_ely.exists() {
        return Ok(with_ely);
    }

    // Try with .elyx extension
    let with_elyx = from_dir.join(format!("{}.elyx", clean.trim_end_matches(".ely")));
    if with_elyx.exists() {
        return Ok(with_elyx);
    }

    Err(format!(
        "cannot find import `{}` from `{}` (tried: {}, {}, {})",
        clean,
        from_dir.display(),
        candidate.display(),
        with_ely.display(),
        with_elyx.display()
    ))
}

/// Compile the merged program (root + all imports) through the full pipeline.
fn compile_merged(
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
        codegen.write_to_file(out_path.to_str().unwrap())?;
        println!("Compiled to {}", out_path.display());
    }

    Ok(())
}

fn compile_file(file: &PathBuf, output: Option<PathBuf>, emit_ir: bool, debug_enabled: bool, env: &str) -> error::Result<()> {
    let file_path = file.to_string_lossy().to_string();

    let (mut source, mut program) = if has_imports(file)? {
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

fn compile_and_run(file: &PathBuf, debug_enabled: bool, emit_ir: bool, env: &str) -> error::Result<()> {
    let file_path = file.to_string_lossy().to_string();

    let (mut source, mut program) = if has_imports(file)? {
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

fn check_file(file: &PathBuf, env: &str) -> error::Result<()> {
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

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed.");
    Ok(())
}

/// Resolve environment aliases from the project's elysium.json manifest.
/// Supports built-in envs (local, dev, test, prod) and custom aliases
/// defined in the `environments` field of the manifest.
fn resolve_env_alias(file: &PathBuf, env: &str) -> String {
    // Look for elysium.json in the same directory as the source file
    let dir = file.parent().unwrap_or(Path::new("."));
    let manifest_path = dir.join("elysium.json");
    if let Ok(content) = fs::read_to_string(manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
            if let Some(envs) = manifest.get("environments") {
                if let Some(alias_map) = envs.as_object() {
                    if let Some(resolved) = alias_map.get(env) {
                        if let Some(val) = resolved.as_str() {
                            return val.to_string();
                        }
                    }
                }
            }
        }
    }
    env.to_string()
}

/// Filter out stub functions that don't match the target environment.
/// When a function is `stub: [env1, env2]`, it is only kept if target env matches one of them.
/// When a function is `stub` (no env list), it's a generic stub — kept for all envs (body is empty).
fn filter_stubs(mut prog: ast::Program, aliases: &std::collections::HashSet<String>, env: &str) -> ast::Program {
    desugar_module_calls(&mut prog, aliases);
    filter_stubs_raw(prog, env)
}

/// Desugar `console.debug("msg")` → `__console_debug("msg")` and
/// `fs.readFile("path")` → `__fs_readFile("path")` etc.
/// Also desugar `print(x)` → `__console_print(x)`.
fn desugar_builtin_calls(program: &mut ast::Program) {
    for item in &mut program.items {
        if let ast::Item::Function(f) = &mut item.value {
            desugar_builtin_in_block(&mut f.body);
        }
    }
}

fn desugar_builtin_in_block(block: &mut ast::Block) {
    for stmt in &mut block.statements {
        desugar_builtin_in_stmt(stmt);
    }
}

fn desugar_builtin_in_stmt(stmt: &mut ast::Node<ast::Stmt>) {
    match &mut stmt.value {
        ast::Stmt::Let(boxed) => {
            if let Some(val) = &mut boxed.value.value {
                desugar_builtin_in_expr(val);
            }
        }
        ast::Stmt::Expr(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value);
        }
        ast::Stmt::Return(ret) => {
            if let Some(e) = ret {
                desugar_builtin_in_expr(&mut e.value);
            }
        }
        ast::Stmt::Assign(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.target.value);
            desugar_builtin_in_expr(&mut boxed.value.value.value);
        }
        ast::Stmt::If(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.condition.value);
            desugar_builtin_in_block(&mut boxed.value.then_block);
            if let Some(eb) = &mut boxed.value.else_block {
                desugar_builtin_in_block(eb);
            }
        }
        ast::Stmt::For(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.iterable.value);
            desugar_builtin_in_block(&mut boxed.value.body);
        }
        ast::Stmt::While(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.condition.value);
            desugar_builtin_in_block(&mut boxed.value.body);
        }
        ast::Stmt::Match(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.value.value);
            for arm in &mut boxed.value.arms {
                desugar_builtin_in_block(&mut arm.body);
            }
        }
        ast::Stmt::TryCatch(boxed) => {
            desugar_builtin_in_block(&mut boxed.value.try_block);
            desugar_builtin_in_block(&mut boxed.value.catch_block);
        }
        ast::Stmt::OnlyGuard(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.condition.value);
            desugar_builtin_in_block(&mut boxed.value.body);
        }
        ast::Stmt::UnsafeBlock(boxed) => {
            desugar_builtin_in_block(&mut boxed.value.body);
        }
        ast::Stmt::BcAssert(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.condition.value);
        }
        ast::Stmt::Expect(boxed) => {
            desugar_builtin_in_expr(&mut boxed.value.expr.value);
        }
        ast::Stmt::Todo(_) | ast::Stmt::Question(_) => {}
        ast::Stmt::Bench(boxed) => {
            desugar_builtin_in_block(&mut boxed.value.body);
        }
    }
}

fn desugar_builtin_in_expr(expr: &mut ast::Expr) {
    match expr {
        ast::Expr::Literal(_) | ast::Expr::Identifier(_) => {}
        ast::Expr::BinaryOp { left, right, .. } => {
            desugar_builtin_in_expr(&mut left.value);
            desugar_builtin_in_expr(&mut right.value);
        }
        ast::Expr::UnaryOp { operand, .. } => {
            desugar_builtin_in_expr(&mut operand.value);
        }
        ast::Expr::Call { callee, args } => {
            // Check if this is a `print` call
            if let ast::Expr::Identifier(name) = &callee.value {
                if name == "print" {
                    callee.value = ast::Expr::Identifier("__console_print".to_string());
                }
            }
            desugar_builtin_in_expr(&mut callee.value);
            for arg in args {
                desugar_builtin_in_expr(&mut arg.value);
            }
        }
        ast::Expr::MethodCall { object, method, args } => {
            // Check if object is "console" → desugar to __console_<method>
            // or "fs" → desugar to __fs_<method>
            if let ast::Expr::Identifier(obj_name) = &object.value {
                let prefix = match obj_name.as_str() {
                    "console" => "__console_",
                    "fs" => "__fs_",
                    _ => "",
                };
                if !prefix.is_empty() {
                    let aliased_name = format!("{}{}", prefix, method);
                    let new_callee = ast::Expr::Identifier(aliased_name);
                    let mut new_args = Vec::new();
                    std::mem::swap(args, &mut new_args);
                    *expr = ast::Expr::Call {
                        callee: Box::new(ast::Node::new(new_callee, object.span.clone())),
                        args: new_args,
                    };
                    return;
                }
            }
            desugar_builtin_in_expr(&mut object.value);
            for arg in args {
                desugar_builtin_in_expr(&mut arg.value);
            }
        }
        ast::Expr::MemberAccess { target, field } => {
            desugar_builtin_in_expr(&mut target.value);
            let _ = field;
        }
        ast::Expr::IfThenElse { condition, then_expr, else_expr } => {
            desugar_builtin_in_expr(&mut condition.value);
            desugar_builtin_in_expr(&mut then_expr.value);
            if let Some(e) = else_expr {
                desugar_builtin_in_expr(&mut e.value);
            }
        }
        ast::Expr::Lambda { body, .. } => {
            desugar_builtin_in_expr(&mut body.value);
        }
        ast::Expr::Block(block) => {
            desugar_builtin_in_block(block);
        }
        ast::Expr::Array(items) => {
            for item in items {
                desugar_builtin_in_expr(&mut item.value);
            }
        }
        ast::Expr::Tuple(items) => {
            for item in items {
                desugar_builtin_in_expr(&mut item.value);
            }
        }
        ast::Expr::Record(fields) => {
            for (_, e) in fields {
                desugar_builtin_in_expr(&mut e.value);
            }
        }
        ast::Expr::Index { target, index } => {
            desugar_builtin_in_expr(&mut target.value);
            desugar_builtin_in_expr(&mut index.value);
        }
        ast::Expr::Range { start, end, .. } => {
            desugar_builtin_in_expr(&mut start.value);
            desugar_builtin_in_expr(&mut end.value);
        }
        ast::Expr::Spread(inner) => {
            desugar_builtin_in_expr(&mut inner.value);
        }
        ast::Expr::BcAnnotation { expr: inner, .. } => {
            desugar_builtin_in_expr(&mut inner.value);
        }
        ast::Expr::ErrorPropagate(inner) => {
            desugar_builtin_in_expr(&mut inner.value);
        }
        ast::Expr::MatchExpression { value, arms } => {
            desugar_builtin_in_expr(&mut value.value);
            for arm in arms {
                desugar_builtin_in_block(&mut arm.body);
            }
        }
    }
}

fn filter_stubs_raw(mut program: ast::Program, env: &str) -> ast::Program {
    program.items.retain(|item| {
        match &item.value {
            ast::Item::Function(f) => {
                match &f.stub_envs {
                    Some(envs) => {
                        // Empty envs list = bare stub, keep for all envs
                        // Non-empty list = only keep if target env is in the list
                        envs.is_empty() || envs.iter().any(|e| e == env)
                    }
                    None => true, // Not a stub, always keep
                }
            }
            _ => true, // Non-functions always kept
        }
    });
    program
}

/// Apply alias desugaring to a program loaded with imports.
fn desugared_program(mut prog: ast::Program, aliases: &std::collections::HashSet<String>) -> ast::Program {
    desugar_module_calls(&mut prog, aliases);
    prog
}

/// Quick check whether a file contains any `import` statements.
fn has_imports(file: &PathBuf) -> error::Result<bool> {
    let source = read_source(file)?;
    // Cheap line scan — does the file contain `import "`?
    Ok(source.contains("import \""))
}

fn build_elyx(file: &PathBuf, output: Option<PathBuf>, emit_ir: bool) -> error::Result<()> {
    let source = read_source(file)?;

    let elyx_file = elyx::parse_elyx(&source)?;
    let component = &elyx_file.component;

    let component_name = component.value.name.clone();
    println!("Parsed .elyx component: {}", component_name);

    let program = ast::Program {
        items: vec![ast::Node::new(
            ast::Item::Component(component.value.clone()),
            component.span.clone(),
        )],
    };

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed for .elyx component: {}", component_name);
    Ok(())
}

fn check_elyx(file: &PathBuf) -> error::Result<()> {
    let source = read_source(file)?;
    let elyx_file = elyx::parse_elyx(&source)?;
    let component = &elyx_file.component;

    let component_name = component.value.name.clone();

    let program = ast::Program {
        items: vec![ast::Node::new(
            ast::Item::Component(component.value.clone()),
            component.span.clone(),
        )],
    };

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed for .elyx component: {}", component_name);
    Ok(())
}

fn highlight_file(
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

fn lint_file(file: &PathBuf, format: &str) -> error::Result<()> {
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

fn doc_file(file: &PathBuf, output: &Option<PathBuf>) -> error::Result<()> {
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

fn dep_graph_file(file: &PathBuf, format: &str, output: &Option<PathBuf>) -> error::Result<()> {
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

fn gen_test_file(file: &PathBuf, output: &Option<PathBuf>) -> error::Result<()> {
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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::process::Command;

    fn assert_check_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "check", relative_path])
            .output()
            .expect("failed to run cargo check");
        assert!(
            output.status.success(),
            "check failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_highlight_ok(relative_path: &str, format: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "highlight", relative_path, "--format", format])
            .output()
            .expect("failed to run cargo highlight");
        assert!(
            output.status.success(),
            "highlight {} failed:\nstderr: {}",
            format,
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_lint_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "lint", relative_path, "--format", "text"])
            .output()
            .expect("failed to run cargo lint");
        assert!(
            output.status.success(),
            "lint failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_doc_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "doc", relative_path])
            .output()
            .expect("failed to run cargo doc");
        assert!(
            output.status.success(),
            "doc failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_dep_graph_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "dep-graph", relative_path, "--format", "dot"])
            .output()
            .expect("failed to run cargo dep-graph");
        assert!(
            output.status.success(),
            "dep-graph failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_gen_test_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "gen-test", relative_path])
            .output()
            .expect("failed to run cargo gen-test");
        assert!(
            output.status.success(),
            "gen-test failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    // All integration tests run `cargo run` recursively, so they are #[ignore] by default.
    // Run with `cargo test -- --ignored` to execute them.

    #[test]
    #[ignore]
    fn test_integration_hello_check() { assert_check_ok("examples/hello.ely"); }

    #[test]
    #[ignore]
    fn test_integration_counter_check() { assert_check_ok("examples/counter.ely"); }

    #[test]
    #[ignore]
    fn test_integration_discount_check() { assert_check_ok("examples/discount.ely"); }

    #[test]
    #[ignore]
    fn test_integration_todo_question_check() { assert_check_ok("examples/todo_question.ely"); }

    #[test]
    #[ignore]
    fn test_integration_bench_check() { assert_check_ok("examples/bench.ely"); }

    #[test]
    #[ignore]
    fn test_integration_use_math_alias_check() { assert_check_ok("examples/use_math_alias.ely"); }

    #[test]
    #[ignore]
    fn test_integration_counter_elyx_check() { assert_check_ok("examples/counter.elyx"); }

    #[test]
    #[ignore]
    fn test_integration_highlight_ansi() { assert_highlight_ok("examples/hello.ely", "ansi"); }

    #[test]
    #[ignore]
    fn test_integration_highlight_html() { assert_highlight_ok("examples/hello.ely", "html"); }

    #[test]
    #[ignore]
    fn test_integration_highlight_todo_question() { assert_highlight_ok("examples/todo_question.ely", "ansi"); }

    #[test]
    #[ignore]
    fn test_integration_highlight_bench() { assert_highlight_ok("examples/bench.ely", "ansi"); }

    #[test]
    #[ignore]
    fn test_integration_lint_hello() { assert_lint_ok("examples/hello.ely"); }

    #[test]
    #[ignore]
    fn test_integration_lint_todo_question() { assert_lint_ok("examples/todo_question.ely"); }

    #[test]
    #[ignore]
    fn test_integration_lint_bench() { assert_lint_ok("examples/bench.ely"); }

    #[test]
    #[ignore]
    fn test_integration_doc_hello() { assert_doc_ok("examples/hello.ely"); }

    #[test]
    #[ignore]
    fn test_integration_doc_discount() { assert_doc_ok("examples/discount.ely"); }

    #[test]
    #[ignore]
    fn test_integration_doc_todo_question() { assert_doc_ok("examples/todo_question.ely"); }

    #[test]
    #[ignore]
    fn test_integration_doc_bench() { assert_doc_ok("examples/bench.ely"); }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_hello() { assert_dep_graph_ok("examples/hello.ely"); }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_discount() { assert_dep_graph_ok("examples/discount.ely"); }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_bench() { assert_dep_graph_ok("examples/bench.ely"); }

    #[test]
    #[ignore]
    fn test_integration_gen_test_hello() { assert_gen_test_ok("examples/hello.ely"); }

    #[test]
    #[ignore]
    fn test_integration_gen_test_discount() { assert_gen_test_ok("examples/discount.ely"); }

    #[test]
    #[ignore]
    fn test_integration_gen_test_counter() { assert_gen_test_ok("examples/counter.ely"); }

    #[test]
    #[ignore]
    fn test_integration_spec_keywords_check() { assert_check_ok("examples/spec_keywords.ely"); }

    #[test]
    #[ignore]
    fn test_integration_import_alias_check() { assert_check_ok("examples/use_math_alias.ely"); }
}
