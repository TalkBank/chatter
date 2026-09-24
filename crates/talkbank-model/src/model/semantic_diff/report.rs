//! Semantic-diff report storage and rendering helpers.
//!
//! `SemanticDiffReport` serves as both collector and renderer for structured
//! differences. It is designed for assertion failures, roundtrip debugging, and
//! CLI output where path-aware mismatch context is more useful than `==` failure.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::fmt;
use std::num::NonZeroUsize;

use super::context::SemanticDiffContext;
use super::path::SemanticPath;
use super::types::{DEFAULT_MAX_DIFFS, SemanticDiffKind, SemanticDifference};
use crate::Span;

/// Collect and render semantic differences between two model values.
///
/// The report keeps insertion order so the first discovered mismatch remains
/// deterministic across runs, which is important for snapshot-based tests.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct SemanticDiffReport {
    differences: Vec<SemanticDifference>,
    budget: ReportBudget,
}

/// Capacity exhaustion is not truncation until another difference is found.
/// An available slot count is positive by construction; the requested limit
/// is conserved as stored entries plus remaining slots, not stored twice.
#[derive(Debug, Clone, Copy)]
enum ReportBudget {
    Available(NonZeroUsize),
    AtCapacity,
    Truncated,
}

impl ReportBudget {
    fn remaining(slots: usize) -> Self {
        match NonZeroUsize::new(slots) {
            Some(slots) => Self::Available(slots),
            None => Self::AtCapacity,
        }
    }
}

impl SemanticDiffReport {
    /// Creates a new [`SemanticDiffReport`] that will collect at most `max_diffs` differences.
    ///
    /// Truncation is intentional: it prevents runaway diff output on deeply
    /// divergent structures while still surfacing representative failures.
    pub fn new(max_diffs: usize) -> Self {
        Self {
            differences: Vec::new(),
            budget: ReportBudget::remaining(max_diffs),
        }
    }

    /// Returns `true` if no differences were recorded.
    ///
    /// This describes stored entries, not necessarily semantic equality: a
    /// zero-capacity report can be empty and truncated while differences exist.
    /// Equality requires both an empty report and `!self.is_truncated()`.
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }

    /// Returns `true` if the report was truncated because `max_diffs` was reached.
    ///
    /// A truncated report is still valid, but consumers should avoid assuming it
    /// enumerates every mismatch.
    pub fn is_truncated(&self) -> bool {
        matches!(self.budget, ReportBudget::Truncated)
    }

    /// Returns the collected differences.
    ///
    /// The slice is ordered by discovery order during semantic traversal.
    pub fn differences(&self) -> &[SemanticDifference] {
        &self.differences
    }

    /// Record one difference unless the report is already truncated.
    ///
    /// Once `max_diffs` is reached, subsequent pushes only mark truncation and
    /// intentionally drop additional entries.
    pub fn push(
        &mut self,
        path: &SemanticPath,
        kind: SemanticDiffKind,
        left: impl Into<String>,
        right: impl Into<String>,
        span: Option<Span>,
    ) {
        let remaining = match self.budget {
            ReportBudget::Available(slots) => slots,
            ReportBudget::AtCapacity | ReportBudget::Truncated => {
                self.budget = ReportBudget::Truncated;
                return;
            }
        };

        self.differences.push(SemanticDifference {
            path: path.to_string(),
            kind,
            left: left.into(),
            right: right.into(),
            span,
        });
        self.budget = ReportBudget::remaining(remaining.get() - 1);
    }

    /// Successful pushes conserve this sum. At capacity, all slots are stored
    /// entries; a refused push changes only the truncation state.
    fn capacity(&self) -> usize {
        match self.budget {
            ReportBudget::Available(remaining) => self.differences.len() + remaining.get(),
            ReportBudget::AtCapacity | ReportBudget::Truncated => self.differences.len(),
        }
    }

    /// Records a difference using the span from the given [`SemanticDiffContext`].
    ///
    /// This helper keeps call sites concise and ensures span provenance follows
    /// the current traversal context consistently.
    pub fn push_with_context(
        &mut self,
        path: &SemanticPath,
        kind: SemanticDiffKind,
        left: impl Into<String>,
        right: impl Into<String>,
        ctx: &SemanticDiffContext,
    ) {
        self.push(path, kind, left, right, ctx.current_span());
    }

    /// Renders all differences as a multi-line plain text report.
    ///
    /// This format is optimized for terminal output and CI logs where a compact
    /// but human-readable diff overview is needed.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Semantic Diff Report\n");
        out.push_str(&format!(
            "Differences: {}{}\n",
            self.differences.len(),
            if self.is_truncated() {
                " (truncated)"
            } else {
                ""
            }
        ));

        if let Some(first) = self.differences.first() {
            out.push_str(&format!(
                "First diff: {} [{}]
  left:  {}
  right: {}\n",
                first.path,
                first.kind.as_str(),
                first.left,
                first.right
            ));
            if let Some(span) = first.span {
                out.push_str(&format!("  span:  {}..{}\n", span.start, span.end));
            }
        }

        out.push_str(
            "
Differences (first ",
        );
        out.push_str(&self.capacity().to_string());
        out.push_str("):\n");

        for (idx, diff) in self.differences.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} [{}]
   left:  {}
   right: {}\n",
                idx + 1,
                diff.path,
                diff.kind.as_str(),
                diff.left,
                diff.right
            ));
            if let Some(span) = diff.span {
                out.push_str(&format!("   span:  {}..{}\n", span.start, span.end));
            }
        }

        out
    }
}

impl Default for SemanticDiffReport {
    /// Uses the crate default diff cap (`DEFAULT_MAX_DIFFS`).
    fn default() -> Self {
        Self::new(DEFAULT_MAX_DIFFS)
    }
}

impl fmt::Display for SemanticDiffReport {
    /// Renders the summary-oriented diff text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}
