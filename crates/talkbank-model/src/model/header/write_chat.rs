//! WriteChat implementation for Header.
//!
//! Serializes Header enum variants to CHAT format strings.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use super::{
    WriteChat,
    header_enum::{ChatOptionFlag, Header},
};

impl WriteChat for Header {
    /// Serializes each header variant to its canonical `@Header:\t...` CHAT line.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        match self {
            // Required structure headers
            Header::Utf8 => w.write_str("@UTF8"),
            Header::Begin => w.write_str("@Begin"),
            Header::End => w.write_str("@End"),

            // Participant headers
            Header::UnparsedLanguages(raw) => raw.write_chat(w),
            Header::UnparsedParticipants(raw) => raw.write_chat(w),
            Header::UnparsedID(raw) => raw.write_chat(w),
            Header::Languages { codes } => {
                let joined = codes
                    .iter()
                    .map(|code| code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                {
                    w.write_str("@Languages:\t")?;
                    crate::model::WriteChat::write_chat(&joined, w)
                }
            }

            Header::Participants { entries } => {
                w.write_str("@Participants:\t")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    entry.write_chat(w)?;
                }
                Ok(())
            }

            Header::ID(id_header) => id_header.write_chat(w),

            // Metadata headers
            Header::Date { date } => {
                w.write_str("@Date:\t")?;
                crate::model::WriteChat::write_chat(&date, w)
            }

            Header::Comment { content } => {
                w.write_str("@Comment:\t")?;
                content.write_chat(w)
            }

            Header::Pid { pid } => {
                w.write_str("@PID:\t")?;
                crate::model::WriteChat::write_chat(&pid, w)
            }

            Header::Media(media_header) => media_header.write_chat(w),

            Header::Situation { text } => {
                w.write_str("@Situation:\t")?;
                crate::model::WriteChat::write_chat(&text, w)
            }

            Header::Types(types_header) => types_header.write_chat(w),

            // Gem headers
            Header::BeginGem { label } => {
                w.write_str("@Bg")?;
                if let Some(lbl) = label {
                    {
                        w.write_str(":\t")?;
                        crate::model::WriteChat::write_chat(&lbl, w)
                    }?;
                }
                Ok(())
            }

            Header::EndGem { label } => {
                w.write_str("@Eg")?;
                if let Some(lbl) = label {
                    {
                        w.write_str(":\t")?;
                        crate::model::WriteChat::write_chat(&lbl, w)
                    }?;
                }
                Ok(())
            }

            Header::LazyGem { label } => {
                w.write_str("@G")?;
                if let Some(lbl) = label {
                    {
                        w.write_str(":\t")?;
                        crate::model::WriteChat::write_chat(&lbl, w)
                    }?;
                }
                Ok(())
            }

            // CLAN display headers
            Header::Font { font } => {
                w.write_str("@Font:\t")?;
                crate::model::WriteChat::write_chat(&font, w)
            }
            Header::Window { geometry } => {
                w.write_str("@Window:\t")?;
                crate::model::WriteChat::write_chat(&geometry, w)
            }
            Header::ColorWords { colors } => {
                w.write_str("@Color words:\t")?;
                crate::model::WriteChat::write_chat(&colors, w)
            }

            // Recording/session headers
            Header::Number { number } => {
                w.write_str("@Number:\t")?;
                crate::model::WriteChat::write_chat(&number.as_str(), w)
            }

            Header::RecordingQuality { quality } => {
                w.write_str("@Recording Quality:\t")?;
                crate::model::WriteChat::write_chat(&quality.as_str(), w)
            }

            Header::Transcription { transcription } => {
                w.write_str("@Transcription:\t")?;
                crate::model::WriteChat::write_chat(&transcription.as_str(), w)
            }

            Header::NewEpisode => w.write_str("@New Episode"),
            Header::TapeLocation { location } => {
                w.write_str("@Tape Location:\t")?;
                crate::model::WriteChat::write_chat(&location, w)
            }
            Header::TimeDuration { duration } => {
                w.write_str("@Time Duration:\t")?;
                crate::model::WriteChat::write_chat(&duration, w)
            }
            Header::TimeStart { start } => {
                w.write_str("@Time Start:\t")?;
                crate::model::WriteChat::write_chat(&start, w)
            }
            Header::Location { location } => {
                w.write_str("@Location:\t")?;
                crate::model::WriteChat::write_chat(&location, w)
            }
            Header::RoomLayout { layout } => {
                w.write_str("@Room Layout:\t")?;
                crate::model::WriteChat::write_chat(&layout, w)
            }

            // Participant-specific headers
            Header::Birth { participant, date } => {
                w.write_str("@Birth of ")?;
                crate::model::WriteChat::write_chat(&participant, w)?;
                w.write_str(":\t")?;
                crate::model::WriteChat::write_chat(&date, w)
            }
            Header::Birthplace { participant, place } => {
                w.write_str("@Birthplace of ")?;
                crate::model::WriteChat::write_chat(&participant, w)?;
                w.write_str(":\t")?;
                crate::model::WriteChat::write_chat(&place, w)
            }
            Header::L1Of {
                participant,
                language,
            } => {
                w.write_str("@L1 of ")?;
                crate::model::WriteChat::write_chat(&participant, w)?;
                w.write_str(":\t")?;
                crate::model::WriteChat::write_chat(&language, w)
            }

            // Other headers
            Header::Blank => w.write_str("@Blank"),
            Header::Transcriber { transcriber } => {
                w.write_str("@Transcriber:\t")?;
                crate::model::WriteChat::write_chat(&transcriber, w)
            }
            Header::Warning { text } => {
                w.write_str("@Warning:\t")?;
                crate::model::WriteChat::write_chat(&text, w)
            }
            Header::Unknown { text, .. } => {
                // Unknown headers store the raw text as-is (including @ prefix)
                crate::model::WriteChat::write_chat(&text, w)
            }
            Header::Activities { activities } => {
                w.write_str("@Activities:\t")?;
                crate::model::WriteChat::write_chat(&activities, w)
            }
            Header::Bck { bck } => {
                w.write_str("@Bck:\t")?;
                crate::model::WriteChat::write_chat(&bck, w)
            }
            Header::Options { options } => {
                let joined = options
                    .iter()
                    .map(ChatOptionFlag::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                {
                    w.write_str("@Options:\t")?;
                    crate::model::WriteChat::write_chat(&joined, w)
                }
            }
            Header::Page { page } => {
                w.write_str("@Page:\t")?;
                crate::model::WriteChat::write_chat(&page, w)
            }
            Header::Videos { videos } => {
                w.write_str("@Videos:\t")?;
                crate::model::WriteChat::write_chat(&videos, w)
            }
            Header::T { text } => {
                w.write_str("@T:\t")?;
                crate::model::WriteChat::write_chat(&text, w)
            }
        }
    }
}
