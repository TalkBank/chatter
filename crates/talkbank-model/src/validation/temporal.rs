//! Temporal validation for media bullets
//!
//! Implements CLAN CHECK command temporal constraints:
//! - E701 (Error 83): Per-speaker start-time monotonicity
//! - E704 (Error 133): Per-speaker overlap with 500ms tolerance
//!
//! Cross-speaker overlap (CLAN Error 84, the `+c0` strict-timeline mode) is
//! deliberately NOT implemented here: CLAN only fires error 84 when the user
//! passes `+c0` to CHECK, and in normal conversational transcripts
//! cross-speaker overlap is ubiquitous and expected.
//!
//! Note: E702/E703 (strict timeline mode) are reserved for future use.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::{Bullet, ChatFile, UtteranceContent, Word};
use crate::{ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use std::collections::HashMap;

// Import error codes
use crate::codes::temporal::{E701, E704};

/// CLAN Error 133 tolerance for same-speaker overlap in milliseconds.
///
/// Small overlaps are common around annotation boundaries; this threshold mirrors
/// CHECK behavior before issuing `E704`.
///
/// The boundary is INCLUSIVE of the tolerance: an overlap of exactly 500 ms is
/// accepted, 501 ms is reported. Both sides are pinned in
/// `tests/temporal_validation_tests.rs`, because the comparison below is a
/// single `>` and flipping it to `>=` changes behaviour at exactly one value
/// that no other case in that file visits.
pub const SPEAKER_OVERLAP_TOLERANCE_MS: u64 = 500;

/// Validates temporal constraints on utterance bullets.
///
/// This follows CLAN CHECK semantics for bullet timing:
/// 1. Per-speaker start-time monotonicity (`E701` / Error 83)
/// 2. Per-speaker self-overlap with tolerance (`E704` / Error 133)
///
/// Runs in CA mode too (gate removed 2026-07-29). The wholesale CA skip had
/// no CLAN CHECK counterpart, no recorded rationale (it bottoms out in a
/// squashed initial commit), and protected ZERO occurrences across all 994
/// kept CA-declared files, while disabling timing validation exactly where
/// overlap is densest. E362 (bullet monotonicity) always ran on CA files, so
/// the skip was also internally incoherent: one timing rule active, two off.
pub fn validate_temporal_constraints(file: &ChatFile, errors: &impl ErrorSink) {
    // Collect all relevant bullets in document order
    let bullets = collect_bullets(file);

    // 1. Per-speaker start-time monotonicity (E701 - CLAN Error 83)
    validate_global_timeline(&bullets, errors);

    // 2. Per-speaker overlap (E704 - CLAN Error 133)
    validate_speaker_timelines(&bullets, errors);

    // E729 (CLAN Error 84) is NOT run here. CLAN only fires error 84 when
    // the user passes `+c0` to CHECK, enabling strict timeline contiguity.
    // Cross-speaker overlap is normal in conversational transcripts.
}

/// Captured bullet metadata used by temporal validation passes.
#[derive(Debug)]
struct BulletInfo<'a> {
    utterance_idx: usize,
    speaker: &'a str,
    bullet: &'a Bullet,
}

/// Collects utterance bullets used by temporal validators.
///
/// Rules:
/// - Main speaker tiers only (ignore dependent tiers)
/// - Only check terminator bullets (the single bullet in TierContent)
///
/// The collected vector preserves utterance order so monotonicity and overlap
/// checks share a consistent traversal basis.
fn collect_bullets(file: &ChatFile) -> Vec<BulletInfo<'_>> {
    let mut bullets = Vec::new();

    for (idx, utt) in file.utterances().enumerate() {
        // Prefer explicit terminator bullet; recover from internal bullet token if needed.
        let bullet = utt.media_bullet();

        if let Some(bullet) = bullet {
            bullets.push(BulletInfo {
                utterance_idx: idx,
                speaker: utt.main.speaker.as_ref(),
                bullet,
            });
        }
    }

    bullets
}

/// Returns whether utterance content includes at least one transcribed word,
/// at any depth.
///
/// Untranscribed placeholders (`xxx`, `yyy`, `www`) and words with empty
/// cleaned text do not supply lexical transcription. This is not a timing
/// eligibility predicate: untranscribed speech still occupies time, and every
/// collected bullet participates in same-speaker temporal validation.
///
/// Classifying through [`ContentStructure`] means this predicate cannot hold a
/// different opinion about which variants are containers than the traversals
/// around it, which is the drift that produced both bugs.
pub fn has_transcribed_content(content: &[UtteranceContent]) -> bool {
    content
        .iter()
        .any(|item| item.structure().any_word(&word_is_transcribed))
}

/// Returns `true` for a lexical word token with usable transcription.
fn word_is_transcribed(word: &Word) -> bool {
    word.untranscribed().is_none() && !word.cleaned_text().is_empty()
}

/// Validate per-speaker start-time monotonicity (`E701`, CLAN Error 83).
///
/// Rule: Each speaker's utterances must have non-decreasing start times.
/// Cross-speaker non-monotonicity is expected in multi-party conversations
/// (speakers naturally overlap), so only same-speaker violations are reported.
///
/// CLAN fires error 83 globally (cross-speaker), but its early-return
/// implementation accidentally suppresses many cross-speaker hits. We scope
/// to same-speaker intentionally; it matches the real intent of detecting
/// disordered timestamps without flagging normal conversational overlap.
fn validate_global_timeline(bullets: &[BulletInfo], errors: &impl ErrorSink) {
    let mut speaker_last_start: HashMap<&str, (usize, u64)> = HashMap::new();

    for bullet_info in bullets {
        if let Some(&(prev_idx, prev_start_ms)) = speaker_last_start.get(bullet_info.speaker)
            && bullet_info.bullet.timing.start_ms < prev_start_ms
        {
            errors.report(
                ParseError::new(
                    E701,
                    Severity::Error,
                    SourceLocation::new(bullet_info.bullet.span),
                    ErrorContext::new(
                        bullet_text(bullet_info.bullet),
                        Span::from_usize(0, bullet_text(bullet_info.bullet).len()),
                        bullet_text(bullet_info.bullet),
                    ),
                    format!(
                        "Same-speaker start time not monotonic: speaker '{}' utterance {} \
                         starts at {}ms but their utterance {} started at {}ms",
                        bullet_info.speaker,
                        bullet_info.utterance_idx + 1,
                        bullet_info.bullet.timing.start_ms,
                        prev_idx + 1,
                        prev_start_ms
                    ),
                )
                .with_suggestion(format!(
                    "Adjust bullet to start at or after {}ms",
                    prev_start_ms
                )),
            );
        }

        speaker_last_start.insert(
            bullet_info.speaker,
            (
                bullet_info.utterance_idx,
                bullet_info.bullet.timing.start_ms,
            ),
        );
    }
}

/// Validate per-speaker timelines (`E704`, CLAN Error 133).
///
/// Rule: Same speaker cannot overlap with themselves beyond 500ms tolerance
/// current.start_ms >= (previous.end_ms - 500)
/// Every collected bullet constrains timing, including untranscribed speech.
fn validate_speaker_timelines(bullets: &[BulletInfo], errors: &impl ErrorSink) {
    let mut speaker_last_end: HashMap<&str, (usize, u64)> = HashMap::new();

    for bullet_info in bullets {
        if let Some((prev_idx, prev_end_ms)) = speaker_last_end.get(bullet_info.speaker) {
            // Calculate overlap (0 if no overlap)
            let overlap = prev_end_ms.saturating_sub(bullet_info.bullet.timing.start_ms);

            if overlap > SPEAKER_OVERLAP_TOLERANCE_MS {
                errors.report(
                    ParseError::new(
                        E704,
                        Severity::Error,
                        SourceLocation::new(bullet_info.bullet.span),
                        ErrorContext::new(
                            bullet_text(bullet_info.bullet),
                            Span::from_usize(0, bullet_text(bullet_info.bullet).len()),
                            bullet_text(bullet_info.bullet),
                        ),
                        format!(
                            "Speaker '{}' overlaps with self: utterance {} ends at {}ms \
                             but utterance {} starts at {}ms ({}ms overlap exceeds {}ms tolerance)",
                            bullet_info.speaker,
                            prev_idx + 1,
                            prev_end_ms,
                            bullet_info.utterance_idx + 1,
                            bullet_info.bullet.timing.start_ms,
                            overlap,
                            SPEAKER_OVERLAP_TOLERANCE_MS
                        ),
                    )
                    .with_suggestion(format!(
                        "Adjust bullet to start at or after {}ms (tolerating {}ms overlap)",
                        prev_end_ms - SPEAKER_OVERLAP_TOLERANCE_MS,
                        SPEAKER_OVERLAP_TOLERANCE_MS
                    )),
                );
            }
        }

        // Update speaker's last end time
        speaker_last_end.insert(
            bullet_info.speaker,
            (bullet_info.utterance_idx, bullet_info.bullet.timing.end_ms),
        );
    }
}

/// Formats bullet timing as `start_end` for diagnostic context payloads.
fn bullet_text(bullet: &Bullet) -> String {
    format!("{}_{}", bullet.timing.start_ms, bullet.timing.end_ms)
}

#[cfg(test)]
mod tests {
    use super::has_transcribed_content;
    use crate::Span;
    use crate::model::{
        BracketedContent, BracketedItem, Group, Retrace, RetraceKind, UtteranceContent, Word,
        WordCategory,
    };

    // Canonical timing integration contracts live in talkbank-parser-tests.

    #[test]
    fn fillers_count_as_transcribed_content() {
        let content = vec![UtteranceContent::Word(Box::new(
            Word::new_unchecked("&-you_know", "you_know").with_category(WordCategory::Filler),
        ))];

        assert!(has_transcribed_content(&content));
    }

    #[test]
    fn untranscribed_only_content_has_no_lexical_transcription() {
        let content = vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "xxx", "xxx",
        )))];

        assert!(!has_transcribed_content(&content));
    }

    /// Retraced lexical words remain transcribed content. This classification
    /// is independent of the timing obligation shared by all timed speech.
    #[test]
    fn retraced_words_are_transcribed_content() {
        let retraced = Retrace {
            content: BracketedContent::new(vec![BracketedItem::Word(Box::new(Word::simple(
                "dog",
            )))]),
            kind: RetraceKind::Full,
            is_group: true,
            span: Span::from_usize(0, 0),
            marker_span: None,
        };
        let content = vec![
            UtteranceContent::Retrace(Box::new(retraced)),
            // Everything OUTSIDE the retrace is untranscribed, so the retrace
            // is the only thing that supplies lexical transcription.
            UtteranceContent::Word(Box::new(Word::simple("xxx"))),
        ];

        assert!(has_transcribed_content(&content));
    }

    /// The bracketed half recursed into nothing at all, so a word one level
    /// down inside a group was invisible to it.
    #[test]
    fn words_nested_inside_a_bracketed_group_are_transcribed_content() {
        let inner = Group {
            content: BracketedContent::new(vec![BracketedItem::Word(Box::new(Word::simple(
                "dog",
            )))]),
            span: Span::from_usize(0, 0),
            trailing_space: None,
        };
        let outer = Group {
            // A nested group carrying no annotations, which is what this test
            // means and could not say until `BracketedItem::Group` existed: it
            // used to build an ANNOTATED group with an empty list.
            content: BracketedContent::new(vec![BracketedItem::Group(inner)]),
            span: Span::from_usize(0, 0),
            trailing_space: None,
        };
        let content = vec![UtteranceContent::Group(outer)];

        assert!(has_transcribed_content(&content));
    }
}
