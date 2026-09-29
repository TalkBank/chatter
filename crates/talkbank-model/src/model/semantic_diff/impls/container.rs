//! `SemanticDiff` implementations for container and wrapper types.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! Design note: sequence containers intentionally prefer element-level diffs
//! before length-only diffs. This keeps reports focused on the first semantic
//! divergence instead of flooding callers with cascading size noise.

use indexmap::IndexMap;
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::sync::Arc;

use crate::model::semantic_diff::{
    SemanticDiff, SemanticDiffContext, SemanticDiffKind, SemanticDiffReport, SemanticPath,
};

// =============================================================================
// Option
// =============================================================================

impl<T: SemanticDiff> SemanticDiff for Option<T> {
    /// Compares `Some` payloads recursively and reports `Some/None` shape mismatches.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        match (self, other) {
            (Some(left), Some(right)) => left.semantic_diff_into(right, path, report, ctx),
            (None, None) => {}
            (Some(_), None) => {
                report.push_with_context(path, SemanticDiffKind::ValueMismatch, "Some", "None", ctx)
            }
            (None, Some(_)) => {
                report.push_with_context(path, SemanticDiffKind::ValueMismatch, "None", "Some", ctx)
            }
        }
    }
}

// =============================================================================
// Vec
// =============================================================================

/// Sequence storage does not change comparison policy. Borrowed slices bind
/// element traversal and tail length to the same two containers.
fn diff_sequence<T: SemanticDiff>(
    left: &[T],
    right: &[T],
    path: &mut SemanticPath,
    report: &mut SemanticDiffReport,
    ctx: &mut SemanticDiffContext,
) {
    for (idx, (left, right)) in left.iter().zip(right).enumerate() {
        if report.is_truncated() {
            return;
        }
        path.push_index(idx);
        left.semantic_diff_into(right, path, report, ctx);
        path.pop();
    }
    if report.is_truncated() {
        return;
    }
    // The ordering selects the shorter sequence's length, which is exactly
    // the first unmatched index. Equal lengths have no tail to report.
    let (index, kind, left_value, right_value) = match left.len().cmp(&right.len()) {
        Ordering::Equal => return,
        Ordering::Greater => (
            right.len(),
            SemanticDiffKind::ExtraKey,
            format!("item at [{}]", right.len()),
            "missing".to_owned(),
        ),
        Ordering::Less => (
            left.len(),
            SemanticDiffKind::MissingKey,
            "missing".to_owned(),
            format!("item at [{}]", left.len()),
        ),
    };
    path.push_index(index);
    report.push_with_context(path, kind, left_value, right_value, ctx);
    path.pop();
}

impl<T: SemanticDiff> SemanticDiff for Vec<T> {
    /// Diffs vectors by shared prefix first, then reports extra/missing tail.
    ///
    /// This ordering makes diagnostics point at the first true divergence
    /// rather than always emitting a top-level length mismatch.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        diff_sequence(self.as_slice(), other.as_slice(), path, report, ctx);
    }
}

// =============================================================================
// SmallVec
// =============================================================================

impl<A: smallvec::Array> SemanticDiff for SmallVec<A>
where
    A::Item: SemanticDiff,
{
    /// Mirrors `Vec<T>` diff semantics for `SmallVec` containers.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        diff_sequence(self.as_slice(), other.as_slice(), path, report, ctx);
    }
}

// =============================================================================
// Box
// =============================================================================

impl<T: SemanticDiff> SemanticDiff for Box<T> {
    /// Delegates to the boxed value.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        (**self).semantic_diff_into(&**other, path, report, ctx);
    }
}

// =============================================================================
// Arc
// =============================================================================

impl<T: SemanticDiff + ?Sized> SemanticDiff for Arc<T> {
    /// Delegates to the shared value instead of pointer identity.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        (**self).semantic_diff_into(&**other, path, report, ctx);
    }
}

// =============================================================================
// Tuples
// =============================================================================

impl<A: SemanticDiff, B: SemanticDiff> SemanticDiff for (A, B) {
    /// Diffs tuple fields by index order (`0`, then `1`).
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if report.is_truncated() {
            return;
        }
        path.push_index(0);
        self.0.semantic_diff_into(&other.0, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(1);
        self.1.semantic_diff_into(&other.1, path, report, ctx);
        path.pop();
    }
}

impl<A: SemanticDiff, B: SemanticDiff, C: SemanticDiff> SemanticDiff for (A, B, C) {
    /// Diffs tuple fields by index order (`0`, `1`, then `2`).
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if report.is_truncated() {
            return;
        }
        path.push_index(0);
        self.0.semantic_diff_into(&other.0, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(1);
        self.1.semantic_diff_into(&other.1, path, report, ctx);
        path.pop();
        if report.is_truncated() {
            return;
        }
        path.push_index(2);
        self.2.semantic_diff_into(&other.2, path, report, ctx);
        path.pop();
    }
}

// =============================================================================
// IndexMap
// =============================================================================

impl<K: SemanticDiff, V: SemanticDiff> SemanticDiff for IndexMap<K, V> {
    /// Diffs ordered map entries while preserving insertion-order semantics.
    fn semantic_diff_into(
        &self,
        other: &Self,
        path: &mut SemanticPath,
        report: &mut SemanticDiffReport,
        ctx: &mut SemanticDiffContext,
    ) {
        if self.len() != other.len() {
            report.push_with_context(
                path,
                SemanticDiffKind::LengthMismatch,
                format!("len={}", self.len()),
                format!("len={}", other.len()),
                ctx,
            );
        }

        // The iterator supplies two existing borrowed entries per step; there
        // is no index lookup that can spuriously stop a complete comparison.
        for (idx, ((left_key, left_value), (right_key, right_value))) in
            self.iter().zip(other.iter()).enumerate()
        {
            if report.is_truncated() {
                return;
            }
            path.push_index(idx);
            path.push_field("key");
            left_key.semantic_diff_into(right_key, path, report, ctx);
            path.pop();
            if report.is_truncated() {
                path.pop();
                return;
            }
            path.push_field("value");
            left_value.semantic_diff_into(right_value, path, report, ctx);
            path.pop();
            path.pop();
        }
    }
}
