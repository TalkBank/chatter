//! Participant data handlers for the LSP.
//!
//! Provides `talkbank/getParticipants` and `talkbank/formatIdLine` execute-command
//! handlers. These delegate CHAT parsing and serialization to the Rust model so the
//! TypeScript side remains a thin UI layer.

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use talkbank_model::model::{
    AgeValue, ChatFile, CorpusName, CustomIdField, EducationDescription, GroupName, Header,
    IDHeader, LanguageCode, Line, ParticipantRole, SesValue, Sex, SpeakerCode,
};
use tower_lsp::lsp_types::Url;

use super::LspBackendError;
use super::documents;
use super::execute_commands::{DocumentUriRequest, IdLineFieldsRequest, ParticipantCommandRequest};
use super::state::Backend;
use super::utils::LineIndex;

/// Feature-oriented execute-command service for participant commands.
pub(crate) struct ParticipantCommandService;

impl ParticipantCommandService {
    /// Dispatch one participant-family execute-command request.
    pub(crate) fn dispatch(
        &self,
        backend: &Backend,
        request: ParticipantCommandRequest,
    ) -> Result<Value, LspBackendError> {
        match request {
            ParticipantCommandRequest::GetParticipants(request) => {
                handle_get_participants(backend, &request)
            }
            ParticipantCommandRequest::FormatIdLine(request) => handle_format_id_line(&request),
        }
    }
}

/// JSON-serializable participant entry returned by `talkbank/getParticipants`.
#[derive(Serialize)]
struct ParticipantEntry {
    /// 0-based line number of the `@ID` header in the document.
    line: usize,
    /// The ten pipe-delimited `@ID` fields as plain strings.
    fields: ParticipantFields,
}

/// The ten fields of an `@ID` line, all as plain strings for the webview.
#[derive(Serialize)]
struct ParticipantFields {
    /// Language code field.
    language: String,
    /// Corpus name field.
    corpus: String,
    /// Speaker code field.
    speaker: String,
    /// Age field.
    age: String,
    /// Sex field.
    sex: String,
    /// Group field.
    group: String,
    /// SES field.
    ses: String,
    /// Role field.
    role: String,
    /// Education field.
    education: String,
    /// Custom field.
    custom: String,
}

/// Handle `talkbank/getParticipants`, extract participant `@ID` data from a parsed document.
///
/// Returns: JSON array of `ParticipantEntry`.
pub(crate) fn handle_get_participants(
    backend: &Backend,
    request: &DocumentUriRequest,
) -> Result<Value, LspBackendError> {
    let text = documents::get_document_text(backend, &request.uri)
        .ok_or(LspBackendError::DocumentNotFound)?;

    let chat_file = get_chat_file(backend, &request.uri, &text)?;

    let entries = extract_participant_entries(&chat_file, &text);

    // `SerializationFailed` converts from `serde_json::Error` via `#[from]`.
    Ok(serde_json::to_value(&entries)?)
}

/// Handle `talkbank/formatIdLine`, construct a canonical `@ID` line from field values.
///
/// Returns: JSON string containing the formatted `@ID` line.
pub(crate) fn handle_format_id_line(
    request: &IdLineFieldsRequest,
) -> Result<Value, LspBackendError> {
    let id_header = build_id_header(request)?;
    let formatted = id_header.to_string();

    Ok(Value::String(formatted))
}

/// Build an `IDHeader` from user-provided plain string fields.
///
/// # Errors
///
/// Returns [`LspBackendError::ArgumentInvalid`] when the `language` field is
/// empty: `LanguageCode` construction is fallible on empty input, and the
/// language here is a user-typed webview field, so the failure surfaces as a
/// normal command-argument validation error instead of tearing down the
/// server (the previous panicking constructor crashed here on an empty
/// language field).
fn build_id_header(input: &IdLineFieldsRequest) -> Result<IDHeader, LspBackendError> {
    let language =
        LanguageCode::new(&input.language).map_err(|error| LspBackendError::ArgumentInvalid {
            label: "language",
            reason: error.to_string(),
        })?;
    let mut header = IDHeader::new(
        language,
        SpeakerCode::new(&input.speaker),
        ParticipantRole::new(&input.role),
    );

    if !input.corpus.is_empty() {
        header = header.with_corpus(CorpusName::new(&input.corpus));
    }
    if !input.age.is_empty() {
        header = header.with_age(AgeValue::new(&input.age));
    }
    if !input.sex.is_empty() {
        header = header.with_sex(Sex::from_text(&input.sex));
    }
    if !input.group.is_empty() {
        header = header.with_group(GroupName::new(&input.group));
    }
    if !input.ses.is_empty() {
        header = header.with_ses(SesValue::from_text(&input.ses));
    }
    if !input.education.is_empty() {
        header = header.with_education(EducationDescription::new(&input.education));
    }
    if !input.custom.is_empty() {
        header = header.with_custom_field(CustomIdField::new(&input.custom));
    }

    Ok(header)
}

/// Extract participant entries with line numbers from the parsed `ChatFile`.
fn extract_participant_entries(chat_file: &ChatFile, text: &str) -> Vec<ParticipantEntry> {
    let line_index = LineIndex::new(text);

    chat_file
        .lines
        .iter()
        .filter_map(|line| match line {
            Line::Header {
                header, location, ..
            } => match header.as_ref() {
                Header::ID(id) => {
                    // Line 0 stands for a header with no source position, as before.
                    let line_number = location
                        .span()
                        .map_or(0, |span| line_index.byte_to_line(span.start()));
                    Some(ParticipantEntry {
                        line: line_number,
                        fields: id_header_to_fields(id),
                    })
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Convert an `IDHeader` to plain string fields for the webview.
fn id_header_to_fields(id: &IDHeader) -> ParticipantFields {
    ParticipantFields {
        language: id
            .language
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        corpus: id.corpus.as_str().to_string(),
        speaker: id.speaker.as_str().to_string(),
        age: id
            .age
            .as_ref()
            .map_or(String::new(), |a| a.as_str().to_string()),
        sex: id
            .sex
            .as_ref()
            .map_or(String::new(), |s| s.as_str().to_string()),
        group: id
            .group
            .as_ref()
            .map_or(String::new(), |g| g.as_str().to_string()),
        ses: id.ses.as_ref().map_or(String::new(), |s| s.as_str()),
        role: id.role.as_str().to_string(),
        education: id
            .education
            .as_ref()
            .map_or(String::new(), |e| e.as_str().to_string()),
        custom: id
            .custom_field
            .as_ref()
            .map_or(String::new(), |c| c.as_str().to_string()),
    }
}

/// Thin alias for the shared [`chat_file_cache::load_chat_file`]
/// entry point. Kept named `get_chat_file` because two call sites
/// in this module use it; new code should reach the shared module
/// directly.
fn get_chat_file(
    backend: &Backend,
    uri: &Url,
    doc: &str,
) -> Result<Arc<ChatFile>, LspBackendError> {
    crate::backend::chat_file_cache::load_chat_file(backend, uri, doc)
}
