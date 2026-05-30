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
