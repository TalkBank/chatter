//! WriteChat implementation for Header.
//!
//! Serializes Header enum variants to CHAT format strings. Every header's
//! prefix comes from its kind ([`HeaderPrefix`], over the label table in
//! `header_enum/kind.rs`), so the writer spells no header name of its own.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use super::{
    WriteChat,
    header_enum::{
        FixedKind, GemKind, Header, HeaderPrefix, PlainKind, QualifiedKind, StructuredKind,
        TextKind,
    },
};
use crate::model::{ChatWriter, GemLabel, SpeakerCode};

/// A colon-bearing header line: its prefix, the separator, then `value`.
fn line<W: ChatWriter, V: WriteChat + ?Sized>(
    w: &mut W,
    prefix: HeaderPrefix<'_>,
    value: &V,
) -> Result<(), W::Error> {
    prefix.write_start(w)?;
    value.write_chat(w)
}

/// A text-kind header line.
fn text_line<W: ChatWriter, V: WriteChat + ?Sized>(
    w: &mut W,
    kind: TextKind,
    value: &V,
) -> Result<(), W::Error> {
    line(w, HeaderPrefix::from(kind), value)
}

/// A participant-qualified header line.
fn qualified_line<W: ChatWriter, V: WriteChat + ?Sized>(
    w: &mut W,
    kind: QualifiedKind,
    participant: &SpeakerCode,
    value: &V,
) -> Result<(), W::Error> {
    line(w, HeaderPrefix::qualified(kind, participant), value)
}

/// A gem line: the bare marker, or the labelled form.
fn gem_line<W: ChatWriter>(
    w: &mut W,
    kind: GemKind,
    label: Option<&GemLabel>,
) -> Result<(), W::Error> {
    let prefix = HeaderPrefix::from(PlainKind::Gem(kind));
    match label {
        Some(label) => line(w, prefix, label),
        None => prefix.write_chat(w),
    }
}

/// Writes `items` separated by a comma and a space.
fn comma_list<W: ChatWriter, T: WriteChat>(
    w: &mut W,
    items: impl IntoIterator<Item = T>,
) -> Result<(), W::Error> {
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            w.write_str(", ")?;
        }
        item.write_chat(w)?;
    }
    Ok(())
}

impl WriteChat for Header {
    /// Serializes each header variant to its canonical `@Header:\t...` CHAT line.
    fn write_chat<W: ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        match self {
            // Marker headers: the prefix alone.
            Header::Utf8 => HeaderPrefix::from(FixedKind::Utf8).write_chat(w),
            Header::Begin => HeaderPrefix::from(FixedKind::Begin).write_chat(w),
            Header::End => HeaderPrefix::from(FixedKind::End).write_chat(w),
            Header::NewEpisode => HeaderPrefix::from(FixedKind::NewEpisode).write_chat(w),
            Header::Blank => HeaderPrefix::from(FixedKind::Blank).write_chat(w),

            // Declaration headers.
            Header::UnparsedLanguages(raw) => raw.write_chat(w),
            Header::UnparsedParticipants(raw) => raw.write_chat(w),
            Header::UnparsedID(raw) => raw.write_chat(w),
            Header::Languages { codes } => {
                HeaderPrefix::from(FixedKind::Languages).write_start(w)?;
                comma_list(w, codes.iter())
            }
            Header::Participants { entries } => {
                HeaderPrefix::from(FixedKind::Participants).write_start(w)?;
                comma_list(w, entries.iter())
            }
            Header::ID(id_header) => id_header.write_chat(w),

            Header::EmptyContent(empty) => empty.write_chat(w),

            // Text-kind headers.
            Header::Date { date } => text_line(w, TextKind::Date, date),
            Header::Pid { pid } => text_line(w, TextKind::Pid, pid),
            Header::Situation { text } => text_line(w, TextKind::Situation, text),
            Header::TapeLocation { location } => text_line(w, TextKind::TapeLocation, location),
            Header::TimeDuration { duration } => text_line(w, TextKind::TimeDuration, duration),
            Header::TimeStart { start } => text_line(w, TextKind::TimeStart, start),
            Header::Location { location } => text_line(w, TextKind::Location, location),
            Header::RoomLayout { layout } => text_line(w, TextKind::RoomLayout, layout),
            Header::Transcriber { transcriber } => text_line(w, TextKind::Transcriber, transcriber),
            Header::Warning { text } => text_line(w, TextKind::Warning, text),
            Header::Activities { activities } => text_line(w, TextKind::Activities, activities),
            Header::Bck { bck } => text_line(w, TextKind::Bck, bck),
            Header::Videos { videos } => text_line(w, TextKind::Videos, videos),
            Header::T { text } => text_line(w, TextKind::T, text),
            Header::Font { font } => text_line(w, TextKind::Font, font),
            Header::Window { geometry } => text_line(w, TextKind::Window, geometry),
            Header::ColorWords { colors } => text_line(w, TextKind::ColorWords, colors),
            Header::Number { number } => text_line(w, TextKind::Number, number),
            Header::RecordingQuality { quality } => {
                text_line(w, TextKind::RecordingQuality, quality)
            }
            Header::Transcription { transcription } => {
                text_line(w, TextKind::Transcription, transcription)
            }

            // Gem headers.
            Header::BeginGem { label } => gem_line(w, GemKind::Begin, label.as_ref()),
            Header::EndGem { label } => gem_line(w, GemKind::End, label.as_ref()),
            Header::LazyGem { label } => gem_line(w, GemKind::Lazy, label.as_ref()),

            // Headers with their own body grammar.
            Header::Comment { content } => {
                HeaderPrefix::from(StructuredKind::Comment).write_start(w)?;
                content.write_chat(w)
            }
            Header::Options { options } => {
                HeaderPrefix::from(StructuredKind::Options).write_start(w)?;
                comma_list(w, options.iter())
            }
            Header::Media(media_header) => media_header.write_chat(w),
            Header::Types(types_header) => types_header.write_chat(w),
            Header::Page { page } => line(w, HeaderPrefix::from(StructuredKind::Page), page),

            // Participant-qualified headers.
            Header::Birth { participant, date } => {
                qualified_line(w, QualifiedKind::Birth, participant, date)
            }
            Header::Birthplace { participant, place } => {
                qualified_line(w, QualifiedKind::Birthplace, participant, place)
            }
            Header::L1Of {
                participant,
                language,
            } => qualified_line(w, QualifiedKind::L1Of, participant, language),

            // Unknown headers store the raw text as-is (including the `@`).
            Header::Unknown { text, .. } => text.write_chat(w),
        }
    }
}
