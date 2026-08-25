//! Which mark governs a word's language: the ONE precedence decision.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use std::borrow::Cow;

use crate::model::{CodeSwitchSpan, Word};
use crate::validation::word::language::{
    LanguageResolutionOutcome, resolve_word_language_with_marker,
};
use crate::{LanguageCode, LanguageSource, UtteranceLanguage, WordLanguageMarker};

/// Which mark governs a word's language, decided ONCE.
///
/// The precedence (a word's own marker, else an enclosing span, else the
/// utterance) used to be written twice inside metadata computation alone: once
/// to pick the marker that resolves the CODE, and again to pick the
/// `LanguageSource` that records the PROVENANCE. Two implementations of one
/// rule, twenty lines apart, computed from the same two inputs, with nothing
/// tying them together. Changing one order silently produced a record whose
/// code came from the span while its source said the word: a wrong value in the
/// exact field whose only job is to say where the value came from.
///
/// It lives HERE, rather than beside either caller, because there are now two:
/// metadata computation and word VALIDATION. Validation had no notion of an
/// enclosing span at all, so `E220`/`E763` gated on the tier's language while
/// the metadata for the same word recorded the span's. That is the same
/// disagreement one level up, and keeping the rule in one place is what makes
/// it unconstructible rather than merely unlikely.
///
/// Deciding once and deriving both answers is the point; do not add a second
/// route that computes either half independently.
pub enum GoverningMarker<'a> {
    /// The word carries its own `@s` / `@s:code`. It wins: the more specific
    /// mark is the defensible answer, and attested transcripts rely on it,
    /// marking a switched stretch with a span and individual donor-language
    /// items inside it with their own code.
    Own(&'a WordLanguageMarker),
    /// No marker on the word, but an enclosing `<...> [@s]` span governs it.
    Span(&'a CodeSwitchSpan),
    /// Neither; the word inherits whatever the utterance resolved to.
    Utterance,
}

impl<'a> GoverningMarker<'a> {
    /// The one precedence decision.
    ///
    /// Takes the enclosing span directly rather than a `LanguageScope`, which
    /// is the walk's threading device: word validation holds an `Option` and
    /// the walk holds the enum, and mapping between them at each call site gave
    /// "absent means the utterance governs" two owners.
    pub fn of(word: &'a Word, enclosing: Option<&'a CodeSwitchSpan>) -> Self {
        match (word.lang.as_ref(), enclosing) {
            (Some(own), _) => Self::Own(own),
            (None, Some(span)) => Self::Span(span),
            (None, None) => Self::Utterance,
        }
    }

    /// Resolve `word`'s language under this governing mark.
    ///
    /// THE ONLY ROUTE to a word's language, and that is the point rather than a
    /// convenience. While a `resolve_word_language(word, ..)` existed beside
    /// it, a caller could ask the question without saying what scope it was
    /// asking under, and two did. One of them, `debug fix-s`, then CORRUPTED
    /// data: given `<how@s:fra to@s:fra> [@s:eng]` it resolved both words
    /// unscoped to `fra`, wrote `[- fra]`, stripped the `@s:fra` suffixes, and
    /// left the words inside a `[@s:eng]` span that now governed them. A
    /// transform advertised as a normalization changed every word it touched
    /// from French to English, and nothing objected, because the question it
    /// asked had no place to put the span.
    ///
    /// Taking the marker from `self` rather than as a parameter is what closes
    /// that: a caller cannot supply a marker inconsistent with the scope,
    /// because it does not supply one at all.
    pub fn resolve(
        &self,
        word: &Word,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> LanguageResolutionOutcome {
        let marker = self.marker();
        resolve_word_language_with_marker(
            word,
            marker.as_deref(),
            tier_language,
            declared_languages,
        )
    }

    /// The marker the resolver should apply.
    ///
    /// Borrowed for a word's own marker, which is the common path and would
    /// otherwise clone a `Vec` for every `Multiple`/`Ambiguous` word; only a
    /// span-governed word constructs anything.
    fn marker(&self) -> Option<Cow<'a, WordLanguageMarker>> {
        match self {
            Self::Own(own) => Some(Cow::Borrowed(*own)),
            Self::Span(span) => Some(Cow::Owned(WordLanguageMarker::from(*span))),
            Self::Utterance => None,
        }
    }

    /// Where the resolved language came from.
    ///
    /// The span variants are deliberately distinct from the word ones: the
    /// resolved CODE is identical either way and the provenance is not, so a
    /// consumer asking "did the transcriber mark this word, or the span around
    /// it?" can tell.
    pub fn source(
        &self,
        resolution: &crate::validation::LanguageResolution,
        utterance_language: &UtteranceLanguage,
    ) -> LanguageSource {
        // An unresolved language has no provenance to report: naming a source
        // for a value that does not exist is the fabrication this field exists
        // to prevent. This half used to live at the one call site, so the next
        // producer of a `LanguageSource` would have re-written it.
        if matches!(
            resolution,
            crate::validation::LanguageResolution::Unresolved
        ) {
            return LanguageSource::Unresolved;
        }
        match self {
            Self::Own(WordLanguageMarker::Shortcut) => LanguageSource::WordShortcut,
            Self::Own(
                WordLanguageMarker::Explicit(_)
                | WordLanguageMarker::Multiple(_)
                | WordLanguageMarker::Ambiguous(_),
            ) => LanguageSource::WordExplicit,
            Self::Span(CodeSwitchSpan::Shortcut) => LanguageSource::SpanShortcut,
            Self::Span(CodeSwitchSpan::Explicit(_)) => LanguageSource::SpanExplicit,
            Self::Utterance => utterance_language.source(),
        }
    }
}
