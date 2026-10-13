//! Canonical public-fragment regressions for context and original-source ownership.
//! Models are obtained by parsing; synthetic fault injection stays in its own deck.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use talkbank_model::{ChatOptionFlag, ChatParser, ErrorCollector, FragmentSemanticContext};
use talkbank_parser::TreeSitterParser;

/// Expose only the public contract, not similarly named concrete conveniences.
fn parser() -> impl ChatParser {
    TreeSitterParser::new().expect("grammar loads")
}

/// Policy and source ownership through the canonical public API, not merely
/// agreement between two backends. Non-lexical markers do not turn an omission
/// into an ordinary shortening; an explicitly omitted word keeps its category.
#[test]
fn contextual_admission_uses_the_normalization_shape_owner() {
    let p = parser();
    for offset in [0, "@Comment:\tPréface café\n".len()] {
        for option_flags in [
            vec![],
            vec![ChatOptionFlag::NoAlign],
            vec![ChatOptionFlag::Ca],
            vec![ChatOptionFlag::Ca, ChatOptionFlag::NoAlign],
        ] {
            let context = FragmentSemanticContext::new().with_option_flags(option_flags);
            for input in [
                "*CHI:\t(café) .\n",
                "*CHI:\t⌊(café) .\n",
                "*CHI:\tword [: (café)] .\n",
            ] {
                let errors = ErrorCollector::new();
                let parsed = p.parse_main_tier_with_context(input, offset, &context, &errors);
                if context.ca_mode() {
                    assert!(parsed.is_parsed(), "{input:?}");
                    assert!(errors.is_empty(), "{:?}", errors.to_vec());
                } else {
                    assert!(parsed.is_rejected(), "{input:?}");
                    assert!(
                        errors
                            .to_vec()
                            .iter()
                            .any(|error| error.severity == talkbank_model::Severity::Error)
                    );
                }
            }
            for input in [
                "*CHI:\t(be)cause .\n",
                "*CHI:\t0(café) .\n",
                "*CHI:\tcafé ↗\n",
            ] {
                let errors = ErrorCollector::new();
                assert!(
                    p.parse_main_tier_with_context(input, offset, &context, &errors)
                        .is_parsed(),
                    "{input:?}"
                );
                assert!(errors.is_empty(), "{:?}", errors.to_vec());
            }
        }
    }
}

/// Container metadata must not claim that the adapter-supplied tier label came
/// from the caller's body. The source owner projects before exposing the tier.
#[test]
fn body_fragment_container_spans_exclude_synthetic_labels() {
    let p = parser();
    let input = "n|café .";
    for offset in [0, 25] {
        let errors = ErrorCollector::new();
        let tier = p
            .parse_mor_tier(input, offset, &errors)
            .into_option()
            .unwrap();
        assert_eq!(
            tier.span,
            talkbank_model::Span::from_usize(offset, offset + input.len())
        );
        assert!(errors.is_empty());
        for ending in ["\n", "\r\n"] {
            let source = format!("{input}{ending}");
            let errors = ErrorCollector::new();
            let tier = p
                .parse_mor_tier(&source, offset, &errors)
                .into_option()
                .unwrap();
            assert_eq!(
                tier.span,
                talkbank_model::Span::from_usize(offset, offset + source.len())
            );
            assert!(errors.is_empty());
            let source = format!("%com:\t{ending}");
            let errors = ErrorCollector::new();
            let tier = p
                .parse_dependent_tier(&source, offset, &errors)
                .into_option()
                .unwrap();
            assert_eq!(
                tier.span(),
                talkbank_model::Span::from_usize(offset, offset + source.len())
            );
            assert!(errors.is_empty());
        }
        let errors = ErrorCollector::new();
        let empty = p.parse_com_tier("", offset, &errors).into_option().unwrap();
        assert_eq!(empty.span, talkbank_model::Span::DUMMY);
        assert!(errors.is_empty());
    }
}

/// Empty input still has a unique caller boundary; an empty rejection is not
/// permission to leave a diagnostic at document byte zero.
#[test]
fn empty_fragment_diagnostics_use_the_admitted_document_boundary() {
    let p = parser();
    for offset in [0, 25] {
        let errors = ErrorCollector::new();
        assert!(p.parse_header("", offset, &errors).is_rejected());
        assert!(!errors.is_empty());
        for error in errors.to_vec() {
            assert_eq!(
                error.location.span,
                talkbank_model::Span::from_usize(offset, offset)
            );
        }
    }
}

/// Reapplying the shared owner to an already interpreted, parsed word must
/// preserve its meaning, original spelling and source coordinates.
#[test]
fn contextual_word_normalization_is_a_source_preserving_fixed_point() {
    let p = parser();
    for flags in [
        vec![ChatOptionFlag::Ca],
        vec![ChatOptionFlag::Ca, ChatOptionFlag::NoAlign],
    ] {
        let context = FragmentSemanticContext::new().with_option_flags(flags);
        for offset in [0, 25] {
            for (index, input) in ["(café)", "⌊(café)", "(be)cause", "0(café)"]
                .into_iter()
                .enumerate()
            {
                let errors = ErrorCollector::new();
                let mut word = p
                    .parse_word_with_context(input, offset, &context, &errors)
                    .into_option()
                    .expect("declared contextual word");
                assert!(errors.is_empty(), "{:?}", errors.to_vec());
                if index < 2 {
                    assert_eq!(
                        word.category,
                        Some(talkbank_model::WordCategory::CAOmission)
                    );
                }
                let expected = word.clone();
                talkbank_model::model::content::word::ca::normalize_ca_omission_word(&mut word);
                assert_eq!(word, expected, "{input:?} at {offset}");
            }
        }
    }
}

/// Real alignment findings supply secondary labels; rebasing must exercise
/// those labels positively rather than passing a vacuous empty-label check.
#[test]
fn declared_alignment_labels_preserve_original_coordinates_under_rebasing() {
    use talkbank_model::{ErrorCode, ErrorSink, FragmentSource, Span, TranscriptName};
    use talkbank_parser_tests::repo_paths::workspace_root;

    let p = parser();
    let fixtures =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (fixture, mismatch) in [
        ("E720_2.cha", false),
        ("E720_3.cha", true),
        ("E720_4.cha", true),
    ] {
        let input = std::fs::read_to_string(fixtures.join(fixture)).expect("declared E720 fixture");
        let parse_errors = ErrorCollector::new();
        let mut file = p
            .parse_chat_file(&input, 0, &parse_errors)
            .into_option()
            .expect("declared alignment syntax");
        assert!(
            parse_errors.is_empty(),
            "{fixture}: {:?}",
            parse_errors.to_vec()
        );
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let produced = errors
            .to_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::MorGraCountMismatch)
            .collect::<Vec<_>>();
        if !mismatch {
            assert!(produced.is_empty(), "legal chunk-count control {fixture}");
            continue;
        }
        assert!(!produced.is_empty(), "missing declared mismatch: {fixture}");
        let local = produced;
        for error in &local {
            assert_eq!(error.labels.len(), 2, "{fixture}: {error:?}");
            // Alignment produces raw source ranges; SourceDiagnostic owns
            // later presentation enhancement. Never manufacture raw context.
            assert!(error.context.is_none(), "raw alignment finding: {fixture}");
            for span in std::iter::once(error.location.span)
                .chain(error.labels.iter().map(|label| label.span))
            {
                assert_ne!(span, Span::DUMMY, "source-owned alignment range: {fixture}");
                assert!(input.get(span.start as usize..span.end as usize).is_some());
            }
        }
        for origin in [0, 25] {
            let source = FragmentSource::new(&input, origin).expect("admitted source range");
            let rebased = ErrorCollector::new();
            source.error_sink(&rebased).report_vec(local.clone().into());
            let mut expected = local.clone();
            for error in &mut expected {
                error.location.span = Span::from_usize(
                    error.location.span.start as usize + origin,
                    error.location.span.end as usize + origin,
                );
                for label in &mut error.labels {
                    label.span = Span::from_usize(
                        label.span.start as usize + origin,
                        label.span.end as usize + origin,
                    );
                }
            }
            assert_eq!(rebased.to_vec(), expected, "{fixture} at {origin}");
        }
    }
}
