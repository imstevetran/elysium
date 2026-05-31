/// Extension loader — discovers and registers extensions from elysium.json.
///
/// Extensions are declared in `elysium.json` under the `"extensions"` field:
/// ```json
/// { "extensions": ["extensions/json"] }
/// ```
///
/// Each extension directory contains an `.ely` file with an `extension` statement:
/// ```ely
/// extension "json" {
///     keywords: ["json.parse", "json.buildObject", ...],
///     runtime: { other: "./runtime/json.c" }
/// }
/// ```

use std::path::{Path, PathBuf};
use std::fs;
use crate::ast::{self, Item};
use crate::error::{self, Result};
use crate::parser::Parser;

/// Loaded extension data.
#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub keywords: Vec<String>,
    pub runtime_files: Vec<PathBuf>,
}

/// Load all extensions declared in elysium.json.
/// Returns None if no extensions field is present.
pub fn load_extensions(project_root: &Path) -> Result<Vec<Extension>> {
    let manifest_path = project_root.join("elysium.json");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| error::CompileError::new(format!("cannot read elysium.json: {}", e)))?;

    let parsed: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| error::CompileError::new(format!("invalid elysium.json: {}", e)))?;

    let ext_paths = match parsed.get("extensions") {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(_) => return Err(error::CompileError::new(
            "\"extensions\" field in elysium.json must be an array of strings"
        )),
        None => return Ok(Vec::new()),
    };

    let mut extensions = Vec::new();
    for ext_val in ext_paths {
        let ext_path_str = ext_val.as_str().ok_or_else(||
            error::CompileError::new("extensions array entry must be a string (path)")
        )?;
        let ext_dir = project_root.join(ext_path_str);
        let ext = load_single_extension(&ext_dir, project_root)?;
        extensions.push(ext);
    }

    Ok(extensions)
}

/// Load a single extension from a directory.
fn load_single_extension(ext_dir: &Path, project_root: &Path) -> Result<Extension> {
    if !ext_dir.is_dir() {
        return Err(error::CompileError::new(
            format!("extension directory not found: {}", ext_dir.display())
        ));
    }

    // Find the .ely file in the extension directory
    let entries = fs::read_dir(ext_dir)
        .map_err(|e| error::CompileError::new(format!("cannot read extension dir: {}", e)))?;

    let mut ely_file: Option<PathBuf> = None;
    for entry in entries {
        let entry = entry.map_err(|e| error::CompileError::new(format!("readdir: {}", e)))?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "ely") {
            ely_file = Some(path);
            break;
        }
    }

    let ely_path = ely_file.ok_or_else(||
        error::CompileError::new(format!("no .ely file found in extension `{}`", ext_dir.display()))
    )?;

    let source = fs::read_to_string(&ely_path)
        .map_err(|e| error::CompileError::new(format!("cannot read extension file: {}", e)))?;

    let mut parser = Parser::new(&source);
    let program = parser.parse_program().map_err(|e| {
        error::CompileError::new(format!("failed to parse extension `{}`: {}", ext_dir.display(), e.message))
    })?;

    // Find the extension statement
    for item in &program.items {
        if let Item::Extension(reg) = &item.value {
            let _ext_dir_parent = ext_dir.parent().unwrap_or(ext_dir);
            let runtime_files = resolve_runtime_files(&reg.runtime, ext_dir, project_root);

            let all_keywords = reg.keywords.clone();

            // If the extension has a name like "json", also register keywords
            // with that prefix
            if !reg.name.is_empty() && all_keywords.is_empty() {
                // Try to find keywords from the register body
            }

            return Ok(Extension {
                name: reg.name.clone(),
                keywords: all_keywords,
                runtime_files,
            });
        }
    }

    Err(error::CompileError::new(
        format!("no `extension` statement found in extension `{}`", ext_dir.display())
    ))
}

/// Resolve runtime file paths (relative to extension dir) to absolute paths.
fn resolve_runtime_files(
    entries: &[ast::RuntimeEntry],
    ext_dir: &Path,
    _project_root: &Path,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in entries {
        // Resolve relative paths from the extension directory
        let path = if Path::new(&entry.path).is_relative() {
            ext_dir.join(&entry.path)
        } else {
            PathBuf::from(&entry.path)
        };

        if path.exists() {
            files.push(path);
        }
        // If not found, skip silently — the platform may not need it
    }
    files
}
