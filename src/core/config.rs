use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub work_path: Option<String>,
    pub workspaces: HashMap<String, String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let data = fs::read(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        // If the file is empty or corrupted, return default config
        if data.is_empty() {
            return Ok(Config::default());
        }

        let config = bincode::deserialize(&data)
            .unwrap_or_else(|_| {
                // If deserialization fails, return default config
                // (this can happen when the config format changes)
                Config::default()
            });

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let data = bincode::serialize(self)
            .with_context(|| "Failed to serialize config")?;
        
        fs::write(&config_path, data)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
        
        Ok(())
    }

    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .with_context(|| "Could not determine config directory")?;
        
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
}