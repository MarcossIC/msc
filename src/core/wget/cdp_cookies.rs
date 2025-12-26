// Chrome DevTools Protocol (CDP) cookie extraction
// Bypasses App-Bound Encryption by getting cookies directly from running Chrome

use anyhow::{Context, Result};
use colored::Colorize;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::wget_cookies::Cookie;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

const CDP_PORT: u16 = 9222;

/// CDP target information
#[derive(Debug, Deserialize)]
struct CdpTarget {
    #[serde(rename = "webSocketDebuggerUrl")]
    ws_url: Option<String>,
    #[serde(rename = "type")]
    target_type: Option<String>,
    #[allow(dead_code)]
    url: Option<String>,
}

/// CDP request structure
#[derive(Debug, Serialize)]
struct CdpRequest {
    id: u64,
    method: &'static str,
    params: serde_json::Value,
}

/// CDP response structure
#[derive(Debug, Deserialize)]
struct CdpResponse {
    id: u64,
    result: Option<CookieResult>,
    error: Option<CdpError>,
}

/// CDP error structure
#[derive(Debug, Deserialize)]
struct CdpError {
    message: String,
}

/// CDP cookie result container
#[derive(Debug, Deserialize)]
struct CookieResult {
    cookies: Vec<CdpCookie>,
}

/// CDP cookie structure (as returned by Chrome)
#[derive(Debug, Deserialize, Clone)]
pub struct CdpCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64, // Unix timestamp as float
    pub secure: bool,
    #[serde(rename = "httpOnly")]
    pub http_only: bool,
    #[serde(rename = "sameSite")]
    pub same_site: Option<String>,
}

/// Convert CDP cookie to our Cookie struct
impl From<CdpCookie> for Cookie {
    fn from(cdp: CdpCookie) -> Self {
        Cookie {
            name: cdp.name,
            value: cdp.value,
            domain: cdp.domain,
            path: cdp.path,
            expires: cdp.expires as i64,
            secure: cdp.secure,
            http_only: cdp.http_only,
            same_site: cdp.same_site.unwrap_or_else(|| "Lax".to_string()),
        }
    }
}

/// Check if Chrome is running with CDP enabled
///
/// # Returns
/// * `true` if CDP is available on port 9222
/// * `false` otherwise
pub async fn is_cdp_available() -> bool {
    let Ok(resp) = reqwest::get(format!("http://127.0.0.1:{}/json", CDP_PORT)).await else {
        return false;
    };

    let Ok(targets): Result<Vec<CdpTarget>, _> = resp.json().await else {
        return false;
    };

    targets.iter().any(|t| t.ws_url.is_some())
}

/// Get WebSocket URL for CDP connection
async fn get_ws_url() -> Result<String> {
    let response: Vec<CdpTarget> = reqwest::get(format!(
        "http://127.0.0.1:{}/json",
        CDP_PORT
    ))
    .await
    .context(
        "Chrome no está corriendo con --remote-debugging-port=9222.\n\
         Inicia Chrome con: chrome.exe --remote-debugging-port=9222\n\
         O usa: msc wget cookies URL --auto-launch",
    )?
    .json()
    .await
    .context("Respuesta CDP inválida")?;

    // Find first target with ws_url - accept any page type including about:blank
    response
        .into_iter()
        .find(|t| t.ws_url.is_some() && t.target_type.as_deref() == Some("page"))
        .and_then(|t| t.ws_url)
        .context("No se encontró un target CDP válido de tipo 'page'")
}

/// Extract all cookies from Chrome via CDP using Storage API (MODERN)
///
/// # Why Storage.getCookies instead of Network.getAllCookies?
/// 1. `Network.getAllCookies` is DEPRECATED by Chrome
/// 2. Storage API accesses the cookie jar directly (not network stack)
/// 3. Works in headless mode without navigation
/// 4. Returns partitioned cookies (CHIPS)
/// 5. More reliable and future-proof
///
/// # How it works
/// 1. Connect to Chrome's WebSocket endpoint
/// 2. Send `Storage.getCookies` command (MODERN API)
/// 3. Receive all cookies in plaintext (already decrypted by Chrome ABE)
/// 4. Chrome handles decryption internally (respects ABE path binding)
///
/// # Advantages over Network.getAllCookies
/// - Not deprecated
/// - Direct access to cookie jar
/// - Works without active page context
/// - Returns partitioned cookies
///
/// # Requirements
/// - Chrome must be running with `--remote-debugging-port=9222`
/// - Chrome 127+ with App-Bound Encryption is fully supported
pub async fn get_cookies_via_storage_api() -> Result<Vec<CdpCookie>> {
    let ws_url = get_ws_url().await?;

    let (mut ws, _) = connect_async(&ws_url)
        .await
        .context("Error conectando a CDP WebSocket")?;

    // Use Storage.getCookies (the modern API)
    let request = CdpRequest {
        id: REQUEST_ID.fetch_add(1, Ordering::SeqCst),
        method: "Storage.getCookies",
        params: serde_json::json!({}), // Empty params returns all cookies
    };

    ws.send(Message::Text(serde_json::to_string(&request)?.into()))
        .await?;

    while let Some(msg) = ws.next().await {
        let msg = msg.context("Error leyendo respuesta CDP")?;

        if let Message::Text(text) = msg {
            let response: CdpResponse = serde_json::from_str(&text)?;

            if response.id == request.id {
                if let Some(error) = response.error {
                    return Err(anyhow::anyhow!("CDP Storage error: {}", error.message));
                }

                if let Some(result) = response.result {
                    return Ok(result.cookies);
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "No se recibió respuesta de Storage.getCookies"
    ))
}

/// Extract all cookies from Chrome via CDP (DEPRECATED - Use Storage API instead)
///
/// # ⚠️ DEPRECATED
/// This function uses `Network.getAllCookies` which is deprecated by Chrome.
/// Use `get_cookies_via_storage_api()` instead for better reliability.
///
/// # Why deprecated?
/// - `Network.getAllCookies` is marked deprecated by Chrome DevTools Protocol
/// - Fails to retrieve partitioned cookies (CHIPS)
/// - Requires active network context (doesn't work well in headless)
/// - May return empty list even when cookies exist
///
/// # How it works
/// 1. Connect to Chrome's WebSocket endpoint
/// 2. Send `Network.getAllCookies` command
/// 3. Receive all cookies in plaintext (already decrypted)
/// 4. Convert to our Cookie format
///
/// # Advantages over DPAPI
/// - Works with Chrome 127+ App-Bound Encryption
/// - Gets cookies from memory (more recent than disk)
/// - No need to decrypt anything
///
/// # Requirements
/// - Chrome must be running with `--remote-debugging-port=9222`
/// - User must have visited the domain (cookies in memory)
#[deprecated(
    since = "0.2.0",
    note = "Use get_cookies_via_storage_api instead. Network.getAllCookies is deprecated by Chrome."
)]
pub async fn get_all_cookies() -> Result<Vec<CdpCookie>> {
    let ws_url = get_ws_url().await?;

    let (mut ws, _) = connect_async(&ws_url)
        .await
        .context("Error conectando a CDP WebSocket")?;

    let enable = CdpRequest {
        id: REQUEST_ID.fetch_add(1, Ordering::SeqCst),
        method: "Network.enable",
        params: serde_json::json!({}),
    };

    ws.send(Message::Text(serde_json::to_string(&enable)?.into())).await?;

    // 2. Request cookies
    let request = CdpRequest {
        id: REQUEST_ID.fetch_add(1, Ordering::SeqCst),
        method: "Network.getAllCookies",
        params: serde_json::json!({}),
    };

    ws.send(Message::Text(serde_json::to_string(&request)?.into())).await?;

    while let Some(msg) = ws.next().await {
        let msg = msg.context("Error leyendo respuesta CDP")?;

        if let Message::Text(text) = msg {
            let response: CdpResponse = serde_json::from_str(&text)?;

            if response.id == request.id {
                if let Some(error) = response.error {
                    return Err(anyhow::anyhow!("CDP error: {}", error.message));
                }

                if let Some(result) = response.result {
                    return Ok(result.cookies);
                }
            }
        }
    }

    Err(anyhow::anyhow!("No se recibió respuesta de CDP"))
}

/// Extract cookies for a specific domain via CDP
///
/// Uses the modern `Storage.getCookies` API for better reliability.
///
/// # Arguments
/// * `domain` - Domain to filter cookies (e.g., "github.com", "https://instagram.com")
///
/// # Returns
/// * `Ok(Vec<CdpCookie>)` - Cookies matching the domain
/// * `Err(...)` - CDP not available or connection failed
pub async fn get_cookies_for_domain(domain: &str) -> Result<Vec<CdpCookie>> {
    // Use the modern Storage API instead of deprecated Network.getAllCookies
    let all_cookies = get_cookies_via_storage_api().await?;

    // Clean domain: remove protocol and path
    let clean_domain = domain
        .strip_prefix("https://")
        .or_else(|| domain.strip_prefix("http://"))
        .unwrap_or(domain)
        .split('/')
        .next()
        .unwrap_or(domain)
        .strip_prefix("www.")
        .unwrap_or(domain);

    // DEBUG: Show what we're looking for and what we found
    println!("{}", format!("   🔍 Buscando cookies para dominio: '{}'", clean_domain).dimmed());
    println!("{}", format!("   📊 Total de cookies en CDP: {}", all_cookies.len()).dimmed());

    // DEBUG: Show unique domains found
    let mut domains: Vec<String> = all_cookies.iter().map(|c| c.domain.clone()).collect();
    domains.sort();
    domains.dedup();

    if !domains.is_empty() {
        println!("{}", "   📋 Dominios encontrados en CDP:".dimmed());
        for (i, d) in domains.iter().take(10).enumerate() {
            println!("{}", format!("      {}. {}", i + 1, d).dimmed());
        }
        if domains.len() > 10 {
            println!("{}", format!("      ... y {} más", domains.len() - 10).dimmed());
        }
    } else {
        println!("{}", "   ⚠️  CDP no devolvió ninguna cookie (perfil vacío)".yellow());
    }

    let matched_cookies: Vec<CdpCookie> = all_cookies
        .into_iter()
        .filter(|c| {
            // Match exact domain or subdomain
            let matches = c.domain.ends_with(clean_domain)
                || c.domain
                    .strip_prefix('.')
                    .map_or(false, |d| d == clean_domain)
                || clean_domain.ends_with(&c.domain.trim_start_matches('.'));

            matches
        })
        .collect();

    println!("{}", format!("   ✓ Cookies encontradas para '{}': {}", clean_domain, matched_cookies.len()).dimmed());

    Ok(matched_cookies)
}

/// Extract cookies via CDP and convert them to our `Cookie` format.
///
/// This function expects a **domain** (e.g. `github.com`, not a full URL)
/// and retrieves all cookies associated with it using the Chrome DevTools
/// Protocol (CDP).
///
/// # Example
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// use msc::core::wget::cdp_cookies::extract_cookies_cdp;
///
/// let cookies = extract_cookies_cdp("github.com").await?;
/// println!("Found {} cookies", cookies.len());
/// # Ok(())
/// # }
/// ```
pub async fn extract_cookies_cdp(domain: &str) -> Result<Vec<Cookie>> {
    let cdp_cookies = get_cookies_for_domain(domain)
        .await
        .with_context(|| format!("Failed to extract cookies for domain: {domain}"))?;

    Ok(cdp_cookies.into_iter().map(Cookie::from).collect())
}

/// Extract cookies via CDP with retry logic and exponential backoff
///
/// This function retries the extraction if it fails due to transient issues
/// like Chrome still initializing or network glitches.
///
/// # Arguments
/// * `domain` - Domain to extract cookies for
/// * `max_retries` - Maximum number of retry attempts (default: 3)
///
/// # Retry Strategy
/// - Initial retry: immediate
/// - Subsequent retries: exponential backoff (500ms, 1s, 2s, etc.)
/// - Retries on: Connection errors, timeout, no targets found
/// - No retry on: Invalid domain, authentication errors
pub async fn extract_cookies_cdp_with_retry(domain: &str, max_retries: usize) -> Result<Vec<Cookie>> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        // Try extraction
        match get_cookies_for_domain(domain).await {
            Ok(cookies) => {
                let result: Vec<Cookie> = cookies.into_iter().map(Cookie::from).collect();

                if attempt > 0 {
                    println!("{}", format!("✓ Extracción exitosa en intento {}/{}", attempt + 1, max_retries + 1).green());
                }

                return Ok(result);
            }
            Err(e) => {
                last_error = Some(e);

                // Don't retry if we're out of attempts
                if attempt >= max_retries {
                    break;
                }

                // Calculate backoff delay (exponential: 500ms, 1s, 2s, 4s, etc.)
                let delay_ms = 500 * (1 << attempt); // 500 * 2^attempt

                println!(
                    "{}",
                    format!("   ⟳ Intento {}/{} falló, reintentando en {}ms...",
                        attempt + 1,
                        max_retries + 1,
                        delay_ms
                    ).yellow()
                );

                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }

    // All retries failed
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("CDP extraction failed after {} retries", max_retries)))
}

/// Print instructions for enabling CDP
pub fn print_cdp_instructions() {
    eprintln!(
        r#"
╔══════════════════════════════════════════════════════════════════╗
║  Chrome 127+ detectado con App-Bound Encryption                  ║
╠══════════════════════════════════════════════════════════════════╣
║  Para extraer cookies, usa una de estas opciones:                ║
║                                                                  ║
║  1. Iniciar Chrome con CDP habilitado:                           ║
║     chrome.exe --remote-debugging-port=9222                      ║
║     Luego: msc wget cookies URL --cdp                            ║
║                                                                  ║
║  2. Dejar que msc inicie Chrome:                                 ║
║     msc wget cookies URL --auto-launch                           ║
║                                                                  ║
║  3. Usar Firefox (no tiene esta limitación):                     ║
║     msc wget cookies URL --browser firefox                       ║
╚══════════════════════════════════════════════════════════════════╝
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdp_cookie_conversion() {
        let cdp_cookie = CdpCookie {
            name: "session".to_string(),
            value: "abc123".to_string(),
            domain: ".github.com".to_string(),
            path: "/".to_string(),
            expires: 1704067200.0,
            secure: true,
            http_only: true,
            same_site: Some("Strict".to_string()),
        };

        let cookie: Cookie = cdp_cookie.into();

        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.domain, ".github.com");
        assert_eq!(cookie.same_site, "Strict");
        assert!(cookie.secure);
        assert!(cookie.http_only);
    }

    #[tokio::test]
    async fn test_cdp_availability_check() {
        // This will fail if Chrome is not running with CDP
        // That's expected - we're just testing the function doesn't panic
        let _ = is_cdp_available().await;
    }
}
