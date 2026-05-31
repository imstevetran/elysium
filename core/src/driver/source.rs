//! Source file I/O and quick scans.

use std::fs;
use std::path::PathBuf;

use crate::error;

pub fn is_elyx_file(file: &PathBuf) -> bool {
    file.extension().map(|ext| ext == "elyx").unwrap_or(false)
}

pub fn read_source(file: &PathBuf) -> error::Result<String> {
    fs::read_to_string(file)
        .map_err(|e| error::CompileError::new(format!("Failed to read file: {}", e)))
}

/// Quick check whether a file contains any `import` statements.
pub fn has_imports(file: &PathBuf) -> error::Result<bool> {
    let source = read_source(file)?;
    Ok(source.contains("import \""))
}
