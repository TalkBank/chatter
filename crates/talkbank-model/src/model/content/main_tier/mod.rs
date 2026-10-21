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
pub use language_switch::UnspannedSwitchTarget;
#[cfg(test)]
mod tests;
mod wor;

use super::{
    Bullet, LanguagePrecode, Linker, Postcode, SpeakerCode, Terminator, TierContent, TierSeparator,
    UtteranceContent, WriteChat,
};
use crate::alignment::helpers::{WordItem, walk_words};
use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
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
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
    /// Source span for speaker token only.
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub speaker_location: crate::Location,
    /// The `colon tab trailing_space?` separator between the speaker token
    /// and the tier content. Provenance only: E758 (illegal trailing space)
    /// is detected from this field; the JSON wire and schema are unchanged.
    #[serde(skip)]
    #[schemars(skip)]
    pub separator: TierSeparator,
}

/// Where a main tier sits in its source: the whole line, the speaker label
/// and the content region after the separator.
///
/// Three locations of one type travelled positionally through
/// [`MainTier::new`] until 2026-10-18, so swapping the line and the speaker
/// compiled and only misplaced diagnostics. Named fields carry the meaning
/// with the value. A builder with no source writes [`Self::UNLOCATED`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MainTierLocations {
    /// The whole `*SPK:\tcontent` line, including its line ending when the
    /// source has one (both parser backends measure a line this way, as
    /// they do a header or a dependent tier).
    pub line: crate::Location,
    /// The speaker label only.
    pub speaker: crate::Location,
    /// The content after the separator's colon, through the end of `line`:
    /// the tab is content, as in the grammar, so a content offset can be
    /// compared across backends byte for byte.
    pub content: crate::Location,
}

impl MainTierLocations {
    /// A main tier built without source: every part unlocated.
    pub const UNLOCATED: Self = Self {
        line: crate::Location::Unlocated,
        speaker: crate::Location::Unlocated,
        content: crate::Location::Unlocated,
    };
}

impl MainTier {
    /// Builds a main tier from required fields with empty optional components.
    pub fn new(
        locations: MainTierLocations,
        speaker: impl Into<SpeakerCode>,
        content: Vec<UtteranceContent>,
        terminator: impl Into<Option<Terminator>>,
    ) -> Self {
        Self {
            speaker: speaker.into(),
            content: TierContent::with_all(
                locations.content,
                Vec::new(),
                None,
                content,
                terminator.into(),
                Vec::new(),
                None,
            ),
            location: locations.line,
            speaker_location: locations.speaker,
            separator: TierSeparator::CLEAN,
        }
    }

    /// Sets the label-to-content separator (E758 provenance).
    pub fn with_separator(mut self, separator: TierSeparator) -> Self {
        self.separator = separator;
        self
    }

    /// Replaces linker list.
    pub fn with_linkers(mut self, linkers: Vec<Linker>) -> Self {
        self.content = self.content.with_linkers(linkers);
        self
    }

    /// Appends one linker.
    pub fn with_linker(mut self, linker: Linker) -> Self {
        self.content.linkers.push(linker);
        self
    }

    /// Sets utterance-scoped language code (`[- code]`).
    pub fn with_language_code(mut self, precode: LanguagePrecode) -> Self {
        self.content = self.content.with_language_code(precode);
        self
    }

    /// Replaces postcode list.
    pub fn with_postcodes(mut self, postcodes: Vec<Postcode>) -> Self {
        self.content = self.content.with_postcodes(postcodes);
        self
    }

    /// Appends one postcode.
    pub fn with_postcode(mut self, postcode: Postcode) -> Self {
        self.content.postcodes.push(postcode);
        self
    }

    /// Sets utterance-level terminal bullet.
    pub fn with_bullet(mut self, bullet: Bullet) -> Self {
        self.content = self.content.with_bullet(bullet);
        self
    }

    /// Serialize this main tier to an owned CHAT string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// The location of the first context-dependent CA omission in this main
    /// tier, if there is one.
    ///
    /// Fragment parsers use this to reject parenthesized omission shorthand
    /// when they do not have file-level `@Options: CA` context. This matches
    /// both explicit `CAOmission` words and standalone shortening-only forms.
    /// The word's PRESENCE is the answer; its location, which a front end
    /// may not have recorded, only says where to point. Until 2026-10-18 this
    /// returned the span and skipped an unlocated word, so a front end that
    /// recorded no offset for the word would have admitted the shorthand.
    pub fn find_context_dependent_ca_omission(&self) -> Option<crate::Location> {
        use crate::model::content::word::ca::requires_ca_omission_context;

        let mut found = None;
        walk_words(&self.content.content, None, &mut |leaf| match leaf {
            WordItem::Word(word) if found.is_none() && requires_ca_omission_context(word) => {
                found = Some(word.location);
            }
            WordItem::ReplacedWord(replaced) if found.is_none() => {
                if requires_ca_omission_context(&replaced.word) {
                    found = Some(replaced.word.location);
                } else {
                    found = replaced
                        .replacement
                        .words
                        .iter()
                        .find(|word| requires_ca_omission_context(word))
                        .map(|word| word.location);
                }
            }
            WordItem::Word(_) | WordItem::ReplacedWord(_) | WordItem::Separator(_) => {}
        });

        found
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
            check_no_nested_quotations, check_no_pauses_in_pho_groups, report_unknown_annotations,
        };
        use crate::validation::retrace::check_retraces;

        // Validate speaker
        let speaker_str = self.speaker.as_str();

        crate::validation::check_speaker_id(speaker_str, self.speaker_location, errors);

        // E308: Check if speaker is in participant list (unless speaker is "0" - unidentified)
        if !context.shared.participant_ids.is_empty()
            && !context.shared.participant_ids.contains(&self.speaker)
            && speaker_str != "0"
        {
            errors.report(
                ParseError::at_location(
                    ErrorCode::UndeclaredSpeaker,
                    Severity::Error,
                    self.speaker_location,
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

            // Preserve the expected-speaker metadata beside a model-value
            // snippet, never use the document offset inside that short text.
            let warning_context = ErrorContext::new(
                speaker_str,
                crate::Span::new(0, speaker_str.len()),
                "speaker_not_found",
            )
            .with_expected(expected_speakers);
            errors.report(
                ParseError::new(
                    ErrorCode::SpeakerNotFoundInParticipants,
                    Severity::Warning,
                    DiagnosticLocation::from(self.speaker_location),
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

        crate::validation::main_tier::check_leading_bullets(self, context, errors);

        // E305: missing terminator, main tier utterances must end with a
        // terminator unless CA mode is active. Skipped on a parse-recovered
        // tier: the terminator may sit inside the unparsed region, so its
        // absence from the model is unknowable, not a finding (the E316
        // recovery diagnostic already marks the utterance invalid).
        if tier_content.terminator.is_none()
            && !context.shared.ca_mode
            && !context.main_parse_tainted
        {
            errors.report(
                ParseError::at_location(
                    ErrorCode::MissingTerminator,
                    Severity::Error,
                    self.location,
                    "Expected terminator not found",
                )
                .with_suggestion("Add a terminator at the end: Standard (. ? !), Interruption (+... +/. +//. +/? +//? +/??  +..? +\"/. +\". +.), or CA intonation (⇗ ↗ → ↘ ⇘ ≋ +≋ ≈ +≈)"),
            );
        }

        // Validate content at the content region's own location (the portion
        // after the colon) for precise highlighting; programmatic content
        // falls back to the tier's location.
        let content_location = tier_content.location.or(self.location);
        let content_context = context
            .clone()
            .with_field_location(content_location)
            .with_field_text(""); // the location, not text, is the context
        tier_content.content.validate(&content_context, errors);

        // Determine tier-level language for word validation
        // Tier can override with language_code (e.g., *CHI@s:spa)
        let tier_language = tier_content
            .language_code
            .as_ref()
            .map(|precode| precode.code.clone())
            .or_else(|| context.shared.default_language.clone());
        let word_context = context.clone().with_tier_language(tier_language);

        // Validate every word at EVERY depth with the appropriate language.
        //
        // This iterated flatly until 2026-07-27, matching only the three
        // word-like variants with a `_ => {}` catch-all swallowing every
        // container. Words nested in a retrace, a reformulation, or an angle
        // group were therefore never word-validated at all: not for digits,
        // not for illegal characters, not for anything. The same token was
        // rejected outside a group and accepted inside one, and E220 carried
        // that hole for as long as the rule has existed.
        crate::validation::main_tier::validate_words_at_every_depth(
            content_items,
            &word_context,
            errors,
        );

        // Validate bullet if present
        if let Some(bullet) = &tier_content.bullet {
            crate::validation::check_bullet(bullet, errors);
        }

        // E370 and E377, over one traversal of the tier.
        check_retraces(self, errors);

        // E371: Validate no pauses inside phonological groups
        check_no_pauses_in_pho_groups(self, errors);

        // E372: Validate no nested quotations
        check_no_nested_quotations(self, errors);

        // E207: an unrecognised scoped annotation, on ANY construct.
        report_unknown_annotations(self, errors);

        if let Some(target_lang) = self.whole_utterance_language_switch_target(
            word_context.shared.default_language.as_ref(),
            &word_context.shared.declared_languages,
        ) {
            errors.report(
                ParseError::new(
                    ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode,
                    Severity::Error,
                    DiagnosticLocation::from(content_location),
                    None,
                    format!(
                        "Whole-utterance language switch to '{}' should use utterance precode [- {}] instead of tagging every lexical word with @s",
                        target_lang.language().as_str(),
                        target_lang.language().as_str()
                    ),
                )
                .with_suggestion(format!(
                    "Rewrite the utterance as '[- {}] ...' and remove the per-word @s markers",
                    target_lang.language().as_str()
                )),
            );
        }

        // Style-level whitespace checks are intentionally disabled in core
        // validation to avoid false positives on valid reference CHAT corpora.
    }
}
