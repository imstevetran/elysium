// Code-generation tools that operate on .ely source files without any LLM.
//
// 1. **doc**   — extracts doc comments + signatures → Markdown docs
// 2. **dep-graph** — parses imports and calls → DOT / JSON
// 3. **gen-test** — generates test stubs from every function

use crate::ast::*;
use crate::error::Result;
use crate::parser::Parser;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;

// ============================================================================
// Helpers: doc-comment extraction from raw source
// ============================================================================

fn extract_doc_comment(source: &str, before_offset: usize) -> String {
    let prefix = &source[..before_offset];
    let lines: Vec<&str> = prefix.lines().collect();

    let mut doc_lines: Vec<&str> = Vec::new();
    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if let Some(body) = trimmed.strip_prefix("///") {
            doc_lines.push(body.trim());
        } else if doc_lines.is_empty() && (trimmed.is_empty() || trimmed.starts_with("//")) {
            continue;
        } else if !doc_lines.is_empty() {
            break;
        }
    }
    doc_lines.reverse();
    doc_lines.join("\n")
}

fn expr_call_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.clone()),
        Expr::MemberAccess { target, field } => {
            let target_name = expr_call_name(&target.value);
            target_name.map(|t| format!("{}.{}", t, field))
        }
        _ => None,
    }
}

fn collect_calls_in_block(block: &Block, calls: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_calls_in_stmt(&stmt.value, calls);
    }
}

fn collect_calls_in_expr(expr: &Expr, calls: &mut HashSet<String>) {
    match expr {
        Expr::Call { callee, args } => {
            if let Some(name) = expr_call_name(&callee.value) {
                calls.insert(name);
            }
            collect_calls_in_expr(&callee.value, calls);
            for arg in args {
                collect_calls_in_expr(&arg.value, calls);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_calls_in_expr(&object.value, calls);
            for arg in args {
                collect_calls_in_expr(&arg.value, calls);
            }
        }
        Expr::MemberAccess { target, field: _ } => {
            collect_calls_in_expr(&target.value, calls);
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_calls_in_expr(&left.value, calls);
            collect_calls_in_expr(&right.value, calls);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_calls_in_expr(&operand.value, calls);
        }
        Expr::IfThenElse { condition, then_expr, else_expr } => {
            collect_calls_in_expr(&condition.value, calls);
            collect_calls_in_expr(&then_expr.value, calls);
            if let Some(e) = else_expr {
                collect_calls_in_expr(&e.value, calls);
            }
        }
        Expr::Lambda { body, .. } => {
            collect_calls_in_expr(&body.value, calls);
        }
        Expr::Block(block) => {
            for stmt in &block.statements {
                collect_calls_in_stmt(&stmt.value, calls);
            }
        }
        Expr::Array(items) => {
            for item in items { collect_calls_in_expr(&item.value, calls); }
        }
        Expr::Tuple(items) => {
            for item in items { collect_calls_in_expr(&item.value, calls); }
        }
        Expr::Index { target, index } => {
            collect_calls_in_expr(&target.value, calls);
            collect_calls_in_expr(&index.value, calls);
        }
        Expr::Range { start, end, inclusive: _ } => {
            collect_calls_in_expr(&start.value, calls);
            collect_calls_in_expr(&end.value, calls);
        }
        Expr::Spread(inner) => collect_calls_in_expr(&inner.value, calls),
        Expr::BcAnnotation { expr, .. } => collect_calls_in_expr(&expr.value, calls),
        Expr::ErrorPropagate(inner) => collect_calls_in_expr(&inner.value, calls),
        Expr::Await(inner) => collect_calls_in_expr(&inner.value, calls),
        Expr::MatchExpression { value, arms } => {
            collect_calls_in_expr(&value.value, calls);
            for arm in arms { collect_calls_in_block(&arm.body, calls); }
        }
        Expr::Record(fields) => {
            for (_, expr) in fields { collect_calls_in_expr(&expr.value, calls); }
        }
        _ => {}
    }
}

fn collect_calls_in_stmt(stmt: &Stmt, calls: &mut HashSet<String>) {
    match stmt {
        Stmt::Expr(expr) => collect_calls_in_expr(&expr.value, calls),
        Stmt::Let(let_stmt) => {
            if let Some(val) = &let_stmt.value.value {
                collect_calls_in_expr(val, calls);
            }
        }
        Stmt::Assign(assign) => {
            collect_calls_in_expr(&assign.value.target.value, calls);
            collect_calls_in_expr(&assign.value.value.value, calls);
        }
        Stmt::Return(ret) => {
            if let Some(expr) = ret {
                collect_calls_in_expr(&expr.value, calls);
            }
        }
        Stmt::If(ifs) => {
            collect_calls_in_expr(&ifs.value.condition.value, calls);
            collect_calls_in_block(&ifs.value.then_block, calls);
            if let Some(eb) = &ifs.value.else_block {
                collect_calls_in_block(eb, calls);
            }
        }
        Stmt::For(fs) => {
            collect_calls_in_expr(&fs.value.iterable.value, calls);
            collect_calls_in_block(&fs.value.body, calls);
        }
        Stmt::While(ws) => {
            collect_calls_in_expr(&ws.value.condition.value, calls);
            collect_calls_in_block(&ws.value.body, calls);
        }
        Stmt::Match(ms) => {
            collect_calls_in_expr(&ms.value.value.value, calls);
            for arm in &ms.value.arms {
                collect_calls_in_block(&arm.body, calls);
            }
        }
        Stmt::TryCatch(tc) => {
            collect_calls_in_block(&tc.value.try_block, calls);
            collect_calls_in_block(&tc.value.catch_block, calls);
            if let Some(fb) = &tc.value.finally_block {
                collect_calls_in_block(fb, calls);
            }
        }
        Stmt::OnlyGuard(og) => {
            collect_calls_in_expr(&og.value.condition.value, calls);
            collect_calls_in_block(&og.value.body, calls);
        }
        Stmt::UnsafeBlock(ub) => {
            collect_calls_in_block(&ub.value.body, calls);
        }
        Stmt::BcAssert(ba) => {
            collect_calls_in_expr(&ba.value.condition.value, calls);
        }
        Stmt::Expect(expect) => {
            collect_calls_in_expr(&expect.value.expr.value, calls);
        }
        Stmt::Todo(_) | Stmt::Question(_) | Stmt::Wait(_) => {},
        Stmt::Bench(boxed) => {
            collect_calls_in_block(&boxed.value.body, calls);
        }
        Stmt::Parallel(boxed) => {
            for item in &boxed.value.items {
                collect_calls_in_stmt(&item.value, calls);
            }
        }
    }
}

// ============================================================================
// 1. DOCUMENTATION GENERATOR
// ============================================================================

pub fn generate_doc(source: &str, program: &Program) -> String {
    let mut out = String::new();

    out.push_str("# API Documentation\n\n");
    out.push_str(&format!("Generated from {} items.\n\n", program.items.len()));

    for item in &program.items {
        let doc = extract_doc_comment(source, item.span.offset);
        match &item.value {
            Item::Function(f) => {
                doc_fn(&mut out, source, f, &doc);
            }
            Item::Class(c) => {
                out.push_str(&format!("## `class {}`\n\n", c.name));
                if !doc.is_empty() {
                    out.push_str(&doc);
                    out.push_str("\n\n");
                }
                if !c.fields.is_empty() {
                    out.push_str("### Fields\n\n| Field | Type | Mutable |\n|-------|------|---------|\n");
                    for fld in &c.fields {
                        write!(out, "| `{}` | ", fld.name).ok();
                        if let Some(ty) = &fld.type_ann {
                            push_type(ty, &mut out);
                        } else {
                            out.push_str("_inferred_");
                        }
                        writeln!(out, " | {} |", if fld.is_mutable { "yes" } else { "no" }).ok();
                    }
                    out.push('\n');
                }
                if !c.methods.is_empty() {
                    out.push_str("### Methods\n\n");
                    for m in &c.methods {
                        doc_fn(&mut out, source, m, &extract_doc_comment(source, 0));
                    }
                }
            }
            Item::Enum(e) => {
                out.push_str(&format!("## `enum {}`\n\n", e.name));
                if !doc.is_empty() {
                    out.push_str(&doc);
                    out.push_str("\n\n");
                }
                out.push_str("| Variant |\n|---------|\n");
                for v in &e.variants {
                    writeln!(out, "| `{}` |", v.name).ok();
                }
                out.push('\n');
            }
            Item::Component(c) => {
                out.push_str(&format!("## `component {}`\n\n", c.name));
                if !doc.is_empty() {
                    out.push_str(&doc);
                    out.push_str("\n\n");
                }
                out.push_str("UI component.\n\n");
            }
            Item::TypeAlias(ta) => {
                out.push_str(&format!("## `typealias {}`\n\n", ta.name));
                out.push_str("```elysium\ntypealias ");
                out.push_str(&ta.name);
                out.push_str(" = ");
                push_type(&ta.type_expr, &mut out);
                out.push_str("\n```\n\n");
            }
            Item::Import(path, _alias) => {
                writeln!(out, "### Import `{}`\n", path).ok();
            }
            Item::Spec(s) => {
                writeln!(out, "### Spec `{}`\n", s.name).ok();
                for feat in &s.feats {
                    writeln!(out, "- feat `{}`", feat.name).ok();
                }
                out.push_str("\n");
            }
            Item::Worker(w) => {
                writeln!(out, "### Worker `{}`\n", w.name).ok();
            }
        }
    }

    out
}

fn doc_fn(out: &mut String, source: &str, f: &Function, doc: &str) {
    out.push_str(&format!("## `func {}`\n\n", f.name));
    if !doc.is_empty() {
        out.push_str(doc);
        out.push_str("\n\n");
    }
    out.push_str("**Signature:**\n\n```elysium\nfunc ");
    out.push_str(&f.name);
    out.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push_str(&p.name);
        if let Some(ty) = &p.type_ann {
            out.push_str(": ");
            push_type(ty, out);
        }
    }
    out.push(')');
    if let Some(ret) = &f.return_type {
        out.push_str(" -> ");
        push_type(ret, out);
    }
    out.push_str("\n```\n\n");

    if !f.params.is_empty() {
        out.push_str("| Parameter | Type |\n|-----------|------|\n");
        for p in &f.params {
            write!(out, "| `{}` | ", p.name).ok();
            if let Some(ty) = &p.type_ann {
                push_type(ty, out);
            } else {
                out.push_str("_inferred_");
            }
            out.push_str(" |\n");
        }
        out.push('\n');
    }
}

fn push_type(ty: &TypeExpr, out: &mut String) {
    match ty {
        TypeExpr::Named(name) => out.push_str(name),
        TypeExpr::Generic(base, params) => {
            out.push_str(base);
            out.push('<');
            for (i, p) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                push_type(p, out);
            }
            out.push('>');
        }
        TypeExpr::Array(inner) => {
            out.push('[');
            push_type(inner, out);
            out.push(']');
        }
        TypeExpr::Option(inner) => {
            out.push_str("Option<");
            push_type(inner, out);
            out.push('>');
        }
        TypeExpr::Result(ok, err) => {
            out.push_str("Result<");
            push_type(ok, out);
            out.push_str(", ");
            push_type(err, out);
            out.push('>');
        }
        TypeExpr::Union(types) => {
            for (i, t) in types.iter().enumerate() {
                if i > 0 { out.push_str(" | "); }
                push_type(t, out);
            }
        }
        TypeExpr::Function(params, ret) => {
            out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                push_type(p, out);
            }
            out.push_str(") -> ");
            push_type(ret, out);
        }
        TypeExpr::Tuple(ts) => {
            out.push('(');
            for (i, t) in ts.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                push_type(t, out);
            }
            out.push(')');
        }
        TypeExpr::Record(fields) => {
            out.push_str("{ ");
            for (i, (name, ty)) in fields.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(name);
                out.push_str(": ");
                push_type(ty, out);
            }
            out.push_str(" }");
        }
        TypeExpr::Infer => out.push_str("_"),
    }
}

// ============================================================================
// 2. DEPENDENCY GRAPH GENERATOR
// ============================================================================

pub struct DepGraph {
    pub imports: Vec<String>,
    pub calls: Vec<(String, String)>,
}

pub fn analyse_deps(program: &Program) -> DepGraph {
    let mut imports = Vec::new();
    let mut calls = Vec::new();

    for item in &program.items {
        match &item.value {
            Item::Import(path, _alias) => {
                imports.push(path.clone());
            }
            Item::Function(f) => {
                let caller = f.name.clone();
                let mut callees = HashSet::new();
                collect_calls_in_block(&f.body, &mut callees);
                for callee in callees {
                    calls.push((caller.clone(), callee));
                }
            }
            _ => {}
        }
    }

    DepGraph { imports, calls }
}

pub fn render_dot(source: &str, program: &Program) -> String {
    let deps = analyse_deps(program);
    let mut out = String::new();

    let label = Path::new(source)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "source".to_string());

    out.push_str("digraph G {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, style=rounded];\n");
    out.push_str(&format!("  label=\"{}\";\n", escape_dot_label(&label)));
    out.push_str("  fontsize=14;\n\n");

    let mut functions: HashSet<String> = HashSet::new();
    for item in &program.items {
        match &item.value {
            Item::Function(f) => { functions.insert(f.name.clone()); }
            Item::Import(path, _alias) => {
                let node = format!("import:{}", path);
                out.push_str(&format!("  \"{}\" [shape=folder, style=filled, fillcolor=lightyellow];\n", escape_dot(&node)));
            }
            _ => {}
        }
    }

    for fn_name in &functions {
        out.push_str(&format!("  \"{}\" [shape=box];\n", escape_dot(fn_name)));
    }

    for (caller, callee) in &deps.calls {
        out.push_str(&format!("  \"{}\" -> \"{}\";\n", escape_dot(caller), escape_dot(callee)));
    }

    for imp in &deps.imports {
        if functions.contains(imp) {
            out.push_str(&format!("  \"{}\" -> \"{}\" [style=dashed, label=\"import\"];\n", escape_dot(imp), escape_dot(imp)));
        }
    }

    out.push_str("}\n");
    out
}

pub fn render_json(source: &str, program: &Program) -> String {
    let deps = analyse_deps(program);
    let mut out = String::new();

    let file_name = Path::new(source)
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or(std::borrow::Cow::Borrowed("source"));

    out.push_str("{\n");
    out.push_str(&format!("  \"file\": \"{}\",\n", json_escape(&file_name)));
    out.push_str("  \"imports\": [\n");
    for (i, imp) in deps.imports.iter().enumerate() {
        if i > 0 { out.push_str(",\n"); }
        out.push_str(&format!("    \"{}\"", json_escape(imp)));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"calls\": [\n");
    for (i, (caller, callee)) in deps.calls.iter().enumerate() {
        if i > 0 { out.push_str(",\n"); }
        out.push_str(&format!(
            "    {{ \"caller\": \"{}\", \"callee\": \"{}\" }}",
            json_escape(caller),
            json_escape(callee)
        ));
    }
    out.push_str("\n  ]\n}\n");
    out
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn escape_dot_label(s: &str) -> String {
    s.replace('"', "\\\"")
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ============================================================================
// 3. TEST TEMPLATE GENERATOR
// ============================================================================

pub fn generate_tests(file_path: &str, source: &str, program: &Program) -> String {
    let mut out = String::new();

    let file_name = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or(std::borrow::Cow::Borrowed("source"));

    out.push_str(&format!("// Auto-generated tests for {}\n", file_name));
    out.push_str("// Run with: elysium test\n\n");

    if program.items.is_empty() {
        out.push_str("// No items found — nothing to test.\n");
        return out;
    }

    for item in &program.items {
        match &item.value {
            Item::Function(f) => {
                let doc = extract_doc_comment(source, item.span.offset);
                if !doc.is_empty() {
                    for line in doc.lines() {
                        writeln!(out, "/// {}", line).ok();
                    }
                }
                writeln!(out, "func test_{}() -> Bool {{", f.name).ok();

                let mut args = Vec::new();
                for param in &f.params {
                    args.push(default_value_string(&param.type_ann));
                }

                writeln!(out, "    let result = {}({})", f.name, args.join(", ")).ok();
                if f.return_type.is_some() {
                    writeln!(out, "    result == {} // TODO: fill in expected value", default_value_string(&f.return_type)).ok();
                } else {
                    writeln!(out, "    true // TODO: add assertions").ok();
                }
                writeln!(out, "}}\n").ok();
            }
            Item::Class(c) => {
                let doc = extract_doc_comment(source, item.span.offset);
                if !doc.is_empty() {
                    for line in doc.lines() {
                        writeln!(out, "/// {}", line).ok();
                    }
                }
                writeln!(out, "func test_{}() -> Bool {{", c.name).ok();
                writeln!(out, "    // TODO: instantiate and test {} class", c.name).ok();
                writeln!(out, "    true\n}}\n").ok();
            }
            Item::Enum(e) => {
                let doc = extract_doc_comment(source, item.span.offset);
                if !doc.is_empty() {
                    for line in doc.lines() {
                        writeln!(out, "/// {}", line).ok();
                    }
                }
                writeln!(out, "func test_{}() -> Bool {{", e.name).ok();
                writeln!(out, "    // TODO: test {} enum variants", e.name).ok();
                writeln!(out, "    true\n}}\n").ok();
            }
            _ => {}
        }
    }

    out
}

fn default_value_string(ty: &Option<TypeExpr>) -> String {
    match ty {
        Some(TypeExpr::Named(n)) => match n.as_str() {
            "Int" | "int" => "0".to_string(),
            "Float" | "float" => "0.0".to_string(),
            "Bool" | "bool" => "false".to_string(),
            "String" | "string" => "\"\"".to_string(),
            "Char" | "char" => "'\\0'".to_string(),
            _ => format!("/* {} */ nil", n),
        },
        Some(TypeExpr::Option(_)) => "nil".to_string(),
        Some(TypeExpr::Array(_)) => "[]".to_string(),
        Some(TypeExpr::Tuple(_)) => "()".to_string(),
        _ => "nil".to_string(),
    }
}

// ============================================================================
// Convenience: parse source once
// ============================================================================

pub fn parse_source(source: &str) -> Result<Program> {
    let mut parser = Parser::new(source);
    parser.parse_program()
}
