//! Word bodies, annotation categories and their source representations.

use super::RetraceKindParsed;
use serde::Serialize;

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
            [digit @ b'1'..=b'9'] => {
                Some(talkbank_model::model::OverlapMarkerIndex::new(digit - b'0').ok()?)
            }
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
/// word_with_optional_annotations = seq(standalone_word, repeat(annotation))
///
/// Mirrors the model Word structure: category prefix, body content, suffix markers.
#[derive(Default, Debug, Clone, PartialEq, Serialize)]
pub struct WordWithAnnotations<'a> {
    /// Category prefix: Zero (0), PrefixFiller (&-), PrefixNonword (&~), PrefixFragment (&+).
    pub category: Option<WordCategory>,
    /// Word body content, mirrors model WordContent.
    pub body: Vec<WordBodyItem<'a>>,
    /// Declared form marker, admitted by the shared registry before lowering.
    pub form_marker: Option<talkbank_model::model::FormType>,
    /// Language suffix. None if absent.
    pub lang: Option<ParsedLangSuffix<'a>>,
    /// POS tag: tag-extracted content (e.g., "n", "adj"). None if absent.
    pub pos_tag: Option<&'a str>,
    /// Trailing scoped annotations: `[*]`, `[= text]`, `[/]`, `[!]`, etc.
    pub annotations: Vec<ParsedAnnotation<'a>>,
    /// The word as written: its slice of the source, whether the lexer read
    /// it as one rich token or the parser assembled it from subtokens (then
    /// the slice runs from the first token's lexeme through the last's, and
    /// includes a curly quote the file parser stripped and reported between
    /// them, which `body` does not). Always borrowed, so every word can be
    /// placed.
    pub raw_text: &'a str,
}

/// A bracketed construct, classified once by the token-to-AST producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParsedAnnotation<'a> {
    /// A scoped annotation; conversion to ContentAnnotation is total.
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
/// opening `[:` and closing `]` are kept as written, so the construct can be
/// placed from the first delimiter to the last.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplacementParsed<'a> {
    first: Box<WordWithAnnotations<'a>>,
    rest: Vec<WordWithAnnotations<'a>>,
    begin: &'a str,
    end: &'a str,
}

impl<'a> ReplacementParsed<'a> {
    pub(crate) fn new(
        begin: &'a str,
        first: WordWithAnnotations<'a>,
        rest: Vec<WordWithAnnotations<'a>>,
        end: &'a str,
    ) -> Self {
        Self {
            first: Box::new(first),
            rest,
            begin,
            end,
        }
    }

    /// The construct's extent in `source`, from `[:` through `]`; `None` when
    /// either delimiter is not a slice of `source`.
    pub fn span_in(
        &self,
        source: crate::source_text::SourceText<'_>,
    ) -> Option<talkbank_model::Span> {
        Some(source.span_of(self.begin)?.merge(source.span_of(self.end)?))
    }

    /// The required first word, admitted by the replacement grammar.
    pub fn first(&self) -> &WordWithAnnotations<'a> {
        &self.first
    }

    /// Additional intended words, in authored order.
    pub fn rest(&self) -> &[WordWithAnnotations<'a>] {
        &self.rest
    }

    fn chat_text(&self) -> String {
        let mut text = format!("[: {}", self.first.raw_text);
        for word in &self.rest {
            text.push(' ');
            text.push_str(word.raw_text);
        }
        text.push(']');
        text
    }
}

/// A parsed scoped annotation. Every variant keeps the marker as written, so
/// the annotation can be placed; a payload-bearing marker keeps its payload
/// beside the lexeme (`Extracted`), a bare marker keeps the lexeme alone.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ScopedAnnotationParsed<'a> {
    /// `[!]`, stressing
    Stressing(&'a str),
    /// `[!!]`, contrastive stressing
    ContrastiveStressing(&'a str),
    /// `[?]`, uncertain
    Uncertain(&'a str),
    /// `[e]`, exclude
    Exclude(&'a str),
    /// `[@s]`, code-switch span resolving the way a bare `word@s` does.
    CodeSwitchShortcut(&'a str),
    /// `[@s:code]`, code-switch span naming its language. Payload is the code.
    CodeSwitchExplicit(crate::token::Extracted<'a>),
    /// `[* code]`, error marker. Payload is the code (may be empty).
    Error(crate::token::Extracted<'a>),
    /// `[<]` or `[<1]`, overlap precedes. Content is the optional index digit.
    OverlapPrecedes(ScopedOverlapIndex<'a>),
    /// `[>]` or `[>1]`, overlap follows
    OverlapFollows(ScopedOverlapIndex<'a>),
    /// `[= text]`, explanation
    Explanation(crate::token::Extracted<'a>),
    /// `[=! text]`, paralinguistic
    Paralinguistic(crate::token::Extracted<'a>),
    /// `[=? text]`, alternative
    Alternative(crate::token::Extracted<'a>),
    /// `[% text]`, percent comment
    PercentComment(crate::token::Extracted<'a>),
    /// A bracketed annotation whose marker no rule recognises. The payload is
    /// the text BETWEEN the brackets, so `[@ xyz]` carries `"@ xyz"`.
    ///
    /// Reaching the model as `ContentAnnotation::Unknown` is the point: the
    /// validator then reports E207, which is a statement about the FILE, where
    /// a parse failure (E321) is a statement about the parser.
    Unknown(crate::token::Extracted<'a>),
}

impl<'a> ParsedAnnotation<'a> {
    /// The annotation as written: its lexeme, or for a replacement (whose
    /// words are parsed structurally) the spelling rebuilt from them.
    pub fn chat_text(&self) -> String {
        match self {
            Self::Scoped(scoped) => scoped.lexeme().to_owned(),
            Self::Retrace(_, marker) => (*marker).to_owned(),
            Self::Replacement(replacement) => replacement.chat_text(),
            Self::Langcode(code) => code.lexeme().to_owned(),
            Self::Postcode(postcode) => postcode.lexeme().to_owned(),
        }
    }

    /// The annotation as written in `source`, delimiters included; `None`
    /// when its slices are not from `source`. Unlike [`Self::chat_text`],
    /// this is the exact bytes, spacing and all.
    pub fn source_text(&self, source: crate::source_text::SourceText<'a>) -> Option<&'a str> {
        self.span_in(source).map(|span| source.text_at(span))
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

    /// Scoped annotations have a total model conversion, unlike structural codes.
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
        matches!(
            self,
            Self::Retrace(..)
                | Self::Replacement(_)
                | Self::Scoped(
                    ScopedAnnotationParsed::OverlapPrecedes(_)
                        | ScopedAnnotationParsed::OverlapFollows(_)
                )
        )
    }
}

impl<'a> ScopedAnnotationParsed<'a> {
    /// The marker as written, including its delimiters.
    pub fn lexeme(&self) -> &'a str {
        match self {
            Self::Stressing(lexeme)
            | Self::ContrastiveStressing(lexeme)
            | Self::Uncertain(lexeme)
            | Self::Exclude(lexeme)
            | Self::CodeSwitchShortcut(lexeme) => lexeme,
            Self::Unknown(token)
            | Self::CodeSwitchExplicit(token)
            | Self::Error(token)
            | Self::Explanation(token)
            | Self::Paralinguistic(token)
            | Self::Alternative(token)
            | Self::PercentComment(token) => token.lexeme(),
            Self::OverlapPrecedes(index) | Self::OverlapFollows(index) => index.lexeme(),
        }
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

/// Category of a word, determined by its prefix token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WordCategory {
    /// `0word`, omitted word
    Omission,
    /// `&~word`, babbling/nonword
    Nonword,
    /// `&-word`, filler
    Filler,
    /// `&+word`, phonological fragment
    Fragment,
}

/// Parsed language suffix from `@s` tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ParsedLangSuffix<'a> {
    /// Bare `@s`, toggle shortcut
    Shortcut,
    /// `@s:eng` or `@s:eng+zho` or `@s:eng&spa`, carries the code(s)
    Explicit(&'a str),
}

/// A single item inside a word body. Mirrors model `WordContent`.
///
/// Every marker carries its lexeme, the slice of the word it was read from,
/// so the converter can place it with `convert::locate`; the word itself is
/// placed from its `raw_text`, a slice of the same source on both parse paths.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WordBodyItem<'a> {
    /// Plain text segment (e.g., "hello", "want")
    Text(&'a str),
    /// Shortened syllable, tag-extracted content (e.g., "be" from "(be)")
    Shortening(&'a str),
    /// Syllable lengthening (:, ::, :::): the run of colons as written.
    Lengthening(LengtheningRun<'a>),
    /// Compound marker (+), as written.
    CompoundMarker(&'a str),
    /// Stress marker (primary ˈ or secondary ˌ), as written.
    Stress(StressKind, &'a str),
    /// Overlap point (⌈, ⌉, ⌊, ⌋ with optional index)
    OverlapPoint(OverlapKind, &'a str),
    /// Syllable pause (^), as written.
    SyllablePause(&'a str),
    /// Clitic boundary (~), as written.
    CliticBoundary(&'a str),
    /// CA element (single symbol like ↑, ↓, ≠, etc.), as written.
    CaElement(CaElementKind, &'a str),
    /// CA delimiter (paired like °softer°, ∆faster∆, etc.), as written.
    CaDelimiter(CaDelimiterKind, &'a str),
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
/// at least one): both producers, the character scanner and the subtoken
/// word builder, admit their run through [`Self::new`] instead of each
/// checking a count. The count is proven once, at admission, and stored, so
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
        let count = std::num::NonZeroUsize::new(run.len())?;
        run.bytes()
            .all(|b| b == b':')
            .then_some(Self { text: run, count })
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
}

/// Primary vs secondary stress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StressKind {
    Primary,
    Secondary,
}

/// Overlap point direction and position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OverlapKind {
    TopBegin,
    TopEnd,
    BottomBegin,
    BottomEnd,
}

/// CA element types, one per symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CaElementKind {
    BlockedSegments, // ≠
    Constriction,    // ∾
    Hardening,       // ☇
    HurriedStart,    // ⇗
    Inhalation,      // ∙
    LaughInWord,     // ꓸ
    PitchDown,       // ↓
    PitchReset,      // ↕
    PitchUp,         // ↑
    SuddenStop,      // ≋
}

/// CA delimiter types, paired markers that scope content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CaDelimiterKind {
    Unsure,            // ⁇
    Precise,           // §
    Creaky,            // ⁎
    Softer,            // °
    SegmentRepetition, // ↫
    Faster,            // ∆
    Slower,            // ∇
    Whisper,           // ∬
    Singing,           // ∮
    LowPitch,          // ▁
    HighPitch,         // ▔
    Louder,            // ◉
    SmileVoice,        // ☺
    BreathyVoice,      // ♋
    Yawn,              // Ϋ
}
