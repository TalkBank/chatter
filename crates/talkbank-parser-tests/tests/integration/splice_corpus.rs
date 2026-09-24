//! Splice protocol contracts over immutable reference CHAT sources.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::{Span, WriteChat, model::SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::splice::{
    EditProvenance, EditTarget, GateError, Replacement, SkipReason, SpliceEdit, SpliceError,
    TransformName, admit_edits, apply_edits_verified, mapped_edit_sites, verify_splice,
};

fn identity_edit(source: &str, span: Span) -> SpliceEdit {
    SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new(
            source
                .get(span.start as usize..span.end as usize)
                .expect("source-owned main-tier span"),
        ),
        EditProvenance::Transform(TransformName::new(format!("identity-at-{}", span.start))),
    )
}

#[test]
fn reference_splice_mapping_preserves_source_and_refuses_coordinate_corruption() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut edits_seen = 0;
    let mut unicode_refusals = 0;
    let mut normalized_changes = 0;
    let mut crossing_refusals = 0;
    let mut within_tier_boundaries = 0;
    for fixture in corpus.fixtures() {
        let source = fixture.source();
        let file = strict_parse(parser.parse_chat_file(source)).expect("reference parses");
        for utterance in file.utterances() {
            if let Some(last) = utterance.dependent_tiers.last() {
                let span = Span::new(utterance.main.span.start, last.tier.span().end);
                let accepted = admit_edits(&file, vec![identity_edit(source, span)]);
                assert!(
                    accepted.skipped.is_empty(),
                    "multiple tiers of the same utterance remain one scope"
                );
                assert_eq!(accepted.admitted.len(), 1);
                within_tier_boundaries += 1;
            }
        }
        let mut rows = file.utterances();
        if let (Some(first), Some(second)) = (rows.next(), rows.next()) {
            let crossing = identity_edit(
                source,
                Span::new(first.main.span.start, second.main.span.end),
            );
            let refused = admit_edits(&file, vec![crossing]);
            assert!(
                refused.admitted.is_empty(),
                "a clean start cannot license a cross-utterance replacement"
            );
            assert_eq!(refused.skipped.len(), 1);
            assert_eq!(refused.skipped[0].reason, SkipReason::OutsideAnyUtterance);
            crossing_refusals += 1;
        }
        let mut proposed: Vec<_> = file
            .utterances()
            .map(|u| identity_edit(source, u.main.span))
            .collect();
        proposed.reverse();
        let admission = admit_edits(&file, proposed);
        assert!(
            admission.skipped.is_empty(),
            "reference main tiers have clean parser provenance"
        );
        assert_eq!(admission.admitted.len(), file.utterances().count());
        let mapped = mapped_edit_sites(source, &admission.admitted)
            .expect("source-derived nonoverlapping spans");
        assert_eq!(mapped.len(), admission.admitted.len());
        for ((site, utterance), proposed) in mapped
            .iter()
            .zip(file.utterances())
            .zip(admission.admitted.iter().rev())
        {
            assert_eq!(
                site.original(),
                utterance.main.span,
                "mapping restores source order"
            );
            assert_eq!(
                site.spliced(),
                site.original(),
                "identity edits have zero displacement"
            );
            assert_eq!(
                site.replacement().as_str(),
                &source[site.original().start as usize..site.original().end as usize]
            );
            assert_eq!(site.provenance(), proposed.provenance());
        }
        let output = apply_edits_verified(source, &admission.admitted).expect("verified splice");
        assert_eq!(
            output, source,
            "identity edits preserve every byte, including outside all main tiers"
        );
        verify_splice(source, &output, &admission.admitted).expect("external output verification");
        let reparsed =
            strict_parse(parser.parse_chat_file(&output)).expect("spliced reference parses");
        assert!(file.semantic_eq(&reparsed));
        assert_eq!(
            apply_edits_verified(source, &[]).expect("no-op splice"),
            source
        );

        let normalized: Vec<_> = file
            .utterances()
            .map(|u| {
                let span = u.main.span;
                let original = &source[span.start as usize..span.end as usize];
                let mut replacement = u.main.to_chat_string();
                // The model serializes tier content, not source line framing.
                // Preserve the actual span's ending without interpreting CHAT text.
                if original.ends_with("\r\n") {
                    replacement.push_str("\r\n");
                } else if original.ends_with('\n') {
                    replacement.push('\n');
                }
                SpliceEdit::new(
                    EditTarget::Replace(span),
                    Replacement::new(replacement),
                    EditProvenance::Transform(TransformName::new("canonical-main-tier")),
                )
            })
            .collect();
        let normalization = admit_edits(&file, normalized);
        assert!(normalization.skipped.is_empty());
        let normalized = apply_edits_verified(source, &normalization.admitted)
            .expect("canonical main-tier splice");
        let normalized_model =
            strict_parse(parser.parse_chat_file(&normalized)).expect("canonical splice parses");
        assert!(
            file.semantic_eq(&normalized_model),
            "canonical splice semantics: {}",
            fixture.path().display()
        );
        for site in
            mapped_edit_sites(source, &normalization.admitted).expect("normalization mapping")
        {
            assert_eq!(
                &normalized[site.spliced().start as usize..site.spliced().end as usize],
                site.replacement().as_str(),
                "replacement resides at its mapped coordinate"
            );
        }
        normalized_changes += usize::from(normalized != source);

        let mut extra = output.clone();
        extra.push('\n');
        assert!(
            matches!(
                verify_splice(source, &extra, &admission.admitted),
                Err(GateError::UnexpectedChangeOutsideSpans { .. })
            ),
            "unrecorded extra tail must refuse"
        );
        let mut truncated = output;
        truncated.pop().expect("nonempty CHAT source");
        assert!(
            matches!(
                verify_splice(source, &truncated, &admission.admitted),
                Err(GateError::UnexpectedChangeOutsideSpans { .. })
            ),
            "unrecorded truncation must refuse"
        );
        if let Some(first) = admission.admitted.first() {
            let duplicate = [first.clone(), first.clone()];
            assert!(matches!(
                mapped_edit_sites(source, &duplicate),
                Err(SpliceError::Overlap { .. })
            ));
            assert!(matches!(
                apply_edits_verified(source, &duplicate),
                Err(GateError::NotReproducible {
                    source: SpliceError::Overlap { .. }
                })
            ));
        }
        if let Some((offset, _)) = source
            .char_indices()
            .find(|(_, character)| character.len_utf8() > 1)
        {
            let offset = u32::try_from(offset + 1).expect("fixture fits source coordinates");
            let edit = SpliceEdit::new(
                EditTarget::InsertAt(offset),
                Replacement::new(""),
                EditProvenance::Transform(TransformName::new("invalid-coordinate")),
            );
            assert!(matches!(mapped_edit_sites(source, &[edit]),
                Err(SpliceError::NotCharBoundary { offset: actual }) if actual == offset));
            unicode_refusals += 1;
        }
        edits_seen += mapped.len();
    }
    assert!(
        edits_seen > 0 && unicode_refusals > 0,
        "source spans and multibyte boundaries are witnessed"
    );
    assert!(
        normalized_changes > 0,
        "canonical references witness non-identity mapped replacements"
    );
    assert!(
        crossing_refusals > 0,
        "multi-utterance reference ranges must exercise containment refusal"
    );
    assert!(
        within_tier_boundaries > 0,
        "containment is not narrowed to one tier"
    );
}
