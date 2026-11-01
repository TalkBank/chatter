//! Which header a line is, as one typed graph: the single owner of every
//! header's label, and of the move from "this kind, with this content" to a
//! model [`Header`].
//!
//! The graph, top to bottom:
//!
//! ```text
//! HeaderKind ── Plain(PlainKind) ── Text(TextKind)        built from text: TextKind::header
//!     │                  ├───────── Gem(GemKind)          bare marker, or a label
//!     │                  └───────── Structured(StructuredKind)   own body grammar
//!     ├────── Qualified(QualifiedKind)                    names a participant
//!     ├────── Fixed(FixedKind)    Utf8 | Begin | End | Languages | Participants
//!     │                           | Id | NewEpisode | Blank
//!     └────── Unknown             recovery: no label of its own to write
//! ```
//!
//! A parser maps its node or token to a kind and nothing else: the label a
//! kind is written with ([`HeaderKind::label`]), the header its content builds
//! ([`TextKind::header`], [`QualifiedKind::header`]) and its empty form
//! ([`PlainKind::empty`], [`QualifiedKind::empty`]) all live here, so the two
//! parsers, the writer and the validator cannot spell or build a header
//! differently. Pairing one header's empty form with another header's
//! builder is not expressible: both come from the same kind value.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use super::empty::{EmptyHeader, HeaderText, WrittenSeparator};
use super::header::Header;
use crate::model::{
    ActivitiesDescription, BackgroundDescription, BirthplaceDescription, ChatDate, ColorWordList,
    FontSpec, GemLabel, LanguageCode, LocationDescription, Number, PidValue, RecordingQuality,
    RoomLayoutDescription, SituationDescription, SpeakerCode, TDescription,
    TapeLocationDescription, TimeDurationValue, TimeStartValue, TranscriberName, Transcription,
    VideoSpec, WarningText, WindowGeometry, WriteChat,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// What a header line writes between its `@` and its separator, minus the
/// participant: the identity of every header the model carries.
///
/// One variant per header family and no catch-all, so a new header must say
/// here which family it joins. Returned by [`Header::kind`]; an empty header
/// has the kind of the header it would have been.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeaderKind {
    /// A header with a fixed shape: a marker or a declaration.
    Fixed(FixedKind),
    /// A content header with no participant.
    Plain(PlainKind),
    /// A content header naming a participant.
    Qualified(QualifiedKind),
    /// A header the parser could not recognize or lower. It has no label of
    /// its own to write: its recovery keeps the text as written.
    Unknown,
}

impl HeaderKind {
    /// The canonical label without the leading `@` (`"Unknown"` for a
    /// header the parser could not recognize, which names it in
    /// diagnostics and metadata views but is never written as a prefix).
    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed(kind) => kind.label(),
            Self::Plain(kind) => kind.label(),
            Self::Qualified(kind) => kind.label(),
            Self::Unknown => "Unknown",
        }
    }
}

/// A header whose shape is fixed by CHAT: the markers, which write their
/// prefix alone, and the three declarations, whose malformed forms keep
/// their own admission states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FixedKind {
    /// `@UTF8`.
    Utf8,
    /// `@Begin`.
    Begin,
    /// `@End`.
    End,
    /// `@Languages`, admitted or not.
    Languages,
    /// `@Participants`, admitted or not.
    Participants,
    /// `@ID`, admitted or not.
    Id,
    /// `@New Episode`.
    NewEpisode,
    /// `@Blank`.
    Blank,
}

impl FixedKind {
    /// The canonical label without the leading `@`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF8",
            Self::Begin => "Begin",
            Self::End => "End",
            Self::Languages => "Languages",
            Self::Participants => "Participants",
            Self::Id => "ID",
            Self::NewEpisode => "New Episode",
            Self::Blank => "Blank",
        }
    }
}

/// A content header that names no participant.
///
/// Serialized as the bare snake-case name of the inner kind (`"situation"`,
/// `"begin_gem"`, `"options"`): the three families never share a name, so
/// the name alone says which family it is.
//
// That disjointness is what the untagged reading relies on:
// `plain_kind_families_have_disjoint_wire_names` enumerates every family's
// names from their derived schemas (complete by construction) and refuses
// any overlap.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(untagged)]
pub enum PlainKind {
    /// A header whose whole content is one text or scalar value.
    Text(TextKind),
    /// `@Bg`, `@Eg` or `@G`, which also have a legal bare form.
    Gem(GemKind),
    /// A header whose content has its own grammar.
    Structured(StructuredKind),
}

impl PlainKind {
    /// The canonical label without the leading `@`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Text(kind) => kind.label(),
            Self::Gem(kind) => kind.label(),
            Self::Structured(kind) => kind.label(),
        }
    }

    /// This header written with nothing after its separator: the one empty
    /// state, which only a written separator can make.
    pub fn empty(self, separator: WrittenSeparator) -> Header {
        Header::EmptyContent(EmptyHeader::Plain {
            kind: self,
            separator,
        })
    }
}

/// A header whose content is one text or scalar value, built by
/// [`TextKind::header`] and nothing else.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    /// `@Date`.
    Date,
    /// `@PID`.
    Pid,
    /// `@Situation`.
    Situation,
    /// `@Tape Location`.
    TapeLocation,
    /// `@Time Duration`.
    TimeDuration,
    /// `@Time Start`.
    TimeStart,
    /// `@Location`.
    Location,
    /// `@Room Layout`.
    RoomLayout,
    /// `@Transcriber`.
    Transcriber,
    /// `@Warning`.
    Warning,
    /// `@Activities`.
    Activities,
    /// `@Bck`.
    Bck,
    /// `@Videos`.
    Videos,
    /// `@T`.
    T,
    /// `@Font`.
    Font,
    /// `@Window`.
    Window,
    /// `@Color words`.
    ColorWords,
    /// `@Number`.
    Number,
    /// `@Recording Quality`.
    RecordingQuality,
    /// `@Transcription`.
    Transcription,
}

impl TextKind {
    /// The canonical label without the leading `@`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Pid => "PID",
            Self::Situation => "Situation",
            Self::TapeLocation => "Tape Location",
            Self::TimeDuration => "Time Duration",
            Self::TimeStart => "Time Start",
            Self::Location => "Location",
            Self::RoomLayout => "Room Layout",
            Self::Transcriber => "Transcriber",
            Self::Warning => "Warning",
            Self::Activities => "Activities",
            Self::Bck => "Bck",
            Self::Videos => "Videos",
            Self::T => "T",
            Self::Font => "Font",
            Self::Window => "Window",
            Self::ColorWords => "Color words",
            Self::Number => "Number",
            Self::RecordingQuality => "Recording Quality",
            Self::Transcription => "Transcription",
        }
    }

    /// The header this kind makes from its content: the one constructor both
    /// parsers call, so neither builds a payload of its own.
    pub fn header(self, text: HeaderText<'_>) -> Header {
        match self {
            Self::Date => Header::Date {
                date: ChatDate::from_content(text),
            },
            Self::Pid => Header::Pid {
                pid: PidValue::from_content(text),
            },
            Self::Situation => Header::Situation {
                text: SituationDescription::from_content(text),
            },
            Self::TapeLocation => Header::TapeLocation {
                location: TapeLocationDescription::from_content(text),
            },
            Self::TimeDuration => Header::TimeDuration {
                duration: TimeDurationValue::from_content(text),
            },
            Self::TimeStart => Header::TimeStart {
                start: TimeStartValue::from_content(text),
            },
            Self::Location => Header::Location {
                location: LocationDescription::from_content(text),
            },
            Self::RoomLayout => Header::RoomLayout {
                layout: RoomLayoutDescription::from_content(text),
            },
            Self::Transcriber => Header::Transcriber {
                transcriber: TranscriberName::from_content(text),
            },
            Self::Warning => Header::Warning {
                text: WarningText::from_content(text),
            },
            Self::Activities => Header::Activities {
                activities: ActivitiesDescription::from_content(text),
            },
            Self::Bck => Header::Bck {
                bck: BackgroundDescription::from_content(text),
            },
            Self::Videos => Header::Videos {
                videos: VideoSpec::from_content(text),
            },
            Self::T => Header::T {
                text: TDescription::from_content(text),
            },
            Self::Font => Header::Font {
                font: FontSpec::from_content(text),
            },
            Self::Window => Header::Window {
                geometry: WindowGeometry::from_content(text),
            },
            Self::ColorWords => Header::ColorWords {
                colors: ColorWordList::from_content(text),
            },
            Self::Number => Header::Number {
                number: Number::from_text(text),
            },
            Self::RecordingQuality => Header::RecordingQuality {
                quality: RecordingQuality::from_text(text),
            },
            Self::Transcription => Header::Transcription {
                transcription: Transcription::from_text(text),
            },
        }
    }
}

/// A gem header: `@Bg`, `@Eg` or `@G`.
///
/// Each has two legal forms, the bare marker with no colon ([`Self::bare`])
/// and the labelled one ([`Self::labelled`]); the colon form with nothing
/// after the colon is the empty state ([`PlainKind::empty`]).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
pub enum GemKind {
    /// `@Bg`.
    #[serde(rename = "begin_gem")]
    Begin,
    /// `@Eg`.
    #[serde(rename = "end_gem")]
    End,
    /// `@G`.
    #[serde(rename = "lazy_gem")]
    Lazy,
}

impl GemKind {
    /// The canonical label without the leading `@`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Begin => "Bg",
            Self::End => "Eg",
            Self::Lazy => "G",
        }
    }

    /// The bare marker, written with no colon.
    pub fn bare(self) -> Header {
        self.with_label(None)
    }

    /// The labelled gem, its label built from the content by
    /// [`GemLabel::from_label_content`], the one label joiner.
    pub fn labelled(self, text: HeaderText<'_>) -> Header {
        self.with_label(Some(GemLabel::from_label_content(text)))
    }

    fn with_label(self, label: Option<GemLabel>) -> Header {
        match self {
            Self::Begin => Header::BeginGem { label },
            Self::End => Header::EndGem { label },
            Self::Lazy => Header::LazyGem { label },
        }
    }
}

/// A header whose content has its own grammar, so each keeps a dedicated
/// builder; its emptiness is still the one shared decision.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum StructuredKind {
    /// `@Comment`: text with media bullets and pictures.
    Comment,
    /// `@Options`: a comma-separated option list.
    Options,
    /// `@Media`: filename, type and optional status.
    Media,
    /// `@Types`: design, activity and group.
    Types,
    /// `@Page`: a page number.
    Page,
}

impl StructuredKind {
    /// The canonical label without the leading `@`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Comment => "Comment",
            Self::Options => "Options",
            Self::Media => "Media",
            Self::Types => "Types",
            Self::Page => "Page",
        }
    }
}

/// A content header naming a participant: `@Birth of SPK`, `@Birthplace of
/// SPK`, `@L1 of SPK`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum QualifiedKind {
    /// `@Birth of`.
    Birth,
    /// `@Birthplace of`.
    Birthplace,
    /// `@L1 of`.
    L1Of,
}

impl QualifiedKind {
    /// The canonical label without the leading `@` or the participant.
    pub fn label(self) -> &'static str {
        match self {
            Self::Birth => "Birth of",
            Self::Birthplace => "Birthplace of",
            Self::L1Of => "L1 of",
        }
    }

    /// The header's keyword with its `@` and without a participant,
    /// `@Birth of`: how a diagnostic names the header kind. Never a prefix
    /// to write, since a qualified prefix needs its participant
    /// ([`HeaderPrefix::qualified`]).
    pub fn keyword(self) -> String {
        format!("@{}", self.label())
    }

    /// The header this kind makes for `participant` from its content.
    pub fn header(self, participant: SpeakerCode, text: HeaderText<'_>) -> Header {
        match self {
            Self::Birth => Header::Birth {
                participant,
                date: ChatDate::from_content(text),
            },
            Self::Birthplace => Header::Birthplace {
                participant,
                place: BirthplaceDescription::from_content(text),
            },
            Self::L1Of => Header::L1Of {
                participant,
                language: LanguageCode::from_non_empty(text.as_non_empty()),
            },
        }
    }

    /// This header, for `participant`, written with nothing after its
    /// separator.
    pub fn empty(self, participant: SpeakerCode, separator: WrittenSeparator) -> Header {
        Header::EmptyContent(EmptyHeader::Qualified {
            kind: self,
            participant,
            separator,
        })
    }
}

/// What a header line writes before its separator: `@`, the label, and for a
/// participant-qualified header a space and the participant. The one prefix
/// writer, shared by the header writer, the empty form and diagnostics.
///
/// Built only from a kind, so the spelling comes from the label table, and
/// only from a kind whose prefix is well formed: an unqualified one
/// ([`FixedKind`] or [`PlainKind`], through `From`), or a qualified one with
/// its participant ([`Self::qualified`]). Neither `@Birth of` without its
/// participant nor a prefix for an unrecognized header can be built.
///
/// ```compile_fail
/// # use talkbank_model::model::{HeaderKind, HeaderPrefix, QualifiedKind};
/// let malformed = HeaderPrefix::from(HeaderKind::Qualified(QualifiedKind::Birth));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct HeaderPrefix<'a> {
    label: &'static str,
    participant: Option<&'a SpeakerCode>,
}

impl<'a> HeaderPrefix<'a> {
    /// The prefix of a header that names no participant.
    fn unqualified(label: &'static str) -> Self {
        Self {
            label,
            participant: None,
        }
    }

    /// The prefix of a participant-qualified header.
    pub fn qualified(kind: QualifiedKind, participant: &'a SpeakerCode) -> Self {
        Self {
            label: kind.label(),
            participant: Some(participant),
        }
    }

    /// Writes the prefix and then the separator, `:\t`: the start of every
    /// colon-bearing header line.
    pub(crate) fn write_start<W: crate::model::ChatWriter>(
        self,
        w: &mut W,
    ) -> Result<(), W::Error> {
        self.write_chat(w)?;
        w.write_str(WrittenSeparator::CANONICAL)
    }
}

impl From<FixedKind> for HeaderPrefix<'_> {
    fn from(kind: FixedKind) -> Self {
        Self::unqualified(kind.label())
    }
}

impl From<PlainKind> for HeaderPrefix<'_> {
    fn from(kind: PlainKind) -> Self {
        Self::unqualified(kind.label())
    }
}

impl From<TextKind> for HeaderPrefix<'_> {
    fn from(kind: TextKind) -> Self {
        Self::unqualified(kind.label())
    }
}

impl From<StructuredKind> for HeaderPrefix<'_> {
    fn from(kind: StructuredKind) -> Self {
        Self::unqualified(kind.label())
    }
}

impl WriteChat for HeaderPrefix<'_> {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_char('@')?;
        w.write_str(self.label)?;
        if let Some(participant) = self.participant {
            w.write_char(' ')?;
            participant.write_chat(w)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for HeaderPrefix<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HeaderContent;

    /// Blank content reaches no payload by any route: JSON refuses it for
    /// every header that carries text, as a parser makes it the empty state.
    #[test]
    fn json_refuses_blank_payloads() {
        for wire in [
            r#"{"type":"situation","text":" "}"#,
            r#"{"type":"situation","text":""}"#,
            r#"{"type":"date","date":"\t"}"#,
            r#"{"type":"pid","pid":"  "}"#,
            r#"{"type":"time_start","start":" "}"#,
            r#"{"type":"time_duration","duration":" "}"#,
            r#"{"type":"number","number":" "}"#,
            r#"{"type":"recording_quality","quality":""}"#,
            r#"{"type":"transcription","transcription":" "}"#,
            r#"{"type":"options","options":[" "]}"#,
            r#"{"type":"options","options":[]}"#,
            r#"{"type":"birth","participant":"CHI","date":" "}"#,
            r#"{"type":"l1_of","participant":"CHI","language":""}"#,
            r#"{"type":"l1_of","participant":"CHI","language":" "}"#,
            r#"{"type":"lazy_gem","label":" "}"#,
        ] {
            assert!(serde_json::from_str::<Header>(wire).is_err(), "{wire}");
        }
    }

    /// What a header writes after its separator is content the one
    /// decision admits again, and the kind's constructor rebuilds the same
    /// header from it: the write/reparse roundtrip both parsers rely on.
    #[test]
    fn written_content_reclassifies_to_the_same_header() {
        let text_cases = [
            (TextKind::Situation, "at home "),
            (TextKind::Date, "01-JAN-2024"),
            (TextKind::Date, "1-jan-24"),
            (TextKind::TimeDuration, "10:00-11:00"),
            (TextKind::TimeStart, "99:99"),
            (TextKind::Number, "more"),
            (TextKind::RecordingQuality, "9"),
            (TextKind::ColorWords, "red 255 0 0"),
        ];
        for (kind, content) in text_cases {
            let HeaderContent::Text(text) = HeaderContent::classify(content) else {
                panic!("{content:?} is content")
            };
            let header = kind.header(text);
            let written = header.to_chat();
            let after = written
                .split_once(WrittenSeparator::CANONICAL)
                .map(|(_, after)| after)
                .expect("separator written");
            let HeaderContent::Text(again) = HeaderContent::classify(after) else {
                panic!("{written:?} reparses as empty")
            };
            assert_eq!(kind.header(again), header, "{written:?}");
            let wire = serde_json::to_string(&header).expect("serialize");
            assert_eq!(
                serde_json::from_str::<Header>(&wire).expect("admit"),
                header
            );
        }
        let HeaderContent::Text(text) = HeaderContent::classify("story\n\tcontinued") else {
            panic!("a label is content")
        };
        assert_eq!(
            GemKind::Lazy.labelled(text).to_chat(),
            "@G:\tstory continued"
        );
    }

    /// The gem label joiner keeps text as written and turns each
    /// continuation into one space, adding none before the first text.
    #[test]
    fn gem_label_joins_continuations() {
        for (content, label) in [
            ("one", "one"),
            ("one\n\ttwo", "one two"),
            ("one\r\n\t  two", "one two"),
            ("one\n\t\n\ttwo", "one  two"),
            ("\n\tlate", "late"),
            ("  spaced\n\tnext", "  spaced next"),
        ] {
            let HeaderContent::Text(text) = HeaderContent::classify(content) else {
                panic!("{content:?} is content")
            };
            assert_eq!(
                GemLabel::from_label_content(text).as_str(),
                label,
                "{content:?}"
            );
        }
    }

    /// Every wire name of the three plain families, read from their derived
    /// schemas so a new variant is enumerated without being listed here.
    fn wire_names<T: JsonSchema>() -> Vec<String> {
        fn collect(value: &serde_json::Value, into: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, value) in map {
                        match (key.as_str(), value) {
                            ("const", serde_json::Value::String(name)) => into.push(name.clone()),
                            ("enum", serde_json::Value::Array(names)) => into.extend(
                                names
                                    .iter()
                                    .filter_map(|name| name.as_str().map(str::to_owned)),
                            ),
                            _ => collect(value, into),
                        }
                    }
                }
                serde_json::Value::Array(items) => {
                    items.iter().for_each(|item| collect(item, into));
                }
                serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_) => {}
            }
        }
        let schema = serde_json::to_value(schemars::schema_for!(T)).expect("schema");
        let mut names = Vec::new();
        collect(&schema, &mut names);
        names
    }

    /// The untagged `PlainKind` reading is unambiguous: the families' wire
    /// names are disjoint, and every name reads back into the family that
    /// wrote it.
    #[test]
    fn plain_kind_families_have_disjoint_wire_names() {
        let families = [
            ("text", wire_names::<TextKind>()),
            ("gem", wire_names::<GemKind>()),
            ("structured", wire_names::<StructuredKind>()),
        ];
        let mut seen = std::collections::BTreeMap::new();
        for (family, names) in &families {
            assert!(!names.is_empty(), "{family} has no wire names");
            for name in names {
                if let Some(other) = seen.insert(name.clone(), *family) {
                    panic!("{name:?} names both a {other} and a {family} kind");
                }
                let wire = serde_json::Value::String(name.clone());
                let kind: PlainKind = serde_json::from_value(wire.clone()).expect("a plain kind");
                let read_family = match kind {
                    PlainKind::Text(_) => "text",
                    PlainKind::Gem(_) => "gem",
                    PlainKind::Structured(_) => "structured",
                };
                assert_eq!(read_family, *family, "{name:?}");
                assert_eq!(serde_json::to_value(kind).expect("serialize"), wire);
            }
        }
    }

    /// Every header's label comes from its kind, and the kind of a header
    /// written empty is the kind of the header with content.
    #[test]
    fn header_name_is_the_kind_label() {
        let separator = WrittenSeparator::admit(":\t").expect("written separator");
        assert_eq!(Header::Utf8.name(), "UTF8");
        assert_eq!(Header::Utf8.kind(), HeaderKind::Fixed(FixedKind::Utf8));
        assert_eq!(QualifiedKind::Birthplace.keyword(), "@Birthplace of");
        assert_eq!(
            PlainKind::Text(TextKind::ColorWords)
                .empty(separator)
                .name(),
            "Color words"
        );
        assert_eq!(
            QualifiedKind::Birth
                .empty(SpeakerCode::new("CHI"), separator)
                .kind(),
            HeaderKind::Qualified(QualifiedKind::Birth)
        );
        assert_eq!(GemKind::Begin.bare().to_chat(), "@Bg");
    }
}
