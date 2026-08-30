use super::*;
use crate::Span;
use crate::model::{Bullet, Terminator, UtteranceContent, Word};

fn word(form: &str) -> UtteranceContent {
    UtteranceContent::Word(Box::new(Word::new_unchecked(form, form)))
}

fn wor_tier(forms: &[&str]) -> WorTier {
    WorTier::from_words(forms.iter().map(|f| Word::new_unchecked(*f, *f)).collect())
}

fn timed_wor_tier(entries: &[(&str, Option<(u64, u64)>)]) -> WorTier {
    WorTier::from_words(
        entries
            .iter()
            .map(|(form, timing)| {
                let word = Word::new_unchecked(*form, *form);
                match timing {
                    Some((start_ms, end_ms)) => {
                        word.with_inline_bullet(Bullet::new(*start_ms, *end_ms))
                    }
                    None => word,
                }
            })
            .collect(),
    )
}

fn corroborated<'source>(
    main: &'source MainTier,
    wor: &'source WorTier,
) -> CorroboratedWorTimings<'source> {
    let WorTimingBinding::CountMatched(count_matched) = bind_wor_timing(main, Some(wor)) else {
        panic!("test fixture must have equal current-policy slot counts");
    };
    let WorTimingCorrespondence::Corroborated(corroborated) = corroborate_wor_timing(count_matched)
    else {
        panic!("test fixture must use the canonical `%wor` display sequence");
    };
    corroborated
}

/// Perfect count match yields `Positional`.
#[test]
fn positional_when_counts_match() {
    let main = MainTier::new(
        "CHI",
        vec![word("hello"), word("world")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = wor_tier(&["hello", "world"]);

    let sidecar = resolve_wor_timing_sidecar(&main, &wor);

    assert_eq!(sidecar, WorTimingSidecar::Positional { count: 2 });
}

/// Main longer than `%wor` yields `Drifted` (not an error).
///
/// Drift is the common case after a transcript edit without
/// re-running `align`.
#[test]
fn drifted_when_main_longer() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two"), word("three")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = wor_tier(&["one", "two"]);

    let sidecar = resolve_wor_timing_sidecar(&main, &wor);

    assert_eq!(
        sidecar,
        WorTimingSidecar::Drifted {
            main_count: 3,
            wor_count: 2
        }
    );
}

/// `%wor` longer than main yields `Drifted` symmetrically.
#[test]
fn drifted_when_wor_longer() {
    let main = MainTier::new(
        "CHI",
        vec![word("one")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = wor_tier(&["one", "extra"]);

    let sidecar = resolve_wor_timing_sidecar(&main, &wor);

    assert_eq!(
        sidecar,
        WorTimingSidecar::Drifted {
            main_count: 1,
            wor_count: 2
        }
    );
}

/// Empty on both sides is still `Positional` with count 0.
#[test]
fn positional_when_both_empty() {
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
    let wor = wor_tier(&[]);

    assert_eq!(
        resolve_wor_timing_sidecar(&main, &wor),
        WorTimingSidecar::Positional { count: 0 }
    );
}

#[test]
fn corroborated_timings_represent_an_unaligned_slot_explicitly() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("one", Some((10, 20))), ("two", None)]);
    let bound = corroborated(&main, &wor);

    assert!(matches!(
        bound.slots()[1].timing(),
        WorSlotTiming::Unaligned
    ));
}

#[test]
fn drifted_timings_cannot_expose_partially_zipped_slots() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("ignored", Some((10, 20)))]);

    let WorTimingBinding::Drifted(drift) = bind_wor_timing(&main, Some(&wor)) else {
        panic!("unequal current-policy slot counts must drift");
    };

    assert_eq!(drift.main_count().get(), 2);
    assert_eq!(drift.wor_count().get(), 1);
}

#[test]
fn missing_wor_is_not_conflated_with_an_empty_count_matched_tier() {
    let main = MainTier::new(
        "CHI",
        vec![word("one")],
        Terminator::Period { span: Span::DUMMY },
    );

    let WorTimingBinding::Missing(missing) = bind_wor_timing(&main, None) else {
        panic!("an absent tier must remain explicitly missing");
    };
    assert_eq!(
        missing.membership_policy(),
        WorSlotMembershipPolicy::FilteredLexicalV1
    );
    assert_eq!(missing.main_count().get(), 1);

    let empty_main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
    let empty_wor = wor_tier(&[]);
    let WorTimingBinding::CountMatched(bound) = bind_wor_timing(&empty_main, Some(&empty_wor))
    else {
        panic!("a present empty tier with zero eligible slots must bind");
    };
    assert_eq!(bound.slot_count().get(), 0);
}

#[test]
fn complete_timing_sequence_exposes_binding_hull_and_gap_geometry() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two"), word("three")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[
        ("one", Some((10, 20))),
        ("two", Some((25, 40))),
        ("three", Some((40, 50))),
    ]);
    let bound = corroborated(&main, &wor);

    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        panic!("complete positive timings must be admitted");
    };

    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 50);
    assert_eq!(complete.hull().duration().get(), 40);
    assert_eq!(complete.slots()[0].main_text(), "one");
    assert_eq!(complete.slots()[2].timing().end().get(), 50);
    assert_eq!(complete.slots()[2].duration().get(), 10);
    assert_eq!(
        complete.adjacencies(),
        &[
            WorAdjacentTimingRelation::Gap {
                previous_slot: WorSlotIndex(0),
                current_slot: WorSlotIndex(1),
                duration: WorDurationMs(5),
            },
            WorAdjacentTimingRelation::Touching {
                previous_slot: WorSlotIndex(1),
                current_slot: WorSlotIndex(2),
            },
        ]
    );
}

#[test]
fn empty_count_matched_sequence_is_distinct_from_complete_timing() {
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
    let wor = wor_tier(&[]);
    let bound = corroborated(&main, &wor);

    let WorTimingSequence::Empty(empty) = assess_wor_timing_sequence(bound) else {
        panic!("an empty binding has no timing hull");
    };
    assert_eq!(empty.slot_count().get(), 0);
}

#[test]
fn incomplete_and_nonpositive_timings_are_rejected_with_slot_identity() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two"), word("three")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[
        ("one", Some((10, 20))),
        ("two", None),
        ("three", Some((30, 30))),
    ]);
    let bound = corroborated(&main, &wor);

    let WorTimingSequence::Rejected(rejected) = assess_wor_timing_sequence(bound) else {
        panic!("missing and zero-duration word timing must reject the sequence");
    };
    assert_eq!(rejected.slot_count().get(), 3);
    assert_eq!(
        rejected.issues(),
        &[
            WorTimingSequenceIssue::Unaligned {
                slot: WorSlotIndex(1),
            },
            WorTimingSequenceIssue::NonPositiveInterval {
                slot: WorSlotIndex(2),
                start: WorMediaOffsetMs(30),
                end: WorMediaOffsetMs(30),
            },
        ]
    );
}

#[test]
fn overlapping_adjacent_word_intervals_keep_a_hull_and_expose_geometry() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("one", Some((10, 30))), ("two", Some((25, 40)))]);
    let bound = corroborated(&main, &wor);

    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        panic!("positive overlapping intervals still have a location hull");
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
    assert_eq!(
        complete.adjacencies(),
        &[WorAdjacentTimingRelation::Overlap {
            previous_slot: WorSlotIndex(0),
            current_slot: WorSlotIndex(1),
            duration: WorDurationMs(5),
        }]
    );
}

#[test]
fn backwards_start_uses_min_max_hull_and_a_distinct_relation() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("one", Some((30, 40))), ("two", Some((10, 20)))]);
    let bound = corroborated(&main, &wor);

    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        panic!("positive backwards intervals still have a location hull");
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
    assert_eq!(
        complete.adjacencies(),
        &[WorAdjacentTimingRelation::BackwardStart {
            previous_slot: WorSlotIndex(0),
            current_slot: WorSlotIndex(1),
            regression: WorDurationMs(20),
        }]
    );
}
