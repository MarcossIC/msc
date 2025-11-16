use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub work_path: Option<String>,
    #[serde(default)]
    pub workspaces: HashMap<String, String>,
    /// System default temp paths (synchronized dynamically on each load)
    #[serde(skip)]
    #[serde(default)]
    pub default_paths: Vec<String>,
    /// Custom paths added by user
    #[serde(default)]
    pub custom_paths: Vec<String>,
    /// Default paths excluded by user (persistent)
    #[serde(default)]
    pub excluded_default_paths: Vec<String>,
    /// Work directory folders that should be ignored during work cache cleanup
    #[serde(default)]
    pub ignored_work_folders: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        let mut config = if !config_path.exists() {
            Config::default()
        } else {
            let data = fs::read(&config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

            // If the file is empty or corrupted, return default config
            if data.is_empty() {
                Config::default()
            } else {
                bincode::deserialize(&data).unwrap_or_else(|_| {
                    // If deserialization fails, return default config
                    // (this can happen when the config format changes)
                    Config::default()
                })
            }
        };

        // Always sync default paths on load (dynamic update)
        config.sync_default_paths();

        Ok(config)
    }

    /// Synchronize default paths with current system configuration
    /// This is called automatically on load to ensure paths are always up-to-date
    pub fn sync_default_paths(&mut self) {
        use crate::platform::get_default_temp_directories;
        self.default_paths = get_default_temp_directories();
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let data = bincode::serialize(self).with_context(|| "Failed to serialize config")?;

        fs::write(&config_path, data)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(())
    }

    fn get_config_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().with_context(|| "Could not determine config directory")?;

        Ok(config_dir.join("msc").join("config.bin"))
    }

    pub fn set_work_path(&mut self, path: String) {
        self.work_path = Some(path);
    }

    pub fn get_work_path(&self) -> Option<&String> {
        self.work_path.as_ref()
    }

    pub fn add_workspace(&mut self, name: String, path: String) {
        self.workspaces.insert(name, path);
    }

    pub fn get_workspaces(&self) -> &HashMap<String, String> {
        &self.workspaces
    }

    pub fn clear_workspaces(&mut self) {
        self.workspaces.clear();
    }

    // Clean paths management

    /// Get all active clean paths (default + custom, excluding removed defaults)
    pub fn get_clean_paths(&self) -> Vec<String> {
        let mut all_paths = Vec::new();

        // Add default paths that are NOT in the excluded list
        for default_path in &self.default_paths {
            if !self.excluded_default_paths.contains(default_path) {
                all_paths.push(default_path.clone());
            }
        }

        // Add custom paths that don't already exist
        for custom_path in &self.custom_paths {
            if !all_paths.contains(custom_path) {
                all_paths.push(custom_path.clone());
            }
        }

        all_paths
    }

    /// Get only custom paths
    pub fn get_custom_paths(&self) -> &Vec<String> {
        &self.custom_paths
    }

    /// Get only default paths
    pub fn get_default_paths(&self) -> &Vec<String> {
        &self.default_paths
    }

    /// Add a custom clean path
    pub fn add_clean_path(&mut self, path: String) -> bool {
        // Check if it's already in defaults
        if self.default_paths.contains(&path) {
            return false; // Already exists as default
        }

        // Check if it's already in customs
        if self.custom_paths.contains(&path) {
            return false; // Already exists as custom
        }

        self.custom_paths.push(path);
        true
    }

    /// Remove a clean path (from defaults or customs)
    /// If it's a default path, add it to excluded list (persistent)
    /// If it's a custom path, remove it from custom_paths permanently
    pub fn remove_clean_path(&mut self, path: &str) -> bool {
        // Check if it's a default path
        if self.default_paths.contains(&path.to_string()) {
            // Add to exclusion list if not already there
            if !self.excluded_default_paths.contains(&path.to_string()) {
                self.excluded_default_paths.push(path.to_string());
                return true;
            }
            return false; // Already excluded
        }

        // Try to remove from custom paths
        if let Some(pos) = self.custom_paths.iter().position(|p| p == path) {
            self.custom_paths.remove(pos);
            return true;
        }

        false
    }

    /// Reset to only system defaults (clears all custom paths and exclusions)
    pub fn reset_to_defaults(&mut self) {
        self.custom_paths.clear();
        self.excluded_default_paths.clear();
        self.sync_default_paths();
    }

    // Work cache ignore management

    /// Get all ignored work folders (always includes "msc" for safety)
    pub fn get_ignored_work_folders(&self) -> Vec<String> {
        let mut ignored = self.ignored_work_folders.clone();

        // Always include "msc" to avoid cleaning the tool itself
        if !ignored.contains(&"msc".to_string()) {
            ignored.push("msc".to_string());
        }

        ignored
    }

    /// Add a folder to the ignore list for work cache cleanup
    pub fn add_ignored_work_folder(&mut self, folder_name: String) -> bool {
        // Don't add "msc" as it's always included
        if folder_name == "msc" {
            return false;
        }

        if self.ignored_work_folders.contains(&folder_name) {
            return false;
        }

        self.ignored_work_folders.push(folder_name);
        true
    }

    /// Remove a folder from the ignore list
    pub fn remove_ignored_work_folder(&mut self, folder_name: &str) -> bool {
        if let Some(pos) = self
            .ignored_work_folders
            .iter()
            .position(|f| f == folder_name)
        {
            self.ignored_work_folders.remove(pos);
            return true;
        }
        false
    }

    /// Get the list of user-configured ignored work folders (without "msc")
    pub fn get_user_ignored_work_folders(&self) -> &Vec<String> {
        &self.ignored_work_folders
    }
}
