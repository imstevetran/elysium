use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error;

/// The structure of an elysium.json project manifest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub environments: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// UI build configuration (browser target: js|wasm, android, ios, etc.)
    #[serde(default)]
    pub ui: UiConfig,
    /// SSR (Server-Side Rendering) configuration — when the code targets a server runtime
    #[serde(default)]
    pub ssr: SsrConfig,
}

/// UI build targets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiConfig {
    /// Browser target configuration.
    #[serde(default)]
    pub browser: BrowserTarget,
    // Future: pub android: AndroidTarget,
    // Future: pub ios: IosTarget,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            browser: BrowserTarget::default(),
        }
    }
}

/// Browser compilation target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowserTarget {
    /// Compilation target: "js" (default) or "wasm"
    #[serde(default = "default_browser_target")]
    pub target: String,
}

impl Default for BrowserTarget {
    fn default() -> Self {
        Self {
            target: default_browser_target(),
        }
    }
}

fn default_browser_target() -> String {
    "js".to_string()
}

/// SSR (Server-Side Rendering) configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SsrConfig {
    /// Whether SSR is enabled for this project.
    #[serde(default)]
    pub enabled: bool,
    /// The server runtime target (e.g. "node", "deno", "bun", "elysium-server").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
}

impl Default for SsrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            runtime: None,
        }
    }
}

/// Walk up from `dir` to find the project root (directory containing elysium.json).
pub fn find_project_root(dir: &Path) -> Option<PathBuf> {
    let mut current = Some(dir.to_path_buf());
    while let Some(d) = current {
        if d.join("elysium.json").exists() {
            return Some(d);
        }
        current = d.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Load the project manifest from a path relative to the given file.
/// Walk up from the file's directory to find elysium.json.
pub fn load_manifest(from_dir: &Path) -> error::Result<Manifest> {
    let root = find_project_root(from_dir)
        .ok_or_else(|| error::CompileError::new(
            format!("Cannot find project root (elysium.json) from `{}`", from_dir.display())
        ))?;
    let manifest_path = root.join("elysium.json");
    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| error::CompileError::new(
            format!("Cannot read {}: {}", manifest_path.display(), e)
        ))?;
    let manifest: Manifest = serde_json::from_str(&content)
        .map_err(|e| error::CompileError::new(
            format!("Cannot parse {}: {}", manifest_path.display(), e)
        ))?;
    Ok(manifest)
}

/// Resolve environment aliases from the project's elysium.json manifest.
/// Supports built-in envs (local, dev, test, prod) and custom aliases
/// defined in the `environments` field of the manifest.
pub fn resolve_env_alias(from_dir: &Path, env: &str) -> String {
    let manifest_path = from_dir.join("elysium.json");
    if let Ok(content) = fs::read_to_string(manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
            if let Some(envs) = manifest.get("environments") {
                if let Some(alias_map) = envs.as_object() {
                    if let Some(resolved) = alias_map.get(env) {
                        if let Some(val) = resolved.as_str() {
                            return val.to_string();
                        }
                    }
                }
            }
        }
    }
    env.to_string()
}
