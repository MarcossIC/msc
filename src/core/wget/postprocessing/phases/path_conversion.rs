use anyhow::{Context, Result};
use colored::Colorize;

use super::super::context::ProcessingContext;

/// Phase 3: Convert absolute server paths (`/path/to/file`) to relative paths
/// for offline viewing.
///
/// When a page references `/blog/post`, that works on the live server but not
/// when viewing locally. This converts it to `../../blog/post` (appropriate
/// number of `../` based on file nesting depth).
pub fn convert_absolute_paths(ctx: &mut ProcessingContext) -> Result<()> {
    let re = regex::Regex::new(r#"((?:src|href)\s*=\s*["'])(/[^"']+)(["'])"#)
        .context("Failed to create absolute path regex")?;

    // Calculate nesting depth relative to base_dir
    let relative_to_base =
        pathdiff::diff_paths(ctx.base_dir, &ctx.parent_dir).unwrap_or_default();
    let depth = relative_to_base.components().count();

    let mut prefix_to_root = String::new();
    for _ in 0..depth {
        prefix_to_root.push_str("../");
    }

    let mut replacements = Vec::new();

    for cap in re.captures_iter(&ctx.content) {
        let (Some(full_match), Some(attr_prefix), Some(abs_path), Some(quote_suffix)) =
            (cap.get(0), cap.get(1), cap.get(2), cap.get(3))
        else {
            continue;
        };

        let abs_path_str = abs_path.as_str();

        // Skip protocol-relative URLs and relative paths
        if abs_path_str.starts_with("//") || abs_path_str.starts_with("./") {
            continue;
        }

        let path_without_slash = abs_path_str.trim_start_matches('/');
        let new_relative_path = if depth == 0 {
            path_without_slash.to_string()
        } else {
            format!("{}{}", prefix_to_root, path_without_slash)
        };

        let replacement = format!(
            "{}{}{}",
            attr_prefix.as_str(),
            new_relative_path,
            quote_suffix.as_str()
        );
        replacements.push((full_match.as_str().to_string(), replacement));
    }

    if !replacements.is_empty() {
        println!(
            "   {}",
            format!(
                "🔧 Corrigiendo {} rutas absolutas a rutas relativas...",
                replacements.len()
            )
            .cyan()
            .dimmed()
        );
        for (old, new) in replacements {
            ctx.content = ctx.content.replace(&old, &new);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Blacklist;
    use std::path::Path;
    use url::Url;

    /// Run `convert_absolute_paths`. `depth` is derived from how deeply
    /// `file_path` nests under `base_dir` — no disk access involved.
    fn run(file_path: &str, base_dir: &str, content: &str) -> String {
        let blacklist = Blacklist::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let bd = Path::new(base_dir);
        let fp = Path::new(file_path);
        let mut ctx =
            ProcessingContext::new(fp, bd, &base_url, &blacklist, content.to_string());
        convert_absolute_paths(&mut ctx).unwrap();
        ctx.content
    }

    #[test]
    fn root_depth_strips_leading_slash() {
        let out = run("/site/page.html", "/site", r#"<a href="/blog/post">x</a>"#);
        assert!(out.contains(r#"href="blog/post""#), "got: {}", out);
    }

    #[test]
    fn nested_depth_prepends_parent_dirs() {
        let out = run("/site/a/b/page.html", "/site", r#"<img src="/img/x.png">"#);
        assert!(out.contains(r#"src="../../img/x.png""#), "got: {}", out);
    }

    #[test]
    fn skips_protocol_relative_urls() {
        let input = r#"<img src="//cdn.com/x.png">"#;
        let out = run("/site/page.html", "/site", input);
        assert_eq!(out, input, "las URLs // no se tocan");
    }
}
