//! Worker-thread execution logic for validation and optional roundtrip.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::cancel::{CancelSignal, ErrorBudget};
use super::config::{ParserKind, RoundtripCheck, RunCache, ValidationConfig};
use super::roundtrip::{self, RoundtripResult};
use super::types::{
    CacheUse, FailedAttempt, FileCompleteEvent, FileDiagnostics, FileStatus, RoundtripVerdict,
    Shown, ValidationEvent, ValidationTally,
};
use crate::paths::StoredTranscript;
use crossbeam_channel::Sender;
use std::fs;
use std::path::Path;
use talkbank_cache::{
    CacheLookup, CacheOutcome, ContentHash, ResolvedPath, RoundtripOutcome, ValidationCache,
    VerdictReader,
};
use talkbank_model::{ChatFile, ChatParser, ErrorSink, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Dispatch enum wrapping both parser backends.
///
/// Avoids trait objects (which would require `ErrorSink` monomorphization)
/// while keeping the worker code parser-agnostic.
pub(super) enum ParserDispatch {
    /// Tree-sitter parser (default, supports incremental reparsing).
    TreeSitter(TreeSitterParser),
    /// Re2c DFA parser (faster batch validation).
    Re2c(Re2cParser),
    /// Inject a producer failure without inventing a malformed-CHAT fixture.
    #[cfg(test)]
    InternalFailure,
    /// Consume one real parse, then inject a fault at the roundtrip boundary.
    #[cfg(test)]
    FailAfterInitial(std::cell::Cell<Option<TreeSitterParser>>),
}

impl ParserDispatch {
    /// Create the appropriate parser for the given `ParserKind`.
    pub(super) fn new(kind: ParserKind) -> Result<Self, WorkerSetupFailure> {
        match kind {
            ParserKind::TreeSitter => TreeSitterParser::new()
                .map(ParserDispatch::TreeSitter)
                .map_err(|error| WorkerSetupFailure {
                    parser: kind,
                    reason: error.to_string(),
                }),
            ParserKind::Re2c => Ok(ParserDispatch::Re2c(Re2cParser::new())),
        }
    }

    /// Parse a complete CHAT file with streaming error reporting.
    ///
    /// Both backends always return a ChatFile (best-effort recovery).
    pub(super) fn parse_chat_file_streaming(
        &self,
        input: &str,
        errors: &impl ErrorSink,
    ) -> ChatFile {
        match self {
            #[cfg(test)]
            Self::FailAfterInitial(first) => match first.take() {
                Some(parser) => parser.parse_chat_file_streaming(input, errors),
                None => Self::InternalFailure.parse_chat_file_streaming(input, errors),
            },
            #[cfg(test)]
            Self::InternalFailure => {
                errors.report(talkbank_model::ParseError::at_span(
                    talkbank_model::ErrorCode::InternalError,
                    talkbank_model::Severity::Warning,
                    talkbank_model::Span::new(0, 1),
                    "injected producer fault",
                ));
                ChatFile::new(vec![])
            }
            Self::TreeSitter(p) => p.parse_chat_file_streaming(input, errors),
            Self::Re2c(p) => match p.parse_chat_file(input, 0, errors) {
                ParseOutcome::Parsed(cf) => cf,
                ParseOutcome::Rejected => ChatFile::new(vec![]),
            },
        }
    }
}

/// What a worker borrows from the run for its whole life. The pool's
/// threads are scoped, so these are plain references: no `Arc` clones and no
/// per-worker copy of the configuration.
pub(super) struct WorkerContext<'a> {
    /// Where per-file events go.
    pub(super) event_tx: &'a Sender<ValidationEvent>,
    /// The run's stop latch.
    pub(super) cancel: &'a CancelSignal,
    /// The run's error count against its limit.
    pub(super) budget: &'a ErrorBudget,
    /// The run's cache, and what the run may do with it.
    pub(super) cache: &'a RunCache,
    /// The run's settings.
    pub(super) config: &'a ValidationConfig,
}

/// A worker that could not start: it took no file from the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkerSetupFailure {
    /// The parser it tried to create.
    pub(super) parser: ParserKind,
    /// Why it could not.
    pub(super) reason: String,
}

/// Main loop executed by each validation worker thread. Returns the worker's
/// own tally, which the runner sums after the join, or why the worker could
/// not start, which the runner reports if files go missing.
pub(super) fn worker_loop(
    work: impl Iterator<Item = StoredTranscript>,
    context: &WorkerContext<'_>,
) -> Result<ValidationTally, WorkerSetupFailure> {
    let parser = ParserDispatch::new(context.config.parser_kind).inspect_err(|failure| {
        tracing::error!(
            parser = ?failure.parser,
            reason = %failure.reason,
            "A validation worker could not create its parser"
        );
    })?;
    Ok(worker_loop_with_parser(work, context, parser))
}

/// The worker owns one initialized parser for its entire queue. The separate
/// boundary also lets tests inject a tool fault into the real cache/event path.
pub(super) fn worker_loop_with_parser(
    mut work: impl Iterator<Item = StoredTranscript>,
    context: &WorkerContext<'_>,
    parser: ParserDispatch,
) -> ValidationTally {
    let mut tally = ValidationTally::default();
    loop {
        // Check for cancellation. Reads a LATCH, not the raw channel: polling
        // the channel here consumed the single cancel token, so only one of the
        // N workers ever saw it and the rest ran the queue to the end. See
        // `CancelSignal`.
        if context.cancel.reason().is_some() {
            break;
        }
        // The queue closed and drained: no more files.
        let Some(stored) = work.next() else {
            break;
        };
        let complete = process_file(stored, context, &parser, &mut tally);
        tally.record(&complete.status, complete.cache);
        // Spent from the status itself, before the next file is taken, so a
        // limit reached here stops this worker at once and the others at
        // their next file.
        context
            .budget
            .spend(complete.status.errors_found(), context.cancel);
        // A closed result stream ends the worker: the completed file is
        // accounted for, but no later file may begin after delivery fails.
        if context
            .event_tx
            .send(ValidationEvent::FileComplete(complete))
            .is_err()
        {
            break;
        }
    }
    tally
}

/// The cache use of a file that was validated (not served from the cache):
/// a miss when there was a cache to consult, nothing otherwise.
fn validated_fresh(cache: &RunCache) -> CacheUse {
    match cache {
        RunCache::Absent => CacheUse::NotConsulted,
        RunCache::ReadOnly(_) | RunCache::ReadWrite(_) => CacheUse::Miss,
    }
}

/// One stored transcript from the queue to its result: its status, how it
/// used the cache, and the diagnostics it showed. The caller counts it and
/// sends it as the file's one event.
fn process_file(
    stored: StoredTranscript,
    context: &WorkerContext<'_>,
    parser: &ParserDispatch,
    tally: &mut ValidationTally,
) -> FileCompleteEvent {
    let config = context.config;
    let complete = |status, cache| FileCompleteEvent {
        path: stored.path().to_path_buf(),
        status,
        cache,
    };

    // Read once. The cache is keyed by the hash of these bytes, so a verdict
    // is looked up and stored for exactly the content that is validated.
    let content = match fs::read_to_string(stored.path()) {
        Ok(content) => content,
        Err(e) => {
            let message = e.to_string();
            return complete(FileStatus::ReadError { message }, CacheUse::NotConsulted);
        }
    };
    let hash = ContentHash::of(content.as_bytes());
    // The transcript's resolved location, made when it was found: every
    // lookup and store below keys by it, as `--force` clears by it.
    let cached = stored.resolved();
    let file_path = stored.path();

    // Serve a cached Valid verdict without parsing. An Invalid verdict is
    // re-validated, so its diagnostics can be shown. Whether the roundtrip
    // verdict was already looked up (and missed) is carried forward, so it
    // is never looked up twice for one file.
    let mut roundtrip_lookup = RoundtripLookup::NotYet;
    if let Some(CacheOutcome::Valid) = lookup(context, tally, file_path, |cache| {
        cache.get(cached, &hash, config.alignment)
    }) {
        tracing::debug!(file = ?file_path, "Cache hit (valid) - skipping reparse");
        match config.roundtrip {
            RoundtripCheck::Skip => {
                // A cached Valid verdict means the file showed nothing.
                return complete(
                    FileStatus::Valid {
                        roundtrip: RoundtripVerdict::NotRequested,
                        warnings: None,
                    },
                    CacheUse::Hit,
                );
            }
            RoundtripCheck::Run => {
                match lookup(context, tally, file_path, |cache| {
                    cache.get_roundtrip(cached, &hash, config.alignment)
                }) {
                    Some(roundtrip) => {
                        return complete(cached_roundtrip_status(roundtrip, None), CacheUse::Hit);
                    }
                    // Roundtrip not cached: fall through to full processing.
                    None => roundtrip_lookup = RoundtripLookup::Missed,
                }
            }
        }
    }

    tracing::debug!(file = ?file_path, "Cache miss - parsing file");
    let cache_use = validated_fresh(context.cache);
    let policy = talkbank_model::validation::ValidationPolicy::new(config.rules, config.alignment);
    let (completed, chat_file) =
        match validate_single_file_streaming(&stored, policy, parser, &content) {
            Ok(completed) => completed,
            Err(failure) => {
                // The attempt's own diagnostics stay on the failure, shown
                // against the file's text; no cache admission, presentation
                // suppression, or roundtrip.
                return complete(
                    FileStatus::InternalFailure {
                        failure,
                        attempt: FailedAttempt::Validation { source: content },
                    },
                    cache_use,
                );
            }
        };
    let complete_diagnostics = completed.into_diagnostics();

    // THE CACHED FACT, derived from the COMPLETE diagnostic set and
    // therefore true under every presentation policy: did this file
    // produce any diagnostic at all?
    //
    // Only a file with none is cached Valid. Warnings must be shown
    // on every run until the user fixes them, so a warnings-only
    // file must not be cached Valid (that would silently hide it),
    // and a file whose only diagnostics are currently suppressed
    // must not be either, or the row would be a rendering rather
    // than a fact and the next run with different `--suppress`
    // would be served a view built for someone else.
    let validation_outcome = match complete_diagnostics.is_empty() {
        true => CacheOutcome::Valid,
        false => CacheOutcome::Invalid,
    };

    // Presentation is applied HERE, at the boundary where the run
    // hands results to a consumer, and never upstream of the fact
    // above.
    let shown = config.presentation.apply_all(complete_diagnostics);

    // The file's cache use is that of the verdict that decided its status:
    // a cached roundtrip verdict on a freshly validated file is a hit. The
    // text moves into the status with the diagnostics that point into it.
    let mut passed = |warnings| match config.roundtrip {
        RoundtripCheck::Run => run_roundtrip(
            &chat_file,
            parser,
            RoundtripTarget {
                file_path,
                cached,
                hash: &hash,
                lookup: roundtrip_lookup,
                warnings,
            },
            context,
            tally,
        ),
        RoundtripCheck::Skip => (
            FileStatus::Valid {
                roundtrip: RoundtripVerdict::NotRequested,
                warnings,
            },
            cache_use,
        ),
    };
    let (status, cache_use) = match Shown::of(shown, content) {
        Shown::Errors(diagnostics) => (FileStatus::Invalid { diagnostics }, cache_use),
        Shown::Nothing => passed(None),
        Shown::Warnings(warnings) => passed(Some(warnings)),
    };

    // A producer fault during the optional reparse publishes no cache
    // entry, and its status carries none of the input's findings:
    // consumers would read the findings as an invalid file, contradicting
    // the failure.
    match &status {
        FileStatus::InternalFailure { .. } => {}
        FileStatus::Valid { .. }
        | FileStatus::Invalid { .. }
        | FileStatus::RoundtripFailed { .. }
        | FileStatus::ReadError { .. } => store(context, tally, file_path, |cache| {
            cache.set(cached, &hash, config.alignment, validation_outcome)
        }),
    }

    complete(status, cache_use)
}

/// Whether a file's roundtrip verdict has been looked up yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoundtripLookup {
    /// Not yet: look it up before running the check.
    NotYet,
    /// Looked up and missed (on the cached-valid path): run the check.
    Missed,
}

/// Read the cache, when the run has one: a hit's verdict, `None` on a miss
/// or with no cache, and on a failure `None` too, counted in the tally and
/// logged, so the file is validated without the cache and the run's totals
/// say the cache failed.
fn lookup<V>(
    context: &WorkerContext<'_>,
    tally: &mut ValidationTally,
    file_path: &Path,
    query: impl FnOnce(&dyn VerdictReader) -> Result<CacheLookup<V>, talkbank_cache::CacheError>,
) -> Option<V> {
    let reader: &dyn VerdictReader = match context.cache {
        RunCache::Absent => return None,
        RunCache::ReadOnly(reader) => reader.as_ref(),
        RunCache::ReadWrite(cache) => {
            let cache: &dyn ValidationCache = cache.as_ref();
            cache
        }
    };
    match query(reader) {
        Ok(CacheLookup::Hit(outcome)) => Some(outcome),
        Ok(CacheLookup::Miss) => None,
        Err(error) => {
            tracing::warn!(file = ?file_path, %error, "Cache read failed; validating without it");
            tally.record_cache_error();
            None
        }
    }
}

/// Write to the cache, when the run may; a failure is counted and logged.
/// Only a [`RunCache::ReadWrite`] has a write method to call.
fn store(
    context: &WorkerContext<'_>,
    tally: &mut ValidationTally,
    file_path: &Path,
    write: impl FnOnce(&dyn ValidationCache) -> Result<(), talkbank_cache::CacheError>,
) {
    let cache = match context.cache {
        RunCache::Absent | RunCache::ReadOnly(_) => return,
        RunCache::ReadWrite(cache) => cache.as_ref(),
    };
    if let Err(error) = write(cache) {
        tracing::warn!(file = ?file_path, %error, "Cache write failed");
        tally.record_cache_error();
    }
}

/// The status a cached roundtrip verdict gives a valid file with these
/// warnings. One place for both routes to a cached roundtrip: a
/// cached-valid file (no warnings), and a freshly validated one.
fn cached_roundtrip_status(
    roundtrip: RoundtripOutcome,
    warnings: Option<FileDiagnostics>,
) -> FileStatus {
    match roundtrip {
        RoundtripOutcome::Passed => FileStatus::Valid {
            roundtrip: RoundtripVerdict::Passed,
            warnings,
        },
        RoundtripOutcome::Failed => FileStatus::RoundtripFailed {
            reason: "Roundtrip failed (cached)".to_owned(),
            diff: None,
            warnings,
        },
    }
}

/// The file a roundtrip check is about, whether its cached verdict has
/// been looked up yet, and the warnings its validation showed.
struct RoundtripTarget<'a> {
    file_path: &'a Path,
    cached: &'a ResolvedPath,
    hash: &'a ContentHash,
    lookup: RoundtripLookup,
    warnings: Option<FileDiagnostics>,
}

/// Run the roundtrip check on a valid file, or read its cached verdict.
/// Returns the resulting status and its cache use: a hit when the roundtrip
/// verdict came from the cache, otherwise the use of the fresh validation
/// that preceded it.
fn run_roundtrip(
    chat_file: &ChatFile,
    parser: &ParserDispatch,
    target: RoundtripTarget<'_>,
    context: &WorkerContext<'_>,
    tally: &mut ValidationTally,
) -> (FileStatus, CacheUse) {
    let config = context.config;
    let RoundtripTarget {
        file_path,
        cached,
        hash,
        lookup: looked_up,
        warnings,
    } = target;
    // Check the roundtrip cache first, unless the cached-valid path already
    // did and missed.
    let cached_verdict = match looked_up {
        RoundtripLookup::NotYet => lookup(context, tally, file_path, |cache| {
            cache.get_roundtrip(cached, hash, config.alignment)
        }),
        RoundtripLookup::Missed => None,
    };
    if let Some(roundtrip) = cached_verdict {
        return (cached_roundtrip_status(roundtrip, warnings), CacheUse::Hit);
    }
    let fresh = validated_fresh(context.cache);

    let result = match roundtrip::run_roundtrip(chat_file, parser) {
        Ok(result) => result,
        // These spans belong to serialized roundtrip text, not the original
        // source. Retain the fault in the status without attributing it to
        // the user's source as diagnostics.
        Err(failure) => {
            let attempt = FailedAttempt::RoundtripReparse;
            return (FileStatus::InternalFailure { failure, attempt }, fresh);
        }
    };

    // Cache the roundtrip result
    let roundtrip_outcome = match &result {
        RoundtripResult::Passed => RoundtripOutcome::Passed,
        RoundtripResult::Failed(_) => RoundtripOutcome::Failed,
    };
    store(context, tally, file_path, |cache| {
        cache.set_roundtrip(cached, hash, config.alignment, roundtrip_outcome)
    });

    let status = match result {
        RoundtripResult::Passed => FileStatus::Valid {
            roundtrip: RoundtripVerdict::Passed,
            warnings,
        },
        RoundtripResult::Failed(failure) => FileStatus::RoundtripFailed {
            reason: failure.reason(),
            diff: failure.diff().map(str::to_string),
            warnings,
        },
    };
    (status, fresh)
}

/// Validate one file and return its COMPLETE diagnostic set, plus the parsed
/// `ChatFile` (which roundtrip testing needs).
///
/// "Complete" is the load-bearing word: every diagnostic the selected rules
/// produced, at the severity the validator assigned, with nothing suppressed or
/// re-labelled. The caller derives the cached fact and the run's tallies from
/// this, then applies the presentation policy on the way to a reader.
///
/// It emits no events and takes no sink, so there is no seam here through which
/// a display preference could reach what gets cached: that seam is exactly what
/// let a `--suppress` list decide a cache row's value in v0.6.0.
fn validate_single_file_streaming(
    transcript: &StoredTranscript,
    policy: talkbank_model::validation::ValidationPolicy,
    parser: &ParserDispatch,
    content: &str,
) -> Result<(talkbank_model::CompletedDiagnostics, ChatFile), talkbank_model::InternalFailure> {
    // Collect all diagnostics during validation (no streaming).
    let collector = talkbank_model::ErrorCollector::new();

    // Parse with error collection.
    let mut chat_file = parser.parse_chat_file_streaming(content, &collector);
    let parsed = talkbank_model::CompletedDiagnostics::admit(collector.into_vec())?;
    let collector = talkbank_model::ErrorCollector::new();
    collector.report_all(parsed.into_diagnostics());

    // Disk validation requires a stored identity. Argument spellings and
    // anonymous contexts cannot enter this function; resolution failures were
    // reported as read errors before cache admission.
    let name = transcript.name();

    chat_file.validate_at(policy, &collector, name);

    Ok((
        talkbank_model::CompletedDiagnostics::admit(collector.into_vec())?,
        chat_file,
    ))
}
