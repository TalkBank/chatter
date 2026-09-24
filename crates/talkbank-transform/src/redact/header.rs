//! Header-level sanitization.

use talkbank_model::{BulletContent, Header, IDHeader};

use super::REDACTED_TEXT;

/// Sanitizes a single header in place.
pub(crate) fn sanitize_header(header: &mut Header) {
    match header {
        Header::Participants { entries } => {
            for entry in entries.as_mut_slice().iter_mut() {
                entry.name = None;
            }
        }
        Header::ID(id) => {
            anonymize_id(id);
        }
        Header::Comment { content } => {
            *content = BulletContent::from_text(REDACTED_TEXT);
        }
        // Preserve each header's typed identity, replacing only its payload.
        // In particular, Birthplace's speaker reference is not free text.
        Header::Situation { text } => *text = REDACTED_TEXT.into(),
        Header::TapeLocation { location } => *location = REDACTED_TEXT.into(),
        Header::Location { location } => *location = REDACTED_TEXT.into(),
        Header::RoomLayout { layout } => *layout = REDACTED_TEXT.into(),
        Header::Birthplace { place, .. } => *place = REDACTED_TEXT.into(),
        Header::Transcriber { transcriber } => *transcriber = REDACTED_TEXT.into(),
        Header::Warning { text } => *text = REDACTED_TEXT.into(),
        Header::Activities { activities } => *activities = REDACTED_TEXT.into(),
        Header::Bck { bck } => *bck = REDACTED_TEXT.into(),
        // Explicit preservation policy, not a default for future variants.
        // Several of these can identify a person; sanitization is not a
        // complete de-identification guarantee. Unknown recovery is retained.
        Header::Utf8
        | Header::Begin
        | Header::End
        | Header::Languages { .. }
        | Header::Date { .. }
        | Header::Pid { .. }
        | Header::Media(_)
        | Header::Types(_)
        | Header::BeginGem { .. }
        | Header::EndGem { .. }
        | Header::LazyGem { .. }
        | Header::Font { .. }
        | Header::Window { .. }
        | Header::ColorWords { .. }
        | Header::Number { .. }
        | Header::RecordingQuality { .. }
        | Header::Transcription { .. }
        | Header::NewEpisode
        | Header::TimeDuration { .. }
        | Header::TimeStart { .. }
        | Header::Birth { .. }
        | Header::L1Of { .. }
        | Header::Blank
        | Header::Unknown { .. }
        | Header::Options { .. }
        | Header::Page { .. }
        | Header::Videos { .. }
        | Header::T { .. } => {}
    }
}

fn anonymize_id(id: &mut IDHeader) {
    id.custom_field = None;
    id.education = None;
}
