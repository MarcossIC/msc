use anyhow::{Context, Result};

use super::super::context::ProcessingContext;

/// Phase 4: Apply accumulated URL replacements to the HTML content.
///
/// Images require special handling: we locate the parent `<img>` tag and replace
/// within its scope to avoid corrupting non-image attributes with the same URL.
pub fn apply_replacements(ctx: &mut ProcessingContext) -> Result<()> {
    let src_pattern = regex::Regex::new(r#"src\s*=\s*["'][^"']*["']"#)
        .context("Failed to create regex pattern")?;

    let replacements = std::mem::take(&mut ctx.replacements);

    for repl in &replacements {
        if repl.is_image {
            apply_image_replacement(ctx, repl, &src_pattern);
        } else {
            ctx.content = ctx.content.replace(&repl.target, &repl.replacement);
        }
    }

    Ok(())
}

/// Image replacement: find the `<img>` tag containing the target URL and
/// replace within the tag scope. Falls back to global replace if scoped fails.
fn apply_image_replacement(
    ctx: &mut ProcessingContext,
    repl: &super::super::context::Replacement,
    src_pattern: &regex::Regex,
) {
    // Try both the original URL and its HTML-encoded variant
    let target_encoded = repl.target.replace('&', "&amp;");
    let targets_to_try = if repl.target != target_encoded {
        vec![repl.target.clone(), target_encoded]
    } else {
        vec![repl.target.clone()]
    };

    let mut replaced_any = false;

    for curr_target in &targets_to_try {
        let mut search_start = 0;
        while let Some(idx) = ctx.content[search_start..].find(curr_target.as_str()) {
            let absolute_idx = search_start + idx;
            let tag_start = ctx.content[..absolute_idx].rfind("<img");

            if let Some(start) = tag_start {
                let tag_end_offset = ctx.content[absolute_idx..].find('>');

                if let Some(end_offset) = tag_end_offset {
                    let end = absolute_idx + end_offset + 1;
                    let tag_content = &ctx.content[start..end];

                    if tag_content.contains(curr_target.as_str()) {
                        let mut new_tag = tag_content.to_string();
                        new_tag = new_tag.replace(curr_target.as_str(), &repl.replacement);
                        new_tag = src_pattern
                            .replace(
                                &new_tag,
                                format!("src=\"{}\"", repl.replacement).as_str(),
                            )
                            .to_string();

                        ctx.content.replace_range(start..end, &new_tag);
                        search_start = start + new_tag.len();
                        replaced_any = true;
                        continue;
                    }
                }
            }
            search_start = absolute_idx + curr_target.len();
        }
    }

    if !replaced_any {
        ctx.content = ctx.content.replace(&repl.target, &repl.replacement);
    }
}
