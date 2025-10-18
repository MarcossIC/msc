
### FASE 1: Infraestructura Base (BAJO RIESGO)
**Duración estimada**: 2-3 horas
**Riesgo**: Bajo
**Objetivo**: Crear estructura base, error handling y logging

#### Paso 1.1: Crear estructura de directorios
```bash
# Windows PowerShell
New-Item -ItemType Directory -Path src/commands -Force
New-Item -ItemType Directory -Path src/core -Force
New-Item -ItemType Directory -Path src/ui -Force
New-Item -ItemType Directory -Path src/platform -Force
New-Item -ItemType Directory -Path src/git -Force
New-Item -ItemType Directory -Path src/utils -Force
New-Item -ItemType Directory -Path tests/integration -Force
New-Item -ItemType Directory -Path tests/fixtures/test_workspace -Force

# Unix/Linux/Mac
mkdir -p src/{commands,core,ui,platform,git,utils}
mkdir -p tests/{integration,fixtures/test_workspace}
```

#### Paso 1.2: Crear src/error.rs
```rust
// src/error.rs
use thiserror::Error;

/// Custom error types for MSC CLI
#[derive(Error, Debug)]
pub enum MscError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Path error: {0}")]
    Path(String),
    
    #[error("Git error: {0}")]
    Git(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias using MscError
pub type Result<T> = std::result::Result<T, MscError>;
```

#### Paso 1.3: Crear archivos mod.rs
Crear los siguientes archivos con contenido inicial:

```rust
// src/commands/mod.rs
// Command handlers will be added in Phase 6

// src/core/mod.rs
// Core business logic will be added in Phase 5

// src/ui/mod.rs
pub mod formatters;
pub mod prompts;

// Re-exports
pub use formatters::{format_size, format_time, format_permissions};
pub use prompts::{confirm, warn, info, success, error};

// src/platform/mod.rs
pub mod elevation;
pub mod temp_dirs;
pub mod fs;

// Re-exports
pub use elevation::{is_elevated, elevate_and_rerun, ensure_elevated};
pub use temp_dirs::get_temp_directories;
pub use fs::is_hidden;

// src/git/mod.rs
pub mod status;
pub mod ignore;
pub mod colors;

// Re-exports
pub use status::{GitStatus, load_git_status, get_git_status_for_file};
pub use ignore::{load_gitignore, is_gitignored};
pub use colors::apply_git_colors;

// src/utils/mod.rs
pub mod icons;
pub mod path;

// Re-exports
pub use icons::get_file_icon;
```

#### Paso 1.4: Crear src/lib.rs
```rust
// src/lib.rs
//! MSC CLI - A modular command-line interface tool
//!
//! This library provides the core functionality for the MSC CLI,
//! organized into modular components for better maintainability.

pub mod commands;
pub mod core;
pub mod ui;
pub mod platform;
pub mod git;
pub mod utils;
pub mod error;

// Re-export commonly used types
pub use error::{MscError, Result};
```

#### Paso 1.5: Actualizar main.rs para incluir lib y logging
```rust
// Al inicio de src/main.rs, REEMPLAZAR imports con:
use msc::commands;
use msc::core;
use msc::ui;
use msc::platform;
use msc::git;
use msc::utils;
use msc::error::Result;

// Agregar en la función main (primera línea):
fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    log::info!("Starting MSC CLI");
    
    // ... resto del código
}
```

#### Paso 1.6: Extraer utils/icons.rs
**Origen**: `src/main.rs` función `get_file_icon()`
**Destino**: `src/utils/icons.rs`

```rust
// src/utils/icons.rs
use std::path::Path;

/// Returns an emoji icon based on file extension
pub fn get_file_icon(filename: &str) -> &'static str {
    let path = Path::new(filename);

    if let Some(ext) = path.extension() {
        match ext.to_str().unwrap_or("").to_lowercase().as_str() {
            // Programming languages
            "rs" => "🦀",
            "py" => "🐍",
            "js" | "jsx" | "mjs" | "cjs" => "🟨",
            "ts" | "tsx" => "🔷",
            "vue" => "🟩",
            "svelte" => "🟥",
            "java" => "☕",
            "php" => "🐘",
            "swift" => "🟠",
            "astro" => "🚀",
            "pl" => "🐪",
            "lua" => "🌙",
            "r" => "📊",
            "cs" => "🟣",
            "rb" => "💎",
            "dart" | "scala" | "hs" | "clj" | "cljs" | "cljc" | "ex" | "exs" | "m" | "f90" | "for" | "jl" | "c" | "cpp" | "tsv" => "📘",

            // Web
            "html" | "htm" => "🌐",
            "rst" => "🌐",
            "css" | "scss" | "sass" => "🎨",
            "svg" => "🎨",

            // Data formats
            "json" => "🔧",
            "xml" => "📰",
            "yaml" | "yml" | "uml" | "toml" => "📒",
            "ini" | "cfg" | "conf" | ".editorconfig" | ".dockerignore" | ".gitignore" | ".gitattributes" => "⚙",
            "env" => "🌱",
            "sql" | "sqlite" | "sqlite3" | "db" | "mdb" | "accdb" | "dbf" | "parquet" | "avro" | "orc" => "🗄️",

            // Documents
            "md" => "📖",
            "txt" => "📝",
            "pdf" => "📄",
            "doc" | "docx" => "📄",
            "xls" | "xlsx" | "xlsm" => "📊",
            "ppt" | "pptx" => "🎞️",
            "odt" | "ods" | "odp" => "📄",

            // Images
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "heic" | "psd" | "ai" | "xcf" => "🖼️",
            "ico" => "🎯",

            // Fonts
            "ttf" | "otf" | "woff" | "woff2" => "🔤",

            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "aac" => "🎵",

            // Video
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "webm" => "🎬",

            // Archives
            "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "iso" | "cab" | "zst" | "lzma" | "xz" => "📦",

            // Executables
            "exe" | "msi" | "deb" | "rpm" | "dmg" => "⚡",
            "apk" | "ipa" => "📱",

            // Lock files
            "lock" => "🔒",

            // Logs
            "log" | "logs" | "bak" | "tmp" | "temp" | "swp" | "torrent" => "📋",

            // Certificates
            "crt" | "pem" | "key" | "cert" | "pfx" | "p12" | "der" | "cer" => "🔐",

            // Suspicious/unknown potentially dangerous
            "bat" | "cmd" | "ps1" | "sh" | "bash" | "scr" | "vbs" | "jar" => "❓",

            _ => "📄",
        }
    } else {
        // Files without extension
        let name_lower = filename.to_lowercase();
        match name_lower.as_str() {
            "head" | "config" | "description" | "exclude" | "hooks" | "info" | "objects" | "refs" => "⚙",
            "makefile" | "dockerfile" | "license" | "readme" | "changelog" | "authors" => "📄",
            _ => "📄",
        }
    }
}
```

**Actualizar src/utils/mod.rs**:
```rust
// src/utils/mod.rs
pub mod icons;

// Re-export
pub use icons::get_file_icon;
```

**En src/main.rs**, reemplazar todas las llamadas a `get_file_icon()` por `utils::get_file_icon()`.

#### Paso 1.7: Extraer ui/formatters.rs
**Origen**: `src/main.rs` funciones `format_size()`, `format_time()`, `format_permissions()`
**Destino**: `src/ui/formatters.rs`

```rust
// src/ui/formatters.rs
use std::time::SystemTime;
use chrono::{DateTime, Local};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Format bytes to human-readable size (B, KB, MB, GB)
pub fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format SystemTime to readable date-time string
pub fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Format file permissions (Windows attributes or Unix mode)
#[cfg(windows)]
pub fn format_permissions(metadata: &std::fs::Metadata) -> String {
    let attributes = metadata.file_attributes();
    let mut perms = String::new();

    if (attributes & 1) != 0 { perms.push('R'); } else { perms.push('r'); } // Read-only
    if (attributes & 2) != 0 { perms.push('H'); } else { perms.push('-'); } // Hidden
    if (attributes & 4) != 0 { perms.push('S'); } else { perms.push('-'); } // System
    if (attributes & 16) != 0 { perms.push('D'); } else { perms.push('-'); } // Directory
    if (attributes & 32) != 0 { perms.push('A'); } else { perms.push('-'); } // Archive

    perms
}

#[cfg(not(windows))]
pub fn format_permissions(metadata: &std::fs::Metadata) -> String {
    let mode = metadata.permissions().mode();
    let mut perms = String::new();

    // Owner permissions
    perms.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o100 != 0 { 'x' } else { '-' });

    // Group permissions
    perms.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o010 != 0 { 'x' } else { '-' });

    // Other permissions
    perms.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o001 != 0 { 'x' } else { '-' });

    perms
}
```

**Actualizar src/ui/mod.rs**:
```rust
// src/ui/mod.rs
pub mod formatters;

// Re-exports
pub use formatters::{format_size, format_time, format_permissions};
```

**En src/main.rs**, reemplazar llamadas a estas funciones por `ui::format_size()`, `ui::format_time()`, etc.

#### Paso 1.8: Compilar, validar y commit
```bash
cargo build
cargo test
```

**Validación**: 
- ✅ Debe compilar sin errores
- ✅ Debe compilar sin warnings
- ✅ Tests deben pasar (si hay)

```bash
git add .
git commit -m "feat: phase 1 - infrastructure setup with error handling and logging"
```

---
