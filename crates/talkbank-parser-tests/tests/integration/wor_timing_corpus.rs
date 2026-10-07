//! Timing capabilities are reached through their full source-backed transition chain.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::ErrorCollector;
use talkbank_model::alignment::{
    WorAdjacentTimingRelation, WorSlotMembershipPolicy, WorSlotTiming, WorTimingBinding,
    WorTimingCorrespondence, WorTimingSequence, WorTimingSequenceIssue, WorTimingSidecar,
    assess_wor_timing_sequence, bind_wor_timing, corroborate_wor_timing,
    resolve_wor_timing_sidecar,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};

/// Generated display tiers use the same projection owner as timing binding.
/// Utterance-level timing must not be distributed into invented word intervals.
#[test]
fn reference_wor_generation_preserves_projection_without_inventing_word_timing() {
    use talkbank_model::model::dependent_tier::wor::WorItem;
    use talkbank_model::model::{SemanticEq, WriteChat};
    use talkbank_parser_tests::test_error::strict_parse;

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut word_slots = 0;
    let mut empty_projections = 0;
    let mut timed_turns_without_word_timing = 0;
    let mut separators = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let projection = utterance.main.wor_projection();
            assert_eq!(
                projection.membership_policy(),
                WorSlotMembershipPolicy::FilteredLexicalV1
            );
            let count = projection.slot_count();
            assert_eq!(count.to_string(), count.get().to_string());
            let generated = projection.generate_tier();
            assert_eq!(generated.word_count(), count.get());
            assert_eq!(
                generated.language_code,
                utterance.main.content.language_code
            );
            assert_eq!(generated.terminator, utterance.main.content.terminator);
            assert!(
                generated.span.is_dummy(),
                "generation does not invent a source location"
            );
            let convenience = utterance.main.generate_wor_tier();
            assert!(convenience.semantic_eq(&generated));
            assert_eq!(convenience.to_chat_string(), generated.to_chat_string());
            for item in &generated.items {
                if let WorItem::Separator { .. } = item {
                    separators += 1;
                }
            }
            let WorTimingBinding::CountMatched(matched) = projection.bind_timing(Some(&generated))
            else {
                panic!("generation drifted from its owning projection");
            };
            let WorTimingCorrespondence::Corroborated(corroborated) =
                corroborate_wor_timing(matched)
            else {
                panic!("generated display words must corroborate their main-tier owner");
            };
            assert_eq!(corroborated.slots().len(), count.get());
            let mut all_unaligned = true;
            for slot in corroborated.slots() {
                assert_eq!(slot.wor_word().cleaned_text(), slot.main_text());
                assert_eq!(
                    slot.wor_word().inline_bullet,
                    slot.main_word().inline_bullet
                );
                match slot.timing() {
                    WorSlotTiming::Unaligned => assert!(slot.main_word().inline_bullet.is_none()),
                    WorSlotTiming::Timed(_) => {
                        assert!(slot.main_word().inline_bullet.is_some());
                        all_unaligned = false;
                    }
                }
            }
            word_slots += count.get();
            empty_projections += usize::from(count.get() == 0);
            if utterance.main.content.bullet.is_some() && count.get() > 0 && all_unaligned {
                timed_turns_without_word_timing += 1;
            }
        }
    }
    assert!(word_slots > 0 && empty_projections > 0);
    assert!(
        timed_turns_without_word_timing > 0,
        "utterance timing is not word timing"
    );
    assert!(
        separators > 0,
        "reference tag separators remain in generated display tiers"
    );
}

/// Presence is a borrowed structural observation, not whole-file admission.
/// The main-only, word-only and absent states derive from one canonical source.
#[test]
fn transcript_timing_presence_retains_its_document_binding() {
    use talkbank_model::model::{DependentTier, Line, TranscriptTimingEvidence};
    use talkbank_parser_tests::test_error::strict_parse;
    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let mut file = strict_parse(parser.parse_chat_file(source)).expect("canonical source parses");
    let TranscriptTimingEvidence::Recorded(recorded) = file.timing_evidence() else {
        panic!("canonical main-tier timing must be observed");
    };
    assert!(std::ptr::eq(recorded.document(), &file));
    assert_eq!(recorded.bullet().timing.end_ms, 600);
    for line in file.lines.as_mut_slice() {
        if let Line::Utterance(utterance) = line {
            utterance.main.content.bullet = None;
        }
    }
    let TranscriptTimingEvidence::Recorded(recorded) = file.timing_evidence() else {
        panic!("actual word-only timing is still evidence");
    };
    assert!(std::ptr::eq(recorded.document(), &file));
    assert_eq!(recorded.bullet().timing.end_ms, 300);
    for line in file.lines.as_mut_slice() {
        if let Line::Utterance(utterance) = line {
            utterance
                .dependent_tiers
                .retain(|entry| !matches!(&entry.tier, DependentTier::Wor(_)));
        }
    }
    assert!(matches!(
        file.timing_evidence(),
        TranscriptTimingEvidence::Absent
    ));
}

#[test]
fn canonical_wor_sequences_require_positive_complete_timing_before_exposing_hulls() {
    use talkbank_model::model::WriteChat;

    let parser = TreeSitterParser::new().expect("parser");
    let mut states = [0; 3];
    let mut geometry = [0; 4];
    let mut bindings = [0; 3];
    let mut reconstructed_bullets = 0;
    let mut timing_evidence = [0; 2];
    let mut construction_shapes = [0; 2];
    let mut lexical_refusals = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &errors);
            for utterance in file.utterances() {
                // Compatibility metadata remains count-only. Exercise its public
                // views, but never use them to admit a timing sequence below.
                if let Some(wor) = utterance.wor_tier() {
                    use talkbank_model::model::dependent_tier::wor::WorItem;
                    use talkbank_model::model::{WorTier, WorTimingEvidence};
                    let first_bullet = wor.words().find_map(|word| word.inline_bullet.as_ref());
                    match wor.timing_evidence() {
                        WorTimingEvidence::Absent => {
                            assert!(first_bullet.is_none());
                            timing_evidence[0] += 1;
                        }
                        WorTimingEvidence::Recorded(recorded) => {
                            assert!(std::ptr::eq(
                                recorded.bullet(),
                                first_bullet.expect("recorded evidence retains its source bullet"),
                            ));
                            timing_evidence[1] += 1;
                        }
                    }
                    // Admit the word-only constructor only when the entire
                    // typed item sequence consists of words. Never filter away
                    // separators to make the convenience constructor fit.
                    let words: Option<Vec<_>> = wor
                        .items
                        .iter()
                        .map(|item| match item {
                            WorItem::Word(word) => Some(word.as_ref().clone()),
                            WorItem::Separator { .. } => None,
                        })
                        .collect();
                    let rebuilt = match words {
                        Some(words) => {
                            construction_shapes[0] += 1;
                            WorTier::from_words(words)
                        }
                        None => {
                            construction_shapes[1] += 1;
                            WorTier::new(wor.items.clone())
                        }
                    }
                    .with_terminator(wor.terminator.clone())
                    .with_language_code(wor.language_code.clone())
                    .with_span(wor.span);
                    assert_eq!(
                        &rebuilt, wor,
                        "tier construction preserves all payload and spans"
                    );
                    // Reconstruction retains recorded metadata; it does not
                    // bypass the binding/corroboration/sequence checks below.
                    for word in wor.words() {
                        let mut rebuilt = word.clone();
                        let Some(bullet) = rebuilt.inline_bullet.take() else {
                            continue;
                        };
                        assert_eq!(rebuilt.cleaned_text(), word.cleaned_text());
                        rebuilt = rebuilt.with_inline_bullet(bullet);
                        assert_eq!(rebuilt.raw_text(), word.raw_text());
                        assert_eq!(rebuilt.cleaned_text(), word.cleaned_text());
                        assert_eq!(rebuilt.span, word.span);
                        assert_eq!(rebuilt.to_chat_string(), word.to_chat_string());
                        assert_eq!(
                            serde_json::to_value(&rebuilt).expect("rebuilt timing metadata"),
                            serde_json::to_value(word).expect("recorded timing metadata")
                        );
                        reconstructed_bullets += 1;
                    }
                    let legacy = resolve_wor_timing_sidecar(&utterance.main, wor);
                    let expected_count = match &legacy {
                        WorTimingSidecar::Positional { count } => Some(*count),
                        WorTimingSidecar::Drifted { .. } => None,
                    };
                    #[allow(deprecated)]
                    {
                        assert_eq!(legacy.positional_count(), expected_count);
                        assert_eq!(legacy.is_positional(), expected_count.is_some());
                    }
                }
                let matched = match bind_wor_timing(&utterance.main, utterance.wor_tier()) {
                    WorTimingBinding::CountMatched(matched) => {
                        assert_eq!(
                            matched.membership_policy(),
                            WorSlotMembershipPolicy::FilteredLexicalV1
                        );
                        assert_eq!(
                            matched.slot_count().get(),
                            utterance.wor_tier().expect("present tier").word_count()
                        );
                        bindings[2] += 1;
                        matched
                    }
                    WorTimingBinding::Missing(missing) => {
                        assert!(utterance.wor_tier().is_none(), "absence is not count drift");
                        assert_eq!(
                            missing.membership_policy(),
                            WorSlotMembershipPolicy::FilteredLexicalV1
                        );
                        assert_eq!(
                            missing.main_count(),
                            utterance.main.wor_projection().slot_count()
                        );
                        bindings[0] += 1;
                        continue;
                    }
                    WorTimingBinding::Drifted(drift) => {
                        let wor = utterance.wor_tier().expect("drift retains a present tier");
                        assert_eq!(
                            drift.membership_policy(),
                            WorSlotMembershipPolicy::FilteredLexicalV1
                        );
                        assert_eq!(
                            drift.main_count(),
                            utterance.main.wor_projection().slot_count()
                        );
                        assert_eq!(drift.wor_count().get(), wor.word_count());
                        assert_ne!(drift.main_count().get(), drift.wor_count().get());
                        bindings[1] += 1;
                        continue;
                    }
                };
                let corroborated = match corroborate_wor_timing(matched) {
                    WorTimingCorrespondence::Corroborated(corroborated) => corroborated,
                    WorTimingCorrespondence::Uncorroborated(refused) => {
                        let projection = utterance.main.wor_projection();
                        assert_eq!(refused.membership_policy(), projection.membership_policy());
                        // Compare the public refusal with the complete display
                        // projection and recorded tier, not just the first fault.
                        // This checks correspondence, not the shared membership policy.
                        let expected_tier = projection.generate_tier();
                        let recorded = utterance.wor_tier().expect("count-matched tier");
                        let expected: Vec<_> = expected_tier
                            .words()
                            .zip(recorded.words())
                            .enumerate()
                            .filter_map(|(index, (main, wor))| {
                                (main.cleaned_text() != wor.cleaned_text()).then_some((
                                    index,
                                    main.cleaned_text(),
                                    wor.cleaned_text(),
                                ))
                            })
                            .collect();
                        let actual: Vec<_> = refused
                            .mismatches()
                            .iter()
                            .map(|mismatch| {
                                (
                                    mismatch.slot().get(),
                                    mismatch.main_text(),
                                    mismatch.wor_text(),
                                )
                            })
                            .collect();
                        assert!(!actual.is_empty(), "refusal requires lexical evidence");
                        assert_eq!(actual, expected, "{}", fixture.path().display());
                        lexical_refusals += 1;
                        continue;
                    }
                };
                let policy = corroborated.membership_policy();
                let expected: Vec<_> = corroborated
                    .slots()
                    .iter()
                    .map(|slot| (slot.main_word(), slot.timing()))
                    .collect();
                let issues: Vec<_> = expected
                    .iter()
                    .enumerate()
                    .filter_map(|(index, (_, timing))| match timing {
                        WorSlotTiming::Unaligned => Some((index, None)),
                        WorSlotTiming::Timed(interval)
                            if interval.end().get() <= interval.start().get() =>
                        {
                            Some((index, Some((interval.start().get(), interval.end().get()))))
                        }
                        WorSlotTiming::Timed(_) => None,
                    })
                    .collect();
                match assess_wor_timing_sequence(corroborated) {
                    WorTimingSequence::Empty(empty) => {
                        assert!(expected.is_empty());
                        assert_eq!(empty.slot_count().get(), 0);
                        assert_eq!(empty.membership_policy(), policy);
                        states[0] += 1;
                    }
                    WorTimingSequence::Rejected(rejected) => {
                        assert!(
                            !issues.is_empty(),
                            "unjustified timing refusal: {}",
                            fixture.path().display()
                        );
                        assert_eq!(rejected.slot_count().get(), expected.len());
                        assert_eq!(rejected.membership_policy(), policy);
                        let actual: Vec<_> = rejected
                            .issues()
                            .iter()
                            .map(|issue| match issue {
                                WorTimingSequenceIssue::Unaligned { slot } => (slot.get(), None),
                                WorTimingSequenceIssue::NonPositiveInterval {
                                    slot,
                                    start,
                                    end,
                                } => (slot.get(), Some((start.get(), end.get()))),
                            })
                            .collect();
                        assert_eq!(
                            actual,
                            issues,
                            "all timing defects retained: {}",
                            fixture.path().display()
                        );
                        states[1] += 1;
                    }
                    WorTimingSequence::Complete(complete) => {
                        assert!(!expected.is_empty());
                        assert!(
                            issues.is_empty(),
                            "incomplete timing admitted: {}",
                            fixture.path().display()
                        );
                        assert_eq!(complete.membership_policy(), policy);
                        assert_eq!(complete.slots().len(), expected.len());
                        for (actual, (word, timing)) in complete.slots().iter().zip(&expected) {
                            assert!(
                                std::ptr::eq(actual.main_word(), *word),
                                "main tier retains lexical ownership"
                            );
                            assert_eq!(actual.main_text(), word.cleaned_text());
                            assert_eq!(WorSlotTiming::Timed(actual.timing()), *timing);
                            assert_eq!(
                                actual.duration().get(),
                                actual.timing().end().get() - actual.timing().start().get()
                            );
                        }
                        let start = complete
                            .slots()
                            .iter()
                            .map(|slot| slot.timing().start().get())
                            .min()
                            .expect("nonempty");
                        let end = complete
                            .slots()
                            .iter()
                            .map(|slot| slot.timing().end().get())
                            .max()
                            .expect("nonempty");
                        assert_eq!(complete.hull().start().get(), start);
                        assert_eq!(complete.hull().end().get(), end);
                        assert_eq!(complete.hull().duration().get(), end - start);
                        assert_eq!(complete.hull().start().to_string(), start.to_string());
                        assert_eq!(complete.hull().end().to_string(), end.to_string());
                        assert_eq!(
                            complete.hull().duration().to_string(),
                            (end - start).to_string()
                        );
                        assert_eq!(complete.adjacencies().len(), complete.slots().len() - 1);
                        for (index, (pair, adjacency)) in complete
                            .slots()
                            .windows(2)
                            .zip(complete.adjacencies())
                            .enumerate()
                        {
                            let previous = pair[0].timing();
                            let current = pair[1].timing();
                            let (left, right) = match adjacency {
                                WorAdjacentTimingRelation::Gap {
                                    previous_slot,
                                    current_slot,
                                    duration,
                                } => {
                                    assert!(current.start() > previous.end());
                                    assert_eq!(
                                        duration.get(),
                                        current.start().get() - previous.end().get()
                                    );
                                    geometry[0] += 1;
                                    (previous_slot, current_slot)
                                }
                                WorAdjacentTimingRelation::Touching {
                                    previous_slot,
                                    current_slot,
                                } => {
                                    assert_eq!(current.start(), previous.end());
                                    geometry[1] += 1;
                                    (previous_slot, current_slot)
                                }
                                WorAdjacentTimingRelation::Overlap {
                                    previous_slot,
                                    current_slot,
                                    duration,
                                } => {
                                    assert!(
                                        current.start() >= previous.start()
                                            && current.start() < previous.end()
                                    );
                                    assert_eq!(
                                        duration.get(),
                                        previous.end().get() - current.start().get()
                                    );
                                    geometry[2] += 1;
                                    (previous_slot, current_slot)
                                }
                                WorAdjacentTimingRelation::BackwardStart {
                                    previous_slot,
                                    current_slot,
                                    regression,
                                } => {
                                    assert!(current.start() < previous.start());
                                    assert_eq!(
                                        regression.get(),
                                        previous.start().get() - current.start().get()
                                    );
                                    geometry[3] += 1;
                                    (previous_slot, current_slot)
                                }
                            };
                            assert_eq!(left.get(), index);
                            assert_eq!(right.get(), index + 1);
                            assert_eq!(left.to_string(), index.to_string());
                            assert_eq!(right.to_string(), (index + 1).to_string());
                        }
                        states[2] += 1;
                    }
                }
            }
        }
    }
    assert!(
        reconstructed_bullets > 0,
        "recorded word timing must be exercised"
    );
    assert!(timing_evidence.iter().all(|count| *count > 0));
    assert!(
        lexical_refusals > 0,
        "canonical sources must witness lexical drift"
    );
    assert!(construction_shapes.iter().all(|count| *count > 0));
    assert!(
        bindings.iter().all(|count| *count > 0),
        "every binding state requires canonical witnesses: {bindings:?}"
    );
    assert!(
        states.iter().all(|count| *count > 0),
        "every timing state requires witnesses: {states:?}"
    );
    assert!(
        geometry.iter().all(|count| *count > 0),
        "every adjacency class requires witnesses: {geometry:?}"
    );
    eprintln!(
        "canonical bindings (missing/drifted/count-matched): {bindings:?}; timing states (empty/rejected/complete): {states:?}; geometry (gap/touch/overlap/backward): {geometry:?}"
    );
}
