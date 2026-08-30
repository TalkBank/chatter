use super::super::*;
use crate::Span;
use crate::model::{Bullet, Terminator, UtteranceContent, Word, WordCategory};

fn word(form: &str) -> UtteranceContent {
    UtteranceContent::Word(Box::new(Word::new_unchecked(form, form)))
}

fn timed_wor_tier(entries: &[(&str, u64, u64)]) -> WorTier {
    WorTier::from_words(
        entries
            .iter()
            .map(|(form, start_ms, end_ms)| {
                Word::new_unchecked(*form, *form)
                    .with_inline_bullet(Bullet::new(*start_ms, *end_ms))
            })
            .collect(),
    )
}

#[test]
fn same_count_edit_cannot_expose_timings_without_lexical_corroboration() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("changed")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("one", 10, 20), ("two", 21, 30)]);
    let WorTimingBinding::CountMatched(count_matched) = bind_wor_timing(&main, Some(&wor)) else {
        panic!("equal slot counts must reach the count-matched state");
    };

    let WorTimingCorrespondence::Uncorroborated(uncorroborated) =
        corroborate_wor_timing(count_matched)
    else {
        panic!("same-count lexical drift must fail closed");
    };

    assert_eq!(uncorroborated.mismatches().len(), 1);
    assert_eq!(uncorroborated.mismatches()[0].slot().get(), 1);
    assert_eq!(uncorroborated.mismatches()[0].main_text(), "changed");
    assert_eq!(uncorroborated.mismatches()[0].wor_text(), "two");
}

#[test]
fn canonical_cleaned_display_text_corroborates_without_becoming_lexical_identity() {
    let filler = Word::new_unchecked("&-um", "um").with_category(WordCategory::Filler);
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(filler)), word("there")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("um", 10, 20), ("there", 21, 30)]);
    let WorTimingBinding::CountMatched(count_matched) = bind_wor_timing(&main, Some(&wor)) else {
        panic!("equal slot counts must reach the count-matched state");
    };

    let WorTimingCorrespondence::Corroborated(corroborated) = corroborate_wor_timing(count_matched)
    else {
        panic!("the canonical generated display sequence must corroborate");
    };

    assert_eq!(corroborated.slots()[0].main_text(), "um");
    let UtteranceContent::Word(main_word) = &main.content.content[0] else {
        panic!("fixture must retain its typed main-tier word");
    };
    assert!(std::ptr::eq(
        corroborated.slots()[0].main_word(),
        main_word.as_ref()
    ));
}

#[test]
fn only_corroborated_timings_can_reach_sequence_assessment() {
    let main = MainTier::new(
        "CHI",
        vec![word("one"), word("two")],
        Terminator::Period { span: Span::DUMMY },
    );
    let wor = timed_wor_tier(&[("one", 10, 20), ("two", 25, 40)]);
    let WorTimingBinding::CountMatched(count_matched) = bind_wor_timing(&main, Some(&wor)) else {
        panic!("equal slot counts must reach the count-matched state");
    };
    let WorTimingCorrespondence::Corroborated(corroborated) = corroborate_wor_timing(count_matched)
    else {
        panic!("matching canonical display text must corroborate");
    };

    let WorTimingSequence::Complete(complete) = assess_wor_timing_sequence(corroborated) else {
        panic!("complete corroborated timing must be admitted");
    };

    assert_eq!(complete.hull().start().get(), 10);
    assert_eq!(complete.hull().end().get(), 40);
}
