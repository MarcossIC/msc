use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::alias_validator::validate_alias_command;

/// Represents a single alias
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
}

impl Alias {
    /// Create a new alias
    ///
    /// # Security
    /// This function validates the command for shell injection vulnerabilities.
    /// It will reject commands containing dangerous shell metacharacters like:
    /// `;`, `|`, `&`, `$`, `` ` ``, `(`, `)`, `<`, `>`, etc.
    ///
    /// # Errors
    /// Returns an error if the command contains dangerous patterns
    pub fn new(name: String, command: String) -> Result<Self> {
        // Validate command for security vulnerabilities
        validate_alias_command(&command)?;

        Ok(Self {
            name,
            command,
            description: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Create a new alias with description
    ///
    /// # Security
    /// This function validates the command for shell injection vulnerabilities.
    /// It will reject commands containing dangerous shell metacharacters.
    ///
    /// # Errors
    /// Returns an error if the command contains dangerous patterns
    pub fn with_description(name: String, command: String, description: String) -> Result<Self> {
        // Validate command for security vulnerabilities
        validate_alias_command(&command)?;

        Ok(Self {
            name,
            command,
            description: Some(description),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}

/// Configuration for all aliases
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AliasConfig {
    pub aliases: HashMap<String, Alias>,
}

impl AliasConfig {
    /// Load configuration from disk
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let config: AliasConfig = serde_json::from_str(&content)
            .with_context(|| "Failed to parse alias configuration")?;

        Ok(config)
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;

        // Create parent directories
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        // Serialize to JSON with pretty printing
        let content = serde_json::to_string_pretty(self)
            .with_context(|| "Failed to serialize alias configuration")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config to {:?}", config_path))?;

        Ok(())
    }

    /// Add or update an alias
    pub fn add_alias(&mut self, alias: Alias) -> bool {
        let name = alias.name.clone();
        let is_new = !self.aliases.contains_key(&name);
        self.aliases.insert(name, alias);
        is_new
    }

    /// Remove an alias
    pub fn remove_alias(&mut self, name: &str) -> bool {
        self.aliases.remove(name).is_some()
    }

    /// Get an alias by name
    pub fn get_alias(&self, name: &str) -> Option<&Alias> {
        self.aliases.get(name)
    }

    /// Get all aliases sorted by name
    pub fn list_aliases(&self) -> Vec<&Alias> {
        let mut aliases: Vec<&Alias> = self.aliases.values().collect();
        aliases.sort_by(|a, b| a.name.cmp(&b.name));
        aliases
    }

    /// Check if an alias exists
    pub fn exists(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Get the path to the configuration file
    fn get_config_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().with_context(|| "Could not determine config directory")?;

        Ok(config_dir.join("msc").join("aliases").join("aliases.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_creation() {
        let alias = Alias::new("pyh".to_string(), "python3 -m http.server 5000".to_string())
            .expect("Should create alias with safe command");

        assert_eq!(alias.name, "pyh");
        assert_eq!(alias.command, "python3 -m http.server 5000");
        assert!(alias.description.is_none());
        assert!(!alias.created_at.is_empty());
    }

    #[test]
    fn test_alias_with_description() {
        let alias = Alias::with_description(
            "gp".to_string(),
            "git push".to_string(),
            "Quick git push".to_string(),
        )
        .expect("Should create alias with safe command");

        assert_eq!(alias.description, Some("Quick git push".to_string()));
    }

    #[test]
    fn test_config_add_remove() {
        let mut config = AliasConfig::default();

        let alias = Alias::new("test".to_string(), "echo test".to_string())
            .expect("Should create alias with safe command");
        assert!(config.add_alias(alias));

        assert!(config.exists("test"));
        assert!(config.remove_alias("test"));
        assert!(!config.exists("test"));
    }

    #[test]
    fn test_config_serialization() {
        let mut config = AliasConfig::default();
        config.add_alias(
            Alias::new("pyh".to_string(), "python3 -m http.server".to_string())
                .expect("Should create alias with safe command"),
        );

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AliasConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.aliases.len(), deserialized.aliases.len());
        assert!(deserialized.exists("pyh"));
    }

    #[test]
    fn test_list_aliases_sorted() {
        let mut config = AliasConfig::default();
        config.add_alias(Alias::new("zebra".to_string(), "cmd1".to_string()).unwrap());
        config.add_alias(Alias::new("alpha".to_string(), "cmd2".to_string()).unwrap());
        config.add_alias(Alias::new("beta".to_string(), "cmd3".to_string()).unwrap());

        let list = config.list_aliases();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
        assert_eq!(list[2].name, "zebra");
    }
}
