//! Timing capabilities are reached through their full source-backed transition chain.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::ErrorCollector;
use talkbank_model::alignment::{
    WorAdjacentTimingRelation, WorSlotTiming, WorTimingBinding, WorTimingCorrespondence,
    WorTimingSequence, WorTimingSequenceIssue, assess_wor_timing_sequence, bind_wor_timing,
    corroborate_wor_timing,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};

#[test]
fn canonical_wor_sequences_require_positive_complete_timing_before_exposing_hulls() {
    let parser = TreeSitterParser::new().expect("parser");
    let mut states = [0; 3];
    let mut geometry = [0; 4];
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &errors);
            for utterance in file.utterances() {
                let matched = match bind_wor_timing(&utterance.main, utterance.wor_tier()) {
                    WorTimingBinding::CountMatched(matched) => matched,
                    WorTimingBinding::Missing(_) | WorTimingBinding::Drifted(_) => continue,
                };
                let corroborated = match corroborate_wor_timing(matched) {
                    WorTimingCorrespondence::Corroborated(corroborated) => corroborated,
                    WorTimingCorrespondence::Uncorroborated(_) => continue,
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
                        }
                        states[2] += 1;
                    }
                }
            }
        }
    }
    assert!(
        states.iter().all(|count| *count > 0),
        "every timing state requires witnesses: {states:?}"
    );
    assert!(
        geometry.iter().all(|count| *count > 0),
        "every adjacency class requires witnesses: {geometry:?}"
    );
    eprintln!(
        "canonical timing states (empty/rejected/complete): {states:?}; geometry (gap/touch/overlap/backward): {geometry:?}"
    );
}
