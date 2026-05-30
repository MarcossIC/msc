use anyhow::{Context, Result};
use colored::Colorize;

use super::super::context::ProcessingContext;

/// Phase 10: Fix JavaScript variables and HTML quirks that break offline viewing.
///
/// Handles:
/// - `preventAutoplayForAVModal` forced to `true`
/// - `embedCode` JSON fields with improperly escaped quotes in iframes
pub fn apply_quirk_fixes(ctx: &mut ProcessingContext) -> Result<()> {
    fix_autoplay_prevention(ctx)?;
    fix_embed_code_escaping(ctx)?;
    Ok(())
}

/// Force `preventAutoplayForAVModal = true;` to disable autoplay in offline mode.
fn fix_autoplay_prevention(ctx: &mut ProcessingContext) -> Result<()> {
    let re = regex::Regex::new(r"preventAutoplayForAVModal\s*=\s*[^;]+;")
        .context("Failed to create preventAutoplayForAVModal regex")?;

    if re.is_match(&ctx.content) {
        println!(
            "   {}",
            "🎬 Ajustando preventAutoplayForAVModal = true..."
                .cyan()
                .dimmed()
        );
        ctx.content = re
            .replace_all(&ctx.content, "preventAutoplayForAVModal = true;")
            .to_string();
    }

    Ok(())
}

/// Fix `"embedCode":"<iframe src="..." ...>"` → properly escape quotes for valid JSON.
fn fix_embed_code_escaping(ctx: &mut ProcessingContext) -> Result<()> {
    let re = regex::Regex::new(r#""embedCode"\s*:\s*"(<iframe[^>]*>(?:</iframe>)?)"#)
        .context("Failed to create embedCode regex")?;

    let mut replacements = Vec::new();

    for cap in re.captures_iter(&ctx.content) {
        let (Some(full_match), Some(iframe_content)) = (cap.get(0), cap.get(1)) else {
            continue;
        };
        let original_iframe = iframe_content.as_str();
        let full_match_str = full_match.as_str();

        let escaped_iframe = original_iframe
            .replace('\\', r"\\")
            .replace('"', r#"\""#)
            .replace('/', r"\/");

        let corrected = format!(r#""embedCode":"{}""#, escaped_iframe);

        if full_match_str != corrected {
            replacements.push((full_match_str.to_string(), corrected));
        }
    }

    if !replacements.is_empty() {
        println!(
            "   {}",
            format!(
                "🔧 Corrigiendo {} embedCode con comillas mal escapadas...",
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

    /// Run `apply_quirk_fixes` over `content` and return the resulting HTML.
    /// Paths are synthetic — these phases never touch disk.
    fn run(content: &str) -> String {
        let blacklist = Blacklist::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let base_dir = Path::new("/site");
        let file_path = Path::new("/site/page.html");
        let mut ctx = ProcessingContext::new(
            file_path,
            base_dir,
            &base_url,
            &blacklist,
            content.to_string(),
        );
        apply_quirk_fixes(&mut ctx).unwrap();
        ctx.content
    }

    #[test]
    fn forces_autoplay_prevention_true() {
        let out = run("var x; preventAutoplayForAVModal = false; var y;");
        assert!(
            out.contains("preventAutoplayForAVModal = true;"),
            "got: {}",
            out
        );
        assert!(!out.contains("= false"), "got: {}", out);
    }

    #[test]
    fn escapes_embed_code_quotes() {
        let out = run(r#"{"embedCode":"<iframe src="http://x.com/v"></iframe>"}"#);
        // Las comillas internas del iframe deben quedar escapadas para JSON válido.
        assert!(out.contains(r#"src=\""#), "got: {}", out);
        assert!(out.contains(r"\/"), "got: {}", out);
    }

    #[test]
    fn leaves_content_without_quirks_untouched() {
        let input = "<html><body><p>nada que arreglar</p></body></html>";
        let out = run(input);
        assert_eq!(out, input);
    }
}
