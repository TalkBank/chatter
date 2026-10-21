//! One checked mapping and the immutable source from which it was computed.

use super::engine::{EditProvenance, EditTarget, Replacement, SpliceEdit, SpliceError};
use talkbank_model::Span;

/// One edit's immutable original and resulting byte coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedEdit {
    original: Span,
    spliced: Span,
    provenance: EditProvenance,
    replacement: Replacement,
}

impl MappedEdit {
    /// The replaced original span, or its insertion point.
    pub fn original(&self) -> Span {
        self.original
    }
    /// The resulting replacement's span, including a zero-width deletion.
    pub fn spliced(&self) -> Span {
        self.spliced
    }
    /// The edit's recorded origin.
    pub fn provenance(&self) -> &EditProvenance {
        &self.provenance
    }
    /// The requested replacement text.
    pub fn replacement(&self) -> &Replacement {
        &self.replacement
    }
}

/// `base + added`, refused rather than wrapped.
fn advance(base: usize, added: usize) -> Result<usize, SpliceError> {
    base.checked_add(added)
        .ok_or(SpliceError::OffsetOverflow { base, added })
}

#[derive(Debug)]
struct BoundedSource<'source> {
    text: &'source str,
    len: usize,
}

impl<'source> BoundedSource<'source> {
    fn admit(text: &'source str) -> Self {
        Self {
            text,
            len: text.len(),
        }
    }

    fn validate(&self, edit: &SpliceEdit) -> Result<(), SpliceError> {
        match *edit.target() {
            EditTarget::Replace(span) => {
                if span.end() > self.len {
                    return Err(SpliceError::OutOfBounds {
                        span,
                        len: self.len,
                    });
                }
                if span.start() == span.end() {
                    return Err(SpliceError::EmptyReplaceRange { span });
                }
                for offset in [span.start(), span.end()] {
                    if !self.text.is_char_boundary(offset) {
                        return Err(SpliceError::NotCharBoundary { offset });
                    }
                }
            }
            EditTarget::InsertAt(offset) => {
                if offset > self.len {
                    return Err(SpliceError::OutOfBounds {
                        span: Span::at(offset),
                        len: self.len,
                    });
                }
                if !self.text.is_char_boundary(offset) {
                    return Err(SpliceError::NotCharBoundary { offset });
                }
            }
        }
        Ok(())
    }
}

/// A mapping issued against its exact retained source, including output extent.
///
/// Read-only sites describe geometry, not catalog policy or source/model origin.
/// The internal assembly and byte verifier take this owner, never a separately
/// replaceable source and site slice.
///
/// ```compile_fail
/// use talkbank_transform::splice::MappedPlan;
/// let forged = MappedPlan { source: "", mapped: Vec::new(), output_len: 0 };
/// ```
#[derive(Debug)]
pub struct MappedPlan<'source> {
    source: BoundedSource<'source>,
    mapped: Vec<MappedEdit>,
    output_len: usize,
}

impl<'source> MappedPlan<'source> {
    /// The exact source against which targets and UTF-8 boundaries were admitted.
    pub fn source(&self) -> &'source str {
        self.source.text
    }
    /// Read-only computed sites; these do not themselves issue a mapping owner.
    pub fn sites(&self) -> &[MappedEdit] {
        &self.mapped
    }

    /// Assemble from this mapping once; byte verification remains a next phase.
    pub(super) fn assemble(self) -> AssembledSplice<'source> {
        let mut text = String::with_capacity(self.output_len);
        let mut cursor = 0;
        for edit in &self.mapped {
            text.push_str(&self.source.text[cursor..edit.original.start()]);
            text.push_str(edit.replacement.as_str());
            cursor = edit.original.end();
        }
        text.push_str(&self.source.text[cursor..]);
        AssembledSplice { plan: self, text }
    }
}

impl std::ops::Deref for MappedPlan<'_> {
    type Target = [MappedEdit];
    fn deref(&self) -> &Self::Target {
        self.sites()
    }
}

/// The assembly and its producer-owned mapping, before byte verification.
pub(super) struct AssembledSplice<'source> {
    pub(super) plan: MappedPlan<'source>,
    pub(super) text: String,
}

impl AssembledSplice<'_> {
    pub(super) fn into_text(self) -> String {
        self.text
    }
}

/// Validate, order and map candidates, retaining their actual source.
///
/// Same-start edits and intersecting extents refuse; adjacent replacements do
/// not. Output offsets use checked `usize` arithmetic. No CHAT-size, validity, health or semantic policy is added.
pub fn mapped_edit_sites<'source>(
    source: &'source str,
    edits: &[SpliceEdit],
) -> Result<MappedPlan<'source>, SpliceError> {
    let source = BoundedSource::admit(source);
    for edit in edits {
        source.validate(edit)?;
    }
    let mut sorted: Vec<&SpliceEdit> = edits.iter().collect();
    sorted.sort_unstable_by_key(|edit| edit.target().start_offset());
    let mut mapped = Vec::with_capacity(sorted.len());
    let mut cursor = 0;
    // Bytes removed and inserted so far. Tracked separately so offsets stay
    // unsigned: every removed byte lies before the current edit's start.
    let mut removed = 0usize;
    let mut added = 0usize;
    let mut previous: Option<&SpliceEdit> = None;
    for edit in sorted {
        let start = edit.target().start_offset();
        let end = edit.target().end_offset();
        if let Some(previous_edit) = previous
            && (previous_edit.target().start_offset() == start || start < cursor)
        {
            return Err(SpliceError::Overlap {
                earlier: previous_edit.provenance().clone(),
                later: edit.provenance().clone(),
            });
        }
        let replacement_len = edit.replacement().as_str().len();
        let spliced_start = advance(start - removed, added)?;
        let spliced_end = advance(spliced_start, replacement_len)?;
        mapped.push(MappedEdit {
            original: Span::new(start, end),
            spliced: Span::new(spliced_start, spliced_end),
            provenance: edit.provenance().clone(),
            replacement: edit.replacement().clone(),
        });
        removed += end - start;
        added = advance(added, replacement_len)?;
        cursor = end;
        previous = Some(edit);
    }
    let output_len = advance(source.len - removed, added)?;
    Ok(MappedPlan {
        source,
        mapped,
        output_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spliced_offsets_refuse_overflow_instead_of_wrapping() {
        assert_eq!(advance(3, 4).unwrap(), 7);
        assert!(matches!(
            advance(usize::MAX, 1),
            Err(SpliceError::OffsetOverflow {
                base: usize::MAX,
                added: 1
            })
        ));
    }
}
