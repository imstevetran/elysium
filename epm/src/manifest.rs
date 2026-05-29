use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// The elysium.json manifest for an Elysium project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub entry: Option<String>,
    pub license: Option<String>,
    pub author: Option<String>,
    pub repository: Option<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    /// Custom environment aliases (e.g. "staging" -> "dev", "production" -> "prod")
    #[serde(default)]
    pub environments: HashMap<String, String>,
}

impl Manifest {
    /// Create a new manifest from scratch with required fields.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            entry: None,
            license: None,
            author: None,
            repository: None,
            dependencies: HashMap::new(),
            environments: HashMap::new(),
        }
    }

    /// Parse a manifest from JSON string.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse elysium.json: {}", e))
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize manifest: {}", e))
    }

    /// Read manifest from the given directory (looks for elysium.json).
    pub fn load_from_dir(dir: &std::path::Path) -> Result<Self, String> {
        let path = dir.join("elysium.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        Self::from_json(&content)
    }

    /// Save manifest to the given directory as elysium.json.
    pub fn save_to_dir(&self, dir: &std::path::Path) -> Result<(), String> {
        let path = dir.join("elysium.json");
        let json = self.to_json()?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
        Ok(())
    }
}

/// Registry index entry for a published package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub versions: Vec<String>,
}

/// Top-level registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub packages: HashMap<String, RegistryEntry>,
}

impl RegistryIndex {
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse registry index: {}", e))
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize registry: {}", e))
    }
}
