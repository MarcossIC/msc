use colored::Colorize;

/// A declarative rule for regex-based HTML removal/replacement.
/// Each rule replaces ~20-30 lines of repetitive match-log-replace code.
pub struct SanitizeRule {
    pub name: &'static str,
    pub emoji: &'static str,
    pub pattern: &'static str,
    pub replacement: &'static str,
    pub dotall: bool,
}

/// A predicate-based rule for script removal.
/// Used when regex alone is insufficient — the match must be further inspected.
pub struct ScriptPredicate {
    pub name: &'static str,
    pub emoji: &'static str,
    pub matches: fn(full_script: &str, script_body: &str) -> bool,
}

/// Apply a batch of regex sanitize rules to content.
pub fn apply_sanitize_rules(content: &mut String, rules: &[SanitizeRule]) {
    for rule in rules {
        let pattern = if rule.dotall {
            format!("(?s){}", rule.pattern)
        } else {
            rule.pattern.to_string()
        };

        let Ok(re) = regex::Regex::new(&pattern) else {
            log::warn!(
                "Invalid sanitize regex for '{}': {}",
                rule.name,
                rule.pattern
            );
            continue;
        };

        let matches: Vec<String> = re
            .find_iter(content)
            .map(|m| m.as_str().to_string())
            .collect();

        if !matches.is_empty() {
            println!(
                "   {}",
                format!(
                    "{} Eliminando {} {}...",
                    rule.emoji,
                    matches.len(),
                    rule.name
                )
                .yellow()
                .dimmed()
            );
            for m in matches {
                *content = content.replace(&m, rule.replacement);
            }
        }
    }
}

/// Remove `<script>` blocks matching predicate-based rules.
///
/// Script sanitization requires inspecting the content of each `<script>` tag,
/// not just matching a regex pattern. This handles redirect scripts, cookie banners,
/// analytics, CAPTCHA, bot detection, etc.
pub fn remove_dangerous_scripts(content: &mut String) {
    let Ok(re) = regex::Regex::new(r#"(?s)<script[^>]*>(.*?)</script>"#) else {
        return;
    };

    let predicates = default_script_predicates();
    let mut removals = Vec::new();

    for cap in re.captures_iter(content) {
        let (Some(full_match), Some(body_match)) = (cap.get(0), cap.get(1)) else {
            continue;
        };
        let full_script = full_match.as_str();
        let script_body = body_match.as_str();

        for pred in &predicates {
            if (pred.matches)(full_script, script_body) {
                removals.push((full_script.to_string(), pred.name, pred.emoji));
                break; // one reason is enough
            }
        }
    }

    if !removals.is_empty() {
        println!(
            "   {}",
            format!("🛡️  Eliminando {} scripts peligrosos...", removals.len())
                .yellow()
                .dimmed()
        );
        for (script, name, emoji) in &removals {
            println!(
                "   {} {}",
                format!("{} Detectado script de {}", emoji, name)
                    .yellow()
                    .dimmed(),
                format!("({} bytes)", script.len()).dimmed()
            );
            *content = content.replace(script.as_str(), "");
        }
    }
}

fn default_script_predicates() -> Vec<ScriptPredicate> {
    vec![
        ScriptPredicate {
            name: "redirección",
            emoji: "🔀",
            matches: |_, body| {
                body.contains("redirectUrl =") || body.contains("createAccountUrl =")
            },
        },
        ScriptPredicate {
            name: "banner de cookies (inline)",
            emoji: "🍪",
            matches: |_, body| {
                body.contains("typeof CookieHelper != 'undefined'")
                    || body.contains("typeof CookieHelper !== 'undefined'")
                    || body.contains("var essentialCookiesListAll =")
                    || body.contains("var customizeCookiesTemplate =")
            },
        },
        ScriptPredicate {
            name: "banner de cookies (src)",
            emoji: "🍪",
            matches: |full, _| {
                full.contains("src=")
                    && (full.contains("cookie_banner.js") || full.contains("cookie_banner.min.js"))
            },
        },
        ScriptPredicate {
            name: "analytics/tracking",
            emoji: "📊",
            matches: |full, _| full.contains("googletagmanager.com/gtm.js"),
        },
        ScriptPredicate {
            name: "CAPTCHA/reCAPTCHA",
            emoji: "🤖",
            matches: |full, body| {
                body.contains("captchaType")
                    || body.contains("captchaToken")
                    || full.contains("www.google.com/recaptcha")
            },
        },
        ScriptPredicate {
            name: "mensajes/cookies de usuario",
            emoji: "💬",
            matches: |full, body| {
                full.contains(r#"<a onclick="hideUserMessage();"#)
                    || body.contains("setCookieAdvanced(")
            },
        },
        ScriptPredicate {
            name: "bot/whitelist detection",
            emoji: "🔍",
            matches: |_, body| {
                body.contains("'google'")
                    || body.contains("'googlebot'")
                    || body.contains("isInWhitelist")
            },
        },
        ScriptPredicate {
            name: "geo-localización",
            emoji: "🌍",
            matches: |_, body| {
                body.contains("geo-localization")
                    || body.contains("geolocalization")
                    || body.contains("geo_localization")
            },
        },
    ]
}

/// Default sanitize rules that apply to all websites.
pub fn default_sanitize_rules() -> Vec<SanitizeRule> {
    vec![
        // Phase 7: dns-prefetch links
        SanitizeRule {
            name: "elementos <link> dns-prefetch",
            emoji: "🔗",
            pattern: r#"<link[^>]*rel\s*=\s*["']dns-prefetch["'][^>]*/?>"#,
            replacement: "",
            dotall: false,
        },
        // Phase 7: GTM links
        SanitizeRule {
            name: "elementos <link> de Google Tag Manager",
            emoji: "📊",
            pattern: r#"<link[^>]*href\s*=\s*["']https?://[^"']*googletagmanager\.com[^"']*["'][^>]*/?>"#,
            replacement: "",
            dotall: false,
        },
        // Phase 8: create-account forms
        SanitizeRule {
            name: "elementos <v-create-account-form>",
            emoji: "📝",
            pattern: r#"<v-create-account-form[^>]*>.*?</v-create-account-form>"#,
            replacement: "",
            dotall: true,
        },
        // Phase 8: cookie banner menu items
        SanitizeRule {
            name: "elementos <li> con showFullCookieBanner",
            emoji: "🍪",
            pattern: r#"<li[^>]*>.*?<a[^>]*onclick\s*=\s*["']showFullCookieBanner\(\);["'][^>]*>.*?</a>.*?</li>"#,
            replacement: "",
            dotall: true,
        },
        // Phase 8: adBlock alert if-blocks
        SanitizeRule {
            name: "bloques if de adBlock alert",
            emoji: "🚫",
            pattern: r#"if\s*\(\s*!getCookieAdvanced\s*\(\s*['"]adBlockAlertHidden['"]\s*\)\s*\)\s*\{[^{}]*\}"#,
            replacement: "",
            dotall: false,
        },
        // Phase 8: holiday promo if-else blocks
        SanitizeRule {
            name: "bloques if-else de holiday_promo_prem",
            emoji: "🎁",
            pattern: r#"if\s*\(\s*typeof\s+page_params\.holiday_promo_prem\s*===\s*['"]undefined['"]\s*\)\s*\{[^{}]*\}\s*else\s*\{[^{}]*\}"#,
            replacement: "",
            dotall: false,
        },
        // Phase 10: GTM iframes
        SanitizeRule {
            name: "iframes de Google Tag Manager",
            emoji: "📊",
            pattern: r#"<iframe[^>]*src\s*=\s*["']https?://www\.googletagmanager\.com/[^"']*["'][^>]*>.*?</iframe>"#,
            replacement: "",
            dotall: true,
        },
        // Phase 10: rel=nofollow without quotes
        SanitizeRule {
            name: "atributos rel=nofollow sin comillas",
            emoji: "🔗",
            pattern: r#"\brel=nofollow\b"#,
            replacement: r#"rel="nofollow""#,
            dotall: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sanitize_rules_all_compile() {
        for rule in default_sanitize_rules() {
            let pattern = if rule.dotall {
                format!("(?s){}", rule.pattern)
            } else {
                rule.pattern.to_string()
            };
            assert!(
                regex::Regex::new(&pattern).is_ok(),
                "regex inválido en regla '{}': {}",
                rule.name,
                rule.pattern
            );
        }
    }

    #[test]
    fn removes_redirect_script() {
        let mut content =
            r#"<html><script>var redirectUrl = "http://evil.com";</script><p>ok</p></html>"#
                .to_string();
        remove_dangerous_scripts(&mut content);
        assert!(!content.contains("redirectUrl"), "got: {}", content);
        assert!(content.contains("<p>ok</p>"));
    }

    #[test]
    fn removes_gtm_analytics_script() {
        let mut content =
            r#"<script src="https://www.googletagmanager.com/gtm.js?id=GTM-XXX"></script>"#
                .to_string();
        remove_dangerous_scripts(&mut content);
        assert!(!content.contains("googletagmanager"), "got: {}", content);
    }

    #[test]
    fn keeps_harmless_script() {
        let mut content = r#"<script>console.log("hola");</script>"#.to_string();
        let original = content.clone();
        remove_dangerous_scripts(&mut content);
        assert_eq!(content, original, "no debe tocar scripts inocentes");
    }

    #[test]
    fn remove_dangerous_scripts_is_idempotent() {
        let mut content =
            r#"<script>var redirectUrl = "x";</script><div>contenido</div>"#.to_string();
        remove_dangerous_scripts(&mut content);
        let once = content.clone();
        remove_dangerous_scripts(&mut content);
        assert_eq!(
            content, once,
            "correr dos veces debe dar el mismo resultado"
        );
    }

    #[test]
    fn apply_rules_removes_dns_prefetch_link() {
        let rules = default_sanitize_rules();
        let mut content =
            r#"<head><link rel="dns-prefetch" href="//cdn.x.com"><title>t</title></head>"#
                .to_string();
        apply_sanitize_rules(&mut content, &rules);
        assert!(!content.contains("dns-prefetch"), "got: {}", content);
        assert!(content.contains("<title>t</title>"));
    }

    #[test]
    fn apply_rules_quotes_bare_nofollow() {
        let rules = default_sanitize_rules();
        let mut content = r#"<a rel=nofollow href="x">link</a>"#.to_string();
        apply_sanitize_rules(&mut content, &rules);
        assert!(content.contains(r#"rel="nofollow""#), "got: {}", content);
    }
}
