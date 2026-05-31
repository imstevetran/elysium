use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};

use crate::error;
use crate::manifest::{self, Manifest};

/// Determine the project root by looking for `elysium.json` walking up from `cwd`.
fn find_project_root(cwd: &Path) -> Option<PathBuf> {
    manifest::find_project_root(cwd)
}

/// A single entry in the registry index.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub description: Option<String>,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryIndex {
    pub packages: HashMap<String, RegistryEntry>,
}

/// Check for available updates for one dependency.
/// Returns `(current_version, best_new_version, constraint)`.
fn check_update(
    pkg_name: &str,
    current_constraint: &str,
    registry: &RegistryIndex,
    to_latest: bool,
) -> Option<(String, String, String)> {
    let entry = registry.packages.get(pkg_name)?;
    let versions = &entry.versions;

    // Parse available versions into semver
    let mut parsed: Vec<Version> = versions
        .iter()
        .filter_map(|v| Version::parse(v.trim_start_matches('v')).ok())
        .collect();
    parsed.sort();
    parsed.reverse(); // newest first

    if parsed.is_empty() {
        return None;
    }

    if to_latest {
        // Show absolute latest version
        let latest = parsed[0].to_string();
        // Determine current constraint target
        let constraint_req = parse_constraint(current_constraint);
        let current_matched = parsed
            .iter()
            .find(|v| constraint_req.matches(v))
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_string());

        if latest != current_matched {
            return Some((current_matched, latest, current_constraint.to_string()));
        }
        return None;
    }

    // Normal mode: find latest version within the constraint range
    let constraint_req = parse_constraint(current_constraint);
    let current_matched = parsed
        .iter()
        .find(|v| constraint_req.matches(v))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".to_string());

    let best_in_range = parsed
        .iter()
        .find(|v| constraint_req.matches(v))
        .map(|v| v.to_string());

    match best_in_range {
        Some(ref b) if *b != current_matched => Some((current_matched, b.clone(), current_constraint.to_string())),
        _ => None,
    }
}

fn parse_constraint(s: &str) -> VersionReq {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return VersionReq::STAR;
    }
    if trimmed.contains('>') || trimmed.contains('<') || trimmed.contains('=') || trimmed.contains('^') || trimmed.contains('~') {
        VersionReq::parse(trimmed).unwrap_or(VersionReq::STAR)
    } else {
        // Bare version like "0.1.0" → caret "^0.1.0"
        VersionReq::parse(&format!("^{}", trimmed.trim_start_matches('v')))
            .unwrap_or(VersionReq::STAR)
    }
}

/// Run the `elysium update` command.
/// Reads elysium.json, queries the EPM registry, and reports/reconciles updates.
pub fn cmd_update(
    pkg_opt: Option<&str>,
    apply: bool,
    to_latest: bool,
    _force: bool,
) -> error::Result<()> {
    let cwd = std::env::current_dir().map_err(|e| error::CompileError::new(format!("Cannot get cwd: {}", e)))?;

    let root = find_project_root(&cwd)
        .ok_or_else(|| error::CompileError::new("No elysium.json found in current or parent directories"))?;

    // Read manifest
    let manifest_path = root.join("elysium.json");
    let manifest_content = fs::read_to_string(&manifest_path)
        .map_err(|e| error::CompileError::new(format!("Cannot read {}: {}", manifest_path.display(), e)))?;
    let mut manifest: Manifest = serde_json::from_str(&manifest_content)
        .map_err(|e| error::CompileError::new(format!("Cannot parse {}: {}", manifest_path.display(), e)))?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies in elysium.json.");
        return Ok(());
    }

    // Read the EPM registry index
    let registry = load_registry()?;

    let mut any_updates = false;
    let mut updates: Vec<(String, String, String)> = Vec::new(); // (pkg, current, available)

    let deps_to_check: Vec<(String, String)> = if let Some(pkg) = pkg_opt {
        match manifest.dependencies.get(pkg) {
            Some(constraint) => vec![(pkg.to_string(), constraint.clone())],
            None => {
                eprintln!("Package '{}' not found in dependencies.", pkg);
                return Err(error::CompileError::new("Package not found"));
            }
        }
    } else {
        manifest.dependencies.clone().into_iter().collect()
    };

    for (pkg_name, constraint) in &deps_to_check {
        match check_update(pkg_name, constraint, &registry, to_latest) {
            Some((current, available, _)) => {
                if current == available || current == "?" {
                    println!("  {} @ {} (current: {}) — up to date", pkg_name, available, current);
                    continue;
                }
                any_updates = true;
                let display_current = if current == "?" {
                    format!("? (constrained by {})", constraint)
                } else {
                    current.clone()
                };
                println!("  {}: {} → {}", pkg_name, display_current, available);
                updates.push((pkg_name.clone(), current, available));
            }
            None => {
                // Check if the package exists at all in the registry
                if registry.packages.contains_key(pkg_name) {
                    println!("  {} @ {} — up to date", pkg_name, constraint);
                } else {
                    println!("  {} — not found in registry", pkg_name);
                }
            }
        }
    }

    if !any_updates {
        println!("\nAll dependencies are up to date.");
        return Ok(());
    }

    if apply {
        // Apply updates to the manifest
        for (pkg_name, _current, available) in &updates {
            // Update the constraint to careted new version
            manifest.dependencies.insert(pkg_name.clone(), format!("^{}", available));
        }

        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| error::CompileError::new(format!("Failed to serialize manifest: {}", e)))?;
        fs::write(&manifest_path, json + "\n")
            .map_err(|e| error::CompileError::new(format!("Failed to write {}: {}", manifest_path.display(), e)))?;

        println!("\n✓ Updated {} package(s) in elysium.json", updates.len());
        println!("  Run `epm install` to update the lockfile and installed packages.");
    } else {
        println!("\nUse `--apply` to write these updates to elysium.json.");
    }

    Ok(())
}

/// Load the registry index from the EPM local cache.
/// Load the registry index from the EPM local cache.
pub fn load_registry() -> error::Result<RegistryIndex> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::CompileError::new("Cannot determine home directory"))?;
    let registry_cache = home.join(".epm").join(".epm-registry");
    let index_path = registry_cache.join("registry.json");

    if !index_path.exists() {
        // Try to sync the registry first
        sync_registry(&registry_cache)?;
    }

    let content = fs::read_to_string(&index_path)
        .map_err(|e| error::CompileError::new(format!("Cannot read registry index at {}: {}",
            index_path.display(), e)))?;

    // Handle both flat {"pkg": {...}} and nested {"packages": {"pkg": {...}}}
    if let Ok(index) = serde_json::from_str::<RegistryIndex>(&content) {
        return Ok(index);
    }
    // Fallback: try the nested format
    #[derive(serde::Deserialize)]
    struct Wrapper {
        packages: HashMap<String, RegistryEntry>,
    }
    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&content) {
        return Ok(RegistryIndex { packages: wrapper.packages });
    }

    Err(error::CompileError::new("Cannot parse registry index: expected {\"packages\": {...}}"))
}

/// Try to clone or pull the EPM registry cache.
fn sync_registry(cache_dir: &Path) -> error::Result<()> {
    let registry_url = "https://github.com/imstevetran/epm-registry.git";

    if cache_dir.exists() {
        // Pull
        let status = std::process::Command::new("git")
            .args(["-C", cache_dir.to_str().unwrap_or(""), "pull", "--ff-only", "--quiet"])
            .status()
            .map_err(|e| error::CompileError::new(format!("Git pull failed: {}", e)))?;
        if !status.success() {
            eprintln!("Warning: git pull in {} failed (stale cache)", cache_dir.display());
        }
    } else {
        // Clone
        if let Some(parent) = cache_dir.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| error::CompileError::new(format!("Cannot create {}: {}", parent.display(), e)))?;
        }
        let status = std::process::Command::new("git")
            .args(["clone", "--quiet", registry_url, cache_dir.to_str().unwrap_or("")])
            .status()
            .map_err(|e| error::CompileError::new(format!("Git clone failed: {}", e)))?;
        if !status.success() {
            return Err(error::CompileError::new("Failed to clone EPM registry"));
        }
    }
    Ok(())
}
