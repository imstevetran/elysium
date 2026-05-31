use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "epm", version = "0.1.0", about = "Elysium Package Manager")]
pub struct Cli {
    /// Path to .env file (defaults to .env in current directory)
    #[arg(short = 'e', long = "env-file", default_value = ".env", global = true)]
    pub env_file: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Elysium package in the current directory
    Init {
        /// Package name within the org (defaults to directory name)
        name: Option<String>,
        /// Org scope (required for publishing); creates name `@org/name`
        #[arg(long)]
        org: Option<String>,
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
        /// Allow multiple versions of the same dependency (legacy mode)
        #[arg(long)]
        legacy: bool,
    },

    /// Generate a lockfile (elysium.lock) from current resolution
    Lock,

    /// Publish the current package to the registry (requires GitHub sign-in)
    Publish {
        /// Registry URL (reserved; default registry is configured in EPM)
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

    /// Trace why a dependency is included (show dependency paths)
    Why {
        /// Package name to trace
        package: String,
    },

    /// Sign in with your GitHub account (via GitHub CLI; no token stored by EPM)
    Login,

    /// Sign out of GitHub on this machine (delegates to GitHub CLI)
    Logout,

    /// Show the GitHub account you are signed in as
    Whoami,

    /// Manage registry orgs (scoped as `@org/package`)
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },

    /// Grant another GitHub user permission to publish this package (owner only)
    Grant {
        /// GitHub username to grant publish access
        github_login: String,
    },

    /// List installed packages
    List,
}

#[derive(Subcommand)]
pub enum OrgCommands {
    /// Create an org you own (`@org` scope for packages)
    Create {
        /// Org slug (letters, numbers, hyphens; used as `@org`)
        name: String,
    },
    /// List orgs you own
    List,
}
