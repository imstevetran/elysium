use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::acl::{self, parse_scoped_package_name};
use crate::auth;
use crate::manifest::{Manifest, OrgEntry, RegistryEntry, RegistryIndex};

const REGISTRY_URL: &str = "https://github.com/imstevetran/epm-registry.git";
const REGISTRY_DIR_NAME: &str = ".epm-registry";
const REGISTRY_INDEX_FILE: &str = "registry.json";
const PACKAGES_DIR: &str = "packages";
const EPM_CACHE_DIR: &str = ".epm";

fn registry_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(EPM_CACHE_DIR).join(REGISTRY_DIR_NAME)
}

fn ensure_registry_cloned() -> Result<PathBuf, String> {
    let cache_dir = registry_cache_dir();

    if cache_dir.join(REGISTRY_INDEX_FILE).exists() {
        let status = std::process::Command::new("git")
            .args(["-C", cache_dir.to_str().unwrap(), "pull", "--rebase", "--quiet"])
            .status()
            .map_err(|e| format!("Failed to run git pull: {}", e))?;
        if !status.success() {
            return Err("git pull in registry cache failed".to_string());
        }
        Ok(cache_dir)
    } else {
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

fn read_registry_file(registry_dir: &Path) -> Result<RegistryIndex, String> {
    let index_path = registry_dir.join(REGISTRY_INDEX_FILE);
    let content = std::fs::read_to_string(&index_path)
        .map_err(|e| format!("Cannot read registry index: {}", e))?;
    RegistryIndex::from_json(&content)
}

fn write_and_push_registry(index: &RegistryIndex, registry_dir: &Path) -> Result<(), String> {
    let index_path = registry_dir.join(REGISTRY_INDEX_FILE);
    let json = index.to_json()?;
    std::fs::write(&index_path, json)
        .map_err(|e| format!("Cannot write registry index: {}", e))?;

    auth::with_git_credentials(|| {
        let git_add = std::process::Command::new("git")
            .args(["-C", registry_dir.to_str().unwrap(), "add", "."])
            .status()
            .map_err(|e| format!("git add: {}", e))?;
        if !git_add.success() {
            return Err("git add failed".to_string());
        }

        let _git_commit = std::process::Command::new("git")
            .args([
                "-C",
                registry_dir.to_str().unwrap(),
                "commit",
                "--allow-empty",
                "-m",
                "Update registry from epm",
            ])
            .status()
            .map_err(|e| format!("git commit: {}", e))?;

        let git_push = std::process::Command::new("git")
            .args(["-C", registry_dir.to_str().unwrap(), "push"])
            .status()
            .map_err(|e| format!("git push: {}", e))?;
        if !git_push.success() {
            return Err(
                "git push failed — sign in with `epm login` and ensure you can push to the registry"
                    .to_string(),
            );
        }
        Ok(())
    })
}

pub fn read_registry_index() -> Result<HashMap<String, RegistryEntry>, String> {
    let registry_dir = ensure_registry_cloned()?;
    Ok(read_registry_file(&registry_dir)?.packages)
}

fn read_full_registry() -> Result<(PathBuf, RegistryIndex), String> {
    let registry_dir = ensure_registry_cloned()?;
    let index = read_registry_file(&registry_dir)?;
    Ok((registry_dir, index))
}

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

/// Create a new org owned by `github_login`.
pub fn create_org(github_login: &str, org_slug: &str) -> Result<(), String> {
    acl::validate_slug(org_slug, "org")?;
    let (registry_dir, mut index) = read_full_registry()?;

    if index.orgs.contains_key(org_slug) {
        return Err(format!(
            "Org '@{}' already exists (owner: @{}).",
            org_slug,
            index.orgs[org_slug].owner
        ));
    }

    index.orgs.insert(
        org_slug.to_string(),
        OrgEntry {
            owner: github_login.to_string(),
            created_at: Some(Utc::now().to_rfc3339()),
        },
    );

    write_and_push_registry(&index, &registry_dir)?;
    println!("Created org @{} (owner: @{}).", org_slug, github_login);
    Ok(())
}

/// List orgs owned by or visible to the user.
pub fn list_orgs(github_login: &str) -> Result<Vec<(String, OrgEntry)>, String> {
    let index = read_full_registry()?.1;
    let mut owned: Vec<_> = index
        .orgs
        .into_iter()
        .filter(|(_, o)| o.owner.eq_ignore_ascii_case(github_login))
        .collect();
    owned.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(owned)
}

/// Grant a collaborator publish access on a package (owner only).
pub fn grant_collaborator(
    github_login: &str,
    package_name: &str,
    collaborator: &str,
) -> Result<(), String> {
    if collaborator.eq_ignore_ascii_case(github_login) {
        return Err("You cannot grant yourself — you already own the package.".to_string());
    }

    let (registry_dir, mut index) = read_full_registry()?;
    let entry = index
        .packages
        .get(package_name)
        .ok_or_else(|| format!("Package '{}' is not in the registry", package_name))?
        .clone();

    if !acl::can_grant(github_login, &entry) {
        return Err(format!(
            "Only package owner @{} can grant access to '{}'.",
            entry.owner, package_name
        ));
    }

    let mut entry = entry;
    entry
        .collaborators
        .insert(collaborator.to_string(), "publish".to_string());

    index.packages.insert(package_name.to_string(), entry);
    write_and_push_registry(&index, &registry_dir)?;
    println!(
        "Granted @{} publish access to {}.",
        collaborator, package_name
    );
    Ok(())
}

/// Publish a package tarball to the registry (requires GitHub login + ACL).
pub fn publish_package(
    github_login: &str,
    manifest: &Manifest,
    tarball_path: &Path,
) -> Result<(), String> {
    let (org_slug, _pkg_slug) = parse_scoped_package_name(&manifest.name)?;
    let (registry_dir, mut index) = read_full_registry()?;

    let packages_dir = registry_dir.join(PACKAGES_DIR);
    std::fs::create_dir_all(&packages_dir)
        .map_err(|e| format!("Cannot create packages dir: {}", e))?;

    let rel = tarball_rel_path(&manifest.name, &manifest.version);
    let dest = packages_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
    }
    std::fs::copy(tarball_path, &dest)
        .map_err(|e| format!("Cannot copy tarball to registry: {}", e))?;

    let is_new = !index.packages.contains_key(&manifest.name);

    if is_new {
        acl::can_create_package(github_login, &org_slug, &index.orgs)?;
    }

    let entry = index
        .packages
        .entry(manifest.name.clone())
        .or_insert_with(|| RegistryEntry {
        name: manifest.name.clone(),
        org: Some(org_slug.clone()),
        owner: github_login.to_string(),
        collaborators: HashMap::new(),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        license: manifest.license.clone(),
        repository: manifest.repository.clone(),
        versions: vec![],
    });

    if is_new {
        entry.owner = github_login.to_string();
        entry.org = Some(org_slug);
    } else if !acl::can_publish(github_login, entry) {
        return Err(format!(
            "You (@{}) do not have permission to publish '{}'. \
             Ask owner @{} to run: epm grant <your-github-login>",
            github_login, manifest.name, entry.owner
        ));
    }

    if entry.description.is_none() {
        entry.description = manifest.description.clone();
    }
    if entry.author.is_none() {
        entry.author = manifest.author.clone();
    }
    if entry.license.is_none() {
        entry.license = manifest.license.clone();
    }
    if entry.repository.is_none() {
        entry.repository = manifest.repository.clone();
    }

    if !entry.versions.contains(&manifest.version) {
        entry.versions.push(manifest.version.clone());
        entry.versions.sort_by(|a, b| {
            let a_parts: Vec<i32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
            let b_parts: Vec<i32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
            b_parts.cmp(&a_parts)
        });
    }

    write_and_push_registry(&index, &registry_dir)?;
    Ok(())
}

pub fn install_package(
    name: &str,
    version: Option<&str>,
    output_dir: &Path,
) -> Result<(), String> {
    let registry_dir = ensure_registry_cloned()?;
    let index = read_registry_file(&registry_dir)?;
    let entry = index
        .packages
        .get(name)
        .ok_or_else(|| format!("Package '{}' not found in registry", name))?;

    let ver = match version {
        Some(v) => v.to_string(),
        None => entry
            .versions
            .first()
            .ok_or_else(|| format!("No versions available for '{}'", name))?
            .clone(),
    };

    let rel = tarball_rel_path(name, &ver);
    let tarball_path = registry_dir.join(PACKAGES_DIR).join(&rel);

    if !tarball_path.exists() {
        return Err(format!(
            "Tarball {}/{} not found in registry (expected {})",
            name,
            ver,
            rel.display()
        ));
    }

    let tarball_file = std::fs::File::open(&tarball_path)
        .map_err(|e| format!("Cannot open tarball: {}", e))?;
    let decoder = flate2::read::GzDecoder::new(tarball_file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(output_dir)
        .map_err(|e| format!("Cannot extract tarball: {}", e))?;

    Ok(())
}

pub fn get_package_info(name: &str) -> Result<RegistryEntry, String> {
    let index = read_registry_index()?;
    index
        .get(name)
        .cloned()
        .ok_or_else(|| format!("Package '{}' not found in registry", name))
}

pub fn search_packages(query: &str) -> Result<Vec<RegistryEntry>, String> {
    let index = read_registry_index()?;
    let query_lower = query.to_lowercase();
    let results: Vec<RegistryEntry> = index
        .values()
        .filter(|e| {
            e.name.to_lowercase().contains(&query_lower)
                || e
                    .description
                    .as_deref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
        })
        .cloned()
        .collect();
    Ok(results)
}
