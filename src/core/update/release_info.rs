use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

impl ReleaseInfo {
    /// Extrae la versión del tag_name (ej: "v0.1.10" -> "0.1.10")
    pub fn version(&self) -> String {
        self.tag_name.trim_start_matches('v').to_string()
    }

    /// Obtiene el changelog (body) de la release
    pub fn changelog(&self) -> &str {
        &self.body
    }
}

/// Obtiene información sobre la última release desde GitHub API
pub fn fetch_latest_release(repo: &str) -> Result<ReleaseInfo> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let response = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "MSC-CLI")
        .send()
        .context("Failed to fetch latest release from GitHub")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "GitHub API returned status {}: {}",
            response.status(),
            response.text().unwrap_or_default()
        ));
    }

    let release: ReleaseInfo = response
        .json()
        .context("Failed to parse GitHub release JSON")?;

    Ok(release)
}

/// Determina el nombre del archivo binario según la plataforma actual
pub fn get_platform_binary_name() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "msc-x86_64-pc-windows-msvc.msi";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "msc-x86_64-unknown-linux-gnu.tar.xz";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "msc-aarch64-unknown-linux-gnu.tar.xz";

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "msc-x86_64-apple-darwin.tar.xz";

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "msc-aarch64-apple-darwin.tar.xz";
}

/// Obtiene los assets del binario y checksum para la plataforma actual
pub fn get_platform_assets(release: &ReleaseInfo) -> Result<(&ReleaseAsset, &ReleaseAsset)> {
    let binary_name = get_platform_binary_name();
    let checksum_name = format!("{}.sha256", binary_name);

    // Buscar el asset del binario
    let binary_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == binary_name)
        .ok_or_else(|| anyhow!("Binary asset '{}' not found in release", binary_name))?;

    // Buscar el asset del checksum
    let checksum_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .ok_or_else(|| anyhow!("Checksum asset '{}' not found in release", checksum_name))?;

    Ok((binary_asset, checksum_asset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_extraction() {
        let release = ReleaseInfo {
            tag_name: "v0.1.10".to_string(),
            name: "Release 0.1.10".to_string(),
            body: "Changelog".to_string(),
            assets: vec![],
        };

        assert_eq!(release.version(), "0.1.10");
    }

    #[test]
    fn test_platform_binary_name() {
        let name = get_platform_binary_name();
        assert!(!name.is_empty());
        #[cfg(windows)]
        assert!(name.ends_with(".msi"));
        #[cfg(unix)]
        assert!(name.ends_with(".tar.xz"));
    }
}
