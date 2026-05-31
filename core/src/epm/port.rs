/// Elysium Port — TypeScript / JavaScript to Elysium translator.
///
/// Converts `.ts` and `.js` files into equivalent `.ely` source code.
/// Uses a simple token-aware line translator that handles the most common
/// patterns. For v1, this covers:
///
/// - Function declarations & arrow functions
/// - Variable declarations (const, let, var)
/// - Import/export statements
/// - Class declarations
/// - TypeScript type annotations (removes them or maps to Elysium types)
/// - Template literals
/// - Common operators (=== → ==, !== → !=, null/undefined → nil)
///
/// Usage:
///   elysium port input.ts         # prints to stdout
///   elysium port input.js -o output.ely
use crate::error::{CompileError, Result};
use std::path::Path;

/// Determine the language from the file extension.
pub fn detect_lang(file: &Path, lang_override: &Option<String>) -> &'static str {
    if let Some(lang) = lang_override {
        return match lang.as_str() {
            "ts" | "typescript" => "typescript",
            "js" | "javascript" => "javascript",
            _ => "typescript", // default
        };
    }
    match file.extension().and_then(|e| e.to_str()) {
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("mjs") => "javascript",
        Some("cjs") => "javascript",
        _ => "typescript", // default
    }
}

/// Port a TS/JS source string to Elysium.
pub fn port_source(source: &str, lang: &str) -> Result<String> {
    let mut out = String::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    // Emit a header comment
    out.push_str(&format!(
        "// Ported from {} — Elysium 2.0\n",
        if lang == "typescript" {
            "TypeScript"
        } else {
            "JavaScript"
        }
    ));
    out.push_str("// Manual review and adjustment may be needed for advanced patterns.\n\n");

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip empty lines and pure comment lines (passthrough)
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }

        // Handle multi-line comments /* ... */
        if let Some(_rest) = trimmed.strip_prefix("/*") {
            out.push_str(line);
            out.push('\n');
            i += 1;
            while i < lines.len() && !lines[i].trim().contains("*/") {
                out.push_str(lines[i]);
                out.push('\n');
                i += 1;
            }
            if i < lines.len() {
                out.push_str(lines[i]);
                out.push('\n');
                i += 1;
            }
            continue;
        }

        // Try to translate the line
        let translated = translate_line(trimmed, lang);
        let indentation = &line[..line.len() - trimmed.len()];
        out.push_str(indentation);
        out.push_str(&translated);
        out.push('\n');

        i += 1;
    }

    Ok(out)
}

/// Translate a single significant line of TS/JS into Elysium.
fn translate_line(line: &str, _lang: &str) -> String {
    let mut result = line.to_string();

    // 1. Strip trailing semicolons (but not from for-loop headers)
    if !result.starts_with("for") && !result.starts_with("while") {
        result = result.trim_end_matches(';').to_string();
    }

    // 2. Handle export statements
    if let Some(rest) = result.strip_prefix("export ") {
        result = translate_export(rest);
    }

    // 3. Handle import statements
    if result.starts_with("import ") {
        result = translate_import(&result);
        return result;
    }

    // 4. Function declarations
    if let Some(rest) = result.strip_prefix("async function ") {
        result = translate_async_func_decl(rest);
    } else if let Some(rest) = result.strip_prefix("function ") {
        result = translate_func_decl(rest);
    }

    // 5. Arrow functions assigned to const/let
    if let Some(rest) = result.strip_prefix("const ") {
        result = translate_const_arrow(rest, false);
    } else if let Some(rest) = result.strip_prefix("let ") {
        result = translate_const_arrow(rest, false);
    } else if let Some(rest) = result.strip_prefix("var ") {
        result = translate_const_arrow(rest, true);
    }

    // 6. Class declarations
    if let Some(rest) = result.strip_prefix("class ") {
        result = translate_class(rest);
    }

    // 7. Interface declarations
    if let Some(rest) = result.strip_prefix("interface ") {
        result = translate_interface(rest);
    }

    // 8. Type annotations
    if result.starts_with("type ") {
        result = translate_type_alias(&result);
    }

    // 9. Enum declarations
    if let Some(rest) = result.strip_prefix("enum ") {
        result = translate_enum(rest);
    }

    // Apply inline conversions
    result = apply_inline_conversions(&result);

    result
}

fn translate_export(rest: &str) -> String {
    // export function / export async function / export const / export class / export interface / export default
    if let Some(body) = rest.strip_prefix("default ") {
        return format!("// export default: {}", body);
    }
    if let Some(body) = rest.strip_prefix("function ") {
        return translate_func_decl(body);
    }
    if let Some(body) = rest.strip_prefix("async function ") {
        return translate_async_func_decl(body);
    }
    if let Some(body) = rest.strip_prefix("const ") {
        return translate_const_arrow(body, false);
    }
    if let Some(body) = rest.strip_prefix("class ") {
        return translate_class(body);
    }
    if let Some(body) = rest.strip_prefix("interface ") {
        return translate_interface(body);
    }
    // Just strip the export keyword
    rest.to_string()
}

fn translate_import(line: &str) -> String {
    // import defaultExport from 'module'
    // import { named1, named2 } from 'module'
    // import * as name from 'module'
    // import 'module' (side-effect)
    let trimmed = line.trim_end_matches(';');
    // Strip the leading "import "
    let body = trimmed.strip_prefix("import ").unwrap_or(trimmed);

    // Check for side-effect import: import 'module'
    if body.starts_with('\'') || body.starts_with('"') || body.starts_with('`') {
        return format!("// {}", line); // comment out side-effect imports
    }

    // Check for namespace import: import * as name from 'module'
    if body.starts_with("* as ") {
        let rest = body.strip_prefix("* as ").unwrap();
        if let Some((alias, _module)) = rest.split_once(" from ") {
            let alias = alias.trim();
            let module_path = extract_string_literal(_module).unwrap_or_default();
            return format!("import \"{}\" as {}", module_path, alias);
        }
    }

    // Check for default import with optional named imports
    if let Some((first_part, module_part)) = body.split_once(" from ") {
        let module_path = extract_string_literal(module_part).unwrap_or_default();

        // Default import: import Foo from 'module'
        if !first_part.contains('{') && !first_part.contains('*') {
            let name = first_part.trim().trim_end_matches(',');
            return format!("import \"{}\" as {}", module_path, name);
        }

        // Named import: import { a, b } from 'module'  OR  import default, { a } from 'module'
        if first_part.contains('{') {
            // Check if this is purely named (starts with '{') or combined default + named
            if first_part.trim().starts_with('{') {
                // Pure named import: import { a, b } from 'module'
                let named_content = first_part.trim().trim_start_matches('{').trim_end_matches('}');
                let names: Vec<&str> = named_content.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                if names.len() == 1 {
                    return format!("import \"{}\" as {}", module_path, names[0]);
                }
                // For multiple named imports, use the first as the alias and comment the rest
                let mut result = format!("import \"{}\" as {}", module_path, names[0]);
                for n in &names[1..] {
                    result.push_str(&format!("\n// imported symbol: {}", n));
                }
                return result;
            } else {
                // Combined default + named: import default, { a, b } from 'module'
                if let Some((default_part, named_part)) = first_part.split_once(',') {
                    let default_name = default_part.trim();
                    let named = named_part.trim().trim_start_matches('{').trim_end_matches('}');
                    let names: Vec<&str> = named.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                    let mut result = format!("import \"{}\" as {}", module_path, default_name);
                    for n in names {
                        result.push_str(&format!("\n// imported symbol: {}", n));
                    }
                    return result;
                }
            }
        }
    }

    // Fallback: comment out
    format!("// {}", line)
}

fn extract_string_literal(s: &str) -> Option<String> {
    let s = s.trim();
    let stripped = s
        .strip_prefix('\'')
        .or_else(|| s.strip_prefix('"'))
        .or_else(|| s.strip_prefix('`'))?;
    let end = stripped
        .find('\'')
        .or_else(|| stripped.find('"'))
        .or_else(|| stripped.find('`'))?;
    Some(stripped[..end].to_string())
}

fn translate_func_decl(rest: &str) -> String {
    // function name(params)[: RetType] { body }
    let mut input = rest;

    // Check for async prefix
    let prefix = "";

    // Extract name
    let name_end = input.find(|c: char| c == '(' || c == '<' || c == ' ').unwrap_or(input.len());
    let name = &input[..name_end];
    input = &input[name_end..];

    // For generic type params <T>, skip them (Elysium doesn't support generics yet)
    if input.starts_with('<') {
        // Find matching >
        let mut depth = 0;
        let mut gen_end = 0;
        for (i, c) in input.char_indices() {
            if c == '<' { depth += 1; }
            if c == '>' { depth -= 1; }
            if depth == 0 { gen_end = i + 1; break; }
        }
        input = &input[gen_end..];
    }

    // Extract params and return type
    let (params_str, remainder) = extract_paren_group(input);
    let return_type = extract_return_type(&remainder);

    // Clean type annotations from params
    let cleaned_params = clean_param_types(&params_str);

    // Preserve the opening brace if the original input had one
    let suffix = if remainder.contains('{') || input.trim().ends_with('{') { " {" } else { "" };

    format!(
        "{prefix}func {name}({cleaned_params}){return_type}{suffix}",
        prefix = prefix,
        name = name,
        cleaned_params = cleaned_params,
        return_type = if return_type.is_empty() {
            String::new()
        } else {
            format!(" -> {}", return_type)
        },
        suffix = suffix,
    )
}

fn translate_async_func_decl(rest: &str) -> String {
    let base = translate_func_decl(rest);
    format!("async {}", base)
}

fn translate_const_arrow(rest: &str, _is_var: bool) -> String {
    // const name = (...) => { ... }  or  const name = (...) => expr
    // const name: Type = ...
    if let Some(eq_pos) = rest.find('=') {
        let left = rest[..eq_pos].trim();
        let right = rest[eq_pos + 1..].trim();

        // Extract name (strip type annotation)
        let name = left.split(':').next().unwrap_or(left).trim();

        // Arrow function?
        if right.starts_with('(') || right.starts_with("async") {
            let arrow_body = if right.starts_with("async") {
                right.trim_start_matches("async").trim()
            } else {
                right
            };
            // Find the => separator, skipping past any return type annotation
            let arrow_pos = arrow_body.find("=>");
            if let Some(ap) = arrow_pos {
                let params_and_ret = &arrow_body[..ap].trim();
                let body_part = arrow_body[ap + 2..].trim();

                // Extract the return type from between params and =>
                // The params part ends at the first `)` not part of a nested group
                let ret_type = if let Some(paren_end) = params_and_ret.rfind(')') {
                    let after_paren = params_and_ret[paren_end + 1..].trim();
                    if after_paren.starts_with(':') {
                        map_type(after_paren[1..].trim())
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Extract params
                let params_clean = if let Some(paren_start) = params_and_ret.find('(') {
                    let inner = &params_and_ret[paren_start..];
                    if let Some(paren_end) = inner.rfind(')') {
                        clean_param_types(&inner[1..paren_end])
                    } else {
                        clean_param_types(params_and_ret)
                    }
                } else {
                    params_and_ret.split(':').next().unwrap_or(params_and_ret).trim().to_string()
                };

                let ret_suffix = if ret_type.is_empty() { String::new() } else { format!(" -> {}", ret_type) };

                // Body: block or expression
                if body_part.starts_with('{') {
                    format!("func {}({}){}{} {{", name, params_clean,
                        if right.starts_with("async") { "async " } else { "" },
                        ret_suffix)
                } else {
                    format!("func {}({}) -> {{ return {} }}", name, params_clean, body_part)
                }
            } else {
                format!("let {} = {}", name, arrow_body)
            }
        } else if right.starts_with('{') && !right.contains("=>") {
            // Object literal — keep as let
            format!("let {} = {}", name, right)
        } else {
            // Simple value
            format!("let {} = {}", name, right)
        }
    } else {
        // No initializer
        let name = rest.split(':').next().unwrap_or(rest).trim();
        format!("let {}", name)
    }
}

fn translate_class(rest: &str) -> String {
    // class Name[<T>] [extends Base] [implements I1, I2] { ... }
    let mut input = rest.trim();

    // Extract class name
    let name_end = input.find(|c: char| c == '<' || c == ' ' || c == '{').unwrap_or(input.len());
    let name = &input[..name_end];
    input = &input[name_end..];

    // Skip generics
    if input.starts_with('<') {
        let mut depth = 0;
        let mut end = 0;
        for (i, c) in input.char_indices() {
            if c == '<' { depth += 1; }
            if c == '>' { depth -= 1; }
            if depth == 0 { end = i + 1; break; }
        }
        input = &input[end..];
    }

    // Skip extends clause (Elysium doesn't support inheritance yet)
    if let Some(ext) = input.strip_prefix("extends ") {
        let _base_end = ext.find(|c: char| c == ' ' || c == '{' || c == 'i').unwrap_or(ext.len());
        let _base_name = &ext[.._base_end];
        let _ = ext[_base_end..].trim_start();
        // Emit a comment about extends
    }

    // Skip implements clause
    if let Some(imp) = input.strip_prefix("implements ") {
        let impl_end = imp.find('{').unwrap_or(imp.len());
        let _ifaces = &imp[..impl_end];
        let _ = imp[impl_end..].trim_start();
    }

    format!("class {} {{", name)
}

fn translate_interface(rest: &str) -> String {
    // interface Name[<T>] [extends Base] { ... }
    let mut input = rest.trim();

    let name_end = input.find(|c: char| c == '<' || c == ' ' || c == '{').unwrap_or(input.len());
    let name = &input[..name_end];
    input = &input[name_end..];

    if input.starts_with('<') {
        let mut depth = 0;
        let mut end = 0;
        for (i, c) in input.char_indices() {
            if c == '<' { depth += 1; }
            if c == '>' { depth -= 1; }
            if depth == 0 { end = i + 1; break; }
        }
        input = &input[end..];
    }

    if let Some(ext) = input.strip_prefix("extends ") {
        let _end = ext.find('{').unwrap_or(ext.len());
        let _ = ext[_end..].trim_start();
    }

    // Convert to a doc-comment + type stub
    format!("// interface {} -- define or stub members below", name)
}

fn translate_type_alias(line: &str) -> String {
    // type Name = ...;
    let body = line.strip_prefix("type ").unwrap_or(line);
    if let Some((name, _value)) = body.split_once('=') {
        let name = name.trim();
        format!("// type alias: {} (define as needed)\n// typealias {} = ...", name, name)
    } else {
        format!("// type alias: {}", body.trim())
    }
}

fn translate_enum(rest: &str) -> String {
    let name = rest.split(|c: char| c == ' ' || c == '{').next().unwrap_or(rest).trim();
    format!("enum {} {{", name)
}

// ---- Helpers ----

/// Extract the content inside the first balanced parentheses group.
/// Returns (inner_content, remainder_after_closing_paren).
fn extract_paren_group(s: &str) -> (String, String) {
    let s = s.trim();
    if !s.starts_with('(') {
        return (String::new(), s.to_string());
    }
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c == '(' { depth += 1; }
        if c == ')' { depth -= 1; }
        if depth == 0 { end = i + 1; break; }
    }
    if end == 0 { return (String::new(), s.to_string()); }
    let inner = &s[1..end - 1];
    let remainder = s[end..].trim();
    (inner.to_string(), remainder.to_string())
}

/// Extract the return type annotation after a closing paren.
/// Returns the mapped Elysium type (or empty string if void/any).
fn extract_return_type(s: &str) -> String {
    let s = s.trim();
    if let Some(colon_pos) = s.find(':') {
        let type_part = s[colon_pos + 1..].trim();
        // Take until first space, brace, or newline
        let type_str = type_part.split(|c: char| c == ' ' || c == '{').next().unwrap_or(type_part).trim();
        let mapped = map_type(type_str);
        if mapped.is_empty() { String::new() } else { mapped }
    } else {
        String::new()
    }
}

/// Clean TypeScript type annotations from parameter declarations.
/// "x: string" → "x: String", "x" → "x"
fn clean_param_types(params: &str) -> String {
    if params.trim().is_empty() {
        return String::new();
    }
    params
        .split(',')
        .map(|p| {
            let p = p.trim();
            if let Some((name, _ty)) = p.split_once(':') {
                let name = name.trim();
                let ty = p.split_once(':').map(|(_, t)| t.trim()).unwrap_or("");
                if !ty.is_empty() {
                    let mapped = map_type(ty);
                    if mapped.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}: {}", name, mapped)
                    }
                } else {
                    name.to_string()
                }
            } else {
                // Check for default value (x = 5)
                p.split('=').next().unwrap_or(p).trim().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Map TypeScript types to Elysium types.
fn map_type(ts_type: &str) -> String {
    let t = ts_type.trim().trim_end_matches("[]");
    let is_array = ts_type.trim().ends_with("[]");
    let mapped: String = match t {
        "string" => "String".to_string(),
        "number" | "int" | "integer" | "bigint" => "Int".to_string(),
        "boolean" | "bool" => "Bool".to_string(),
        "void" | "undefined" | "never" => String::new(),
        "any" | "unknown" | "object" => String::new(),
        "null" => String::new(),
        "nil" => "Nil".to_string(),
        _ => {
            // Check for Promise<T>
            if let Some(inner) = t.strip_prefix("Promise<") {
                let inner_t = inner.trim_end_matches('>').trim();
                let mapped_inner = map_type(inner_t);
                if mapped_inner.is_empty() {
                    "Future".to_string()
                } else {
                    format!("Future<{}>", mapped_inner)
                }
            } else if let Some(inner) = t.strip_prefix("Array<") {
                let inner_t = inner.trim_end_matches('>').trim();
                let mapped_inner = map_type(inner_t);
                if mapped_inner.is_empty() {
                    "[]".to_string()
                } else {
                    format!("[{}]", mapped_inner)
                }
            } else {
                // Keep as-is (assumed to be a custom type)
                let cleaned = t.trim_end_matches(',').trim();
                if cleaned.is_empty() { return String::new(); }
                // Capitalize first letter
                let mut c = cleaned.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        }
    };
    if mapped.is_empty() {
        String::new()
    } else if is_array {
        format!("[{}]", mapped)
    } else {
        mapped.to_string()
    }
}

/// Apply inline string-level conversions for common patterns.
fn apply_inline_conversions(s: &str) -> String {
    let mut r = s.to_string();

    // Operators: JS/TS → Elysium
    r = r.replace("===", "==");
    r = r.replace("!==", "!=");

    // null/undefined → nil
    r = r.replace("null", "nil");
    r = r.replace("undefined", "nil");

    // Template literals: `...${expr}...` → str concat
    // Simple case: replace backtick strings with quoted strings
    if r.contains('`') {
        r = convert_template_literal(&r);
    }

    // Arrow function body: ... => {  →  { (keep as is)
    // Arrow shorthand: ... => expr  →  { return expr }
    // We handle this partially in translate_const_arrow

    // Optional chaining: ?.  →  .
    // (Elysium doesn't have optional chaining yet; emit a caution)
    r = r.replace("?.", ".");

    // Nullish coalescing: ??  →  // ?? (emit caution)
    // Keep as-is but add a comment note when we encounter it
    if r.contains("??") {
        // Don't replace, just note. User will need to handle manually.
    }

    // Strip type assertions: as Type
    // Simple heuristic: remove " as " followed by an identifier/type
    r = remove_type_assertions(&r);

    // Strip non-null assertions: !
    // e.g., x! → x
    r = remove_non_null_assertions(&r);

    // Strip definite assignment: x!: Type → x:
    r = remove_definite_assignment(&r);

    // Console.log → print (keep console.log as-is, user can adjust)
    // Actually keep as console.log — desugaring handles it

    r
}

fn convert_template_literal(s: &str) -> String {
    // Simple conversion: replace backtick template literals with string concat
    // Handle `${expr}` interpolation
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    let mut in_template = false;

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                if in_template {
                    // Closing backtick
                    in_template = false;
                } else {
                    in_template = true;
                }
                // Replace backtick with "
                result.push('"');
            }
            '$' if in_template && chars.peek() == Some(&'{') => {
                chars.next(); // skip {
                result.push_str("\" + ");
                // Collect the expression until }
                let mut expr_depth = 1;
                let mut expr = String::new();
                while let Some(&ec) = chars.peek() {
                    match ec {
                        '{' => { expr_depth += 1; expr.push(chars.next().unwrap()); }
                        '}' => {
                            expr_depth -= 1;
                            if expr_depth == 0 {
                                chars.next(); // skip }
                                break;
                            }
                            expr.push(chars.next().unwrap());
                        }
                        _ => expr.push(chars.next().unwrap()),
                    }
                }
                // Clean and append the expression
                let expr = expr.trim();
                if !expr.is_empty() {
                    result.push_str(expr);
                    result.push_str(" + \"");
                } else {
                    result.push('"');
                }
            }
            _ => result.push(c),
        }
    }

    if in_template {
        // Unclosed template — just return original
        s.to_string()
    } else {
        result
    }
}

fn remove_type_assertions(s: &str) -> String {
    // Remove " as Type" patterns (heuristic: " as " followed by word chars)
    let mut result = String::new();
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < s.len() {
        if i + 4 < s.len() && &s[i..i + 4] == " as " {
            // Look ahead to find the end of the type name
            let start = i + 4;
            let mut end = start;
            while end < s.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'<') {
                end += 1;
                if end < s.len() && bytes[end] == b'>' {
                    end += 1;
                    break;
                }
            }
            i = end;
            continue;
        }
        result.push(s.as_bytes()[i] as char);
        i += 1;
    }
    result
}

fn remove_non_null_assertions(s: &str) -> String {
    // Remove "!" that follows ident-like chars (heuristic)
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if bytes[i] == b'!' && i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b')' || bytes[i - 1] == b']') {
            // Check it's not a != / !== operator
            if i + 1 < s.len() && (bytes[i + 1] == b'=') {
                result.push('!');
            }
            // Else skip the '!' (non-null assertion)
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}

fn remove_definite_assignment(s: &str) -> String {
    // Remove "!" in "x!: Type" patterns
    s.replace("!: ", ": ")
}

/// Port a file from the given path, writing to output_path or stdout.
pub fn port_file(file_path: &Path, output_path: Option<&std::path::Path>, lang_override: &Option<String>) -> Result<()> {
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| CompileError::new(format!("Failed to read {}: {}", file_path.display(), e)))?;

    let lang = detect_lang(file_path, lang_override);
    let result = port_source(&source, lang)?;

    match output_path {
        Some(path) => {
            std::fs::write(path, &result)
                .map_err(|e| CompileError::new(format!("Failed to write {}: {}", path.display(), e)))?;
            println!("Ported {} → {}", file_path.display(), path.display());
        }
        None => {
            println!("{}", result);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_lang_ts() {
        let p = Path::new("foo.ts");
        assert_eq!(detect_lang(p, &None), "typescript");
    }

    #[test]
    fn test_detect_lang_js() {
        let p = Path::new("foo.js");
        assert_eq!(detect_lang(p, &None), "javascript");
    }

    #[test]
    fn test_detect_lang_override() {
        let p = Path::new("foo.xyz");
        assert_eq!(detect_lang(p, &Some("js".to_string())), "javascript");
    }

    #[test]
    fn test_port_func_decl() {
        let r = translate_line("function add(a: number, b: number): number {", "typescript");
        assert_eq!(r, "func add(a: Int, b: Int) -> Int {");
    }

    #[test]
    fn test_port_func_decl_no_return() {
        let r = translate_line("function greet(name: string) {", "typescript");
        assert_eq!(r, "func greet(name: String) {");
    }

    #[test]
    fn test_port_func_decl_void_return() {
        let r = translate_line("function log(msg: string): void {", "typescript");
        assert_eq!(r, "func log(msg: String) {");
    }

    #[test]
    fn test_port_async_func() {
        let r = translate_line("async function fetchData(url: string): Promise<string> {", "typescript");
        assert_eq!(r, "async func fetchData(url: String) -> Future<String> {");
    }

    #[test]
    fn test_port_const_arrow() {
        let r = translate_line("const add = (a: number, b: number): number => {", "typescript");
        assert_eq!(r, "func add(a: Int, b: Int) -> Int {");
    }

    #[test]
    fn test_port_const_arrow_single_param() {
        let r = translate_line("const double = (x: number) => x * 2", "typescript");
        assert!(r.contains("func double"));
    }

    #[test]
    fn test_port_let_value() {
        let r = translate_line("let x: number = 42", "typescript");
        assert_eq!(r, "let x = 42");
    }

    #[test]
    fn test_port_import_default() {
        let r = translate_import("import express from 'express'");
        assert_eq!(r, "import \"express\" as express");
    }

    #[test]
    fn test_port_import_named() {
        let r = translate_import("import { readFile, writeFile } from 'fs'");
        assert!(r.contains("import \"fs\" as readFile"), "got: {:?}", r);
        assert!(r.contains("writeFile"), "got: {:?}", r);
    }

    #[test]
    fn test_port_import_namespace() {
        let r = translate_import("import * as fs from 'fs'");
        assert_eq!(r, "import \"fs\" as fs");
    }

    #[test]
    fn test_port_class() {
        let r = translate_line("class Animal {", "typescript");
        assert_eq!(r, "class Animal {");
    }

    #[test]
    fn test_port_class_extends() {
        let r = translate_line("class Dog extends Animal implements Pet {", "typescript");
        assert_eq!(r, "class Dog {");
    }

    #[test]
    fn test_port_interface() {
        let r = translate_line("interface User {", "typescript");
        assert_eq!(r, "// interface User -- define or stub members below");
    }

    #[test]
    fn test_port_enum() {
        let r = translate_line("enum Color {", "typescript");
        assert_eq!(r, "enum Color {");
    }

    #[test]
    fn test_port_type_alias() {
        let r = translate_line("type Callback = (err: Error) => void", "typescript");
        assert!(r.contains("typealias"));
    }

    #[test]
    fn test_null_to_nil() {
        let r = apply_inline_conversions("let x = null");
        assert_eq!(r, "let x = nil");
    }

    #[test]
    fn test_undefined_to_nil() {
        let r = apply_inline_conversions("if (x === undefined) {");
        assert_eq!(r, "if (x == nil) {");
    }

    #[test]
    fn test_strict_equals() {
        let r = apply_inline_conversions("x === y");
        assert_eq!(r, "x == y");
    }

    #[test]
    fn test_port_export_func() {
        let r = translate_line("export function sum(a: number, b: number): number {", "typescript");
        assert_eq!(r, "func sum(a: Int, b: Int) -> Int {");
    }

    #[test]
    fn test_semicolon_strip() {
        let r = translate_line("let x = 42;", "javascript");
        assert_eq!(r, "let x = 42");
    }

    #[test]
    fn test_template_literal_basic() {
        let r = convert_template_literal("`hello`");
        assert_eq!(r, "\"hello\"");
    }

    #[test]
    fn test_template_literal_interp() {
        let r = convert_template_literal("`Hello, ${name}!`");
        assert_eq!(r, "\"Hello, \" + name + \"!\"");
    }

    #[test]
    fn test_type_assertion_strip() {
        let r = remove_type_assertions("let x = y as string");
        assert_eq!(r, "let x = y");
    }

    #[test]
    fn test_non_null_assertion_strip() {
        let r = remove_non_null_assertions("x!");
        assert_eq!(r, "x");
    }
}
