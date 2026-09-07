//! Word bodies, annotation categories and their source representations.

use super::RetraceKindParsed;
use serde::Serialize;

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
    /// Form marker suffix: tag-extracted content (e.g., "f", "z:grm"). None if absent.
    pub form_marker: Option<&'a str>,
    /// Language suffix. None if absent.
    pub lang: Option<ParsedLangSuffix<'a>>,
    /// POS tag: tag-extracted content (e.g., "n", "adj"). None if absent.
    pub pos_tag: Option<&'a str>,
    /// Trailing scoped annotations: `[*]`, `[= text]`, `[/]`, `[!]`, etc.
    pub annotations: Vec<ParsedAnnotation<'a>>,
    /// Raw word text: borrowed from source for rich tokens, owned when rebuilt
    /// from subtoken display forms. Only borrowed text can recover a source span.
    pub raw_text: std::borrow::Cow<'a, str>,
}

/// A bracketed construct, classified once by the token-to-AST producer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParsedAnnotation<'a> {
    /// A scoped annotation; conversion to ContentAnnotation is total.
    Scoped(ScopedAnnotationParsed<'a>),
    /// Retrace markers change the preceding content structure.
    Retrace(RetraceKindParsed),
    /// A replacement changes the word's structure.
    Replacement(&'a str),
    /// An utterance language code.
    Langcode(&'a str),
    /// An utterance postcode.
    Postcode(&'a str),
}

/// A parsed scoped annotation. Tag-extracted content, no delimiters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ScopedAnnotationParsed<'a> {
    /// `[!]`, stressing
    Stressing,
    /// `[!!]`, contrastive stressing
    ContrastiveStressing,
    /// `[?]`, uncertain
    Uncertain,
    /// `[e]`, exclude
    Exclude,
    /// `[@s]`, code-switch span resolving the way a bare `word@s` does.
    CodeSwitchShortcut,
    /// `[@s:code]`, code-switch span naming its language. Content is the code.
    CodeSwitchExplicit(&'a str),
    /// `[* code]`, error marker. Content is the code (may be empty).
    Error(&'a str),
    /// `[<]` or `[<1]`, overlap precedes. Content is the optional index digit.
    OverlapPrecedes(&'a str),
    /// `[>]` or `[>1]`, overlap follows
    OverlapFollows(&'a str),
    /// `[= text]`, explanation
    Explanation(&'a str),
    /// `[=! text]`, paralinguistic
    Paralinguistic(&'a str),
    /// `[=? text]`, alternative
    Alternative(&'a str),
    /// `[% text]`, percent comment
    PercentComment(&'a str),
    /// A bracketed annotation whose marker no rule recognises. Content is the
    /// text BETWEEN the brackets, so `[@ xyz]` carries `"@ xyz"`.
    ///
    /// Reaching the model as `ContentAnnotation::Unknown` is the point: the
    /// validator then reports E207, which is a statement about the FILE, where
    /// a parse failure (E321) is a statement about the parser.
    Unknown(&'a str),
}

impl<'a> ParsedAnnotation<'a> {
    /// The source spelling reconstructed from the admitted annotation kind.
    pub fn chat_text(&self) -> String {
        match self {
            Self::Scoped(scoped) => scoped.chat_text(),
            Self::Retrace(kind) => match kind {
                RetraceKindParsed::Partial => "[/]".to_owned(),
                RetraceKindParsed::Complete => "[//]".to_owned(),
                RetraceKindParsed::Multiple => "[///]".to_owned(),
                RetraceKindParsed::Reformulation => "[/-]".to_owned(),
            },
            Self::Replacement(text) => format!("[: {text}]"),
            Self::Langcode(code) => format!("[- {code}]"),
            Self::Postcode(code) => format!("[+ {code}]"),
        }
    }

    /// Scoped annotations have a total model conversion, unlike structural codes.
    pub fn scoped(&self) -> Option<&ScopedAnnotationParsed<'a>> {
        match self {
            Self::Scoped(scoped) => Some(scoped),
            Self::Retrace(_) | Self::Replacement(_) | Self::Langcode(_) | Self::Postcode(_) => None,
        }
    }

    /// Return replacement content directly, never an index to look up again.
    pub fn replacement_text(&self) -> Option<&'a str> {
        match self {
            Self::Replacement(text) => Some(text),
            Self::Scoped(_) | Self::Retrace(_) | Self::Langcode(_) | Self::Postcode(_) => None,
        }
    }

    /// The borrowed source payload, when the marker carries one.
    pub fn content_slice(&self) -> Option<&'a str> {
        match self {
            Self::Scoped(scoped) => scoped.content_slice(),
            Self::Replacement(text) | Self::Langcode(text) | Self::Postcode(text) => Some(text),
            Self::Retrace(_) => None,
        }
    }

    /// Postfix constructs cannot start an utterance (E759).
    pub fn is_postfix(&self) -> bool {
        matches!(
            self,
            Self::Retrace(_)
                | Self::Replacement(_)
                | Self::Scoped(
                    ScopedAnnotationParsed::OverlapPrecedes(_)
                        | ScopedAnnotationParsed::OverlapFollows(_)
                )
        )
    }
}

impl<'a> ScopedAnnotationParsed<'a> {
    /// Reconstruct a scoped marker, including its delimiters (E759 context).
    pub fn chat_text(&self) -> String {
        match self {
            Self::Unknown(inner) => format!("[{inner}]"),
            Self::Stressing => "[!]".to_owned(),
            Self::ContrastiveStressing => "[!!]".to_owned(),
            Self::Uncertain => "[?]".to_owned(),
            Self::Exclude => "[e]".to_owned(),
            Self::CodeSwitchShortcut => "[@s]".to_owned(),
            Self::CodeSwitchExplicit(code) => format!("[@s:{code}]"),
            Self::Error(code) => format!("[* {code}]"),
            Self::OverlapPrecedes(index) => format!("[<{index}]"),
            Self::OverlapFollows(index) => format!("[>{index}]"),
            Self::Explanation(text) => format!("[= {text}]"),
            Self::Paralinguistic(text) => format!("[=! {text}]"),
            Self::Alternative(text) => format!("[=? {text}]"),
            Self::PercentComment(text) => format!("[% {text}]"),
        }
    }

    /// A source slice exists only for payload-bearing markers.
    pub fn content_slice(&self) -> Option<&'a str> {
        match self {
            Self::Unknown(text)
            | Self::CodeSwitchExplicit(text)
            | Self::Error(text)
            | Self::OverlapPrecedes(text)
            | Self::OverlapFollows(text)
            | Self::Explanation(text)
            | Self::Paralinguistic(text)
            | Self::Alternative(text)
            | Self::PercentComment(text) => Some(text),
            Self::Stressing
            | Self::ContrastiveStressing
            | Self::Uncertain
            | Self::Exclude
            | Self::CodeSwitchShortcut => None,
        }
    }
}

impl ParsedAnnotation<'_> {
    /// Whether this annotation is a retrace marker.
    pub fn is_retrace(&self) -> bool {
        matches!(self, ParsedAnnotation::Retrace(_))
    }

    /// Extract retrace kind if this is a retrace annotation.
    pub fn retrace_kind(&self) -> Option<RetraceKindParsed> {
        match self {
            ParsedAnnotation::Retrace(k) => Some(*k),
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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WordBodyItem<'a> {
    /// Plain text segment (e.g., "hello", "want")
    Text(&'a str),
    /// Shortened syllable, tag-extracted content (e.g., "be" from "(be)")
    Shortening(&'a str),
    /// Syllable lengthening (:, ::, :::), count of colons
    Lengthening(std::num::NonZeroUsize),
    /// Compound marker (+)
    CompoundMarker,
    /// Stress marker (primary ˈ or secondary ˌ)
    Stress(StressKind),
    /// Overlap point (⌈, ⌉, ⌊, ⌋ with optional index)
    OverlapPoint(OverlapKind, &'a str),
    /// Syllable pause (^)
    SyllablePause,
    /// Clitic boundary (~)
    CliticBoundary,
    /// CA element (single symbol like ↑, ↓, ≠, etc.)
    CaElement(CaElementKind),
    /// CA delimiter (paired like °softer°, ∆faster∆, etc.)
    CaDelimiter(CaDelimiterKind),
    /// Word-internal underline begin (`␂␁`), as in `j␂␁a`.
    ///
    /// Underline markers glue to letters inside a word in CA transcripts, and
    /// the word-body scanner used to consume and DISCARD them with a
    /// "skip for now" comment. That silently unbalanced the underline check:
    /// a word-internal begin vanished while a word-initial end survived as a
    /// content-level marker, so `chatter validate --parser re2c` reported 768
    /// spurious E357 "unmatched underline end" across CA corpora that
    /// tree-sitter reads as clean.
    UnderlineBegin,
    /// Word-internal underline end (`␂␂`). See [`WordBodyItem::UnderlineBegin`].
    UnderlineEnd,
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
