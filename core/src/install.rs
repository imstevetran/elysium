use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};

use crate::error;
use crate::manifest::{self, Manifest};
use crate::update;

/// Run `elysium install`.
/// Resolves dependencies from the project's elysium.json, queries the EPM registry,
/// downloads packages into elysium_modules/, and generates ellysium.lock.
pub fn cmd_install(pkg_opt: Option<&str>) -> error::Result<()> {
    let cwd = std::env::current_dir()
        .map_err(|e| error::CompileError::new(format!("Cannot get current directory: {}", e)))?;

    let root = manifest::find_project_root(&cwd).ok_or_else(|| {
        error::CompileError::new("No elysium.json found in current or parent directories")
    })?;

    let manifest_path = root.join("elysium.json");
    let manifest_content = fs::read_to_string(&manifest_path)
        .map_err(|e| error::CompileError::new(format!("Cannot read {}: {}", manifest_path.display(), e)))?;
    let manifest: Manifest = serde_json::from_str(&manifest_content)
        .map_err(|e| error::CompileError::new(format!("Cannot parse {}: {}", manifest_path.display(), e)))?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies in elysium.json.");
        return Ok(());
    }

    // Load or sync the EPM registry
    let registry = update::load_registry()?;

    let mods_dir = root.join("elysium_modules");

    // Track installed packages for lockfile
    let mut lock_packages: HashMap<String, serde_json::Value> = HashMap::new();

    let deps_to_install: Vec<(String, String)> = if let Some(pkg) = pkg_opt {
        match manifest.dependencies.get(pkg) {
            Some(constraint) => vec![(pkg.to_string(), constraint.clone())],
            None => {
                // Allow installing a specific version: pkg@version
                if let Some(at_pos) = pkg.find('@') {
                    let name = &pkg[..at_pos];
                    let version_str = &pkg[at_pos + 1..];
                    vec![(name.to_string(), format!("^{}", version_str))]
                } else {
                    return Err(error::CompileError::new(format!(
                        "Package '{}' not found in dependencies. Use 'elysium install pkg@version' to add it.",
                        pkg
                    )));
                }
            }
        }
    } else {
        manifest.dependencies.clone().into_iter().collect()
    };

    for (pkg_name, constraint) in &deps_to_install {
        println!("Installing `{}` (constraint: {})...", pkg_name, constraint);

        // Look up the package in the registry
        let entry = registry.packages.get(pkg_name).ok_or_else(|| {
            error::CompileError::new(format!(
                "Package `{}` not found in EPM registry",
                pkg_name
            ))
        })?;

        // Parse versions and find best match
        let constraint_req = parse_constraint(constraint);
        let mut parsed_versions: Vec<Version> = entry
            .versions
            .iter()
            .filter_map(|v| Version::parse(v.trim_start_matches('v')).ok())
            .collect();
        parsed_versions.sort();
        parsed_versions.reverse();

        let best_version = parsed_versions.iter().find(|v| constraint_req.matches(v));
        let version_to_install = match best_version {
            Some(v) => v.to_string(),
            None => {
                eprintln!("  No version matching `{}` found for `{}`", constraint, pkg_name);
                continue;
            }
        };

        println!("  Resolved to version {}", version_to_install);

        // Create the target directory
        let target_dir = mods_dir.join(pkg_name);
        fs::create_dir_all(&target_dir)
            .map_err(|e| error::CompileError::new(format!("Cannot create {}: {}", target_dir.display(), e)))?;

        // Download and extract the package tarball
        download_package(pkg_name, &version_to_install, &target_dir)?;

        // Track for lockfile
        lock_packages.insert(
            pkg_name.clone(),
            serde_json::json!({
                "version": version_to_install,
                "integrity": "sha256-pending"
            }),
        );

        println!("  ✓ Installed");
    }

    // Generate ellysium.lock
    if !lock_packages.is_empty() {
        let lockfile_path = root.join("elysium.lock");
        let lock_content = serde_json::json!({
            "version": 1,
            "packages": lock_packages,
        });
        let lock_json = serde_json::to_string_pretty(&lock_content)
            .map_err(|e| error::CompileError::new(format!("Failed to serialize lockfile: {}", e)))?;
        fs::write(&lockfile_path, lock_json + "\n")
            .map_err(|e| error::CompileError::new(format!("Failed to write {}: {}", lockfile_path.display(), e)))?;
        println!("\n✓ Generated ellysium.lock");
    }

    Ok(())
}

/// Parse a version constraint string (same logic as update.rs)
fn parse_constraint(s: &str) -> VersionReq {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return VersionReq::STAR;
    }
    if trimmed.contains('>') || trimmed.contains('<') || trimmed.contains('=') || trimmed.contains('^') || trimmed.contains('~') {
        VersionReq::parse(trimmed).unwrap_or(VersionReq::STAR)
    } else {
        VersionReq::parse(&format!("^{}", trimmed.trim_start_matches('v')))
            .unwrap_or(VersionReq::STAR)
    }
}

/// Download a package tarball from the registry and extract it into `target_dir`.
/// Uses a simple tar+fetch approach for now — the registry is git-based, so we
/// build the tarball by downloading source from the registry repository.
///
/// For v1: we simulate this by creating a placeholder structure.
/// TODO: Replace with real HTTP download + tar extraction when the registry
///       server is ready.
fn download_package(_name: &str, _version: &str, _target_dir: &Path) -> error::Result<()> {
    // For now, this is a placeholder — the EPM registry is git-based.
    // In a future iteration, this will:
    //   1. Query the registry API at the version's tarball URL
    //   2. Download the .tar.gz
    //   3. Extract into target_dir
    // For development, users should manually place packages into elysium_modules/.
    eprintln!("  Note: download from EPM registry not yet implemented.");
    eprintln!("  Place the package manually into {}", _target_dir.display());
    Ok(())
}
