//! Bullet (media timestamp) validation functions
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::model::Bullet;
use crate::{ErrorCode, ErrorSink, ParseError, Severity};

/// Validate one media bullet's internal timing invariants.
///
/// Currently enforces `start_ms < end_ms` (`E362`). The model retains the
/// absolute location; source-aware presentation attaches the actual snippet.
pub(crate) fn check_bullet(bullet: &Bullet, errors: &impl ErrorSink) {
    // E362: Check start < end
    if bullet.timing.start_ms >= bullet.timing.end_ms {
        errors.report(
            ParseError::at_location(
                ErrorCode::TimestampBackwards,
                Severity::Error,
                bullet.location,
                format!(
                    "Media bullet start time ({}ms) must be less than end time ({}ms)",
                    bullet.timing.start_ms, bullet.timing.end_ms
                ),
            )
            .with_suggestion("Ensure start time is before end time in media bullets"),
        );
    }
}

/// Validate one `%wor` word bullet: it must describe a positive interval.
///
/// Both reversed and zero-duration intervals are `E362`. An absent bullet is
/// different: untimed words remain legitimate partial alignment.
pub(crate) fn check_word_interval(bullet: &Bullet, errors: &impl ErrorSink) {
    if bullet.timing.start_ms >= bullet.timing.end_ms {
        errors.report(
            ParseError::at_location(
                ErrorCode::TimestampBackwards,
                Severity::Error,
                bullet.location,
                format!(
                    "Word timing start ({}ms) must be less than end ({}ms)",
                    bullet.timing.start_ms, bullet.timing.end_ms
                ),
            )
            .with_suggestion(
                "Correct the word timing, or omit its bullet if timing is unavailable",
            ),
        );
    }
}

/// Validate chronological ordering across a sequence of bullets.
///
/// Emits `E362` when a bullet starts before the previous bullet's start time
/// (the tier-level ordering rule, E701, is a separate check).
pub fn check_bullet_monotonicity(bullets: &[&Bullet], errors: &impl ErrorSink) {
    for i in 1..bullets.len() {
        // E362: Check monotonicity - each start time must be >= previous start time
        if bullets[i].timing.start_ms < bullets[i - 1].timing.start_ms {
            errors.report(
                ParseError::at_location(
                    ErrorCode::TimestampBackwards,
                    Severity::Error,
                    bullets[i].location,
                    format!(
                        "Media bullet timestamp {}ms comes before previous timestamp {}ms (timestamps must increase monotonically)",
                        bullets[i].timing.start_ms, bullets[i-1].timing.start_ms
                    ),
                )
                .with_suggestion("Ensure media bullets are in chronological order within the same media file"),
            );
        }
    }
}

// Cross-speaker bullet overlap (CLAN Error 84, the `+c0` strict-timeline mode)
// is intentionally not validated; see the module docs in validation::temporal
// for why it is excluded from default validation.
