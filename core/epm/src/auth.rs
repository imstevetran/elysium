//! GitHub authentication via the GitHub CLI (`gh`).
//! EPM never writes tokens or PATs to disk — credentials stay in `gh`'s keychain.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

/// Ensure the user is signed in to GitHub through `gh` and return their login.
pub fn require_github_user() -> Result<String, String> {
    ensure_gh_installed()?;
    if !gh_is_authenticated()? {
        println!("Sign in with your GitHub account to use EPM.");
        run_gh_auth_login()?;
    }
    gh_current_login()
}

/// Print the current GitHub user if authenticated.
pub fn current_github_user() -> Result<Option<String>, String> {
    ensure_gh_installed()?;
    if !gh_is_authenticated()? {
        return Ok(None);
    }
    gh_current_login().map(Some)
}

pub fn ensure_gh_installed() -> Result<(), String> {
    let ok = Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(
            "GitHub CLI (gh) is required. Install: https://cli.github.com/ \
             Then run: epm login"
                .to_string(),
        )
    }
}

fn gh_is_authenticated() -> Result<bool, String> {
    let status = Command::new("gh")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run `gh auth status`: {}", e))?;
    Ok(status.success())
}

fn gh_current_login() -> Result<String, String> {
    let output = Command::new("gh")
        .args(["api", "user", "-q", ".login"])
        .output()
        .map_err(|e| format!("Failed to run `gh api user`: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Not authenticated with GitHub: {}", err.trim()));
    }
    let login = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if login.is_empty() {
        return Err("Could not read GitHub username from `gh`.".to_string());
    }
    Ok(login)
}

/// Interactive GitHub login (browser/device flow handled by `gh`).
pub fn run_gh_auth_login() -> Result<(), String> {
    let status = Command::new("gh")
        .args(["auth", "login", "-h", "github.com", "-p", "https", "-s", "repo"])
        .status()
        .map_err(|e| format!("Failed to run `gh auth login`: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err("`gh auth login` did not complete successfully.".to_string())
    }
}

/// Run `f` with GIT_ASKPASS set to a script that calls `gh auth token` (never stored by EPM).
pub fn with_git_credentials<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let cache_dir = epm_cache_dir();
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Cannot create EPM cache dir: {}", e))?;

    let askpass_path = cache_dir.join("gh-askpass.sh");
    std::fs::write(
        &askpass_path,
        "#!/bin/sh\nexec gh auth token\n",
    )
    .map_err(|e| format!("Cannot write git askpass helper: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&askpass_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Cannot set askpass permissions: {}", e))?;
    }

    let prev_askpass = std::env::var("GIT_ASKPASS").ok();
    let prev_terminal = std::env::var("GIT_TERMINAL_PROMPT").ok();

    std::env::set_var("GIT_ASKPASS", askpass_path.to_str().unwrap());
    std::env::set_var("GIT_TERMINAL_PROMPT", "0");

    let result = f();

    match prev_askpass {
        Some(v) => std::env::set_var("GIT_ASKPASS", v),
        None => std::env::remove_var("GIT_ASKPASS"),
    }
    match prev_terminal {
        Some(v) => std::env::set_var("GIT_TERMINAL_PROMPT", v),
        None => std::env::remove_var("GIT_TERMINAL_PROMPT"),
    }

    result
}

pub fn epm_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".epm")
}

pub fn cmd_login() -> Result<(), String> {
    ensure_gh_installed()?;
    if gh_is_authenticated()? {
        let login = gh_current_login()?;
        println!("Already signed in to GitHub as @{}.", login);
        println!("EPM does not store credentials — your session is managed by the GitHub CLI.");
        return Ok(());
    }
    println!("Opening GitHub sign-in (browser or device code)…");
    run_gh_auth_login()?;
    let login = gh_current_login()?;
    println!("Signed in as @{}.", login);
    println!("EPM does not store secrets. Publish with: epm publish");
    Ok(())
}

pub fn cmd_logout() -> Result<(), String> {
    ensure_gh_installed()?;
    if !gh_is_authenticated()? {
        println!("Not signed in to GitHub.");
        return Ok(());
    }
    print!("Sign out of GitHub on this machine? [y/N] ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    if line.trim().eq_ignore_ascii_case("y") {
        let status = Command::new("gh")
            .args(["auth", "logout", "-h", "github.com"])
            .status()
            .map_err(|e| format!("Failed to run `gh auth logout`: {}", e))?;
        if status.success() {
            println!("Signed out of GitHub.");
        } else {
            return Err("`gh auth logout` failed.".to_string());
        }
    } else {
        println!("Cancelled.");
    }
    Ok(())
}

pub fn cmd_whoami() -> Result<(), String> {
    ensure_gh_installed()?;
    match current_github_user()? {
        Some(login) => {
            println!("@{}", login);
            println!("Authenticated via GitHub CLI (no token stored by EPM).");
        }
        None => {
            println!("Not signed in. Run: epm login");
        }
    }
    Ok(())
}
