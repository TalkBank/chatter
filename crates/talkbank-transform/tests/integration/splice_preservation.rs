//! Independent byte oracle for the production splice mapping and receipt.
//! These are generated text operations, not CHAT-validity fixtures or proofs.
use proptest::prelude::*;
use talkbank_model::Span;
use talkbank_transform::splice::{
    EditProvenance, EditTarget, Replacement, SpliceEdit, TransformName, mapped_edit_sites,
    verify_splice,
};

/// A generated disjoint edit set, its immutable source and an independent oracle.
struct DisjointCase {
    source: String,
    edits: Vec<SpliceEdit>,
    expected: String,
}

impl DisjointCase {
    fn from_segments(segments: Vec<(String, String, String)>, tail: String) -> Self {
        let mut source = String::new();
        let mut ranges = Vec::new();
        for (gap, removed, replacement) in segments {
            // A nonempty gap keeps insertion points distinct, including when
            // the preceding edit was an insertion or a deletion.
            assert!(!gap.is_empty());
            source.push_str(&gap);
            let start = source.len();
            source.push_str(&removed);
            ranges.push((start..source.len(), replacement));
        }
        source.push_str(&tail);
        let mut expected = source.clone();
        let mut edits = Vec::new();
        // Reverse in-place editing uses original coordinates throughout. It
        // does not share the production cumulative-delta mapping or gap walk.
        for (range, replacement) in ranges.into_iter().rev() {
            expected.replace_range(range.clone(), &replacement);
            let target = if range.is_empty() {
                EditTarget::InsertAt(range.start)
            } else {
                EditTarget::Replace(Span::new(range.start, range.end))
            };
            edits.push(SpliceEdit::new(
                target,
                Replacement::new(replacement),
                EditProvenance::Transform(TransformName::new("byte-oracle")),
            ));
        }
        Self {
            source,
            edits,
            expected,
        }
    }
}

fn text(min: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), min..9).prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn splice_preservation_matches_independent_reverse_edits(
        segments in prop::collection::vec((text(1), text(0), text(0)), 0..12),
        tail in text(0),
    ) {
        let case = DisjointCase::from_segments(segments, tail);
        let plan = mapped_edit_sites(&case.source, &case.edits).unwrap();
        let verified = plan.apply_verified().unwrap();
        prop_assert_eq!(verified.text(), case.expected.as_str());
        prop_assert_eq!(verified.plan().source(), case.source.as_str());
        prop_assert!(verify_splice(&case.source, &case.expected, &case.edits).is_ok());
        for edit in verified.plan().sites() {
            let span = edit.spliced();
            prop_assert_eq!(&case.expected[span.start()..span.end()], edit.replacement().as_str());
        }
        let extended = format!("{}#", case.expected);
        prop_assert!(verify_splice(&case.source, &extended, &case.edits).is_err());
        if let Some(first) = case.expected.chars().next() {
            // Every nonempty generated output begins with an untouched gap
            // (or, for no edits, the untouched tail). Change one whole scalar
            // so the candidate remains valid UTF-8 even for non-BMP sources.
            let mut tampered = case.expected.clone();
            let replacement = match first { '#' => "!", _ => "#" };
            tampered.replace_range(..first.len_utf8(), replacement);
            prop_assert!(verify_splice(&case.source, &tampered, &case.edits).is_err());
            let truncated = &case.expected[..case.expected.char_indices().last().unwrap().0];
            prop_assert!(verify_splice(&case.source, truncated, &case.edits).is_err());
        }
    }
}
