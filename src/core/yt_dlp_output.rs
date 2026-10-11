//! Lógica pura sobre la salida de yt-dlp: ocultar la URL descargada,
//! clasificar por qué falló y calcular la antigüedad de la versión.
//! Sin I/O para poder testearla con casos fijos.

use chrono::NaiveDate;
use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

/// Marcador que reemplaza la parte privada de una URL.
const HIDDEN: &str = "…";

/// Segmentos de path / valores de query más cortos que esto no se ocultan:
/// suelen ser palabras genéricas ("v", "watch"), no identificadores.
const MIN_SECRET_LEN: usize = 6;

/// yt-dlp avisa a partir de este umbral; usamos el mismo.
pub const STALE_AFTER_DAYS: i64 = 90;

/// URLs del propio yt-dlp (ej. "report this issue on ..."): se dejan visibles.
const PUBLIC_URL_PREFIX: &str = "https://github.com/yt-dlp/";

static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[^\s"'<>()\[\]\x1b]+"#).expect("URL regex válida"));

/// `https://www.example.com/video/123?x=1` → `https://www.example.com/…`
pub fn redact_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(u) => match u.host_str() {
            Some(host) => format!("{}://{}/{}", u.scheme(), host, HIDDEN),
            None => HIDDEN.to_string(),
        },
        Err(_) => HIDDEN.to_string(),
    }
}

/// Oculta la URL objetivo en la salida de yt-dlp: toda URL queda reducida a su
/// dominio, y los identificadores de su path/query (ej. el ID del video que
/// yt-dlp imprime como `[extractor] ID: ...`) se reemplazan por `…`.
pub struct UrlRedactor {
    secrets: Vec<String>,
}

impl UrlRedactor {
    pub fn new(target_url: &str) -> Self {
        let mut secrets: Vec<String> = Vec::new();
        if let Ok(u) = Url::parse(target_url) {
            if let Some(segments) = u.path_segments() {
                secrets.extend(segments.map(String::from));
            }
            secrets.extend(u.query_pairs().map(|(_, v)| v.into_owned()));
            if let Some(fragment) = u.fragment() {
                secrets.push(fragment.to_string());
            }
        }
        secrets.retain(|s| s.chars().count() >= MIN_SECRET_LEN);
        // Más largos primero: si un secreto contiene a otro, se oculta entero.
        secrets.sort_by(|a, b| b.len().cmp(&a.len()));
        secrets.dedup();
        Self { secrets }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for m in URL_RE.find_iter(text) {
            out.push_str(&self.hide_secrets(&text[last..m.start()]));
            if m.as_str().starts_with(PUBLIC_URL_PREFIX) {
                out.push_str(m.as_str());
            } else {
                out.push_str(&redact_url(m.as_str()));
            }
            last = m.end();
        }
        out.push_str(&self.hide_secrets(&text[last..]));
        out
    }

    fn hide_secrets(&self, text: &str) -> String {
        self.secrets
            .iter()
            .fold(text.to_string(), |acc, s| acc.replace(s.as_str(), HIDDEN))
    }
}

/// Causa de una descarga fallida, deducida de las líneas `ERROR:` de yt-dlp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// HTTP 410: el sitio dice que el recurso se eliminó de forma permanente.
    Gone,
    /// HTTP 404: el recurso no existe.
    NotFound,
    /// HTTP 403: el sitio rechazó el acceso.
    Forbidden,
    /// HTTP 429: demasiadas peticiones.
    RateLimited,
    /// Ningún extractor reconoce la URL.
    UnsupportedUrl,
    Unknown,
}

impl FailureKind {
    /// Etiqueta corta para el mensaje de error final
    pub fn label(self) -> &'static str {
        match self {
            Self::Gone => "HTTP 410 Gone",
            Self::NotFound => "HTTP 404 Not Found",
            Self::Forbidden => "HTTP 403 Forbidden",
            Self::RateLimited => "HTTP 429 Too Many Requests",
            Self::UnsupportedUrl => "URL no soportada por yt-dlp",
            Self::Unknown => "error de yt-dlp",
        }
    }

    /// Qué significa, en una línea. `None` si no sabemos más que yt-dlp.
    pub fn meaning(self) -> Option<&'static str> {
        match self {
            Self::Gone => Some("el sitio indica que el contenido se eliminó de forma PERMANENTE."),
            Self::NotFound => Some("el sitio no encuentra esa página."),
            Self::Forbidden => Some("el sitio rechazó el acceso."),
            Self::RateLimited => Some("el sitio te limitó por exceso de peticiones."),
            Self::UnsupportedUrl => Some("ningún extractor de yt-dlp reconoce esta URL."),
            Self::Unknown => None,
        }
    }

    /// Causas probables, de más a menos frecuente, con la acción de cada una.
    pub fn causes(self) -> &'static [&'static str] {
        match self {
            Self::Gone => &[
                "El video fue borrado, dado de baja (DMCA/denuncia) o la cuenta se cerró. \
                 Abrí la página en el navegador: si ahí tampoco está, ninguna actualización lo arregla.",
                "El sitio retiró el endpoint que usa el extractor de yt-dlp. \
                 Si la página SÍ abre en el navegador, actualizar yt-dlp lo resuelve.",
            ],
            Self::NotFound => &[
                "La URL está mal copiada o incompleta.",
                "El video fue eliminado o movido.",
                "El sitio cambió sus rutas y el extractor de yt-dlp quedó desactualizado.",
            ],
            Self::Forbidden => &[
                "El contenido requiere sesión o verificación de edad: probá con --cb <navegador>.",
                "Restricción geográfica: no está disponible en tu país.",
                "El sitio bloquea extractores viejos (muy común en YouTube): actualizá yt-dlp.",
            ],
            Self::RateLimited => &[
                "Esperá unos minutos antes de reintentar.",
                "Las cookies de una sesión iniciada (--cb <navegador>) suelen subir el límite.",
            ],
            Self::UnsupportedUrl => &[
                "La URL no apunta a un video (ej. página de inicio o de búsqueda).",
                "El sitio no está soportado, o se agregó en una versión más nueva de yt-dlp.",
            ],
            Self::Unknown => &[],
        }
    }

    /// Si actualizar yt-dlp puede llegar a resolverlo (un 429 no).
    pub fn update_may_help(self) -> bool {
        !matches!(self, Self::RateLimited)
    }
}

/// Mira el último `ERROR:` (el que cortó la ejecución) y lo clasifica.
pub fn classify_failure<S: AsRef<str>>(stderr_lines: &[S]) -> FailureKind {
    const PATTERNS: &[(&str, FailureKind)] = &[
        ("HTTP Error 410", FailureKind::Gone),
        ("HTTP Error 404", FailureKind::NotFound),
        ("HTTP Error 403", FailureKind::Forbidden),
        ("HTTP Error 429", FailureKind::RateLimited),
        ("Unsupported URL", FailureKind::UnsupportedUrl),
    ];

    stderr_lines
        .iter()
        .rev()
        .map(AsRef::<str>::as_ref)
        .filter(|line| line.contains("ERROR:"))
        .find_map(|line| {
            PATTERNS
                .iter()
                .find(|(pattern, _)| line.contains(pattern))
                .map(|(_, kind)| *kind)
        })
        .unwrap_or(FailureKind::Unknown)
}

/// Días desde la fecha de una versión de yt-dlp (`2026.03.17`, o nightly
/// `2026.03.17.232000`). `None` si no tiene formato de fecha.
pub fn version_age_days(version: &str, today: NaiveDate) -> Option<i64> {
    let mut parts = version.trim().split('.');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    let released = NaiveDate::from_ymd_opt(year, month, day)?;
    Some((today - released).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = "https://www.example.com/watch/abc123XYZ?list=PLsecret99&t=10";

    #[test]
    fn redact_url_keeps_only_scheme_and_host() {
        assert_eq!(redact_url(TARGET), "https://www.example.com/…");
        assert_eq!(redact_url("http://example.com"), "http://example.com/…");
    }

    #[test]
    fn redact_url_hides_everything_when_unparseable() {
        assert_eq!(redact_url("not a url"), "…");
    }

    #[test]
    fn redacts_extracting_url_line() {
        let r = UrlRedactor::new(TARGET);
        assert_eq!(
            r.redact(&format!("[generic] Extracting URL: {}\n", TARGET)),
            "[generic] Extracting URL: https://www.example.com/…\n"
        );
    }

    #[test]
    fn redacts_video_id_printed_without_url() {
        let r = UrlRedactor::new(TARGET);
        assert_eq!(
            r.redact("[example] abc123XYZ: Downloading pc webpage"),
            "[example] …: Downloading pc webpage"
        );
        assert_eq!(
            r.redact("ERROR: [example] abc123XYZ: Unable to download webpage: HTTP Error 410: Gone"),
            "ERROR: [example] …: Unable to download webpage: HTTP Error 410: Gone"
        );
    }

    #[test]
    fn redacts_query_values_but_not_short_generic_words() {
        let r = UrlRedactor::new(TARGET);
        // "watch" (5) y "10" son genéricos; "PLsecret99" es un identificador.
        assert_eq!(
            r.redact("watch playlist PLsecret99 at 10"),
            "watch playlist … at 10"
        );
    }

    #[test]
    fn redacts_other_urls_like_redirects_and_cdns() {
        let r = UrlRedactor::new(TARGET);
        assert_eq!(
            r.redact("Redirecting to https://cdn.example.net/v/xyz?token=abc"),
            "Redirecting to https://cdn.example.net/…"
        );
    }

    #[test]
    fn keeps_yt_dlp_own_urls_visible() {
        let r = UrlRedactor::new(TARGET);
        let line = "please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= ";
        assert_eq!(r.redact(line), line);
    }

    #[test]
    fn url_match_stops_at_closing_paren_and_ansi() {
        let r = UrlRedactor::new(TARGET);
        assert_eq!(
            r.redact("(see https://www.example.com/a/b)"),
            "(see https://www.example.com/…)"
        );
        assert_eq!(
            r.redact("\x1b[0;31mhttps://www.example.com/x\x1b[0m"),
            "\x1b[0;31mhttps://www.example.com/…\x1b[0m"
        );
    }

    #[test]
    fn leaves_progress_lines_untouched() {
        let r = UrlRedactor::new(TARGET);
        let line = "\r[download]  45.3% of ~ 12.34MiB at 1.20MiB/s ETA 00:10";
        assert_eq!(r.redact(line), line);
    }

    #[test]
    fn classify_410_from_real_yt_dlp_output() {
        let lines = [
            "WARNING: Your yt-dlp version (2026.03.17) is older than 90 days!",
            "ERROR: [example] abc: Unable to download webpage: HTTP Error 410: Gone (caused by <HTTPError 410: Gone>)",
        ];
        assert_eq!(classify_failure(&lines), FailureKind::Gone);
    }

    #[test]
    fn classify_known_http_codes_and_unsupported_url() {
        let cases = [
            ("ERROR: x: HTTP Error 404: Not Found", FailureKind::NotFound),
            ("ERROR: x: HTTP Error 403: Forbidden", FailureKind::Forbidden),
            ("ERROR: x: HTTP Error 429: Too Many Requests", FailureKind::RateLimited),
            ("ERROR: Unsupported URL: https://x.com/…", FailureKind::UnsupportedUrl),
            ("ERROR: something else broke", FailureKind::Unknown),
        ];
        for (line, expected) in cases {
            assert_eq!(classify_failure(&[line]), expected, "{line}");
        }
    }

    #[test]
    fn classify_ignores_non_error_lines_and_uses_last_error() {
        let lines = [
            "WARNING: [x] HTTP Error 403 on a fragment, retrying",
            "ERROR: first: HTTP Error 404: Not Found",
            "ERROR: last: HTTP Error 410: Gone",
        ];
        assert_eq!(classify_failure(&lines), FailureKind::Gone);
        assert_eq!(classify_failure::<&str>(&[]), FailureKind::Unknown);
    }

    #[test]
    fn every_known_failure_explains_itself() {
        let known = [
            FailureKind::Gone,
            FailureKind::NotFound,
            FailureKind::Forbidden,
            FailureKind::RateLimited,
            FailureKind::UnsupportedUrl,
        ];
        for kind in known {
            assert!(kind.meaning().is_some(), "{kind:?} sin significado");
            assert!(!kind.causes().is_empty(), "{kind:?} sin causas");
        }
        assert_eq!(FailureKind::Unknown.meaning(), None);
        assert!(FailureKind::Unknown.causes().is_empty());
    }

    #[test]
    fn gone_lists_removed_content_before_outdated_extractor() {
        // El orden importa: el 410 casi siempre es contenido borrado, y
        // sugerir primero "actualizá" manda a una pista falsa.
        let causes = FailureKind::Gone.causes();
        assert!(causes[0].contains("borrado"));
        assert!(causes[1].contains("actualizar yt-dlp"));
    }

    #[test]
    fn update_does_not_help_rate_limits() {
        assert!(!FailureKind::RateLimited.update_may_help());
        assert!(FailureKind::Gone.update_may_help());
        assert!(FailureKind::Unknown.update_may_help());
    }

    #[test]
    fn version_age_from_stable_and_nightly_versions() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        assert_eq!(version_age_days("2026.03.17\n", today), Some(206));
        assert_eq!(version_age_days("2026.10.01.232000", today), Some(8));
    }

    #[test]
    fn version_age_none_for_non_date_versions() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        assert_eq!(version_age_days("unknown", today), None);
        assert_eq!(version_age_days("2026.13.40", today), None);
    }
}
