use super::*;

#[test]
fn first_recorded_word_timing_follows_serialized_order() {
    let untimed_word = Word::simple(crate::Location::Unlocated, "first");
    let timed_word = Word::simple(crate::Location::Unlocated, "second")
        .with_inline_bullet(Bullet::new(crate::Location::Unlocated, 10, 20));
    let tier = WorTier::from_words(crate::Location::Unlocated, vec![untimed_word, timed_word]);

    let WorTimingEvidence::Recorded(recorded) = tier.timing_evidence() else {
        panic!("a timed word must produce recorded timing evidence");
    };

    assert_eq!(recorded.bullet().timing.start_ms, 10);
}

#[test]
fn an_untimed_wor_tier_has_no_timing_evidence() {
    let tier = WorTier::from_words(
        crate::Location::Unlocated,
        vec![Word::simple(crate::Location::Unlocated, "hello")],
    );

    assert!(matches!(tier.timing_evidence(), WorTimingEvidence::Absent));
}

/// Reversed and zero-duration word intervals are E362,
/// and untimed, overlapping and out-of-order words are not this rule's business.
#[test]
fn nonpositive_word_intervals_are_errors() {
    let tier =
        WorTier::from_words(
            crate::Location::Unlocated,
            vec![
                Word::simple(crate::Location::Unlocated, "backward")
                    .with_inline_bullet(Bullet::new(crate::Location::Unlocated, 20, 10)),
                Word::simple(crate::Location::Unlocated, "instant")
                    .with_inline_bullet(Bullet::new(crate::Location::Unlocated, 20, 20)),
                Word::simple(crate::Location::Unlocated, "forward")
                    .with_inline_bullet(Bullet::new(crate::Location::Unlocated, 20, 30)),
                Word::simple(crate::Location::Unlocated, "earlier")
                    .with_inline_bullet(Bullet::new(crate::Location::Unlocated, 5, 25)),
                Word::simple(crate::Location::Unlocated, "untimed"),
            ],
        );
    let errors = crate::ErrorCollector::new();
    tier.validate_word_intervals(&errors);
    let diagnostics = errors.into_vec();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.code == crate::ErrorCode::TimestampBackwards })
    );
}
