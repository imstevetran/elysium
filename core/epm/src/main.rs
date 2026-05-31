mod acl;
mod auth;
mod cli;
mod manifest;
mod registry;
mod resolver;
mod tree_shake;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    // Load .env file before dispatching commands
    if let Err(e) = load_env_file(&cli.env_file) {
        eprintln!("Warning: failed to load env file '{}': {}", cli.env_file, e);
    }

    let result = match &cli.command {
        cli::Commands::Init {
            name,
            org,
            version,
            description,
            author,
            license,
            force,
        } => cmd_init(name, org.as_deref(), version, description, author, license, *force),
        cli::Commands::Install { package, version, save, shake, legacy } => {
            cmd_install(package.as_deref(), version.as_deref(), *save, *shake, *legacy)
        }
        cli::Commands::Lock => {
            cmd_lock()
        }
        cli::Commands::Publish { registry: _ } => cmd_publish(),
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
        cli::Commands::Why { package } => {
            cmd_why(package)
        }
        cli::Commands::Login => auth::cmd_login(),
        cli::Commands::Logout => auth::cmd_logout(),
        cli::Commands::Whoami => auth::cmd_whoami(),
        cli::Commands::Org { command } => match command {
            cli::OrgCommands::Create { name } => cmd_org_create(&name),
            cli::OrgCommands::List => cmd_org_list(),
        },
        cli::Commands::Grant { github_login } => cmd_grant(&github_login),
        cli::Commands::List => cmd_list(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

// =============== Commands ===============

fn cmd_init(
    name: &Option<String>,
    org: Option<&str>,
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

    let pkg_part = name.clone().unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("package")
            .to_string()
    });

    let package_name = match org {
        Some(org_slug) => {
            acl::validate_slug(org_slug, "org")?;
            acl::validate_slug(&pkg_part, "package")?;
            acl::format_scoped_name(org_slug, &pkg_part)
        }
        None => {
            eprintln!(
                "Tip: use --org <slug> so the package is publishable as @org/{} (run `epm org create` first).",
                pkg_part
            );
            pkg_part
        }
    };

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
    legacy: bool,
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

            println!("Resolving dependencies (legacy={}) ...", legacy);

            let resolution = if legacy {
                resolver::resolve_legacy(&manifest.name, &manifest.version, &manifest.dependencies)?
            } else {
                resolver::resolve(&manifest.name, &manifest.version, &manifest.dependencies)?
            };

            println!("Resolution:");
            for dep in &resolution.tree {
                println!("  {}@{}", dep.name, dep.version);
            }

            // Install all resolved deps
            let deps_dir = cwd.join("elysium_modules");
            std::fs::create_dir_all(&deps_dir)
                .map_err(|e| format!("Cannot create elysium_modules dir: {}", e))?;

            let total = resolution.tree.len();
            for (i, dep) in resolution.tree.iter().enumerate() {
                println!("[{}/{}] Installing {}@{} ...", i + 1, total, dep.name, dep.version);
                let target_dir = deps_dir.join(&dep.name);
                // Ensure parent exists for scoped packages (e.g. @org/)
                if let Some(parent) = target_dir.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
                }
                if target_dir.exists() {
                    // Remove existing to reinstall correct version
                    std::fs::remove_dir_all(&target_dir)
                        .map_err(|e| format!("Cannot remove old {}: {}", dep.name, e))?;
                }
                registry::install_package(&dep.name, Some(&dep.version), &target_dir)?;
                println!("  -> installed to {}", target_dir.display());
            }

            // Write lockfile
            resolver::write_lockfile(&cwd, &resolution)?;
            println!("Lockfile written to elysium.lock");

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
            // Ensure parent exists for scoped packages (e.g. @org/)
            if let Some(parent) = target_dir.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
            }
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

fn cmd_lock() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies to lock.");
        return Ok(());
    }

    let resolution = resolver::resolve(&manifest.name, &manifest.version, &manifest.dependencies)?;

    println!("Locking dependencies for {}@{}:", manifest.name, manifest.version);
    for dep in &resolution.tree {
        println!("  {}@{}", dep.name, dep.version);
    }

    resolver::write_lockfile(&cwd, &resolution)?;
    println!("Lockfile written to elysium.lock");
    Ok(())
}

fn cmd_publish() -> Result<(), String> {
    let github_login = auth::require_github_user()?;
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;

    if manifest.name.is_empty() {
        return Err("Package name is required in elysium.json".to_string());
    }
    if manifest.version.is_empty() {
        return Err("Package version is required in elysium.json".to_string());
    }
    acl::parse_scoped_package_name(&manifest.name)?;

    println!(
        "Publishing {} v{} as @{} ...",
        manifest.name, manifest.version, github_login
    );

    // Check that entry file exists
    let entry_file = manifest.entry.as_deref().unwrap_or("main.ely");
    let entry_path = cwd.join(entry_file);
    if !entry_path.exists() {
        return Err(format!("Entry file '{}' not found", entry_file));
    }

    // Create a temporary directory and build tarball
    let temp_dir = tempfile::tempdir().map_err(|e| format!("Cannot create temp dir: {}", e))?;
    // Manifest name may contain `/` (@org/pkg) — keep tarball filename a single path segment
    let tarball_filename = format!(
        "{}-{}.tar.gz",
        manifest.name.replace('@', "").replace('/', "-"),
        manifest.version
    );
    let tarball_path = temp_dir.path().join(tarball_filename);

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
        "elysium.lock",
        ".env",
    ];

    add_dir_to_tar(&cwd, &cwd, &mut archive, &excludes)
        .map_err(|e| format!("Cannot create tarball: {}", e))?;

    let encoder = archive.into_inner().map_err(|e| format!("{}", e))?;
    encoder.finish().map_err(|e| format!("{}", e))?;

    println!("Created tarball: {}", tarball_path.display());

    registry::publish_package(&github_login, &manifest, &tarball_path)?;

    println!("Published {} v{} to registry!", manifest.name, manifest.version);
    Ok(())
}

fn cmd_org_create(org_slug: &str) -> Result<(), String> {
    let login = auth::require_github_user()?;
    registry::create_org(&login, org_slug)
}

fn cmd_org_list() -> Result<(), String> {
    let login = auth::require_github_user()?;
    let orgs = registry::list_orgs(&login)?;
    if orgs.is_empty() {
        println!("You do not own any orgs yet. Create one: epm org create <name>");
        return Ok(());
    }
    println!("Orgs owned by @{}:", login);
    for (slug, entry) in orgs {
        let created = entry.created_at.as_deref().unwrap_or("-");
        println!("  @{}  (created {})", slug, created);
    }
    Ok(())
}

fn cmd_grant(collaborator: &str) -> Result<(), String> {
    let login = auth::require_github_user()?;
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;
    acl::parse_scoped_package_name(&manifest.name)?;
    registry::grant_collaborator(&login, &manifest.name, collaborator)
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

    // First try lockfile
    let lockfile = resolver::read_lockfile(&cwd)?;

    if let Some(resolution) = lockfile {
        println!("Dependency tree for {}@{} (from elysium.lock):", manifest.name, manifest.version);
        if resolution.tree.is_empty() {
            println!("(no dependencies)");
        } else {
            for dep in &resolution.tree {
                println!("  {}@{}", dep.name, dep.version);
            }
        }
        return Ok(());
    }

    // No lockfile: resolve from manifest directly (flat)
    println!("Resolving dependency tree for {}@{} ...", manifest.name, manifest.version);

    if manifest.dependencies.is_empty() {
        println!("(no dependencies)");
        return Ok(());
    }

    let resolution = resolver::resolve(&manifest.name, &manifest.version, &manifest.dependencies)?;
    for dep in &resolution.tree {
        println!("  {}@{}", dep.name, dep.version);
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

fn cmd_why(package: &str) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {}", e))?;
    let manifest = manifest::Manifest::load_from_dir(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies in this project.");
        return Ok(());
    }

    println!("Tracing why {} is required...\n", package);

    let paths = resolver::trace_dep(
        &manifest.name,
        &manifest.version,
        &manifest.dependencies,
        package,
    )?;

    if paths.is_empty() {
        println!("'{}' is not a dependency (direct or transitive) of this project.", package);
        return Ok(());
    }

    for (i, path) in paths.iter().enumerate() {
        println!("Path #{}:", i + 1);
        if path.chain.len() == 1 {
            // Only the package itself — direct dependency
            let (name, ver, constraint) = &path.chain[0];
            println!("  (direct) {}@{}  required as \"{}\" in elysium.json", name, ver, constraint);
        } else {
            for (j, (name, ver, constraint)) in path.chain.iter().enumerate() {
                let indent = "  ".repeat(j + 1);
                println!("{}{}@{}  (required as \"{}\")", indent, name, ver, constraint);
            }
        }
        println!();
    }

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
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip non-directories
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        // Handle scoped directories (e.g. @org/)
        if file_name.starts_with('@') {
            let scope_name = file_name;
            let scoped_entries = std::fs::read_dir(&path)
                .map_err(|e| format!("Cannot read scope dir {}: {}", scope_name, e))?;
            for scoped in scoped_entries {
                let scoped = scoped.map_err(|e| format!("Cannot read entry: {}", e))?;
                if scoped.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let pkg_name = format!("{}/{}", scope_name, scoped.file_name().to_string_lossy());
                    let manifest_path = scoped.path().join("elysium.json");
                    let version = if manifest_path.exists() {
                        match manifest::Manifest::load_from_dir(&scoped.path()) {
                            Ok(m) => format!("v{}", m.version),
                            Err(_) => "?".to_string(),
                        }
                    } else {
                        "?".to_string()
                    };
                    println!("  {}  {}", pkg_name, version);
                    count += 1;
                }
            }
        } else {
            let version = if path.join("elysium.json").exists() {
                match manifest::Manifest::load_from_dir(&path) {
                    Ok(m) => format!("v{}", m.version),
                    Err(_) => "?".to_string(),
                }
            } else {
                "?".to_string()
            };
            println!("  {}  {}", file_name, version);
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

/// Load environment variables from a .env file.
///
/// Defaults to `.env` in the current directory. Skips silently if the file
/// doesn't exist. Lines are parsed as `KEY=VALUE` (supports quoted values and
/// comments with `#`).
fn load_env_file(path: &str) -> Result<(), String> {
    let env_path = std::path::Path::new(path);
    if !env_path.exists() {
        // Default .env is optional — skip silently
        if path == ".env" {
            return Ok(());
        }
        return Err(format!("file not found: {}", path));
    }

    let content = std::fs::read_to_string(env_path)
        .map_err(|e| format!("cannot read {}: {}", path, e))?;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Split on the first '='
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let mut value = trimmed[eq_pos + 1..].trim().to_string();

            // Strip surrounding quotes if present
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                let len = value.len();
                value = value[1..len - 1].to_string();
            }

            if key.is_empty() {
                return Err(format!("{}:{}: empty key", path, line_num + 1));
            }

            std::env::set_var(&key, &value);
        } else {
            return Err(format!("{}:{}: malformed line (expected KEY=VALUE)", path, line_num + 1));
        }
    }

    Ok(())
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
