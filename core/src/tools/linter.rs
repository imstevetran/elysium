// Linter for Elysium 2.0
// Provides rule-based static analysis with helpful suggestions.

use crate::ast::*;
use std::collections::HashSet;

/// A lint diagnostic.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub severity: Severity,
    pub rule_id: String,
    pub message: String,
    pub offset: usize,
    pub length: usize,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

impl Severity {
    pub fn prefix(&self) -> &str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Style => "style",
        }
    }
}

/// Lint result with all diagnostics.
#[derive(Debug)]
pub struct LintResult {
    pub diagnostics: Vec<LintDiagnostic>,
}

/// Run all lint rules on source + parsed AST.
pub fn lint(source: &str, program: &Program) -> LintResult {
    let mut linter = Linter::new(source, program);
    linter.run();
    LintResult {
        diagnostics: linter.diagnostics,
    }
}

struct Linter<'a> {
    source: &'a str,
    program: &'a Program,
    diagnostics: Vec<LintDiagnostic>,
    used_names: HashSet<String>,
    declared_names: Vec<ScopeEntry>,
}

#[derive(Clone)]
struct ScopeEntry {
    name: String,
    offset: usize,
    length: usize,
}

impl<'a> Linter<'a> {
    fn new(source: &'a str, program: &'a Program) -> Self {
        Linter {
            source,
            program,
            diagnostics: Vec::new(),
            used_names: HashSet::new(),
            declared_names: Vec::new(),
        }
    }

    fn run(&mut self) {
        self.check_no_tab_indentation();
        for item in &self.program.items {
            self.check_item(item);
        }
        self.check_unused_declarations();
    }

    fn add_diag(
        &mut self,
        severity: Severity,
        rule_id: &str,
        msg: String,
        offset: usize,
        length: usize,
        help: Option<String>,
    ) {
        self.diagnostics.push(LintDiagnostic {
            severity,
            rule_id: rule_id.to_string(),
            message: msg,
            offset,
            length,
            help,
        });
    }

    /// Rule: No tab indentation.
    fn check_no_tab_indentation(&mut self) {
        for (i, ch) in self.source.char_indices() {
            if ch == '\t' {
                self.add_diag(
                    Severity::Style,
                    "no-tabs",
                    "Use spaces for indentation, not tabs.".to_string(),
                    i,
                    1,
                    Some("Configure your editor to insert spaces when pressing Tab.".to_string()),
                );
            }
        }
    }

    fn check_item(&mut self, item: &Node<Item>) {
        match &item.value {
            Item::Function(f) => self.check_function(item, f),
            Item::Class(c) => self.check_class(item, c),
            Item::Enum(e) => self.check_enum(item, e),
            Item::Component(c) => self.check_component(item, c),
            Item::TypeAlias(_) => {}
            Item::Import(..) => {}
            Item::Spec(s) => {
                for feat in &s.feats {
                    self.check_block(&feat.body);
                }
            }
            Item::Worker(_) => {}
            Item::Extension(_) => {}
        }
    }

    fn check_function(&mut self, item: &Node<Item>, f: &Function) {
        // Rule: missing-doc — all public functions should have a doc comment
        if f.doc_comment.is_none() {
            self.add_diag(
                Severity::Warning,
                "missing-doc",
                format!("Function `{}` has no documentation comment.", f.name),
                item.span.offset,
                item.span.length,
                Some(format!("Add `/// Summary: ...` above the function `{}`.", f.name)),
            );
        }

        // Rule: naming-convention — functions should be camelCase
        if !is_camel_case(&f.name) {
            self.add_diag(
                Severity::Style,
                "naming-convention",
                format!(
                    "Function `{}` should use camelCase naming (e.g., `{}`).",
                    f.name, to_camel_case(&f.name)
                ),
                item.span.offset,
                item.span.length,
                None,
            );
        }

        // For stub functions, skip body-related lints
        if f.stub_envs.is_some() {
            // Track the name
            self.declared_names.push(ScopeEntry {
                name: f.name.clone(),
                offset: item.span.offset,
                length: item.span.length,
            });
            return;
        }

        // Rule: return-type — functions with bodies > 1 line should have explicit return types
        if f.body.statements.len() > 1 && f.return_type.is_none() {
            self.add_diag(
                Severity::Info,
                "explicit-return-type",
                format!("Function `{}` should declare an explicit return type for clarity.", f.name),
                item.span.offset,
                item.span.length,
                Some("Add `-> ReturnType` after the parameter list.".to_string()),
            );
        }

        // Rule: check bc annotation
        if f.bc_reason.is_none() && f.body.statements.len() > 3 {
            self.add_diag(
                Severity::Info,
                "bc-annotation",
                format!(
                    "Consider adding a `bc` annotation to explain the purpose of `{}`.",
                    f.name
                ),
                item.span.offset,
                item.span.length,
                Some("Add `bc \"explanation\"` after the return type.".to_string()),
            );
        }

        // Track declared names
        self.declared_names.push(ScopeEntry {
            name: f.name.clone(),
            offset: item.span.offset,
            length: item.span.length,
        });

        self.check_block(&f.body);
    }

    fn check_class(&mut self, item: &Node<Item>, c: &Class) {
        // Rule: naming-convention — classes should be PascalCase
        if !is_pascal_case(&c.name) {
            self.add_diag(
                Severity::Style,
                "naming-convention",
                format!(
                    "Class `{}` should use PascalCase naming (e.g., `{}`).",
                    c.name, to_pascal_case(&c.name)
                ),
                item.span.offset,
                item.span.length,
                None,
            );
        }

        // Rule: missing-doc
        if c.doc_comment.is_none() {
            self.add_diag(
                Severity::Warning,
                "missing-doc",
                format!("Class `{}` has no documentation comment.", c.name),
                item.span.offset,
                item.span.length,
                Some(format!("Add `/// Summary: ...` above `class {}`.", c.name)),
            );
        }

        // Check init method exists
        let has_init = c.methods.iter().any(|m| m.name == "init");
        if c.fields.iter().any(|f| f.name != "this") && !has_init {
            self.add_diag(
                Severity::Info,
                "missing-init",
                format!("Class `{}` has fields but no `init` method.", c.name),
                item.span.offset,
                item.span.length,
                Some("Add an `init` method to initialize field values.".to_string()),
            );
        }

        for method in &c.methods {
            self.check_block(&method.body);
        }
    }

    fn check_enum(&mut self, item: &Node<Item>, e: &Enum) {
        if !is_pascal_case(&e.name) {
            self.add_diag(
                Severity::Style,
                "naming-convention",
                format!("Enum `{}` should use PascalCase naming.", e.name),
                item.span.offset,
                item.span.length,
                None,
            );
        }

        if e.doc_comment.is_none() {
            self.add_diag(
                Severity::Warning,
                "missing-doc",
                format!("Enum `{}` has no documentation comment.", e.name),
                item.span.offset,
                item.span.length,
                Some(format!("Add `/// Summary: ...` above `enum {}`.", e.name)),
            );
        }

        // Check variant naming
        for variant in &e.variants {
            if !is_pascal_case(&variant.name) {
                self.add_diag(
                    Severity::Style,
                    "naming-convention",
                    format!("Enum variant `{}` should use PascalCase naming.", variant.name),
                    item.span.offset,
                    item.span.length,
                    None,
                );
            }
        }
    }

    fn check_component(&mut self, item: &Node<Item>, c: &Component) {
        if !is_pascal_case(&c.name) {
            self.add_diag(
                Severity::Style,
                "naming-convention",
                format!("Component `{}` should use PascalCase naming.", c.name),
                item.span.offset,
                item.span.length,
                None,
            );
        }

        self.check_block(&c.body);
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Node<Stmt>) {
        match &stmt.value {
            Stmt::Let(boxed) => {
                let ls = &boxed.value;
                // Rule: naming-convention for variables
                if !is_camel_case(&ls.name) {
                    self.add_diag(
                        Severity::Style,
                        "naming-convention",
                        format!(
                            "Variable `{}` should use camelCase naming (e.g., `{}`).",
                            ls.name,
                            to_camel_case(&ls.name)
                        ),
                        stmt.span.offset,
                        stmt.span.length,
                        None,
                    );
                }

                // Rule: let-shadow — flag shadowing
                if self.declared_names.iter().any(|d| d.name == ls.name) {
                    self.add_diag(
                        Severity::Warning,
                        "variable-shadow",
                        format!("Variable `{}` shadows a previously declared name.", ls.name),
                        stmt.span.offset,
                        stmt.span.length,
                        Some("Consider renaming to avoid confusion.".to_string()),
                    );
                }

                self.declared_names.push(ScopeEntry {
                    name: ls.name.clone(),
                    offset: stmt.span.offset,
                    length: stmt.span.length,
                });

                if let Some(val) = &ls.value {
                    self.check_expr(val, stmt.span.offset);
                }
            }
            Stmt::Assign(boxed) => {
                let a = &boxed.value;
                self.check_expr(&a.target.value, stmt.span.offset);
                self.check_expr(&a.value.value, stmt.span.offset);
            }
            Stmt::Expr(boxed) => {
                self.check_expr(&boxed.value, stmt.span.offset);
            }
            Stmt::Return(ret) => {
                if let Some(boxed) = ret {
                    self.check_expr(&boxed.value, stmt.span.offset);
                }
            }
            Stmt::If(boxed) => {
                let ifs = &boxed.value;
                self.check_expr(&ifs.condition.value, stmt.span.offset);
                self.check_block(&ifs.then_block);
                if let Some(eb) = &ifs.else_block {
                    self.check_block(eb);
                }
            }
            Stmt::For(boxed) => {
                let fs = &boxed.value;
                self.declared_names.push(ScopeEntry {
                    name: fs.variable.clone(),
                    offset: stmt.span.offset,
                    length: stmt.span.length,
                });
                self.check_expr(&fs.iterable.value, stmt.span.offset);
                self.check_block(&fs.body);
            }
            Stmt::While(boxed) => {
                let ws = &boxed.value;
                self.check_expr(&ws.condition.value, stmt.span.offset);
                self.check_block(&ws.body);
            }
            Stmt::Match(boxed) => {
                let ms = &boxed.value;
                self.check_expr(&ms.value.value, stmt.span.offset);
                for arm in &ms.arms {
                    self.check_block(&arm.body);
                }
            }
            Stmt::TryCatch(boxed) => {
                let tc = &boxed.value;
                self.check_block(&tc.try_block);
                self.check_block(&tc.catch_block);
                if let Some(fb) = &tc.finally_block {
                    self.check_block(fb);
                }
            }
            Stmt::BcAssert(boxed) => {
                let ba = &boxed.value;
                self.check_expr(&ba.condition.value, stmt.span.offset);
            }
            Stmt::OnlyGuard(boxed) => {
                let og = &boxed.value;
                self.check_expr(&og.condition.value, stmt.span.offset);
                self.check_block(&og.body);
            }
            Stmt::UnsafeBlock(boxed) => {
                let ub = &boxed.value;
                self.check_block(&ub.body);
            }
            Stmt::Expect(boxed) => {
                self.check_expr(&boxed.value.expr.value, stmt.span.offset);
            }
            Stmt::Todo(_) | Stmt::Question(_) => {},
            Stmt::Wait(_) => {},
            Stmt::Bench(boxed) => {
                self.check_block(&boxed.value.body);
            }
            Stmt::Parallel(boxed) => {
                for item in &boxed.value.items {
                    self.check_stmt(item);
                }
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, parent_offset: usize) {
        match expr {
            Expr::Identifier(name) => {
                self.used_names.insert(name.clone());
            }
            Expr::BinaryOp { left, right, .. } => {
                self.check_expr(&left.value, parent_offset);
                self.check_expr(&right.value, parent_offset);
            }
            Expr::UnaryOp { operand, .. } => {
                self.check_expr(&operand.value, parent_offset);
            }
            Expr::Call { callee, args } => {
                self.check_expr(&callee.value, parent_offset);
                for arg in args {
                    self.check_expr(&arg.value, parent_offset);
                }
            }
            Expr::MethodCall { object, args, .. } => {
                self.check_expr(&object.value, parent_offset);
                for arg in args {
                    self.check_expr(&arg.value, parent_offset);
                }
            }
            Expr::IfThenElse { condition, then_expr, else_expr } => {
                self.check_expr(&condition.value, parent_offset);
                self.check_expr(&then_expr.value, parent_offset);
                if let Some(ee) = else_expr {
                    self.check_expr(&ee.value, parent_offset);
                }
            }
            Expr::Lambda { body, .. } => {
                self.check_expr(&body.value, parent_offset);
            }
            Expr::Block(block) => {
                self.check_block(block);
            }
            Expr::Array(items) | Expr::Tuple(items) => {
                for item in items {
                    self.check_expr(&item.value, parent_offset);
                }
            }
            Expr::Index { target, index } => {
                self.check_expr(&target.value, parent_offset);
                self.check_expr(&index.value, parent_offset);
            }
            Expr::MemberAccess { target, .. } => {
                self.check_expr(&target.value, parent_offset);
            }
            Expr::Range { start, end, .. } => {
                self.check_expr(&start.value, parent_offset);
                self.check_expr(&end.value, parent_offset);
            }
            Expr::Spread(operand) => {
                self.check_expr(&operand.value, parent_offset);
            }
            Expr::BcAnnotation { expr, .. } => {
                self.check_expr(&expr.value, parent_offset);
            }
            Expr::ErrorPropagate(operand) => {
                self.check_expr(&operand.value, parent_offset);
            }
            Expr::Await(inner) => {
                self.check_expr(&inner.value, parent_offset);
            }
            Expr::MatchExpression { value, arms } => {
                self.check_expr(&value.value, parent_offset);
                for arm in arms {
                    self.check_block(&arm.body);
                }
            }
            Expr::Is { value, .. } => {
                self.check_expr(&value.value, parent_offset);
            }
            Expr::Literal(_) | Expr::Record(_) => {}
        }
    }

    fn check_unused_declarations(&mut self) {
        let unused: Vec<_> = self
            .declared_names
            .iter()
            .filter(|entry| !self.used_names.contains(&entry.name))
            .cloned()
            .collect();
        for entry in unused {
            self.add_diag(
                Severity::Warning,
                "unused-variable",
                format!("Unused declaration `{}`.", entry.name),
                entry.offset,
                entry.length,
                Some(format!("Remove `{}` or prefix with `_` to silence this warning.", entry.name)),
            );
        }
    }
}

// ---- naming helpers ----

fn is_camel_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let mut chars = s.chars();
    chars.next().unwrap().is_lowercase() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn is_pascal_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let mut chars = s.chars();
    chars.next().unwrap().is_uppercase() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn to_camel_case(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' || c == '-' {
            capitalize_next = true;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' || c == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_src(src: &str) -> LintResult {
        let mut p = crate::parser::Parser::new(src);
        let program = p.parse_program().expect("parse failed");
        lint(src, &program)
    }

    fn assert_no_lint_errors(src: &str) {
        let result = lint_src(src);
        let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "expected no lint errors, got: {:?}", errors);
    }

    #[test]
    fn test_lint_spec() {
        assert_no_lint_errors(r##"spec "math" { feat "add" { expect 1 + 1 } }"##);
    }

    #[test]
    fn test_lint_describe() {
        assert_no_lint_errors(r##"describe "suite" { it "pass" { expect true } }"##);
    }

    #[test]
    fn test_lint_todo() {
        assert_no_lint_errors("/// f\nfunc f() { todo }");
    }

    #[test]
    fn test_lint_question() {
        assert_no_lint_errors("/// f\nfunc f() { question }");
    }

    #[test]
    fn test_lint_bench() {
        assert_no_lint_errors("/// f\nfunc f() { bench { let x = 1 } }");
    }

    #[test]
    fn test_lint_bm() {
        assert_no_lint_errors("/// f\nfunc f() { bm { let x = 42 } }");
    }

    #[test]
    fn test_lint_expect() {
        assert_no_lint_errors("/// f\nfunc f() { expect 1 + 2 }");
    }
}
