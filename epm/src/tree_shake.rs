use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// A node in the dependency tree.
#[derive(Debug, Clone)]
pub struct DepNode {
    pub name: String,
    pub version: String,
    pub dependencies: HashMap<String, String>,
    pub children: Vec<DepNode>,
}

/// Build a dependency tree starting from the given manifest.
pub fn build_dep_tree(
    manifest: &crate::manifest::Manifest,
    deps_dir: &Path,
) -> DepNode {
    let children = build_children(deps_dir, &manifest.dependencies, 0);
    DepNode {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        dependencies: manifest.dependencies.clone(),
        children,
    }
}

fn build_children(
    deps_dir: &Path,
    deps: &HashMap<String, String>,
    depth: usize,
) -> Vec<DepNode> {
    const MAX_DEPTH: usize = 20;
    if depth > MAX_DEPTH {
        return vec![];
    }

    let mut children = Vec::new();
    for (dep_name, _dep_ver) in deps {
        let pkg_dir = deps_dir.join(dep_name);
        if !pkg_dir.exists() {
            continue;
        }

        let sub_manifest = match crate::manifest::Manifest::load_from_dir(&pkg_dir) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let grand_children = build_children(deps_dir, &sub_manifest.dependencies, depth + 1);
        children.push(DepNode {
            name: dep_name.clone(),
            version: sub_manifest.version.clone(),
            dependencies: sub_manifest.dependencies.clone(),
            children: grand_children,
        });
    }
    children
}

/// Print a dependency tree to stdout.
pub fn print_tree(node: &DepNode, indent: usize) {
    let prefix = "  ".repeat(indent);
    let deps_str = if node.dependencies.is_empty() {
        String::new()
    } else {
        let dep_names: Vec<&str> = node.dependencies.keys().map(|s| s.as_str()).collect();
        format!(" ({})", dep_names.join(", "))
    };
    println!("{}├── {}@{}{}", prefix, node.name, node.version, deps_str);

    for child in &node.children {
        print_tree(child, indent + 1);
    }
}

// ==================== Tree-Shaking ====================

/// Collect all `.ely` files that are reachable from the entry points,
/// following `import` statements.
pub fn collect_reachable_files(root_dir: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();

    // Find all entry .ely files in the root (not in elysium_modules)
    let entries = find_entry_files(root_dir)?;
    for entry in entries {
        queue.push_back(entry);
    }

    while let Some(path) = queue.pop_front() {
        if reachable.contains(&path) {
            continue;
        }
        reachable.insert(path.clone());

        // Read the file and find imports
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let found_imports = find_imports_in_source(&content, &path, root_dir)?;
        for imported in found_imports {
            if !reachable.contains(&imported) {
                queue.push_back(imported);
            }
        }
    }

    Ok(reachable)
}

/// Find all `.ely` files in the root dir (excluding elysium_modules and hidden dirs).
fn find_entry_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_ely_files(dir, dir, &mut files)?;
    Ok(files)
}

fn collect_ely_files(base: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Cannot read entry: {}", e))?;
        let path = entry.path();
        let file_name = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Skip hidden dirs, elysium_modules, .git, etc.
        if file_name.starts_with('.') || file_name == "elysium_modules" || file_name == "target" {
            continue;
        }

        if path.is_dir() {
            collect_ely_files(base, &path, files)?;
        } else if path.extension().map(|e| e == "ely" || e == "elyx").unwrap_or(false) {
            files.push(path);
        }
    }
    Ok(())
}

/// Parse a source file and extract all `import "..."` or `import "..." as ...` paths.
/// Returns resolved absolute paths.
fn find_imports_in_source(
    source: &str,
    source_path: &Path,
    root_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let source_dir = source_path.parent().unwrap_or(root_dir);

    for line in source.lines() {
        let trimmed = line.trim();
        // Match: import "./foo.ely" or import "./foo.ely" as alias
        if trimmed.starts_with("import ") && trimmed.contains('"') {
            if let Some(start) = trimmed.find('"') {
                if let Some(end) = trimmed[start + 1..].find('"') {
                    let import_path = &trimmed[start + 1..start + 1 + end];
                    // Skip registry-style imports (bare names, no ./ or ../)
                    if import_path.starts_with("./") || import_path.starts_with("../") {
                        let resolved = source_dir.join(import_path);
                        // Try with .ely extension if missing
                        let candidate = if resolved.exists() {
                            resolved
                        } else {
                            let with_ext = source_dir.join(format!("{}.ely", import_path.trim_end_matches(".elyx")));
                            if with_ext.exists() { with_ext } else { continue; }
                        };
                        paths.push(candidate);
                    }
                }
            }
        }
    }

    Ok(paths)
}

/// Perform tree-shaking on elysium_modules: remove any .ely files not reachable
/// from each package's entry point.
pub fn shake_packages(
    deps_dir: &Path,
    dry_run: bool,
) -> Result<ShakeReport, String> {
    let mut report = ShakeReport {
        scanned_files: 0,
        kept_files: 0,
        removed_files: 0,
        files_removed: vec![],
    };

    if !deps_dir.exists() {
        return Ok(report);
    }

    for entry in std::fs::read_dir(deps_dir).map_err(|e| format!("Cannot read deps dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Cannot read entry: {}", e))?;
        let pkg_dir = entry.path();
        if !pkg_dir.is_dir() {
            continue;
        }

        // Read manifest to find entry point
        let manifest_path = pkg_dir.join("elysium.json");
        let entry_file = if manifest_path.exists() {
            match crate::manifest::Manifest::load_from_dir(&pkg_dir) {
                Ok(m) => m.entry.unwrap_or_else(|| "main.ely".to_string()),
                Err(_) => continue,
            }
        } else {
            continue;
        };

        // Collect all .ely files in this package
        let mut all_files = Vec::new();
        collect_ely_files(&pkg_dir, &pkg_dir, &mut all_files)?;

        // Make the entry file absolute
        let entry_path = pkg_dir.join(&entry_file);
        if !entry_path.exists() {
            // Entry not found, keep everything to be safe
            report.scanned_files += all_files.len();
            report.kept_files += all_files.len();
            continue;
        }

        // Build a temporary file list that only includes files from THIS package dir
        let pkg_files: HashSet<PathBuf> = all_files.iter().cloned().collect();

        // Find reachable files starting from the entry
        let reachable = compute_reachable_from_entry(&entry_path, &pkg_files, &pkg_dir)?;

        // Remove unreachable files
        for file_path in &all_files {
            report.scanned_files += 1;
            if reachable.contains(file_path) {
                report.kept_files += 1;
            } else if !dry_run {
                std::fs::remove_file(file_path)
                    .map_err(|e| format!("Cannot remove {}: {}", file_path.display(), e))?;
                report.removed_files += 1;
                report.files_removed.push(file_path.clone());
            } else {
                report.removed_files += 1;
                report.files_removed.push(file_path.clone());
            }
        }
    }

    Ok(report)
}

/// Starting from `entry_path`, follow all `import` statements to find reachable `.ely` files.
/// Only considers files within `valid_files` (i.e. within this package).
fn compute_reachable_from_entry(
    entry_path: &Path,
    valid_files: &HashSet<PathBuf>,
    root_dir: &Path,
) -> Result<HashSet<PathBuf>, String> {
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(entry_path.to_path_buf());

    while let Some(path) = queue.pop_front() {
        if reachable.contains(&path) || !valid_files.contains(&path) {
            continue;
        }
        reachable.insert(path.clone());

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let imports = find_imports_in_source(&content, &path, root_dir)?;
        for imp in imports {
            if valid_files.contains(&imp) && !reachable.contains(&imp) {
                queue.push_back(imp);
            }
        }
    }

    Ok(reachable)
}

#[derive(Debug, Clone)]
pub struct ShakeReport {
    pub scanned_files: usize,
    pub kept_files: usize,
    pub removed_files: usize,
    pub files_removed: Vec<PathBuf>,
}
