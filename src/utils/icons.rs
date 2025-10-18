use std::path::Path;

/// Returns an appropriate icon for a given filename based on its extension
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
