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
}
