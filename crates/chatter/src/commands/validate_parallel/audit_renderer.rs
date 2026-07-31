//! Audit-mode renderer: JSONL bulk output driven by the unified event stream.
//!
//! Audit mode is a *sink*, not a separate pipeline. It used to be the latter,
//! a standalone worker loop that re-walked the tree and accepted only four of
//! the command's options, so everything the runtime layers on top of raw
//! validation was silently dropped: `--suppress`, `--parser`,
//! `--strict-linkers`, `--roundtrip`, `--jobs` and `--max-errors`. Found when
//! `--suppress xphon --audit` reported `Invalid: 0`, exited 0, and wrote every
//! suppressed diagnostic into the audit file anyway.
//!
//! Modelling audit as one more [`ValidationRenderer`] fixes that by
//! construction: the renderer sees events from the SAME worker pool every
//! other presentation does, and suppression joins the rule set upstream of
//! validation (a suppressed code is never emitted at all), so every option
//! applies to the JSONL without anyone remembering to thread it through a
//! second implementation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::renderer::ValidationRenderer;
use crate::commands::validate::audit_reporter::{AuditReporter, AuditReporterHandle, AuditStats};
use talkbank_transform::validation_runner::{
    ErrorEvent, FileCompleteEvent, RoundtripEvent, ValidationStatsSnapshot,
};

/// Renderer that writes streamed diagnostics to a JSONL audit file.
pub struct AuditRenderer {
    /// Writer-thread owner, taken at `handle_finished` to flush and join.
    reporter: Option<AuditReporter>,
    /// Cloneable handle used to send records to the writer thread.
    handle: AuditReporterHandle,
    /// Paths already accounted for by an `Errors` event.
    ///
    /// `report_file_results` marks a file processed, so the matching
    /// `FileComplete` must not mark it a second time or `total_files` would
    /// double-count every file that produced diagnostics.
    reported: HashSet<PathBuf>,
    /// Per-code totals from the writer thread, available only after it joins.
    audit_stats: Option<AuditStats>,
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
        let reporter = AuditReporter::new(output_path)?;
        let handle = reporter.reporter();
        // Printed at construction rather than on `Discovering`: that event is
        // only emitted when the runner performs its own directory walk, and
        // the CLI hands this pipeline a pre-collected file list.
        println!("Running validation in audit mode...");
        println!("Output file: {}", output_path.display());
        println!();
        Ok(Self {
            reporter: Some(reporter),
            handle,
            reported: HashSet::new(),
            audit_stats: None,
            output_path: output_path.to_path_buf(),
        })
    }
}

impl ValidationRenderer for AuditRenderer {
    fn handle_discovering(&mut self) {}

    fn handle_started(&mut self, total_files: usize) {
        println!("Found {} files to validate", total_files);
        println!();
    }

    fn handle_errors(&mut self, error_event: &ErrorEvent) -> usize {
        // Reaching here means the worker's own `ValidationConfig` already
        // excluded suppressed codes, so every error in it is one the user
        // asked to see.
        self.handle.report_file_results(
            &error_event.path.to_string_lossy(),
            error_event.errors.clone(),
        );
        self.reported.insert(error_event.path.clone());
        error_event.errors.len()
    }

    fn handle_roundtrip_complete(&mut self, event: &RoundtripEvent) -> usize {
        // Roundtrip failures carry a reason rather than a `ParseError` list, so
        // they cannot become JSONL diagnostic records; account for the file and
        // let the run's exit code and summary carry the failure.
        if event.passed {
            return 0;
        }
        if self.reported.insert(event.path.clone()) {
            self.handle.mark_file_done(true);
        }
        1
    }

    fn handle_file_complete(&mut self, file_event: &FileCompleteEvent, files_completed: usize) {
        if files_completed.is_multiple_of(AUDIT_PROGRESS_INTERVAL) {
            eprintln!("Progress: {} files...", files_completed);
        }
        // A file whose diagnostics already streamed was marked processed by
        // `report_file_results`; marking it again would inflate `total_files`.
        if self.reported.remove(&file_event.path) {
            return;
        }
        self.handle
            .mark_file_done(super::renderer::status_is_error(&file_event.status));
    }

    fn handle_finished(
        &mut self,
        _stats: &ValidationStatsSnapshot,
        _files_completed: usize,
        _max_errors: Option<usize>,
        _error_count: usize,
    ) {
        // Flush and join the writer thread here: `print_summary` takes `&self`
        // but `AuditReporter::finish` consumes the reporter, so the audit
        // totals must be captured while `&mut self` is still available.
        let Some(reporter) = self.reporter.take() else {
            return;
        };
        match reporter.finish() {
            Ok(stats) => self.audit_stats = Some(stats),
            Err(error) => eprintln!("Error finalizing audit output: {}", error),
        }
    }

    fn print_summary(&self, _path: &Path, stats: &ValidationStatsSnapshot, _roundtrip: bool) {
        match self.audit_stats.as_ref() {
            Some(audit_stats) => audit_stats.print_summary(),
            None => eprintln!("Warning: audit summary unavailable (writer thread did not finish)"),
        }

        // Cache accounting comes from the RUN, not from the audit sink: the
        // sink sees only files that produced records. The runtime's snapshot
        // is the authority, and taking it from there is also why this renderer
        // needs no counters of its own.
        println!("Cache hits: {}", stats.cache_hits);
        println!("Cache misses: {}", stats.cache_misses);
        // The snapshot's own accessor, so audit and streaming report the SAME
        // number. A hand-rolled `hits / (hits + misses)` here silently gave a
        // different rate than every other surface, under an identical label.
        println!("Hit rate: {:.1}%", stats.cache_hit_rate());
        println!();
        println!("Detailed errors written to: {}", self.output_path.display());
    }
}
