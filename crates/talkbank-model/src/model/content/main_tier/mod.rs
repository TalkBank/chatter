//! Main-tier model (`*SPK:` lines) for CHAT transcripts.
//!
//! This type combines speaker identity with shared utterance payload
//! ([`TierContent`]) and span metadata used during validation/reporting.
//!
//! # CHAT Format Structure
//!
//! ```text
//! *SPEAKER:\tCONTENT TERMINATOR [+ postcode] •timestamp•
//! ```
//!
//! The main tier consists of:
//! 1. Speaker code (e.g., `*CHI:`, `*MOT:`)
//! 2. Linkers (optional discourse markers like `++`, `+<`, `+^`)
//! 3. Language code (optional, for code-switching: `[- zho]`)
//! 4. Content (words, events, pauses, groups, etc.)
//! 5. Terminator (punctuation like `.`, `?`, `!`)
//! 6. Postcodes (optional utterance-level annotations)
//! 7. Bullet (optional media timestamp)
//!
//! # CHAT Format Examples
//!
//! ```text
//! *CHI: I want cookie .
//! *MOT: do you want a cookie ?
//! *CHI: ++ yeah !
//! *CHI: [- spa] hola ! [+ code-switching]
//! *CHI: I want &-uh cookie . •12345_23456•
//! ```
//!
//! # References
//!
//! - [Main Tier](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
//! - [Speaker Codes](https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes)
//! - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
//! - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)

mod language_switch;
#[cfg(test)]
mod tests;
mod wor;

use super::{
    Bullet, LanguageCode, Linker, Postcode, SpeakerCode, Terminator, TierContent, UtteranceContent,
    WriteChat,
};
use crate::alignment::helpers::{WordItem, walk_words};
use crate::model::content::word::Word;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Typed representation of one main-tier utterance line.
///
/// Main tiers are the alignment anchor for dependent tiers (`%mor`, `%gra`,
/// `%pho`, `%wor`, ...).
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want cookie .
/// *MOT: do you want a cookie ?
/// *CHI: ++ yeah !
/// *FAT: [- spa] hola ! [+ code-switching]
/// *CHI: the dog &=barks ! •12345_23456•
/// ```
///
/// # Components
///
/// - **Speaker**: Three-letter code identifying the participant (e.g., `CHI`, `MOT`, `INV`)
/// - **Linkers**: Optional discourse markers (`++`, `+<`, `+^`) appearing after the speaker
/// - **Language code**: Optional marker for code-switching (`[- code]`)
/// - **Content**: Sequence of words, events, pauses, groups, and other elements
/// - **Terminator**: Punctuation (`.`, `?`, `!`, `+...`, etc.) marking utterance end
/// - **Postcodes**: Optional annotations after terminator (`[+ text]`, `[+bch]`, etc.)
/// - **Bullet**: Optional media timestamp (`•start_end•`)
///
/// # References
///
/// - [Main Tier](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [Speaker Codes](https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes)
/// - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
/// - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MainTier {
    /// Speaker code (for example `CHI`, `MOT`, `INV`).
    pub speaker: SpeakerCode,
    /// Shared tier payload (content, markers, terminator, postcodes, bullet).
    pub content: TierContent,
    /// Source span for the full main-tier line.
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
    /// Source span for speaker token only.
    #[serde(skip)]
    #[schemars(skip)]
    pub speaker_span: Span,
}

impl MainTier {
    /// Builds a main tier from required fields with empty optional components.
    pub fn new(
        speaker: impl Into<SpeakerCode>,
        content: Vec<UtteranceContent>,
        terminator: impl Into<Option<Terminator>>,
    ) -> Self {
        Self {
            speaker: speaker.into(),
            content: TierContent::with_all(
                Vec::new(),
                None,
                content,
                terminator.into(),
                Vec::new(),
                None,
            ),
            span: Span::DUMMY,
            speaker_span: Span::DUMMY,
        }
    }

    /// Sets full-line source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Sets speaker-token source span metadata.
    pub fn with_speaker_span(mut self, speaker_span: Span) -> Self {
        self.speaker_span = speaker_span;
        self
    }

    /// Replaces linker list.
    pub fn with_linkers(mut self, linkers: Vec<Linker>) -> Self {
        self.content = self.content.with_linkers(linkers);
        self
    }

    /// Appends one linker.
    pub fn with_linker(mut self, linker: Linker) -> Self {
        self.content.linkers.0.push(linker);
        self
    }

    /// Sets utterance-scoped language code (`[- code]`).
    pub fn with_language_code(mut self, language_code: impl Into<LanguageCode>) -> Self {
        self.content = self.content.with_language_code(language_code.into());
        self
    }

    /// Replaces postcode list.
    pub fn with_postcodes(mut self, postcodes: Vec<Postcode>) -> Self {
        self.content = self.content.with_postcodes(postcodes);
        self
    }

    /// Appends one postcode.
    pub fn with_postcode(mut self, postcode: Postcode) -> Self {
        self.content.postcodes.0.push(postcode);
        self
    }

    /// Sets utterance-level terminal bullet.
    pub fn with_bullet(mut self, bullet: Bullet) -> Self {
        self.content = self.content.with_bullet(bullet);
        self
    }

    /// Sets source span for content region (portion after speaker token).
    pub fn with_content_span(mut self, content_span: Span) -> Self {
        self.content = self.content.with_content_span(content_span);
        self
    }

    /// Serialize this main tier to an owned CHAT string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Find the first context-dependent CA omission span in this main tier.
    ///
    /// Fragment parsers use this to reject parenthesized omission shorthand
    /// when they do not have file-level `@Options: CA` context. This matches
    /// both explicit `CAOmission` words and standalone shortening-only forms.
    pub fn find_context_dependent_ca_omission_span(&self) -> Option<Span> {
        fn word_requires_ca_file_context(word: &Word) -> bool {
            let standalone_shortening = word.content.len() == 1
                && matches!(word.content[0], crate::model::WordContent::Shortening(_));

            matches!(word.category, Some(crate::model::WordCategory::CAOmission))
                || standalone_shortening
        }

        let mut span = None;
        walk_words(&self.content.content, None, &mut |leaf| match leaf {
            WordItem::Word(word) if span.is_none() && word_requires_ca_file_context(word) => {
                span = Some(word.span);
            }
            WordItem::ReplacedWord(replaced) if span.is_none() => {
                if word_requires_ca_file_context(&replaced.word) {
                    span = Some(replaced.word.span);
                } else {
                    span = replaced
                        .replacement
                        .words
                        .iter()
                        .find(|word| word_requires_ca_file_context(word))
                        .map(|word| word.span);
                }
            }
            WordItem::Word(_) | WordItem::ReplacedWord(_) | WordItem::Separator(_) => {}
        });

        span
    }
}

impl WriteChat for MainTier {
    /// Serialize one main tier line as CHAT text.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('*')?;
        self.speaker.write_chat(w)?;
        w.write_str(":\t")?;
        self.content.write_tier_content(w)?;
        Ok(())
    }
}

impl std::fmt::Display for MainTier {
    /// Formats this main tier as one canonical CHAT utterance line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

impl crate::validation::Validate for MainTier {
    /// Validates speaker IDs, tier content, and delegation to nested validators.
    fn validate(&self, context: &crate::validation::ValidationContext, errors: &impl ErrorSink) {
        use crate::validation::main_tier::{
            check_no_nested_quotations, check_no_pauses_in_pho_groups,
        };
        use crate::validation::retrace::check_retraces_have_content;

        // Validate speaker
        let speaker_str = self.speaker.as_str();

        // Check length
        if speaker_str.len() > 7 {
            errors.report(ParseError::new(
                ErrorCode::InvalidSpeaker,
                Severity::Error,
                SourceLocation::new(self.speaker_span),
                ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                format!(
                    "Speaker ID '{}' exceeds maximum length of 7 characters",
                    speaker_str
                ),
            ));
        }

        // Check for invalid characters
        if let Some(invalid_char) = crate::validation::has_invalid_speaker_chars(speaker_str) {
            errors.report(ParseError::new(
                ErrorCode::InvalidSpeaker,
                Severity::Error,
                SourceLocation::new(self.speaker_span),
                ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                format!(
                    "Speaker ID '{}' contains invalid character '{}'. Speaker IDs cannot contain colon (:) or whitespace",
                    speaker_str, invalid_char
                ),
            ));
        }

        // E308: Check if speaker is in participant list (unless speaker is "0" - unidentified)
        if !context.shared.participant_ids.is_empty()
            && !context.shared.participant_ids.contains(&self.speaker)
            && speaker_str != "0"
        {
            errors.report(
                ParseError::new(
                    ErrorCode::UndeclaredSpeaker,
                    Severity::Error,
                    SourceLocation::new(self.speaker_span),
                    ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                    format!("Speaker '{}' is not in the participant list", speaker_str),
                )
                .with_suggestion("Add this speaker to the @Participants header"),
            );

            let mut expected_speakers: Vec<String> = context
                .shared
                .participant_ids
                .iter()
                .map(|speaker| speaker.as_str().to_string())
                .collect();
            expected_speakers.sort();

            let warning_context =
                ErrorContext::new(speaker_str, self.speaker_span, "speaker_not_found")
                    .with_expected(expected_speakers);
            errors.report(
                ParseError::new(
                    ErrorCode::SpeakerNotFoundInParticipants,
                    Severity::Warning,
                    SourceLocation::new(self.speaker_span),
                    warning_context,
                    format!(
                        "Speaker '{}' not found in @Participants header",
                        speaker_str
                    ),
                )
                .with_suggestion(
                    "Add this speaker to @Participants, or correct the speaker code on the tier",
                ),
            );
        }

        let tier_content = &self.content;
        let content_items = &tier_content.content;

        // E305: missing terminator, main tier utterances must end with a
        // terminator unless CA mode is active.
        if tier_content.terminator.is_none() && !context.shared.ca_mode {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingTerminator,
                    Severity::Error,
                    SourceLocation::new(self.span),
                    ErrorContext::new(self.speaker.as_str(), self.span, self.speaker.as_str()),
                    "Expected terminator not found",
                )
                .with_suggestion("Add a terminator at the end: Standard (. ? !), Interruption (+... +/. +//. +/? +//? +/??  +..? +\"/. +\". +.), or CA intonation (⇗ ↗ → ↘ ⇘ ≋ +≋ ≈ +≈)"),
            );
        }

        // Validate content using the content_span for precise error highlighting
        // The content_span points to the exact portion after the colon where content errors occur
        let content_context = if let Some(content_span) = tier_content.content_span {
            // Use content's own span for precise error highlighting
            context
                .clone()
                .with_field_span(content_span)
                .with_field_text("") // Empty text - span is used for location, not text extraction
        } else {
            // Fallback for backward compatibility (shouldn't happen after parser update)
            context
                .clone()
                .with_field_span(self.span)
                .with_field_text("")
        };
        tier_content.content.validate(&content_context, errors);

        // Determine tier-level language for word validation
        // Tier can override with language_code (e.g., *CHI@s:spa)
        let tier_language = tier_content
            .language_code
            .clone()
            .or_else(|| context.shared.default_language.clone());
        let word_context = context.clone().with_tier_language(tier_language);

        // Validate all content items with the appropriate language
        for content_item in content_items.iter() {
            match content_item {
                UtteranceContent::Word(word) => {
                    word.validate(&word_context, errors);
                }
                UtteranceContent::AnnotatedWord(annotated_word) => {
                    annotated_word.validate(&word_context, errors);
                }
                UtteranceContent::ReplacedWord(replaced_word) => {
                    replaced_word.validate(&word_context, errors);
                }
                // Future: Validate other content types (events, pauses, groups, etc.)
                _ => {}
            }
        }

        // Validate bullet if present
        if let Some(bullet) = &tier_content.bullet {
            crate::validation::check_bullet(bullet, errors);
        }

        // E370: Validate retraces are followed by content
        check_retraces_have_content(self, errors);

        // E371: Validate no pauses inside phonological groups
        check_no_pauses_in_pho_groups(self, errors);

        // E372: Validate no nested quotations
        check_no_nested_quotations(self, errors);

        if let Some(target_lang) = self.whole_utterance_language_switch_target(
            word_context.shared.default_language.as_ref(),
            &word_context.shared.declared_languages,
        ) {
            let span = tier_content.content_span.unwrap_or(self.span);
            errors.report(
                ParseError::new(
                    ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode,
                    Severity::Error,
                    SourceLocation::new(span),
                    None,
                    format!(
                        "Whole-utterance language switch to '{}' should use utterance precode [- {}] instead of tagging every lexical word with @s",
                        target_lang.as_str(),
                        target_lang.as_str()
                    ),
                )
                .with_suggestion(format!(
                    "Rewrite the utterance as '[- {}] ...' and remove the per-word @s markers",
                    target_lang.as_str()
                )),
            );
        }

        // Style-level whitespace checks are intentionally disabled in core
        // validation to avoid false positives on valid reference CHAT corpora.
    }
}
