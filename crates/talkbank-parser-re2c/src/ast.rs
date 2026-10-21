//! AST types, mirrors talkbank-model structure.
//!
//! Focused on main tier for now. Will expand to full ChatFile.

use crate::token::Token;
use serde::Serialize;

mod file;
mod rejected_tier;
pub use rejected_tier::RejectedMorTier;

mod word;
pub use word::{
    CaDelimiterKind, CaElementKind, LengtheningRun, OverlapKind, ParsedAnnotation,
    ParsedLangSuffix, ReplacementParsed, ScopedAnnotationParsed, ScopedOverlapIndex, StressKind,
    WordBodyItem, WordCategory, WordWithAnnotations,
};

/// A parsed main tier: *SPEAKER:\t tier_body
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MainTier<'a> {
    #[serde(skip)]
    pub separator: talkbank_model::model::TierSeparator,
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
    pub langcode: Option<crate::token::Extracted<'a>>,
    pub contents: Vec<ContentItem<'a>>,
    pub terminator: Option<Token<'a>>,
    pub postcodes: Vec<crate::token::PostcodeToken<'a>>,
    pub media_bullet: Option<crate::token::MediaBulletToken<'a>>,
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
    /// `(.)`, retaining its complete lexical extent.
    Short(crate::token::PauseLexeme<'a>),
    /// `(..)`, retaining its complete lexical extent.
    Medium(crate::token::PauseLexeme<'a>),
    /// `(...)`, retaining its complete lexical extent.
    Long(crate::token::PauseLexeme<'a>),
    /// Numeric payload and full parenthesized extent from one lexer match.
    Timed(crate::token::PauseLexeme<'a>),
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
    /// Freecode (`[^ text]`), a first-class content item. Its payload is the
    /// tag-extracted text, its lexeme the whole `[^ text]`.
    Freecode(crate::token::Extracted<'a>),
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
        /// The marker as written, borrowed from the source, so the converter
        /// can place it.
        text: &'a str,
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
    Event(crate::token::Extracted<'a>),
    /// Event with annotations: `&=description [annotation1] [annotation2] ...`
    /// grammar.js: nonword_with_optional_annotations wraps events.
    /// Retrace markers are dropped (not applicable to events).
    AnnotatedEvent {
        /// The event, with its description as the lexer tag-extracted it.
        event: crate::token::Extracted<'a>,
        annotations: Vec<ParsedAnnotation<'a>>,
    },
    /// Media bullet with lexer-owned numeric admission and original spelling.
    /// Rejected timing remains present until lowering, never a zero sentinel.
    MediaBullet {
        bullet: crate::token::MediaBulletToken<'a>,
    },
    // `CaMarker` was deleted 2026-08-09: nothing constructed it. CA element
    // and delimiter tokens are consumed by the word-body scanner into
    // `WordBodyItem::CaElement` / `CaDelimiter`, so they never reach content
    // level, yet both converters carried an arm for the state and one of them
    // built a whole `Word` for it. The five label variants below carry
    // `&'a str` rather than a `Token`: every consumer read them as
    // `tok.text()` on an already tag-extracted token, so the token was a
    // wrapper around the one field anyone wanted.
    /// Underline begin marker (`␂␁`), as written, so the converter can place
    /// it.
    UnderlineBegin(&'a str),
    /// Underline end marker (`␂␂`). See [`ContentItem::UnderlineBegin`].
    UnderlineEnd(&'a str),
    /// Other spoken event (`&*SPK:word`), with its two fields separated.
    ///
    /// Replaces a raw `Token` whose only purpose was to be re-destructured in
    /// the converter behind an `unreachable!()`, i.e. a panic in a parser that
    /// existed solely because the variant could hold a token the parser never
    /// puts there.
    OtherSpokenEvent {
        speaker: &'a str,
        text: &'a str,
        /// The whole `&*SPK:word`, for placement.
        lexeme: &'a str,
    },
    /// Phonological group: ‹ contents ›
    PhoGroup(Vec<ContentItem<'a>>),
    /// Sign group: 〔 contents 〕
    SinGroup(Vec<ContentItem<'a>>),
    /// Long feature begin (`&{l=LABEL`); its payload is the label alone.
    LongFeatureBegin(crate::token::Extracted<'a>),
    /// Long feature end (`&}l=LABEL`); its payload is the label alone.
    LongFeatureEnd(crate::token::Extracted<'a>),
    /// Nonvocal begin (`&{n=LABEL`); its payload is the label alone.
    NonvocalBegin(crate::token::Extracted<'a>),
    /// Nonvocal end (`&}n=LABEL`); its payload is the label alone.
    NonvocalEnd(crate::token::Extracted<'a>),
    /// Nonvocal simple, self-closing (`&{n=LABEL}`); its payload is the label alone.
    NonvocalSimple(crate::token::Extracted<'a>),
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

impl<'a> ContentItem<'a> {
    /// The content this item encloses. Empty for a leaf.
    ///
    /// # Why this exists
    ///
    /// The one place that knows which variants of this enum contain other
    /// content. Every recursive walker needs that fact, and until this existed
    /// each answered it with its own match, which is how
    /// the missing-annotation walker came to descend into `Retrace`, `Group`
    /// and `Quotation` but not `PhoGroup` or `SinGroup`. A missing annotation
    /// inside `‹...›` was invisible to it.
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
            Self::Retrace(retrace) => retrace.body.items(),
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
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
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

/// The opening and closing delimiters of an enclosed run (`<` and `>`, or the
/// two quotation marks), as written, so the construct can be placed from one
/// to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Enclosure<'a> {
    pub open: &'a str,
    pub close: &'a str,
}

impl Enclosure<'_> {
    /// The extent from the opening delimiter through the closing one, when
    /// both are slices of `source`.
    pub fn span_in(
        self,
        source: crate::source_text::SourceText<'_>,
    ) -> Option<talkbank_model::Span> {
        Some(
            source
                .span_of(self.open)?
                .merge(source.span_of(self.close)?),
        )
    }
}

/// What a retrace marker scopes over: the two shapes the chain fold builds,
/// each carrying exactly what its extent needs, so "no brackets means one
/// item" is the type rather than a convention a converter has to re-check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RetraceBody<'a> {
    /// `<a b> [/]`: a bare group handed its brackets to the retrace (the
    /// model's `is_group`); the extent runs from the opening bracket.
    Enclosed(Enclosure<'a>, Vec<ContentItem<'a>>),
    /// `word [/]`, `<a b> [?] [/]`, `“a” [/]`: the one item the marker
    /// scopes over, whose own extent starts the retrace's.
    Single(Box<ContentItem<'a>>),
}

impl<'a> RetraceBody<'a> {
    /// The retraced items, in order.
    pub fn items(&self) -> &[ContentItem<'a>] {
        match self {
            Self::Enclosed(_, items) => items,
            Self::Single(item) => std::slice::from_ref(item),
        }
    }

    /// The retraced items, owned.
    pub fn into_items(self) -> Vec<ContentItem<'a>> {
        match self {
            Self::Enclosed(_, items) => items,
            Self::Single(item) => vec![*item],
        }
    }
}

/// Retraced content: words the speaker said then corrected.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Retrace<'a> {
    /// What the marker scopes over.
    pub body: RetraceBody<'a>,
    /// The retrace kind.
    pub kind: RetraceKindParsed,
    /// The retrace marker as written (`[/]`, `[//]`, ...), for placement.
    pub marker: &'a str,
    /// Non-retrace annotations that followed the retrace marker (e.g., `[?]` after `[/]`).
    /// In grammar.js, annotations attach to `word_with_optional_annotations`, so
    /// they belong to the retrace, not the word inside it.
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

impl Retrace<'_> {
    /// Whether this retrace originated from a `<group> [/]` (angle brackets).
    pub fn is_group(&self) -> bool {
        matches!(self.body, RetraceBody::Enclosed(..))
    }
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
    /// The `<` and `>` as written.
    pub enclosure: Enclosure<'a>,
    pub contents: Vec<ContentItem<'a>>,
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

/// grammar.js: quotation
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quotation<'a> {
    /// The two quotation marks as written.
    pub enclosure: Enclosure<'a>,
    pub contents: Vec<ContentItem<'a>>,
    /// Markers attached to the quotation, retained by the ordered chain fold.
    pub annotations: Vec<ParsedAnnotation<'a>>,
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

/// A parsed participant header or another header's preserved lexer tokens.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum HeaderParsed<'a> {
    /// Participant syntax has already been parsed and diagnosed; lowering must
    /// consume these entries rather than run a second token-splitting parser.
    Participants(ParticipantsHeaderParsed<'a>),
    /// Other header families retain their lexer representation for lowering.
    Other {
        /// Header discriminator produced by the lexer.
        prefix: Token<'a>,
        /// Content tokens, including whitespace and continuation newlines.
        content: Vec<Token<'a>>,
    },
}

// ═══════════════════════════════════════════════════════════════
// Full file AST
// ═══════════════════════════════════════════════════════════════

pub use file::{
    ChatFile, DependentTierEntryParsed, HeaderProvenance, Line, LocatedMainTier, Utterance,
};

/// A parsed dependent tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DependentTierParsed<'a> {
    /// Rejected morphological syntax, retained only for raw AST inspection.
    RejectedMor(RejectedMorTier<'a>),
    Mor(MorTier<'a>),
    Gra(GraTier<'a>),
    Pho(PhoTier<'a>),
    Mod(PhoTier<'a>),
    Sin(SinTierParsed),
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
    pub langcode: Option<crate::token::Extracted<'a>>,
    pub items: Vec<WorItemParsed<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A parsed %wor item: word with optional timing bullet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorItemParsed<'a> {
    /// A word with optional timing bullet. The whole token is kept, not only
    /// its numeric admission, so the bullet can be placed at its own bytes.
    Word {
        word: Box<WordWithAnnotations<'a>>,
        bullet: Option<crate::token::MediaBulletToken<'a>>,
    },
    /// Separator (comma, tag marker, vocative marker), with its text so the
    /// converter can place it.
    Separator {
        kind: SeparatorKindParsed,
        text: &'a str,
    },
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
pub struct SinTierParsed {
    pub items: Vec<SinItemParsed>,
}

/// A single %sin item: either a token or a 〔group〕.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SinItemParsed {
    Token(talkbank_model::model::SinToken),
    Group(Vec<talkbank_model::model::SinToken>),
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
    Bullet(crate::token::MediaBulletToken<'a>),
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
            body: RetraceBody::Enclosed(
                Enclosure {
                    open: "<",
                    close: ">",
                },
                content,
            ),
            kind: RetraceKindParsed::Complete,
            marker: "[//]",
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
    /// (the missing-annotation check) is the one that silently skipped
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
                    enclosure: Enclosure {
                        open: "<",
                        close: ">",
                    },
                    contents: vec![marker()],
                    annotations: vec![],
                }),
            ),
            (
                "Quotation",
                ContentItem::Quotation(Quotation {
                    enclosure: Enclosure {
                        open: "\u{201C}",
                        close: "\u{201D}",
                    },
                    contents: vec![marker()],
                    annotations: vec![],
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
        assert!(
            ContentItem::Event(crate::token::Extracted::from_lexed("laughs", "&=laughs"))
                .children()
                .is_empty()
        );
    }

    /// SURVIVES: behaviour. Descent through `children()` reaches content nested
    /// inside a pho group, which is the precise case
    /// the missing-annotation check used to miss.
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
