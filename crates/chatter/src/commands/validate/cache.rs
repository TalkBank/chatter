//! Shared validation-cache setup and access helpers.

use std::path::Path;
use std::sync::Arc;

use talkbank_transform::{GRAMMAR_FINGERPRINT, RulesVersion, UnifiedCache, VersionPruneOutcome};

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
///
/// `rules` is the SAME [`talkbank_model::RuleSelection`] the run will hand to
/// the worker for actual validation. Threading the identical value through
/// here, rather than re-deriving a summary of it, is what keeps the cache key
/// and the validation behaviour from ever disagreeing: it is folded into the
/// pool's [`RulesVersion`] via [`RulesVersion::current_with_rule_selection`],
/// so a verdict produced under one rule set is never served to a run with a
/// different one.
///
/// What is deliberately ABSENT is the run's presentation policy (`--suppress`
/// and severity remapping). v0.6.0 folded it in, which gave every distinct
/// suppression set its own private cache and re-validated a 106,000-file corpus
/// from cold on the second run. It is not merely omitted here: this function
/// cannot be handed one, because `RulesVersion::current_with_rule_selection`
/// does not accept one and `talkbank-cache` cannot name the type.
///
/// The PARSE dimension is folded in too: `RulesVersion::current_with_config`
/// takes `talkbank_transform::GRAMMAR_FINGERPRINT` (re-exported from the
/// grammar crate via `talkbank-parser`) as a mandatory parameter, so a
/// verdict produced under one compiled-in grammar is never served back to a
/// binary built against a different one. This CLI already depends on both
/// the parser and the cache, which is exactly the seam
/// `RulesVersion::current_with_config`'s doc comment names as the intended
/// place to close that gap.
pub(crate) fn initialize_validation_cache(
    files: &[std::path::PathBuf],
    cache_refresh: CacheRefreshMode,
    rules: &talkbank_model::RuleSelection,
) -> Option<ValidationCacheHandle> {
    let rules_version = RulesVersion::current_with_rule_selection(rules, GRAMMAR_FINGERPRINT);
    let cache = UnifiedCache::open_or_else_with_rules_version(rules_version, |error| {
        eprintln!("Warning: Failed to initialize cache: {}", error);
    })?;

    // Opening prunes rows no reader can ever bind again (superseded rule
    // versions). Reported rather than silent: the first prune on a long-lived
    // cache reclaims most of the file, and an operator who is told nothing
    // concludes the cleanup does nothing.
    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable => {}
        VersionPruneOutcome::Pruned(report) => eprintln!("note: {report}"),
    }

    if cache_refresh.should_clear_cache() {
        // One batched clear over the resolved file list. Never loop
        // per-file clears here: each `clear_prefix` call was a full-table
        // scan, so the loop this replaces was quadratic in corpus size and
        // pinned `validate --force <corpus-root>` at 100% CPU behind a blank
        // screen (v0.5.0 DOA, 2026-07-30; regression test
        // `force_refresh_scales_to_corpus_sized_input`).
        match cache.clear_paths(files) {
            Ok(cleared) => eprintln!("Cleared {} cache entries", cleared),
            Err(error) => eprintln!("Warning: Failed to clear cache: {}", error),
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
