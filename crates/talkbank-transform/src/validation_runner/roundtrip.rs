//! Shared roundtrip testing logic.
//!
//! The roundtrip test verifies serialization idempotency:
//!   1. Parse original → serialize → text_A
//!   2. Parse text_A → serialize → text_B
//!   3. If text_A == text_B, the roundtrip passes
//!
//! This approach is robust to tier materialization: since both passes go
//! through the same pipeline, any normalization is transparent.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorCollector;
use talkbank_model::{ChatFile, WriteChat};

use super::worker::ParserDispatch;

/// Result of a roundtrip test on a single file.
///
/// Until 2026-09-09 this was `{ passed: bool, failure_reason: Option<String>,
/// diff: Option<String> }`, three fields whose agreement nothing held: a
/// failure without a reason was representable, and the runner invented one
/// ("Roundtrip failed") for it. A failure carries its reason now.
#[derive(Debug)]
pub enum RoundtripResult {
    /// Serialization is idempotent.
    Passed,
    /// It is not, or could not be tried.
    Failed(RoundtripFailure),
}

/// Why a roundtrip failed.
#[derive(Debug)]
pub enum RoundtripFailure {
    /// The model would not serialize on the given pass.
    Serialization {
        /// Which of the two passes refused.
        pass: SerializationPass,
        /// The writer's own error.
        error: String,
    },
    /// The two passes serialized differently.
    Mismatch {
        /// The first few differing lines, from [`build_text_diff`].
        diff: String,
    },
}

/// The two serializations a roundtrip makes.
#[derive(Debug, Clone, Copy)]
pub enum SerializationPass {
    /// The already-parsed file, written.
    First,
    /// The re-parse of that text, written again.
    Second,
}

impl RoundtripFailure {
    /// The failure as one line for a report.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::Serialization { pass, error } => {
                let which = match pass {
                    SerializationPass::First => 1,
                    SerializationPass::Second => 2,
                };
                format!("Serialization failed (pass {which}): {error}")
            }
            Self::Mismatch { .. } => {
                "Roundtrip mismatch (serialization not idempotent)".to_string()
            }
        }
    }

    /// The text diff, which only a mismatch has.
    #[must_use]
    pub fn diff(&self) -> Option<&str> {
        match self {
            Self::Serialization { .. } => None,
            Self::Mismatch { diff } => Some(diff),
        }
    }
}

/// Run roundtrip test: serialize → re-parse → serialize → compare.
///
/// Assumes validation already passed (caller checks for real errors first).
/// The `chat_file` is the already-parsed result from validation.
pub(super) fn run_roundtrip(chat_file: &ChatFile, parser: &ParserDispatch) -> RoundtripResult {
    // Pass 1: serialize the already-parsed ChatFile
    let mut serialized_a = String::new();
    if let Err(err) = chat_file.write_chat(&mut serialized_a) {
        return RoundtripResult::Failed(RoundtripFailure::Serialization {
            pass: SerializationPass::First,
            error: err.to_string(),
        });
    }

    // Pass 2: re-parse the serialized output (parse-only, skip validation,
    // roundtrip checks serialization fidelity, not content validity)
    let reparse_sink = ErrorCollector::new();
    let reparsed = parser.parse_chat_file_streaming(&serialized_a, &reparse_sink);

    // Serialize again (pass 2 output)
    let mut serialized_b = String::new();
    if let Err(err) = reparsed.write_chat(&mut serialized_b) {
        return RoundtripResult::Failed(RoundtripFailure::Serialization {
            pass: SerializationPass::Second,
            error: err.to_string(),
        });
    }

    // Compare: is serialization idempotent?
    if serialized_a == serialized_b {
        RoundtripResult::Passed
    } else {
        RoundtripResult::Failed(RoundtripFailure::Mismatch {
            diff: build_text_diff(&serialized_a, &serialized_b),
        })
    }
}

/// Build a human-readable text diff showing the first few differences
/// between two strings, line by line.
pub fn build_text_diff(text_a: &str, text_b: &str) -> String {
    let lines_a: Vec<&str> = text_a.lines().collect();
    let lines_b: Vec<&str> = text_b.lines().collect();
    let mut diffs = Vec::new();
    let max_diffs = 5;

    let max_len = lines_a.len().max(lines_b.len());
    for i in 0..max_len {
        if diffs.len() >= max_diffs {
            diffs.push(format!(
                "  ... and more (total lines: pass1={}, pass2={})",
                lines_a.len(),
                lines_b.len()
            ));
            break;
        }
        let line_a = lines_a.get(i).copied().unwrap_or("<missing>");
        let line_b = lines_b.get(i).copied().unwrap_or("<missing>");
        if line_a != line_b {
            diffs.push(format!(
                "  line {}:
    pass1: {}
    pass2: {}",
                i + 1,
                line_a,
                line_b
            ));
        }
    }

    if diffs.is_empty() {
        "no text differences found (possible trailing newline difference)".to_string()
    } else {
        diffs.join("\n")
    }
}
