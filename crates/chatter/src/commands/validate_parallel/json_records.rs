//! The `chatter validate --format json` wire records: one typed model,
//! serialized once where a record is written.
//!
//! Every record is a [`JsonRecord`], tagged by `type`; a record with
//! variants carries its own second tag (`status`, `reason`, `action`,
//! `notice`). The model is what the diagnostic contract documents, so a
//! field cannot be added, renamed or spelled differently in one place and
//! not another, and no value on the wire is a `Debug` rendering.

use std::borrow::Cow;
use std::path::Path;

use serde::Serialize;

/// One JSONL record.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum JsonRecord<'a> {
    /// A file's outcome (`status` says which).
    File(FileRecord<'a>),
    /// The run's totals, last.
    Summary(SummaryRecord<'a>),
    /// The run stopped with files left (`reason` says why); before the
    /// summary.
    Stop(StopRecord),
    /// The run lost files nobody asked to skip; before the summary.
    Incomplete {
        /// Files never accounted for.
        lost_files: usize,
        /// Files discovered.
        total_files: usize,
        /// What kind of loss it was.
        cause: WireLossCause,
        /// The cause as a sentence for a person.
        detail: String,
    },
    /// The run died before producing totals; replaces the summary.
    Aborted {
        /// Why, as a sentence for a person.
        reason: String,
    },
    /// What cache maintenance did (`action` says what).
    Cache(CacheRecord<'a>),
    /// A fact about the run, said before it starts (`notice` says which).
    Notice(NoticeRecord<'a>),
}

/// A file record, tagged by `status`.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum FileRecord<'a> {
    /// No error: the file is valid, with any warnings it showed.
    Valid {
        /// The file.
        file: Cow<'a, str>,
        /// Whether the verdict came from the cache.
        cache_hit: bool,
        /// Its warnings, absent when there are none.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<WireDiagnostic<'a>>,
    },
    /// Errors were found.
    Invalid {
        /// The file.
        file: Cow<'a, str>,
        /// How many diagnostics of severity `Error`; warnings not counted.
        error_count: usize,
        /// Every diagnostic shown, errors and warnings, each with its
        /// severity.
        errors: Vec<WireDiagnostic<'a>>,
        /// Present when structural errors may have kept other checks from
        /// running.
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<&'static str>,
    },
    /// The file validated but did not write back byte for byte.
    RoundtripFailed {
        /// The file.
        file: Cow<'a, str>,
        /// Why.
        reason: &'a str,
        /// The difference, when it was computed (not for a cached verdict).
        diff: Option<&'a str>,
        /// The warnings it showed, absent when there are none.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<WireDiagnostic<'a>>,
    },
    /// The tool failed on this file; its validity is undetermined.
    InternalFailure {
        /// The file.
        file: Cow<'a, str>,
        /// The failure.
        error: String,
        /// The diagnostics of the failed attempt.
        errors: Vec<WireDiagnostic<'a>>,
    },
    /// The file could not be read.
    ReadError {
        /// The file.
        file: Cow<'a, str>,
        /// Why.
        error: &'a str,
    },
}

/// A diagnostic as the JSON records carry it.
#[derive(Debug, Serialize)]
pub(super) struct WireDiagnostic<'a> {
    /// The error code, e.g. `E316`.
    code: &'static str,
    /// `Error` or `Warning`.
    severity: WireSeverity,
    /// The message.
    message: &'a str,
}

/// A diagnostic's severity on the wire: `Error` or `Warning`, spelled as the
/// contract says, not borrowed from a `Debug` rendering. The one spelling for
/// every wire that carries a severity: these records and each `--audit`
/// JSONL record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum WireSeverity {
    /// `Severity::Error`.
    Error,
    /// `Severity::Warning`.
    Warning,
}

impl From<talkbank_model::Severity> for WireSeverity {
    fn from(severity: talkbank_model::Severity) -> Self {
        match severity {
            talkbank_model::Severity::Error => Self::Error,
            talkbank_model::Severity::Warning => Self::Warning,
        }
    }
}

impl<'a> WireDiagnostic<'a> {
    /// The record form of `error`.
    pub(super) fn of(error: &'a talkbank_model::ParseError) -> Self {
        Self {
            code: error.code.as_str(),
            severity: error.severity.into(),
            message: &error.message,
        }
    }
}

/// The summary record.
#[derive(Debug, Serialize)]
pub(super) struct SummaryRecord<'a> {
    /// The run's label (its first argument).
    pub(super) directory: Cow<'a, str>,
    /// How the run ended.
    pub(super) outcome: SummaryOutcome,
    /// The counts, absent for a run that found nothing to validate.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(super) counts: Option<SummaryCounts>,
}

/// A summary's counts.
#[derive(Debug, Serialize)]
pub(super) struct SummaryCounts {
    /// Files discovered.
    pub(super) total_files: usize,
    /// Files with no diagnostic.
    pub(super) valid: usize,
    /// Invalid or unreadable files.
    pub(super) invalid: usize,
    /// Files the tool failed on.
    pub(super) internal_failures: usize,
    /// Verdicts served from the cache.
    pub(super) cache_hits: usize,
    /// Cache consultations with no usable verdict.
    pub(super) cache_misses: usize,
    /// Hits over consultations, a percentage; `null` when nothing consulted
    /// the cache.
    pub(super) cache_hit_rate: Option<f64>,
    /// Cache reads or writes that failed.
    pub(super) cache_errors: usize,
    /// Roundtrip totals, present when `--roundtrip` was set.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(super) roundtrip: Option<RoundtripTotals>,
}

/// How a summarized run ended.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SummaryOutcome {
    /// Every discovered file was accounted for.
    Complete,
    /// The input named no transcript; the summary has no counts.
    NothingFound,
    /// A stop record precedes the summary.
    Stopped,
    /// An incomplete record precedes the summary.
    Incomplete,
}

/// The summary's roundtrip totals.
#[derive(Debug, Serialize)]
pub(super) struct RoundtripTotals {
    /// Files whose roundtrip passed.
    pub(super) roundtrip_passed: usize,
    /// Files whose roundtrip failed.
    pub(super) roundtrip_failed: usize,
}

/// A stop record, tagged by `reason`.
#[derive(Debug, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub(super) enum StopRecord {
    /// The errors found reached `--max-errors`.
    MaxErrors {
        /// The limit.
        limit: usize,
        /// Files never validated.
        unprocessed_files: usize,
    },
    /// The run was cancelled.
    Cancelled {
        /// Files never validated.
        unprocessed_files: usize,
    },
}

/// A cache record, tagged by `action`.
#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum CacheRecord<'a> {
    /// Unreachable rows were reclaimed on open.
    Prune {
        /// Rows deleted.
        rows_deleted: u64,
        /// Distinct versions they belonged to.
        versions_deleted: usize,
    },
    /// `--force` cleared the run's rows.
    Clear {
        /// Rows cleared.
        entries_cleared: usize,
    },
    /// A cache operation failed; the run continued without the cache.
    Warning {
        /// Which operation.
        operation: crate::commands::validate::cache::CacheOperation,
        /// Why, as a sentence for a person.
        error: &'a str,
    },
}

/// An incomplete record's `cause`: the kind of a run's loss, from the
/// runner's `LossCause`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WireLossCause {
    /// Workers failed; `detail` lists each failure.
    WorkerFaults,
    /// No worker failed and no stop was asked for: a validator defect.
    Unexplained,
}

impl WireLossCause {
    /// The wire kind of `cause`.
    pub(super) fn of(cause: &talkbank_transform::validation_runner::LossCause) -> Self {
        use talkbank_transform::validation_runner::LossCause;
        match cause {
            LossCause::WorkerFaults(_) => Self::WorkerFaults,
            LossCause::Unexplained => Self::Unexplained,
        }
    }
}

/// A notice record, tagged by `notice`.
#[derive(Debug, Serialize)]
#[serde(tag = "notice", rename_all = "snake_case")]
pub(super) enum NoticeRecord<'a> {
    /// These codes are suppressed.
    Suppressing {
        /// The codes, sorted.
        codes: Vec<&'static str>,
    },
    /// A deprecated flag was passed.
    DeprecatedFlag {
        /// The flag.
        flag: &'static str,
    },
    /// The Ctrl-C handler could not be installed.
    InterruptUnavailable {
        /// Why, as a sentence for a person.
        error: &'a str,
    },
}

/// A path as the records carry it.
pub(super) fn wire_path(path: &Path) -> Cow<'_, str> {
    path.to_string_lossy()
}

impl JsonRecord<'_> {
    /// Write the record as one line to `out`, serialized straight into it
    /// (no intermediate string). The records have no non-string map key, so
    /// serialization cannot fail; a write can, and is returned.
    pub(super) fn write_line(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        serde_json::to_writer(&mut *out, self).map_err(std::io::Error::from)?;
        out.write_all(b"\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(record: &JsonRecord<'_>) -> serde_json::Value {
        serde_json::to_value(record).expect("records serialize")
    }

    /// The nested tags land in one flat object, as the contract shows.
    #[test]
    fn records_flatten_their_tags_into_one_object() {
        assert_eq!(
            value(&JsonRecord::File(FileRecord::Valid {
                file: Cow::Borrowed("a.cha"),
                cache_hit: true,
                warnings: Vec::new(),
            })),
            serde_json::json!({"type": "file", "status": "valid", "file": "a.cha", "cache_hit": true})
        );
        assert_eq!(
            value(&JsonRecord::Stop(StopRecord::MaxErrors {
                limit: 5,
                unprocessed_files: 2,
            })),
            serde_json::json!({"type": "stop", "reason": "max_errors", "limit": 5, "unprocessed_files": 2})
        );
        assert_eq!(
            value(&JsonRecord::Cache(CacheRecord::Warning {
                operation: crate::commands::validate::cache::CacheOperation::Initialize,
                error: "locked",
            })),
            serde_json::json!({"type": "cache", "action": "warning", "operation": "initialize", "error": "locked"})
        );
        assert_eq!(
            value(&JsonRecord::Notice(NoticeRecord::DeprecatedFlag {
                flag: "--check-xphon"
            })),
            serde_json::json!({"type": "notice", "notice": "deprecated_flag", "flag": "--check-xphon"})
        );
    }

    /// The roundtrip totals appear only when present, a hit rate with
    /// nothing consulted is `null`, and a run that found nothing has an
    /// outcome and no counts.
    #[test]
    fn the_summary_carries_counts_and_roundtrip_totals_only_when_present() {
        let summary = |roundtrip| {
            value(&JsonRecord::Summary(SummaryRecord {
                directory: Cow::Borrowed("corpus"),
                outcome: SummaryOutcome::Complete,
                counts: Some(SummaryCounts {
                    total_files: 1,
                    valid: 1,
                    invalid: 0,
                    internal_failures: 0,
                    cache_hits: 0,
                    cache_misses: 0,
                    cache_hit_rate: None,
                    cache_errors: 0,
                    roundtrip,
                }),
            }))
        };
        let plain = summary(None);
        assert_eq!(plain["type"], "summary");
        assert_eq!(plain["outcome"], "complete");
        assert_eq!(plain["cache_hit_rate"], serde_json::Value::Null);
        assert!(plain.get("roundtrip_passed").is_none());
        let checked = summary(Some(RoundtripTotals {
            roundtrip_passed: 1,
            roundtrip_failed: 0,
        }));
        assert_eq!(checked["roundtrip_passed"], 1);
        assert_eq!(checked["roundtrip_failed"], 0);
        assert_eq!(
            value(&JsonRecord::Summary(SummaryRecord {
                directory: Cow::Borrowed("empty"),
                outcome: SummaryOutcome::NothingFound,
                counts: None,
            })),
            serde_json::json!({"type": "summary", "directory": "empty", "outcome": "nothing_found"})
        );
    }
}
