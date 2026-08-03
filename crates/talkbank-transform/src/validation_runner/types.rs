//! Validation event types and status enums
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use talkbank_model::ParseError;
// ParseError used in ErrorEvent below

/// File validation status (without error details - those stream separately)
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// File passed validation (and roundtrip, if enabled).
    Valid {
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
    /// File has validation errors.
    Invalid {
        /// Number of errors with `Severity::Error`.
        error_count: usize,
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
    /// Validation passed but roundtrip failed.
    RoundtripFailed {
        /// Whether the result came from the cache.
        cache_hit: bool,
        /// Human-readable description of the failure.
        reason: String,
    },
    /// File could not be parsed at all.
    ParseError {
        /// Human-readable parse error message.
        message: String,
    },
    /// File could not be read from disk.
    ReadError {
        /// Human-readable I/O error message.
        message: String,
    },
}

/// Validation statistics with atomic counters (lock-free)
#[derive(Debug)]
pub struct ValidationStats {
    /// Total number of `.cha` files to validate (set once at start).
    pub total_files: usize,
    valid_files: AtomicUsize,
    invalid_files: AtomicUsize,
    cache_hits: AtomicUsize,
    cache_misses: AtomicUsize,
    parse_errors: AtomicUsize,
    roundtrip_passed: AtomicUsize,
    roundtrip_failed: AtomicUsize,
    cancelled: AtomicBool,
}

impl ValidationStats {
    /// Create new stats for a validation run over the given number of files.
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            valid_files: AtomicUsize::new(0),
            invalid_files: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
            parse_errors: AtomicUsize::new(0),
            roundtrip_passed: AtomicUsize::new(0),
            roundtrip_failed: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Record that a file passed validation.
    pub fn record_valid_file(&self) {
        self.valid_files.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file failed validation.
    pub fn record_invalid_file(&self) {
        self.invalid_files.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file could not be parsed.
    pub fn record_parse_error(&self) {
        self.parse_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a result was served from cache.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file was not in cache and required parsing.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file passed the roundtrip test.
    pub fn record_roundtrip_passed(&self) {
        self.roundtrip_passed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file failed the roundtrip test.
    pub fn record_roundtrip_failed(&self) {
        self.roundtrip_failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark the validation run as cancelled by the user.
    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Get current stats snapshot (for reporting)
    pub fn snapshot(&self) -> ValidationStatsSnapshot {
        ValidationStatsSnapshot {
            total_files: self.total_files,
            valid_files: self.valid_files.load(Ordering::Relaxed),
            invalid_files: self.invalid_files.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
            roundtrip_passed: self.roundtrip_passed.load(Ordering::Relaxed),
            roundtrip_failed: self.roundtrip_failed.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
        }
    }

    /// Cache hit rate as a percentage (0.0--100.0).
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files > 0 {
            self.cache_hits.load(Ordering::Relaxed) as f64 / self.total_files as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Snapshot of validation stats at a point in time (Clone + Send)
#[derive(Debug, Clone)]
pub struct ValidationStatsSnapshot {
    /// Total number of `.cha` files discovered.
    pub total_files: usize,
    /// Files that passed validation.
    pub valid_files: usize,
    /// Files that failed validation.
    pub invalid_files: usize,
    /// Files whose results were served from cache.
    pub cache_hits: usize,
    /// Files that required fresh parsing.
    pub cache_misses: usize,
    /// Files that could not be parsed at all.
    pub parse_errors: usize,
    /// Files that passed the roundtrip test.
    pub roundtrip_passed: usize,
    /// Files that failed the roundtrip test.
    pub roundtrip_failed: usize,
    /// Whether the run was cancelled before completion.
    pub cancelled: bool,
}

/// How much of what a run discovered it actually accounted for.
///
/// Derived from a [`ValidationStatsSnapshot`], never counted alongside it: a
/// third counter tracking "files lost" could drift from the two it is supposed
/// to reconcile, whereas a subtraction cannot. See
/// [`ValidationStatsSnapshot::coverage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCoverage {
    /// Every discovered file produced a per-file result. The snapshot's totals
    /// describe the whole of what was asked for.
    Complete,
    /// The user cancelled, so the run legitimately stopped short. NOT a fault
    /// and NOT incompleteness: the shortfall was requested.
    Cancelled {
        /// Files discovered but never reached because of the cancellation.
        unprocessed_files: usize,
    },
    /// Files were discovered, the run was not cancelled, and they never
    /// produced a result: a worker abandoned them. The snapshot's totals
    /// describe ONLY what was processed, so no claim about "all files" can be
    /// made from them.
    Lost {
        /// Files discovered but never accounted for.
        lost_files: usize,
    },
}

impl ValidationStatsSnapshot {
    /// Cache hit rate as a percentage (0.0--100.0).
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files > 0 {
            self.cache_hits as f64 / self.total_files as f64 * 100.0
        } else {
            0.0
        }
    }

    /// Files that produced a per-file result of any kind.
    ///
    /// Exactly one of these three counters is incremented per completed file
    /// (`update_stats` in `worker.rs` folds roundtrip failures and read errors
    /// into `invalid_files`), so this is a count of files, not of events.
    pub fn files_accounted_for(&self) -> usize {
        self.valid_files
            .saturating_add(self.invalid_files)
            .saturating_add(self.parse_errors)
    }

    /// Reconcile what was discovered against what was actually processed.
    ///
    /// THE reason this exists: a worker thread that unwinds abandons whatever
    /// files it had taken off the queue, and the run then reports totals that
    /// look perfectly clean because the missing files contributed nothing to
    /// any counter. A 500-file corpus could validate 480 and report "all
    /// valid". This is the single place that difference is computed, so no
    /// consumer has to know to look for it.
    pub fn coverage(&self) -> RunCoverage {
        let missing = self.total_files.saturating_sub(self.files_accounted_for());
        match (missing, self.cancelled) {
            (0, _) => RunCoverage::Complete,
            (unprocessed_files, true) => RunCoverage::Cancelled { unprocessed_files },
            (lost_files, false) => RunCoverage::Lost { lost_files },
        }
    }
}

/// Message sent when errors are discovered
#[derive(Debug, Clone)]
pub struct ErrorEvent {
    /// Path to the file that produced errors.
    pub path: PathBuf,
    /// Parse and/or validation errors for the file.
    pub errors: Vec<ParseError>,
    /// Original source text (needed for miette rendering).
    pub source: Arc<str>,
}

/// Message sent when file completes
#[derive(Debug, Clone)]
pub struct FileCompleteEvent {
    /// Path to the completed file.
    pub path: PathBuf,
    /// Validation result for the file.
    pub status: FileStatus,
}

/// Message sent when roundtrip test completes
#[derive(Debug, Clone)]
pub struct RoundtripEvent {
    /// Path to the file that was roundtrip-tested.
    pub path: PathBuf,
    /// Whether serialization was idempotent.
    pub passed: bool,
    /// Failure reason (e.g. "Roundtrip mismatch (serialization not idempotent)").
    pub failure_reason: Option<String>,
    /// First few differing lines between pass-1 and pass-2 output.
    pub diff: Option<String>,
}

/// Why a validation run stopped without finishing.
///
/// Carried by [`ValidationEvent::Aborted`] so a consumer can tell the user
/// something specific rather than "it just stopped". Deliberately a closed
/// enum rather than a string: an abort reason is a fact the runner knows, and
/// consumers (a CLI exit path, a TUI banner, a desktop dialog) each want to
/// act on it differently, which a prose message cannot support.
///
/// Expected to GROW. Only reasons the runner can actually distinguish belong
/// here; inventing variants it cannot tell apart would produce confidently
/// wrong diagnoses. Today the only detectable cause is an unwinding
/// orchestrator thread, so there is exactly one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortReason {
    /// The thread driving the run unwound (a panic), so it never reached the
    /// point where it would have sent [`ValidationEvent::Finished`].
    ///
    /// Any per-file events already delivered are real, but the run's totals
    /// were never computed and whatever remained is unprocessed.
    Panicked,
}

impl std::fmt::Display for AbortReason {
    /// Render the reason as a sentence safe to show a user verbatim.
    ///
    /// Lives here, next to the variants, so every surface says the same thing:
    /// before this existed the desktop bridge composed its own wording, which
    /// is how two consumers of one fact start describing it differently.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Panicked => f.write_str(
                "The validator stopped without finishing: an internal error \
                 ended the run. Any results shown are incomplete.",
            ),
        }
    }
}

/// Validation events streamed to caller
///
/// # Exhaustive on purpose
///
/// This enum is deliberately NOT `#[non_exhaustive]`. Adding a variant is a
/// breaking change for external consumers, and that is the point: a new
/// terminal event that a consumer silently ignores is exactly the defect
/// [`ValidationEvent::Aborted`] was added to fix. A downstream crate that
/// bumps chatter should get `error[E0004]` and decide what a dead run means
/// for its own UI, rather than inheriting a default of "pretend it finished".
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    /// Directory discovery started - shows user that work is beginning
    Discovering,
    /// File discovery complete and validation starting.
    Started {
        /// Number of `.cha` files found.
        total_files: usize,
    },
    /// Batch of errors discovered for a single file.
    Errors(ErrorEvent),
    /// A file finished validation.
    FileComplete(FileCompleteEvent),
    /// Roundtrip test completed for a file
    RoundtripComplete(RoundtripEvent),
    /// Every discovered file was accounted for; final summary statistics.
    ///
    /// This variant is the ONLY basis for a claim about the whole input, such
    /// as "all files valid" or a zero exit status. A cancelled run still
    /// arrives here (the shortfall was requested, and `stats.cancelled` says
    /// so), but a run that lost files does not; see
    /// [`ValidationEvent::FinishedIncomplete`].
    Finished(ValidationStatsSnapshot),
    /// The run reached its end but did NOT cover everything it discovered:
    /// worker threads unwound and abandoned files.
    ///
    /// Separate from [`ValidationEvent::Finished`] rather than a `lost` field
    /// beside it, because a field is something every consumer must remember to
    /// check, and forgetting produces a false clean bill of health, the worst
    /// available failure for a tool whose job is telling researchers whether
    /// their data is sound. As a distinct variant the compiler asks the
    /// question instead.
    FinishedIncomplete {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: ValidationStatsSnapshot,
        /// Files discovered but never accounted for. Always non-zero here.
        lost_files: usize,
    },
    /// The run stopped without processing every file, so no final statistics
    /// exist. Terminal, and mutually exclusive with
    /// [`ValidationEvent::Finished`]: exactly one of the two ends a stream.
    Aborted(AbortReason),
}
