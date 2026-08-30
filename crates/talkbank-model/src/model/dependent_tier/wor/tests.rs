use super::*;

#[test]
fn first_recorded_word_timing_follows_serialized_order() {
    let untimed_word = Word::simple("first");
    let timed_word = Word::simple("second").with_inline_bullet(Bullet::new(10, 20));
    let tier = WorTier::from_words(vec![untimed_word, timed_word]);

    let WorTimingEvidence::Recorded(recorded) = tier.timing_evidence() else {
        panic!("a timed word must produce recorded timing evidence");
    };

    assert_eq!(recorded.bullet().timing.start_ms, 10);
}

#[test]
fn an_untimed_wor_tier_has_no_timing_evidence() {
    let tier = WorTier::from_words(vec![Word::simple("hello")]);

    assert!(matches!(tier.timing_evidence(), WorTimingEvidence::Absent));
}
