use std::fs;
use std::path::Path;

use crate::error;
use crate::manifest;

/// Run `elysium publish`.
/// Reads the package's elysium.json, validates it, and pushes it to the EPM registry.
pub fn cmd_publish(path_opt: Option<&Path>) -> error::Result<()> {
    let cwd = std::env::current_dir()
        .map_err(|e| error::CompileError::new(format!("Cannot get current directory: {}", e)))?;

    // Determine the package directory
    let pkg_dir = match path_opt {
        Some(p) => {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            }
        }
        None => cwd.clone(),
    };

    if !pkg_dir.exists() {
        return Err(error::CompileError::new(format!(
            "Package directory `{}` does not exist",
            pkg_dir.display()
        )));
    }

    // Read the package manifest
    let manifest_path = pkg_dir.join("elysium.json");
    if !manifest_path.exists() {
        return Err(error::CompileError::new(format!(
            "No elysium.json found in `{}`. Are you in a package directory?",
            pkg_dir.display()
        )));
    }

    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| error::CompileError::new(format!("Cannot read {}: {}", manifest_path.display(), e)))?;
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| error::CompileError::new(format!("Cannot parse {}: {}", manifest_path.display(), e)))?;

    // Validate required fields
    let name = manifest.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error::CompileError::new("Package manifest is missing 'name' field"))?;
    let version = manifest.get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error::CompileError::new("Package manifest is missing 'version' field"))?;
    let entry = manifest.get("entry")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error::CompileError::new("Package manifest is missing 'entry' field"))?;

    // Validate entry file exists
    let entry_path = pkg_dir.join(entry);
    if !entry_path.exists() {
        return Err(error::CompileError::new(format!(
            "Entry file `{}` (from 'entry' field) not found in `{}`",
            entry,
            pkg_dir.display()
        )));
    }

    println!("Publishing `{}` v{}...", name, version);
    println!("  Package directory: {}", pkg_dir.display());
    println!("  Entry file: {}", entry_path.display());
    println!();

    // For v1, publishing requires manual registry setup.
    // The EPM registry is git-based — publishing means:
    //   1. Tag the package version in the registry
    //   2. Update registry.json with the new version entry
    //   3. Push to GitHub
    //
    // We print instructions for now until the full automation is built.
    let description = manifest.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let author = manifest.get("author").and_then(|v| v.as_str()).unwrap_or("");

    eprintln!("  Note: automated publishing is not yet implemented.");
    eprintln!("  To publish manually:");
    eprintln!("    1. Fork or clone the EPM registry repository:");
    eprintln!("       git clone https://github.com/imstevetran/epm-registry.git");
    eprintln!("    2. Add your package to registry.json:");
    eprintln!("       {{\"name\": \"{}\", \"description\": \"{}\", \"versions\": [\"{}\"]}}", name, description, version);
    eprintln!("    3. Copy your package directory to the registry's packages/{}", name);
    eprintln!("    4. Commit and push");
    eprintln!();
    println!("  Package `{}` v{} is ready for publishing.", name, version);

    Ok(())
}
