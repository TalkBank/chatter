//! Top-level orchestration.

use talkbank_model::alignment::helpers::{ContentItemMut, walk_content_mut};
use talkbank_model::{ChatFile, EventType, Freecode, Line, OtherSpokenEvent, Word, WriteChat};

use super::dependent_tier::{keep_dependent_tier, sanitize_dependent_tier};
use super::error::RedactError;
use super::header::sanitize_header;
use super::placeholder::PlaceholderState;
use super::policy::SanitizationPolicy;
use super::wor::WorPlan;
use super::word::sanitize_word;

/// The result of sanitizing a parsed `ChatFile`.
///
/// Spans become stale after sanitization; `WriteChat` doesn't depend on
/// them, but downstream JSON consumers should treat sanitized output as
/// a fresh document with no offsets back into the original.
pub struct SanitizedDocument {
    inner: ChatFile,
}

impl SanitizedDocument {
    /// Writes the sanitized document in CHAT surface form to `w`.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.inner.write_chat(w)
    }

    /// Serializes the sanitized document to a fresh `String`.
    pub fn to_chat_string(&self) -> String {
        self.inner.to_chat_string()
    }
}

/// Sanitizes a parsed `ChatFile` according to `policy`.
pub fn sanitize(
    mut input: ChatFile,
    _policy: &SanitizationPolicy,
) -> Result<SanitizedDocument, RedactError> {
    let mut state = PlaceholderState::new();
    let event_replacement = EventType::new("redacted")?;

    for line in input.lines.as_mut_slice().iter_mut() {
        match line {
            Line::Header { header, .. } => {
                sanitize_header(header)?;
            }
            Line::Utterance(utt) => {
                if utt
                    .main
                    .content
                    .postcodes
                    .iter()
                    .any(|code| code.text().is_none())
                {
                    return Err(RedactError::UnparsedPostcode);
                }
                for header in utt.preceding_headers.iter_mut() {
                    sanitize_header(header)?;
                }

                utt.dependent_tiers
                    .retain(|entry| keep_dependent_tier(&entry.tier));
                // Whether each `%wor` tier is the main tier's words is
                // decided while both still carry their text.
                let wor_plan =
                    WorPlan::decide(&utt.main, utt.dependent_tiers.iter().map(|e| &e.tier));

                let main_content = utt.main.content.content.as_mut_slice();
                let mut rejection = None;
                walk_content_mut(main_content, None, &mut |item| {
                    if let Err(error) = sanitize_content_item(item, &mut state, &event_replacement)
                    {
                        rejection = Some(error);
                    }
                });
                if let Some(error) = rejection {
                    return Err(error);
                }

                for entry in utt.dependent_tiers.iter_mut() {
                    sanitize_dependent_tier(&mut entry.tier, &mut state)?;
                }
                wor_plan.apply(
                    &utt.main,
                    utt.dependent_tiers.iter_mut().map(|e| &mut e.tier),
                    &mut state,
                );
            }
        }
    }

    Ok(SanitizedDocument { inner: input })
}

/// Single-pass redaction of every leaf item visited by `walk_content_mut`.
fn sanitize_content_item(
    item: ContentItemMut<'_>,
    state: &mut PlaceholderState,
    event_replacement: &EventType,
) -> Result<(), RedactError> {
    match item {
        ContentItemMut::OrphanAnnotation(_) => return Err(RedactError::OrphanAnnotation),
        ContentItemMut::Word(word) => sanitize_word(word, state),
        ContentItemMut::ReplacedWord(replaced) => {
            sanitize_word(&mut replaced.word, state);
            for word in replaced.replacement.words.as_mut_slice().iter_mut() {
                sanitize_word(word, state);
            }
        }
        ContentItemMut::Event(event) => event.event_type = event_replacement.clone(),
        ContentItemMut::Freecode(fc) => {
            if fc.text().is_none() { return Err(RedactError::UnparsedFreecode); }
            InlineRedaction::Freecode(fc).apply();
        },
        ContentItemMut::OtherSpokenEvent(ose) => InlineRedaction::OtherSpokenEvent(ose).apply(),
        ContentItemMut::Separator(_)
        | ContentItemMut::Pause(_)
        | ContentItemMut::Action(_)
        | ContentItemMut::OverlapPoint(_)
        // Both parsed timing and explicit rejected source survive redaction.
        | ContentItemMut::InternalBullet(_)
        | ContentItemMut::LongFeatureBegin(_)
        | ContentItemMut::LongFeatureEnd(_)
        | ContentItemMut::UnderlineBegin(_)
        | ContentItemMut::UnderlineEnd(_)
        | ContentItemMut::NonvocalBegin(_)
        | ContentItemMut::NonvocalEnd(_)
        | ContentItemMut::NonvocalSimple(_) => {}
    }
    Ok(())
}

/// An exclusive inline target, distinct from free-text header/tier payloads.
/// Consuming this target applies a delimiter-free token: square brackets belong
/// to the enclosing CHAT syntax and cannot be nested into these payloads.
enum InlineRedaction<'a> {
    Freecode(&'a mut Freecode),
    OtherSpokenEvent(&'a mut OtherSpokenEvent),
}

impl InlineRedaction<'_> {
    fn apply(self) {
        const TOKEN: &str = "redacted";
        match self {
            Self::Freecode(code) => *code = Freecode::new(code.location, TOKEN),
            Self::OtherSpokenEvent(event) => {
                let word = event.word_mut();
                *word = Word::new(
                    word.location,
                    talkbank_model::non_empty_literal!("redacted").into(),
                );
            }
        }
    }
}
