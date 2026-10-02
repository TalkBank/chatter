//! Shared validation-cache setup and access helpers.

use std::sync::Arc;

use talkbank_transform::{
    ReadOnlyCache, RunCache, UnifiedCache, ValidationConfig, ValidationRun, VersionPruneOutcome,
};

use crate::commands::CacheRefreshMode;
use crate::commands::validate_parallel::CachePolicy;

/// A fact about what cache maintenance did, or failed to do.
///
/// Returned, never printed: opening a cache decides nothing about a file
/// descriptor, and each renderer says the event in its own channel (a
/// `cache` record in JSON mode, whose stderr stays empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CacheEvent {
    /// Rows no reader could ever bind again were reclaimed on open.
    Pruned { rows: u64, versions: usize },
    /// `--force` cleared entries for the resolved file list.
    Cleared { entries: usize },
    /// A cache operation failed and the run continues without the cache.
    MaintenanceFailed {
        operation: CacheOperation,
        error: String,
    },
}

/// The cache operation that failed: a closed set, serialized as the JSON
/// record's `operation` value (`initialize`, `clear`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheOperation {
    /// Opening (creating, migrating) the cache.
    Initialize,
    /// `--force` clearing the run's rows.
    Clear,
}

impl CacheOperation {
    /// The operation as it reads in "Failed to ... cache".
    fn phrase(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Clear => "clear",
        }
    }
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
                format!("Warning: Failed to {} cache: {error}", operation.phrase())
            }
        }
    }
}

/// What [`initialize_validation_cache`] did: the run, bound to the cache
/// opened for it, and every fact opening it produced. A cache that would not
/// open is a run without one, with the failure as an event; the run
/// continues without it.
pub(crate) struct CacheInit {
    /// The run's configuration and the cache it uses, and how.
    pub(crate) run: ValidationRun,
    /// What maintenance did (or failed to do) on the way.
    pub(crate) events: Vec<CacheEvent>,
}

/// Open the cache the way `policy` allows, for `config`'s identity (its rule
/// generation and parser namespace; see `ValidationConfig::cache_identity`),
/// and bind it to the run.
///
/// - `ReadWrite`: a validation cache, which prunes unreachable generations
///   as it opens (reported), and with `--force` clears every row of the
///   files the run will validate: the resolved file list, never a label.
/// - `ReadOnly` (an audit): a read-only handle; opening runs no maintenance
///   and nothing can be cleared or written.
pub(crate) fn initialize_validation_cache(
    files: &[talkbank_transform::paths::StoredTranscript],
    policy: CachePolicy,
    config: ValidationConfig,
) -> CacheInit {
    let (cache, mut events) = open_cache(files, policy, &config);
    // The cache was opened for `config`'s own identity, so the binding holds;
    // were it ever refused, the run goes on without the cache and says why.
    let run = match ValidationRun::new(config.clone(), cache) {
        Ok(run) => run,
        Err(mismatch) => {
            events.push(CacheEvent::MaintenanceFailed {
                operation: CacheOperation::Initialize,
                error: mismatch.to_string(),
            });
            ValidationRun::uncached(config)
        }
    };
    CacheInit { run, events }
}

/// The cache `policy` allows for `config`'s identity, and what opening it
/// did.
fn open_cache(
    files: &[talkbank_transform::paths::StoredTranscript],
    policy: CachePolicy,
    config: &ValidationConfig,
) -> (RunCache, Vec<CacheEvent>) {
    let identity = config.cache_identity();
    let unavailable = |error: talkbank_transform::CacheError| {
        (
            RunCache::Absent,
            vec![CacheEvent::MaintenanceFailed {
                operation: CacheOperation::Initialize,
                error: error.to_string(),
            }],
        )
    };
    let refresh = match policy {
        CachePolicy::ReadOnly => {
            return match ReadOnlyCache::open(identity) {
                Ok(cache) => (RunCache::ReadOnly(Arc::new(cache)), Vec::new()),
                Err(error) => unavailable(error),
            };
        }
        CachePolicy::ReadWrite { refresh } => refresh,
    };
    let cache = match UnifiedCache::new(identity) {
        Ok(cache) => Arc::new(cache),
        Err(error) => return unavailable(error),
    };
    let mut events = Vec::new();
    // Opening prunes rows no reader can ever bind again (superseded rule
    // versions). Reported rather than silent: the first prune on a long-lived
    // cache reclaims most of the file, and an operator who is told nothing
    // concludes the cleanup does nothing.
    match cache.version_prune() {
        VersionPruneOutcome::NothingUnreachable | VersionPruneOutcome::FreshDatabase => {}
        VersionPruneOutcome::Pruned(report) => events.push(CacheEvent::Pruned {
            rows: report.rows_deleted(),
            versions: report.versions_deleted(),
        }),
    }

    match refresh {
        CacheRefreshMode::ReuseExisting => {}
        CacheRefreshMode::ForceRefresh => {
            // One batched clear, by each file's resolved location: the
            // identity the worker keys its rows by, made when the file was
            // found, so no spelling of an argument can miss its rows.
            match cache.clear_paths(files.iter().map(|stored| stored.resolved())) {
                Ok(cleared) => events.push(CacheEvent::Cleared { entries: cleared }),
                Err(error) => events.push(CacheEvent::MaintenanceFailed {
                    operation: CacheOperation::Clear,
                    error: error.to_string(),
                }),
            }
        }
    }

    (RunCache::ReadWrite(cache), events)
}
