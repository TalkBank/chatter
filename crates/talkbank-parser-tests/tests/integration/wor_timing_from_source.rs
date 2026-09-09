// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! `%wor` timing binding, over a main tier and `%wor` tier the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `alignment/wor/tests.rs` built both tiers by hand for
//! twelve tests, `alignment/wor/correspondence/tests.rs` for three more and
//! `alignment/location_tests.rs` for four sidecar tests: `Word::new_unchecked("one", "one").with_inline_bullet(
//! Bullet::new(10, 20))` beside a main tier of `Word::new_unchecked("one",
//! "one")`. Each stated the CHAT it meant twice, and nothing checked that a
//! `%wor` line reading `one \u{15}10_20\u{15}` builds the word the test
//! assembled. Here each case is the two tiers as written, and the tiers are
//! whatever the parser builds.
//!
//! # The one input no parse produces
//!
//! The originals' "empty" cases held a main tier with NO content and a `%wor`
//! tier with no words. `*CHI: .` is E253, E306 and E342: an utterance with
//! nothing in it is not CHAT. The state those tests were about is ZERO
//! ELIGIBLE SLOTS, and an event-only utterance (`&=laughs .`) reaches it from
//! a parse, which additionally pins that the membership policy excludes
//! events. So the rows here are a different input arriving at the same state,
//! and say so.
//!
//! # These are METHOD RESULTS, not diagnostics
//!
//! Each test asks the binding pipeline one question and checks its answer.
//! The `Wor*` newtypes have private fields (only the pipeline mints a slot
//! index), so expectations are compared through their `get()` accessors
//! rather than by constructing the newtype, which this crate cannot do and
//! should not be able to.

use talkbank_model::alignment::{
    CorroboratedWorTimings, WorAdjacentTimingRelation, WorSlotMembershipPolicy, WorSlotTiming,
    WorTimingBinding, WorTimingCorrespondence, WorTimingSequence, WorTimingSequenceIssue,
    WorTimingSidecar, assess_wor_timing_sequence, bind_wor_timing, corroborate_wor_timing,
    resolve_wor_timing_sidecar,
};
use talkbank_model::model::{Utterance, UtteranceContent};
use talkbank_parser_tests::from_source::{Media, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

/// A `%wor` word bullet, as the transcript writes it.
fn bullet(start: u64, end: u64) -> String {
    format!("\u{15}{start}_{end}\u{15}")
}

/// Parse a whole file around one utterance, with or without a `%wor` tier,
/// and return the utterance that owns both tiers.
///
/// Every fixture here is VALID CHAT and the helper refuses any other verdict.
/// `@Media` is declared exactly when the `%wor` tier carries a bullet: a
/// `%wor` bullet with no `@Media` is E752, and `@Media` with no timing
/// evidence is E544. No main-tier bullet appears in this file, so the `%wor`
/// tier is the only surface that decides it.
fn utterance(main: &str, wor: Option<&str>) -> Result<Utterance, TestError> {
    let media = match wor {
        Some(wor) if wor.contains('\u{15}') => Media::Declared,
        Some(_) | None => Media::Undeclared,
    };
    let lines = match wor {
        Some(wor) => format!("*CHI:\t{main}\n%wor:\t{wor}"),
        None => format!("*CHI:\t{main}"),
    };
    SingleSpeaker {
        media,
        ..SingleSpeaker::english(&lines)
    }
    .utterance(&[])
}

/// The legacy count sidecar for a parsed pair.
fn sidecar(main: &str, wor: &str) -> Result<WorTimingSidecar, TestError> {
    let utterance = utterance(main, Some(wor))?;
    let wor = utterance
        .wor_tier()
        .ok_or_else(|| TestError::Failure("the fixture built no %wor tier".into()))?;
    Ok(resolve_wor_timing_sidecar(&utterance.main, wor))
}

/// Bind and corroborate a parsed pair, refusing anything short of
/// `Corroborated` as a fixture fault rather than a finding.
fn corroborated(utterance: &Utterance) -> Result<CorroboratedWorTimings<'_>, TestError> {
    let WorTimingBinding::CountMatched(count_matched) =
        bind_wor_timing(&utterance.main, utterance.wor_tier())
    else {
        return Err(TestError::Failure(
            "test fixture must have equal current-policy slot counts".into(),
        ));
    };
    match corroborate_wor_timing(count_matched) {
        WorTimingCorrespondence::Corroborated(corroborated) => Ok(corroborated),
        WorTimingCorrespondence::Uncorroborated(_) => Err(TestError::Failure(
            "test fixture must use the canonical `%wor` display sequence".into(),
        )),
    }
}

/// An adjacency relation projected onto plain values, since the slot and
/// duration newtypes cannot be constructed here.
#[derive(Debug, PartialEq, Eq)]
enum Adjacency {
    Gap(usize, usize, u64),
    Touching(usize, usize),
    Overlap(usize, usize, u64),
    BackwardStart(usize, usize, u64),
}

fn adjacencies(relations: &[WorAdjacentTimingRelation]) -> Vec<Adjacency> {
    relations
        .iter()
        .map(|relation| match *relation {
            WorAdjacentTimingRelation::Gap {
                previous_slot,
                current_slot,
                duration,
            } => Adjacency::Gap(previous_slot.get(), current_slot.get(), duration.get()),
            WorAdjacentTimingRelation::Touching {
                previous_slot,
                current_slot,
            } => Adjacency::Touching(previous_slot.get(), current_slot.get()),
            WorAdjacentTimingRelation::Overlap {
                previous_slot,
                current_slot,
                duration,
            } => Adjacency::Overlap(previous_slot.get(), current_slot.get(), duration.get()),
            WorAdjacentTimingRelation::BackwardStart {
                previous_slot,
                current_slot,
                regression,
            } => {
                Adjacency::BackwardStart(previous_slot.get(), current_slot.get(), regression.get())
            }
        })
        .collect()
}

/// A sequence issue projected onto plain values, for the same reason.
#[derive(Debug, PartialEq, Eq)]
enum Issue {
    Unaligned(usize),
    NonPositiveInterval(usize, u64, u64),
}

fn issues(issues: &[WorTimingSequenceIssue]) -> Vec<Issue> {
    issues
        .iter()
        .map(|issue| match *issue {
            WorTimingSequenceIssue::Unaligned { slot } => Issue::Unaligned(slot.get()),
            WorTimingSequenceIssue::NonPositiveInterval { slot, start, end } => {
                Issue::NonPositiveInterval(slot.get(), start.get(), end.get())
            }
        })
        .collect()
}

// ── resolve_wor_timing_sidecar ──────────────────────────────────────────

/// Perfect count match yields `Positional`.
#[test]
fn positional_when_counts_match() -> Result<(), TestError> {
    let wor = format!("hello {} world {} .", bullet(1, 2), bullet(3, 4));
    assert_eq!(
        sidecar("hello world .", &wor)?,
        WorTimingSidecar::Positional { count: 2 }
    );
    Ok(())
}

/// Main longer than `%wor` yields `Drifted` (not an error).
///
/// Drift is the common case after a transcript edit without re-running
/// `align`.
#[test]
fn drifted_when_main_longer() -> Result<(), TestError> {
    let wor = format!("one {} two {} .", bullet(1, 2), bullet(3, 4));
    assert_eq!(
        sidecar("one two three .", &wor)?,
        WorTimingSidecar::Drifted {
            main_count: 3,
            wor_count: 2
        }
    );
    Ok(())
}

/// `%wor` longer than main yields `Drifted` symmetrically.
#[test]
fn drifted_when_wor_longer() -> Result<(), TestError> {
    let wor = format!("one {} extra {} .", bullet(1, 2), bullet(3, 4));
    assert_eq!(
        sidecar("one .", &wor)?,
        WorTimingSidecar::Drifted {
            main_count: 1,
            wor_count: 2
        }
    );
    Ok(())
}

/// Zero eligible slots on both sides is still `Positional` with count 0.
///
/// The original held an EMPTY main tier, which no parse produces. An
/// event-only utterance has zero Wor-domain slots, and the parser accepts a
/// `%wor` tier holding only a terminator beside it.
#[test]
fn positional_when_both_empty() -> Result<(), TestError> {
    assert_eq!(
        sidecar("&=laughs .", ".")?,
        WorTimingSidecar::Positional { count: 0 }
    );
    Ok(())
}

/// A timed filler aligns positionally: `&-dt` on the main tier is one Wor
/// slot, and `%wor` carries one timed token for it.
///
/// From `alignment/location_tests.rs`, where it was the OCSC field-report
/// shape, hand-built; here the filler is parsed as one.
#[test]
fn positional_when_a_filler_is_timed() -> Result<(), TestError> {
    let wor = format!("dt {} there {} .", bullet(0, 120), bullet(120, 260));
    assert_eq!(
        sidecar("&-dt there .", &wor)?,
        WorTimingSidecar::Positional { count: 2 }
    );
    Ok(())
}

// ── bind_wor_timing and corroborate_wor_timing ──────────────────────────

/// A `%wor` word with no bullet is an explicitly UNALIGNED slot, not a
/// missing one.
#[test]
fn corroborated_timings_represent_an_unaligned_slot_explicitly() -> Result<(), TestError> {
    let wor = format!("one {} two .", bullet(10, 20));
    let utterance = utterance("one two .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    assert!(matches!(
        bound.slots()[1].timing(),
        WorSlotTiming::Unaligned
    ));
    Ok(())
}

/// Unequal counts drift and expose no slots, however many `%wor` words
/// carry timing.
#[test]
fn drifted_timings_cannot_expose_partially_zipped_slots() -> Result<(), TestError> {
    let wor = format!("ignored {} .", bullet(10, 20));
    let utterance = utterance("one two .", Some(&wor))?;
    let WorTimingBinding::Drifted(drift) = bind_wor_timing(&utterance.main, utterance.wor_tier())
    else {
        return Err(TestError::Failure(
            "unequal current-policy slot counts must drift".into(),
        ));
    };
    assert_eq!(drift.main_count().get(), 2);
    assert_eq!(drift.wor_count().get(), 1);
    Ok(())
}

/// An absent `%wor` tier is `Missing`; a present tier with zero slots binds
/// as count-matched at zero. The two are different states.
#[test]
fn missing_wor_is_not_conflated_with_an_empty_count_matched_tier() -> Result<(), TestError> {
    let without = utterance("one .", None)?;
    let WorTimingBinding::Missing(missing) = bind_wor_timing(&without.main, without.wor_tier())
    else {
        return Err(TestError::Failure(
            "an absent tier must remain explicitly missing".into(),
        ));
    };
    assert_eq!(
        missing.membership_policy(),
        WorSlotMembershipPolicy::FilteredLexicalV1
    );
    assert_eq!(missing.main_count().get(), 1);

    let empty = utterance("&=laughs .", Some("."))?;
    let WorTimingBinding::CountMatched(bound) = bind_wor_timing(&empty.main, empty.wor_tier())
    else {
        return Err(TestError::Failure(
            "a present empty tier with zero eligible slots must bind".into(),
        ));
    };
    assert_eq!(bound.slot_count().get(), 0);
    Ok(())
}

// ── assess_wor_timing_sequence ──────────────────────────────────────────

/// Complete positive timings expose the hull, per-slot durations, and the
/// gap and touching relations between neighbours.
#[test]
fn complete_timing_sequence_exposes_binding_hull_and_gap_geometry() -> Result<(), TestError> {
    let wor = format!(
        "one {} two {} three {} .",
        bullet(10, 20),
        bullet(25, 40),
        bullet(40, 50)
    );
    let utterance = utterance("one two three .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "complete positive timings must be admitted".into(),
        ));
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 50);
    assert_eq!(complete.hull().duration().get(), 40);
    assert_eq!(complete.slots()[0].main_text(), "one");
    assert_eq!(complete.slots()[2].timing().end().get(), 50);
    assert_eq!(complete.slots()[2].duration().get(), 10);
    assert_eq!(
        adjacencies(complete.adjacencies()),
        [Adjacency::Gap(0, 1, 5), Adjacency::Touching(1, 2)]
    );
    Ok(())
}

/// Zero slots is `Empty`, a state with no hull, distinct from `Complete`.
#[test]
fn empty_count_matched_sequence_is_distinct_from_complete_timing() -> Result<(), TestError> {
    let utterance = utterance("&=laughs .", Some("."))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Empty(empty) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "an empty binding has no timing hull".into(),
        ));
    };
    assert_eq!(empty.slot_count().get(), 0);
    Ok(())
}

/// A missing bullet and a zero-duration bullet each reject the sequence,
/// naming the slot. `30_30` is a bullet the parser ACCEPTS, so this state is
/// reachable from a transcript.
#[test]
fn incomplete_and_nonpositive_timings_are_rejected_with_slot_identity() -> Result<(), TestError> {
    let wor = format!("one {} two three {} .", bullet(10, 20), bullet(30, 30));
    let utterance = utterance("one two three .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Rejected(rejected) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "missing and zero-duration word timing must reject the sequence".into(),
        ));
    };
    assert_eq!(rejected.slot_count().get(), 3);
    assert_eq!(
        issues(rejected.issues()),
        [Issue::Unaligned(1), Issue::NonPositiveInterval(2, 30, 30)]
    );
    Ok(())
}

/// Overlapping positive intervals keep a hull and report the overlap.
#[test]
fn overlapping_adjacent_word_intervals_keep_a_hull_and_expose_geometry() -> Result<(), TestError> {
    let wor = format!("one {} two {} .", bullet(10, 30), bullet(25, 40));
    let utterance = utterance("one two .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "positive overlapping intervals still have a location hull".into(),
        ));
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
    assert_eq!(
        adjacencies(complete.adjacencies()),
        [Adjacency::Overlap(0, 1, 5)]
    );
    Ok(())
}

/// A later word starting before its predecessor is a distinct relation, and
/// the hull is min/max rather than first/last.
#[test]
fn backwards_start_uses_min_max_hull_and_a_distinct_relation() -> Result<(), TestError> {
    let wor = format!("one {} two {} .", bullet(30, 40), bullet(10, 20));
    let utterance = utterance("one two .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "positive backwards intervals still have a location hull".into(),
        ));
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
    assert_eq!(
        adjacencies(complete.adjacencies()),
        [Adjacency::BackwardStart(0, 1, 20)]
    );
    Ok(())
}

// ── corroborate_wor_timing, the lexical check after a count match ───────
//
// From `alignment/wor/correspondence/tests.rs`, over parsed tiers now.

/// A same-count edit to the main tier cannot reuse the old `%wor` timing:
/// count matched, corroboration refused, the mismatch named by slot.
#[test]
fn a_same_count_edit_cannot_expose_timings_without_corroboration() -> Result<(), TestError> {
    let wor = format!("one {} two {} .", bullet(10, 20), bullet(21, 30));
    let utterance = utterance("one changed .", Some(&wor))?;
    let WorTimingBinding::CountMatched(count_matched) =
        bind_wor_timing(&utterance.main, utterance.wor_tier())
    else {
        return Err(TestError::Failure(
            "equal slot counts must reach the count-matched state".into(),
        ));
    };
    let WorTimingCorrespondence::Uncorroborated(uncorroborated) =
        corroborate_wor_timing(count_matched)
    else {
        return Err(TestError::Failure(
            "same-count lexical drift must fail closed".into(),
        ));
    };
    assert_eq!(uncorroborated.mismatches().len(), 1);
    assert_eq!(uncorroborated.mismatches()[0].slot().get(), 1);
    assert_eq!(uncorroborated.mismatches()[0].main_text(), "changed");
    assert_eq!(uncorroborated.mismatches()[0].wor_text(), "two");
    Ok(())
}

/// The canonical display text of a filler (`um` for `&-um`) corroborates,
/// and the slot still points at the main-tier word, not at the `%wor` copy.
#[test]
fn a_fillers_display_text_corroborates_without_replacing_the_word() -> Result<(), TestError> {
    let wor = format!("um {} there {} .", bullet(10, 20), bullet(21, 30));
    let utterance = utterance("&-um there .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    assert_eq!(bound.slots()[0].main_text(), "um");
    let UtteranceContent::Word(main_word) = &utterance.main.content.content[0] else {
        return Err(TestError::Failure(
            "the parsed main tier must start with the filler word".into(),
        ));
    };
    assert!(std::ptr::eq(
        bound.slots()[0].main_word(),
        main_word.as_ref()
    ));
    Ok(())
}

/// Only a corroborated binding can be assessed as a sequence, and a complete
/// one yields its hull.
#[test]
fn only_corroborated_timings_reach_sequence_assessment() -> Result<(), TestError> {
    let wor = format!("one {} two {} .", bullet(10, 20), bullet(25, 40));
    let utterance = utterance("one two .", Some(&wor))?;
    let bound = corroborated(&utterance)?;
    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(bound) else {
        return Err(TestError::Failure(
            "complete corroborated timing must be admitted".into(),
        ));
    };
    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
    Ok(())
}
