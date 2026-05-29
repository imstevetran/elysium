mod cli;
mod manifest;
mod registry;
mod tree_shake;

use clap::Parser;
use std::path::PathBuf;

fn main() {
    let cli = cli::Cli::parse();

    let result = match &cli.command {
        cli::Commands::Init { name, version, description, author, license, force } => {
            cmd_init(name, version, description, author, license, *force)
        }
        cli::Commands::Install { package, version, save, shake } => {
            cmd_install(package.as_deref(), version.as_deref(), *save, *shake)
        }
        cli::Commands::Publish { registry: _ } => {
            cmd_publish()
        }
        cli::Commands::Search { query } => {
            cmd_search(query)
        }
        cli::Commands::Info { package } => {
            cmd_info(package)
        }
        cli::Commands::Tree { all } => {
            cmd_tree(*all)
        }
        cli::Commands::Shake { dry_run } => {
            cmd_shake(*dry_run)
        }
        cli::Commands::Login { token } => {
            cmd_login(token)
        }
        cli::Commands::List => {
            cmd_list()
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

// =============== Commands ===============

fn cmd_init(
    name: &Option<String>,
    version: &str,
    description: &Option<String>,
    author: &Option<String>,
    license: &Option<String>,
    force: bool,
) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest_path = cwd.join("elysium.json");

    if manifest_path.exists() && !force {
        return Err(format!("{} already exists. Use --force to overwrite.", manifest_path.display()));
    }

    let package_name = name.clone().unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("package")
            .to_string()
    });

    let mut m = manifest::Manifest::new(&package_name, version);
    m.description = description.clone();
    m.author = author.clone();
    m.license = license.clone();
    m.entry = Some("main.ely".to_string());

    m.save_to_dir(&cwd)?;
    println!("Created {}", manifest_path.display());

    // Create a default main.ely if it doesn't exist
    let main_path = cwd.join("main.ely");
    if !main_path.exists() {
        std::fs::write(&main_path, format!("// Welcome to {package_name}!\n\nfunc main() {{\n    let message = \"Hello, Elysium!\"\n    _ = message\n}}\n", package_name = package_name))
            .map_err(|e| format!("Cannot create main.ely: {}", e))?;
        println!("Created {}", main_path.display());
    }

    Ok(())
}

fn cmd_install(
    package: Option<&str>,
    version: Option<&str>,
    save: bool,
    shake: bool,
) -> Result<(), String> {
    match package {
        None => {
            // Install all dependencies from elysium.json
            let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
            let manifest = manifest::Manifest::load_from_dir(&cwd)?;

            if manifest.dependencies.is_empty() {
                println!("No dependencies to install.");
                return Ok(());
            }

            let deps_dir = cwd.join("elysium_modules");
            std::fs::create_dir_all(&deps_dir)
                .map_err(|e| format!("Cannot create elysium_modules dir: {}", e))?;

            let total = manifest.dependencies.len();
            for (i, (dep_name, dep_ver)) in manifest.dependencies.iter().enumerate() {
                println!("[{}/{}] Installing {}@{} ...", i + 1, total, dep_name, dep_ver);
                let target_dir = deps_dir.join(dep_name);
                let ver = if dep_ver.is_empty() || dep_ver == "*" { None } else { Some(dep_ver.as_str()) };
                registry::install_package(dep_name, ver, &target_dir)?;
                println!("  -> installed to {}", target_dir.display());
            }

            println!("All dependencies installed.");

            if shake {
                println!("\nTree-shaking installed packages...");
                let report = tree_shake::shake_packages(&deps_dir, false)?;
                println!("  Removed {} unused file(s).", report.removed_files);
            }

            Ok(())
        }
        Some(pkg) => {
            // Install a specific package
            let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
            let deps_dir = cwd.join("elysium_modules");
            std::fs::create_dir_all(&deps_dir)
                .map_err(|e| format!("Cannot create elysium_modules dir: {}", e))?;

            let target_dir = deps_dir.join(pkg);
            let ver = version.filter(|v| !v.is_empty() && *v != "*");
            let display_ver = ver.unwrap_or("latest");

            println!("Installing {}@{} ...", pkg, display_ver);
            registry::install_package(pkg, ver, &target_dir)?;
            println!("  -> installed to {}", target_dir.display());

            if save {
                let mut manifest = manifest::Manifest::load_from_dir(&cwd)?;
                manifest.dependencies.insert(pkg.to_string(), ver.unwrap_or("*").to_string());
                manifest.save_to_dir(&cwd)?;
                println!("  -> saved to elysium.json");
            }

            if shake {
                println!("\nTree-shaking installed packages...");
                let report = tree_shake::shake_packages(&deps_dir, false)?;
                println!("  Removed {} unused file(s).", report.removed_files);
            }

            Ok(())
        }
    }
}

fn cmd_publish() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;

    // Read token
    let token = load_token()?;

    // Validate manifest fields
    if manifest.name.is_empty() {
        return Err("Package name is required in elysium.json".to_string());
    }
    if manifest.version.is_empty() {
        return Err("Package version is required in elysium.json".to_string());
    }

    println!("Publishing {} v{} ...", manifest.name, manifest.version);

    // Check that entry file exists
    let entry_file = manifest.entry.as_deref().unwrap_or("main.ely");
    let entry_path = cwd.join(entry_file);
    if !entry_path.exists() {
        return Err(format!("Entry file '{}' not found", entry_file));
    }

    // Create a temporary directory and build tarball
    let temp_dir = tempfile::tempdir().map_err(|e| format!("Cannot create temp dir: {}", e))?;
    let tarball_path = temp_dir.path().join(format!("{}-{}.tar.gz", manifest.name, manifest.version));

    // Create tarball of the package dir (excluding elysium_modules, .git, etc.)
    let tar_file = std::fs::File::create(&tarball_path)
        .map_err(|e| format!("Cannot create tarball: {}", e))?;
    let encoder = flate2::write::GzEncoder::new(tar_file, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);

    // Walk the current directory and add files
    // Exclude common patterns
    let excludes = [
        "elysium_modules",
        ".git",
        ".epm",
        "target",
        "Cargo.lock",
    ];

    add_dir_to_tar(&cwd, &cwd, &mut archive, &excludes)
        .map_err(|e| format!("Cannot create tarball: {}", e))?;

    let encoder = archive.into_inner().map_err(|e| format!("{}", e))?;
    encoder.finish().map_err(|e| format!("{}", e))?;

    println!("Created tarball: {}", tarball_path.display());

    // Publish to registry
    // Configure git credential helper with token
    let token_dir = registry_cache_dir();
    std::fs::create_dir_all(&token_dir).map_err(|e| format!("Cannot create cache dir: {}", e))?;

    // Temporarily set GIT_ASKPASS to a script that returns the token
    std::env::set_var("EPM_GIT_TOKEN", &token);

    // Write a temporary askpass script that outputs the token
    let askpass_path = token_dir.join("git-askpass.sh");
    std::fs::write(&askpass_path, format!(
        "#!/bin/sh\necho \"${{EPM_GIT_TOKEN}}\""
    )).map_err(|e| format!("Cannot write askpass: {}", e))?;

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&askpass_path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Cannot set executable: {}", e))?;

    std::env::set_var("GIT_ASKPASS", askpass_path.to_str().unwrap());

    registry::publish_package(&manifest, &tarball_path)?;

    println!("Published {} v{} to registry!", manifest.name, manifest.version);
    Ok(())
}

fn cmd_search(query: &str) -> Result<(), String> {
    let results = registry::search_packages(query)?;

    if results.is_empty() {
        println!("No packages found matching '{}'", query);
        return Ok(());
    }

    println!("Search results for '{}':", query);
    println!("{:-<60}", "");
    for entry in &results {
        let desc = entry.description.as_deref().unwrap_or("(no description)");
        let latest = entry.versions.first().map(|v| v.as_str()).unwrap_or("-");
        println!("  {}@{}", entry.name, latest);
        println!("    {}", desc);
    }
    println!("{:-<60}", "");

    Ok(())
}

fn cmd_info(package: &str) -> Result<(), String> {
    let entry = registry::get_package_info(package)?;

    println!("Package: {}", entry.name);
    println!("Description: {}", entry.description.as_deref().unwrap_or("(none)"));
    println!("Author: {}", entry.author.as_deref().unwrap_or("(none)"));
    println!("License: {}", entry.license.as_deref().unwrap_or("(none)"));
    println!("Repository: {}", entry.repository.as_deref().unwrap_or("(none)"));
    println!("Versions:");
    for v in &entry.versions {
        println!("  - {}", v);
    }

    Ok(())
}

fn cmd_tree(_all: bool) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;
    let deps_dir = cwd.join("elysium_modules");

    if !deps_dir.exists() {
        println!("No dependencies installed. Run `epm install` first.");
        return Ok(());
    }

    let tree = tree_shake::build_dep_tree(&manifest, &deps_dir);
    println!("Dependency tree for {}@{}:", manifest.name, manifest.version);
    for child in &tree.children {
        tree_shake::print_tree(child, 0);
    }

    if tree.children.is_empty() {
        println!("(no dependencies)");
    }
    Ok(())
}

fn cmd_shake(dry_run: bool) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let deps_dir = cwd.join("elysium_modules");

    if !deps_dir.exists() {
        println!("No dependencies installed. Run `epm install` first.");
        return Ok(());
    }

    if dry_run {
        println!("Dry run — no files will be deleted.\n");
    }

    let report = tree_shake::shake_packages(&deps_dir, dry_run)?;

    println!("Tree-shaking report:");
    println!("  Scanned: {} file(s)", report.scanned_files);
    println!("  Kept:    {} file(s)", report.kept_files);
    println!("  Removed: {} file(s)", report.removed_files);

    if !report.files_removed.is_empty() {
        println!("\nFiles:");
        for f in &report.files_removed {
            let display = f.strip_prefix(&deps_dir).unwrap_or(f);
            println!("  - {}", display.display());
        }
    }

    if dry_run && report.removed_files > 0 {
        println!("\nRun `epm shake` (without --dry-run) to actually remove these files.");
    }

    Ok(())
}

fn cmd_login(token: &str) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let epm_dir = PathBuf::from(&home).join(".epm");
    std::fs::create_dir_all(&epm_dir)
        .map_err(|e| format!("Cannot create .epm directory: {}", e))?;

    let token_path = epm_dir.join("token");
    // Store token with restricted permissions (readable only by user)
    std::fs::write(&token_path, format!("{}\n", token))
        .map_err(|e| format!("Cannot write token: {}", e))?;

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Cannot set token file permissions: {}", e))?;

    println!("Logged in. Token stored in {}", token_path.display());
    Ok(())
}

fn cmd_list() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let deps_dir = cwd.join("elysium_modules");

    if !deps_dir.exists() {
        println!("No packages installed (elysium_modules not found).");
        return Ok(());
    }

    let entries = std::fs::read_dir(&deps_dir)
        .map_err(|e| format!("Cannot read elysium_modules: {}", e))?;

    let mut count = 0;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Cannot read entry: {}", e))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let name = entry.file_name().to_string_lossy().to_string();
            // Try to read its elysium.json for version info
            let manifest_path = entry.path().join("elysium.json");
            let version = if manifest_path.exists() {
                match manifest::Manifest::load_from_dir(&entry.path()) {
                    Ok(m) => format!("v{}", m.version),
                    Err(_) => "?".to_string(),
                }
            } else {
                "?".to_string()
            };
            println!("  {}  {}", name, version);
            count += 1;
        }
    }

    if count == 0 {
        println!("No packages installed.");
    } else {
        println!("Total: {} package(s)", count);
    }

    Ok(())
}

// =============== Helpers ===============

fn load_token() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let token_path = PathBuf::from(&home).join(".epm").join("token");
    let token = std::fs::read_to_string(&token_path)
        .map_err(|_| "Not logged in. Run `epm login <token>` first.".to_string())?;
    Ok(token.trim().to_string())
}

fn registry_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".epm")
}

fn add_dir_to_tar(
    base: &std::path::Path,
    dir: &std::path::Path,
    archive: &mut tar::Builder<flate2::write::GzEncoder<std::fs::File>>,
    excludes: &[&str],
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap_or(&path);

        // Check if any component of the relative path matches an exclude pattern
        let components: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        let excluded = components.iter().any(|c| excludes.contains(&c.as_str()));

        if excluded {
            continue;
        }

        if path.is_dir() {
            archive.append_dir(relative, &path)?;
            add_dir_to_tar(base, &path, archive, excludes)?;
        } else {
            let mut file = std::fs::File::open(&path)?;
            archive.append_file(relative, &mut file)?;
        }
    }
    Ok(())
}
