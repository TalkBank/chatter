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

/// A reversed word interval is E362; a zero-duration one is a legal instant,
/// and untimed, overlapping and out-of-order words are not this rule's business.
#[test]
fn only_a_reversed_word_interval_is_an_error() {
    let tier = WorTier::from_words(vec![
        Word::simple("backward").with_inline_bullet(Bullet::new(20, 10)),
        Word::simple("instant").with_inline_bullet(Bullet::new(20, 20)),
        Word::simple("forward").with_inline_bullet(Bullet::new(20, 30)),
        Word::simple("earlier").with_inline_bullet(Bullet::new(5, 25)),
        Word::simple("untimed"),
    ]);
    let errors = crate::ErrorCollector::new();
    tier.validate_word_intervals(&errors);
    let diagnostics = errors.into_vec();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, crate::ErrorCode::TimestampBackwards);
}
