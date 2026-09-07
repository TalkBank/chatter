use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::backend::state::Backend;

use super::context::{document_text, get_chat_file};

pub(super) async fn handle_formatting(
    backend: &Backend,
    params: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    let uri = params.text_document.uri;
    let doc = match document_text(backend, &uri) {
        Some(doc) => doc,
        None => return Ok(None),
    };
    let chat_file = match get_chat_file(backend, &uri, &doc) {
        Ok(file) => file,
        Err(error) => return Err(tower_lsp::jsonrpc::Error::invalid_params(error.to_string())),
    };

    let formatted = chat_file.to_chat();
    if formatted == doc {
        return Ok(None);
    }

    Ok(Some(vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: crate::backend::utils::offset_to_position(&doc, doc.len() as u32),
        },
        new_text: formatted,
    }]))
}
