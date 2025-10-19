//! File icon utilities
//!
//! This module provides functions to get appropriate icons/emojis
//! for different file types based on their extensions.

use std::path::Path;

/// Returns an appropriate icon for a given filename based on its extension
///
/// # Arguments
///
/// * `filename` - The name of the file (with or without extension)
///
/// # Returns
///
/// An emoji icon representing the file type
///
/// # Examples
///
/// ```
/// use msc::utils::icons::get_file_icon;
///
/// assert_eq!(get_file_icon("main.rs"), "🦀");
/// assert_eq!(get_file_icon("script.py"), "🐍");
/// assert_eq!(get_file_icon("README.md"), "📖");
/// ```
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
        // Files without extension - check if they are configuration files
        let name_lower = filename.to_lowercase();
        match name_lower.as_str() {
            "head" | "config" | "description" | "exclude" | "hooks" | "info" | "objects" | "refs" => "⚙",
            "makefile" | "dockerfile" | "license" | "readme" | "changelog" | "authors" => "📄",
            _ => "📄",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_file_icon() {
        assert_eq!(get_file_icon("main.rs"), "🦀");
        assert_eq!(get_file_icon("lib.rs"), "🦀");
    }

    #[test]
    fn test_python_file_icon() {
        assert_eq!(get_file_icon("script.py"), "🐍");
    }

    #[test]
    fn test_javascript_icons() {
        assert_eq!(get_file_icon("app.js"), "🟨");
        assert_eq!(get_file_icon("component.jsx"), "🟨");
        assert_eq!(get_file_icon("module.mjs"), "🟨");
    }

    #[test]
    fn test_typescript_icons() {
        assert_eq!(get_file_icon("app.ts"), "🔷");
        assert_eq!(get_file_icon("Component.tsx"), "🔷");
    }

    #[test]
    fn test_web_files() {
        assert_eq!(get_file_icon("index.html"), "🌐");
        assert_eq!(get_file_icon("styles.css"), "🎨");
        assert_eq!(get_file_icon("logo.svg"), "🎨");
    }

    #[test]
    fn test_data_formats() {
        assert_eq!(get_file_icon("config.json"), "🔧");
        assert_eq!(get_file_icon("data.xml"), "📰");
        assert_eq!(get_file_icon("settings.yaml"), "📒");
        assert_eq!(get_file_icon("app.toml"), "📒");
    }

    #[test]
    fn test_documents() {
        assert_eq!(get_file_icon("README.md"), "📖");
        assert_eq!(get_file_icon("notes.txt"), "📝");
        assert_eq!(get_file_icon("document.pdf"), "📄");
    }

    #[test]
    fn test_images() {
        assert_eq!(get_file_icon("photo.jpg"), "🖼️");
        assert_eq!(get_file_icon("image.png"), "🖼️");
        assert_eq!(get_file_icon("icon.ico"), "🎯");
    }

    #[test]
    fn test_archives() {
        assert_eq!(get_file_icon("archive.zip"), "📦");
        assert_eq!(get_file_icon("backup.tar"), "📦");
        assert_eq!(get_file_icon("data.7z"), "📦");
    }

    #[test]
    fn test_executables() {
        assert_eq!(get_file_icon("app.exe"), "⚡");
        assert_eq!(get_file_icon("installer.msi"), "⚡");
        assert_eq!(get_file_icon("app.apk"), "📱");
    }

    #[test]
    fn test_lock_files() {
        assert_eq!(get_file_icon("Cargo.lock"), "🔒");
        assert_eq!(get_file_icon("yarn.lock"), "🔒");
        assert_eq!(get_file_icon("package-lock.json"), "🔧");
    }

    #[test]
    fn test_log_files() {
        assert_eq!(get_file_icon("app.log"), "📋");
        assert_eq!(get_file_icon("backup.bak"), "📋");
    }

    #[test]
    fn test_certificates() {
        assert_eq!(get_file_icon("cert.pem"), "🔐");
        assert_eq!(get_file_icon("private.key"), "🔐");
    }

    #[test]
    fn test_scripts() {
        assert_eq!(get_file_icon("script.sh"), "❓");
        assert_eq!(get_file_icon("install.bat"), "❓");
        assert_eq!(get_file_icon("automation.ps1"), "❓");
    }

    #[test]
    fn test_database_files() {
        assert_eq!(get_file_icon("query.sql"), "🗄️");
        assert_eq!(get_file_icon("data.db"), "🗄️");
        assert_eq!(get_file_icon("database.sqlite"), "🗄️");
    }

    #[test]
    fn test_unknown_extension() {
        assert_eq!(get_file_icon("file.unknown"), "📄");
        assert_eq!(get_file_icon("test.xyz"), "📄");
    }

    #[test]
    fn test_no_extension_config_files() {
        assert_eq!(get_file_icon("Makefile"), "📄");
        assert_eq!(get_file_icon("Dockerfile"), "📄");
        assert_eq!(get_file_icon("config"), "⚙");
        assert_eq!(get_file_icon("LICENSE"), "📄");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(get_file_icon("FILE.RS"), "🦀");
        assert_eq!(get_file_icon("SCRIPT.PY"), "🐍");
        assert_eq!(get_file_icon("APP.JS"), "🟨");
    }
}
