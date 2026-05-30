use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-process toggle. When false, `get` always returns None and `set` is a no-op.
/// Default: enabled. Set via `cache::set_enabled(false)` from CLI when `--no-cache` is on.
static CACHE_ENABLED: AtomicBool = AtomicBool::new(true);

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    value: serde_json::Value,
    saved_at_secs: u64,
    ttl_secs: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub fn set_enabled(enabled: bool) {
    CACHE_ENABLED.store(enabled, Ordering::Relaxed);
}

fn is_enabled() -> bool {
    CACHE_ENABLED.load(Ordering::Relaxed)
}

fn cache_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("msc");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("sys_cache.json"))
}

fn load_file() -> CacheFile {
    let path = match cache_path() {
        Some(p) => p,
        None => return CacheFile::default(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return CacheFile::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_file(file: &CacheFile) {
    let path = match cache_path() {
        Some(p) => p,
        None => return,
    };
    if let Ok(json) = serde_json::to_string_pretty(file) {
        let _ = std::fs::write(&path, json);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Try to read a cached value for `key`. Returns None if cache is disabled,
/// the entry is missing, expired, or fails to deserialize.
pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    if !is_enabled() {
        return None;
    }
    let file = load_file();
    let entry = file.entries.get(key)?;
    let age = now_secs().saturating_sub(entry.saved_at_secs);
    if age > entry.ttl_secs {
        return None;
    }
    serde_json::from_value(entry.value.clone()).ok()
}

/// Store `value` under `key` with a TTL in days. No-op if cache is disabled.
pub fn set<T: Serialize>(key: &str, value: &T, ttl_days: u64) {
    if !is_enabled() {
        return;
    }
    let json_value = match serde_json::to_value(value) {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut file = load_file();
    file.entries.insert(
        key.to_string(),
        CacheEntry {
            value: json_value,
            saved_at_secs: now_secs(),
            ttl_secs: ttl_days * 24 * 60 * 60,
        },
    );
    save_file(&file);
}

/// Wipe all cached entries. Used for `--clear-cache`.
pub fn clear_all() {
    let path = match cache_path() {
        Some(p) => p,
        None => return,
    };
    let _ = std::fs::remove_file(path);
}
