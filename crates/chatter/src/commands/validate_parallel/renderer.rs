//! Output renderers for non-interactive parallel validation.
//!
//! The runtime drives a single event stream, while concrete renderers decide
//! how to present those events as text or JSONL.

use super::json_records::{
    CacheRecord, FileRecord, JsonRecord, NoticeRecord, RoundtripTotals, StopRecord, SummaryCounts,
    SummaryOutcome, SummaryRecord, WireDiagnostic, WireLossCause, wire_path,
};
use super::shared::{
    InterruptReport, RunNotice, incomplete_sentence, nothing_found_sentence, stop_sentence,
};
use crate::commands::validate::cache::CacheEvent;
use talkbank_transform::RoundtripCheck;

use std::path::Path;

use indicatif::ProgressStyle;

use super::{StreamedPresentation, Verbosity};
use crate::output::{CASCADING_HINT, print_errors, should_show_cascading_hint};
use crate::progress::ProgressThrottle;
use talkbank_transform::validation_runner::{
    AbortReason, CacheUse, FileCompleteEvent, FileDiagnostics, FileStatus, RunEnding,
    ValidationEvent, ValidationStatsSnapshot,
};

/// Build the renderer for a presentation mode.
///
/// Renderer construction lives here rather than in the runtime loop so that
/// the runtime has one line where presentation matters, and so every renderer
/// (text, JSON, audit) is created through a single seam.
///
/// Audit construction creates its output file, so a failure here means the
/// user asked for an unwritable `--audit` path. It is returned before any
/// validation work begins, and the command's boundary reports it and exits;
/// nothing here terminates the process.
pub fn create_presentation_renderer(
    presentation: &StreamedPresentation,
) -> Result<Box<dyn ValidationRenderer>, AuditFileError> {
    match presentation {
        StreamedPresentation::Lines { verbosity } => Ok(lines_renderer(*verbosity)),
        StreamedPresentation::Json => Ok(Box::new(JsonRenderer::new())),
        StreamedPresentation::Audit { output_path } => {
            match super::audit_renderer::AuditRenderer::new(output_path) {
                Ok(renderer) => Ok(Box::new(renderer)),
                Err(error) => Err(AuditFileError {
                    path: output_path.clone(),
                    error,
                }),
            }
        }
    }
}

/// The human-readable text renderer, which nothing can keep from being
/// built.
pub(super) fn lines_renderer(verbosity: Verbosity) -> Box<dyn ValidationRenderer> {
    Box::new(TextRenderer::new(verbosity))
}

/// The `--audit` file could not be created: the only way to build a
/// renderer that can fail, so the only failure, with its path.
pub struct AuditFileError {
    /// The path asked for.
    pub path: std::path::PathBuf,
    /// Why it could not be created.
    pub error: std::io::Error,
}

/// The stderr sentence for a run whose cache failed, for the renderers
/// whose channel is a terminal. JSON mode carries `cache_errors` in its
/// summary record instead, keeping stderr empty.
pub(super) fn cache_errors_sentence(stats: &ValidationStatsSnapshot) -> Option<String> {
    match stats.cache_errors() {
        0 => None,
        failures => Some(format!(
            "Warning: {failures} cache read(s) or write(s) failed; those files were validated without the cache."
        )),
    }
}

/// What every summary needs besides the run's ending.
#[derive(Clone, Copy)]
pub struct SummaryContext<'a> {
    /// The label the summary names the run by (the first input argument).
    pub label: &'a Path,
    /// Whether roundtrip counts belong in the summary.
    pub roundtrip: RoundtripCheck,
    /// Files whose completion this renderer saw.
    pub files_completed: usize,
}

/// A run rendered to its ending: how it ended, and whether what the
/// renderer promised was written.
pub struct RenderedRun {
    /// How the run ended.
    pub ending: RunEnding,
    /// Whether the renderer's output is whole.
    pub output: RenderedOutput,
}

/// Render a run's `events` through `renderer` to the run's ending: the one
/// command-line loop over the runner's stream, for `validate` and `watch`.
///
/// A stream that closes with no ending is an abort, never a success: the
/// runner's drop guard makes it unreachable, and if that ever regressed
/// this keeps the failure visible.
pub(super) fn render_run(
    events: impl IntoIterator<Item = ValidationEvent>,
    renderer: &mut dyn ValidationRenderer,
    label: &Path,
    roundtrip: RoundtripCheck,
) -> RenderedRun {
    let mut ending: Option<RunEnding> = None;
    let mut files_completed = 0usize;
    for event in events {
        match event {
            ValidationEvent::Discovering => renderer.handle_discovering(),
            ValidationEvent::Started { total_files } => renderer.handle_started(total_files),
            ValidationEvent::FileComplete(file) => {
                files_completed += 1;
                renderer.handle_file_complete(file, files_completed);
            }
            ValidationEvent::Finished(finished) => ending = Some(finished),
        }
    }
    let ending = ending.unwrap_or(RunEnding::Aborted(AbortReason::NoEnding));
    let output = renderer.finish(
        &ending,
        SummaryContext {
            label,
            roundtrip,
            files_completed,
        },
    );
    RenderedRun { ending, output }
}

/// Whether a run's output is whole: an audit's file, or JSON mode's
/// standard output, can fail apart from the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderedOutput {
    /// Everything the run promised was written.
    Complete,
    /// The `--audit` file could not be written whole (its error already
    /// said), so the run fails whatever it found.
    AuditFileIncomplete,
    /// JSON mode's standard output stopped taking records (a consumer that
    /// closed the pipe), so the stream is incomplete and the run fails.
    StdoutIncomplete,
}

/// Rendering interface for streamed validation events.
///
/// Rendering only: nothing here decides when a run stops. The error limit
/// is the runner's ([`talkbank_transform::ErrorLimit`]), counted from each
/// file's status, so a renderer returns no counts.
pub trait ValidationRenderer {
    /// Handle the start of file discovery.
    fn handle_discovering(&mut self);
    /// Handle the start of validation once the total file count is known.
    fn handle_started(&mut self, total_files: usize);
    /// Render one file's result: its status and its diagnostics, together,
    /// once. Taken by value, so a renderer that keeps the diagnostics moves
    /// them rather than copying.
    fn handle_file_complete(&mut self, file: FileCompleteEvent, files_completed: usize);
    /// Render how the run ended, in this renderer's channel: the progress
    /// display's end, a stop or loss, and the summary. Exhaustive over
    /// [`RunEnding`], so every ending, including the failures, is rendered on
    /// purpose and never by a stray write elsewhere. Says whether the output
    /// the run promised is whole.
    fn finish(&mut self, end: &RunEnding, summary: SummaryContext<'_>) -> RenderedOutput;

    /// Say one fact about the run before it starts, in this renderer's
    /// channel.
    fn handle_notice(&mut self, notice: &RunNotice);

    /// Whether a Ctrl-C may be acknowledged on stderr.
    fn interrupt_report(&self) -> InterruptReport;

    /// Render one fact about what cache maintenance did.
    ///
    /// On the trait rather than decided inside the cache, because "how does
    /// this run speak" is exactly what a renderer is for, and a cache event is
    /// an event like any other. Putting it here also forces the question the
    /// old stderr write let everyone avoid: whether an AUDIT run records that
    /// it pruned twelve rows.
    fn handle_cache_event(&mut self, event: &CacheEvent);
}

/// Human-readable renderer with optional progress throttling.
struct TextRenderer {
    /// How much to print.
    verbosity: Verbosity,
    /// Whether a progress display is showing.
    progress: TextProgress,
}

/// The text renderer's progress display. A valid file prints no line of its
/// own in any state; it is counted in the summary.
enum TextProgress {
    /// Showing, until the first detailed error output or the run's end.
    Showing(ProgressThrottle),
    /// Not showing: quiet mode, or taken down for detailed error output or
    /// at the run's end.
    Off,
}

impl TextRenderer {
    /// Create a new text renderer.
    fn new(verbosity: Verbosity) -> Self {
        Self {
            verbosity,
            progress: match verbosity {
                Verbosity::Quiet => TextProgress::Off,
                Verbosity::Normal => TextProgress::Showing(ProgressThrottle::new(
                    0,
                    "{spinner:.green} [{bar:40.cyan/blue}] {msg} {elapsed_precise}",
                    "#>-",
                    1,
                    200,
                )),
            },
        }
    }

    /// Take the progress display down, returning it to finish or clear.
    fn take_progress(&mut self) -> Option<ProgressThrottle> {
        match std::mem::replace(&mut self.progress, TextProgress::Off) {
            TextProgress::Showing(progress_bar) => Some(progress_bar),
            TextProgress::Off => None,
        }
    }

    /// End the progress display with `message`, at the files seen.
    fn finish_progress(&mut self, files_completed: usize, message: &'static str) {
        if let Some(progress_bar) = self.take_progress() {
            progress_bar.bar().set_position(files_completed as u64);
            progress_bar.bar().finish_with_message(message);
        }
    }

    /// Remove the progress display: before detailed output (a file's
    /// diagnostics or failure), or for a run with no totals to show.
    fn clear_progress(&mut self) {
        if let Some(mut progress_bar) = self.take_progress() {
            progress_bar.finish_and_clear();
        }
    }

    /// Say that the cache failed, if it did. Not skipped in quiet mode: a
    /// failing cache is a problem to report.
    fn report_cache_errors(&self, stats: &ValidationStatsSnapshot) {
        if let Some(sentence) = cache_errors_sentence(stats) {
            eprintln!("{sentence}");
        }
    }

    /// The summary block. `status` names an ending other than complete, so
    /// the counts are never read as totals for the input.
    fn print_summary(
        &self,
        stats: &ValidationStatsSnapshot,
        roundtrip: RoundtripCheck,
        status: Option<&str>,
    ) {
        match self.verbosity {
            Verbosity::Quiet => return,
            Verbosity::Normal => {}
        }

        outln!("\n=== Summary ===");
        outln!("Total files: {}", stats.total_files());
        outln!("Valid: {}", stats.valid_files());
        outln!("Invalid: {}", stats.invalid_files());
        if stats.internal_failures() > 0 {
            outln!(
                "Internal failures (validity undetermined): {}",
                stats.internal_failures()
            );
        }
        if let RoundtripCheck::Run = roundtrip {
            outln!("\n=== Roundtrip ===");
            outln!("Passed: {}", stats.roundtrip_passed());
            outln!("Failed: {}", stats.roundtrip_failed());
        }
        if let Some(status) = status {
            outln!("Status: {status}");
        }
        outln!("\n=== Cache Statistics ===");
        for line in cache_summary_lines(stats) {
            outln!("{line}");
        }
    }
}

/// The cache lines of a summary, said the same way by the text and audit
/// renderers: the counts and rate of the consultations there were, or that
/// the run consulted no cache (never a "0 hits, N misses" it did not have).
pub(super) fn cache_summary_lines(stats: &ValidationStatsSnapshot) -> Vec<String> {
    match stats.cache_hit_rate() {
        None => vec!["Cache: not consulted".to_owned()],
        Some(rate) => vec![
            format!("Cache hits: {}", stats.cache_hits()),
            format!("Cache misses: {}", stats.cache_misses()),
            format!("Hit rate: {rate:.1}%"),
        ],
    }
}

impl ValidationRenderer for TextRenderer {
    fn handle_discovering(&mut self) {
        if let TextProgress::Showing(progress_bar) = &self.progress {
            progress_bar.set_length(1);
            progress_bar.bar().set_message("Discovering files...");
        }
    }

    fn handle_started(&mut self, total_files: usize) {
        if let TextProgress::Showing(progress_bar) = &self.progress {
            progress_bar.set_length(total_files as u64);
            let style = match ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {msg} {elapsed_precise}")
            {
                Ok(style) => style,
                Err(error) => {
                    eprintln!("Error setting progress style: {}", error);
                    ProgressStyle::default_bar()
                }
            };
            progress_bar.bar().set_style(style.progress_chars("#>-"));
        }
    }

    fn handle_file_complete(&mut self, file: FileCompleteEvent, files_completed: usize) {
        // A failed file names itself on the progress line.
        if let (TextProgress::Showing(progress_bar), true) =
            (&mut self.progress, file.status.failed())
        {
            progress_bar.bar().set_position(files_completed as u64);
            let filename = file
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| file.path.display().to_string());
            progress_bar.set_message_throttled(format!("✗ {filename}"), true);
        }

        if let Some(shown) = file.status.shown() {
            self.clear_progress();
            print_errors(&file.path, shown.source, shown.errors);
            if let Verbosity::Normal = self.verbosity
                && should_show_cascading_hint(shown.errors)
            {
                eprintln!("{}", CASCADING_HINT);
            }
        }

        match &file.status {
            FileStatus::Valid { .. } | FileStatus::Invalid { .. } => {}
            FileStatus::InternalFailure { failure, .. } => {
                self.clear_progress();
                eprintln!("✗ {} ({failure})", file.path.display());
            }
            FileStatus::RoundtripFailed { reason, diff, .. } => {
                self.clear_progress();
                eprintln!("✗ {} (roundtrip failed)", file.path.display());
                eprintln!("  {reason}");
                if let Some(diff) = diff {
                    eprintln!("{diff}");
                }
            }
            FileStatus::ReadError { message } => {
                self.clear_progress();
                eprintln!("✗ {} (read error: {})", file.path.display(), message);
            }
        }
    }

    fn finish(&mut self, end: &RunEnding, summary: SummaryContext<'_>) -> RenderedOutput {
        match end {
            RunEnding::Complete(stats) => {
                let stats = stats.snapshot();
                self.finish_progress(summary.files_completed, "Validation complete");
                self.report_cache_errors(stats);
                self.print_summary(stats, summary.roundtrip, None);
            }
            RunEnding::NothingFound => {
                self.clear_progress();
                eprintln!("{}", nothing_found_sentence(summary.label));
            }
            RunEnding::Stopped { stats, reason } => {
                let unprocessed = stats.missing_files();
                let stats = stats.snapshot();
                self.finish_progress(summary.files_completed, "Validation stopped");
                self.report_cache_errors(stats);
                eprintln!("{}", stop_sentence(*reason, unprocessed));
                let status = format!("STOPPED ({reason}); {unprocessed} file(s) not validated");
                self.print_summary(stats, summary.roundtrip, Some(&status));
            }
            RunEnding::Incomplete { stats, cause } => {
                let lost_files = stats.missing_files();
                let stats = stats.snapshot();
                self.finish_progress(summary.files_completed, "Validation incomplete");
                // Before the summary, because the summary's numbers are
                // exactly what must not be read as totals for the input.
                eprintln!("{}", incomplete_sentence(stats, lost_files, cause));
                self.report_cache_errors(stats);
                let status = format!("INCOMPLETE; {lost_files} file(s) never accounted for");
                self.print_summary(stats, summary.roundtrip, Some(&status));
            }
            RunEnding::Aborted(reason) => {
                self.clear_progress();
                eprintln!("Error: {reason}");
            }
        }
        RenderedOutput::Complete
    }

    /// A terminal run says it in words, on stderr, where it always did.
    fn handle_cache_event(&mut self, event: &CacheEvent) {
        eprintln!("{}", event.sentence());
    }

    fn handle_notice(&mut self, notice: &RunNotice) {
        eprintln!("{}", notice.sentence());
    }

    fn interrupt_report(&self) -> InterruptReport {
        InterruptReport::OnStderr
    }
}

/// JSONL renderer for machine-readable output, writing to stdout (whose
/// own line buffering flushes each record).
struct JsonRenderer {
    /// Standard output, or why it stopped taking records: a consumer that
    /// closed the pipe, for one. After a failure nothing more is written.
    out: JsonOutput,
}

/// Where the JSON renderer's records go.
enum JsonOutput {
    /// Standard output, still taking records.
    Writing(std::io::Stdout),
    /// A write failed; the stream is incomplete.
    Failed,
}

impl JsonRenderer {
    fn new() -> Self {
        Self {
            out: JsonOutput::Writing(std::io::stdout()),
        }
    }

    /// Write one record, unless an earlier write failed. A failed write
    /// ends the stream (stderr stays empty in JSON mode; the run's exit
    /// status says it).
    fn write(&mut self, record: &JsonRecord<'_>) {
        if let JsonOutput::Writing(stdout) = &self.out
            && record.write_line(&mut stdout.lock()).is_err()
        {
            self.out = JsonOutput::Failed;
        }
    }
}

impl ValidationRenderer for JsonRenderer {
    fn handle_discovering(&mut self) {}

    fn handle_started(&mut self, _total_files: usize) {}

    /// One record per file, from its one event: a valid file with warnings
    /// is one `valid` record carrying them, never an `invalid` record and
    /// then a `valid` one.
    fn handle_file_complete(&mut self, file: FileCompleteEvent, _files_completed: usize) {
        self.write(&file_record(&file));
    }

    /// Every ending is records on stdout, never a stderr line: JSON mode
    /// keeps stderr empty. A stop or a loss is its own record, BEFORE the
    /// summary, and the summary's `outcome` names the ending.
    fn finish(&mut self, end: &RunEnding, summary: SummaryContext<'_>) -> RenderedOutput {
        // The record that explains the ending, before the summary.
        match end {
            RunEnding::Complete(_) | RunEnding::NothingFound => {}
            RunEnding::Stopped { stats, reason, .. } => {
                let unprocessed_files = stats.missing_files().get();
                self.write(&JsonRecord::Stop(match reason {
                    talkbank_transform::CancelReason::ErrorLimit { limit } => {
                        StopRecord::MaxErrors {
                            limit: limit.get(),
                            unprocessed_files,
                        }
                    }
                    talkbank_transform::CancelReason::Requested => {
                        StopRecord::Cancelled { unprocessed_files }
                    }
                }));
            }
            RunEnding::Incomplete { stats, cause } => {
                self.write(&JsonRecord::Incomplete {
                    lost_files: stats.missing_files().get(),
                    total_files: stats.snapshot().total_files().get(),
                    cause: WireLossCause::of(cause),
                    detail: cause.to_string(),
                });
            }
            RunEnding::Aborted(reason) => self.write(&JsonRecord::Aborted {
                reason: reason.to_string(),
            }),
        }
        if let Some(record) = json_summary(summary, end) {
            self.write(&record);
        }
        match self.out {
            JsonOutput::Writing(_) => RenderedOutput::Complete,
            JsonOutput::Failed => RenderedOutput::StdoutIncomplete,
        }
    }

    /// A machine-facing run gets a record on the stream, because stderr is
    /// promised empty and silence would throw away a result the caller asked
    /// for by passing `--force`.
    fn handle_cache_event(&mut self, event: &CacheEvent) {
        self.write(&JsonRecord::Cache(match event {
            CacheEvent::Pruned { rows, versions } => CacheRecord::Prune {
                rows_deleted: *rows,
                versions_deleted: *versions,
            },
            CacheEvent::Cleared { entries } => CacheRecord::Clear {
                entries_cleared: *entries,
            },
            CacheEvent::MaintenanceFailed { operation, error } => CacheRecord::Warning {
                operation: *operation,
                error,
            },
        }));
    }

    /// A record on stdout: stderr is promised empty.
    fn handle_notice(&mut self, notice: &RunNotice) {
        self.write(&JsonRecord::Notice(match notice {
            RunNotice::Suppressing(codes) => NoticeRecord::Suppressing {
                codes: codes.codes().iter().map(|code| code.as_str()).collect(),
            },
            RunNotice::CheckXphonIgnored => NoticeRecord::DeprecatedFlag {
                flag: "--check-xphon",
            },
            RunNotice::InterruptUnavailable(error) => NoticeRecord::InterruptUnavailable { error },
        }));
    }

    /// Silent: the stop is reported as the run's `stop` record.
    fn interrupt_report(&self) -> InterruptReport {
        InterruptReport::Silent
    }
}

/// The JSON summary record for a run's ending, its outcome and counts read
/// off the one ending so they cannot disagree. Only a complete summary
/// counts the whole input, a run that found nothing has no counts at all,
/// and an aborted run, which has no totals, has no summary.
fn json_summary<'a>(summary: SummaryContext<'a>, end: &RunEnding) -> Option<JsonRecord<'a>> {
    let (outcome, stats) = match end {
        RunEnding::Complete(stats) => (SummaryOutcome::Complete, Some(stats.snapshot())),
        RunEnding::NothingFound => (SummaryOutcome::NothingFound, None),
        RunEnding::Stopped { stats, .. } => (SummaryOutcome::Stopped, Some(stats.snapshot())),
        RunEnding::Incomplete { stats, .. } => (SummaryOutcome::Incomplete, Some(stats.snapshot())),
        RunEnding::Aborted(_) => return None,
    };
    Some(JsonRecord::Summary(SummaryRecord {
        directory: wire_path(summary.label),
        outcome,
        counts: stats.map(|stats| SummaryCounts {
            total_files: stats.total_files().get(),
            valid: stats.valid_files(),
            invalid: stats.invalid_files(),
            internal_failures: stats.internal_failures(),
            cache_hits: stats.cache_hits(),
            cache_misses: stats.cache_misses(),
            cache_hit_rate: stats.cache_hit_rate(),
            cache_errors: stats.cache_errors(),
            roundtrip: match summary.roundtrip {
                RoundtripCheck::Run => Some(RoundtripTotals {
                    roundtrip_passed: stats.roundtrip_passed(),
                    roundtrip_failed: stats.roundtrip_failed(),
                }),
                RoundtripCheck::Skip => None,
            },
        }),
    }))
}

/// The wire form of `errors`.
fn wire_diagnostics(errors: &[talkbank_model::ParseError]) -> Vec<WireDiagnostic<'_>> {
    errors.iter().map(WireDiagnostic::of).collect()
}

/// The wire form of a file's warnings, empty when it showed none.
fn wire_warnings(warnings: Option<&FileDiagnostics>) -> Vec<WireDiagnostic<'_>> {
    match warnings {
        Some(warnings) => wire_diagnostics(warnings.errors()),
        None => Vec::new(),
    }
}

/// A file's one JSON record, from its status and the diagnostics it carries.
fn file_record(file: &FileCompleteEvent) -> JsonRecord<'_> {
    JsonRecord::File(match &file.status {
        // `cache_hit` is a boolean on the wire, as the contract says: a hit
        // or not. A file that consulted no cache is simply not a hit.
        FileStatus::Valid { warnings, .. } => FileRecord::Valid {
            file: wire_path(&file.path),
            cache_hit: matches!(file.cache, CacheUse::Hit),
            warnings: wire_warnings(warnings.as_ref()),
        },
        FileStatus::Invalid { diagnostics } => FileRecord::Invalid {
            file: wire_path(&file.path),
            error_count: diagnostics.error_count().get(),
            errors: wire_diagnostics(diagnostics.shown().errors()),
            note: match should_show_cascading_hint(diagnostics.shown().errors()) {
                true => Some(
                    "Some additional checks may not have run because of structural errors. \
                     Fix the structural errors first, then re-validate.",
                ),
                false => None,
            },
        },
        FileStatus::RoundtripFailed {
            reason,
            diff,
            warnings,
        } => FileRecord::RoundtripFailed {
            file: wire_path(&file.path),
            reason,
            diff: diff.as_deref(),
            warnings: wire_warnings(warnings.as_ref()),
        },
        // The failure owns the diagnostics of its attempt: one record, never
        // an invalid-file record beside it. A wire diagnostic carries no
        // location, so a reparse failure's evidence is safe to list too.
        FileStatus::InternalFailure { failure, .. } => FileRecord::InternalFailure {
            file: wire_path(&file.path),
            error: failure.to_string(),
            errors: wire_diagnostics(failure.diagnostics()),
        },
        FileStatus::ReadError { message } => FileRecord::ReadError {
            file: wire_path(&file.path),
            error: message,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tool failure is one `internal_failure` record that keeps its
    /// attempt's diagnostics, and it fails the file.
    #[test]
    fn internal_failure_json_preserves_evidence_without_a_validity_record() {
        let failure =
            talkbank_model::CompletedDiagnostics::admit(vec![talkbank_model::ParseError::at_span(
                talkbank_model::ErrorCode::InternalError,
                talkbank_model::Severity::Warning,
                talkbank_model::Span::new(0, 1),
                "fault",
            )])
            .unwrap_err();
        let file = FileCompleteEvent {
            path: "sample.cha".into(),
            status: FileStatus::InternalFailure {
                failure,
                attempt: talkbank_transform::validation_runner::FailedAttempt::RoundtripReparse,
            },
            cache: CacheUse::Miss,
        };
        let record = serde_json::to_value(file_record(&file)).expect("records serialize");
        assert_eq!(record["type"], "file");
        assert_eq!(record["status"], "internal_failure");
        assert_eq!(record["errors"][0]["severity"], "Warning");
        assert_eq!(record["errors"][0]["code"], "E001");
        assert!(
            record["error"]
                .as_str()
                .unwrap()
                .contains("validity was not determined")
        );
        assert!(file.status.failed());
    }
}
