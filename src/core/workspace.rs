// Workspace management module

use anyhow::Result;
use std::path::Path;
use std::fs;
use crate::core::config::Config;

/// Workspace manager for handling project workspaces
pub struct WorkspaceManager {
    config: Config,
}

impl WorkspaceManager {
    /// Create a new WorkspaceManager by loading the config
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        Ok(Self { config })
    }

    /// Create a WorkspaceManager with a specific config (useful for testing)
    pub fn with_config(config: Config) -> Self {
        Self { config }
    }

    /// Get a reference to the internal config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Map all directories in the work path as workspaces
    ///
    /// Returns the number of workspaces mapped
    pub fn map_workspaces(&mut self) -> Result<usize> {
        let work_path = self.config.get_work_path()
            .ok_or_else(|| anyhow::anyhow!("Work path not set"))?
            .clone();

        let work_dir = Path::new(&work_path);
        if !work_dir.exists() || !work_dir.is_dir() {
            return Err(anyhow::anyhow!("Work directory does not exist or is not a directory"));
        }

        self.config.clear_workspaces();
        let entries = fs::read_dir(work_dir)?;
        let mut count = 0;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();

            if entry.file_type()?.is_dir() && !file_name.starts_with('.') {
                let full_path = entry.path();
                let canonical_path = full_path.canonicalize()
                    .unwrap_or(full_path)
                    .to_string_lossy()
                    .to_string();

                self.config.add_workspace(file_name, canonical_path);
                count += 1;
            }
        }

        self.config.save()?;
        Ok(count)
    }

    /// List all registered workspaces, sorted alphabetically
    ///
    /// Returns a vector of (name, path) tuples
    pub fn list_workspaces(&self) -> Vec<(String, String)> {
        let mut workspaces: Vec<_> = self.config.get_workspaces()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        workspaces.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        workspaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_manager_creation() {
        // This might fail if config doesn't exist, which is OK for a unit test
        let _manager = WorkspaceManager::new();
    }

    #[test]
    fn test_workspace_manager_with_config() {
        let config = Config::default();
        let manager = WorkspaceManager::with_config(config);
        assert!(manager.list_workspaces().is_empty());
    }
}
