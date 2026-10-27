//! Streaming audit reporting for bulk validation runs.
//!
//! Audit mode needs to write JSONL diagnostics for large corpora without holding
//! all errors in memory. This module keeps that streaming behavior, but it no
//! longer shares a `BufWriter<File>` and stats accumulator behind mutexes.
//! Instead, [`AuditReporter`] owns a dedicated writer thread and exposes a
//! cloneable [`AuditReporterHandle`] for worker threads to send file results to.
//!
//! That split makes the concurrency boundary explicit:
//! - validation workers only send immutable audit events
//! - the writer thread owns file IO and summary accounting
//! - shutdown is a single `finish()` call that joins the writer thread and
//!   returns the final [`AuditStats`]

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use talkbank_model::{ErrorCode, ParseError, Severity};

use crate::commands::validate_parallel::json_records::WireSeverity;

mod destination;
pub(crate) use destination::AuditDestination;
pub use destination::AuditDestinationError;

/// Number of in-flight audit messages allowed before worker threads block.
const AUDIT_CHANNEL_CAPACITY: usize = 256;

/// How many diagnostics of each severity: what every count in the summary
/// is, so an error total cannot include a warning.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SeverityCounts {
    /// Diagnostics of severity `Error`.
    pub errors: usize,
    /// Diagnostics of severity `Warning`.
    pub warnings: usize,
}

impl SeverityCounts {
    /// Count one diagnostic of `severity`.
    fn add(&mut self, severity: Severity) {
        match severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
        }
    }
}

/// Statistics collected during audit runs to support the CLI’s bulk validation mode.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AuditStats {
    /// Total files processed.
    pub total_files: usize,
    /// Files that failed (invalid, unreadable, failing their roundtrip, or
    /// a tool failure), as the run counts them: a valid file with warnings
    /// is not one.
    pub files_failed: usize,
    /// Diagnostics across all files, by severity.
    pub diagnostics: SeverityCounts,
    /// Diagnostics grouped by code, by severity.
    pub diagnostics_by_code: HashMap<ErrorCode, SeverityCounts>,
    /// Distinct file paths grouped by code.
    pub files_by_code: HashMap<ErrorCode, HashSet<String>>,
}

impl AuditStats {
    /// Start a fresh audit accumulator with zeroed counters and empty maps.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one diagnostic of one file, counted under its own severity.
    pub fn record(&mut self, file: &str, diagnostic: &ParseError) {
        self.diagnostics.add(diagnostic.severity);
        self.diagnostics_by_code
            .entry(diagnostic.code)
            .or_default()
            .add(diagnostic.severity);
        self.files_by_code
            .entry(diagnostic.code)
            .or_default()
            .insert(file.to_string());
    }

    /// Mark a file as processed, tracking whether it failed.
    pub fn mark_file_processed(&mut self, failed: bool) {
        self.total_files += 1;
        if failed {
            self.files_failed += 1;
        }
    }

    /// Print the audit summary shown after CLI audit runs complete.
    pub fn print_summary(&self) -> crate::exit_codes::CommandResult {
        outln!("\n{}", "=".repeat(80));
        outln!("VALIDATION AUDIT SUMMARY");
        outln!("{}", "=".repeat(80));
        outln!();
        outln!("Total files processed: {}", self.total_files);
        outln!(
            "Files that failed: {} ({:.1}%)",
            self.files_failed,
            if self.total_files > 0 {
                100.0 * self.files_failed as f64 / self.total_files as f64
            } else {
                0.0
            }
        );
        outln!("Total errors: {}", self.diagnostics.errors);
        outln!("Total warnings: {}", self.diagnostics.warnings);
        outln!();

        if !self.diagnostics_by_code.is_empty() {
            outln!("Diagnostics by code:");
            let mut codes: Vec<_> = self.diagnostics_by_code.iter().collect();
            codes.sort_by_key(|(code, _)| code.as_str());
            for (code, counts) in codes {
                let file_count = self
                    .files_by_code
                    .get(code)
                    .map(|files| files.len())
                    .unwrap_or(0);
                outln!(
                    "  {}: {} error(s), {} warning(s) in {} file(s)",
                    code.as_str(),
                    counts.errors,
                    counts.warnings,
                    file_count
                );
            }
        }
        outln!();
        Ok(())
    }
}

/// Cloneable worker-side handle used to report completed file results.
#[derive(Clone)]
pub struct AuditReporterHandle {
    sender: Sender<AuditCommand>,
}

impl AuditReporterHandle {
    /// Create a new reporting handle for the given audit command sender.
    fn new(sender: Sender<AuditCommand>) -> Self {
        Self { sender }
    }

    /// Report all parse or validation errors for a file and mark it as processed.
    pub fn report_file_results(&self, file_path: &str, errors: Vec<ParseError>, failed: bool) {
        self.send(AuditCommand::FileResults {
            file_path: file_path.to_string(),
            errors,
            failed,
        });
    }

    /// Mark a file as processed when it showed no diagnostics.
    pub fn mark_file_done(&self, failed: bool) {
        self.send(AuditCommand::FileDone { failed });
    }

    /// Send one audit command to the writer thread.
    fn send(&self, command: AuditCommand) {
        if let Err(error) = self.sender.send(command) {
            eprintln!(
                "Warning: Failed to send audit event to writer thread: {}",
                error
            );
        }
    }
}

/// Lifecycle owner for the dedicated audit writer thread: made together with
/// the handle workers send through, and the one way to stop and join it.
pub struct AuditReporter {
    /// Owned until explicit completion or drop-time cleanup consumes it.
    running: Option<RunningAudit>,
}

/// A live worker can be joined once; only its result supplies audit statistics.
struct RunningAudit {
    /// The writer thread's sender for the shutdown command.
    shutdown: Sender<AuditCommand>,
    /// The writer thread owned by this completion operation.
    worker: JoinHandle<std::io::Result<FinishedAudit>>,
}

impl AuditReporter {
    /// Start the writer from the destination admitted against this run's inputs.
    pub(crate) fn new(
        destination: AuditDestination,
    ) -> std::io::Result<(Self, AuditReporterHandle)> {
        let mut writer = destination.into_writer()?;
        let (sender, receiver) = crossbeam_channel::bounded(AUDIT_CHANNEL_CAPACITY);
        let worker = std::thread::spawn(move || {
            let stats = run_audit_writer(&mut writer, receiver)?;
            let publication = writer.finish()?;
            Ok(FinishedAudit { stats, publication })
        });
        let handle = AuditReporterHandle::new(sender.clone());
        Ok((
            Self {
                running: Some(RunningAudit {
                    shutdown: sender,
                    worker,
                }),
            },
            handle,
        ))
    }

    /// Join and flush the writer, returning output that still requires run admission.
    pub fn finish(mut self) -> std::io::Result<FinishedAudit> {
        self.running
            .take()
            .ok_or_else(|| std::io::Error::other("audit writer was already consumed"))?
            .finish()
    }
}

impl RunningAudit {
    /// Consume the worker and return its actual completion, including failures.
    fn finish(self) -> std::io::Result<FinishedAudit> {
        // A closed channel means the writer is already gone; joining says why.
        let _ = self.shutdown.send(AuditCommand::Shutdown);
        self.worker
            .join()
            .map_err(|_| std::io::Error::other("audit writer thread panicked"))?
    }
}

/// Successfully flushed output, still unpublished until a complete run admits it.
pub struct FinishedAudit {
    stats: AuditStats,
    publication: destination::PendingPublication,
}

/// Publication outcome keeps a complete writer distinct from a complete run.
pub enum AuditCompletion {
    /// A complete run published its regular report or finished its stream.
    Published(AuditStats),
    /// The writer finished, but the run did not; staged output was discarded.
    Withheld(AuditStats),
}

impl FinishedAudit {
    /// Run completion, not validation success, authorizes regular report publication.
    pub fn conclude(
        self,
        end: &talkbank_transform::validation_runner::RunEnding,
    ) -> std::io::Result<AuditCompletion> {
        use talkbank_transform::validation_runner::RunEnding;
        match end {
            RunEnding::Complete(_) => self.publish().map(AuditCompletion::Published),
            RunEnding::NothingFound
            | RunEnding::Stopped { .. }
            | RunEnding::Incomplete { .. }
            | RunEnding::Aborted(_) => Ok(AuditCompletion::Withheld(self.stats)),
        }
    }

    /// Publish a completed run's report. Dropping this value discards staged output.
    fn publish(self) -> std::io::Result<AuditStats> {
        self.publication.publish()?;
        Ok(self.stats)
    }
}

impl Drop for AuditReporter {
    /// Join on drop; discard staged regular output without publishing it.
    fn drop(&mut self) {
        if let Some(running) = self.running.take()
            && let Err(error) = running.finish()
        {
            eprintln!("Warning: Failed to finalize audit output: {}", error);
        }
    }
}

/// Commands sent from validation workers to the audit writer thread.
enum AuditCommand {
    /// Report the full structured results for one completed file.
    FileResults {
        /// File path used in JSONL output and summary accounting.
        file_path: String,
        /// The diagnostics it showed, errors and warnings, one record each.
        errors: Vec<ParseError>,
        /// Whether the file failed, from its status: its diagnostics alone
        /// cannot say (warnings do not fail a file).
        failed: bool,
    },
    /// Mark a file complete when it showed no diagnostics.
    FileDone {
        /// Whether the file failed (unreadable, a failed roundtrip, a tool
        /// failure).
        failed: bool,
    },
    /// Stop the writer thread after all earlier commands have been processed.
    Shutdown,
}

/// Where the writer thread's output stands: writing, or failed at its first
/// write error, after which it writes nothing more (an audit file with a hole
/// in it would read as a complete one).
enum AuditOutput<W: Write> {
    /// Every record so far reached the buffer.
    Writing(W),
    /// A write failed; the file is incomplete.
    Failed(std::io::Error),
}

/// Run the dedicated audit writer loop until it receives a shutdown command.
///
/// Every command is consumed even after a write failure, so the workers
/// sending to the bounded channel never block; the failure is returned at
/// the end, so the run that wrote an incomplete audit file fails.
fn run_audit_writer<W: Write>(
    writer: W,
    receiver: Receiver<AuditCommand>,
) -> std::io::Result<AuditStats> {
    let mut stats = AuditStats::new();
    let mut output = AuditOutput::Writing(writer);

    for command in receiver {
        match command {
            AuditCommand::FileResults {
                file_path,
                errors,
                failed,
            } => {
                for error in errors {
                    stats.record(&file_path, &error);
                    output = match output {
                        AuditOutput::Writing(mut writer) => {
                            match write_error_record(&mut writer, &file_path, &error) {
                                Ok(()) => AuditOutput::Writing(writer),
                                Err(failure) => AuditOutput::Failed(failure),
                            }
                        }
                        AuditOutput::Failed(failure) => AuditOutput::Failed(failure),
                    };
                }
                stats.mark_file_processed(failed);
            }
            AuditCommand::FileDone { failed } => {
                stats.mark_file_processed(failed);
            }
            AuditCommand::Shutdown => {
                break;
            }
        }
    }

    match output {
        AuditOutput::Writing(mut writer) => {
            writer.flush()?;
            Ok(stats)
        }
        AuditOutput::Failed(failure) => Err(failure),
    }
}

/// One audit JSONL record: a single diagnostic of one file, an error or a
/// warning. The book's diagnostic contract documents this shape.
#[derive(serde::Serialize)]
struct AuditRecord<'a> {
    /// The file, as the run names it.
    file: &'a str,
    /// The diagnostic's code, e.g. `E502`.
    code: &'static str,
    /// `Error` or `Warning`, spelled as the `--format json` records spell it.
    severity: WireSeverity,
    /// The message.
    message: &'a str,
    /// 1-based line, `null` when the diagnostic has no line position.
    line: Option<usize>,
    /// 1-based column, `null` when the diagnostic has no line position.
    column: Option<usize>,
}

/// Write one JSONL audit record for a single parse or validation error.
fn write_error_record(
    writer: &mut impl Write,
    file_path: &str,
    error: &ParseError,
) -> std::io::Result<()> {
    let record = AuditRecord {
        file: file_path,
        code: error.code.as_str(),
        severity: error.severity.into(),
        message: &error.message,
        line: error.location.position().map(|p| p.line.get()),
        column: error.location.position().map(|p| p.column.get()),
    };
    serde_json::to_writer(&mut *writer, &record).map_err(std::io::Error::from)?;
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the audit reporter boundary.

    use std::collections::{HashMap, HashSet};
    use std::fs;

    use serde_json::Value;
    use talkbank_model::{DiagnosticLocation, ErrorCode, ParseError, Severity};

    use super::{AuditDestination, AuditReporter, AuditStats, SeverityCounts};

    /// Sequential reports should produce matching JSONL output and summary stats.
    #[test]
    fn finish_returns_stats_and_jsonl_output() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let output_path = temp_dir.path().join("audit.jsonl");
        let destination = AuditDestination::admit(
            &output_path,
            &talkbank_transform::paths::expand_transcript_arguments(&[]),
        )
        .expect("separate report destination");
        let (reporter, handle) =
            AuditReporter::new(destination).expect("audit reporter should be created");

        handle.report_file_results("one.cha", vec![test_error("first error", 2, 4)], true);
        handle.report_file_results(
            "two.cha",
            vec![
                test_error("second error", 5, 1),
                test_error("third error", 8, 3),
            ],
            true,
        );
        handle.mark_file_done(false);

        let stats = reporter
            .finish()
            .expect("audit reporter should finish cleanly")
            .publish()
            .expect("publish completed report");
        let output = fs::read_to_string(&output_path).expect("audit output should be readable");
        let lines: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("line should be valid JSON"))
            .collect();

        assert_eq!(
            stats,
            AuditStats {
                total_files: 3,
                files_failed: 2,
                diagnostics: SeverityCounts {
                    errors: 3,
                    warnings: 0
                },
                diagnostics_by_code: HashMap::from([(
                    ErrorCode::TestError,
                    SeverityCounts {
                        errors: 3,
                        warnings: 0
                    }
                )]),
                files_by_code: HashMap::from([(
                    ErrorCode::TestError,
                    HashSet::from([String::from("one.cha"), String::from("two.cha")]),
                )]),
            }
        );
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["file"], "one.cha");
        assert_eq!(lines[0]["severity"], "Error");
        assert_eq!(lines[1]["message"], "second error");
        assert_eq!(lines[2]["line"], 8);
        assert_eq!(lines[2]["column"], 3);
    }

    /// A valid file whose only diagnostics are warnings has its records
    /// written but did not fail, and its warning is no error: the summary's
    /// counts agree with the run's, which counts it valid, and its record
    /// says `"severity": "Warning"`, so a reader of the file alone can tell
    /// it from an error.
    #[test]
    fn a_warnings_only_file_did_not_fail_and_has_no_errors() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let output_path = temp_dir.path().join("audit.jsonl");
        let destination = AuditDestination::admit(
            &output_path,
            &talkbank_transform::paths::expand_transcript_arguments(&[]),
        )
        .expect("separate report destination");
        let (reporter, handle) =
            AuditReporter::new(destination).expect("audit reporter should be created");
        let mut warning = test_error("a warning", 1, 1);
        warning.severity = Severity::Warning;
        handle.report_file_results("valid.cha", vec![warning], false);
        let stats = reporter
            .finish()
            .expect("audit reporter should finish cleanly")
            .publish()
            .expect("publish completed report");
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.files_failed, 0);
        assert_eq!(
            stats.diagnostics,
            SeverityCounts {
                errors: 0,
                warnings: 1
            },
            "a warning is counted as a warning, and its record is still written"
        );
        let output = fs::read_to_string(&output_path).expect("audit output should be readable");
        let record: Value = serde_json::from_str(output.trim_end()).expect("one JSON record");
        assert_eq!(record["severity"], "Warning", "{record}");
    }

    /// Finishing an unused reporter should still flush a valid empty file and zero stats.
    #[test]
    fn finish_without_reports_returns_empty_stats() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let output_path = temp_dir.path().join("audit.jsonl");
        let destination = AuditDestination::admit(
            &output_path,
            &talkbank_transform::paths::expand_transcript_arguments(&[]),
        )
        .expect("separate report destination");
        let (reporter, _handle) =
            AuditReporter::new(destination).expect("audit reporter should be created");

        let stats = reporter
            .finish()
            .expect("audit reporter should finish cleanly")
            .publish()
            .expect("publish completed report");
        let output = fs::read_to_string(&output_path).expect("audit output should be readable");

        assert_eq!(stats, AuditStats::new());
        assert!(output.is_empty());
    }

    /// A write the file refuses is the writer's result, not a warning it
    /// writes past: every later command is still consumed (so senders never
    /// block), nothing more is written, and `finish` returns the failure,
    /// which fails the run.
    #[test]
    fn a_refused_write_is_the_writers_result() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let path = temp_dir.path().join("audit.jsonl");
        fs::write(&path, "").expect("create the file");
        // Opened for reading only: every write to it fails.
        let read_only = fs::File::open(&path).expect("open read-only");
        let (sender, receiver) = crossbeam_channel::unbounded();
        let records = std::iter::repeat_with(|| test_error(&"x".repeat(100), 1, 1))
            .take(200)
            .collect();
        sender
            .send(super::AuditCommand::FileResults {
                file_path: "one.cha".to_owned(),
                errors: records,
                failed: true,
            })
            .expect("send");
        sender
            .send(super::AuditCommand::FileDone { failed: false })
            .expect("send");
        sender.send(super::AuditCommand::Shutdown).expect("send");
        let outcome = super::run_audit_writer(std::io::BufWriter::new(read_only), receiver);
        assert!(outcome.is_err(), "{outcome:?}");
        assert!(fs::read_to_string(&path).expect("read").is_empty());
    }

    #[test]
    fn drop_joins_without_publishing_pending_records() {
        let temp_dir = tempfile::tempdir().expect("temporary report directory");
        let output_path = temp_dir.path().join("audit.jsonl");
        let destination = AuditDestination::admit(
            &output_path,
            &talkbank_transform::paths::expand_transcript_arguments(&[]),
        )
        .expect("separate destination");
        let (reporter, handle) = AuditReporter::new(destination).expect("writer");
        handle.report_file_results("one.cha", vec![test_error("pending", 1, 1)], true);
        drop(reporter);
        assert!(
            !output_path.exists(),
            "abandoned report must stay unpublished"
        );
    }

    #[test]
    fn aborted_run_discards_even_successfully_flushed_records() {
        use talkbank_transform::validation_runner::{AbortReason, RunEnding};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.jsonl");
        fs::write(&path, "retained report").unwrap();
        let destination = AuditDestination::admit(
            &path,
            &talkbank_transform::paths::expand_transcript_arguments(&[]),
        )
        .unwrap();
        let (reporter, handle) = AuditReporter::new(destination).unwrap();
        handle.report_file_results("one.cha", vec![test_error("pending", 1, 1)], true);
        let finished = reporter.finish().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "retained report");
        let result = finished
            .conclude(&RunEnding::Aborted(AbortReason::NoEnding))
            .unwrap();
        assert!(matches!(result, super::AuditCompletion::Withheld(_)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "retained report");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn panicked_worker_cannot_supply_success_statistics() {
        let (shutdown, receiver) = crossbeam_channel::bounded(1);
        let worker = std::thread::spawn(move || {
            drop(receiver);
            panic!("injected audit worker failure");
        });
        let reporter = AuditReporter {
            running: Some(super::RunningAudit { shutdown, worker }),
        };
        let error = reporter
            .finish()
            .err()
            .expect("worker failure is not empty success");
        assert_eq!(error.to_string(), "audit writer thread panicked");
    }

    /// Build a deterministic test error with line and column information.
    fn test_error(message: &str, line: usize, column: usize) -> ParseError {
        ParseError::new(
            ErrorCode::TestError,
            Severity::Error,
            DiagnosticLocation::known_at(
                talkbank_model::Span::new(0, 1),
                talkbank_model::LineColumn::one_based(line, column).expect("one-based position"),
            ),
            Option::<talkbank_model::ErrorContext>::None,
            message,
        )
    }
}
