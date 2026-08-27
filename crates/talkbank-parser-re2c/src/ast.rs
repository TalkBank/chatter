//! AST types, mirrors talkbank-model structure.
//!
//! Focused on main tier for now. Will expand to full ChatFile.

use crate::token::Token;
use serde::Serialize;

/// A parsed main tier: *SPEAKER:\t tier_body
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MainTier<'a> {
    pub speaker: Token<'a>,
    pub tier_body: TierBody<'a>,
}

/// grammar.js: tier_body = seq(
///   optional(linkers),
///   optional(seq(langcode, whitespaces)),
///   contents,
///   utterance_end
/// )
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TierBody<'a> {
    pub linkers: Vec<Token<'a>>,
    pub langcode: Option<Token<'a>>,
    pub contents: Vec<ContentItem<'a>>,
    pub terminator: Option<Token<'a>>,
    pub postcodes: Vec<Token<'a>>,
    pub media_bullet: Option<Token<'a>>,
}

/// Which terminator a tier ends with.
///
/// One owner for the thirteen terminator tokens, keyed on the DISCRIMINANT
/// because one consumer (the line-phase machine in `parser/file.rs`) has only
/// that. Before this, `classify::is_terminator` and the converter's
/// `token_to_terminator` each carried the same thirteen with nothing binding
/// them, which is the duplication the separator work had just removed one
/// function over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TerminatorKindParsed {
    Period,
    Question,
    Exclamation,
    TrailingOff,
    Interruption,
    SelfInterruption,
    InterruptedQuestion,
    BrokenQuestion,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    TrailingOffQuestion,
    BreakForCoding,
}

impl TerminatorKindParsed {
    /// THE list. Every other terminator question is derived from this.
    pub fn from_discriminant(d: crate::token::TokenDiscriminants) -> Option<Self> {
        use crate::token::TokenDiscriminants as D;
        Some(match d {
            D::Period => Self::Period,
            D::Question => Self::Question,
            D::Exclamation => Self::Exclamation,
            D::TrailingOff => Self::TrailingOff,
            D::Interruption => Self::Interruption,
            D::SelfInterruption => Self::SelfInterruption,
            D::InterruptedQuestion => Self::InterruptedQuestion,
            D::BrokenQuestion => Self::BrokenQuestion,
            D::QuotedNewLine => Self::QuotedNewLine,
            D::QuotedPeriodSimple => Self::QuotedPeriodSimple,
            D::SelfInterruptedQuestion => Self::SelfInterruptedQuestion,
            D::TrailingOffQuestion => Self::TrailingOffQuestion,
            D::BreakForCoding => Self::BreakForCoding,
            _ => return None,
        })
    }

    /// The same question asked of a token.
    pub fn from_token(tok: &Token<'_>) -> Option<Self> {
        Self::from_discriminant(crate::token::TokenDiscriminants::from(tok))
    }
}

/// Which pause a `(.)`, `(..)`, `(...)` or timed pause is.
///
/// The AST used to store the raw `Token` here, which meant both converters had
/// to re-derive the kind from a ~180-variant enum and end in
/// `_ => PauseDuration::Short`. That arm was not `unreachable!()`, it was a
/// SILENT WRONG ANSWER: any token arriving unexpectedly became a short pause,
/// in two places, and looked like a reasonable default while doing it.
///
/// The parser already knows the kind at construction (`main_tier.rs` matches
/// `Token::PauseShort` and friends one by one), so recording it here costs
/// nothing and makes both conversions exhaustive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PauseKindParsed<'a> {
    /// `(.)`
    Short,
    /// `(..)`
    Medium,
    /// `(...)`
    Long,
    /// `(1.5)` and friends; carries the duration text verbatim.
    Timed(&'a str),
}

/// Which separator a `ContentItem::Separator` is.
///
/// The converter used to re-derive this from a raw `Token` and end in
/// `_ => Separator::Comma`, which is the pause bug with worse consequences:
/// commas are load-bearing for validation (`ContentItem::comma_span` feeds
/// E258), so an unexpected token became a FABRICATED comma that a rule could
/// then report on. The parser knows which separator it matched, so it records
/// it and the conversion becomes exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SeparatorKindParsed {
    Comma,
    Semicolon,
    Colon,
    CaContinuation,
    Tag,
    Vocative,
    UnmarkedEnding,
    Uptake,
    CaNoBreak,
    CaTechnicalBreak,
    RisingToHigh,
    RisingToMid,
    Level,
    FallingToMid,
    FallingToLow,
}

impl SeparatorKindParsed {
    /// THE list of which token is which separator.
    ///
    /// One owner, because there were briefly FOUR: this enum, the parser's
    /// `select!`, the converter's kind-to-model map, and `classify::is_separator`.
    /// Only one of those pairs was compiler-checked, and the drift they allowed
    /// is not benign: a separator token added to the lexer but missed here makes
    /// the `%wor` item parser stop matching, which surfaces as E316 "unparsable
    /// tier content" on valid CHAT. That is precisely the failure the `%wor`
    /// language-precode bug caused three lines from the same call site, 510
    /// times over a 2% corpus sample.
    pub fn from_token(tok: &Token<'_>) -> Option<Self> {
        Some(match tok {
            Token::Comma(_) => Self::Comma,
            Token::Semicolon(_) => Self::Semicolon,
            Token::Colon(_) => Self::Colon,
            Token::CaContinuationMarker(_) => Self::CaContinuation,
            Token::TagMarker(_) => Self::Tag,
            Token::VocativeMarker(_) => Self::Vocative,
            Token::UnmarkedEnding(_) => Self::UnmarkedEnding,
            Token::UptakeSymbol(_) => Self::Uptake,
            Token::CaNoBreak(_) => Self::CaNoBreak,
            Token::CaTechnicalBreak(_) => Self::CaTechnicalBreak,
            Token::RisingToHigh(_) => Self::RisingToHigh,
            Token::RisingToMid(_) => Self::RisingToMid,
            Token::LevelPitch(_) => Self::Level,
            Token::FallingToMid(_) => Self::FallingToMid,
            Token::FallingToLow(_) => Self::FallingToLow,
            _ => return None,
        })
    }

    /// The CHAT symbol this separator is written as.
    ///
    /// Exact rather than approximate: every one of the fifteen lexer rules
    /// matches a fixed literal (verified in `lexer.re`), so there is no
    /// source variation for this to lose. It lets the `%wor` tier carry a
    /// resolved kind instead of a raw token.
    pub fn chat_text(self) -> &'static str {
        match self {
            Self::Comma => ",",
            Self::Semicolon => ";",
            Self::Colon => ":",
            Self::CaContinuation => "[^c]",
            Self::Tag => "\u{201E}",
            Self::Vocative => "\u{2021}",
            Self::UnmarkedEnding => "\u{221E}",
            Self::Uptake => "\u{2261}",
            Self::CaNoBreak => "\u{2248}",
            Self::CaTechnicalBreak => "\u{224B}",
            Self::RisingToHigh => "\u{21D7}",
            Self::RisingToMid => "\u{2197}",
            Self::Level => "\u{2192}",
            Self::FallingToMid => "\u{2198}",
            Self::FallingToLow => "\u{21D8}",
        }
    }
}

/// grammar.js: contents = repeat1(choice(whitespaces, content_item, separator, overlap_point))
/// Whitespace is not stored, structural only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ContentItem<'a> {
    /// A word with optional trailing annotations.
    Word(WordWithAnnotations<'a>),
    /// grammar.js: pause_token
    Pause(PauseKindParsed<'a>),
    /// Freecode (`[^ text]`), a first-class content item. Carries the
    /// tag-extracted text, not the token.
    Freecode(&'a str),
    /// An annotation with nothing to scope over, e.g. a `[/]` opening an
    /// utterance. Ungrammatical, and the subject of E759.
    ///
    /// `grammar.js` has no standalone-annotation production at all: an
    /// annotation is always attached to a word or nonword. This variant exists
    /// so the invalid construct survives far enough to be REPORTED, not
    /// because it is content.
    ///
    /// It carried a raw `Token` until 2026-08-09, justified in this comment by
    /// "E759 quotes the marker as written". That was false, and the lexer said
    /// so: three of the seven trigger tokens are tag-extracted, so re2c
    /// reported `Annotation '1'` where tree-sitter reported `Annotation '[<1]'`.
    /// A typed annotation plus [`ParsedAnnotation::chat_text`] reconstructs the
    /// written form and makes the two backends agree.
    OrphanAnnotation(ParsedAnnotation<'a>),
    /// Separator: comma, semicolon, intonation contours, etc.
    ///
    /// Carries the source `text` as well as the kind, because the converter
    /// needs the POSITION and this is the only thing that still knows it. The
    /// variant held a bare kind until 2026-08-27, so `separator_from_kind`
    /// opened with `let s = Span::DUMMY;` and every separator reached the
    /// model at offset zero, which silently disabled every validation rule
    /// that reads a separator span (E258 among them).
    Separator {
        /// Which separator this is.
        kind: SeparatorKindParsed,
        /// The separator's own text, borrowed from the source, so
        /// [`crate::source_text::SourceText::span_of`] can place it.
        text: &'a str,
    },
    /// grammar.js: overlap_point, with its kind and index already resolved.
    ///
    /// Typed for the reason [`PauseKindParsed`] was: the parser selects the
    /// four overlap tokens one by one, so it knows the kind at construction,
    /// and storing a raw `Token` forced the converter to re-derive it and
    /// invent a fallback for a shape that cannot arrive. Both call sites then
    /// turned an overlap marker into a WORD on that dead branch.
    OverlapPoint {
        kind: OverlapKind,
        /// `⌈2` and friends; the digit written after the marker.
        index: Option<u32>,
    },
    /// Retrace: word(s) followed by [/], [//], [///], [/-], or [/?]
    /// The retraced content is wrapped here; the corrected content follows.
    Retrace(Retrace<'a>),
    /// grammar.js: group_with_annotations = seq(<, contents, >, annotations)
    Group(Group<'a>),
    /// grammar.js: quotation = seq(left_double_quote, contents, right_double_quote)
    Quotation(Quotation<'a>),
    /// Bare event (`&=description`) with no annotations.
    ///
    /// One token, not a `Vec`. The only constructor ever built `vec![event]`,
    /// so the container could not hold anything else, and both converters read
    /// it as `.first().map(..).unwrap_or("")`: an empty-string sentinel for a
    /// state the parser cannot produce. A `Vec` that is always length one is a
    /// dead axis, and its emptiness case is a wrong answer waiting for a
    /// reader who does not know that.
    Event(&'a str),
    /// Event with annotations: `&=description [annotation1] [annotation2] ...`
    /// grammar.js: nonword_with_optional_annotations wraps events.
    /// Retrace markers are dropped (not applicable to events).
    AnnotatedEvent {
        /// The event description, as the lexer tag-extracted it.
        event: &'a str,
        annotations: Vec<ParsedAnnotation<'a>>,
    },
    /// Media bullet, with its two timestamps already separated by the lexer.
    ///
    /// Typed for the same reason as [`PauseKindParsed`]: storing the raw
    /// `Token` forced every consumer to re-destructure a ~180-variant enum and
    /// invent a fallback for a shape the parser never builds.
    MediaBullet {
        /// Start timestamp text, digits as written.
        start: &'a str,
        /// End timestamp text, digits as written.
        end: &'a str,
    },
    // `CaMarker` was deleted 2026-08-09: nothing constructed it. CA element
    // and delimiter tokens are consumed by the word-body scanner into
    // `WordBodyItem::CaElement` / `CaDelimiter`, so they never reach content
    // level, yet both converters carried an arm for the state and one of them
    // built a whole `Word` for it. The five label variants below carry
    // `&'a str` rather than a `Token`: every consumer read them as
    // `tok.text()` on an already tag-extracted token, so the token was a
    // wrapper around the one field anyone wanted.
    /// Underline begin marker (`␂␁`). Carries no payload: both converters
    /// ignored the token entirely, so the field only kept a wrong one
    /// constructible.
    UnderlineBegin,
    /// Underline end marker (`␂␂`). See [`ContentItem::UnderlineBegin`].
    UnderlineEnd,
    /// Other spoken event (`&*SPK:word`), with its two fields separated.
    ///
    /// Replaces a raw `Token` whose only purpose was to be re-destructured in
    /// the converter behind an `unreachable!()`, i.e. a panic in a parser that
    /// existed solely because the variant could hold a token the parser never
    /// puts there.
    OtherSpokenEvent { speaker: &'a str, text: &'a str },
    /// Phonological group: ‹ contents ›
    PhoGroup(Vec<ContentItem<'a>>),
    /// Sign group: 〔 contents 〕
    SinGroup(Vec<ContentItem<'a>>),
    /// Long feature begin (`&{l=LABEL`); carries the label alone.
    LongFeatureBegin(&'a str),
    /// Long feature end (`&}l=LABEL`); carries the label alone.
    LongFeatureEnd(&'a str),
    /// Nonvocal begin (`&{n=LABEL`); carries the label alone.
    NonvocalBegin(&'a str),
    /// Nonvocal end (`&}n=LABEL`); carries the label alone.
    NonvocalEnd(&'a str),
    /// Nonvocal simple, self-closing (`&{n=LABEL}`); carries the label alone.
    NonvocalSimple(&'a str),
    /// Standalone zero (0), action without speech.
    /// grammar.js: nonword = choice(event, zero)
    /// With annotations: annotated_action; without: bare action.
    Action {
        /// The omission marker as written; always `0` today, kept as text
        /// because `%wor` reconstructs `raw_text` from it.
        zero: &'a str,
        annotations: Vec<ParsedAnnotation<'a>>,
    },
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
    /// Form marker suffix: tag-extracted content (e.g., "f", "z:grm"). None if absent.
    pub form_marker: Option<&'a str>,
    /// Language suffix. None if absent.
    pub lang: Option<ParsedLangSuffix<'a>>,
    /// POS tag: tag-extracted content (e.g., "n", "adj"). None if absent.
    pub pos_tag: Option<&'a str>,
    /// Trailing scoped annotations: `[*]`, `[= text]`, `[/]`, `[!]`, etc.
    pub annotations: Vec<ParsedAnnotation<'a>>,
    /// Raw text of the entire word, sliced directly from source.
    /// Eliminates the need for `source` in conversion, the AST is self-contained.
    pub raw_text: &'a str,
}

/// A parsed scoped annotation. Tag-extracted content, no delimiters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParsedAnnotation<'a> {
    /// `[/]`, `[//]`, `[///]`, `[/-]`, retrace markers
    Retrace(RetraceKindParsed),
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
    /// `[: replacement words]`, replacement
    Replacement(&'a str),
    /// `[- lang]`, language code (on utterance, not word, but can appear in annotation position)
    Langcode(&'a str),
    /// `[+ code]`, postcode (rare in word annotation position)
    Postcode(&'a str),
    /// A bracketed annotation whose marker no rule recognises. Content is the
    /// text BETWEEN the brackets, so `[@ xyz]` carries `"@ xyz"`.
    ///
    /// Reaching the model as `ContentAnnotation::Unknown` is the point: the
    /// validator then reports E207, which is a statement about the FILE, where
    /// a parse failure (E321) is a statement about the parser.
    Unknown(&'a str),
}

impl<'a> ParsedAnnotation<'a> {
    /// The annotation as WRITTEN in the source, reconstructed.
    ///
    /// Needed because the lexer tag-extracts most annotation payloads: the
    /// token for `[<1]` carries `"1"`, and for `[: word]` it carries `"word"`.
    /// E759 quotes the offending marker, so reporting the payload alone gave
    /// `Annotation '1' at utterance start`, which tells a reader nothing, while
    /// the tree-sitter backend reported `Annotation '[<1]'`. Reconstructing
    /// here makes the two agree.
    ///
    /// Returns an owned `String` only because the bracketed forms have to be
    /// rebuilt; the fixed markers borrow nothing and allocate nothing beyond
    /// this, and the whole function runs once per E759, a construct that is
    /// invalid CHAT.
    pub fn chat_text(&self) -> String {
        match self {
            Self::Unknown(inner) => format!("[{inner}]"),
            Self::Retrace(kind) => match kind {
                RetraceKindParsed::Partial => "[/]".to_owned(),
                RetraceKindParsed::Complete => "[//]".to_owned(),
                RetraceKindParsed::Multiple => "[///]".to_owned(),
                RetraceKindParsed::Reformulation => "[/-]".to_owned(),
            },
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
            Self::Replacement(text) => format!("[: {text}]"),
            Self::Langcode(code) => format!("[- {code}]"),
            Self::Postcode(code) => format!("[+ {code}]"),
        }
    }

    /// Whether this annotation is POSTFIX: it scopes over material to its left,
    /// so an utterance may not begin with it (E759, mirroring CLAN CHECK 52).
    ///
    /// A property of the annotation kind, asked of the type rather than
    /// re-derived by listing seven token variants at the reporting site.
    pub fn is_postfix(&self) -> bool {
        matches!(
            self,
            Self::Retrace(_)
                | Self::OverlapPrecedes(_)
                | Self::OverlapFollows(_)
                | Self::Replacement(_)
        )
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
    Lengthening(u8),
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
impl<'a> ContentItem<'a> {
    /// The content this item encloses. Empty for a leaf.
    ///
    /// # Why this exists
    ///
    /// The one place that knows which variants of this enum contain other
    /// content. Every recursive walker needs that fact, and until this existed
    /// each answered it with its own match, which is how
    /// `has_synthesized_missing_annotation` came to descend into `Retrace`,
    /// `Group` and `Quotation` but not `PhoGroup` or `SinGroup`: a retrace
    /// carrying a synthesized missing annotation inside `‹...›` was invisible
    /// to it.
    ///
    /// EXHAUSTIVENESS DOES NOT CATCH THAT. A walker can list every variant and
    /// still answer `false` for a container, so `_ => false` and
    /// `PhoGroup(_) => false` are the same defect and
    /// `clippy::wildcard_enum_match_arm` only sees the first. Descent has to be
    /// structural, which is what this accessor makes it: a caller recurses on
    /// `children()` and cannot skip a container, because it never enumerates
    /// them.
    ///
    /// The four container payloads have three different field spellings
    /// (`Retrace.content`, `Group.contents`/`Quotation.contents`, and the bare
    /// `Vec` of the pho/sin groups), which is precisely why a recursion author
    /// stops after the two that look alike.
    ///
    /// `talkbank-model` solved the same problem for its own content enums with
    /// `model::content::structure::ContentStructure`; this is that idea for
    /// re2c's separate AST, which cannot share the type.
    #[inline]
    pub fn children(&self) -> &[ContentItem<'a>] {
        match self {
            Self::Retrace(retrace) => &retrace.content,
            Self::Group(group) => &group.contents,
            Self::Quotation(quotation) => &quotation.contents,
            Self::PhoGroup(items) | Self::SinGroup(items) => items,
            Self::Word(_)
            | Self::Pause(_)
            | Self::Freecode(_)
            | Self::OrphanAnnotation(_)
            | Self::Separator { .. }
            | Self::OverlapPoint { .. }
            | Self::Event(_)
            | Self::AnnotatedEvent { .. }
            | Self::MediaBullet { .. }
            | Self::UnderlineBegin
            | Self::UnderlineEnd
            | Self::OtherSpokenEvent { .. }
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_)
            | Self::Action { .. } => &[],
        }
    }
}

/// Retraced content: words the speaker said then corrected.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Retrace<'a> {
    /// The retraced content (words that were corrected).
    pub content: Vec<ContentItem<'a>>,
    /// The retrace kind.
    pub kind: RetraceKindParsed,
    /// Whether this retrace originated from a `<group> [/]` (angle brackets).
    pub is_group: bool,
    /// True iff this retrace is a RECOVERY from a `<...>` group that had no
    /// following annotation. `group_with_annotations` requires one; on its
    /// absence both parsers recover to `Retrace { kind: Complete, is_group:
    /// true, annotations: [] }`, model-indistinguishable from a real
    /// `<...> [//]`. This flag is the only thing that distinguishes the
    /// synthesized case, so the file-level parser can emit a matching MISSING
    /// diagnostic (E342) on it (recovery is not validity; see this crate's
    /// MISSING-Token Recovery Policy). It does NOT affect model conversion.
    pub synthesized_missing_annotation: bool,
    /// Non-retrace annotations that followed the retrace marker (e.g., `[?]` after `[/]`).
    /// In grammar.js, annotations attach to `word_with_optional_annotations`, so
    /// they belong to the retrace, not the word inside it.
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

/// Retrace kind, matches grammar.js retrace variants exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RetraceKindParsed {
    /// `[/]`
    Partial,
    /// `[//]`
    Complete,
    /// `[///]`
    Multiple,
    /// `[/-]`
    Reformulation,
}

/// grammar.js: group_with_annotations
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group<'a> {
    pub contents: Vec<ContentItem<'a>>,
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

/// grammar.js: quotation
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quotation<'a> {
    pub contents: Vec<ContentItem<'a>>,
}

// ═══════════════════════════════════════════════════════════════
// Header ASTs
// ═══════════════════════════════════════════════════════════════

/// Parsed @ID header fields.
/// Mirrors talkbank_model::IDHeader.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IdHeaderParsed<'a> {
    pub language: &'a str,
    pub corpus: &'a str,
    pub speaker: &'a str,
    pub age: &'a str,
    pub sex: &'a str,
    pub group: &'a str,
    pub ses: &'a str,
    pub role: &'a str,
    pub education: &'a str,
    pub custom_field: &'a str,
}

/// Parsed @Languages header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LanguagesHeaderParsed<'a> {
    pub codes: Vec<&'a str>,
}

/// Parsed @Participants header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParticipantsHeaderParsed<'a> {
    pub entries: Vec<ParticipantEntryParsed<'a>>,
}

/// A single participant entry: SPK Name Role
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParticipantEntryParsed<'a> {
    pub words: Vec<&'a str>,
}

/// Parsed @Media header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaHeaderParsed<'a> {
    pub fields: Vec<&'a str>,
}

/// Parsed @Types header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypesHeaderParsed<'a> {
    pub raw: &'a str,
    pub design: &'a str,
    pub activity: &'a str,
    pub group: &'a str,
}

/// A generic header (prefix + content tokens).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeaderParsed<'a> {
    pub prefix: Token<'a>,
    /// All content tokens (may be empty for @UTF8, @Begin, @End, etc.)
    pub content: Vec<Token<'a>>,
}

// ═══════════════════════════════════════════════════════════════
// Full file AST
// ═══════════════════════════════════════════════════════════════

/// A parsed CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatFile<'a> {
    pub lines: Vec<Line<'a>>,
    /// Original source text, needed for lossless raw_text reconstruction via spans.
    pub source: &'a str,
}

/// A line in a CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Line<'a> {
    Header(HeaderParsed<'a>),
    Utterance(Box<Utterance<'a>>),
}

/// An utterance: main tier + dependent tiers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance<'a> {
    pub main_tier: MainTier<'a>,
    pub dependent_tiers: Vec<DependentTierParsed<'a>>,
}

/// A parsed dependent tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DependentTierParsed<'a> {
    Mor(MorTier<'a>),
    Gra(GraTier<'a>),
    Pho(PhoTier<'a>),
    Mod(PhoTier<'a>),
    Sin(SinTierParsed<'a>),
    /// %wor tier: words with optional inline timing bullets.
    Wor(WorTierParsed<'a>),
    /// Generic text tier (content is raw text segments + bullets).
    Text {
        prefix: Token<'a>,
        content: Vec<Token<'a>>,
    },
}

/// A parsed `%wor` tier: an optional language precode, the timed items, and an
/// optional terminator.
///
/// A named struct rather than a tuple because adding the language precode made
/// it a three-slot return, and a bare tuple that wide is a seam nobody can read
/// at the call site. It is also how the precode came to be DROPPED: `WorTier`
/// has carried `language_code: Option<LanguageCode>` all along, documented with
/// the `[- spa]` form, and this parser simply had nowhere to put one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorTierParsed<'a> {
    /// `[- zho]` and friends, written before the first timed word.
    pub langcode: Option<Token<'a>>,
    pub items: Vec<WorItemParsed<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A parsed %wor item: word with optional timing bullet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorItemParsed<'a> {
    /// A word with optional timing bullet.
    Word {
        word: WordWithAnnotations<'a>,
        bullet: Option<(u64, u64)>,
    },
    /// Separator (comma, tag marker, vocative marker).
    Separator(SeparatorKindParsed),
}

// ═══════════════════════════════════════════════════════════════
// %pho tier AST
// ═══════════════════════════════════════════════════════════════

/// Parsed %pho tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhoTier<'a> {
    pub items: Vec<PhoItemParsed<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A parsed %pho item: either a single word or a ‹group› of words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PhoItemParsed<'a> {
    /// Single word (possibly compound with +)
    Word(PhoWordParsed<'a>),
    /// ‹grouped words›
    Group(Vec<PhoWordParsed<'a>>),
}

/// A phonological word (possibly compound with +).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhoWordParsed<'a> {
    pub segments: Vec<&'a str>,
}

/// A parsed %sin tier, gesture/sign words with optional 〔groups〕.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SinTierParsed<'a> {
    pub items: Vec<SinItemParsed<'a>>,
}

/// A single %sin item: either a token or a 〔group〕.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SinItemParsed<'a> {
    Token(&'a str),
    Group(Vec<&'a str>),
}

// ═══════════════════════════════════════════════════════════════
// Text tier AST (for %com, %act, %cod, %exp, etc.)
// ═══════════════════════════════════════════════════════════════

/// Parsed text tier content (text_with_bullets).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextTierParsed<'a> {
    pub segments: Vec<TextTierSegment<'a>>,
}

/// A segment in a text tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TextTierSegment<'a> {
    Text(&'a str),
    Bullet(Token<'a>),
    Pic(Token<'a>),
}

// ═══════════════════════════════════════════════════════════════
// %mor tier AST, mirrors talkbank_model::MorTier/Mor/MorWord
// ═══════════════════════════════════════════════════════════════

/// Parsed %mor tier.
/// grammar.js: mor_contents = seq(mor_content+, optional(terminator))
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorTier<'a> {
    pub items: Vec<MorItem<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A single %mor item: main word + optional post-clitics.
/// grammar.js: mor_content = seq(mor_word, repeat(seq(tilde, mor_word)))
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorItem<'a> {
    pub main: MorWordParsed<'a>,
    pub post_clitics: Vec<MorWordParsed<'a>>,
}

/// A parsed %mor word: POS, lemma, features.
/// Extracted from a single MorWord rich token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorWordParsed<'a> {
    /// Part-of-speech tag (e.g., "verb", "pro:sub")
    pub pos: &'a str,
    /// Lemma/stem (e.g., "want", "I")
    pub lemma: &'a str,
    /// Feature values (e.g., ["Fin", "Ind", "Pres"])
    pub features: Vec<&'a str>,
}

// ═══════════════════════════════════════════════════════════════
// %gra tier AST, mirrors talkbank_model::GraTier/GrammaticalRelation
// ═══════════════════════════════════════════════════════════════

/// Parsed %gra tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraTier<'a> {
    pub relations: Vec<GraRelationParsed<'a>>,
}

/// A parsed %gra relation: index, head, relation name.
/// Extracted from a single GraRelation rich token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraRelationParsed<'a> {
    pub index: &'a str,
    pub head: &'a str,
    pub relation: &'a str,
}

#[cfg(test)]
mod children_tests {
    use super::*;

    /// A distinguishable leaf to look for on the far side of a container.
    fn marker<'a>() -> ContentItem<'a> {
        ContentItem::PhoGroup(vec![])
    }

    fn retrace<'a>(content: Vec<ContentItem<'a>>) -> ContentItem<'a> {
        ContentItem::Retrace(Retrace {
            content,
            kind: RetraceKindParsed::Complete,
            is_group: false,
            synthesized_missing_annotation: false,
            annotations: vec![],
        })
    }

    /// SURVIVES: policy. WHICH variants of this enum enclose content is a fact
    /// about the CHAT model, not something a signature can carry: nothing stops
    /// `children()` returning `&[]` for a real container, which is exactly the
    /// defect it exists to prevent.
    ///
    /// This is the ONE test the class needs, and it is deliberately on the owner
    /// rather than on each walker. Before `children()` every recursion answered
    /// "which variants nest" itself, so guarding the class meant a test per
    /// walker, and the walker that never got one
    /// (`has_synthesized_missing_annotation`) is the one that silently skipped
    /// `PhoGroup` and `SinGroup`.
    ///
    /// Exhaustiveness cannot replace it. A `match` listing every variant and
    /// answering `&[]` for a container compiles clean and passes clippy, so
    /// `_ => &[]` and `PhoGroup(_) => &[]` are the same bug and only the first
    /// is a lint. The compiler guards that every variant is CONSIDERED; this
    /// guards that each is considered CORRECTLY.
    #[test]
    fn every_container_yields_its_children() {
        let cases: Vec<(&str, ContentItem<'_>)> = vec![
            ("Retrace", retrace(vec![marker()])),
            (
                "Group",
                ContentItem::Group(Group {
                    contents: vec![marker()],
                    annotations: vec![],
                }),
            ),
            (
                "Quotation",
                ContentItem::Quotation(Quotation {
                    contents: vec![marker()],
                }),
            ),
            ("PhoGroup", ContentItem::PhoGroup(vec![marker()])),
            ("SinGroup", ContentItem::SinGroup(vec![marker()])),
        ];

        for (name, item) in cases {
            assert_eq!(
                item.children().len(),
                1,
                "{name} encloses content but children() did not yield it; a walker \
                 recursing through children() would silently skip everything inside it"
            );
        }
    }

    /// SURVIVES: policy. That a leaf yields nothing is the other half of the
    /// same fact, and a `children()` that returned its own item would loop
    /// forever rather than answer wrong.
    #[test]
    fn a_leaf_yields_nothing() {
        assert!(ContentItem::Event("laughs").children().is_empty());
    }

    /// SURVIVES: behaviour. Descent through `children()` reaches content nested
    /// inside a pho group, which is the precise case
    /// `has_synthesized_missing_annotation` used to miss.
    #[test]
    fn descent_reaches_content_inside_a_pho_group() {
        let nested = ContentItem::PhoGroup(vec![retrace(vec![])]);
        let found = nested
            .children()
            .iter()
            .any(|item| matches!(item, ContentItem::Retrace(_)));
        assert!(found, "a retrace inside a pho group must be reachable");
    }
}
