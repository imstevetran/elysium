use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "elysium", version = "0.1.0", about = "Elysium 2.0 compiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build an Elysium source file
    Build {
        /// Path to the .ely or .elyx source file
        file: PathBuf,
        /// Output file path (default: a.out)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Print LLVM IR instead of compiling
        #[arg(long)]
        emit_ir: bool,
        /// Include DWARF debug info for debugging with lldb/gdb
        #[arg(long)]
        debug: bool,
        /// Target environment for stub resolution (default: local)
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// Run an Elysium source file (compile and execute)
    Run {
        /// Path to the .ely or .elyx source file
        file: PathBuf,
        /// Include DWARF debug info
        #[arg(long)]
        debug: bool,
        /// Generate an LLVM IR dump
        #[arg(long)]
        emit_ir: bool,
        /// Target environment for stub resolution (default: local)
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// Type-check an Elysium source file without compiling
    Check {
        /// Path to the .ely or .elyx source file
        file: PathBuf,
        /// Target environment for stub resolution (default: local)
        #[arg(long, default_value = "local")]
        env: String,
    },
    /// Highlight an Elysium source file with syntax coloring
    Highlight {
        /// Path to the .ely or .elyx source file
        file: PathBuf,
        /// Output format (ansi, html)
        #[arg(long, default_value = "ansi")]
        format: String,
        /// Output file (default: stdout for ansi, file for html)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Lint an Elysium source file
    Lint {
        /// Path to the .ely or .elyx source file
        file: PathBuf,
        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Generate HTML CSS for syntax highlighting
    HighlightCss,
    /// Launch the interactive REPL (Read-Eval-Print Loop)
    Repl,
    /// Generate Markdown documentation from a source file
    Doc {
        /// Path to the .ely source file
        file: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate a dependency graph (DOT or JSON) from a source file
    DepGraph {
        /// Path to the .ely source file
        file: PathBuf,
        /// Output format (dot, json)
        #[arg(long, default_value = "dot")]
        format: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate test stubs from a source file
    GenTest {
        /// Path to the .ely source file
        file: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Run spec-driven tests (transform spec/feat/expect into runtime assertions)
    Test {
        /// Path to a .ely file or a directory containing .ely test files (default: tests/)
        path: Option<PathBuf>,
        /// Run tests without executing (just type-check and show spec structure)
        #[arg(long)]
        dry_run: bool,
        /// Target environment for stub resolution (default: test)
        #[arg(long, default_value = "test")]
        env: String,
    },
    /// Update package dependencies to latest compatible versions
    Update {
        /// Target package (omit to check all dependencies)
        package: Option<String>,
        /// Apply updates to elysium.json (default: dry-run / list only)
        #[arg(short, long)]
        apply: bool,
        /// Update to latest version (ignores constraint range)
        #[arg(long)]
        latest: bool,
        /// Force downgrade if latest is lower than current
        #[arg(long)]
        force: bool,
    },
    /// Migrate Elysium source files — automatically update syntax to the latest dialect
    Migrate {
        /// Path to a .ely source file or directory (default: recursive scan)
        file: Option<PathBuf>,
        /// Skip files that are already up-to-date
        #[arg(long)]
        check: bool,
        /// Only show what would change, don't write anything
        #[arg(long)]
        dry_run: bool,
        /// Force migration even for patterns marked as "requires manual review"
        #[arg(long)]
        force: bool,
    },
    /// Port a TypeScript or JavaScript file to Elysium
    Port {
        /// Path to the .ts or .js source file
        file: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Language override (typescript, javascript) — auto-detected from extension by default
        #[arg(long)]
        lang: Option<String>,
    },
}
