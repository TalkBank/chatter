//! Which mark governs a word's language: the ONE precedence decision.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use std::borrow::Cow;

use crate::model::{CodeSwitchSpan, Separator, Word};
use crate::validation::word::language::{
    LanguageResolutionOutcome, resolve_marker_at, resolve_without_marker,
};
use crate::{LanguageCode, LanguageSource, Span, UtteranceLanguage, WordLanguageMarker};

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
pub(crate) enum GoverningMarker<'a> {
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
    pub(crate) fn of(word: &'a Word, enclosing: Option<&'a CodeSwitchSpan>) -> Self {
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
    pub(crate) fn resolve(
        &self,
        word: &Word,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> LanguageResolutionOutcome {
        self.resolve_at(word.span, tier_language, declared_languages)
    }

    /// Resolve at a SPAN rather than from a `Word`.
    ///
    /// The word was only ever needed to place diagnostics. A caller holding an
    /// already-extracted word (Batchalign's `ExtractedWord`) has a span and no
    /// `Word`, and used to fabricate one with `Word::new_unchecked` purely to
    /// satisfy the signature. This is the honest entry point for it, and it is
    /// what lets that fabrication be deleted.
    pub(crate) fn resolve_at(
        &self,
        span: crate::Span,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> LanguageResolutionOutcome {
        let marker = self.marker();
        resolve_marker_at(marker.as_deref(), span, tier_language, declared_languages)
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
    pub(crate) fn source(
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

/// Which mark governs a word's language, OWNED, carrying the span it was born
/// with.
///
/// This is the public face of the precedence rule; `GoverningMarker` is the
/// borrowed form used inside this crate's hot validation path, and is
/// deliberately NOT public.
///
/// # Why the span rides along
///
/// The resolver needs a span to place diagnostics. An earlier design took it as
/// a PARAMETER, and a caller then had to hand-pair a mark with the right span:
/// `words[0].mark.resolve(words[1].span, ..)` type-checked and anchored every
/// diagnostic at the wrong offset, with nothing but a reviewer to notice. The
/// span is captured in [`GoverningMark::of`] from the same `word` that decided
/// the mark, so the pairing is not maintained by convention; there is no
/// parameter left to get wrong.
///
/// # Why it is opaque
///
/// BOTH constructors take the enclosing span: [`GoverningMark::of`] for a word
/// and [`GoverningMark::of_separator`] for a separator. There is therefore no
/// way to obtain a value of this type, and so no way to resolve a language,
/// without having answered the scope question. An earlier version of this
/// paragraph said "the only constructor is `of`" while a second public
/// constructor sat sixty lines below it, which is the check this very paragraph
/// tells the reader to run. A
/// previous doc claimed `GoverningMarker` was already "the ONLY route"; it was
/// not, because its variants were public and any crate could build one
/// directly. A proof type is only as strong as its weakest constructor, and a
/// public variant is a constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoverningMark {
    mark: Mark,
}

/// The owned payload of a [`GoverningMark`]. Private: see the opacity note.
///
/// THE SPAN LIVES ON THE MARKED VARIANTS, not beside them. A span exists to
/// anchor a diagnostic, only a marker can raise one, and `Utterance` means
/// there is no marker, so a span there could never be read. It used to sit in a
/// sibling field, which forced a `utterance_governed` constructor to supply
/// `Span::DUMMY` for a word that has no position; `Span::DUMMY` is `{0, 0}`, a
/// perfectly legal position at byte 0, so a synthetic word's diagnostic would
/// have anchored at the start of the file indistinguishably from a real one.
/// Nothing caught that because nothing could: the only reason it never fired is
/// that the no-marker path happens to emit no diagnostics, a fact no type
/// recorded. Now the sentinel has nowhere to go.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mark {
    Own(WordLanguageMarker, Span),
    Span(CodeSwitchSpan, Span),
    /// No marker, and therefore no span: nothing here can place a diagnostic.
    Utterance,
}

/// Which KIND of mark governs, with no payload.
///
/// For consumers that must branch on the kind (Batchalign restores a word's
/// surface text only when the WORD itself carried markers) without being able
/// to construct a governing mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoverningMarkKind {
    /// The word carries its own `@s` / `@s:code`.
    Own,
    /// An enclosing `<...> [@s]` span governs it.
    Span,
    /// Neither; the utterance governs.
    Utterance,
}

impl GoverningMark {
    /// The one precedence decision, capturing the word's span.
    #[must_use]
    pub fn of(word: &Word, enclosing: Option<&CodeSwitchSpan>) -> Self {
        let mark = match GoverningMarker::of(word, enclosing) {
            GoverningMarker::Own(own) => Mark::Own(own.clone(), word.span),
            GoverningMarker::Span(span) => Mark::Span(span.clone(), word.span),
            GoverningMarker::Utterance => Mark::Utterance,
        };
        Self { mark }
    }

    /// The mark for a SEPARATOR, which carries no `@s` by construction.
    ///
    /// Takes the separator itself, not a bare span, and that is the whole
    /// design. The first version of this took `(span: Span, enclosing: ..)` and
    /// never consulted any word's `lang`, so
    /// `without_own_marker(word.span, None)` on a word carrying `@s:fra`
    /// returned `Utterance`. That is not a misplaced diagnostic; it is a WRONG
    /// LANGUAGE, and it is the same corruption `debug fix-s` produced and that
    /// this type exists to prevent. A public constructor that can fabricate the
    /// very defect the type is named for is the weakest possible constructor.
    ///
    /// `Separator` has no `lang` field, so "carries no own marker" is now a
    /// property of the ARGUMENT TYPE rather than a promise in this paragraph. A
    /// `Word` cannot be passed here at all.
    #[must_use]
    pub fn of_separator(separator: &Separator, enclosing: Option<&CodeSwitchSpan>) -> Self {
        let mark = match enclosing {
            Some(enclosing) => Mark::Span(enclosing.clone(), separator.span()),
            None => Mark::Utterance,
        };
        Self { mark }
    }

    /// The borrowed form, for delegating to the one rule.
    fn borrowed(&self) -> GoverningMarker<'_> {
        match &self.mark {
            Mark::Own(own, _) => GoverningMarker::Own(own),
            Mark::Span(span, _) => GoverningMarker::Span(span),
            Mark::Utterance => GoverningMarker::Utterance,
        }
    }

    /// Which kind of mark this is.
    #[must_use]
    pub fn kind(&self) -> GoverningMarkKind {
        match &self.mark {
            Mark::Own(..) => GoverningMarkKind::Own,
            Mark::Span(..) => GoverningMarkKind::Span,
            Mark::Utterance => GoverningMarkKind::Utterance,
        }
    }

    /// Resolve the language, at the span this mark was born with.
    #[must_use]
    pub fn resolve(
        &self,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> LanguageResolutionOutcome {
        match &self.mark {
            // A marked position anchors its diagnostics at the span it was born
            // with; the pairing is structural, not a parameter to get wrong.
            Mark::Own(_, span) | Mark::Span(_, span) => {
                self.borrowed()
                    .resolve_at(*span, tier_language, declared_languages)
            }
            // No marker, so no diagnostic and no span to place one at.
            Mark::Utterance => resolve_without_marker(tier_language),
        }
    }
}
