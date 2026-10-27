//! Word bodies, annotation categories and their source representations.

use super::RetraceKindParsed;
use serde::Serialize;
use talkbank_model::model::{
    CADelimiterType, CAElementType, OverlapPointKind, WordStressMarkerType,
};

/// Lexer payload admitted once, retaining the token (its index digit as the
/// payload, the whole `[<1]` as the lexeme) and its valid index.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScopedOverlapIndex<'a> {
    source: crate::token::Extracted<'a>,
    index: Option<talkbank_model::model::OverlapMarkerIndex>,
}

impl<'a> ScopedOverlapIndex<'a> {
    pub(crate) fn from_lexer(source: crate::token::Extracted<'a>) -> Option<Self> {
        let index = match source.text().as_bytes() {
            [] => None,
            [digit] => Some(talkbank_model::model::OverlapMarkerIndex::from_ascii_digit(
                *digit,
            )?),
            _ => return None,
        };
        Some(Self { source, index })
    }

    /// The marker as written, for placement.
    pub fn lexeme(&self) -> &'a str {
        self.source.lexeme()
    }

    pub(crate) fn index(&self) -> Option<talkbank_model::model::OverlapMarkerIndex> {
        self.index
    }
}

/// grammar.js: standalone_word = seq(optional(prefix|zero), word_body, optional(form_marker),
///   optional(word_lang_suffix), optional(pos_tag))
/// word_with_optional_annotations = seq(standalone_word, optional(replacement), base_annotations?)
///
/// Mirrors the model Word structure: category prefix, body content, suffix markers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordWithAnnotations<'a> {
    /// Category prefix: Zero (0), PrefixFiller (&-), PrefixNonword (&~), PrefixFragment (&+).
    pub(crate) category: Option<talkbank_model::model::WordCategory>,
    /// Word body content, mirrors model WordContent.
    pub(crate) body: Vec<WordBodyItem<'a>>,
    /// Declared form marker, admitted by the shared registry before lowering.
    pub(crate) form_marker: Option<talkbank_model::model::FormType>,
    /// Language suffix. None if absent.
    pub(crate) lang: Option<talkbank_model::model::WordLanguageMarker>,
    /// POS tag: tag-extracted content (e.g., "n", "adj"). None if absent.
    pub(crate) pos_tag: Option<&'a str>,
    /// At most one replacement, admitted before the base-annotation run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) replacement: Option<ReplacementParsed<'a>>,
    /// Scoped annotations remaining on this word after marker-chain folding.
    pub(crate) annotations: Vec<ScopedAnnotationParsed<'a>>,
    /// The word as written: its slice of the source, whether the lexer read
    /// it as one rich token or the parser assembled it from subtokens (then
    /// the slice runs from the first token's lexeme through the last's, and
    /// includes a curly quote the file parser stripped and reported between
    /// them, which `body` does not). Always borrowed, so every word can be
    /// placed.
    #[serde(rename = "raw_text", serialize_with = "serialize_word_source")]
    pub(crate) source: crate::source_text::SourceSlice<'a>,
}

fn serialize_word_source<S: serde::Serializer>(
    source: &crate::source_text::SourceSlice<'_>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(source.text())
}

impl<'a> WordWithAnnotations<'a> {
    /// Original word bytes and their admitted position in the full parser source.
    pub fn source(&self) -> crate::source_text::SourceSlice<'a> {
        self.source
    }

    /// Original word spelling, including its prefix and suffixes.
    pub fn raw_text(&self) -> &'a str {
        self.source.text()
    }

    /// Admitted word category, absent for ordinary lexical words.
    pub fn category(&self) -> Option<&talkbank_model::model::WordCategory> {
        self.category.as_ref()
    }

    /// Parsed body items in their authored order.
    pub fn body(&self) -> &[WordBodyItem<'a>] {
        &self.body
    }

    /// Form admitted by the shared registry.
    pub fn form_marker(&self) -> Option<&talkbank_model::model::FormType> {
        self.form_marker.as_ref()
    }

    /// Admitted language suffix.
    pub fn lang(&self) -> Option<&talkbank_model::model::WordLanguageMarker> {
        self.lang.as_ref()
    }

    /// Lexed part-of-speech suffix payload.
    pub fn pos_tag(&self) -> Option<&'a str> {
        self.pos_tag
    }

    /// The word's optional replacement, admitted before scoped annotations.
    pub fn replacement(&self) -> Option<&ReplacementParsed<'a>> {
        self.replacement.as_ref()
    }

    /// Trailing annotations admitted by the grammar.
    pub fn annotations(&self) -> &[ScopedAnnotationParsed<'a>] {
        &self.annotations
    }
}

/// A bracketed construct, classified once by the token-to-AST producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParsedAnnotation<'a> {
    /// A shared scoped annotation plus its original source lexeme.
    Scoped(ScopedAnnotationParsed<'a>),
    /// Retrace markers change the preceding content structure. The marker is
    /// kept as written (`[/]`, `[//]`, ...), for placement.
    Retrace(RetraceKindParsed, &'a str),
    /// A replacement changes the word's structure.
    Replacement(ReplacementParsed<'a>),
    /// An utterance language code: payload `eng`, lexeme `[- eng]`.
    Langcode(crate::token::Extracted<'a>),
    /// An utterance postcode, with its lexeme.
    Postcode(crate::token::PostcodeToken<'a>),
}

/// A replacement admitted by the word grammar, nonempty by construction. Its
/// complete source lexeme is retained at admission, including delimiters and
/// whitespace; later consumers never reconstruct its spelling.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplacementParsed<'a> {
    first: Box<WordWithAnnotations<'a>>,
    rest: Vec<WordWithAnnotations<'a>>,
    lexeme: &'a str,
}

/// Delimiters must belong to the parser source in authored order.
#[derive(Debug)]
pub(crate) struct RefusedReplacementSource;

impl<'a> ReplacementParsed<'a> {
    pub(crate) fn new(
        source: crate::source_text::SourceText<'a>,
        begin: &'a str,
        first: WordWithAnnotations<'a>,
        rest: Vec<WordWithAnnotations<'a>>,
        end: &'a str,
    ) -> Result<Self, RefusedReplacementSource> {
        let begin = source.span_of(begin).ok_or(RefusedReplacementSource)?;
        let end = source.span_of(end).ok_or(RefusedReplacementSource)?;
        if begin.end() > end.start() {
            return Err(RefusedReplacementSource);
        }
        Ok(Self {
            first: Box::new(first),
            rest,
            lexeme: source
                .slice(begin.start()..end.end())
                .map_err(|_| RefusedReplacementSource)?
                .text(),
        })
    }

    /// The construct's extent in `source`, from `[:` through `]`; `None` when
    /// its retained lexeme is not a slice of `source`.
    pub fn span_in(
        &self,
        source: crate::source_text::SourceText<'_>,
    ) -> Option<talkbank_model::Span> {
        source.span_of(self.lexeme)
    }

    /// The required first word, admitted by the replacement grammar.
    pub fn first(&self) -> &WordWithAnnotations<'a> {
        &self.first
    }

    /// Additional intended words, in authored order.
    pub fn rest(&self) -> &[WordWithAnnotations<'a>] {
        &self.rest
    }

    /// Exact source bytes retained when the replacement was admitted.
    pub fn lexeme(&self) -> &'a str {
        self.lexeme
    }
}

/// A shared semantic annotation with its original lexeme for parser placement.
/// The native parser owns source evidence, not a second annotation vocabulary.
#[derive(Debug, Clone, Serialize)]
pub struct ScopedAnnotationParsed<'a> {
    annotation: talkbank_model::ContentAnnotation,
    #[serde(serialize_with = "serialize_word_source")]
    lexeme: crate::source_text::SourceSlice<'a>,
}

// Preserve semantic-and-spelling equality; source coordinates are evidence,
// not part of the annotation's existing equality contract.
impl PartialEq for ScopedAnnotationParsed<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.annotation == other.annotation && self.lexeme() == other.lexeme()
    }
}

impl<'a> ParsedAnnotation<'a> {
    /// The annotation as written, preserving delimiters and whitespace.
    pub fn chat_text(&self) -> String {
        match self {
            Self::Scoped(scoped) => scoped.lexeme().to_owned(),
            Self::Retrace(_, marker) => (*marker).to_owned(),
            Self::Replacement(replacement) => replacement.lexeme().to_owned(),
            Self::Langcode(code) => code.lexeme().to_owned(),
            Self::Postcode(postcode) => postcode.lexeme().to_owned(),
        }
    }

    /// The annotation as written in `source`, delimiters included; `None`
    /// when its slices are not from `source`. This borrows the same exact
    /// bytes that [`Self::chat_text`] returns as an owned string.
    pub fn source_text(&self, source: crate::source_text::SourceText<'a>) -> Option<&'a str> {
        match source.slice(self.span_in(source)?.to_range()) {
            Ok(slice) => Some(slice.text()),
            Err(_) => None,
        }
    }

    /// Where the annotation is in `source`: its lexeme, or for a replacement
    /// the extent from `[:` through `]`. `None` when the slices are not from
    /// `source`.
    pub fn span_in(
        &self,
        source: crate::source_text::SourceText<'_>,
    ) -> Option<talkbank_model::Span> {
        match self {
            Self::Scoped(scoped) => source.span_of(scoped.lexeme()),
            Self::Retrace(_, marker) => source.span_of(marker),
            Self::Replacement(replacement) => replacement.span_in(source),
            Self::Langcode(code) => source.span_of(code.lexeme()),
            Self::Postcode(postcode) => source.span_of(postcode.lexeme()),
        }
    }

    /// Shared scoped annotations, distinct from structural parser constructs.
    pub fn scoped(&self) -> Option<&ScopedAnnotationParsed<'a>> {
        match self {
            Self::Scoped(scoped) => Some(scoped),
            Self::Retrace(..) | Self::Replacement(_) | Self::Langcode(_) | Self::Postcode(_) => {
                None
            }
        }
    }

    /// Return admitted replacement words, without reparsing their spelling.
    pub fn replacement(&self) -> Option<&ReplacementParsed<'a>> {
        match self {
            Self::Replacement(replacement) => Some(replacement),
            Self::Scoped(_) | Self::Retrace(..) | Self::Langcode(_) | Self::Postcode(_) => None,
        }
    }

    /// Postfix constructs cannot start an utterance (E759).
    pub fn is_postfix(&self) -> bool {
        match self {
            Self::Retrace(..) | Self::Replacement(_) => true,
            Self::Scoped(scoped) => matches!(
                scoped.annotation(),
                talkbank_model::ContentAnnotation::OverlapBegin(_)
                    | talkbank_model::ContentAnnotation::OverlapEnd(_)
            ),
            Self::Langcode(_) | Self::Postcode(_) => false,
        }
    }
}

impl<'a> ScopedAnnotationParsed<'a> {
    pub(crate) fn new(
        annotation: talkbank_model::ContentAnnotation,
        lexeme: crate::source_text::SourceSlice<'a>,
    ) -> Self {
        Self { annotation, lexeme }
    }

    /// Shared semantic payload, already constructed by token admission.
    pub fn annotation(&self) -> &talkbank_model::ContentAnnotation {
        &self.annotation
    }

    /// The marker as written, including its delimiters.
    pub fn lexeme(&self) -> &'a str {
        self.lexeme.text()
    }

    /// Coordinates retained when the annotation was admitted by the parser.
    pub fn span(&self) -> talkbank_model::Span {
        self.lexeme.span()
    }
}

impl<'a> ParsedAnnotation<'a> {
    /// Whether this annotation is a retrace marker.
    pub fn is_retrace(&self) -> bool {
        matches!(self, ParsedAnnotation::Retrace(..))
    }

    /// The retrace kind and the marker as written, for a retrace annotation.
    pub fn retrace(&self) -> Option<(RetraceKindParsed, &'a str)> {
        match self {
            ParsedAnnotation::Retrace(kind, marker) => Some((*kind, marker)),
            _ => None,
        }
    }
}

/// A single item inside a word body. Mirrors model `WordContent`.
///
/// Every marker carries its lexeme, the slice of the word it was read from,
/// so the converter can place it with `convert::locate`; the word itself is
/// placed from its `raw_text`, a slice of the same source on both parse paths.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WordBodyItem<'a> {
    /// Plain text segment (e.g., "hello", "want")
    Text(talkbank_model::model::WordText),
    /// Shortened syllable, tag-extracted content (e.g., "be" from "(be)")
    Shortening(talkbank_model::model::WordShortening),
    /// Syllable lengthening (:, ::, :::): the run of colons as written.
    Lengthening(LengtheningRun<'a>),
    /// Compound marker (+), as written.
    CompoundMarker(&'a str),
    /// Stress marker (primary ˈ or secondary ˌ), as written.
    Stress(WordStressMarkerType, &'a str),
    /// Overlap point (⌈, ⌉, ⌊, ⌋ with optional index)
    OverlapPoint(OverlapPointKind, &'a str),
    /// Syllable pause (^), as written.
    SyllablePause(&'a str),
    /// Clitic boundary (~), as written.
    CliticBoundary(&'a str),
    /// CA element (single symbol like ↑, ↓, ≠, etc.), as written.
    CaElement(CAElementType, &'a str),
    /// CA delimiter (paired like °softer°, ∆faster∆, etc.), as written.
    CaDelimiter(CADelimiterType, &'a str),
    /// Word-internal underline begin (`␂␁`), as in `j␂␁a`.
    ///
    /// Underline markers glue to letters inside a word in CA transcripts, and
    /// the word-body scanner used to consume and DISCARD them with a
    /// "skip for now" comment. That silently unbalanced the underline check:
    /// a word-internal begin vanished while a word-initial end survived as a
    /// content-level marker, so `chatter validate --parser re2c` reported 768
    /// spurious E357 "unmatched underline end" across CA corpora that
    /// tree-sitter reads as clean.
    UnderlineBegin(&'a str),
    /// Word-internal underline end (`␂␂`). See [`WordBodyItem::UnderlineBegin`].
    UnderlineEnd(&'a str),
}

/// A nonempty run of lengthening colons, as written in the word.
///
/// The one owner of the proof the model's `WordLengthening` needs (a count of
/// at least one): the character scanner takes an admitted prefix, while the
/// subtoken builder requires whole-token admission through [`Self::new`].
/// The count is proven once, at admission, and stored, so
/// reading it has no fallible arm; it is the run's byte length (one byte per
/// colon), so it cannot drift from the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengtheningRun<'a> {
    text: &'a str,
    count: std::num::NonZeroUsize,
}

impl<'a> LengtheningRun<'a> {
    /// `None` unless `run` is one or more colons and nothing else.
    pub fn new(run: &'a str) -> Option<Self> {
        let (run, remainder) = Self::take_prefix(run)?;
        remainder.is_empty().then_some(run)
    }

    /// Admit the maximal leading colon run and retain the untouched remainder.
    /// `None` means the source does not start with a colon, including empty input.
    /// The source slice and nonzero count come from the same scan; consumers
    /// never reconstruct a run or turn failed admission into ordinary text.
    pub(crate) fn take_prefix(source: &'a str) -> Option<(Self, &'a str)> {
        let count =
            std::num::NonZeroUsize::new(source.bytes().take_while(|&byte| byte == b':').count())?;
        // Every consumed byte is ASCII ':', so this is a UTF-8 boundary in
        // the same source, including when the remainder starts with Unicode.
        let (text, remainder) = source.split_at(count.get());
        Some((Self { text, count }, remainder))
    }

    /// The colons as written, for placement.
    pub fn text(self) -> &'a str {
        self.text
    }

    /// How many colons.
    pub fn count(self) -> std::num::NonZeroUsize {
        self.count
    }
}

/// The AST serializes the run as written; the count is derived from it.
impl Serialize for LengtheningRun<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.text)
    }
}

#[cfg(test)]
mod lengthening_run_tests {
    use super::LengtheningRun;

    #[test]
    fn only_a_nonempty_run_of_colons_admits() {
        for rejected in ["", "a:", ": ", "::a", " "] {
            assert!(LengtheningRun::new(rejected).is_none(), "{rejected:?}");
        }
        let run = LengtheningRun::new(":::").expect("three colons admit");
        assert_eq!(run.count().get(), 3);
        assert_eq!(run.text(), ":::");
        assert_eq!(
            LengtheningRun::new(":")
                .expect("one colon admits")
                .count()
                .get(),
            1
        );
    }

    #[test]
    fn prefix_admission_retains_the_unconsumed_source() {
        for source in ["", "a:", "😀::"] {
            assert!(LengtheningRun::take_prefix(source).is_none());
        }
        for (source, expected_run, expected_remainder) in [
            (":", ":", ""),
            (":::é", ":::", "é"),
            ("::😀a", "::", "😀a"),
            (": a:", ":", " a:"),
        ] {
            let (run, remainder) = LengtheningRun::take_prefix(source).expect("leading colons");
            assert_eq!(run.text(), expected_run);
            assert_eq!(run.count().get(), expected_run.len());
            assert_eq!(remainder, expected_remainder);
            assert_eq!(run.text().as_ptr(), source.as_ptr());
            assert_eq!(remainder.as_ptr(), source[expected_run.len()..].as_ptr());
            assert_eq!(LengtheningRun::new(source).is_some(), remainder.is_empty());
        }
    }
}

#[cfg(test)]
mod replacement_source_tests {
    use super::*;
    use crate::source_text::SourceText;

    #[test]
    fn replacement_admission_retains_bytes_and_refuses_foreign_or_reversed_delimiters() {
        let reference = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/annotation/errors-and-replacements.cha"
        ));
        // Whitespace variation of the reference's existing multiword replacement.
        let source = reference.replace("[: a+er b]", "[:  a+er  b ]");
        let line = source
            .split_inclusive('\n')
            .find(|line| line.contains("[:  a+er"))
            .unwrap();
        let parsed = crate::parser::parse_main_tier_with_source(line).unwrap();
        let replacement = parsed
            .main_tier
            .tier()
            .tier_body
            .contents
            .iter()
            .find_map(|item| {
                let crate::ast::ContentItem::Word(word) = item else {
                    return None;
                };
                word.replacement()
            })
            .unwrap();
        let annotation = ParsedAnnotation::Replacement(replacement.clone());
        let expected = "[:  a+er  b ]";
        assert_eq!(annotation.chat_text(), expected);
        assert_eq!(annotation.source_text(parsed.source()), Some(expected));
        assert_eq!(replacement.lexeme(), expected);

        let foreign = line.to_owned();
        assert_eq!(annotation.source_text(SourceText::new(&foreign)), None);
        let orphan = crate::ast::ContentItem::OrphanAnnotation(annotation.clone());
        let talkbank_model::UtteranceContent::OrphanAnnotation(raw) =
            crate::convert::content_item_to_model(&orphan, SourceText::new(&foreign))
        else {
            panic!("orphan must remain explicit recovery content")
        };
        assert_eq!(raw.source(), expected);
        assert!(raw.location().span().is_none());

        let lexeme = replacement.lexeme();
        let begin = &lexeme[..2];
        let end = &lexeme[lexeme.len() - 1..];
        for (source, begin, end) in [
            (SourceText::new(&foreign), begin, end),
            (parsed.source(), end, begin),
        ] {
            assert!(
                ReplacementParsed::new(
                    source,
                    begin,
                    replacement.first().clone(),
                    replacement.rest().to_vec(),
                    end,
                )
                .is_err()
            );
        }
    }
}
