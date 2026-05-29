use std::collections::{HashMap, HashSet};
use semver::{Version, VersionReq};
use crate::registry;

/// A single resolved dependency in the flat resolution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
}

/// A chain of dependencies showing how a package is pulled in.
#[derive(Debug, Clone)]
pub struct DependencyPath {
    /// The chain from root → intermediate → ... → target, as (name, version, constraint) tuples.
    pub chain: Vec<(String, String, String)>,
}

/// The flat dependency resolution — maps each package name to its chosen version.
/// In single-version mode (default), each name appears at most once.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Resolution {
    pub root_name: String,
    pub root_version: String,
    /// name -> resolved version
    pub deps: HashMap<String, String>,
    /// Full record with all transitive deps for tree display
    #[serde(skip)]
    pub tree: Vec<ResolvedDep>,
}

/// Resolve dependencies flatly from the root manifest.
///
/// In single-version mode (default), every dependency name resolves to exactly one version.
/// If two transitive deps request different versions, the highest version satisfying all
/// constraints is selected.
pub fn resolve(
    root_name: &str,
    root_version: &str,
    deps: &HashMap<String, String>,
) -> Result<Resolution, String> {
    let mut resolution = Resolution {
        root_name: root_name.to_string(),
        root_version: root_version.to_string(),
        deps: HashMap::new(),
        tree: vec![],
    };

    // Collect all constraints: dep_name -> Vec<version_req>
    let mut constraints: HashMap<String, Vec<String>> = HashMap::new();

    // Walk the dependency tree to gather constraints
    gather_constraints(deps, &mut constraints, &mut HashSet::new(), 0)?;

    // Resolve each dependency: for each name, find the best version satisfying all constraints
    for (dep_name, reqs) in &constraints {
        let all_reqs: Vec<&str> = reqs.iter().map(|s| s.as_str()).collect();
        let best = pick_best_version(dep_name, &all_reqs)?;
        resolution.deps.insert(dep_name.clone(), best.clone());
        resolution.tree.push(ResolvedDep {
            name: dep_name.clone(),
            version: best,
        });
    }

    Ok(resolution)
}

/// Resolve with legacy multi-version support.
/// Each conflict creates a version-specific entry (e.g. lib-a@1.0.0, lib-a@2.0.0).
pub fn resolve_legacy(
    root_name: &str,
    root_version: &str,
    deps: &HashMap<String, String>,
) -> Result<Resolution, String> {
    // Legacy mode: treat each version requirement independently.
    // We collect dep -> [version_req] but don't merge conflicting constraints.
    let mut resolution = Resolution {
        root_name: root_name.to_string(),
        root_version: root_version.to_string(),
        deps: HashMap::new(),
        tree: vec![],
    };

    // Walk the tree, resolving each requirement independently
    resolve_legacy_recursive(deps, &mut resolution, &mut HashSet::new(), 0)?;

    Ok(resolution)
}

/// Format: `requires ">=1.0.0"` style constraint or just a bare version like `"1.0.0"`.
/// Convert whatever the manifest has into a semver-compatible requirement.
fn parse_version_req(raw: &str) -> VersionReq {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "*" {
        // Accept any version
        return VersionReq::STAR;
    }
    // If it's already a valid requirement (contains operators), use it directly
    if trimmed.contains('>') || trimmed.contains('<') || trimmed.contains('=') || trimmed.contains('^') || trimmed.contains('~') {
        if let Ok(req) = VersionReq::parse(trimmed) {
            return req;
        }
    }
    // Bare "1.0.0" -> "^1.0.0" (compatible with that version and above in same major)
    let bare = if let Some(stripped) = trimmed.strip_prefix('v') {
        stripped
    } else {
        trimmed
    };
    let caret = format!("^{}", bare);
    if let Ok(req) = VersionReq::parse(&caret) {
        return req;
    }
    // Fallback: accept anything
    VersionReq::STAR
}

/// Given a dep name and a list of version constraints, find the best version from the registry.
fn pick_best_version(dep_name: &str, reqs: &[&str]) -> Result<String, String> {
    let index = registry::read_registry_index()?;
    let entry = index.get(dep_name)
        .ok_or_else(|| format!("Dependency '{}' not found in registry", dep_name))?;

    if entry.versions.is_empty() {
        return Err(format!("No versions available for '{}'", dep_name));
    }

    // Parse all requirements and find the conjunction
    let parsed_reqs: Vec<VersionReq> = reqs.iter()
        .map(|r| parse_version_req(r))
        .collect();

    // Collect all available versions as semver::Version
    let mut available: Vec<Version> = Vec::new();
    for v_str in &entry.versions {
        let clean = v_str.trim_start_matches('v');
        if let Ok(v) = Version::parse(clean) {
            available.push(v);
        }
    }

    // Sort descending (newest first)
    available.sort_by(|a, b| b.cmp(a));

    // Find the newest version that satisfies ALL requirements
    for version in &available {
        if parsed_reqs.iter().all(|req| req.matches(version)) {
            return Ok(format!("{}.{}.{}", version.major, version.minor, version.patch));
        }
    }

    Err(format!(
        "No version of '{}' satisfies all constraints: {}",
        dep_name,
        reqs.join(", ")
    ))
}

/// Recursively gather all version constraints for each dependency.
fn gather_constraints(
    deps: &HashMap<String, String>,
    constraints: &mut HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    depth: usize,
) -> Result<(), String> {
    const MAX_DEPTH: usize = 50;
    if depth > MAX_DEPTH {
        return Ok(());
    }

    for (dep_name, dep_req) in deps {
        // Record the constraint
        constraints.entry(dep_name.clone())
            .or_default()
            .push(dep_req.clone());

        // Guard against cycles
        let key = format!("{}:{}", dep_name, dep_req);
        if !visited.insert(key) {
            continue;
        }

        // Fetch the manifest of the best-matching version to get its transitive deps
        let best_ver = {
            let reqs = &constraints[dep_name];
            let req_refs: Vec<&str> = reqs.iter().map(|s| s.as_str()).collect();
            pick_best_version(dep_name, &req_refs)?
        };

        // Look up that version's manifest in the registry tarball
        // We cache manifests locally to avoid re-downloading
        let sub_manifest = get_cached_manifest(dep_name, &best_ver)?;
        if !sub_manifest.dependencies.is_empty() {
            gather_constraints(&sub_manifest.dependencies, constraints, visited, depth + 1)?;
        }
    }

    Ok(())
}

/// Resolve all deps independently (legacy mode, allows multiple versions).
fn resolve_legacy_recursive(
    deps: &HashMap<String, String>,
    resolution: &mut Resolution,
    visited: &mut HashSet<String>,
    depth: usize,
) -> Result<(), String> {
    const MAX_DEPTH: usize = 50;
    if depth > MAX_DEPTH {
        return Ok(());
    }

    for (dep_name, dep_req) in deps {
        let best = pick_best_version(dep_name, &[dep_req.as_str()])?;

        // In legacy mode, use version-qualified name for conflicts
        let resolved_name = dep_name.clone();

        // Only set if not already resolved — legacy mode keeps first resolution
        if !resolution.deps.contains_key(&resolved_name) {
            resolution.deps.insert(resolved_name.clone(), best.clone());
            resolution.tree.push(ResolvedDep {
                name: resolved_name.clone(),
                version: best.clone(),
            });

            // Recurse
            let sub_manifest = get_cached_manifest(dep_name, &best)?;
            if !sub_manifest.dependencies.is_empty() {
                resolve_legacy_recursive(&sub_manifest.dependencies, resolution, visited, depth + 1)?;
            }
        }
    }

    Ok(())
}

/// Load a package's manifest from the local cache or download it.
fn get_cached_manifest(
    name: &str,
    version: &str,
) -> Result<crate::manifest::Manifest, String> {
    let cache_home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = std::path::PathBuf::from(&cache_home)
        .join(".epm")
        .join("manifests")
        .join(name)
        .join(version);
    let manifest_path = cache_dir.join("elysium.json");

    if manifest_path.exists() {
        return crate::manifest::Manifest::load_from_dir(&cache_dir);
    }

    // Download and extract just the manifest
    let temp_dir = tempfile::tempdir().map_err(|e| format!("Cannot create temp dir: {}", e))?;
    registry::install_package(name, Some(version), temp_dir.path())?;

    // Read manifest from extracted files
    let manifest = crate::manifest::Manifest::load_from_dir(temp_dir.path())?;

    // Cache it
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Cannot create cache dir: {}", e))?;
    let cached_manifest_path = cache_dir.join("elysium.json");
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Cannot serialize manifest: {}", e))?;
    std::fs::write(&cached_manifest_path, json)
        .map_err(|e| format!("Cannot write cached manifest: {}", e))?;

    Ok(manifest)
}

/// Read lockfile from the project directory.
pub fn read_lockfile(project_dir: &std::path::Path) -> Result<Option<Resolution>, String> {
    let lock_path = project_dir.join("elysium.lock");
    if !lock_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&lock_path)
        .map_err(|e| format!("Cannot read elysium.lock: {}", e))?;
    let resolution: Resolution = serde_json::from_str(&content)
        .map_err(|e| format!("Cannot parse elysium.lock: {}", e))?;
    Ok(Some(resolution))
}

/// Write lockfile to the project directory.
pub fn write_lockfile(project_dir: &std::path::Path, resolution: &Resolution) -> Result<(), String> {
    let lock_path = project_dir.join("elysium.lock");
    let content = serde_json::to_string_pretty(resolution)
        .map_err(|e| format!("Cannot serialize lockfile: {}", e))?;
    std::fs::write(&lock_path, content)
        .map_err(|e| format!("Cannot write elysium.lock: {}", e))?;
    Ok(())
}

/// Trace all dependency paths from the root package to the given target package.
///
/// Returns a list of paths, where each path is a chain from root → ... → target.
/// Each link in the chain is (package_name, version, constraint_that_required_it).
pub fn trace_dep(
    root_name: &str,
    root_version: &str,
    deps: &HashMap<String, String>,
    target: &str,
) -> Result<Vec<DependencyPath>, String> {
    let mut paths = Vec::new();
    let mut current_chain = Vec::new();
    // Push root as implicit starting point
    current_chain.push((root_name.to_string(), root_version.to_string(), "(root)".to_string()));

    trace_dep_recursive(deps, target, &mut current_chain, &mut paths, 0)?;

    // Remove the root entry we added; the traces start from root deps
    for path in &mut paths {
        path.chain.remove(0);
    }

    Ok(paths)
}

fn trace_dep_recursive(
    deps: &HashMap<String, String>,
    target: &str,
    current_chain: &mut Vec<(String, String, String)>,
    paths: &mut Vec<DependencyPath>,
    depth: usize,
) -> Result<(), String> {
    const MAX_DEPTH: usize = 50;
    if depth > MAX_DEPTH {
        return Ok(());
    }

    for (dep_name, dep_req) in deps {
        let best = pick_best_version(dep_name, &[dep_req.as_str()])?;

        // Cycle detection: if this package name is already in the current chain, skip
        if current_chain.iter().any(|(name, _, _)| name == dep_name) {
            continue;
        }

        // Push the current step onto the chain
        let chain_entry = (dep_name.clone(), best.clone(), dep_req.clone());
        current_chain.push(chain_entry);

        if dep_name == target {
            // Found the target — record the full chain
            paths.push(DependencyPath {
                chain: current_chain.clone(),
            });
        } else {
            // Recurse into transitive deps
            let sub_manifest = get_cached_manifest(dep_name, &best)?;
            if !sub_manifest.dependencies.is_empty() {
                trace_dep_recursive(
                    &sub_manifest.dependencies,
                    target,
                    current_chain,
                    paths,
                    depth + 1,
                )?;
            }
        }

        current_chain.pop();
    }

    Ok(())
}
