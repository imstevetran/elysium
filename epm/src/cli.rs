use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "epm", version = "0.1.0", about = "Elysium Package Manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Elysium package in the current directory
    Init {
        /// Package name (defaults to directory name)
        name: Option<String>,
        /// Version (default: 0.1.0)
        #[arg(short, long, default_value = "0.1.0")]
        version: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Author
        #[arg(short, long)]
        author: Option<String>,
        /// License
        #[arg(short, long)]
        license: Option<String>,
        /// Force overwrite existing elysium.json
        #[arg(short, long)]
        force: bool,
    },

    /// Install all dependencies from elysium.json
    Install {
        /// Package name to install (if omitted, installs all dependencies)
        package: Option<String>,
        /// Version constraint
        #[arg(short, long)]
        version: Option<String>,
        /// Save as a dependency in elysium.json
        #[arg(long)]
        save: bool,
        /// Tree-shake installed packages after installation
        #[arg(long)]
        shake: bool,
    },

    /// Publish the current package to the registry
    Publish {
        /// Registry URL (default: https://github.com/imstevetran/epm-registry.git)
        #[arg(short, long)]
        registry: Option<String>,
    },

    /// Search for packages in the registry
    Search {
        query: String,
    },

    /// Show info about a package
    Info {
        package: String,
    },

    /// Show the dependency tree of installed packages
    Tree {
        /// Show dev dependencies too (not just regular deps)
        #[arg(long)]
        all: bool,
    },

    /// Tree-shake installed packages: remove unused .ely files
    Shake {
        /// Don't actually delete, only list what would be removed
        #[arg(long)]
        dry_run: bool,
    },

    /// Log in to the registry (stores GitHub token)
    Login {
        /// GitHub personal access token
        token: String,
    },

    /// List installed packages
    List,
}
