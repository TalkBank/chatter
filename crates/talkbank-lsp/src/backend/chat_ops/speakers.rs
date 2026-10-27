//! Speaker extraction for `talkbank/getSpeakers`.

use serde::Serialize;
use talkbank_model::model::ParticipantDeclarationError;

use crate::backend::state::Backend;

use super::shared::get_document_and_chat_file;
use crate::backend::execute_commands::DocumentUriRequest;

/// One speaker entry returned to the VS Code extension.
#[derive(Serialize)]
struct SpeakerInfo {
    /// CHAT speaker code.
    code: String,
    /// Participant display name.
    name: Option<String>,
    /// Participant role label.
    role: Option<String>,
}

/// Handle `talkbank/getSpeakers`.
pub(crate) fn handle_get_speakers(
    backend: &Backend,
    request: &DocumentUriRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (_, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let speakers = extract_speakers(&chat_file)?;
    serde_json::to_value(&speakers).map_err(Into::into)
}

/// Extract speakers only from one admitted shared-model declaration.
fn extract_speakers(
    chat_file: &talkbank_model::model::ChatFile,
) -> Result<Vec<SpeakerInfo>, ParticipantDeclarationError> {
    Ok(chat_file
        .participant_declaration()?
        .iter()
        .map(|participant| SpeakerInfo {
            code: participant.speaker_code.as_str().to_owned(),
            name: participant.name().map(|name| name.as_str().to_owned()),
            role: participant.role().map(|role| role.as_str().to_owned()),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_parser_tests::test_error::ParseProductTestExt;

    #[test]
    fn missing_declared_role_is_null_even_when_id_supplies_metadata() {
        let source = include_str!(
            "../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E513_1.cha"
        );
        let file = talkbank_parser::TreeSitterParser::new()
            .expect("parser")
            .parse_chat_file(source)
            .expect_built();
        let speakers = extract_speakers(&file).expect("admitted declaration");
        assert_eq!(speakers.len(), 1);
        assert_eq!(speakers[0].code, "CHI");
        assert!(speakers[0].role.is_none());
        let wire = serde_json::to_value(speakers).expect("wire");
        assert!(wire[0]["role"].is_null());
    }
}
