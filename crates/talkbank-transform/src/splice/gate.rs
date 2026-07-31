//! The write gate: verify a spliced result before it ever reaches disk.
//!
//! [`verify_splice`] walks the GAPS between the recorded [`SpliceEdit`]s (the
//! untouched original text) and each edit's own replacement region,
//! comparing those bytes directly against `spliced`. Any byte that differs
//! outside what the edits actually asked to change is refused, with the
//! offset of the first such difference. This is the last typed check
//! between "here is a set of edits and a resulting string" and "this is
//! safe to write to a contributor's transcript".
//!
//! # Gap-walk, not re-apply-and-compare
//!
//! chatter runs over corpora of six figures of files. Rebuilding a second
//! copy of every fixed file (via [`apply_edits`](super::engine::apply_edits))
//! just to diff it against the copy already in hand is a real, avoidable
//! per-file cost at that scale, not a style question; a prior release had
//! to be pulled for a per-file cost that was quadratic at that scale. The
//! gap-walk makes no second allocation: it reads bytes directly out of
//! `original` and `spliced` using the same edit-to-span mapping
//! ([`super::engine::mapped_edit_sites`]) that
//! [`apply_edits`](super::engine::apply_edits) itself builds its output
//! from, so the two computations cannot drift apart, and `chatter fix`'s
//! post-splice re-check shares the same mapping rather than folding its
//! own copy of the cumulative-delta arithmetic.
//!
//! # This is necessary, not sufficient
//!
//! Byte identity outside the applied spans proves the splice mechanism did
//! only what it was told to do. It proves nothing about whether what it was
//! told to do was CORRECT. On 2026-05-06 a batch rewriter in this codebase
//! damaged 440 files and 679 utterances; only 5 of those were even
//! detectable by re-validating the result afterwards, because the rest were
//! structurally valid CHAT that said something different from, or nothing
//! like, what the transcript originally said. A gate that only checks "did
//! the bytes I did not touch survive" and "does the result still validate"
//! would have waved every one of those 674 undetected files straight
//! through.
//!
//! That incident is why [`super::BatchSafety`] gates the DEFAULT write set
//! in `chatter fix` (mechanical fixes only, unattended; semantic fixes
//! require a human to name the code; ambiguous fixes are never
//! batch-applied) instead of this gate, or the diagnostic re-check layered
//! on top of it in `chatter fix`, carrying the whole load. Read this module
//! as one necessary tripwire in a chain, never as proof of correctness on
//! its own.
//!
//! # The obvious way to get a gated result
//!
//! [`apply_edits_verified`] splices and verifies in one call, returning the
//! checked string. That is the intended default for anything that reaches
//! disk: prefer it over calling [`apply_edits`](super::engine::apply_edits)
//! and then this module's `verify_splice` separately, which a future
//! caller could forget to do. Reach for the two functions separately only
//! when a caller genuinely needs the raw, unverified splice (this
//! module's own tests, chiefly).

use super::engine::{MappedEdit, SpliceEdit, SpliceError, mapped_edit_sites, splice_with_sites};

/// Why a spliced result was refused before it reached disk.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    /// The spliced text differs from the original somewhere outside the
    /// spans that were deliberately replaced.
    #[error(
        "spliced output differs from the original outside the applied spans (first difference at byte {offset})"
    )]
    UnexpectedChangeOutsideSpans {
        /// Byte offset of the first unexpected difference.
        offset: u32,
    },
    /// Re-applying the recorded edits did not reproduce the spliced text at
    /// all: the engine itself refused the edit set.
    #[error("could not reproduce the spliced output from the recorded edits: {source}")]
    NotReproducible {
        /// The underlying splice failure.
        #[from]
        source: SpliceError,
    },
}

/// Verify that `spliced` is exactly what applying `applied` to `original`
/// produces, and nothing else.
///
/// This is a re-derivation, not a trust of the caller's bookkeeping: the
/// edit-to-span mapping is recomputed from `original` and `applied` from
/// scratch (see the module docs for why that mapping is walked directly
/// rather than materialized into a second copy of the file), and every
/// byte it implies is compared against `spliced`. A caller that assembled
/// `spliced` some other way (a bug in the pipeline that produced it, a
/// future refactor that separates "compute the edits" from "compute the
/// text" and lets the two drift) is caught here rather than trusted.
///
/// See the module docs for what this check does NOT prove.
///
/// A thin wrapper over [`verify_splice_mapped`] that computes the mapping
/// itself; [`apply_edits_verified`] already has the mapping in hand (from
/// [`splice_with_sites`]) and calls that entry point directly instead, so
/// a splice-then-verify never folds [`mapped_edit_sites`] twice over the
/// same inputs.
pub fn verify_splice(
    original: &str,
    spliced: &str,
    applied: &[SpliceEdit],
) -> Result<(), GateError> {
    let mapped = mapped_edit_sites(original, applied)?;
    verify_splice_mapped(original, spliced, &mapped)
}

/// The gap-walk itself, over an ALREADY-COMPUTED edit mapping.
///
/// Exists so [`apply_edits_verified`] can share the one [`MappedEdit`]
/// fold [`splice_with_sites`] already did to build `spliced`, rather than
/// [`verify_splice`] recomputing [`mapped_edit_sites`] on the identical
/// `(original, applied)` a second time: that second fold could never
/// disagree with the first (both are the same deterministic function on
/// the same inputs), so it verified nothing about the mapping itself; the
/// real value of this gap-walk is checking the ASSEMBLY loop's output
/// bytes, which sharing the mapping preserves completely. Not exported
/// beyond this crate: `mapped` must be the actual mapping the caller used
/// to build `spliced`, which only [`splice_with_sites`] and
/// [`mapped_edit_sites`] can honestly produce.
pub(crate) fn verify_splice_mapped(
    original: &str,
    spliced: &str,
    mapped: &[MappedEdit],
) -> Result<(), GateError> {
    let original_bytes = original.as_bytes();
    let spliced_bytes = spliced.as_bytes();

    let mut original_cursor: u32 = 0;
    let mut spliced_cursor: u32 = 0;

    for edit in mapped {
        // The untouched gap since the previous edit (or the start of the
        // file): must survive byte-for-byte. `original` is trusted here
        // (its bounds were already validated against these exact offsets by
        // `mapped_edit_sites`), so this slice never panics; `spliced` is
        // not, so the comparison below indexes it with bounds checks.
        let gap = &original_bytes[original_cursor as usize..edit.original().start as usize];
        if let Some(offset) = first_mismatch(gap, spliced_bytes, spliced_cursor as usize) {
            return Err(GateError::UnexpectedChangeOutsideSpans { offset });
        }

        // The edit's own region: must read back exactly the replacement
        // text that was asked for.
        let replacement = edit.replacement().as_str().as_bytes();
        if let Some(offset) =
            first_mismatch(replacement, spliced_bytes, edit.spliced().start as usize)
        {
            return Err(GateError::UnexpectedChangeOutsideSpans { offset });
        }

        original_cursor = edit.original().end;
        spliced_cursor = edit.spliced().end;
    }

    // The tail after the last edit (or the whole file, if there were none).
    let tail = &original_bytes[original_cursor as usize..];
    if let Some(offset) = first_mismatch(tail, spliced_bytes, spliced_cursor as usize) {
        return Err(GateError::UnexpectedChangeOutsideSpans { offset });
    }

    // `first_mismatch` only walks the bytes `spliced` was expected to have;
    // it cannot see a spurious tail `spliced` has beyond that. Catch that
    // case with an explicit length check.
    let expected_len = spliced_cursor as usize + tail.len();
    if spliced_bytes.len() != expected_len {
        return Err(GateError::UnexpectedChangeOutsideSpans {
            offset: expected_len as u32,
        });
    }

    Ok(())
}

/// Splice `edits` into `source` and verify the result before returning it.
///
/// The single ergonomic way to get gated splice output: see the module
/// docs' "obvious way to get a gated result" section. Built on
/// [`splice_with_sites`] rather than [`apply_edits`](super::engine::apply_edits)
/// plus [`verify_splice`]: the two functions together would fold
/// [`mapped_edit_sites`] twice over the identical `(source, edits)`, once
/// inside `apply_edits` and again inside `verify_splice`, which cannot
/// catch anything a single fold does not (see [`verify_splice_mapped`]'s
/// docs). This computes the mapping once and hands it to both the
/// assembly step and the gap-walk.
pub fn apply_edits_verified(source: &str, edits: &[SpliceEdit]) -> Result<String, GateError> {
    let (spliced, mapped) = splice_with_sites(source, edits)?;
    verify_splice_mapped(source, &spliced, &mapped)?;
    Ok(spliced)
}

/// Compare `expected` byte-for-byte against `spliced` starting at
/// `spliced_at`. Returns the offset (in `spliced`-text coordinates) of the
/// first mismatching byte, or `None` if `spliced` supplies exactly
/// `expected` at that position.
///
/// `expected` is always a slice already safely in hand (a gap of the
/// original source, or an edit's replacement text), so it is indexed
/// directly. `spliced` is untrusted (it may be truncated or tampered), so
/// it is read through `[u8]::get` rather than sliced, and running out of
/// bytes counts as a mismatch at the point it ran out rather than a panic.
fn first_mismatch(expected: &[u8], spliced: &[u8], spliced_at: usize) -> Option<u32> {
    for (i, &want) in expected.iter().enumerate() {
        match spliced.get(spliced_at + i) {
            Some(&got) if got == want => continue,
            _ => return Some((spliced_at + i) as u32),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::errors::Span;

    use super::super::engine::{EditProvenance, EditTarget, Replacement, TransformName};

    /// Builds the single `xx` (6..8) -> `xxx` replacement edit these
    /// fixtures share, against `"*CHI:\txx and yy .\n"`.
    fn xx_to_xxx_edit() -> SpliceEdit {
        SpliceEdit::new(
            EditTarget::Replace(Span::new(6, 8)),
            Replacement::new("xxx"),
            EditProvenance::Transform(TransformName::new("test")),
        )
    }

    /// The core invariant, checked directly rather than trusted.
    #[test]
    fn gate_rejects_a_change_outside_the_spliced_spans() {
        let original = "*CHI:\txx and yy .\n";
        // A "spliced" text that also changed a byte nobody asked to change.
        let tampered = "*CHI:\txxx and zz .\n";
        let applied = vec![xx_to_xxx_edit()];
        assert!(matches!(
            verify_splice(original, tampered, &applied),
            Err(GateError::UnexpectedChangeOutsideSpans { .. })
        ));
    }

    #[test]
    fn gate_accepts_a_faithful_splice() -> Result<(), GateError> {
        let original = "*CHI:\txx and yy .\n";
        let spliced = "*CHI:\txxx and yy .\n";
        let applied = vec![xx_to_xxx_edit()];
        verify_splice(original, spliced, &applied)
    }

    /// An edit set that the engine itself refuses (here: a target past the
    /// end of the source) must surface as `NotReproducible`, not panic and
    /// not silently report a byte mismatch instead.
    #[test]
    fn gate_reports_engine_refusal_as_not_reproducible() {
        let original = "*CHI:\txx .\n";
        let out_of_bounds = SpliceEdit::new(
            EditTarget::Replace(Span::new(100, 102)),
            Replacement::new("xxx"),
            EditProvenance::Transform(TransformName::new("test")),
        );
        let result = verify_splice(original, original, &[out_of_bounds]);
        assert!(
            matches!(result, Err(GateError::NotReproducible { .. })),
            "got {result:?}"
        );
    }

    /// A length-only difference (spliced is a truncated prefix of the
    /// faithful reconstruction) must still be caught: the shared-prefix
    /// loop alone would see no differing byte.
    #[test]
    fn gate_rejects_a_truncated_result() {
        let original = "*CHI:\txx and yy .\n";
        let truncated = "*CHI:\txxx and";
        let applied = vec![xx_to_xxx_edit()];
        assert!(matches!(
            verify_splice(original, truncated, &applied),
            Err(GateError::UnexpectedChangeOutsideSpans { .. })
        ));
    }

    /// Two edits over `"*CHI:\txx and yy .\n"`: `xx` (6..8) and `yy`
    /// (13..15), each tripled. Exercises the gap-walk across the untouched
    /// region BETWEEN two edits, not just the tail after the last one.
    fn two_edits() -> Vec<SpliceEdit> {
        vec![
            xx_to_xxx_edit(),
            SpliceEdit::new(
                EditTarget::Replace(Span::new(13, 15)),
                Replacement::new("yyy"),
                EditProvenance::Transform(TransformName::new("test")),
            ),
        ]
    }

    #[test]
    fn gate_accepts_a_faithful_multi_edit_splice() -> Result<(), GateError> {
        let original = "*CHI:\txx and yy .\n";
        let spliced = "*CHI:\txxx and yyy .\n";
        verify_splice(original, spliced, &two_edits())
    }

    /// A byte changed in the untouched region BETWEEN two edits: the gap
    /// comparison for the second edit must catch this, not just the tail
    /// comparison after the last edit.
    #[test]
    fn gate_rejects_a_change_in_the_gap_between_two_edits() {
        let original = "*CHI:\txx and yy .\n";
        let tampered = "*CHI:\txxx xnd yyy .\n";
        assert!(matches!(
            verify_splice(original, tampered, &two_edits()),
            Err(GateError::UnexpectedChangeOutsideSpans { .. })
        ));
    }

    /// A byte wrong INSIDE a replacement region itself (not a gap): the
    /// per-edit replacement check must catch this independently of the
    /// gap checks either side of it.
    #[test]
    fn gate_rejects_a_change_inside_a_replacement_region_itself() {
        let original = "*CHI:\txx and yy .\n";
        let tampered = "*CHI:\txxx and zzz .\n";
        assert!(matches!(
            verify_splice(original, tampered, &two_edits()),
            Err(GateError::UnexpectedChangeOutsideSpans { .. })
        ));
    }

    /// `apply_edits_verified` is the composed, gated path: it must return
    /// exactly what a faithful `apply_edits` plus a passing `verify_splice`
    /// would.
    #[test]
    fn apply_edits_verified_returns_the_gated_splice() -> Result<(), GateError> {
        let original = "*CHI:\txx and yy .\n";
        let spliced = apply_edits_verified(original, &two_edits())?;
        assert_eq!(spliced, "*CHI:\txxx and yyy .\n");
        Ok(())
    }

    /// An edit set the engine itself refuses must surface through
    /// `apply_edits_verified` too, not just through the two calls
    /// separately.
    #[test]
    fn apply_edits_verified_propagates_engine_refusal() {
        let original = "*CHI:\txx .\n";
        let out_of_bounds = SpliceEdit::new(
            EditTarget::Replace(Span::new(100, 102)),
            Replacement::new("xxx"),
            EditProvenance::Transform(TransformName::new("test")),
        );
        let result = apply_edits_verified(original, &[out_of_bounds]);
        assert!(
            matches!(result, Err(GateError::NotReproducible { .. })),
            "got {result:?}"
        );
    }
}
