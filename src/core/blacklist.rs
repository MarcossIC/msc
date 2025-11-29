use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use url::Url;

/// Blacklist manager for blocking malicious/unwanted domains
pub struct Blacklist {
    domains: HashSet<String>,
}

impl Blacklist {
    /// Create a new empty blacklist
    pub fn new() -> Self {
        Self {
            domains: HashSet::new(),
        }
    }

    /// Load blacklist from a hosts file format
    /// Format: "127.0.0.1 domain.com" or "0.0.0.0 domain.com"
    /// One domain per line
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let mut blacklist = Self::new();

        if !path.exists() {
            // File doesn't exist, return empty blacklist
            return Ok(blacklist);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read blacklist file: {}", path.display()))?;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Parse hosts file format: "127.0.0.1 domain.com" or "0.0.0.0 domain.com"
            let parts: Vec<&str> = trimmed.split_whitespace().collect();

            if parts.len() >= 2 {
                // Second part is the domain
                let domain = parts[1].to_lowercase();

                // Skip localhost entries
                if domain == "localhost" || domain == "localhost.localdomain" {
                    continue;
                }

                blacklist.domains.insert(domain);
            }
        }

        Ok(blacklist)
    }

    /// Check if a URL's domain is blacklisted
    pub fn is_blocked(&self, url: &str) -> bool {
        // Try to parse the URL
        let parsed_url = match Url::parse(url) {
            Ok(u) => u,
            Err(_) => return false, // Invalid URL, don't block
        };

        // Get the domain from the URL
        let domain = match parsed_url.domain() {
            Some(d) => d.to_lowercase(),
            None => return false, // No domain, don't block
        };

        // Check if the exact domain is blacklisted
        if self.domains.contains(&domain) {
            return true;
        }

        // Check if any parent domain is blacklisted
        // For example, if "example.com" is blacklisted, block "subdomain.example.com"
        let domain_parts: Vec<&str> = domain.split('.').collect();

        // Try progressively shorter domain suffixes
        for i in 0..domain_parts.len() {
            let suffix = domain_parts[i..].join(".");
            if self.domains.contains(&suffix) {
                return true;
            }
        }

        false
    }

    /// Get the number of domains in the blacklist
    pub fn len(&self) -> usize {
        self.domains.len()
    }

    /// Check if the blacklist is empty
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blocked_exact_match() {
        let mut blacklist = Blacklist::new();
        blacklist.domains.insert("malicious.com".to_string());

        assert!(blacklist.is_blocked("https://malicious.com/page"));
        assert!(!blacklist.is_blocked("https://safe.com/page"));
    }

    #[test]
    fn test_is_blocked_subdomain() {
        let mut blacklist = Blacklist::new();
        blacklist.domains.insert("malicious.com".to_string());

        assert!(blacklist.is_blocked("https://subdomain.malicious.com/page"));
        assert!(blacklist.is_blocked("https://a.b.malicious.com/page"));
    }

    #[test]
    fn test_is_blocked_case_insensitive() {
        let mut blacklist = Blacklist::new();
        blacklist.domains.insert("malicious.com".to_string());

        assert!(blacklist.is_blocked("https://MALICIOUS.COM/page"));
        assert!(blacklist.is_blocked("https://Malicious.Com/page"));
    }

    #[test]
    fn test_parse_hosts_format() {
        let content = r#"
# This is a comment
127.0.0.1 localhost
127.0.0.1 malicious1.com
0.0.0.0 malicious2.com

127.0.0.1 malicious3.org
"#;

        let mut domains = HashSet::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let domain = parts[1].to_lowercase();
                if domain != "localhost" && domain != "localhost.localdomain" {
                    domains.insert(domain);
                }
            }
        }

        assert_eq!(domains.len(), 3);
        assert!(domains.contains("malicious1.com"));
        assert!(domains.contains("malicious2.com"));
        assert!(domains.contains("malicious3.org"));
        assert!(!domains.contains("localhost"));
    }
}
