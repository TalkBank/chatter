//! Where the prefix marker (`#`) may appear inside a main-tier word.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//!
//! # The construct
//!
//! The marker separates a bound prefix from its stem in languages whose
//! orthography glues the two together, letting a transcriber keep the
//! morphology visible on the main tier: Hebrew `ha# kelev` ("the dog"),
//! Arabic `l# walad`. The marker attaches to the END of the prefix.
//!
//! # Two independent rules
//!
//! Position and language are orthogonal, and the module keeps them apart:
//!
//! - **`E762`, position, language-independent.** A word that is nothing but
//!   the marker, or that opens with it, cannot be the construct above in any
//!   language. Neither shape is attested anywhere in the corpora.
//! - **`E763`, language, position-independent.** A legally-positioned marker
//!   is only meaningful in a language that uses it. The gate reads the WORD's
//!   resolved language, never the file's `@Languages` header, exactly as the
//!   digits rule (`E220`) does.
//!
//! A word classified as `Standalone` or `Initial` is reported by the
//! positional rule only. Reporting the language rule on top of it would name
//! a consequence of the real defect rather than the defect, and the fix for
//! the positional shape is not "change the language".
//!
//! # What is deliberately NOT rejected
//!
//! Word-INTERNAL markers in a marker-using language (`mi#ha#shuk`). Hebrew's
//! BermanLong corpus writes 35,802 of them as glued forms. Rejecting internal
//! markers outright is a separate change blocked on normalizing that corpus;
//! until then they are legal wherever the language allows the marker at all.
//!
//! # Grounding
//!
//! Typed survey over every `#`-bearing corpus file, 2026-07-26: word-final
//! 26,811 Arabic and 8,041 Hebrew, plus 14 strays across seven other
//! languages; word-internal 35,802, all Hebrew; word-initial 0; standalone 0.

use crate::model::Word;
use crate::model::content::word::WordCategory;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

use super::language::LanguageResolution;
use super::language::mixed_language_allows_prefix_marker;

/// The prefix marker itself.
///
/// Functional data rather than prose punctuation: this is the character CHAT
/// word text literally contains, so it is written as an escape.
const PREFIX_MARKER: char = '\u{23}';

/// Where the prefix marker sits inside one word, when it carries one.
///
/// Classification is per WORD, not per marker occurrence: a word holding
/// several markers takes the variant of its outermost one, which is the
/// distinction the rules turn on. The variants are ordered by precedence, so
/// `#dog#` is `Initial` (the opening marker is the defect) and `#` alone is
/// `Standalone` rather than both `Initial` and `Final`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixMarkerPosition {
    /// The whole word is the marker (`#`). Never legal.
    Standalone,
    /// The word opens with the marker (`#dog`). Never legal.
    Initial,
    /// The word closes with the marker (`ha#`). The sanctioned construct.
    Final,
    /// The marker sits between other characters (`mi#ha#shuk`).
    Internal,
}

impl PrefixMarkerPosition {
    /// Classify a word's marker position, or `None` when it carries none.
    pub(crate) fn of(text: &str) -> Option<Self> {
        if !text.contains(PREFIX_MARKER) {
            return None;
        }
        if text.chars().all(|c| c == PREFIX_MARKER) {
            return Some(Self::Standalone);
        }
        if text.starts_with(PREFIX_MARKER) {
            return Some(Self::Initial);
        }
        if text.ends_with(PREFIX_MARKER) {
            return Some(Self::Final);
        }
        Some(Self::Internal)
    }

    /// Whether this position is legal in a language that uses the marker.
    ///
    /// Named as a question about the position alone; whether the word's
    /// language uses the marker at all is the other rule's business.
    fn is_legal_position(self) -> bool {
        match self {
            Self::Standalone | Self::Initial => false,
            Self::Final | Self::Internal => true,
        }
    }

    /// How the diagnostic describes an illegal position.
    fn describe_illegal(self) -> &'static str {
        match self {
            Self::Standalone => "stands alone as a word",
            Self::Initial => "opens the word",
            // Legal positions never reach the diagnostic; naming them here
            // rather than falling through a catch-all keeps the match
            // exhaustive and the omission deliberate.
            Self::Final | Self::Internal => "is legally positioned",
        }
    }
}

/// Report `E762` when a word's prefix marker sits where no language allows it.
///
/// Runs unconditionally, without language context: the shapes it rejects are
/// not a language's business, and requiring a resolved language would let a
/// file with no `@Languages` header carry them silently.
pub(crate) fn check_prefix_marker_position(word: &Word, errors: &impl ErrorSink) {
    let cleaned = word.cleaned_text();
    let Some(position) = PrefixMarkerPosition::of(cleaned) else {
        return;
    };
    if position.is_legal_position() {
        return;
    }

    errors.report(
        ParseError::new(
            ErrorCode::PrefixMarkerIllegalPosition,
            Severity::Error,
            SourceLocation::new(word.span),
            ErrorContext::new(cleaned, word.span, cleaned),
            format!(
                "\"{cleaned}\" is not a legal word: the prefix marker {}",
                position.describe_illegal()
            ),
        )
        .with_suggestion(
            "The prefix marker attaches to the END of the prefix it marks, and \
             the prefix is a word of its own (Hebrew \"ha# kelev\"). Attach the \
             marker to a prefix, or remove it.",
        ),
    );
}

/// Report `E763` when a legally-positioned marker appears in a language that
/// does not use it.
///
/// Mirrors the digits check (`E220`) in every policy decision, deliberately:
///
/// - Omission words (`0word`) are skipped, as there the leading `0` is CHAT
///   notation rather than transcribed material.
/// - `Unresolved` language yields an empty candidate set and the check is
///   skipped, because a check that cannot know the language must not guess.
/// - Mixed and ambiguous codes are permissive: if ANY candidate language uses
///   the marker, the word passes.
pub(crate) fn check_prefix_marker_language(
    word: &Word,
    resolution: &LanguageResolution,
    errors: &impl ErrorSink,
) {
    if word.category == Some(WordCategory::Omission) {
        return;
    }

    let cleaned = word.cleaned_text();
    let Some(position) = PrefixMarkerPosition::of(cleaned) else {
        return;
    };
    // An illegally-positioned marker is `E762`'s to report; adding this
    // diagnostic on top would name a consequence rather than the defect.
    if !position.is_legal_position() {
        return;
    }

    let allowed = resolution
        .languages()
        .iter()
        .any(|lang| mixed_language_allows_prefix_marker(lang.as_str()));
    if allowed {
        return;
    }

    errors.report(
        ParseError::new(
            ErrorCode::PrefixMarkerLanguageNotAllowed,
            Severity::Error,
            SourceLocation::new(word.span),
            ErrorContext::new(cleaned, word.span, cleaned),
            format!(
                "\"{cleaned}\" is not a legal word in language(s) \"{}\": the \
                 prefix marker is not used in that language",
                resolution.as_display_string()
            ),
        )
        .with_suggestion(
            "Languages that use the prefix marker: heb, ara. Remove the marker, \
             or mark the word's own language with @s: if it is a code switch.",
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A word with no marker has no position at all, rather than a default one.
    #[test]
    fn a_word_without_the_marker_has_no_position() {
        assert_eq!(PrefixMarkerPosition::of("kelev"), None);
    }

    /// The bare marker classifies as standalone.
    #[test]
    fn the_bare_marker_is_standalone() {
        assert_eq!(
            PrefixMarkerPosition::of("#"),
            Some(PrefixMarkerPosition::Standalone)
        );
    }

    /// A word of nothing but markers is still standalone, not initial.
    ///
    /// Guards the precedence order: `##` opens with a marker AND ends with
    /// one, and calling it `Initial` would produce a diagnostic that talks
    /// about a stem the word does not have.
    #[test]
    fn a_word_of_only_markers_is_standalone() {
        assert_eq!(
            PrefixMarkerPosition::of("##"),
            Some(PrefixMarkerPosition::Standalone)
        );
    }

    /// The sanctioned Hebrew construct classifies as final.
    #[test]
    fn a_trailing_marker_is_final() {
        assert_eq!(
            PrefixMarkerPosition::of("ha#"),
            Some(PrefixMarkerPosition::Final)
        );
    }

    /// A leading marker classifies as initial.
    #[test]
    fn a_leading_marker_is_initial() {
        assert_eq!(
            PrefixMarkerPosition::of("#dog"),
            Some(PrefixMarkerPosition::Initial)
        );
    }

    /// A word both opening and closing with markers takes the initial verdict.
    #[test]
    fn a_marker_at_both_ends_is_initial() {
        assert_eq!(
            PrefixMarkerPosition::of("#dog#"),
            Some(PrefixMarkerPosition::Initial)
        );
    }

    /// BermanLong's glued form classifies as internal.
    #[test]
    fn markers_between_characters_are_internal() {
        assert_eq!(
            PrefixMarkerPosition::of("mi#ha#shuk"),
            Some(PrefixMarkerPosition::Internal)
        );
    }

    /// Exactly the two never-legal positions are rejected on position alone.
    #[test]
    fn only_standalone_and_initial_are_illegal_positions() {
        assert!(!PrefixMarkerPosition::Standalone.is_legal_position());
        assert!(!PrefixMarkerPosition::Initial.is_legal_position());
        assert!(PrefixMarkerPosition::Final.is_legal_position());
        assert!(PrefixMarkerPosition::Internal.is_legal_position());
    }
}
