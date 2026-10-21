use talkbank_model::ErrorCode;
use talkbank_model::errors::Span;

/// Replacement text for one spliced span.
///
/// A newtype rather than a bare `String` so that a replacement can never be
/// confused with a message, a suggestion, or source text at a call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement(String);

impl Replacement {
    /// Wrap replacement text. Empty is legal: it means "delete this span".
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Borrow the replacement text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Name of a non-diagnostic transform that produced an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformName(String);

impl TransformName {
    /// Wrap a transform name such as `fix-s`.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the transform name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where an edit came from, so that a rejection names a culprit.
///
/// Not every edit originates in a diagnostic: fix-s computes edits from the
/// typed model with no error code involved, which is why this is an enum
/// rather than a bare `ErrorCode`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditProvenance {
    /// The edit implements the catalog fix for a diagnostic code.
    Diagnostic(ErrorCode),
    /// The edit comes from a named transform rather than a diagnostic.
    Transform(TransformName),
}

/// What a single edit does to the source text.
///
/// An explicit target rather than an inferred one: `Span::DUMMY` is `{0,0}`,
/// which is also a real zero-length position at the head of the file, so a
/// span alone cannot distinguish "insert at the very start of the file" from
/// "this value has no source location at all". Making the caller say which it
/// means removes that ambiguity instead of resolving it by guesswork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTarget {
    /// Replace this byte range. Must be non-dummy and non-empty.
    Replace(Span),
    /// Insert at this byte offset, replacing nothing. Offset 0 is legal.
    InsertAt(usize),
}

impl EditTarget {
    /// The byte offset this target starts at, for sort and overlap checks.
    ///
    /// `InsertAt(offset)` starts and ends at `offset`: it claims a single
    /// point, not a range. Public because every module that needs to reason
    /// about where an edit falls (the admission gate's utterance lookup,
    /// `chatter fix`'s diagnostic-to-fix-site mapping) needs exactly this
    /// value; it used to be reimplemented privately in each of those places.
    pub fn start_offset(&self) -> usize {
        match self {
            EditTarget::Replace(span) => span.start(),
            EditTarget::InsertAt(offset) => *offset,
        }
    }

    /// The byte offset this target ends at, i.e. where the cursor resumes.
    ///
    /// Public for the same reason as [`start_offset`](Self::start_offset).
    pub fn end_offset(&self) -> usize {
        match self {
            EditTarget::Replace(span) => span.end(),
            EditTarget::InsertAt(offset) => *offset,
        }
    }
}

/// One replacement or insertion against the ORIGINAL source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpliceEdit {
    target: EditTarget,
    replacement: Replacement,
    provenance: EditProvenance,
    recovery_safety: RecoverySafety,
}

/// Whether an edit requires clean parser provenance or is itself the exact
/// mechanical repair for syntax recovery at its site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoverySafety {
    /// Never write into an utterance whose parsed model needed recovery.
    RequiresClean,
    /// The catalog edit removes the syntax defect that caused recovery; the
    /// caller must still reparse and verify the result after splicing.
    RepairsTaintingSyntax,
    /// The catalog admitted a specific token from a recovery-free typed header.
    VerifiedHeaderToken,
}

impl SpliceEdit {
    /// Minted only after the catalog has bound the header token to its source.
    pub(crate) fn new_header_token(
        target: EditTarget,
        replacement: Replacement,
        diagnostic: ErrorCode,
    ) -> Self {
        Self {
            target,
            replacement,
            provenance: EditProvenance::Diagnostic(diagnostic),
            recovery_safety: RecoverySafety::VerifiedHeaderToken,
        }
    }

    /// Build an edit. Validation happens in [`apply_edits`], against the
    /// source the target actually indexes into.
    pub fn new(target: EditTarget, replacement: Replacement, provenance: EditProvenance) -> Self {
        Self {
            target,
            replacement,
            provenance,
            recovery_safety: RecoverySafety::RequiresClean,
        }
    }

    /// Build a catalog-owned edit that repairs the syntax recovery at its own
    /// site. Crate-private so external callers cannot forge this proof state
    /// merely by attaching a diagnostic label.
    pub(crate) fn new_recovery_repair(
        target: EditTarget,
        replacement: Replacement,
        diagnostic: ErrorCode,
    ) -> Self {
        Self {
            target,
            replacement,
            provenance: EditProvenance::Diagnostic(diagnostic),
            recovery_safety: RecoverySafety::RepairsTaintingSyntax,
        }
    }

    /// Parser-recovery admission state carried with this edit.
    pub(crate) fn recovery_safety(&self) -> RecoverySafety {
        self.recovery_safety
    }

    /// What this edit does to the source text: replace a range or insert at
    /// a point.
    pub fn target(&self) -> &EditTarget {
        &self.target
    }

    /// The text this edit writes in place of its target.
    ///
    /// Exposed so that a caller can map an edit's ORIGINAL-source span onto
    /// the SPLICED-text span it produced (the post-write safety check in
    /// `chatter fix` needs this to know where a fixed diagnostic's code
    /// should no longer fire), without duplicating the replacement text
    /// itself outside this type.
    pub fn replacement(&self) -> &Replacement {
        &self.replacement
    }

    /// What produced this edit.
    pub fn provenance(&self) -> &EditProvenance {
        &self.provenance
    }
}

/// Why a set of edits could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum SpliceError {
    /// A `Replace` span had zero width. Use
    /// `EditTarget::InsertAt` to insert instead, so there is exactly one way
    /// to express an insertion.
    #[error("span {span:?} is empty; use EditTarget::InsertAt to insert")]
    EmptyReplaceRange {
        /// The empty, non-dummy span.
        span: Span,
    },
    /// A span boundary or insertion offset fell inside a multi-byte
    /// character.
    #[error("byte offset {offset} is not a character boundary")]
    NotCharBoundary {
        /// The offending offset.
        offset: usize,
    },
    /// A span or insertion offset reached past the end of the source.
    #[error("span {span:?} is out of bounds for source of {len} bytes")]
    OutOfBounds {
        /// The offending span. For `InsertAt`, this is a zero-width span at
        /// the offending offset.
        span: Span,
        /// Length of the source text in bytes.
        len: usize,
    },
    /// Two edits claimed the same or overlapping bytes.
    ///
    /// Two edits that start at the same offset are refused even when
    /// neither is a `Replace`: two insertions at one point have no defined
    /// order, and an insertion at the start of a replaced range is
    /// ambiguous about whether it lands inside or outside the replacement.
    #[error("edits from {earlier:?} and {later:?} overlap")]
    Overlap {
        /// The edit that starts first, or ties for first.
        earlier: EditProvenance,
        /// The edit that starts inside it, or ties with it.
        later: EditProvenance,
    },
    /// An offset in the spliced text would exceed `usize`. Replacements are
    /// in memory, so this cannot arise from real edits; it is checked rather
    /// than assumed so arithmetic never wraps.
    #[error("offset {base} + {added} in the spliced text overflows")]
    OffsetOverflow {
        /// The offset being advanced.
        base: usize,
        /// The bytes being added to it.
        added: usize,
    },
}

/// Read-only mapping API; its returned owner retains the admitted source.
pub use super::mapping::{MappedEdit, MappedPlan, mapped_edit_sites};

/// Apply raw candidates after geometry admission, without issuing byte-verification
/// or catalog/health/post-check authority. Use the verified operation when needed.
pub fn apply_edits(source: &str, edits: &[SpliceEdit]) -> Result<String, SpliceError> {
    Ok(mapped_edit_sites(source, edits)?.assemble().into_text())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single-`xx` fixture. Byte offsets every test below depends on:
    /// `*CHI:` is 0..5, the tab is 5, `xx` is 6..8, the space is 8, `.` is 9,
    /// the newline is 10, so the whole source is 11 bytes.
    ///
    /// Named rather than retyped per test on purpose: the offsets passed to
    /// `edit()` and `insert()` are only meaningful against THIS exact string,
    /// and a literal repeated a dozen times invites someone to adjust one copy
    /// and silently misalign every offset in the others.
    const SOURCE: &str = "*CHI:\txx .\n";

    /// Two-`xx` fixture, for the multi-edit cases. First `xx` is 6..8, second
    /// is 13..15.
    const TWO_XX_SOURCE: &str = "*CHI:\txx and xx .\n";

    /// Fixture whose 6..8 bytes are a single two-byte `é`, so that offset 7
    /// lands INSIDE a character. Used for the char-boundary rejections.
    const ACCENTED_SOURCE: &str = "*CHI:\té .\n";

    fn edit(start: usize, end: usize, text: &str) -> SpliceEdit {
        SpliceEdit::new(
            EditTarget::Replace(Span::new(start, end)),
            Replacement::new(text),
            EditProvenance::Transform(TransformName::new("test")),
        )
    }

    fn insert(offset: usize, text: &str) -> SpliceEdit {
        SpliceEdit::new(
            EditTarget::InsertAt(offset),
            Replacement::new(text),
            EditProvenance::Transform(TransformName::new("test")),
        )
    }

    /// The core invariant: every byte outside a spliced span survives verbatim.
    #[test]
    fn untouched_regions_are_byte_identical() -> Result<(), SpliceError> {
        let source = TWO_XX_SOURCE;
        // Replace only the FIRST xx (bytes 6..8).
        let out = apply_edits(source, &[edit(6, 8, "xxx")])?;
        assert_eq!(out, "*CHI:\txxx and xx .\n");
        Ok(())
    }

    #[test]
    fn multiple_edits_apply_left_to_right_without_offset_drift() -> Result<(), SpliceError> {
        let source = TWO_XX_SOURCE;
        let out = apply_edits(source, &[edit(13, 15, "xxx"), edit(6, 8, "xxx")])?;
        assert_eq!(out, "*CHI:\txxx and xxx .\n");
        Ok(())
    }

    #[test]
    fn overlapping_edits_are_rejected_loudly() {
        let source = SOURCE;
        let result = apply_edits(source, &[edit(6, 8, "xxx"), edit(7, 9, "yyy")]);
        assert!(
            matches!(result, Err(SpliceError::Overlap { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn non_char_boundary_is_rejected_instead_of_panicking() {
        // "é" is two bytes at 6..8; 7 is inside it.
        let source = ACCENTED_SOURCE;
        let result = apply_edits(source, &[edit(7, 8, "x")]);
        assert!(
            matches!(result, Err(SpliceError::NotCharBoundary { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn out_of_bounds_span_is_rejected() {
        let result = apply_edits(SOURCE, &[edit(100, 102, "xxx")]);
        assert!(
            matches!(result, Err(SpliceError::OutOfBounds { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn empty_edit_set_returns_source_unchanged() -> Result<(), SpliceError> {
        let source = SOURCE;
        assert_eq!(apply_edits(source, &[])?, source);
        Ok(())
    }

    /// A zero-width span that is not the dummy span (e.g. 5..5) must not be
    /// silently treated as an insertion: `InsertAt` is the only spelling for
    /// that, so this is rejected instead of guessed at.
    #[test]
    fn empty_replace_range_is_rejected() {
        let result = apply_edits(SOURCE, &[edit(5, 5, "x")]);
        assert!(
            matches!(result, Err(SpliceError::EmptyReplaceRange { .. })),
            "got {result:?}"
        );
    }

    /// The concrete case this fix exists for: E503's repair inserts
    /// "@UTF8\n" at the very start of a file, offset 0, which the old
    /// span-only interface could never represent because offset 0 collided
    /// with the dummy sentinel.
    #[test]
    fn insert_at_zero_prepends_and_preserves_rest() -> Result<(), SpliceError> {
        let source = SOURCE;
        let out = apply_edits(source, &[insert(0, "@UTF8\n")])?;
        assert_eq!(out, "@UTF8\n*CHI:\txx .\n");
        Ok(())
    }

    #[test]
    fn two_insertions_at_the_same_offset_are_rejected() {
        let source = SOURCE;
        let result = apply_edits(source, &[insert(6, "a"), insert(6, "b")]);
        assert!(
            matches!(result, Err(SpliceError::Overlap { .. })),
            "got {result:?}"
        );
    }

    /// One edit's end exactly equal to the next edit's start is not an
    /// overlap: the strict `<` check must let this pair through.
    #[test]
    fn adjacent_replaces_both_apply() -> Result<(), SpliceError> {
        let source = SOURCE;
        let out = apply_edits(source, &[edit(2, 4, "AA"), edit(4, 6, "BB")])?;
        assert_eq!(out, "*CAABBxx .\n");
        Ok(())
    }

    #[test]
    fn insert_at_out_of_bounds_is_rejected() {
        let result = apply_edits(SOURCE, &[insert(100, "x")]);
        assert!(
            matches!(result, Err(SpliceError::OutOfBounds { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn insert_at_non_char_boundary_is_rejected() {
        // "é" is two bytes at 6..8; 7 is inside it.
        let source = ACCENTED_SOURCE;
        let result = apply_edits(source, &[insert(7, "x")]);
        assert!(
            matches!(result, Err(SpliceError::NotCharBoundary { .. })),
            "got {result:?}"
        );
    }
}
