use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;

/// Calculate the local file path where wget would save a given URL
/// This mirrors wget's behavior with --adjust-extension and directory structure
pub fn calculate_local_path_for_url(url: &Url, base_dir: &Path) -> Option<PathBuf> {
    let domain = url.domain()?;
    let path = url.path();
    let query = url.query(); // Get query parameters

    let mut local_path = base_dir.join(domain);

    if path == "/" || path.is_empty() {
        local_path.push("index.html");
    } else {
        // Remove leading slash and decode percent encoding
        let rel_path = path.trim_start_matches('/');

        if path.ends_with('/') {
            // Directory path - wget adds index.html
            local_path.push(rel_path);
            local_path.push("index.html");
        } else {
            // File path - need to handle query params
            let path_parts: Vec<&str> = rel_path.split('/').collect();
            let mut file_name = path_parts.last().unwrap_or(&"index.html").to_string();

            // If there are query parameters, wget converts them to @-notation
            // Example: "page.php?id=123" becomes "page.php@id=123.html"
            // Note: wget does NOT remove the original extension, it just appends @query.html
            if let Some(query_str) = query {
                // Append query with @ separator (keep the original extension)
                file_name = format!("{}@{}", file_name, query_str);
            }

            // Push the directory structure (excluding the filename)
            if path_parts.len() > 1 {
                for part in &path_parts[..path_parts.len() - 1] {
                    local_path.push(part);
                }
            }

            // Push the filename (with query params if any)
            local_path.push(&file_name);

            // wget's behavior with --adjust-extension:
            // - If the file has query params, wget ALWAYS adds .html at the end
            //   Example: "page.php?id=123" -> "page.php@id=123.html"
            //   Example: "page.html?id=123" -> "page.html@id=123.html"
            // - If no query params and no extension -> adds .html
            // - If no query params and non-html extension (like .php) -> adds .html

            if query.is_some() {
                // Always add .html when there are query params
                let new_name = format!("{}.html", local_path.file_name()?.to_string_lossy());
                local_path.set_file_name(new_name);
            } else {
                // No query params - apply normal wget logic
                let has_extension = file_name.contains('.');
                if !has_extension {
                    // No extension - wget will add .html
                    local_path.set_extension("html");
                } else if !file_name.ends_with(".html") && !file_name.ends_with(".htm") {
                    // Check if it looks like a HTML page (no other file extension)
                    let ext = local_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext != "html"
                        && ext != "htm"
                        && ![
                            "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif",
                            "svg", "ico", "woff", "woff2", "ttf", "eot",
                        ]
                        .contains(&ext)
                    {
                        // Likely an HTML page with unusual extension - wget adds .html
                        let new_name =
                            format!("{}.html", local_path.file_name()?.to_string_lossy());
                        local_path.set_file_name(new_name);
                    }
                }
            }

            // Try to find the file - if it doesn't exist with query params, try without
            if !local_path.exists() && query.is_some() {
                // Reconstruct path without query params
                let mut fallback_path = base_dir.join(domain);

                // Push directory structure
                if path_parts.len() > 1 {
                    for part in &path_parts[..path_parts.len() - 1] {
                        fallback_path.push(part);
                    }
                }

                // Push original filename without query
                let original_file_name = path_parts.last().unwrap_or(&"index.html");
                fallback_path.push(original_file_name);

                // Handle extension
                let orig_has_extension = original_file_name.contains('.');
                if !orig_has_extension {
                    fallback_path.set_extension("html");
                } else if !original_file_name.ends_with(".html")
                    && !original_file_name.ends_with(".htm")
                {
                    let ext = fallback_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext != "html"
                        && ext != "htm"
                        && ![
                            "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif",
                            "svg", "ico", "woff", "woff2", "ttf", "eot",
                        ]
                        .contains(&ext)
                    {
                        let new_name =
                            format!("{}.html", fallback_path.file_name()?.to_string_lossy());
                        fallback_path.set_file_name(new_name);
                    }
                }

                if fallback_path.exists() {
                    return Some(fallback_path);
                }
            }
        }
    }

    Some(local_path)
}

/// Check if a path is already a local relative path (not a remote URL)
pub fn is_local_path(path: &str) -> bool {
    // Check if it's a local relative path
    path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with("assets/")
        || path.starts_with("../assets/")
        || path.starts_with("../../assets/")
        || (!path.starts_with("http://")
            && !path.starts_with("https://")
            && !path.starts_with("//")
            && !path.starts_with("/") // Absolute paths from root are not local relative
            && path.contains("assets")) // Contains 'assets' but no protocol
}

/// Calculate possible local file paths where the URL might be saved
/// Returns multiple possibilities to handle both flat and nested directory structures
pub fn calculate_possible_local_paths(url: &Url, base_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let Some(domain) = url.domain() else {
        return paths;
    };

    let url_path = url.path();

    // Build the relative path from URL
    let mut rel_path_parts = Vec::new();

    if url_path == "/" || url_path.is_empty() {
        rel_path_parts.push("index.html".to_string());
    } else {
        let rel_path = url_path.trim_start_matches('/');

        if url_path.ends_with('/') {
            rel_path_parts.push(rel_path.to_string());
            rel_path_parts.push("index.html".to_string());
        } else {
            rel_path_parts.push(rel_path.to_string());

            // Add .html extension if needed
            let has_extension = rel_path.contains('.')
                && rel_path
                    .split('/')
                    .next_back()
                    .map(|f| f.contains('.'))
                    .unwrap_or(false);

            if !has_extension {
                // Modify last part to add .html
                if let Some(last) = rel_path_parts.last_mut() {
                    *last = format!("{}.html", last);
                }
            } else if !url_path.ends_with(".html") && !url_path.ends_with(".htm") {
                let ext = rel_path.split('.').next_back().unwrap_or("");
                if ![
                    "html", "htm", "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg",
                    "gif", "svg", "ico", "woff", "woff2", "ttf", "eot",
                ]
                .contains(&ext)
                {
                    if let Some(last) = rel_path_parts.last_mut() {
                        *last = format!("{}.html", last);
                    }
                }
            }
        }
    }

    // Build final path string
    let final_rel_path = rel_path_parts.join("");

    // Check if base_dir ends with the domain name
    // This works for both relative (./manhwa-espanol.com) and absolute paths
    // (C:\Users\...\manhwa-espanol.com)
    let base_dir_name = base_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if base_dir_name == domain {
        // base_dir already includes domain, so try both options:
        // Option 1: base_dir/path (without domain duplication - CORRECT)
        let path_without_domain = base_dir.join(&final_rel_path);
        paths.push(path_without_domain);

        // Option 2: base_dir/domain/path (with domain duplication - legacy/alternative)
        let path_with_domain = base_dir.join(domain).join(&final_rel_path);
        paths.push(path_with_domain);
    } else {
        // base_dir doesn't include domain, so only try with domain
        let path_with_domain = base_dir.join(domain).join(&final_rel_path);
        paths.push(path_with_domain);
    }

    paths
}

/// Extract a safe filename from a URL, handling complex CDN URLs with path segments after extensions
/// Example: "https://cdn.com/video.png/plain/rs:fit:323:182?query=123" -> "video_hash.png"
/// Example: "https://upload.wikimedia.org/wikipedia/en/thumb/1/1f/Reddit_logo_2023.svg/330px-Reddit_logo_2023.svg.png" -> "330px-Reddit_logo_2023.svg.png"
pub fn extract_filename_from_url(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // List of known file extensions
    let known_extensions = [
        ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp", ".bmp", ".ico", ".mp4", ".webm", ".ogv",
        ".avi", ".mov", ".m3u8", ".ts", ".mp3", ".ogg", ".wav", ".m4a", ".css", ".js", ".json",
        ".xml", ".html", ".htm", ".pdf", ".txt", ".woff", ".woff2", ".ttf", ".eot", ".otf",
    ];

    // Remove query parameters and fragments
    let url_without_query = url.split('?').next().unwrap_or(url);
    let url_without_fragment = url_without_query
        .split('#')
        .next()
        .unwrap_or(url_without_query);

    // First, try to get the last path segment as a filename
    // This handles cases like: /path/to/file.png or /path/to/330px-image.svg.png
    if let Some(last_segment) = url_without_fragment.split('/').next_back() {
        // Check if this segment has a known extension
        let segment_lower = last_segment.to_lowercase();

        for ext in &known_extensions {
            if segment_lower.ends_with(ext) {
                // This looks like a valid filename with extension
                // Clean it to make it filesystem-safe but preserve dots in filename
                let clean_name: String = last_segment
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                    .collect();

                if !clean_name.is_empty() && clean_name.contains('.') {
                    return clean_name;
                }
            }
        }
    }

    // Fallback: Try to find a file extension anywhere in the URL path (old behavior)
    let mut found_extension: Option<&str> = None;
    let mut name_before_extension: Option<&str> = None;

    for ext in &known_extensions {
        if let Some(pos) = url_without_fragment.rfind(ext) {
            // Found an extension, extract everything before it
            let before = &url_without_fragment[..pos];

            // Get the filename part (after the last '/')
            if let Some(last_slash_pos) = before.rfind('/') {
                name_before_extension = Some(&before[last_slash_pos + 1..]);
            } else {
                name_before_extension = Some(before);
            }

            found_extension = Some(ext);
            break;
        }
    }

    // If we found an extension, create a safe filename
    if let (Some(name), Some(ext)) = (name_before_extension, found_extension) {
        // Clean the name to make it filesystem-safe
        let clean_name: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();

        if !clean_name.is_empty() {
            return format!("{}{}", clean_name, ext);
        }
    }

    // Final fallback: create a hash-based filename
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();

    // Try to detect extension from content-type later, for now use a generic one
    // If there was an extension found, use it, otherwise use .bin
    if let Some(ext) = found_extension {
        format!("resource_{:x}{}", hash, ext)
    } else {
        format!("resource_{:x}.bin", hash)
    }
}

pub fn download_resource(url: &str, path: &PathBuf) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!("Status: {}", response.status()));
    }

    let bytes = response.bytes()?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn is_placeholder_image(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    let valid_extensions = [".jpg", ".png", ".gif", ".svg", ".webp"];
    let valid_names = [
        "loading",
        "readerarea",
        "reader",
        "launching",
        "placeholder",
        "skeleton",
        "spinner",
        "indicator",
        "loader",
    ];

    let has_valid_ext = valid_extensions.iter().any(|ext| url_lower.ends_with(ext));
    if !has_valid_ext {
        return false;
    }

    valid_names.iter().any(|name| url_lower.contains(name))
}
