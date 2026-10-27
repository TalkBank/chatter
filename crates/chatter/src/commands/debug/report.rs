//! Shared exclusive-creation and completion states for exploratory audit exports.

use std::io::Write;
use std::path::{Path, PathBuf};

/// I/O prevented a complete audit; a newly created report may be partial.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// A selected input could not be read as UTF-8 during processing.
    #[error("cannot read audit input {path}: {source}")]
    Read {
        /// Input that could not be read.
        path: PathBuf,
        /// Underlying read failure.
        source: std::io::Error,
    },
    /// Report creation, writing or final flushing failed.
    #[error("cannot complete audit report {path}: {source}; any new partial report is retained")]
    Report {
        /// Selected report destination.
        path: PathBuf,
        /// Underlying output failure.
        source: std::io::Error,
    },
}

impl From<AuditError> for crate::exit_codes::CommandStop {
    fn from(error: AuditError) -> Self {
        super::die(&error.to_string())
    }
}

/// A writable report. Each successful transition retains sole ownership.
pub(super) struct OpenReport<W> {
    path: PathBuf,
    writer: W,
}

/// Only successful explicit flushing produces a completion receipt.
pub(super) struct FinishedReport(PathBuf);

impl FinishedReport {
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl OpenReport<std::io::BufWriter<std::fs::File>> {
    pub(super) fn create(path: &Path) -> Result<Self, AuditError> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| AuditError::Report {
                path: path.to_owned(),
                source,
            })?;
        Ok(Self {
            path: path.to_owned(),
            writer: std::io::BufWriter::new(file),
        })
    }
}

impl<W: Write> OpenReport<W> {
    pub(super) fn record(mut self, record: &serde_json::Value) -> Result<Self, AuditError> {
        writeln!(self.writer, "{record}").map_err(|source| AuditError::Report {
            path: self.path.clone(),
            source,
        })?;
        Ok(self)
    }

    pub(super) fn finish(mut self) -> Result<FinishedReport, AuditError> {
        self.writer.flush().map_err(|source| AuditError::Report {
            path: self.path.clone(),
            source,
        })?;
        Ok(FinishedReport(self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum FailurePoint {
        Write,
        Flush,
    }

    struct FailingWriter(FailurePoint);

    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            match self.0 {
                FailurePoint::Write => Err(std::io::Error::other("injected record failure")),
                FailurePoint::Flush => Ok(bytes.len()),
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("injected flush failure"))
        }
    }

    #[test]
    fn audit_report_failed_record_cannot_finish() {
        let report = OpenReport {
            path: PathBuf::from("report.jsonl"),
            writer: FailingWriter(FailurePoint::Write),
        };
        assert!(report.record(&serde_json::json!({"groups": 1})).is_err());
    }

    #[test]
    fn audit_report_failed_flush_has_no_receipt() {
        let report = OpenReport {
            path: PathBuf::from("report.jsonl"),
            writer: FailingWriter(FailurePoint::Flush),
        };
        let report = report.record(&serde_json::json!({"groups": 1})).unwrap();
        assert!(report.finish().is_err());
    }

    #[test]
    fn audit_report_buffered_write_failure_has_no_receipt() {
        let report = OpenReport {
            path: PathBuf::from("report.jsonl"),
            writer: std::io::BufWriter::new(FailingWriter(FailurePoint::Write)),
        };
        // Small records are accepted into the buffer; the underlying write
        // fails only when completion flushes it.
        let report = report.record(&serde_json::json!({"groups": 1})).unwrap();
        assert!(report.finish().is_err());
    }
}
