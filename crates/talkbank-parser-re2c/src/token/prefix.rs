//! A lexer-owned prefix payload and its separator provenance.

use serde::Serialize;
use talkbank_model::{
    Span,
    model::{TierSeparator, WrittenSeparator},
};

/// Prefix payload retained together with the separator consumed by its lexer
/// rule. Some header prefixes have no separator; their provenance is clean.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrefixToken<'a> {
    text: &'a str,
    #[serde(skip)]
    separator: TierSeparator,
}

impl<'a> PrefixToken<'a> {
    /// Called only at lexical admission. `matched` includes any trailing
    /// separator spaces; `payload` may instead be a tag-extracted speaker.
    pub(crate) fn from_lexed(payload: &'a str, matched: &'a str, start: usize) -> Self {
        Self::lex(payload, matched, start).0
    }

    /// The prefix, and its colon and tab when this rule matched them:
    /// decided once, here, from the matched text, so no consumer re-tests
    /// the spelling.
    fn lex(payload: &'a str, matched: &'a str, start: usize) -> (Self, Option<WrittenSeparator>) {
        let prefix = matched.trim_end_matches(' ');
        // The separator as written: the prefix's last two bytes (the colon
        // and tab, when there are any) and the trailing spaces after them.
        let written = prefix
            .len()
            .checked_sub(WrittenSeparator::CANONICAL.len())
            .and_then(|at| matched.get(at..))
            .and_then(WrittenSeparator::admit);
        let separator = match written {
            Some(_) if prefix.len() < matched.len() => TierSeparator::with_trailing_space(
                Span::new(start + prefix.len(), start + matched.len()),
            ),
            Some(_) | None => TierSeparator::CLEAN,
        };
        let text = match written {
            Some(_) => payload.trim_end_matches(' '),
            None => payload,
        };
        (Self { text, separator }, written)
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn separator(&self) -> TierSeparator {
        self.separator
    }
}

/// A prefix whose lexer rule matched its colon and tab, carrying them as a
/// required [`WrittenSeparator`]: the speaker-qualified headers'
/// (`@Birth of SPK:\t`) tokens, whose rules cannot match without one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeparatedPrefixToken<'a> {
    #[serde(flatten)]
    prefix: PrefixToken<'a>,
    #[serde(skip)]
    written: WrittenSeparator,
}

impl<'a> SeparatedPrefixToken<'a> {
    /// Called only at lexical admission, by a rule that matches the colon
    /// and tab; `None` when the matched text has none, which such a rule
    /// cannot produce (the lexer emits an error line for it).
    pub(crate) fn from_lexed(payload: &'a str, matched: &'a str, start: usize) -> Option<Self> {
        let (prefix, written) = PrefixToken::lex(payload, matched, start);
        written.map(|written| Self { prefix, written })
    }

    /// Original prefix payload, excluding trailing separator spaces.
    pub fn text(&self) -> &'a str {
        self.prefix.text()
    }

    /// Separator admitted by the matching lexer rule.
    pub fn separator(&self) -> TierSeparator {
        self.prefix.separator()
    }

    /// The written colon and tab.
    pub fn written_separator(&self) -> WrittenSeparator {
        self.written
    }
}

/// Body grammar selected by the exact dependent-prefix lexer rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DependentBodyKind {
    /// Morphology with rejected-tier provenance.
    Mor,
    /// Actual phonology.
    Pho,
    /// Model phonology.
    Mod,
    /// Grammatical relations.
    Gra,
    /// Sign and gesture structure.
    Sin,
    /// Timed words.
    Wor,
    /// Text or a specialized tier lowered from text tokens.
    Text,
}

/// Complete dependent prefix carrying the lexer-selected body grammar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DependentPrefixToken<'a> {
    #[serde(flatten)]
    prefix: PrefixToken<'a>,
    kind: DependentBodyKind,
}

impl<'a> DependentPrefixToken<'a> {
    pub(crate) fn from_lexed(prefix: PrefixToken<'a>, kind: DependentBodyKind) -> Self {
        Self { prefix, kind }
    }

    /// Original prefix payload, excluding trailing separator spaces.
    pub fn text(&self) -> &'a str {
        self.prefix.text()
    }

    /// Separator evidence admitted by the lexer.
    pub fn separator(&self) -> TierSeparator {
        self.prefix.separator()
    }

    /// Exact body grammar admitted by the lexer.
    pub fn kind(&self) -> DependentBodyKind {
        self.kind
    }
}

/// The identity of a header whose lexer rule matched its name, colon and
/// tab, selected by an exact lexer rule, never by a name substring. The gem
/// markers, whose colon is optional, and the catch-all rule are not here:
/// they are other [`HeaderPrefixFamily`] variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexedHeader {
    /// `@ID`.
    Id,
    /// `@Types`.
    Types,
    /// `@Comment`.
    Comment,
    /// `@Languages`.
    Languages,
    /// `@Participants`.
    Participants,
    /// `@Date`.
    Date,
    /// `@Media`.
    Media,
    /// `@Options`.
    Options,
    /// `@Recording Quality`.
    RecordingQuality,
    /// `@Transcription`.
    Transcription,
    /// `@Number`.
    Number,
    /// `@Location`.
    Location,
    /// `@Situation`.
    Situation,
    /// `@Activities`.
    Activities,
    /// `@Room Layout`.
    RoomLayout,
    /// `@Tape Location`.
    TapeLocation,
    /// `@Time Duration`.
    TimeDuration,
    /// `@Time Start`.
    TimeStart,
    /// `@Transcriber`.
    Transcriber,
    /// `@Warning`.
    Warning,
    /// `@Page`.
    Page,
    /// `@Videos`.
    Videos,
    /// `@Thumbnail`.
    Thumbnail,
    /// `@T`.
    T,
    /// `@Bck`.
    Bck,
    /// `@PID`.
    Pid,
    /// `@Font`.
    Font,
    /// `@Window`.
    Window,
    /// `@Color words`.
    ColorWords,
}

/// Which lexer rule family a header prefix came from, with the separator
/// evidence that family can carry: required where the rule matched the
/// colon and tab, optional only for the gem markers, absent for the
/// catch-all (whose separator, if any, is the next token).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPrefixFamily {
    /// A known header whose rule matched its colon and tab.
    Separated(LexedHeader, WrittenSeparator),
    /// `@Bg`, `@Eg` or `@G`, written with its colon and tab or bare.
    Gem(talkbank_model::model::GemKind, Option<WrittenSeparator>),
    /// The catch-all rule: a header name the lexer does not know.
    Unknown,
}

/// Header prefix retaining both source/separator evidence and lexical identity.
///
/// Callers can inspect the family but cannot replace its lexer-owned identity.
/// ```compile_fail
/// use talkbank_parser_re2c::token::{HeaderPrefixFamily, HeaderPrefixToken};
/// fn relabel(prefix: &mut HeaderPrefixToken<'_>) {
///     prefix.family = HeaderPrefixFamily::Unknown;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeaderPrefixToken<'a> {
    #[serde(flatten)]
    prefix: PrefixToken<'a>,
    #[serde(skip)]
    family: HeaderPrefixFamily,
}

impl<'a> HeaderPrefixToken<'a> {
    /// Called only at lexical admission, by a rule that matches the header's
    /// colon and tab; `None` when the matched text has none, which such a
    /// rule cannot produce (the lexer emits an error line for it).
    pub(crate) fn separated(matched: &'a str, start: usize, header: LexedHeader) -> Option<Self> {
        let (prefix, written) = PrefixToken::lex(matched, matched, start);
        written.map(|written| Self {
            prefix,
            family: HeaderPrefixFamily::Separated(header, written),
        })
    }

    /// Called only at lexical admission, by a gem marker's rule, with or
    /// without its colon and tab.
    pub(crate) fn gem(matched: &'a str, start: usize, gem: talkbank_model::model::GemKind) -> Self {
        let (prefix, written) = PrefixToken::lex(matched, matched, start);
        Self {
            prefix,
            family: HeaderPrefixFamily::Gem(gem, written),
        }
    }

    /// Called only at lexical admission, by the catch-all header rule.
    pub(crate) fn unknown(matched: &'a str, start: usize) -> Self {
        Self {
            prefix: PrefixToken::from_lexed(matched, matched, start),
            family: HeaderPrefixFamily::Unknown,
        }
    }

    /// Original prefix bytes, excluding trailing separator spaces.
    pub fn text(&self) -> &'a str {
        self.prefix.text()
    }

    /// Separator admitted by the matching lexer rule.
    pub fn separator(&self) -> TierSeparator {
        self.prefix.separator()
    }

    /// The lexer rule family and its separator evidence.
    pub fn family(&self) -> HeaderPrefixFamily {
        self.family
    }
}

/// What a lexed header identity means to the model, decided once from the
/// lexer's rule: the one map from the lexer's kinds to the model's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeaderRoute {
    /// A header whose whole content is one text or scalar value.
    Text(talkbank_model::model::TextKind),
    /// A header whose content has its own grammar.
    Structured(talkbank_model::model::StructuredKind),
    /// A declaration with its own admission state.
    Declaration(Declaration),
    /// A header name the model does not carry.
    Unsupported,
}

/// The three declarations whose malformed forms keep their own admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Declaration {
    /// `@ID`.
    Id,
    /// `@Languages`.
    Languages,
    /// `@Participants`.
    Participants,
}

impl LexedHeader {
    /// The model meaning of this lexed identity.
    pub(crate) fn route(self) -> HeaderRoute {
        use talkbank_model::model::{StructuredKind, TextKind};
        match self {
            Self::Id => HeaderRoute::Declaration(Declaration::Id),
            Self::Languages => HeaderRoute::Declaration(Declaration::Languages),
            Self::Participants => HeaderRoute::Declaration(Declaration::Participants),
            Self::Types => HeaderRoute::Structured(StructuredKind::Types),
            Self::Comment => HeaderRoute::Structured(StructuredKind::Comment),
            Self::Media => HeaderRoute::Structured(StructuredKind::Media),
            Self::Options => HeaderRoute::Structured(StructuredKind::Options),
            Self::Page => HeaderRoute::Structured(StructuredKind::Page),
            Self::Date => HeaderRoute::Text(TextKind::Date),
            Self::RecordingQuality => HeaderRoute::Text(TextKind::RecordingQuality),
            Self::Transcription => HeaderRoute::Text(TextKind::Transcription),
            Self::Number => HeaderRoute::Text(TextKind::Number),
            Self::Location => HeaderRoute::Text(TextKind::Location),
            Self::Situation => HeaderRoute::Text(TextKind::Situation),
            Self::Activities => HeaderRoute::Text(TextKind::Activities),
            Self::RoomLayout => HeaderRoute::Text(TextKind::RoomLayout),
            Self::TapeLocation => HeaderRoute::Text(TextKind::TapeLocation),
            Self::TimeDuration => HeaderRoute::Text(TextKind::TimeDuration),
            Self::TimeStart => HeaderRoute::Text(TextKind::TimeStart),
            Self::Transcriber => HeaderRoute::Text(TextKind::Transcriber),
            Self::Warning => HeaderRoute::Text(TextKind::Warning),
            Self::Videos => HeaderRoute::Text(TextKind::Videos),
            Self::T => HeaderRoute::Text(TextKind::T),
            Self::Bck => HeaderRoute::Text(TextKind::Bck),
            Self::Pid => HeaderRoute::Text(TextKind::Pid),
            Self::Font => HeaderRoute::Text(TextKind::Font),
            Self::Window => HeaderRoute::Text(TextKind::Window),
            Self::ColorWords => HeaderRoute::Text(TextKind::ColorWords),
            // `@Thumbnail` is recognized but has no supported model (E525).
            Self::Thumbnail => HeaderRoute::Unsupported,
        }
    }
}
