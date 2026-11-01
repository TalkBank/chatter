//! Header-level sanitization.

use talkbank_model::{Header, IDHeader, model};

use super::REDACTED_CONTENT;

/// Sanitizes a single header in place.
pub(crate) fn sanitize_header(header: &mut Header) -> Result<(), super::RedactError> {
    match header {
        Header::UnparsedID(_) => return Err(super::RedactError::UnparsedID),
        Header::UnparsedParticipants(_) => return Err(super::RedactError::UnparsedParticipants),
        Header::Participants { entries } => {
            for entry in entries.as_mut_slice().iter_mut() {
                entry.details.clear_name();
            }
        }
        Header::ID(id) => {
            anonymize_id(id);
        }
        Header::Comment { content } => {
            *content = model::CommentContent::from_content(REDACTED_CONTENT);
        }
        // Preserve each header's typed identity, replacing only its payload.
        // In particular, Birthplace's speaker reference is not free text.
        Header::Situation { text } => {
            *text = model::SituationDescription::from_content(REDACTED_CONTENT)
        }
        Header::TapeLocation { location } => {
            *location = model::TapeLocationDescription::from_content(REDACTED_CONTENT)
        }
        Header::Location { location } => {
            *location = model::LocationDescription::from_content(REDACTED_CONTENT)
        }
        Header::RoomLayout { layout } => {
            *layout = model::RoomLayoutDescription::from_content(REDACTED_CONTENT)
        }
        Header::Birthplace { place, .. } => {
            *place = model::BirthplaceDescription::from_content(REDACTED_CONTENT)
        }
        Header::Transcriber { transcriber } => {
            *transcriber = model::TranscriberName::from_content(REDACTED_CONTENT)
        }
        Header::Warning { text } => *text = model::WarningText::from_content(REDACTED_CONTENT),
        Header::Activities { activities } => {
            *activities = model::ActivitiesDescription::from_content(REDACTED_CONTENT)
        }
        Header::Bck { bck } => *bck = model::BackgroundDescription::from_content(REDACTED_CONTENT),
        // Explicit preservation policy, not a default for future variants.
        // Several of these can identify a person; sanitization is not a
        // complete de-identification guarantee. Unknown recovery is retained.
        // Empty content carries nothing to redact (the participant it may
        // name is a speaker reference, not free text).
        Header::EmptyContent(_)
        | Header::Utf8
        | Header::Begin
        | Header::End
        | Header::Languages { .. }
        | Header::UnparsedLanguages(_)
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
    Ok(())
}

fn anonymize_id(id: &mut IDHeader) {
    id.custom_field = None;
    id.education = None;
}
