//! E700-E799: Media/Temporal Validation Errors
//!
//! These errors validate the temporal consistency of media bullets in CHAT files.
//! Corresponds to CLAN CHECK command errors 83, 84, 85, and 133.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::ErrorCode;

/// E701: Tier begin time not monotonic (CLAN Error 83)
///
/// Global timeline constraint: Each utterance's first bullet must have a start time
/// greater than or equal to the previous utterance's first bullet start time.
///
/// Also covers within-tier sequence: bullets within an utterance must be in temporal order.
pub const E701: ErrorCode = ErrorCode::TierBeginTimeNotMonotonic;

/// E702: number occupied by `InvalidMorphologyFormat` (an existing,
/// unrelated `%mor`-tier code); NOT available for reassignment.
///
/// The strict-timeline-mode gap check (CLAN +c1 flag: bullet start times
/// must exactly match previous end times) that this number was once
/// pencilled in for still has no dedicated variant and no implementation.
/// TODO(temporal): allocate a fresh code (E702 is taken) for
/// GapInStrictTimeline when strict mode is implemented.
/// Status: Low priority - strict timeline mode (+c1 flag) is rare in practice
/// Blocked by: Implementing --strict-timeline flag in CLI and ValidationConfig
///
/// E703: genuinely free. Its prior occupant, `UnexpectedMorphologyNode`,
/// was RETIRED 2026-07-31 (no emit site ever constructed it; see
/// `error_code.rs`), so this number is now actually available, unlike
/// E702 above.
///
/// The strict-timeline-mode overlap check (CLAN +c1 flag: no overlaps in
/// the global timeline) this number was pencilled in for still has no
/// implementation.
/// TODO(temporal): add a dedicated OverlapInStrictTimeline variant at
/// E703 when strict mode is implemented.
/// Status: Low priority - strict timeline mode (+c1 flag) is rare in practice
/// Blocked by: Implementing --strict-timeline flag in CLI and ValidationConfig
///
/// E704: Speaker self-overlap (CLAN Error 133)
///
/// Per-speaker constraint: A speaker cannot overlap with themselves.
/// CLAN allows 500ms tolerance - overlaps less than 500ms are permitted.
///
/// Rule: For consecutive utterances by the same speaker,
/// current.start_ms >= (previous.end_ms - 500)
pub const E704: ErrorCode = ErrorCode::SpeakerSelfOverlap;

/// E729: Cross-speaker bullet overlap (CLAN Error 84)
///
/// Cross-speaker constraint: BEG time of current tier is before END time of
/// previous tier (by a different speaker). Unlike E704, this is cross-speaker
/// and is reported as a warning since cross-speaker overlap can be intentional.
pub const E729: ErrorCode = ErrorCode::BulletOverlap;

/// E731: Speaker bullet self-overlap via timing (CLAN Error 133, bullet-based)
///
/// Supplements E704 (which checks overlap markers) with actual bullet timing.
/// Same-speaker BEG < previous END without tolerance threshold.
pub const E731: ErrorCode = ErrorCode::SpeakerBulletSelfOverlap;
