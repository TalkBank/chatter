//! Audit-mode renderer: JSONL bulk output driven by the unified event stream.
//!
//! Audit mode is a *sink*, not a separate pipeline: one more
//! [`ValidationRenderer`] over events from the SAME worker pool every other
//! presentation uses. Suppression joins the rule set upstream of validation
//! (a suppressed code is never emitted at all), so every option
//! (`--suppress`, `--parser`, `--strict-linkers`, `--roundtrip`, `--jobs`,
//! `--max-errors`) applies to the JSONL without being threaded through a
//! second implementation.

use std::path::{Path, PathBuf};

use super::renderer::{RenderedOutput, SummaryContext, ValidationRenderer};
use super::shared::{incomplete_sentence, nothing_found_sentence, stop_sentence};
use crate::commands::validate::audit_reporter::{AuditReporter, AuditReporterHandle, AuditStats};
use talkbank_transform::validation_runner::{
    FileCompleteEvent, RunEnding, ValidationStatsSnapshot,
};

/// Renderer that writes streamed diagnostics to a JSONL audit file.
pub struct AuditRenderer {
    /// Writer-thread owner, taken at `finish` to flush and join.
    reporter: Option<AuditReporter>,
    /// Cloneable handle used to send records to the writer thread.
    handle: AuditReporterHandle,
    /// Where the JSONL went, named in the summary so a corpus-scale run
    /// tells the operator where its artifact is.
    output_path: PathBuf,
}

/// Files between progress lines during long audit runs.
///
/// Audit output is normally redirected to a file rather than a terminal, so
/// the streaming progress bar is not usable here; periodic lines are what a
/// corpus-scale run has to show for itself.
const AUDIT_PROGRESS_INTERVAL: usize = 500;

impl AuditRenderer {
    /// Create an audit renderer writing JSONL records to `output_path`.
    pub fn new(output_path: &Path) -> std::io::Result<Self> {
        let (reporter, handle) = AuditReporter::new(output_path)?;
        // Said once the file exists, before the run starts.
        outln!("Running validation in audit mode...");
        outln!("Output file: {}", output_path.display());
        outln!();
        Ok(Self {
            reporter: Some(reporter),
            handle,
            output_path: output_path.to_path_buf(),
        })
    }
}

impl AuditRenderer {
    /// Flush and join the writer thread and return what it returned: its
    /// per-code totals, or why the audit file is incomplete, said here
    /// whatever the run's ending. A second call has no writer left to
    /// join, and says so as a failure rather than as an empty audit.
    fn finish_writer(&mut self) -> std::io::Result<AuditStats> {
        let written = match self.reporter.take() {
            Some(reporter) => reporter.finish(),
            None => Err(std::io::Error::other(
                "the audit writer was already finished",
            )),
        };
        if let Err(error) = &written {
            eprintln!(
                "Error: the audit file {} could not be written: {error}",
                self.output_path.display()
            );
        }
        written
    }

    /// The audit summary, the run's cache accounting and where the JSONL is.
    fn print_summary(
        &self,
        stats: &ValidationStatsSnapshot,
        written: &std::io::Result<AuditStats>,
    ) {
        if let Some(sentence) = super::renderer::cache_errors_sentence(stats) {
            eprintln!("{sentence}");
        }
        // A write failure was said when the writer finished; only a success
        // has a summary to print.
        if let Ok(audit_stats) = written {
            audit_stats.print_summary();
        }

        // Cache accounting comes from the RUN, not from the audit sink: the
        // sink sees only files that produced records. The lines are the ones
        // the text renderer prints, from the snapshot's own accessors.
        for line in super::renderer::cache_summary_lines(stats) {
            outln!("{line}");
        }
        outln!();
        outln!("Detailed errors written to: {}", self.output_path.display());
    }
}

impl ValidationRenderer for AuditRenderer {
    fn handle_discovering(&mut self) {}

    fn handle_started(&mut self, total_files: usize) {
        outln!("Found {} files to validate", total_files);
        outln!();
    }

    /// One file, accounted for once: its diagnostics, sent to the writer
    /// (the worker's own `ValidationConfig` already excluded suppressed
    /// codes), and whether it failed, read off its status, so the audit's
    /// count of files with errors is the run's (a valid file with warnings
    /// has records but did not fail; an unread file, a failed roundtrip and
    /// a tool failure may have none but did).
    fn handle_file_complete(&mut self, file: FileCompleteEvent, files_completed: usize) {
        if files_completed.is_multiple_of(AUDIT_PROGRESS_INTERVAL) {
            eprintln!("Progress: {} files...", files_completed);
        }
        let failed = file.status.failed();
        match file.status.shown() {
            Some(shown) => self.handle.report_file_results(
                &file.path.to_string_lossy(),
                shown.errors.to_vec(),
                failed,
            ),
            None => self.handle.mark_file_done(failed),
        }
    }

    /// Every ending flushes and joins the writer, so what was recorded is
    /// on disk even for a run that died; the stop or loss is said on the
    /// operator's terminal, and only a run with totals gets the summary.
    fn finish(&mut self, end: &RunEnding, summary: SummaryContext<'_>) -> RenderedOutput {
        let written = self.finish_writer();
        match end {
            RunEnding::Complete(stats) => self.print_summary(stats.snapshot(), &written),
            RunEnding::NothingFound => eprintln!("{}", nothing_found_sentence(summary.label)),
            RunEnding::Stopped { stats, reason } => {
                eprintln!("{}", stop_sentence(*reason, stats.missing_files()));
                self.print_summary(stats.snapshot(), &written);
            }
            RunEnding::Incomplete { stats, cause } => {
                eprintln!(
                    "{}",
                    incomplete_sentence(stats.snapshot(), stats.missing_files(), cause)
                );
                self.print_summary(stats.snapshot(), &written);
            }
            RunEnding::Aborted(reason) => eprintln!("Error: {reason}"),
        }
        match written {
            Ok(_) => RenderedOutput::Complete,
            Err(_) => RenderedOutput::AuditFileIncomplete,
        }
    }

    /// An audit records findings ABOUT TRANSCRIPTS, so a cache event does not
    /// belong in the file, and it is not silently dropped either: the operator
    /// running the audit is at a terminal and gets the sentence there.
    ///
    /// This is the question the old stderr write let everyone avoid. Putting
    /// the event on the trait made someone answer it, which is the point.
    fn handle_cache_event(&mut self, event: &crate::commands::validate::cache::CacheEvent) {
        eprintln!("{}", event.sentence());
    }

    /// The operator is at a terminal; a note about the run is not a finding
    /// about a transcript, so it does not go in the file.
    fn handle_notice(&mut self, notice: &super::shared::RunNotice) {
        eprintln!("{}", notice.sentence());
    }

    fn interrupt_report(&self) -> super::shared::InterruptReport {
        super::shared::InterruptReport::OnStderr
    }
}
