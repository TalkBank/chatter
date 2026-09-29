//! Public admission contracts from canonical reference/spec examples.

use talkbank_model::RuleSelection;
use talkbank_model::model::{FileStem, TranscriptName};
use talkbank_model::validation::AlignmentValidation;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::pseudonymize::{InputRefusal, NameMap};

const REFERENCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/reference/languages/eng-conversation.cha"
));
const MOR_SYNTAX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/error_corpus/validation_errors/E702_1.cha"
));
const MOR_COUNT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/error_corpus/validation_errors/E706_1.cha"
));
const NAMED_MEDIA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/error_corpus/validation_errors/E531_2.cha"
));

fn private_map(parser: &TreeSitterParser) -> NameMap {
    NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'Rose'\nreplacement = 'PersonA'\n",
        parser,
    )
    .unwrap()
}

#[test]
fn morphology_proposals_bind_exact_main_lemma_fields_after_post_clitics() {
    use talkbank_transform::pseudonymize::MorphologyOutcome;

    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/tiers/mor-name-source-fields.cha"
    ));
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    let originals: Vec<_> = plan
        .previews()
        .iter()
        .map(|word| word.original().raw_text())
        .collect();
    assert_eq!(originals, ["Ro(se)", "(Ro)se", "Rose", "Rose"]);
    let locations: Vec<_> = plan
        .previews()
        .iter()
        .map(|word| {
            let location = word.location();
            assert_eq!(
                location.spelling(),
                talkbank_transform::pseudonymize::WordSpelling::Spoken
            );
            (location.utterance(), location.word())
        })
        .collect();
    assert_eq!(locations, [(0, 1), (0, 2), (0, 3), (1, 0)]);
    assert_eq!(
        plan.morphology()
            .iter()
            .map(|review| {
                let location = review.location();
                (location.utterance(), location.word())
            })
            .collect::<Vec<_>>(),
        locations
    );
    let edits: Vec<_> = plan
        .previews()
        .iter()
        .map(|word| {
            word.edits()
                .iter()
                .map(|edit| {
                    assert_eq!(source.get(edit.range()), Some(edit.original()));
                    (edit.original(), edit.replacement())
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        edits,
        [
            vec![("Ro", "PersonA"), ("(se)", "")],
            vec![("(Ro)", "PersonA"), ("se", "")],
            vec![("Rose", "PersonA")],
            vec![("Rose", "PersonA")],
        ]
    );
    assert_eq!(plan.morphology().len(), 4);
    let mut previous_end = 0;
    for review in plan.morphology() {
        let MorphologyOutcome::Proposed {
            original,
            proposed,
            source: lemma,
        } = review.outcome()
        else {
            panic!("exact aligned names must retain their source lemma field");
        };
        assert_eq!(lemma.original(), "Rose");
        assert_eq!(source.get(lemma.range()), Some("Rose"));
        assert!(lemma.range().start > previous_end);
        previous_end = lemma.range().end;
        assert_eq!(original.main.lemma.as_ref(), lemma.original());
        assert_eq!(proposed.main.lemma.as_ref(), "PersonA");
        assert_eq!(original.main.pos, proposed.main.pos);
        assert_eq!(original.main.features, proposed.main.features);
        assert_eq!(original.post_clitics, proposed.post_clitics);
    }
    let first = input.document().document().utterances().next().unwrap();
    let unchanged = &first.mor_tier().unwrap().items()[0];
    assert_eq!(unchanged.main.lemma.as_ref(), "he");
    assert_eq!(unchanged.post_clitics[0].lemma.as_ref(), "be");
    assert!(plan.lemma_findings().is_empty());
    assert!(plan.refusals().is_empty());
}

#[test]
fn canonical_reference_binds_source_mapping_and_alignment_evidence() {
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    let input = names
        .admit_document(
            REFERENCE,
            TranscriptName::Named(FileStem::from_stem("eng-conversation")),
            RuleSelection::new().with_strict_linkers(),
            &parser,
        )
        .unwrap();
    assert!(std::ptr::eq(input.source(), REFERENCE));
    assert!(std::ptr::eq(input.parsed_source().source(), REFERENCE));
    let root = input.parsed_source().root_node();
    assert_eq!(root.end_byte(), REFERENCE.len());
    assert!(!root.has_error());
    assert!(input.parsed_source().bind(root).is_ok());
    assert!(std::ptr::eq(input.names(), names));
    assert_eq!(
        input.document().policy().alignment(),
        AlignmentValidation::IncludeTierAlignment
    );
    assert!(input.document().policy().rules().strict_linkers_enabled());
    assert!(input.document().diagnostics().is_empty());
    assert_eq!(format!("{input:?}"), "PseudonymizationInput(<private>)");
}

#[test]
fn admitted_output_matches_authored_golden_and_preserves_every_unedited_byte() {
    use talkbank_transform::pseudonymize::{EditKind, FreeTextDecision};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/word-features/pseudonymizer-source.cha"
    ));
    let expected = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/word-features/pseudonymizer-expected.cha"
    ));
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    let input = names
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new().with_strict_linkers(),
            &parser,
        )
        .unwrap();
    let output = input.prepare_output(&parser).unwrap();
    assert_eq!(output.text(), expected);
    assert_eq!(output.document().policy(), input.document().policy());
    assert_eq!(output.document().name(), input.document().name());
    assert!(output.document().diagnostics().is_empty());
    assert_eq!(format!("{output:?}"), "PseudonymizedDocument(<private>)");
    let mut input_cursor = 0;
    let mut output_cursor = 0;
    let mut kinds = [0; 5];
    for edit in output.edits() {
        let before = edit.original_range();
        let after = edit.output_range();
        assert_eq!(source.get(before.clone()), Some(edit.original()));
        assert_eq!(output.text().get(after.clone()), Some(edit.replacement()));
        assert_eq!(
            &source[input_cursor..before.start],
            &output.text()[output_cursor..after.start]
        );
        input_cursor = before.end;
        output_cursor = after.end;
        kinds[match edit.kind() {
            EditKind::MainWord => 0,
            EditKind::Morphology => 1,
            EditKind::Timing => 2,
            EditKind::Header(_) => 3,
            EditKind::Prose => 4,
        }] += 1;
    }
    assert_eq!(&source[input_cursor..], &output.text()[output_cursor..]);
    assert_eq!(kinds, [5, 2, 4, 3, 1]);
    let metadata: Vec<_> = output
        .edits()
        .iter()
        .filter_map(|edit| match edit.origin() {
            talkbank_transform::pseudonymize::EditOrigin::Header(location) => {
                Some((location.header(), location.field()))
            }
            _ => None,
        })
        .collect();
    use talkbank_transform::pseudonymize::{HeaderNameField, ProseLocation};
    assert_eq!(
        metadata,
        [
            (3, HeaderNameField::Participant),
            (4, HeaderNameField::Group),
            (4, HeaderNameField::Custom)
        ]
    );
    let prose: Vec<_> = output
        .edits()
        .iter()
        .filter_map(|edit| match edit.origin() {
            talkbank_transform::pseudonymize::EditOrigin::Prose(location) => Some(location),
            _ => None,
        })
        .collect();
    assert_eq!(prose, [ProseLocation::Header(5)]);
    use talkbank_transform::pseudonymize::{EditOrigin, WordSpelling};
    let origins: Vec<_> = output
        .edits()
        .iter()
        .filter_map(|edit| {
            let location = match edit.origin() {
                EditOrigin::MainWord(location)
                | EditOrigin::Morphology(location)
                | EditOrigin::Timing(location) => location,
                EditOrigin::Header(_) | EditOrigin::Prose(_) => return None,
            };
            assert_eq!(location.spelling(), WordSpelling::Spoken);
            Some((edit.kind(), location.utterance(), location.word()))
        })
        .collect();
    assert_eq!(
        origins,
        [
            (EditKind::MainWord, 0, 0),
            (EditKind::MainWord, 0, 0),
            (EditKind::MainWord, 0, 1),
            (EditKind::MainWord, 0, 1),
            (EditKind::Morphology, 0, 0),
            (EditKind::Morphology, 0, 1),
            (EditKind::Timing, 0, 0),
            (EditKind::Timing, 0, 1),
            (EditKind::Timing, 0, 1),
            (EditKind::MainWord, 1, 0),
            (EditKind::Timing, 1, 0),
        ]
    );
    assert!(
        output
            .plan()
            .free_text()
            .unwrap()
            .iter()
            .any(|finding| matches!(finding.decision(), FreeTextDecision::PossessiveReview))
    );
    let again = input.prepare_output(&parser).unwrap();
    assert_eq!(again.text(), expected, "deterministic from the same input");
    let second_input = names
        .admit_document(
            output.text(),
            output.document().name(),
            output.document().policy().rules(),
            &parser,
        )
        .unwrap();
    let second = second_input.prepare_output(&parser).unwrap();
    assert_eq!(second.text(), expected);
    assert!(
        second.edits().is_empty(),
        "idempotent output has no second-pass replacements"
    );
}

#[test]
fn unicode_prose_findings_keep_possessives_and_source_boundaries() {
    use talkbank_transform::pseudonymize::{FreeTextDecision, FreeTextOwner};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/headers/free-text-names.cha"
    ));
    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'Rose'\nreplacement = 'PersonA'\n\
         [[transcripts.names]]\noriginal = 'Élodie'\nreplacement = 'PersonB'\n\
         [[transcripts.names]]\noriginal = 'CHI'\nreplacement = 'PersonA'\n\
         [[transcripts.names]]\noriginal = 'Child'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    let findings = plan.free_text().unwrap();
    let mut kinds = [0; 3];
    let mut owners = [0; 2];
    let mut previous_end = 0;
    let originals: Vec<_> = findings
        .iter()
        .map(|finding| {
            assert_eq!(source.get(finding.range()), Some(finding.original()));
            assert!(finding.range().start >= previous_end);
            previous_end = finding.range().end;
            match finding.owner() {
                FreeTextOwner::Header(_) => owners[0] += 1,
                FreeTextOwner::DependentTier(_) => owners[1] += 1,
            }
            match finding.decision() {
                FreeTextDecision::Replace(text) => {
                    kinds[0] += 1;
                    assert_eq!(
                        text.as_ref(),
                        if finding.original() == "Élodie" {
                            "PersonB"
                        } else {
                            "PersonA"
                        }
                    );
                }
                FreeTextDecision::CaseNearMiss => kinds[1] += 1,
                FreeTextDecision::PossessiveReview => kinds[2] += 1,
            }
            finding.original()
        })
        .collect();
    assert_eq!(
        originals,
        [
            "Rose", "rose", "ROSE", "Élodie", "Rose’s", "Rose's", "Rose", "Élodie", "Rose", "Rose",
            "Élodie", "rose's", "Rose", "Rose", "Rose",
        ]
    );
    assert_eq!(kinds, [10, 2, 3]);
    assert_eq!(owners, [9, 6]);
    use talkbank_transform::pseudonymize::ProseLocation;
    assert_eq!(
        findings
            .iter()
            .map(|finding| finding.location())
            .collect::<Vec<_>>(),
        [
            ProseLocation::Header(6),
            ProseLocation::Header(6),
            ProseLocation::Header(6),
            ProseLocation::Header(6),
            ProseLocation::Header(6),
            ProseLocation::Header(6),
            ProseLocation::Header(7),
            ProseLocation::Header(7),
            ProseLocation::Header(7),
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 0
            },
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 0
            },
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 1
            },
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 1
            },
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 2
            },
            ProseLocation::DependentTier {
                utterance: 0,
                tier: 3
            },
        ]
    );
    assert!(plan.previews().is_empty());
}

#[test]
fn name_map_refuses_placeholders_that_reintroduce_names_at_unicode_boundaries() {
    use talkbank_transform::pseudonymize::NameMapError;
    let parser = TreeSitterParser::new().unwrap();
    for replacement in ["PersonA-Rose", "PersonA-rose"] {
        let error = NameMap::from_toml(
            &format!(
                "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'Rose'\nreplacement = '{replacement}'\n"
            ),
            &parser,
        )
        .unwrap_err();
        assert_eq!(error, NameMapError::ReplacementCollision, "{replacement}");
        assert!(!format!("{error:?} {error}").contains("Rose"));
    }
}

#[test]
fn unicode_boundaries_match_complete_hyphenated_names_without_partial_near_miss_rewrites() {
    use talkbank_transform::pseudonymize::FreeTextDecision;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/headers/unicode-name-boundaries.cha"
    ));
    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'Rose'\nreplacement = 'PersonA'\n\
         [[transcripts.names]]\noriginal = 'Rose-Marie'\nreplacement = 'PersonB'\n",
        &parser,
    )
    .unwrap();
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let output = input.prepare_output(&parser).unwrap();
    let header = output.plan().header_fields().unwrap();
    assert_eq!(
        header
            .iter()
            .map(|field| field.original())
            .collect::<Vec<_>>(),
        [
            "Rose-Marie",
            "Rose-Marie",
            "rose-marie",
            "Rose-Marie’s",
            "Rose"
        ]
    );
    assert!(matches!(
        header[2].decision(),
        FreeTextDecision::CaseNearMiss
    ));
    assert!(matches!(
        header[3].decision(),
        FreeTextDecision::PossessiveReview
    ));
    let prose = output.plan().free_text().unwrap();
    assert_eq!(
        prose
            .iter()
            .map(|field| field.original())
            .collect::<Vec<_>>(),
        ["Rose-Marie", "Rose", "Rose-marie", "Rose-Marie's"]
    );
    assert!(matches!(
        prose[2].decision(),
        FreeTextDecision::CaseNearMiss
    ));
    assert!(matches!(
        prose[3].decision(),
        FreeTextDecision::PossessiveReview
    ));
    assert!(output.text().contains("@Participants:\tCHI PersonB Child"));
    assert!(
        output
            .text()
            .contains("|PersonB rose-marie Rose-Marie’s PersonA Rosebud|")
    );
    assert!(
        output
            .text()
            .contains("“PersonB” met PersonA. Rose-marie kept Rose-Marie's book.")
    );
    assert!(output.text().contains("*CHI:\tPersonB PersonA ."));
    assert_eq!(output.edits().len(), 7);
}

#[test]
fn lexical_source_edits_exclude_markers_and_unselected_compound_partners() {
    use talkbank_transform::pseudonymize::{TimingOutcome, TimingRefusal, WordRefusal};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/word-features/selective-name-fields.cha"
    ));
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    assert!(plan.refusals().is_empty());
    let mut actual = Vec::new();
    assert_eq!(plan.previews().len(), 4);
    for preview in &plan.previews()[..3] {
        let mut local = String::new();
        let span = preview.original().span;
        let mut cursor = span.start as usize;
        let mut fields = Vec::new();
        // Exercise the proposed source edits at the wire boundary. This is not
        // a production writer: only this word is compared with its typed preview.
        for edit in preview.edits() {
            let range = edit.range();
            assert_eq!(source.get(range.clone()), Some(edit.original()));
            local.push_str(&source[cursor..range.start]);
            local.push_str(edit.replacement());
            cursor = range.end;
            fields.push((edit.original(), edit.replacement()));
        }
        local.push_str(&source[cursor..span.end as usize]);
        assert_eq!(local, preview.proposed().to_chat());
        actual.push(fields);
    }
    assert_eq!(
        actual,
        [
            vec![("Ro", "PersonA"), ("se", "")],
            vec![("Rose", "PersonA")],
            vec![("Rose", "PersonA")],
        ]
    );
    let proposals: Vec<_> = plan
        .previews()
        .iter()
        .take(3)
        .map(|word| word.proposed().to_chat())
        .collect();
    assert_eq!(proposals, ["PersonA:", "PersonA+bud", "flower+PersonA"]);
    let TimingOutcome::Corroborated(changes) = plan.timing()[0].outcome() else {
        panic!("same typed component structure must transfer");
    };
    let timing_proposals: Vec<_> = changes
        .iter()
        .map(|word| word.proposed().to_chat())
        .collect();
    assert_eq!(timing_proposals, proposals);
    let timing_fields: Vec<_> = changes
        .iter()
        .map(|word| {
            word.edits()
                .iter()
                .map(|edit| {
                    assert_eq!(source.get(edit.range()), Some(edit.original()));
                    (edit.original(), edit.replacement())
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(timing_fields, actual);
    assert!(matches!(
        plan.timing()[1].outcome(),
        TimingOutcome::Refused(TimingRefusal::UnsafeWord(
            WordRefusal::ComponentCorrespondence
        ))
    ));
    assert!(
        matches!(plan.timing()[2].outcome(), TimingOutcome::Corroborated(changes) if changes.is_empty()),
        "a name appearing only as a timing-tier component is not independently replaced"
    );
    let refused = input.prepare_output(&parser).unwrap_err();
    assert_eq!(
        refused.reason(),
        talkbank_transform::pseudonymize::OutputRefusalReason::UnsafePlan
    );
    assert_eq!(
        refused.plan().previews().len(),
        4,
        "retain review but expose no partial output"
    );
}

#[test]
fn malformed_and_misaligned_specs_cannot_enter_planning() {
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    for (source, expected) in [
        (MOR_SYNTAX, InputRefusal::Parsing),
        (MOR_COUNT, InputRefusal::Validation),
    ] {
        let error = names
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!format!("{error:?} {error}").contains("hello"));
        assert!(!format!("{error:?} {error}").contains("cookie"));
    }
}

#[test]
fn authored_filename_context_is_not_silently_discarded() {
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    assert!(
        names
            .admit_document(
                NAMED_MEDIA,
                TranscriptName::Named(FileStem::from_stem("media_sample")),
                RuleSelection::new(),
                &parser,
            )
            .is_ok()
    );
    assert_eq!(
        names
            .admit_document(
                NAMED_MEDIA,
                TranscriptName::Named(FileStem::from_stem("other_sample")),
                RuleSelection::new(),
                &parser,
            )
            .unwrap_err(),
        InputRefusal::Validation
    );
}

#[test]
fn lexical_view_retains_shortening_boundaries_and_repeated_sound() {
    use std::collections::BTreeSet;
    use talkbank_model::WordContent;
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::content::word::LexicalContribution;

    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    let sources = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/shortenings-in-words.cha"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/words-markers.cha"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/ca/nonvocal-and-long-features.cha"
        )),
    ];
    let expected = [
        ("parc(e)", "parce", "", "", 1),
        ("do(n't)", "don't", "", "", 1),
        ("ice+cream", "icecream", "", "+", 0),
        ("le~ha", "leha", "", "~", 0),
        ("↫s-s-s↫segment", "segment", "s-s-s", "", 0),
        ("∆faster∆", "faster", "", "", 0),
    ];
    let mut witnessed = BTreeSet::new();
    for source in sources {
        let input = names
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        for utterance in input.document().document().utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => return,
                };
                let Some(&(raw, spoken, repeated, boundaries, shortenings)) =
                    expected.iter().find(|row| row.0 == word.raw_text())
                else {
                    return;
                };
                let mut actual_spoken = String::new();
                let mut actual_repeated = String::new();
                let mut actual_boundaries = String::new();
                let mut actual_shortenings = 0;
                let parts: Vec<_> = word.lexical_parts().collect();
                assert_eq!(parts.len(), word.content().len());
                for (part, original) in parts.iter().zip(word.content().iter()) {
                    assert!(std::ptr::eq(part.content(), original));
                    match part.contribution() {
                        LexicalContribution::Spoken(text) => actual_spoken.push_str(text),
                        LexicalContribution::Repeated(text) => actual_repeated.push_str(text),
                        LexicalContribution::Structural => {}
                    }
                    match part.content() {
                        WordContent::CompoundMarker(_) => actual_boundaries.push('+'),
                        WordContent::CliticBoundary(_) => actual_boundaries.push('~'),
                        WordContent::Shortening(_) => actual_shortenings += 1,
                        _ => {}
                    }
                }
                assert_eq!(actual_spoken, spoken, "{raw}");
                assert_eq!(actual_repeated, repeated, "{raw}");
                assert_eq!(actual_boundaries, boundaries, "{raw}");
                assert_eq!(actual_shortenings, shortenings, "{raw}");
                assert_eq!(word.cleaned_text(), spoken, "{raw}");
                witnessed.insert(raw);
            });
        }
    }
    assert_eq!(witnessed.len(), expected.len(), "missing canonical witness");
}

#[test]
fn morphology_positions_select_corrections_and_exclude_retraced_words() {
    use talkbank_model::alignment::helpers::visit_mor_positions;

    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/retrace.cha"
    ));
    let input = names
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let expected: &[&[&str]] = &[
        &["kitty", "is", "nice"],
        &["later", "in", "the", "day"],
        &["female"],
    ];
    let mut witnessed = Vec::new();
    for utterance in input.document().document().utterances() {
        let Some(mor) = utterance.mor_tier() else {
            continue;
        };
        let mut words = Vec::new();
        visit_mor_positions(&utterance.main.content.content, &mut |position| {
            assert_eq!(position.index().as_usize(), words.len());
            words.push(
                position
                    .word()
                    .expect("authored examples have no separators")
                    .cleaned_text(),
            );
        });
        assert_eq!(words.len(), mor.items().len());
        witnessed.push(words);
    }
    assert_eq!(witnessed, expected);
}

#[test]
fn document_lexical_plan_includes_retraced_replacement_targets_without_writing() {
    use talkbank_model::WriteChat;
    use talkbank_transform::pseudonymize::{ComponentDecision, WordRefusal};

    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'female'\nreplacement = 'PersonA'\n\
         [[transcripts.names]]\noriginal = 'tika'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/retrace.cha"
    ));
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let before = input.document().to_chat_string();
    let plan = input.plan_words();
    assert_eq!(format!("{plan:?}"), "LexicalPlan(<private>)");
    assert!(std::ptr::eq(plan.source(), source));
    assert_eq!(plan.previews().len(), 2);
    for (index, preview) in plan.previews().iter().enumerate() {
        let location = preview.location();
        assert_eq!((location.utterance(), location.word()), (7, index));
        assert_eq!(
            location.spelling(),
            talkbank_transform::pseudonymize::WordSpelling::ReplacementTarget(0)
        );
    }
    for preview in plan.previews() {
        assert_eq!(preview.original().raw_text(), "female");
        assert_eq!(preview.proposed().to_chat(), "PersonA");
        let span = preview.original().span;
        assert_eq!(
            source.get(span.start as usize..span.end as usize),
            Some("female")
        );
        assert!(preview.decisions().any(|decision| matches!(decision,
            ComponentDecision::Replace { original, .. } if original == "female")));
    }
    assert_eq!(plan.refusals().len(), 1);
    assert_eq!(plan.refusals()[0].reason(), WordRefusal::PhoneticMaterial);
    let refused_location = plan.refusals()[0].location();
    assert_eq!(
        (refused_location.utterance(), refused_location.word()),
        (5, 0)
    );
    assert_eq!(
        refused_location.spelling(),
        talkbank_transform::pseudonymize::WordSpelling::Spoken
    );
    let refused_span = plan.refusals()[0].span();
    assert_eq!(
        source.get(refused_span.start as usize..refused_span.end as usize),
        Some("tika@u")
    );
    assert_eq!(input.document().to_chat_string(), before);
    // Both replacement targets were inspected, but only the non-retraced one
    // belongs to the morphology positional domain.
    assert_eq!(plan.morphology().len(), 1);
    let review = &plan.morphology()[0];
    assert_eq!(review.word().cleaned_text(), "female");
    let talkbank_transform::pseudonymize::MorphologyOutcome::Proposed { proposed, .. } =
        review.outcome()
    else {
        panic!("exact aligned lemma should have a proposal");
    };
    let original = review.original().unwrap();
    assert_eq!(proposed.main.lemma.as_ref(), "PersonA");
    assert_eq!(original.main.lemma.as_ref(), "female");
    assert_eq!(proposed.main.pos, original.main.pos);
    assert_eq!(proposed.main.features, original.main.features);
    assert_eq!(proposed.post_clitics, original.post_clitics);

    let unmatched = private_map(&parser);
    let unmatched_input = unmatched
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            REFERENCE,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let no_matches = unmatched_input.plan_words();
    assert!(no_matches.previews().is_empty());
    assert!(no_matches.refusals().is_empty());
    assert!(no_matches.morphology().is_empty());
}

#[test]
fn morphology_does_not_guess_a_different_aligned_lemma() {
    use talkbank_transform::pseudonymize::{MorphologyOutcome, MorphologyRefusal};

    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'later'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/retrace.cha"
    ));
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    assert_eq!(plan.morphology().len(), 1);
    let review = &plan.morphology()[0];
    assert_eq!(review.word().cleaned_text(), "later");
    assert_eq!(review.original().unwrap().main.lemma.as_ref(), "late");
    assert!(matches!(
        review.outcome(),
        MorphologyOutcome::Refused {
            reason: MorphologyRefusal::LemmaCorrespondence,
            ..
        }
    ));
    assert_eq!(
        input.prepare_output(&parser).unwrap_err().reason(),
        talkbank_transform::pseudonymize::OutputRefusalReason::UnsafePlan
    );
}

#[test]
fn timing_changes_require_corroboration_and_preserve_original_bullets() {
    use talkbank_model::WriteChat;
    use talkbank_transform::pseudonymize::{TimingOutcome, TimingRefusal};

    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'hello'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let matched = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/tiers/wor.cha"
    ));
    let drifted = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/tiers/wor-drift.cha"
    ));
    let names = map.for_transcript("sample").unwrap();
    let input = names
        .admit_document(
            matched,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let before = input.document().to_chat_string();
    let plan = input.plan_words();
    assert_eq!(plan.timing().len(), 3);
    let mut change_count = 0;
    for review in plan.timing() {
        let TimingOutcome::Corroborated(changes) = review.outcome() else {
            panic!("canonical matching timing tier should corroborate");
        };
        for change in changes {
            change_count += 1;
            assert_eq!(change.original().cleaned_text(), "hello");
            assert_eq!(change.proposed().cleaned_text(), "PersonA");
            assert_eq!(
                change.proposed().inline_bullet,
                change.original().inline_bullet
            );
            assert!(change.proposed().inline_bullet.is_some());
            assert_eq!(change.proposed().span, change.original().span);
            assert_eq!(change.proposed().word_id, change.original().word_id);
            assert_eq!(change.edits().len(), 1);
            let edit = &change.edits()[0];
            assert_eq!(matched.get(edit.range()), Some("hello"));
            assert_eq!(edit.replacement(), "PersonA");
        }
    }
    assert_eq!(change_count, 1);
    assert_eq!(input.document().to_chat_string(), before);

    let input = names
        .admit_document(
            drifted,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    assert_eq!(plan.timing().len(), 3);
    for (review, expected) in plan.timing().iter().zip([
        TimingRefusal::CountDrift,
        TimingRefusal::CountDrift,
        TimingRefusal::LexicalDrift,
    ]) {
        assert!(matches!(review.outcome(), TimingOutcome::Refused(reason) if *reason == expected));
    }
}

#[test]
fn pronunciation_is_refused_without_inventing_placeholder_sounds() {
    use talkbank_model::WriteChat;
    use talkbank_transform::pseudonymize::PronunciationEvidence;

    let parser = TreeSitterParser::new().unwrap();
    for (source, name, direct, companions) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/reference/tiers/pho.cha"
            )),
            "Mommy",
            2,
            0,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/reference/annotation/groups-phonological.cha"
            )),
            "A",
            2,
            0,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/reference/tiers/phon-intervals.cha"
            )),
            "brillig",
            2,
            4,
        ),
    ] {
        let map = NameMap::from_toml(
            &format!(
                "version = 1\n[[transcripts]]\nkey = 'sample'\n\
             [[transcripts.names]]\noriginal = '{name}'\nreplacement = 'PersonA'\n"
            ),
            &parser,
        )
        .unwrap();
        let input = map
            .for_transcript("sample")
            .unwrap()
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        let before = input.document().to_chat_string();
        let plan = input.plan_words();
        let mut aligned = 0;
        let mut review = 0;
        for refusal in plan.pronunciation() {
            assert_eq!(refusal.word().cleaned_text(), name);
            match refusal.evidence() {
                PronunciationEvidence::AlignedItem(item) => {
                    aligned += 1;
                    let expected = match (name, refusal.tier()) {
                        ("Mommy", talkbank_model::DependentTier::Mod(_)) => "ˈmɑmiː",
                        ("Mommy", talkbank_model::DependentTier::Pho(_)) => "ˈɑmɪ",
                        ("A", talkbank_model::DependentTier::Mod(_)) => "d",
                        ("A", talkbank_model::DependentTier::Pho(_)) => "a",
                        ("brillig", _) => "bɹɪlɪɡ",
                        _ => panic!("unexpected pronunciation witness"),
                    };
                    assert_eq!(item.to_chat_string(), expected);
                }
                PronunciationEvidence::CompanionReview => review += 1,
                PronunciationEvidence::MissingAlignment => panic!("reference has alignment"),
            }
        }
        assert_eq!((aligned, review), (direct, companions));
        let refused = input.prepare_output(&parser).unwrap_err();
        assert_eq!(
            refused.reason(),
            talkbank_transform::pseudonymize::OutputRefusalReason::UnsafePlan
        );
        assert_eq!(refused.plan().pronunciation().len(), direct + companions);
        assert_eq!(input.document().to_chat_string(), before);
    }
}

#[test]
fn phonology_word_positions_preserve_atomic_group_membership() {
    use talkbank_model::alignment::helpers::visit_pho_words;
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/groups-phonological.cha"
    ));
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let utterance = input.document().document().utterances().next().unwrap();
    let mut positions = Vec::new();
    visit_pho_words(&utterance.main.content.content, &mut |position| {
        positions.push((position.index().as_usize(), position.word().cleaned_text()));
    });
    assert_eq!(
        positions,
        [(0, "non"), (1, "il"), (1, "y"), (1, "a"), (2, "pas")]
    );
    assert!(input.plan_words().pronunciation().is_empty());
}

#[test]
fn unreplaced_lemmas_are_findings_not_independent_rewrites() {
    use talkbank_model::WriteChat;
    use talkbank_transform::pseudonymize::{
        LemmaFindingKind, MorphologyOutcome, MorphologyRefusal,
    };
    let parser = TreeSitterParser::new().unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/tiers/mor-selective-names.cha"
    ));
    let mut wire = String::from("version = 1\n[[transcripts]]\nkey = 'sample'\n");
    for name in ["ice", "late", "be", "Rose"] {
        wire.push_str(&format!(
            "[[transcripts.names]]\noriginal = '{name}'\nreplacement = 'PersonA'\n"
        ));
    }
    let map = NameMap::from_toml(&wire, &parser).unwrap();
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let before = input.document().to_chat_string();
    let plan = input.plan_words();
    assert_eq!(plan.previews().len(), 2);
    assert_eq!(plan.previews()[0].proposed().to_chat(), "PersonA+cream");
    assert_eq!(plan.morphology().len(), 2);
    assert!(matches!(
        plan.morphology()[0].outcome(),
        MorphologyOutcome::Refused {
            reason: MorphologyRefusal::ComponentCorrespondence,
            ..
        }
    ));
    assert!(
        matches!(plan.morphology()[1].outcome(), MorphologyOutcome::Proposed {
        original, proposed, ..
    } if original.main.lemma.as_ref() == "Rose" && proposed.main.lemma.as_ref() == "PersonA")
    );
    let findings: Vec<_> = plan
        .lemma_findings()
        .iter()
        .map(|finding| {
            assert!(
                source
                    .get(finding.tier().span.start as usize..finding.tier().span.end as usize)
                    .is_some()
            );
            (finding.word().lemma.as_ref(), finding.kind())
        })
        .collect();
    assert_eq!(
        findings,
        [
            ("late", LemmaFindingKind::UnreplacedMatch),
            ("be", LemmaFindingKind::UnreplacedMatch)
        ]
    );
    use talkbank_transform::pseudonymize::LemmaPart;
    let locations: Vec<_> = plan
        .lemma_findings()
        .iter()
        .map(|finding| {
            let location = finding.location();
            (location.utterance(), location.item(), location.part())
        })
        .collect();
    assert_eq!(
        locations,
        [(1, 0, LemmaPart::Main), (2, 0, LemmaPart::PostClitic(0))]
    );
    assert_eq!(input.document().to_chat_string(), before);

    let near = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'LATE'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let input = near
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    assert!(plan.morphology().is_empty());
    assert_eq!(plan.lemma_findings().len(), 1);
    assert_eq!(
        plan.lemma_findings()[0].kind(),
        LemmaFindingKind::CaseNearMiss
    );
}

#[test]
fn header_names_keep_generated_source_ranges_and_exclude_codes_and_roles() {
    use talkbank_transform::pseudonymize::{FreeTextDecision, HeaderNameField};
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/headers/name-fields.cha"
    ));
    let input = map
        .for_transcript("sample")
        .unwrap()
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let plan = input.plan_words();
    let fields = plan.header_fields().unwrap();
    assert_eq!(fields.len(), 8);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.location().header())
            .collect::<Vec<_>>(),
        [3, 3, 4, 4, 4, 4, 5, 5]
    );
    assert_eq!(
        fields.iter().map(|field| field.field()).collect::<Vec<_>>(),
        [
            HeaderNameField::Participant,
            HeaderNameField::Participant,
            HeaderNameField::Group,
            HeaderNameField::Education,
            HeaderNameField::Custom,
            HeaderNameField::Custom,
            HeaderNameField::Custom,
            HeaderNameField::Custom
        ]
    );
    for index in [0, 2, 3, 4, 5] {
        let field = &fields[index];
        assert_eq!(field.original(), "Rose");
        assert_eq!(source.get(field.range()), Some("Rose"));
        assert!(
            matches!(field.decision(), FreeTextDecision::Replace(text) if text.as_ref() == "PersonA")
        );
    }
    for index in [1, 6] {
        assert_eq!(source.get(fields[index].range()), Some("rose"));
        assert!(matches!(
            fields[index].decision(),
            FreeTextDecision::CaseNearMiss
        ));
    }
    assert!(source[fields[0].range().end..].starts_with("  Marie Child"));
    assert_eq!(source.get(fields[7].range()), Some("Rose’s"));
    assert!(matches!(
        fields[7].decision(),
        FreeTextDecision::PossessiveReview
    ));
    assert!(
        fields
            .windows(2)
            .all(|pair| pair[0].range().end <= pair[1].range().start)
    );
    assert!(
        plan.free_text().unwrap().is_empty(),
        "metadata findings have only one owner"
    );
    assert!(plan.previews().is_empty());

    for excluded in ["CHI", "Child", "Mother"] {
        let map = NameMap::from_toml(
            &format!(
                "version = 1\n[[transcripts]]\nkey = 'sample'\n\
             [[transcripts.names]]\noriginal = '{excluded}'\nreplacement = 'PersonA'\n"
            ),
            &parser,
        )
        .unwrap();
        let input = map
            .for_transcript("sample")
            .unwrap()
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        assert!(input.plan_words().header_fields().unwrap().is_empty());
    }
}

#[test]
fn source_retention_does_not_admit_controls_recovery_or_unreviewed_warnings() {
    let parser = TreeSitterParser::new().unwrap();
    let map = private_map(&parser);
    let names = map.for_transcript("sample").unwrap();
    let empty = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/error_corpus/validation_errors/E003_1.cha"
    ));
    let diagnostics = talkbank_model::ErrorCollector::new();
    let (_, parsed) = parser.parse_chat_file_with_source(empty, &diagnostics);
    assert!(parsed.is_some(), "empty-source recovery can retain its CST");
    assert!(diagnostics.has_errors());
    assert_eq!(
        names
            .admit_document(
                empty,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser
            )
            .unwrap_err(),
        InputRefusal::Parsing
    );
    let control_in_text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/error_corpus/validation_errors/E315_2.cha"
    ));
    let missing_role = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/error_corpus/validation_errors/E513_1.cha"
    ));
    for source in [control_in_text, missing_role] {
        let diagnostics = talkbank_model::ErrorCollector::new();
        let (_model, parsed) = parser.parse_chat_file_with_source(source, &diagnostics);
        assert!(parsed.is_some(), "recovery retains its producing CST");
        assert!(diagnostics.has_errors());
        assert_eq!(
            names
                .admit_document(
                    source,
                    TranscriptName::Anonymous,
                    RuleSelection::new(),
                    &parser
                )
                .unwrap_err(),
            InputRefusal::Parsing
        );
    }

    let warning = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/error_corpus/validation_errors/W109_2.cha"
    ));
    let canonical = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/error_corpus/validation_errors/W109_4.cha"
    ));
    let identity = TranscriptName::Named(FileStem::from_stem("Schlüssel"));
    let error = names
        .admit_document(warning, identity, RuleSelection::new(), &parser)
        .unwrap_err();
    assert_eq!(error, InputRefusal::DiagnosticReview);
    assert!(std::error::Error::source(&error).is_none());
    assert!(!error.to_string().contains("Schlüssel"));
    let identity = TranscriptName::Named(FileStem::from_stem("Schlüssel"));
    let accepted = names
        .admit_document(canonical, identity, RuleSelection::new(), &parser)
        .unwrap();
    assert!(accepted.document().diagnostics().is_empty());
    assert!(std::ptr::eq(accepted.parsed_source().source(), canonical));
}
