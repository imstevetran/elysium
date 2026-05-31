use std::fs;
use std::path::{Path, PathBuf};

use crate::error;

// ==================== Migration registry ====================

/// A single migration rule.
struct MigrationRule {
    id: &'static str,
    #[allow(dead_code)]
    description: &'static str,
    /// Whether this migration requires manual review (user must verify the result)
    requires_manual_review: bool,
    /// The function that applies this migration to a source file
    apply: fn(source: &str) -> (String, Vec<String>),
}

/// All registered migrations, ordered by confidence (most automatic first).
fn all_migrations() -> Vec<MigrationRule> {
    vec![
        MigrationRule {
            id: "webworker-to-worker",
            description: "Rename webworker.* method calls to worker.* (webworker API was merged into worker)",
            requires_manual_review: false,
            apply: migrate_webworker_to_worker,
        },
        MigrationRule {
            id: "bm-to-bench",
            description: "Normalize `bm` keyword to `bench` for consistency",
            requires_manual_review: false,
            apply: migrate_bm_to_bench,
        },
        MigrationRule {
            id: "normalize-imports",
            description: "Add ./ prefix to relative imports without one",
            requires_manual_review: false,
            apply: migrate_normalize_imports,
        },
        MigrationRule {
            id: "describe-to-spec",
            description: "Replace `describe` with `spec` and `it` with `feat`",
            requires_manual_review: true,
            apply: migrate_describe_to_spec,
        },
    ]
}

// ==================== Migrations ====================

/// `webworker.method(...)` → `worker.method(...)`
fn migrate_webworker_to_worker(source: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut result = source.to_string();

    // Pattern: `webworker.` that is not part of a longer identifier
    let pattern = "webworker.";
    let replacement = "worker.";

    let mut new = String::new();
    let mut last_end = 0;

    for (idx, _) in source.match_indices(pattern) {
        // Check word boundary before
        if idx > 0 {
            let prev = source.as_bytes()[idx - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue; // part of a longer identifier
            }
        }
        new.push_str(&source[last_end..idx]);
        new.push_str(replacement);
        last_end = idx + pattern.len(); // skip entire "webworker." (incl dot)
    }
    if last_end < source.len() {
        new.push_str(&source[last_end..]);
    }

    if new != source {
        let count = source.matches("webworker.").count();
        changes.push(format!("Replaced {} `webworker.*` call(s) with `worker.*`", count));
        result = new;
    }

    (result, changes)
}

/// `bm { ... }` → `bench { ... }`
fn migrate_bm_to_bench(source: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut result = source.to_string();

    // Replace `bm {` with `bench {` — careful with word boundaries
    let mut new = String::new();
    let mut last_end = 0;

    for (idx, _) in source.match_indices("bm ") {
        if idx > 0 {
            let prev = source.as_bytes()[idx - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }
        // Check it's followed by `{` or space then `{`
        let after = &source[idx + 2..];
        if after.trim_start().starts_with('{') || after.trim_start().starts_with('(') {
            new.push_str(&source[last_end..idx]);
            new.push_str("bench ");
            last_end = idx + 3; // "bm " is 3 chars
        }
    }
    if last_end < source.len() {
        new.push_str(&source[last_end..]);
    }

    if new != source {
        // Count occurrences more carefully
        let count = source.lines().filter(|l| l.trim().starts_with("bm ") || l.trim().starts_with("bm{")).count();
        changes.push(format!("Replaced {} `bm` keyword(s) with `bench`", count));
        result = new;
    }

    (result, changes)
}

/// Normalize import paths: ensure relative paths start with `./`
fn migrate_normalize_imports(source: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut result = source.to_string();

    // Pattern: import "bare_name.ely" → import "./bare_name.ely"
    // We need to find import statements where the path is a relative file
    // that doesn't start with `./`, `../`, `@/`, `#/`

    let mut new = String::new();
    let mut last_end = 0;

    // Match `import "` followed by a non-./, non-@, non-#, non-/ path
    let import_pattern = "import \"";
    let mut idx = 0;
    while idx < source.len() {
        if let Some(pos) = source[idx..].find(import_pattern) {
            let abs_pos = idx + pos;
            let after_quote = abs_pos + import_pattern.len();
            // Look ahead to find the closing quote
            if let Some(end_quote) = source[after_quote..].find('"') {
                let path = &source[after_quote..after_quote + end_quote];
                let should_prefix = !path.starts_with("./")
                    && !path.starts_with("../")
                    && !path.starts_with('@')
                    && !path.starts_with('#')
                    && !path.starts_with('/')
                    && path.contains('.')
                    && !path.contains(' ');

                if should_prefix {
                    new.push_str(&source[last_end..abs_pos + import_pattern.len()]);
                    new.push_str("./");
                    last_end = after_quote;
                }
            }
            idx = abs_pos + 1;
        } else {
            break;
        }
    }
    if last_end < source.len() {
        new.push_str(&source[last_end..]);
    }

    if new != source {
        let count = source.matches("import \"").count()
            - new.matches("import \"").count();
        if count > 0 {
            changes.push(format!("Added `./` prefix to {} import path(s)", count));
        }
        result = new;
    }

    (result, changes)
}

/// `describe "..." { ... }` → `spec "..." { ... }`
/// `it "..." { ... }` → `feat "..." { ... }`
fn migrate_describe_to_spec(source: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut result = source.to_string();

    // Replace `describe ` with `spec ` (not inside strings/comments)
    let mut new = String::new();
    let _last_end = 0;
    let mut describe_count = 0;
    let mut it_count = 0;

    // Simple line-by-line approach for keywords
    for line in source.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if trimmed.starts_with("describe ") || trimmed == "describe" {
            // Check it's not inside a string or comment
            if !line.trim_start().starts_with("//") && !line.trim_start().starts_with("///") {
                let rest = &trimmed[8..]; // after "describe"
                new.push_str(&format!("{}spec{}", indent, rest));
                describe_count += 1;
                new.push('\n');
                continue;
            }
        }
        if trimmed.starts_with("it ") || trimmed == "it" {
            if !line.trim_start().starts_with("//") && !line.trim_start().starts_with("///") {
                let rest = &trimmed[2..]; // after "it"
                // Only replace `it` if it's followed by a string literal (test context)
                if rest.trim_start().starts_with('"') {
                    new.push_str(&format!("{}feat{}", indent, rest));
                    it_count += 1;
                    new.push('\n');
                    continue;
                }
            }
        }

        new.push_str(line);
        new.push('\n');
    }

    if new != source {
        if describe_count > 0 {
            changes.push(format!("Replaced {} `describe` with `spec`", describe_count));
        }
        if it_count > 0 {
            changes.push(format!("Replaced {} `it(...)` with `feat(...)`", it_count));
        }
        result = new;
    }

    (result, changes)
}

// ==================== Main migration command ====================

/// Run the `elysium migrate` command.
pub fn cmd_migrate(
    path_opt: Option<&PathBuf>,
    check_only: bool,
    dry_run: bool,
    force: bool,
) -> error::Result<()> {
    let cwd = std::env::current_dir()
        .map_err(|e| error::CompileError::new(format!("Cannot get cwd: {}", e)))?;

    let target = path_opt.map(|p| {
        if p.is_absolute() {
            p.clone()
        } else {
            cwd.join(p)
        }
    }).unwrap_or_else(|| cwd.clone());

    // Collect .ely files
    let mut files: Vec<PathBuf> = Vec::new();
    collect_ely_files(&target, &mut files);

    if files.is_empty() {
        println!("No .ely files found.");
        return Ok(());
    }

    let migrations = all_migrations();
    let active_migrations: Vec<&MigrationRule> = if force {
        migrations.iter().collect()
    } else {
        migrations.iter().filter(|m| !m.requires_manual_review).collect()
    };

    println!(
        "Migrating {} file(s) with {} migration(s)...{}",
        files.len(),
        active_migrations.len(),
        if dry_run { " (dry-run)" } else { "" }
    );
    println!();

    let mut any_changes = false;
    let mut total_changes = 0;

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  ✗ {}: cannot read — {}", file.display(), e);
                continue;
            }
        };

        let mut current = source.clone();
        let mut file_changes: Vec<(String, Vec<String>)> = Vec::new();

        for migration in &active_migrations {
            let (new_source, changes) = (migration.apply)(&current);
            if !changes.is_empty() {
                file_changes.push((migration.id.to_string(), changes));
                current = new_source;
            }
        }

        if file_changes.is_empty() {
            println!("  ✓ {} — up to date", file.display());
            continue;
        }

        any_changes = true;
        let change_count: usize = file_changes.iter().map(|(_, cs)| cs.len()).sum();
        total_changes += change_count;

        println!("  ~ {} ({} change(s))", file.display(), change_count);
        for (id, changes) in &file_changes {
            for change in changes {
                let rule = migrations.iter().find(|m| m.id == *id);
                let manual_flag = if rule.map(|r| r.requires_manual_review).unwrap_or(false) {
                    " [requires review]"
                } else {
                    ""
                };
                println!("    - {}{}", change, manual_flag);
            }
        }

        if !dry_run && !check_only {
            if let Err(e) = fs::write(file, &current) {
                eprintln!("    ✗ Failed to write: {}", e);
            }
        }
    }

    println!();
    if !any_changes {
        println!("All files are already up to date. ✓");
        return Ok(());
    }

    if check_only {
        println!(
            "Found {} file(s) needing migration ({} change(s)). {}",
            files.iter().filter(|f| {
                // Re-check which files had changes
                let source = fs::read_to_string(f).unwrap_or_default();
                active_migrations.iter().any(|m| {
                    let (_new_s, ch) = (m.apply)(&source);
                    !ch.is_empty()
                })
            }).count(),
            total_changes,
            "Run `elysium migrate` without --check to apply."
        );
        return Err(error::CompileError::new("Some files need migration"));
    }

    if dry_run {
        println!("Dry-run complete. Run without --dry-run to apply changes.");
    } else {
        println!("Migration complete. {} change(s) applied across {} file(s).", total_changes, files.len());
    }

    Ok(())
}

/// Recursively collect .ely files.
fn collect_ely_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if dir.is_file() {
        if dir.extension().map(|e| e == "ely" || e == "elyx").unwrap_or(false) {
            files.push(dir.to_path_buf());
        }
        return;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        let mut sub_files: Vec<PathBuf> = Vec::new();
        let mut sub_dirs: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip common non-source directories
                let name = path.file_name().unwrap_or_default().to_str().unwrap_or("");
                if name != "target" && name != ".git" && name != "node_modules"
                    && name != "elysium_modules" && name != ".epm"
                {
                    sub_dirs.push(path);
                }
            } else if path.extension().map(|e| e == "ely" || e == "elyx").unwrap_or(false) {
                sub_files.push(path);
            }
        }
        sub_files.sort();
        for f in sub_files {
            files.push(f);
        }
        for d in sub_dirs {
            collect_ely_files(&d, files);
        }
    }
}
