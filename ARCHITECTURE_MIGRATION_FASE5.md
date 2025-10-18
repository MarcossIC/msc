
### FASE 5: Extraer Core Business Logic (MEDIO-ALTO RIESGO)
**Duración estimada**: 3-4 horas
**Riesgo**: Medio-Alto

#### Paso 5.1: Mover config.rs a core/
```bash
mv src/config.rs src/core/config.rs
```

**Actualizar src/core/mod.rs**:
```rust
// src/core/mod.rs
pub mod config;
pub mod cleaner;
pub mod workspace;
pub mod file_scanner;  // Para comando list

// Re-exports
pub use config::Config;
pub use cleaner::{TempCleaner, CleanupStats};
pub use workspace::WorkspaceManager;
pub use file_scanner::FileScanner;
```

#### Paso 5.2: Crear core/cleaner.rs
```rust
// src/core/cleaner.rs
use anyhow::Result;
use std::path::Path;
use std::fs;
use crate::system::temp_dirs::get_temp_directories;
use crate::ui::formatters::format_size;

pub struct TempCleaner {
    pub directories: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CleanupStats {
    pub total_files: usize,
    pub total_size: u64,
    pub deleted_files: usize,
    pub deleted_size: u64,
    pub failed_files: usize,
}

impl TempCleaner {
    pub fn new() -> Result<Self> {
        let directories = get_temp_directories();
        Ok(Self { directories })
    }

    pub fn scan(&self) -> CleanupStats {
        let mut stats = CleanupStats::default();

        for temp_dir in &self.directories {
            count_files_recursive(Path::new(temp_dir), &mut stats.total_files, &mut stats.total_size);
        }

        stats
    }

    pub fn clean(&self, dry_run: bool, on_progress: impl Fn(usize, usize)) -> Result<CleanupStats> {
        let mut stats = self.scan();
        let mut processed = 0;

        for temp_dir in &self.directories {
            delete_files_recursive(
                Path::new(temp_dir),
                &mut processed,
                stats.total_files,
                &mut stats.deleted_files,
                &mut stats.deleted_size,
                &mut stats.failed_files,
                dry_run,
                &on_progress,
            );
        }

        Ok(stats)
    }
}

fn count_files_recursive(dir: &Path, total_files: &mut usize, total_size: &mut u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    *total_files += 1;
                    *total_size += metadata.len();
                } else if metadata.is_dir() {
                    count_files_recursive(&entry.path(), total_files, total_size);
                }
            }
        }
    }
}

fn delete_files_recursive(
    dir: &Path,
    processed: &mut usize,
    total_files: usize,
    deleted_files: &mut usize,
    deleted_size: &mut u64,
    failed_files: &mut usize,
    dry_run: bool,
    on_progress: &impl Fn(usize, usize),
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    *processed += 1;
                    let file_path = entry.path();
                    let file_size = metadata.len();

                    on_progress(*processed, total_files);

                    if dry_run {
                        *deleted_files += 1;
                        *deleted_size += file_size;
                    } else {
                        match fs::remove_file(&file_path) {
                            Ok(_) => {
                                *deleted_files += 1;
                                *deleted_size += file_size;
                            }
                            Err(_) => {
                                *failed_files += 1;
                            }
                        }
                    }
                } else if metadata.is_dir() {
                    delete_files_recursive(
                        &entry.path(),
                        processed,
                        total_files,
                        deleted_files,
                        deleted_size,
                        failed_files,
                        dry_run,
                        on_progress,
                    );
                }
            }
        }
    }
}
```

#### Paso 5.3: Crear core/workspace.rs
```rust
// src/core/workspace.rs
use anyhow::Result;
use std::path::Path;
use std::fs;
use crate::core::config::Config;

pub struct WorkspaceManager {
    config: Config,
}

impl WorkspaceManager {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        Ok(Self { config })
    }
    
    pub fn with_config(config: Config) -> Self {
        Self { config }
    }

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

    pub fn list_workspaces(&self) -> Vec<(String, String)> {
        let mut workspaces: Vec<_> = self.config.get_workspaces()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        workspaces.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        workspaces
    }
}
```

#### Paso 5.4: Crear core/file_scanner.rs
```rust
// src/core/file_scanner.rs
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::git::{GitStatus, load_git_status, load_gitignore};
use ignore::gitignore::Gitignore;

/// Scanner para listar archivos con información detallada
pub struct FileScanner {
    path: PathBuf,
    git_status: Option<HashMap<String, GitStatus>>,
    gitignore: Option<Gitignore>,
}

impl FileScanner {
    pub fn new(path: &Path) -> Result<Self> {
        log::debug!("Creating FileScanner for path: {:?}", path);
        
        let git_status = load_git_status(path);
        let gitignore = load_gitignore(path);
        
        Ok(Self {
            path: path.to_path_buf(),
            git_status,
            gitignore,
        })
    }
    
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    pub fn git_status(&self) -> &Option<HashMap<String, GitStatus>> {
        &self.git_status
    }
    
    pub fn gitignore(&self) -> &Option<Gitignore> {
        &self.gitignore
    }
    
    /// Lista archivos y directorios en el path
    pub fn scan(&self, show_hidden: bool) -> Result<Vec<ScanEntry>> {
        use std::fs;
        use crate::platform;
        
        let mut entries = Vec::new();
        let dir_entries = fs::read_dir(&self.path)?;
        
        for entry in dir_entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            // Skip hidden files if not requested
            if !show_hidden {
                if file_name.starts_with('.') || platform::is_hidden(&entry) {
                    continue;
                }
            }
            
            if let Ok(metadata) = entry.metadata() {
                let is_dir = metadata.is_dir();
                let size = if is_dir { 0 } else { metadata.len() };
                
                entries.push(ScanEntry {
                    name: file_name,
                    path: entry.path(),
                    is_dir,
                    size,
                    metadata,
                });
            }
        }
        
        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        
        Ok(entries)
    }
}

#[derive(Debug)]
pub struct ScanEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub metadata: std::fs::Metadata,
}
```

#### Paso 5.5: Actualizar imports en main.rs
```rust
// Reemplazar con:
use msc::core::{Config, TempCleaner, WorkspaceManager, FileScanner};
```

#### Paso 5.6: Compilar, validar y commit
```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

**Validación**: 
- ✅ Debe compilar sin errores
- ✅ Lógica de negocio aislada en `core/`
- ✅ Sin dependencias de UI en core

```bash
git add .
git commit -m "feat: phase 5 - extract core business logic"
```

---
