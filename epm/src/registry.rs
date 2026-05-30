use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::manifest::{Manifest, RegistryEntry, RegistryIndex};

const REGISTRY_URL: &str = "https://github.com/imstevetran/epm-registry.git";
const REGISTRY_DIR_NAME: &str = ".epm-registry";
const REGISTRY_INDEX_FILE: &str = "registry.json";
const PACKAGES_DIR: &str = "packages";
const EPM_CACHE_DIR: &str = ".epm";

/// Resolve the local cache directory for the registry clone.
fn registry_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(EPM_CACHE_DIR).join(REGISTRY_DIR_NAME)
}

/// Clone (or pull) the registry into a temp directory, read it, return the path.
/// We use a persistent cache in ~/.epm/.epm-registry/ to avoid re-cloning every time.
fn ensure_registry_cloned() -> Result<PathBuf, String> {
    let cache_dir = registry_cache_dir();

    if cache_dir.join("registry.json").exists() {
        // Already cloned — do a git pull to refresh
        let status = std::process::Command::new("git")
            .args(["-C", cache_dir.to_str().unwrap(), "pull", "--rebase", "--quiet"])
            .status()
            .map_err(|e| format!("Failed to run git pull: {}", e))?;
        if !status.success() {
            return Err("git pull in registry cache failed".to_string());
        }
        Ok(cache_dir)
    } else {
        // First-time clone
        if let Some(parent) = cache_dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create cache dir: {}", e))?;
        }
        let status = std::process::Command::new("git")
            .args(["clone", "--depth", "1", REGISTRY_URL, cache_dir.to_str().unwrap()])
            .status()
            .map_err(|e| format!("Failed to clone registry: {}", e))?;
        if !status.success() {
            return Err("git clone of registry failed".to_string());
        }
        Ok(cache_dir)
    }
}

/// Read the registry index from the cloned registry.
pub fn read_registry_index() -> Result<HashMap<String, RegistryEntry>, String> {
    let registry_dir = ensure_registry_cloned()?;
    let index_path = registry_dir.join(REGISTRY_INDEX_FILE);
    let content = std::fs::read_to_string(&index_path)
        .map_err(|e| format!("Cannot read registry index: {}", e))?;
    // The index is stored as {"packages": {...}}
    let index_wrapper: RegistryIndex = serde_json::from_str(&content)
        .map_err(|e| format!("Cannot parse registry index: {}", e))?;
    Ok(index_wrapper.packages)
}

/// Write the registry index back to the cloned registry and push.
fn write_and_push_registry(index: &HashMap<String, RegistryEntry>, registry_dir: &Path) -> Result<(), String> {
    let index_path = registry_dir.join(REGISTRY_INDEX_FILE);
    // Serialize with the {"packages": ...} wrapper
    let index_wrapper = RegistryIndex {
        packages: index.clone(),
    };
    let json = serde_json::to_string_pretty(&index_wrapper)
        .map_err(|e| format!("Cannot serialize registry: {}", e))?;
    std::fs::write(&index_path, json)
        .map_err(|e| format!("Cannot write registry index: {}", e))?;

    // Git commit and push
    let git_add = std::process::Command::new("git")
        .args(["-C", registry_dir.to_str().unwrap(), "add", "."])
        .status()
        .map_err(|e| format!("git add: {}", e))?;
    if !git_add.success() {
        return Err("git add failed".to_string());
    }

    let _git_commit = std::process::Command::new("git")
        .args(["-C", registry_dir.to_str().unwrap(), "commit", "--allow-empty", "-m", "Update registry from epm"])
        .status()
        .map_err(|e| format!("git commit: {}", e))?;

    // Push — this may prompt for credentials
    let git_push = std::process::Command::new("git")
        .args(["-C", registry_dir.to_str().unwrap(), "push"])
        .status()
        .map_err(|e| format!("git push: {}", e))?;
    if !git_push.success() {
        return Err("git push failed — make sure you have push access to the registry repo".to_string());
    }

    Ok(())
}

/// Determine the relative path for a package tarball within the packages directory.
/// Scoped packages (e.g. @org/name) are stored in a subdirectory: packages/@org/name-0.1.0.tar.gz.
fn tarball_rel_path(name: &str, version: &str) -> PathBuf {
    if let Some(scope) = name.strip_prefix('@') {
        if let Some(slash) = scope.find('/') {
            let org = &scope[..slash];
            let pkg = &scope[slash + 1..];
            return PathBuf::from(format!("@{}/", org)).join(format!("{}-{}.tar.gz", pkg, version));
        }
    }
    PathBuf::from(format!("{}-{}.tar.gz", name, version))
}

/// Publish a package tarball to the registry.
pub fn publish_package(
    manifest: &Manifest,
    tarball_path: &Path,
) -> Result<(), String> {
    let registry_dir = ensure_registry_cloned()?;

    // Copy tarball into the registry's packages/ directory
    let packages_dir = registry_dir.join(PACKAGES_DIR);
    std::fs::create_dir_all(&packages_dir)
        .map_err(|e| format!("Cannot create packages dir: {}", e))?;

    let rel = tarball_rel_path(&manifest.name, &manifest.version);
    let dest = packages_dir.join(&rel);
    // Ensure parent directory for scoped packages (e.g. packages/@org/)
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
    }
    std::fs::copy(tarball_path, &dest)
        .map_err(|e| format!("Cannot copy tarball to registry: {}", e))?;

    // Update registry index
    let mut index = read_registry_index()?;
    let entry = index.entry(manifest.name.clone()).or_insert(RegistryEntry {
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        license: manifest.license.clone(),
        repository: manifest.repository.clone(),
        versions: vec![],
    });
    if !entry.versions.contains(&manifest.version) {
        entry.versions.push(manifest.version.clone());
        entry.versions.sort_by(|a, b| {
            let a_parts: Vec<i32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
            let b_parts: Vec<i32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
            b_parts.cmp(&a_parts) // newest first
        });
    }

    write_and_push_registry(&index, &registry_dir)?;

    Ok(())
}

/// Download a package tarball from the registry and extract it into the given output directory.
pub fn install_package(
    name: &str,
    version: Option<&str>,
    output_dir: &Path,
) -> Result<(), String> {
    let registry_dir = ensure_registry_cloned()?;

    // Read index to find versions
    let index = read_registry_index()?;
    let entry = index.get(name)
        .ok_or_else(|| format!("Package '{}' not found in registry", name))?;

    let ver = match version {
        Some(v) => v.to_string(),
        None => entry.versions.first()
            .ok_or_else(|| format!("No versions available for '{}'", name))?
            .clone(),
    };

    let rel = tarball_rel_path(name, &ver);
    let tarball_path = registry_dir.join(PACKAGES_DIR).join(&rel);

    if !tarball_path.exists() {
        return Err(format!("Tarball {}/{} not found in registry (expected {})", name, ver, rel.display()));
    }

    // Extract tarball into output_dir
    let tarball_file = std::fs::File::open(&tarball_path)
        .map_err(|e| format!("Cannot open tarball: {}", e))?;
    let decoder = flate2::read::GzDecoder::new(tarball_file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(output_dir)
        .map_err(|e| format!("Cannot extract tarball: {}", e))?;

    // Move contents up if they're in a single directory
    // (tarballs are typically created from a package dir)
    // We'll just leave them as-is; the caller uses --save to update elysium.json

    Ok(())
}

/// Get info about a package from the registry.
pub fn get_package_info(name: &str) -> Result<RegistryEntry, String> {
    let index = read_registry_index()?;
    index.get(name)
        .cloned()
        .ok_or_else(|| format!("Package '{}' not found in registry", name))
}

/// Search packages by query.
pub fn search_packages(query: &str) -> Result<Vec<RegistryEntry>, String> {
    let index = read_registry_index()?;
    let query_lower = query.to_lowercase();
    let results: Vec<RegistryEntry> = index.values()
        .filter(|e| {
            e.name.to_lowercase().contains(&query_lower)
                || e.description.as_deref().map(|d| d.to_lowercase().contains(&query_lower)).unwrap_or(false)
        })
        .cloned()
        .collect();
    Ok(results)
}
