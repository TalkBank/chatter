//! Validation event types and status enums
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::num::NonZeroUsize;
use std::path::PathBuf;
use talkbank_model::ParseError;

/// Whether the roundtrip check ran on a valid file.
///
/// A failed roundtrip is its own [`FileStatus::RoundtripFailed`], so this
/// only has to say whether the check ran and passed, or was never asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundtripVerdict {
    /// The run did not ask for a roundtrip check.
    NotRequested,
    /// The check ran, or its cached result was read, and the file wrote
    /// back byte for byte.
    Passed,
}

/// A file's validation verdict, carrying the diagnostics it showed. Each
/// variant holds exactly the diagnostics its verdict can have: an invalid
/// file always has its errors, a valid one at most warnings, and an
/// unreadable one none, so no consumer pairs a verdict with a list that
/// contradicts it.
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// The tool failed; no valid or invalid verdict was established.
    InternalFailure {
        /// Complete diagnostic evidence from the failed attempt.
        failure: talkbank_model::InternalFailure,
        /// Which attempt failed, which says whether its diagnostics can be
        /// placed in the file's own text.
        attempt: FailedAttempt,
    },
    /// File passed validation, and the roundtrip check if it was requested.
    Valid {
        /// Whether the roundtrip check ran on this file. The counters are
        /// derived from this status, so a valid file whose roundtrip passed
        /// says so here rather than in a counter someone must remember to
        /// bump on every branch that builds a valid status.
        roundtrip: RoundtripVerdict,
        /// The warnings it showed, if any. `None` for a file served valid
        /// from the cache, which by construction showed nothing.
        warnings: Option<FileDiagnostics>,
    },
    /// File has validation errors: at least one shown diagnostic of
    /// `Severity::Error`.
    Invalid {
        /// The diagnostics it showed, errors and warnings, with its count
        /// of errors.
        diagnostics: InvalidDiagnostics,
    },
    /// Validation passed but the file did not write back byte for byte.
    RoundtripFailed {
        /// Why (e.g. "Roundtrip mismatch (serialization not idempotent)").
        reason: String,
        /// The first differing lines, when the check computed them (a cached
        /// failure has none).
        diff: Option<String>,
        /// The warnings it showed, if any (it validated, so no errors).
        warnings: Option<FileDiagnostics>,
    },
    /// File could not be read from disk.
    ReadError {
        /// Human-readable I/O error message.
        message: String,
    },
}

/// Which attempt a tool failure interrupted.
#[derive(Debug, Clone)]
pub enum FailedAttempt {
    /// Validating the file's own text: the failure's diagnostics point into
    /// `source`, so they can be shown against the file.
    Validation {
        /// The file's text.
        source: String,
    },
    /// Reparsing the serialized text during the roundtrip check: the
    /// failure's diagnostics point into that text, not the file's, so no
    /// surface places them in the file. Their codes and messages remain on
    /// the failure as evidence.
    RoundtripReparse,
}

impl FileStatus {
    /// The errors this file spends from the run's
    /// [`ErrorLimit`](super::ErrorLimit): its `Severity::Error` diagnostics
    /// (after suppression), or 1 for a failed roundtrip. Warnings, unread
    /// files and tool failures spend nothing.
    pub(super) fn errors_found(&self) -> usize {
        match self {
            Self::Invalid { diagnostics } => diagnostics.error_count().get(),
            Self::RoundtripFailed { .. } => 1,
            Self::Valid { .. } | Self::InternalFailure { .. } | Self::ReadError { .. } => 0,
        }
    }

    /// Whether the file failed: invalid, unreadable, failed its roundtrip,
    /// or met a tool failure. Only a valid file did not, warnings or not.
    pub fn failed(&self) -> bool {
        match self {
            Self::Valid { .. } => false,
            Self::Invalid { .. }
            | Self::InternalFailure { .. }
            | Self::RoundtripFailed { .. }
            | Self::ReadError { .. } => true,
        }
    }

    /// The diagnostics to show against the file's own text, with that
    /// text, or `None` when there are none to place there: a clean or
    /// cached valid file, an unreadable one, and a tool failure during the
    /// roundtrip reparse, whose spans point into the serialized text.
    pub fn shown(&self) -> Option<ShownDiagnostics<'_>> {
        match self {
            Self::Valid { warnings, .. } | Self::RoundtripFailed { warnings, .. } => {
                warnings.as_ref().map(FileDiagnostics::view)
            }
            Self::Invalid { diagnostics } => Some(diagnostics.shown.view()),
            Self::InternalFailure {
                failure,
                attempt: FailedAttempt::Validation { source },
            } => Some(ShownDiagnostics {
                errors: failure.diagnostics(),
                source,
            }),
            Self::InternalFailure {
                attempt: FailedAttempt::RoundtripReparse,
                ..
            }
            | Self::ReadError { .. } => None,
        }
    }
}

/// How one file used the run's cache: a separate fact from its status,
/// carried on [`FileCompleteEvent`], so a run with no cache never reports
/// a miss it did not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheUse {
    /// The verdict that decided the file's status came from the cache: its
    /// cached Valid verdict, or a cached roundtrip verdict (also on a file
    /// whose validation verdict was not cached).
    Hit,
    /// The cache was consulted and had no usable verdict (or the read
    /// failed, which the run also counts as a cache error).
    Miss,
    /// The cache was not consulted: the run has none, or the file could not
    /// be read.
    NotConsulted,
}

/// One worker's (or the whole run's) counts of file outcomes.
///
/// Plain counters, owned by one worker and RETURNED from it: the pool sums
/// the workers' tallies after the join, so no counter is shared between
/// threads and no total is read while workers still write it. The one way to
/// count a file is [`Self::record`], an exhaustive match on its
/// [`FileStatus`], so each file lands in exactly one outcome bucket.
///
/// Crate-private: a consumer reads counts from a [`ValidationStatsSnapshot`]
/// the runner made, and cannot assemble one from numbers of its own.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidationTally {
    valid_files: usize,
    invalid_files: usize,
    cache_hits: usize,
    cache_misses: usize,
    cache_errors: usize,
    internal_failures: usize,
    roundtrip_passed: usize,
    roundtrip_failed: usize,
}

impl ValidationTally {
    /// Count one completed file: its outcome bucket (an exhaustive match
    /// on its status, so each file lands in exactly one) and how it used the
    /// cache.
    pub(crate) fn record(&mut self, status: &FileStatus, cache: CacheUse) {
        match status {
            FileStatus::InternalFailure { .. } => self.internal_failures += 1,
            FileStatus::Valid { roundtrip, .. } => {
                self.valid_files += 1;
                // The roundtrip count is read off the status, which every
                // branch that builds a Valid status has to fill in.
                match roundtrip {
                    RoundtripVerdict::Passed => self.roundtrip_passed += 1,
                    RoundtripVerdict::NotRequested => {}
                }
            }
            FileStatus::Invalid { .. } => self.invalid_files += 1,
            FileStatus::RoundtripFailed { .. } => {
                // Roundtrip failures count as invalid files, and as roundtrips.
                self.invalid_files += 1;
                self.roundtrip_failed += 1;
            }
            FileStatus::ReadError { .. } => self.invalid_files += 1,
        }
        match cache {
            CacheUse::Hit => self.cache_hits += 1,
            CacheUse::Miss => self.cache_misses += 1,
            CacheUse::NotConsulted => {}
        }
    }

    /// Count one cache read or write that failed. The file was still
    /// validated, without the cache; a failing cache is reported in the run's
    /// totals instead of looking like a cold one.
    pub(crate) fn record_cache_error(&mut self) {
        self.cache_errors += 1;
    }

    /// Two tallies together.
    pub(crate) fn add(self, other: Self) -> Self {
        Self {
            valid_files: self.valid_files + other.valid_files,
            invalid_files: self.invalid_files + other.invalid_files,
            cache_hits: self.cache_hits + other.cache_hits,
            cache_misses: self.cache_misses + other.cache_misses,
            cache_errors: self.cache_errors + other.cache_errors,
            internal_failures: self.internal_failures + other.internal_failures,
            roundtrip_passed: self.roundtrip_passed + other.roundtrip_passed,
            roundtrip_failed: self.roundtrip_failed + other.roundtrip_failed,
        }
    }

    /// The run's report: these counts over `total_files` discovered files.
    /// How the run ENDED (complete, stopped, incomplete) is not a count; it
    /// is the [`RunEnding`] that carries this snapshot.
    pub(crate) fn snapshot(self, total_files: NonZeroUsize) -> ValidationStatsSnapshot {
        ValidationStatsSnapshot {
            total_files,
            counts: self,
        }
    }
}

/// A run's counts over the files it discovered, made only by the runner.
///
/// The counts are the run's own `ValidationTally`, read through accessors:
/// there is no public constructor and no public field, so a consumer (or a
/// test) cannot assert a clean run from numbers it chose. `total_files` is
/// non-zero because a run that found nothing ends [`RunEnding::NothingFound`]
/// and has no snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationStatsSnapshot {
    total_files: NonZeroUsize,
    counts: ValidationTally,
}

/// Producer-admitted counts covering every discovered file.
///
/// A snapshot cloned from a stopped or incomplete run cannot construct this
/// proof. Consumers can inspect or clone admitted counts, but cannot promote
/// partial counts to a complete run.
///
/// ```compile_fail,E0308
/// # use talkbank_transform::{RunEnding, ValidationStatsSnapshot};
/// # fn promote_partial(stats: ValidationStatsSnapshot) -> RunEnding {
/// RunEnding::Complete(stats)
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStats {
    snapshot: ValidationStatsSnapshot,
}

impl CompleteStats {
    /// Read-only counts covering the whole input.
    pub fn snapshot(&self) -> &ValidationStatsSnapshot {
        &self.snapshot
    }
}

/// Producer-admitted counts with discovered files still unaccounted for.
///
/// The nonzero shortfall is derived from these counts at admission, not
/// supplied separately by a caller when constructing a run ending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialStats {
    snapshot: ValidationStatsSnapshot,
    missing: NonZeroUsize,
}

impl PartialStats {
    /// Read-only counts describing only the processed portion of the input.
    pub fn snapshot(&self) -> &ValidationStatsSnapshot {
        &self.snapshot
    }

    /// Discovered files that produced no per-file result.
    pub fn missing_files(&self) -> NonZeroUsize {
        self.missing
    }
}

/// How much of what a run discovered it actually accounted for.
///
/// Derived from a [`ValidationStatsSnapshot`], never counted alongside it: a
/// third counter tracking "files lost" could drift from the two it is supposed
/// to reconcile, whereas a subtraction cannot. See
/// [`ValidationStatsSnapshot::coverage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunCoverage {
    /// Every discovered file produced a per-file result. The snapshot's totals
    /// describe the whole of what was asked for.
    Complete(CompleteStats),
    /// Files were discovered and never produced a result. Whether that was a
    /// requested stop or a loss is not a count; the runner decides it from
    /// why the run stopped and how its workers ended.
    Shortfall(PartialStats),
}

impl ValidationStatsSnapshot {
    /// Transcripts discovered (readable or not).
    pub fn total_files(&self) -> NonZeroUsize {
        self.total_files
    }

    /// Files that passed validation (and the roundtrip check, if asked).
    pub fn valid_files(&self) -> usize {
        self.counts.valid_files
    }

    /// Files that did not: invalid, unreadable, or failing the roundtrip.
    pub fn invalid_files(&self) -> usize {
        self.counts.invalid_files
    }

    /// Files whose validity the tool failed to determine.
    pub fn internal_failures(&self) -> usize {
        self.counts.internal_failures
    }

    /// Files whose deciding verdict was served from the cache.
    pub fn cache_hits(&self) -> usize {
        self.counts.cache_hits
    }

    /// Files for which the cache was consulted and had no usable verdict.
    /// Files that never consulted a cache (a run without one, an unreadable
    /// file) count as neither a hit nor a miss.
    pub fn cache_misses(&self) -> usize {
        self.counts.cache_misses
    }

    /// Cache reads or writes that failed; those files were validated
    /// without the cache.
    pub fn cache_errors(&self) -> usize {
        self.counts.cache_errors
    }

    /// Files whose roundtrip check passed.
    pub fn roundtrip_passed(&self) -> usize {
        self.counts.roundtrip_passed
    }

    /// Files whose roundtrip check failed (also counted as invalid).
    pub fn roundtrip_failed(&self) -> usize {
        self.counts.roundtrip_failed
    }

    /// The share of cache consultations that hit, as a percentage, or
    /// `None` when the run consulted no cache at all (a 0% "rate" would
    /// claim misses that never happened).
    pub fn cache_hit_rate(&self) -> Option<f64> {
        match self.cache_hits() + self.cache_misses() {
            0 => None,
            consulted => Some(self.cache_hits() as f64 / consulted as f64 * 100.0),
        }
    }

    /// Files that failed: invalid, unreadable, failing their roundtrip, or
    /// a tool failure. The one count a run's verdict and every surface's
    /// "files with errors" read.
    pub fn failed_files(&self) -> usize {
        self.invalid_files()
            .saturating_add(self.internal_failures())
    }

    /// Files that produced a per-file result of any kind.
    ///
    /// Exactly one of these three counters is incremented per completed file
    /// (`ValidationTally::record` folds roundtrip failures and read errors
    /// into `invalid_files`), so this is a count of files, not of events.
    pub fn files_accounted_for(&self) -> usize {
        self.valid_files()
            .saturating_add(self.invalid_files())
            .saturating_add(self.internal_failures())
    }

    /// Reconcile what was discovered against what was actually processed.
    ///
    /// A worker thread that unwinds abandons whatever files it had taken off
    /// the queue, and the missing files contribute to no counter, so partial
    /// totals look clean. This is the single place that difference is
    /// computed, and the runner turns it into the run's [`RunEnding`].
    pub(crate) fn coverage(self) -> RunCoverage {
        match NonZeroUsize::new(
            self.total_files
                .get()
                .saturating_sub(self.files_accounted_for()),
        ) {
            None => RunCoverage::Complete(CompleteStats { snapshot: self }),
            Some(missing) => RunCoverage::Shortfall(PartialStats {
                snapshot: self,
                missing,
            }),
        }
    }
}

/// The diagnostics a file showed (after suppression), with the source text
/// they point into. Never empty, and made only by `Shown::of`, which
/// decides from the same list whether the file is invalid.
#[derive(Debug, Clone)]
pub struct FileDiagnostics {
    errors: Vec<ParseError>,
    source: String,
}

impl FileDiagnostics {
    /// The diagnostics, in the order the validator reported them.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// The file's text, which every diagnostic's span points into.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The borrowed form every surface renders.
    fn view(&self) -> ShownDiagnostics<'_> {
        ShownDiagnostics {
            errors: &self.errors,
            source: &self.source,
        }
    }
}

/// An invalid file's diagnostics: at least one of them an error, counted
/// once, where the list was classified.
#[derive(Debug, Clone)]
pub struct InvalidDiagnostics {
    shown: FileDiagnostics,
    error_count: NonZeroUsize,
}

impl InvalidDiagnostics {
    /// The diagnostics, errors and warnings.
    pub fn shown(&self) -> &FileDiagnostics {
        &self.shown
    }

    /// How many of them are `Severity::Error`.
    pub fn error_count(&self) -> NonZeroUsize {
        self.error_count
    }
}

/// Diagnostics to show against a file's own text, borrowed from its status
/// by [`FileStatus::shown`].
#[derive(Debug, Clone, Copy)]
pub struct ShownDiagnostics<'a> {
    /// The diagnostics.
    pub errors: &'a [ParseError],
    /// The file's text, which their spans point into.
    pub source: &'a str,
}

/// What a freshly validated file showed, classified once from its shown
/// list: the one route to a [`FileDiagnostics`] or an
/// [`InvalidDiagnostics`].
pub(crate) enum Shown {
    /// No diagnostic.
    Nothing,
    /// Warnings, no error: the file is valid.
    Warnings(FileDiagnostics),
    /// At least one error: the file is invalid.
    Errors(InvalidDiagnostics),
}

impl Shown {
    /// Classify `errors`, shown against `source`.
    pub(crate) fn of(errors: Vec<ParseError>, source: String) -> Self {
        let error_count = NonZeroUsize::new(
            errors
                .iter()
                .filter(|e| matches!(e.severity, talkbank_model::Severity::Error))
                .count(),
        );
        match (error_count, errors.is_empty()) {
            (Some(error_count), _) => Self::Errors(InvalidDiagnostics {
                shown: FileDiagnostics { errors, source },
                error_count,
            }),
            (None, true) => Self::Nothing,
            (None, false) => Self::Warnings(FileDiagnostics { errors, source }),
        }
    }
}

/// One file's result: exactly one per file the run accounted for, carrying
/// everything the run has to say about that file (its diagnostics are on
/// its status), so a consumer renders it once (one JSON record, one TUI
/// entry, one audit entry).
#[derive(Debug, Clone)]
pub struct FileCompleteEvent {
    /// Path to the completed file.
    pub path: PathBuf,
    /// Validation result for the file, with the diagnostics it showed.
    pub status: FileStatus,
    /// How the file used the run's cache.
    pub cache: CacheUse,
}

/// Why a validation run ended without any totals.
///
/// Carried by [`RunEnding::Aborted`] so a consumer can tell the user
/// something specific rather than "it just stopped". A closed enum rather
/// than a string: each surface (a CLI exit path, a TUI banner, a desktop
/// dialog) acts on the reason, and all of them say it with one wording, its
/// `Display`.
///
/// Only reasons that can actually be told apart belong here: inventing
/// variants nobody can distinguish would produce confidently wrong
/// diagnoses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortReason {
    /// The thread driving the run unwound (a panic), so it never reached the
    /// point where it would have sent its ending. The runner's drop guard
    /// reports this.
    ///
    /// Any per-file events already delivered are real, but the run's totals
    /// were never computed and whatever remained is unprocessed.
    Panicked,
    /// The event stream closed with no ending at all. Said by a CONSUMER,
    /// never by the runner: the drop guard makes it unreachable, and a
    /// consumer that meets it anyway reports it as an abort rather than as
    /// a finished run, so a regression of the guard can never read as
    /// success.
    NoEnding,
}

impl std::fmt::Display for AbortReason {
    /// Render the reason as a sentence safe to show a user verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Panicked => f.write_str(
                "The validator stopped without finishing: an internal error \
                 ended the run. Any results shown are incomplete.",
            ),
            Self::NoEnding => f.write_str(
                "The validator stopped without reporting any result. \
                 Any results shown are incomplete.",
            ),
        }
    }
}

/// Why a run ended without accounting for every file it discovered, when
/// the shortfall is not explained by a requested stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LossCause {
    /// Workers failed. Every failure the run observed is listed; a worker
    /// failure outranks a stop, since it loses files the stop did not
    /// explain.
    WorkerFaults(WorkerFaults),
    /// No worker failed and no stop was asked for, yet files are missing: a
    /// defect in the runner itself.
    Unexplained,
}

/// One way the run's workers failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerFault {
    /// The pool's own failure: workers unwound, or a worker thread was
    /// refused.
    Pool(crate::worker_pool::PoolFault),
    /// Workers could not create their parser, so they took no file. When
    /// every worker fails this way, nothing is validated.
    ParserUnavailable {
        /// How many workers failed.
        workers: NonZeroUsize,
        /// The parser they tried to create.
        parser: talkbank_model::ParserKind,
        /// Why, as the first failing worker reported it.
        reason: String,
    },
}

/// The worker failures one run observed: never empty, since the one
/// constructor answers `None` for a run whose workers all ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerFaults(Vec<WorkerFault>);

impl WorkerFaults {
    /// The faults of a finished pool: how it ended, and the setup failures
    /// its workers returned. `None` when every worker started and returned.
    pub(super) fn observe<'a>(
        pool: &crate::worker_pool::PoolOutcome,
        setup_failures: impl IntoIterator<Item = &'a super::worker::WorkerSetupFailure>,
    ) -> Option<Self> {
        let mut faults: Vec<WorkerFault> = pool.faults().map(WorkerFault::Pool).collect();
        let mut setup_failures = setup_failures.into_iter();
        if let Some(first) = setup_failures.next() {
            faults.push(WorkerFault::ParserUnavailable {
                workers: NonZeroUsize::MIN.saturating_add(setup_failures.count()),
                parser: first.parser,
                reason: first.reason.clone(),
            });
        }
        match faults.is_empty() {
            true => None,
            false => Some(Self(faults)),
        }
    }

    /// Each fault, in the order observed.
    pub fn iter(&self) -> impl Iterator<Item = &WorkerFault> {
        self.0.iter()
    }
}

impl std::fmt::Display for WorkerFault {
    /// The fault as a clause safe to show a user verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pool(fault) => fault.fmt(f),
            Self::ParserUnavailable {
                workers,
                parser,
                reason,
            } => write!(
                f,
                "{workers} worker(s) could not start the {parser:?} parser ({reason})"
            ),
        }
    }
}

impl std::fmt::Display for LossCause {
    /// The cause as a sentence safe to show a user verbatim, so every
    /// surface says the same thing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WorkerFaults(faults) => {
                let mut first = true;
                for fault in faults.iter() {
                    match first {
                        true => write!(f, "{fault}")?,
                        false => write!(f, "; {fault}")?,
                    }
                    first = false;
                }
                f.write_str(".")
            }
            Self::Unexplained => f.write_str(
                "no worker failed and no stop was requested; this is a defect in the validator.",
            ),
        }
    }
}

/// How a validation run ended: exactly one per run, the last event of its
/// stream ([`ValidationEvent::Finished`]).
///
/// The one owner of every claim about the whole input. A consumer matches it
/// exhaustively, so a new ending breaks every consumer's build instead of
/// falling into a default, and [`Self::passed`] is the single answer to
/// "may this run be reported as a success".
///
/// The endings carry producer-admitted [`CompleteStats`] or [`PartialStats`].
/// Cloning a partial snapshot cannot establish complete-run provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnding {
    /// At least one file was discovered and every one was accounted for.
    ///
    /// The ONLY ending entitled to a claim about the whole input, such as
    /// "all files valid" or a zero exit status. A run that was told to stop
    /// but had already covered every file ends here too: nothing was left
    /// to stop, so nothing is reported as stopped.
    Complete(CompleteStats),
    /// The input named no transcript at all, and nothing unreadable either:
    /// the run validated nothing, so it vouches for nothing.
    NothingFound,
    /// The run was told to stop (its error limit, or its caller) and did,
    /// with files never validated.
    ///
    /// Its own variant, never a `cancelled` flag beside full totals: a flag is
    /// something a consumer forgets, and forgetting it here reads a stopped
    /// run as a clean one.
    Stopped {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: PartialStats,
        /// Why the run stopped.
        reason: super::CancelReason,
    },
    /// The run reached its end but did NOT cover everything it discovered,
    /// for a reason other than a requested stop: see [`LossCause`].
    ///
    /// Separate from [`RunEnding::Complete`] rather than a `lost` field beside
    /// it, because a field is something every consumer must remember to
    /// check, and forgetting produces a false clean bill of health.
    Incomplete {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: PartialStats,
        /// Why they were lost.
        cause: LossCause,
    },
    /// The run produced no totals at all.
    Aborted(AbortReason),
}

impl RunEnding {
    /// Whether the run vouches for every file it was given: it is
    /// [`Self::Complete`], and no file was invalid, unreadable, failing its
    /// roundtrip, or a tool failure. Warnings do not fail a run.
    ///
    /// Every surface (the CLI's exit status, the TUI, the desktop app) asks
    /// this one question, so they cannot disagree about a run.
    pub fn passed(&self) -> bool {
        match self {
            Self::Complete(stats) => stats.snapshot().failed_files() == 0,
            Self::NothingFound
            | Self::Stopped { .. }
            | Self::Incomplete { .. }
            | Self::Aborted(_) => false,
        }
    }

    /// The run's counts, for the endings that have them: complete, stopped
    /// and incomplete runs. Only a complete run's counts describe the whole
    /// input.
    pub fn stats(&self) -> Option<&ValidationStatsSnapshot> {
        match self {
            Self::Complete(stats) => Some(stats.snapshot()),
            Self::Stopped { stats, .. } | Self::Incomplete { stats, .. } => Some(stats.snapshot()),
            Self::NothingFound | Self::Aborted(_) => None,
        }
    }
}

/// Validation events streamed to caller
///
/// # Exhaustive on purpose
///
/// This enum is deliberately NOT `#[non_exhaustive]`. Adding a variant is a
/// breaking change for external consumers, and that is the point: a new
/// event that a consumer silently ignores would be a defect. A downstream
/// crate that bumps chatter gets `error[E0004]` and decides what the new
/// event means for its own UI.
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    /// Directory discovery started - shows user that work is beginning
    Discovering,
    /// File discovery complete and validation starting.
    Started {
        /// Number of `.cha` files found.
        total_files: usize,
    },
    /// A file finished: its status, cache use and diagnostics, together.
    FileComplete(FileCompleteEvent),
    /// How the run ended. Terminal: exactly one ends every stream.
    Finished(RunEnding),
}
