
### FASE 3: Extraer Git (BAJO RIESGO)
**Duración estimada**: 1-2 horas
**Riesgo**: Bajo
**Objetivo**: Aislar toda la integración con Git

#### Paso 3.1: Extraer git/status.rs
**Origen**: `src/main.rs` funciones relacionadas a Git status
**Destino**: `src/git/status.rs`

```rust
// src/git/status.rs
use std::path::Path;
use std::collections::HashMap;
use git2::{Repository, Status};

#[derive(Debug, Clone, PartialEq)]
pub enum GitStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Clean,
}

pub fn load_git_status(dir_path: &Path) -> Option<HashMap<String, GitStatus>> {
    let repo = Repository::discover(dir_path).ok()?;
    let mut status_map = HashMap::new();

    let statuses = repo.statuses(None).ok()?;

    for entry in statuses.iter() {
        let path = entry.path()?;
        let flags = entry.status();

        let git_status = if flags.contains(Status::WT_DELETED) || flags.contains(Status::INDEX_DELETED) {
            GitStatus::Deleted
        } else if flags.contains(Status::WT_NEW) || flags.contains(Status::INDEX_NEW) {
            if flags.contains(Status::INDEX_NEW) {
                GitStatus::Added
            } else {
                GitStatus::Untracked
            }
        } else if flags.contains(Status::WT_MODIFIED) || flags.contains(Status::INDEX_MODIFIED) {
            GitStatus::Modified
        } else if flags.contains(Status::WT_RENAMED) || flags.contains(Status::INDEX_RENAMED) {
            GitStatus::Renamed
        } else {
            GitStatus::Clean
        };

        status_map.insert(path.to_string(), git_status);
    }

    Some(status_map)
}

pub fn get_git_status_for_file(
    git_status_map: &Option<HashMap<String, GitStatus>>,
    file_path: &Path,
    base_path: &Path
) -> GitStatus {
    if let Some(status_map) = git_status_map {
        if let Ok(relative_path) = file_path.strip_prefix(base_path) {
            let path_str = relative_path.to_string_lossy().replace('\\', "/");
            return status_map.get(&path_str).cloned().unwrap_or(GitStatus::Clean);
        }
    }
    GitStatus::Clean
}
```

#### Paso 3.2: Extraer git/ignore.rs
```rust
// src/git/ignore.rs
use std::path::Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

pub fn load_gitignore(dir_path: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(dir_path);

    let gitignore_path = dir_path.join(".gitignore");
    if gitignore_path.exists() {
        if builder.add(&gitignore_path).is_some() {
            return None;
        }
    }

    let mut current = dir_path.parent();
    while let Some(parent) = current {
        let parent_gitignore = parent.join(".gitignore");
        if parent_gitignore.exists() {
            let _ = builder.add(&parent_gitignore);
            break;
        }
        current = parent.parent();
    }

    builder.build().ok()
}

pub fn is_gitignored(gitignore: &Option<Gitignore>, path: &Path, is_dir: bool) -> bool {
    if let Some(gi) = gitignore {
        match gi.matched(path, is_dir) {
            ignore::Match::Ignore(_) => true,
            _ => false,
        }
    } else {
        false
    }
}
```

#### Paso 3.3: Extraer git/colors.rs
```rust
// src/git/colors.rs
use colored::ColoredString;
use colored::Colorize;
use crate::git::status::GitStatus;

pub fn apply_git_colors(text: String, git_status: &GitStatus, is_dir: bool, is_dimmed: bool) -> ColoredString {
    match git_status {
        GitStatus::Deleted => {
            text.red().strikethrough()
        }
        GitStatus::Untracked | GitStatus::Modified | GitStatus::Added | GitStatus::Renamed => {
            text.bright_green()
        }
        GitStatus::Clean => {
            if is_dir {
                if is_dimmed {
                    text.blue().dimmed()
                } else {
                    text.blue().bold()
                }
            } else {
                if is_dimmed {
                    text.bright_black()
                } else {
                    text.white()
                }
            }
        }
    }
}
```

#### Paso 3.4: Actualizar src/git/mod.rs
```rust
// src/git/mod.rs
pub mod status;
pub mod ignore;
pub mod colors;

// Re-export commonly used items
pub use status::{GitStatus, load_git_status, get_git_status_for_file};
pub use ignore::{load_gitignore, is_gitignored};
pub use colors::apply_git_colors;
```

#### Paso 3.5: Actualizar imports en main.rs
**En src/main.rs**, reemplazar todas las llamadas a funciones de Git:

```rust
// Reemplazar con imports limpios:
use msc::git::{GitStatus, load_git_status, get_git_status_for_file};
use msc::git::{load_gitignore, is_gitignored};
use msc::git::apply_git_colors;
```

#### Paso 3.6: Compilar, validar y commit
```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

**Validación**: 
- ✅ Debe compilar sin errores
- ✅ Sin warnings
- ✅ Integración Git completamente aislada

```bash
git add .
git commit -m "feat: phase 3 - extract git integration"
```
