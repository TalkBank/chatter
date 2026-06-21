//! Utility functions for cache operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::CacheError;

/// Get unique cache key for a file path with a suffix.
pub fn get_cache_key_with_suffix(path: &Path, suffix: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    suffix.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Compute blake3 content hash of a file, returned as a hex string.
pub fn get_content_hash(path: &Path) -> Result<String, CacheError> {
    let data = std::fs::read(path).map_err(|source| CacheError::Metadata {
        path: path.display().to_string(),
        source,
    })?;
    Ok(blake3::hash(&data).to_hex().to_string())
}

/// Get current time in seconds since epoch.
pub fn now_secs() -> Result<u64, CacheError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|e| CacheError::Message(format!("system time before Unix epoch: {e}")))
}

/// Environment variable that relocates the cache root.
///
/// When set (to an absolute or relative directory path), the cache
/// database lives directly in that directory instead of under the
/// platform cache root. This is the supported way to redirect cache
/// state, and the only reliable isolation mechanism on Windows: the
/// platform default there resolves through the Known Folder API, which
/// ignores `HOME`-style environment variables entirely.
pub const CACHE_DIR_ENV: &str = "TALKBANK_CHAT_CACHE_DIR";

/// Get the default cache directory.
///
/// Resolution order: [`CACHE_DIR_ENV`] if set and non-empty (used
/// verbatim, no `talkbank-chat` suffix appended); otherwise the
/// platform cache root (`~/Library/Caches` on macOS, `XDG_CACHE_HOME`
/// or `~/.cache` on Linux, `%LocalAppData%` on Windows) plus
/// `talkbank-chat`.
pub fn default_cache_dir() -> Result<std::path::PathBuf, CacheError> {
    if let Some(dir) = std::env::var_os(CACHE_DIR_ENV)
        && !dir.is_empty()
    {
        return Ok(std::path::PathBuf::from(dir));
    }
    dirs::cache_dir()
        .map(|d| d.join("talkbank-chat"))
        .ok_or(CacheError::CacheDirMissing)
}
