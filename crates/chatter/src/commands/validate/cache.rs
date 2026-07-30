//! Shared validation-cache setup and access helpers.

use std::path::Path;
use std::sync::Arc;

use talkbank_transform::UnifiedCache;

use crate::commands::CacheRefreshMode;

/// Shared cache handle used by validation entrypoints.
pub(crate) type ValidationCacheHandle = Arc<UnifiedCache>;

/// Create the validation cache and apply `--force` clearing for EVERY file
/// the run will validate.
///
/// The clear must key on the resolved file list, never on a display label:
/// until 2026-07-30 it cleared only the cosmetic `summary_label` (the first
/// input argument), so `chatter validate --force a.cha b.cha` silently served
/// b.cha's stale verdict. That is invisible in normal use (content changes
/// rotate the key) but poisonous when the BINARY changes rule behavior
/// without changing the rule list: a 994-file measurement served 993 stale
/// verdicts from the previous build and reported near-zero impact.
pub(crate) fn initialize_validation_cache(
    files: &[std::path::PathBuf],
    cache_refresh: CacheRefreshMode,
) -> Option<ValidationCacheHandle> {
    let cache = UnifiedCache::open_or_else(|error| {
        eprintln!("Warning: Failed to initialize cache: {}", error);
    })?;

    if cache_refresh.should_clear_cache() {
        let mut cleared = 0usize;
        let mut failed = false;
        for path in files {
            match cache.clear_prefix(&path.to_string_lossy()) {
                Ok(count) => cleared += count,
                Err(error) => {
                    eprintln!("Warning: Failed to clear cache: {}", error);
                    failed = true;
                }
            }
        }
        if !failed {
            eprintln!("Cleared {} cache entries", cleared);
        }
    }

    Some(cache)
}

/// Return one cached validation result when available.
pub(crate) fn get_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
) -> Option<bool> {
    cache.and_then(|cache| cache.get_validation(path, check_alignment))
}

/// Store one validation result, warning on cache-write failures.
pub(crate) fn set_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
    valid: bool,
) {
    if let Some(cache) = cache
        && let Err(error) = cache.set_validation(path, check_alignment, valid)
    {
        eprintln!("Warning: Failed to cache validation results: {}", error);
    }
}
