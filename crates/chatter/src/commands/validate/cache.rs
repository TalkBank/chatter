//! Shared validation-cache setup and access helpers.

use std::path::Path;
use std::sync::Arc;

use talkbank_transform::{CacheIdentity, UnifiedCache, VersionPruneOutcome};

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
/// The caller supplies the identity derived from its actual validation config:
/// rule/build generation and parser namespace, excluding presentation policy.
/// The parser belongs in the row key, not the retained-version window. One
/// binary's default/strict rules and two parser choices therefore retain both
/// rule generations without a cold rotation. See `ValidationConfig::cache_identity`.
///
/// A fact about what cache maintenance did.
///
/// # Why these are returned rather than printed
///
/// They used to go straight to stderr from inside `initialize_validation_cache`,
/// which broke `--format json`'s contract that stdout is the stream and stderr
/// is empty. The first fix passed that function a two-variant `CacheNotices`
/// telling it where to write, which made today's behaviour right and left the
/// wrong pairing representable: nothing but a test would notice a caller
/// handing it the terminal variant during a JSON run.
///
/// The defect was not WHO decided the rendering. It was that a function which
/// opens a database decided anything about a file descriptor at all. This
/// workspace already has the rule, from the `ensure_directory_for_user`
/// incident: on any seam that mutates state, the method returns what it DID.
/// A prune that reclaimed rows and a clear that removed entries are results.
///
/// So the capability is gone rather than redirected. The function cannot write
/// anywhere, the events are values a caller can hold, test, or render late,
/// and "passed the wrong output mode" stopped being a state that exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CacheEvent {
    /// Rows no reader could ever bind again were reclaimed on open.
    Pruned { rows: u64, versions: u64 },
    /// `--force` cleared entries for the resolved file list.
    Cleared { entries: usize },
    /// Maintenance failed and the run continues without it.
    MaintenanceFailed {
        operation: &'static str,
        error: String,
    },
}

impl CacheEvent {
    /// One sentence for a terminal.
    pub(crate) fn sentence(&self) -> String {
        match self {
            Self::Pruned { rows, versions } => {
                format!(
                    "note: pruned {rows} unreachable cache row(s) from {versions} superseded version(s)"
                )
            }
            Self::Cleared { entries } => format!("Cleared {entries} cache entries"),
            Self::MaintenanceFailed { operation, error } => {
                format!("Warning: Failed to {operation} cache: {error}")
            }
        }
    }

    /// One record for a machine-readable stream.
    pub(crate) fn record(&self) -> serde_json::Value {
        match self {
            Self::Pruned { rows, versions } => serde_json::json!({
                "type": "cache",
                "action": "prune",
                "rows_deleted": rows,
                "versions_deleted": versions,
            }),
            Self::Cleared { entries } => serde_json::json!({
                "type": "cache",
                "action": "clear",
                "entries_cleared": entries,
            }),
            Self::MaintenanceFailed { operation, error } => serde_json::json!({
                "type": "cache",
                "action": "warning",
                "operation": operation,
                "error": error,
            }),
        }
    }
}

/// What [`initialize_validation_cache`] did.
///
/// Successful initialization carries its handle and maintenance events. Failure
/// carries the opening error directly, with no callback or missing-error state.
pub(crate) enum CacheInit {
    /// The cache opened. Events describe maintenance done on the way.
    Opened {
        handle: ValidationCacheHandle,
        events: Vec<CacheEvent>,
    },
    /// The cache did not open, and this says why. The run continues without it.
    Unavailable {
        error: talkbank_transform::CacheError,
    },
}

impl CacheInit {
    /// The handle, if any, and every fact this initialisation produced.
    ///
    /// One accessor rather than a pair, because both callers want both: the
    /// handle to validate with, and the events to render once they have a
    /// presenter. Separate `handle(self)` and `events(&self)` methods made the
    /// obvious call order a borrow error, which is a type telling its callers
    /// to do something awkward for no reason.
    pub(crate) fn into_parts(self) -> (Option<ValidationCacheHandle>, Vec<CacheEvent>) {
        match self {
            Self::Opened { handle, events } => (Some(handle), events),
            Self::Unavailable { error } => (
                None,
                vec![CacheEvent::MaintenanceFailed {
                    operation: "initialize",
                    error: error.to_string(),
                }],
            ),
        }
    }
}

pub(crate) fn initialize_validation_cache(
    files: &[std::path::PathBuf],
    cache_refresh: CacheRefreshMode,
    identity: CacheIdentity,
) -> CacheInit {
    let cache = match UnifiedCache::new(identity) {
        Ok(cache) => Arc::new(cache),
        Err(error) => return CacheInit::Unavailable { error },
    };
    let mut events = Vec::new();

    // Opening prunes rows no reader can ever bind again (superseded rule
    // versions). Reported rather than silent: the first prune on a long-lived
    // cache reclaims most of the file, and an operator who is told nothing
    // concludes the cleanup does nothing.
    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable => {}
        VersionPruneOutcome::Pruned(report) => events.push(CacheEvent::Pruned {
            rows: report.rows_deleted(),
            versions: report.versions_deleted(),
        }),
    }

    if cache_refresh.should_clear_cache() {
        // One batched clear over the resolved file list. Never loop
        // per-file clears here: each `clear_prefix` call was a full-table
        // scan, so the loop this replaces was quadratic in corpus size and
        // pinned `validate --force <corpus-root>` at 100% CPU behind a blank
        // screen (v0.5.0 DOA, 2026-07-30; regression test
        // `force_refresh_scales_to_corpus_sized_input`).
        match cache.clear_paths(files) {
            Ok(cleared) => events.push(CacheEvent::Cleared { entries: cleared }),
            Err(error) => events.push(CacheEvent::MaintenanceFailed {
                operation: "clear",
                error: error.to_string(),
            }),
        }
    }

    CacheInit::Opened {
        handle: cache,
        events,
    }
}

/// Return one cached validation result when available.
pub(crate) fn get_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
) -> Option<bool> {
    cache.and_then(|cache| cache.get_validation(path, check_alignment))
}

/// Store one validation result, returning the failure if there was one.
///
/// The last unconditional `eprintln!` on this seam, and the same defect as the
/// three above it: a cache write deciding what goes on a file descriptor, which
/// in JSON mode is a contract violation once per failing file rather than once
/// per run.
///
/// Success returns `None` rather than an event, deliberately. The rule that a
/// mutating seam reports what it DID is about effects a caller can act on or
/// verify, and a record per cached file would be noise in a 106,000-file run
/// with nothing to do about it. A FAILURE is actionable, so it comes back.
#[must_use]
pub(crate) fn set_cached_validation(
    cache: Option<&ValidationCacheHandle>,
    path: &Path,
    check_alignment: bool,
    valid: bool,
) -> Option<CacheEvent> {
    let cache = cache?;
    match cache.set_validation(path, check_alignment, valid) {
        Ok(()) => None,
        Err(error) => Some(CacheEvent::MaintenanceFailed {
            operation: "write validation results to",
            error: error.to_string(),
        }),
    }
}
