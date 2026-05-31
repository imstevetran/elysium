//! Import resolution and merged program loading.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::elyx;
use crate::error;
use crate::manifest;
use crate::parser;

use super::source::read_source;

pub fn load_with_imports(file: &PathBuf) -> error::Result<(String, ast::Program, std::collections::HashSet<String>)> {
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

/// Walk up from `dir` to find the project root (directory containing elysium.json).
pub fn find_project_root(dir: &std::path::Path) -> Option<PathBuf> {
    manifest::find_project_root(dir)
}

/// Map pre-scope package names to the official `@elysium/*` registry names.
fn legacy_package_alias(name: &str) -> &str {
    match name {
        "langchain" => "@elysium/langchain",
        "langgraph" => "@elysium/langgraph",
        "auth" => "@elysium/auth",
        "ble" => "@elysium/ble",
        "zigbee" => "@elysium/zigbee",
        other => other,
    }
}

/// Try to resolve an import path relative to the source directory.
/// Supports:
///   import "./foo.ely"       — relative to importing file
///   import "@/foo"             — relative to project root
///   import "@/sub/bar"
///   import "#/package"        — package in the project's packages/ directory
pub fn find_import_file(from_dir: &std::path::Path, import_path: &str) -> std::result::Result<PathBuf, String> {
    let clean = import_path.trim().trim_matches('"');

    // Resolve @/ prefix: relative to project root
    if let Some(rest) = clean.strip_prefix("@/") {
        let root = find_project_root(from_dir)
            .ok_or_else(|| format!("cannot find project root (elysium.json) from `{}`", from_dir.display()))?;
        let candidate = root.join(rest);
        // Try exact path
        if candidate.exists() {
            return Ok(candidate);
        }
        // Try with .ely extension
        let with_ely = root.join(format!("{}.ely", rest.trim_end_matches(".elyx")));
        if with_ely.exists() {
            return Ok(with_ely);
        }
        // Try with .elyx extension
        let with_elyx = root.join(format!("{}.elyx", rest.trim_end_matches(".ely")));
        if with_elyx.exists() {
            return Ok(with_elyx);
        }
        return Err(format!(
            "cannot find import `{}` (tried: {}, {}, {})",
            clean,
            candidate.display(),
            with_ely.display(),
            with_elyx.display()
        ));
    }

    // Resolve #/ prefix: package installed from the registry (into elysium_modules/)
    if let Some(pkg_name) = clean.strip_prefix("#/") {
        let root = find_project_root(from_dir)
            .ok_or_else(|| format!("cannot find project root (elysium.json) from `{}`", from_dir.display()))?;
        // Legacy short names → official @elysium/* scope
        let pkg_name = legacy_package_alias(pkg_name);
        let mods_dir = root.join("elysium_modules");
        let pkg_dir = mods_dir.join(pkg_name);

        // Try elysium_modules/<name>/<entry> from manifest
        let manifest_path = pkg_dir.join("elysium.json");
        if manifest_path.exists() {
            // Read manifest to find entry file
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
                    let entry = manifest.get("entry")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main.ely");
                    let entry_path = pkg_dir.join(entry);
                    if entry_path.exists() {
                        return Ok(entry_path);
                    }
                }
            }
        }
        // Fallback: try elysium_modules/<name>/main.ely
        let main_path = pkg_dir.join("main.ely");
        if main_path.exists() {
            return Ok(main_path);
        }
        // Fallback: try elysium_modules/<name>.ely
        let single_path = mods_dir.join(format!("{}.ely", pkg_name));
        if single_path.exists() {
            return Ok(single_path);
        }
        // Fallback: try packages/<name>/elysium.json → reads entry field (directory package)
        let pkg_local_dir = root.join("packages").join(pkg_name);
        let pkg_local_manifest = pkg_local_dir.join("elysium.json");
        if pkg_local_manifest.exists() {
            if let Ok(content) = std::fs::read_to_string(&pkg_local_manifest) {
                if let Ok(manifest) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
                    let entry = manifest.get("entry")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main.ely");
                    let entry_path = pkg_local_dir.join(entry);
                    if entry_path.exists() {
                        return Ok(entry_path);
                    }
                }
            }
        }
        // Fallback: try packages/<name>.ely (development: legacy single file)
        let pkg_path = root.join("packages").join(format!("{}.ely", pkg_name));
        if pkg_path.exists() {
            return Ok(pkg_path);
        }
        // Fallback: try packages/<name>/main.ely (development: legacy directory)
        let pkg_dir_path = pkg_local_dir.join("main.ely");
        if pkg_dir_path.exists() {
            return Ok(pkg_dir_path);
        }
        return Err(format!(
            "cannot find package `{}` (tried: {}, {}, {}, {}, {}, {})",
            pkg_name,
            pkg_dir.display(),
            main_path.display(),
            single_path.display(),
            pkg_local_manifest.display(),
            pkg_path.display(),
            pkg_dir_path.display()
        ));
    }

    // Regular relative path
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
