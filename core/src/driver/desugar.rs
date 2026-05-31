//! AST desugaring: module calls and builtin method sugar.

use crate::ast;

pub fn desugar_module_calls(program: &mut ast::Program, aliases: &std::collections::HashSet<String>) {
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
        ast::Stmt::Wait(_) => {},
        ast::Stmt::Bench(boxed) => {
            desugar_module_calls_in_block(&mut boxed.value.body, aliases);
        }
        ast::Stmt::Parallel(boxed) => {
            for item in &mut boxed.value.items {
                desugar_module_calls_in_stmt(item, aliases);
            }
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
        ast::Expr::Await(inner) => {
            desugar_module_calls_in_expr(&mut inner.value, aliases);
        }
        ast::Expr::MatchExpression { value, arms } => {
            desugar_module_calls_in_expr(&mut value.value, aliases);
            for arm in arms {
                desugar_module_calls_in_block(&mut arm.body, aliases);
            }
        }
        ast::Expr::Is { value, .. } => {
            desugar_module_calls_in_expr(&mut value.value, aliases);
        }
    }
}

/// Walk up from `dir` to find the project root (directory containing elysium.json).

/// Desugar `console.debug("msg")` → `__console_debug("msg")` and
/// `fs.readFile("path")` → `__fs_readFile("path")` etc.
/// Also desugar `print(x)` → `__console_print(x)`.
pub fn desugar_builtin_calls(program: &mut ast::Program) {
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
        ast::Stmt::Wait(_) => {}
        ast::Stmt::Bench(boxed) => {
            desugar_builtin_in_block(&mut boxed.value.body);
        }
        ast::Stmt::Parallel(boxed) => {
            for item in &mut boxed.value.items {
                desugar_builtin_in_stmt(item);
            }
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
                "transport" => "__transport_",
                "string" => "__string_",
                "regex" => "__regex_",
                "datetime" => "__datetime_",
                "auth" => "__auth_",
                "langchain" => "__langchain_",
                "langgraph" => "__langgraph_",
                "dict" => "__dict_",
                "json" => "__json_",
                "math" => "__math_",
                "env" => "__env_",
                "http" => "__http_",
                "worker" => "__worker_",
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
            // Check if this is a string-literal method call: "hello".length() → __string_length("hello")
            // Also handles identifier receivers: x.length() → __string_length(x)
            if is_string_method(method) {
                let aliased_name = format!("__string_{}", method);
                let new_callee = ast::Expr::Identifier(aliased_name);
                // Prepend the receiver object as the first argument (clone since we can't move out of &mut)
                let mut new_args: Vec<ast::Node<ast::Expr>> = Vec::new();
                new_args.push(ast::Node::new(object.value.clone(), object.span.clone()));
                std::mem::swap(args, &mut new_args);
                *expr = ast::Expr::Call {
                    callee: Box::new(ast::Node::new(new_callee, object.span.clone())),
                    args: new_args,
                };
                return;
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
            // First desugar all values in the record
            for (_, e) in fields.iter_mut() {
                desugar_builtin_in_expr(&mut e.value);
            }
            // Then transform Record {"k1": v1, "k2": v2} into json.buildObject("k1", v1, "k2", v2)
            let mut new_args = Vec::new();
            for (key, val) in fields.iter() {
                new_args.push(ast::Node::new(
                    ast::Expr::Literal(ast::Node::new(ast::Literal::String(key.clone()), crate::error::SourceSpan::new(0, 0))),
                    crate::error::SourceSpan::new(0, 0),
                ));
                new_args.push(ast::Node::new(
                    val.value.clone(),
                    crate::error::SourceSpan::new(0, 0),
                ));
            }
            let mut old_args = Vec::new();
            std::mem::swap(&mut old_args, &mut new_args);
            *expr = ast::Expr::Call {
                callee: Box::new(ast::Node::new(
                    ast::Expr::Identifier("__json_buildObject".into()),
                    crate::error::SourceSpan::new(0, 0),
                )),
                args: old_args,
            };
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
        ast::Expr::Await(inner) => {
            desugar_builtin_in_expr(&mut inner.value);
        }
        ast::Expr::Is { value, type_name } => {
            desugar_builtin_in_expr(&mut value.value);
            let type_name_lit = ast::Expr::Literal(ast::Node::new(
                ast::Literal::String(type_name.clone()),
                crate::error::SourceSpan::new(0, 0),
            ));
            *expr = ast::Expr::Call {
                callee: Box::new(ast::Node::new(
                    ast::Expr::Identifier("__is_instanceof".into()),
                    crate::error::SourceSpan::new(0, 0),
                )),
                args: vec![
                    ast::Node::new(value.value.clone(), crate::error::SourceSpan::new(0, 0)),
                    ast::Node::new(type_name_lit, crate::error::SourceSpan::new(0, 0)),
                ],
            };
        }
        ast::Expr::MatchExpression { value, arms } => {
            desugar_builtin_in_expr(&mut value.value);
            for arm in arms {
                desugar_builtin_in_block(&mut arm.body);
            }
        }
    }
}

/// Returns true if the method name is a recognized string method.
fn is_string_method(method: &str) -> bool {
    matches!(
        method,
        "length" | "isEmpty" | "startsWith" | "endsWith" | "contains"
            | "toUpper" | "toLower" | "trim" | "trimStart" | "trimEnd"
            | "charAt" | "charCodeAt" | "indexOf" | "lastIndexOf"
            | "slice" | "substring" | "replace" | "split"
            | "padStart" | "padEnd" | "repeat" | "concat"
            | "includes" | "search" | "match" | "toString"
            // crypto methods
            | "sha256" | "md5" | "base64Encode" | "base64Decode"
            | "hexEncode" | "hexDecode" | "hmac"
    )
}

