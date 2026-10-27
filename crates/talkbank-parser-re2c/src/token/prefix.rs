//! A lexer-owned prefix payload and its separator provenance.

use serde::Serialize;
use talkbank_model::{Span, model::TierSeparator};

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
        let prefix = matched.trim_end_matches(' ');
        let separator = if prefix.ends_with(":\t") && prefix.len() < matched.len() {
            TierSeparator::with_trailing_space(Span::new(
                start + prefix.len(),
                start + matched.len(),
            ))
        } else {
            TierSeparator::CLEAN
        };
        let text = if prefix.ends_with(":\t") {
            payload.trim_end_matches(' ')
        } else {
            payload
        };
        Self { text, separator }
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn separator(&self) -> TierSeparator {
        self.separator
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

/// Header meaning selected by an exact lexer rule, never by a name substring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKind {
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
    /// `@Bg`.
    BeginGem,
    /// `@Eg`.
    EndGem,
    /// `@G`.
    LazyGem,
    /// The catch-all header rule preserves an unsupported name.
    Unknown,
}

/// Header prefix retaining both source/separator evidence and lexical identity.
///
/// Callers can inspect the kind but cannot replace its lexer-owned identity.
/// ```compile_fail
/// use talkbank_parser_re2c::token::{HeaderKind, HeaderPrefixToken};
/// fn relabel(prefix: &mut HeaderPrefixToken<'_>) {
///     prefix.kind = HeaderKind::Options;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeaderPrefixToken<'a> {
    #[serde(flatten)]
    prefix: PrefixToken<'a>,
    #[serde(skip)]
    kind: HeaderKind,
}

impl<'a> HeaderPrefixToken<'a> {
    pub(crate) fn from_lexed(prefix: PrefixToken<'a>, kind: HeaderKind) -> Self {
        Self { prefix, kind }
    }

    /// Original prefix bytes, excluding trailing separator spaces.
    pub fn text(&self) -> &'a str {
        self.prefix.text()
    }

    /// Separator admitted by the matching lexer rule.
    pub fn separator(&self) -> TierSeparator {
        self.prefix.separator()
    }

    /// Exact header identity admitted by the lexer.
    pub fn kind(&self) -> HeaderKind {
        self.kind
    }
}
