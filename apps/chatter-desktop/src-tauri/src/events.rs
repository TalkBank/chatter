//! Bridge between `talkbank_transform::ValidationEvent` and Tauri frontend events.
//!
//! All structs use `#[serde(rename_all = "camelCase")]` so the JSON matches
//! the TypeScript types while Rust stays idiomatic snake_case.

use serde::Serialize;
use std::path::Path;
use talkbank_model::ParseError;
use talkbank_transform::validation_runner::{
    FileStatus, RunEnding, ValidationEvent, ValidationStatsSnapshot,
};
use talkbank_transform::{RenderMode, render_diagnostics};

/// A parse diagnostic paired with its pre-rendered miette HTML.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendDiagnostic {
    pub error: ParseError,
    pub rendered_html: String,
    /// Plain text rendering (no ANSI codes) for clipboard copy.
    pub rendered_text: String,
}

/// Serializable event sent to the frontend via `app_handle.emit()`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrontendEvent {
    /// The validation cache would not open; the run goes on without it.
    /// Sent before the run's own events.
    CacheUnavailable {
        /// Why, as a sentence for a person.
        reason: String,
    },

    Discovering,

    #[serde(rename_all = "camelCase")]
    Started {
        total_files: usize,
    },

    Errors {
        file: String,
        diagnostics: Vec<FrontendDiagnostic>,
        source: String,
    },

    #[serde(rename_all = "camelCase")]
    FileComplete {
        file: String,
        status: FrontendFileStatus,
    },

    /// The run ended abnormally: the validator stopped without finishing.
    ///
    /// Distinct from `Finished` because the two demand different things of the
    /// UI, and merging them is how a dead run gets reported as a clean one
    /// with empty results. Carries a reason the user can quote in a report.
    #[serde(rename_all = "camelCase")]
    Aborted {
        /// Human-readable explanation, safe to show verbatim.
        reason: String,
    },
    /// The run reached its end but did NOT cover every file it discovered:
    /// files were abandoned and never examined.
    ///
    /// Separate from `Finished` so that no "all files valid" claim can be
    /// reached from it. `stats` describes only what was processed, and the
    /// frontend's all-valid gate is reachable from `Finished` alone.
    #[serde(rename_all = "camelCase")]
    FinishedIncomplete {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: FrontendStats,
        /// Files discovered but never accounted for. Always non-zero.
        lost_files: usize,
        /// Why they were lost, as the runner words it.
        cause: String,
    },
    /// The run was told to stop (the user's Cancel) and left files
    /// unchecked. Its own event, never a `cancelled` flag on `Finished`, so
    /// the all-valid gate cannot be reached from a stopped run.
    #[serde(rename_all = "camelCase")]
    Stopped {
        /// Totals for the files that WERE processed. Not totals for the input.
        stats: FrontendStats,
        /// Files discovered and never checked. Always non-zero.
        unprocessed_files: usize,
        /// Why it stopped, safe to show verbatim.
        reason: String,
    },
    /// Every discovered file was accounted for. `passed` is the runner's
    /// own verdict ([`RunEnding::passed`]), the one the CLI's exit status
    /// reads: the frontend's all-valid claim is this field, not a rule of
    /// its own over the counts.
    Finished {
        stats: FrontendStats,
        passed: bool,
    },
    /// The target held no CHAT transcript: nothing was validated, so there
    /// are no counts and no claim.
    NothingFound,
}

/// Serializable version of `FileStatus` for the frontend.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrontendFileStatus {
    #[serde(rename_all = "camelCase")]
    Valid { cache_hit: bool },

    #[serde(rename_all = "camelCase")]
    Invalid { error_count: usize, cache_hit: bool },

    #[serde(rename_all = "camelCase")]
    RoundtripFailed { cache_hit: bool, reason: String },

    #[serde(rename_all = "camelCase")]
    InternalFailure { message: String },

    #[serde(rename_all = "camelCase")]
    ReadError { message: String },
}

/// Serializable version of `ValidationStatsSnapshot` for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendStats {
    pub total_files: usize,
    pub valid_files: usize,
    pub invalid_files: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    /// Cache reads or writes that failed; those files were validated
    /// without the cache.
    pub cache_errors: usize,
    pub internal_failures: usize,
    pub roundtrip_passed: usize,
    pub roundtrip_failed: usize,
}

/// The frontend events for one runner event: at most two, because one file's
/// result is the frontend's `errors` event (when it showed diagnostics) and
/// then its `fileComplete` event. Every other runner event is one frontend
/// event.
pub fn to_frontend_events(event: ValidationEvent) -> [Option<FrontendEvent>; 2] {
    match event {
        ValidationEvent::Discovering => [Some(FrontendEvent::Discovering), None],
        ValidationEvent::Started { total_files } => {
            [Some(FrontendEvent::Started { total_files }), None]
        }
        ValidationEvent::FileComplete(e) => {
            let file = path_string(&e.path);
            let errors = e
                .status
                .shown()
                .map(|shown| frontend_errors(file.clone(), shown));
            [
                errors,
                Some(FrontendEvent::FileComplete {
                    file,
                    status: convert_status(e.status, e.cache),
                }),
            ]
        }
        ValidationEvent::Finished(ending) => [Some(frontend_ending(ending)), None],
    }
}

/// The `errors` event for a file's diagnostics, rendered through the SAME
/// shared orchestration the CLI uses, so the GUI cannot diverge from the CLI
/// on how an error is enhanced or which source line the caret lands on.
/// `Ansi` mode yields both the plain text (for "Copy") and the colored form
/// (converted to HTML).
fn frontend_errors(
    file: String,
    shown: talkbank_transform::validation_runner::ShownDiagnostics<'_>,
) -> FrontendEvent {
    let source = shown.source.to_owned();
    let diagnostics = render_diagnostics(shown.errors, &file, &source, RenderMode::Ansi)
        .into_iter()
        .map(|d| {
            let rendered_html = match &d.ansi {
                Some(ansi) => ansi_to_html(ansi),
                // RenderMode::Ansi always populates `ansi`; escape the plain
                // text rather than panic or drop the diagnostic if that
                // invariant ever changes.
                None => ansi_to_html(&d.text),
            };
            FrontendDiagnostic {
                // The ENHANCED error (line/column populated), for the
                // frontend and Open-in-CLAN.
                error: d.error,
                rendered_html,
                rendered_text: d.text,
            }
        })
        .collect();
    FrontendEvent::Errors {
        file,
        diagnostics,
        source,
    }
}

/// The frontend event for how a run ended. Every sentence is the runner's
/// own `Display`, so the desktop and the CLI word a stop, a loss and an
/// abort the same way.
fn frontend_ending(ending: RunEnding) -> FrontendEvent {
    let passed = ending.passed();
    match ending {
        RunEnding::Complete(stats) => FrontendEvent::Finished {
            stats: convert_stats(stats.snapshot()),
            passed,
        },
        RunEnding::NothingFound => FrontendEvent::NothingFound,
        RunEnding::Stopped { stats, reason } => FrontendEvent::Stopped {
            stats: convert_stats(stats.snapshot()),
            unprocessed_files: stats.missing_files().get(),
            reason: reason.to_string(),
        },
        RunEnding::Incomplete { stats, cause } => FrontendEvent::FinishedIncomplete {
            stats: convert_stats(stats.snapshot()),
            lost_files: stats.missing_files().get(),
            cause: cause.to_string(),
        },
        RunEnding::Aborted(reason) => FrontendEvent::Aborted {
            reason: reason.to_string(),
        },
    }
}

/// The wire status. `cacheHit` stays a boolean on the wire (a hit or not);
/// it is read off the event's `CacheUse`, so a run with no cache reports
/// no hit without claiming a miss.
fn convert_status(status: FileStatus, cache: talkbank_transform::CacheUse) -> FrontendFileStatus {
    let cache_hit = matches!(cache, talkbank_transform::CacheUse::Hit);
    match status {
        FileStatus::InternalFailure { failure, .. } => FrontendFileStatus::InternalFailure {
            message: failure.to_string(),
        },
        FileStatus::Valid { .. } => FrontendFileStatus::Valid { cache_hit },
        FileStatus::Invalid { diagnostics } => FrontendFileStatus::Invalid {
            error_count: diagnostics.error_count().get(),
            cache_hit,
        },
        // The desktop shows a roundtrip failure's reason, not its diff.
        FileStatus::RoundtripFailed { reason, .. } => {
            FrontendFileStatus::RoundtripFailed { cache_hit, reason }
        }
        FileStatus::ReadError { message } => FrontendFileStatus::ReadError { message },
    }
}

fn convert_stats(stats: &ValidationStatsSnapshot) -> FrontendStats {
    FrontendStats {
        total_files: stats.total_files().get(),
        valid_files: stats.valid_files(),
        invalid_files: stats.invalid_files(),
        cache_hits: stats.cache_hits(),
        cache_misses: stats.cache_misses(),
        cache_errors: stats.cache_errors(),
        internal_failures: stats.internal_failures(),
        roundtrip_passed: stats.roundtrip_passed(),
        roundtrip_failed: stats.roundtrip_failed(),
    }
}

/// A path as the frontend shows and keys it.
fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Convert ANSI escape codes to HTML `<span>` elements using the `ansi-to-html` crate.
fn ansi_to_html(input: &str) -> String {
    ansi_to_html::convert(input).unwrap_or_else(|_| html_escape(input))
}

/// Fallback HTML escaping if ANSI conversion fails.
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
