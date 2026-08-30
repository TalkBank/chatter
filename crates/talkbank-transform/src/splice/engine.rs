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
    InsertAt(u32),
}

impl EditTarget {
    /// The byte offset this target starts at, for sort and overlap checks.
    ///
    /// `InsertAt(offset)` starts and ends at `offset`: it claims a single
    /// point, not a range. Public because every module that needs to reason
    /// about where an edit falls (the admission gate's utterance lookup,
    /// `chatter fix`'s diagnostic-to-fix-site mapping) needs exactly this
    /// value; it used to be reimplemented privately in each of those places.
    pub fn start_offset(&self) -> u32 {
        match self {
            EditTarget::Replace(span) => span.start,
            EditTarget::InsertAt(offset) => *offset,
        }
    }

    /// The byte offset this target ends at, i.e. where the cursor resumes.
    ///
    /// Public for the same reason as [`start_offset`](Self::start_offset).
    pub fn end_offset(&self) -> u32 {
        match self {
            EditTarget::Replace(span) => span.end,
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
}

impl SpliceEdit {
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
    /// `Span::DUMMY` is `{0, 0}`, which is indistinguishable from a real
    /// zero-length position at the head of the file, so a `Replace` carrying
    /// it would silently splice there.
    #[error("edit from {provenance:?} carries the dummy span, which is not a source location")]
    DummySpan {
        /// The edit's origin.
        provenance: EditProvenance,
    },
    /// A `Replace` span had zero width without being the dummy span. Use
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
        offset: u32,
    },
    /// A span or insertion offset reached past the end of the source.
    #[error("span {span:?} is out of bounds for source of {len} bytes")]
    OutOfBounds {
        /// The offending span. For `InsertAt`, this is a zero-width span at
        /// the offending offset.
        span: Span,
        /// Length of the source text in bytes.
        len: u32,
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
    /// A cumulative offset computed while mapping edits onto the spliced
    /// text did not fit in `u32`. Every real edit set operates over
    /// file-sized text, so this should never trigger in practice; it is
    /// checked rather than assumed so a pathological cumulative shift is a
    /// typed error instead of an unchecked truncation.
    #[error("offset {offset} computed while mapping edits onto spliced text does not fit in u32")]
    OffsetOverflow {
        /// The out-of-range signed offset.
        offset: i64,
    },
}

/// Validate one edit's target against `source`, without regard to any other
/// edit. Overlap between edits is a separate, ordering-dependent check in
/// [`apply_edits`].
fn validate_target(source: &str, len: u32, edit: &SpliceEdit) -> Result<(), SpliceError> {
    match edit.target {
        EditTarget::Replace(span) => {
            if span.is_dummy() {
                return Err(SpliceError::DummySpan {
                    provenance: edit.provenance.clone(),
                });
            }
            if span.end > len || span.start > span.end {
                return Err(SpliceError::OutOfBounds { span, len });
            }
            if span.start == span.end {
                return Err(SpliceError::EmptyReplaceRange { span });
            }
            for offset in [span.start, span.end] {
                if !source.is_char_boundary(offset as usize) {
                    return Err(SpliceError::NotCharBoundary { offset });
                }
            }
        }
        EditTarget::InsertAt(offset) => {
            if offset > len {
                return Err(SpliceError::OutOfBounds {
                    span: Span::at(offset),
                    len,
                });
            }
            if !source.is_char_boundary(offset as usize) {
                return Err(SpliceError::NotCharBoundary { offset });
            }
        }
    }
    Ok(())
}

/// One edit's byte range in both the ORIGINAL source and the text that
/// results from applying every edit up to and including it (the "spliced"
/// text).
///
/// This is the position [`gate::verify_splice`](super::gate::verify_splice)'s
/// gap-walk and `chatter fix`'s post-splice re-check both need: where does
/// this edit's replacement land once every earlier edit (in ascending
/// start-offset order) has grown or shrunk the text ahead of it. Computed
/// once by [`mapped_edit_sites`] so no caller reimplements the
/// cumulative-delta fold that produces it, and, within this crate,
/// `splice_with_sites` folds it exactly once per splice: `apply_edits`
/// and [`gate::apply_edits_verified`](super::gate::apply_edits_verified)
/// both build on it rather than each calling [`mapped_edit_sites`]
/// separately on the same `(source, edits)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedEdit {
    original: Span,
    spliced: Span,
    provenance: EditProvenance,
    replacement: Replacement,
}

impl MappedEdit {
    /// This edit's byte range in the original source: the span it
    /// replaced, or a zero-width point at the insertion offset.
    pub fn original(&self) -> Span {
        self.original
    }

    /// This edit's byte range in the spliced text: where its replacement
    /// now sits, after accounting for how every earlier edit grew or
    /// shrank the text ahead of it.
    pub fn spliced(&self) -> Span {
        self.spliced
    }

    /// What produced this edit.
    pub fn provenance(&self) -> &EditProvenance {
        &self.provenance
    }

    /// The text this edit writes, i.e. the same value as the originating
    /// [`SpliceEdit::replacement`], carried alongside the mapped spans so a
    /// caller does not have to zip two parallel sequences back together.
    pub fn replacement(&self) -> &Replacement {
        &self.replacement
    }
}

/// Convert a cumulative signed offset back to `u32`, typed rather than
/// truncating: see [`SpliceError::OffsetOverflow`].
fn checked_u32_offset(offset: i64) -> Result<u32, SpliceError> {
    u32::try_from(offset).map_err(|_| SpliceError::OffsetOverflow { offset })
}

/// Validate, sort, and map `edits` onto both the original source and the
/// text that results from applying them, WITHOUT building that text.
///
/// This is the shared core [`apply_edits`] and
/// [`gate::verify_splice`](super::gate::verify_splice) both build on:
/// `apply_edits` copies bytes according to the spans this computes, and
/// `verify_splice` compares bytes according to them, so the
/// validate/sort/overlap/cumulative-delta logic exists exactly once.
/// `chatter fix`'s post-splice re-check (`mapped_fix_sites`) uses it too,
/// to find where each fixed diagnostic's code now lives in the spliced
/// text.
pub fn mapped_edit_sites(
    source: &str,
    edits: &[SpliceEdit],
) -> Result<Vec<MappedEdit>, SpliceError> {
    let len = source.len() as u32;

    for edit in edits {
        validate_target(source, len, edit)?;
    }

    // Unstable sort is safe here, and stability was never what made the
    // overlap check work: ANY correct sort groups equal keys contiguously,
    // which is all the "compare against the previous edit" check relies on.
    // The relative order WITHIN a group of tied start offsets is never
    // observed, because a tie is rejected as an overlap either way.
    let mut sorted: Vec<&SpliceEdit> = edits.iter().collect();
    sorted.sort_unstable_by_key(|edit| edit.target.start_offset());

    let mut mapped = Vec::with_capacity(sorted.len());
    let mut cursor: u32 = 0;
    let mut delta: i64 = 0;
    let mut previous: Option<&SpliceEdit> = None;

    for edit in sorted {
        let start = edit.target.start_offset();
        let end = edit.target.end_offset();

        // Two edits tied on start offset overlap regardless of kind (see
        // the doc comment on `SpliceError::Overlap`); otherwise the usual
        // "does this edit begin before the previous one finished" check.
        let overlaps = match previous {
            Some(previous_edit) if previous_edit.target.start_offset() == start => true,
            _ => start < cursor,
        };
        if overlaps {
            let earlier = previous
                .map(|p| p.provenance.clone())
                .unwrap_or_else(|| edit.provenance.clone());
            return Err(SpliceError::Overlap {
                earlier,
                later: edit.provenance.clone(),
            });
        }

        let replacement_len = edit.replacement.as_str().len() as i64;
        let spliced_start = checked_u32_offset(i64::from(start) + delta)?;
        let spliced_end = checked_u32_offset(i64::from(spliced_start) + replacement_len)?;

        mapped.push(MappedEdit {
            original: Span::new(start, end),
            spliced: Span::new(spliced_start, spliced_end),
            provenance: edit.provenance.clone(),
            replacement: edit.replacement.clone(),
        });

        delta += replacement_len - i64::from(end - start);
        cursor = end;
        previous = Some(edit);
    }

    Ok(mapped)
}

/// Splice `edits` into `source`, returning both the resulting text and the
/// [`MappedEdit`] mapping used to build it.
///
/// The shared internal core of [`apply_edits`] and
/// [`gate::apply_edits_verified`](super::gate::apply_edits_verified):
/// both need the mapping (one to build the text from it, the other to
/// additionally verify the text against it), and recomputing a
/// deterministic fold on identical inputs cannot disagree with itself, so
/// `apply_edits_verified` calls this once and hands the mapping it already
/// has to `gate`'s sites-based verify entry point rather than asking
/// [`gate::verify_splice`] to redo [`mapped_edit_sites`] a second time on
/// the same `(source, edits)`.
///
/// Not part of the public API: a tuple return is acceptable at this
/// private seam between two modules of the same crate (the caller
/// destructures it immediately), never at a public one.
pub(super) fn splice_with_sites(
    source: &str,
    edits: &[SpliceEdit],
) -> Result<(String, Vec<MappedEdit>), SpliceError> {
    let mapped = mapped_edit_sites(source, edits)?;

    let mut out = String::with_capacity(source.len());
    let mut cursor: u32 = 0;
    for edit in &mapped {
        out.push_str(&source[cursor as usize..edit.original.start as usize]);
        out.push_str(edit.replacement.as_str());
        cursor = edit.original.end;
    }
    out.push_str(&source[cursor as usize..]);

    Ok((out, mapped))
}

/// Apply every edit to `source`, returning the spliced text.
///
/// Built entirely on [`mapped_edit_sites`] (via `splice_with_sites`):
/// every target indexes the ORIGINAL source, which is the only text it was
/// ever valid against, so copying the mapped gaps and replacements
/// forward, in ascending original order, means no edit can invalidate
/// another's offsets. The alternative (mutating in place from the end
/// backwards) works only because of that reverse ordering, costs O(n) per
/// edit, and gives overlap detection nowhere natural to live.
///
/// This returns the raw spliced string with no write-gate: nothing stops a
/// caller from writing it to disk unverified. Prefer
/// [`gate::apply_edits_verified`](super::gate::apply_edits_verified) for
/// anything that reaches disk; call this directly only when the raw,
/// unverified form is genuinely what is needed.
pub fn apply_edits(source: &str, edits: &[SpliceEdit]) -> Result<String, SpliceError> {
    let (spliced, _mapped) = splice_with_sites(source, edits)?;
    Ok(spliced)
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

    fn edit(start: u32, end: u32, text: &str) -> SpliceEdit {
        SpliceEdit::new(
            EditTarget::Replace(Span::new(start, end)),
            Replacement::new(text),
            EditProvenance::Transform(TransformName::new("test")),
        )
    }

    fn insert(offset: u32, text: &str) -> SpliceEdit {
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

    /// Span::DUMMY is {0,0}, the same bytes as a real position at the head of
    /// the file, so it must never be read as "splice at the start of the file".
    #[test]
    fn dummy_span_is_rejected() {
        let result = apply_edits(SOURCE, &[edit(0, 0, "xxx")]);
        assert!(
            matches!(result, Err(SpliceError::DummySpan { .. })),
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
