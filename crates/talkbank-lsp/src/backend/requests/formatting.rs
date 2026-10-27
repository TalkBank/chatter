use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::backend::documents::{DocumentRevision, RevisionResponse};
use crate::backend::state::Backend;

use super::context::get_chat_file;

pub(super) async fn handle_formatting(
    backend: &Backend,
    params: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    let uri = params.text_document.uri;
    let Some(proposal) = prepare_formatting(backend, &uri)? else {
        return Ok(None);
    };
    proposal.finish(backend)
}

/// Formatting results cannot be exposed without re-admitting their producer.
/// The response stays private, including no-op and refusal results.
type FormattingProposal = RevisionResponse<Option<Vec<TextEdit>>>;

fn prepare_formatting(backend: &Backend, uri: &Url) -> Result<Option<FormattingProposal>> {
    RevisionResponse::prepare(backend, uri, |revision| {
        formatting_response(backend, revision)
    })
}

fn formatting_response(
    backend: &Backend,
    revision: &DocumentRevision,
) -> Result<Option<Vec<TextEdit>>> {
    let uri = revision.uri();
    let doc = revision.source();
    let chat_file = match get_chat_file(backend, uri, doc) {
        Ok(file) => file,
        Err(error) => return Err(tower_lsp::jsonrpc::Error::invalid_params(error.to_string())),
    };

    // A recovered model is useful for read-only features, but serialization
    // alone does not authorize replacing the editor buffer.
    let rewrite = talkbank_transform::Rewrite::of(&chat_file, doc)
        .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))?;
    let formatted = rewrite.text();
    if formatted == doc {
        return Ok(None);
    }

    Ok(Some(vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: crate::backend::utils::offset_to_position(doc, doc.len()),
        },
        new_text: formatted.to_owned(),
    }]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::documents::{DocumentSession, OpenDocument};
    use std::sync::Arc;

    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/core/basic-conversation.cha"
    ));

    #[derive(Debug)]
    enum Replacement {
        Changed,
        Reopened,
        Closed,
        Unsynchronized,
    }

    #[test]
    fn formatting_proposals_require_the_producing_open_revision() {
        let (service, _socket) = tower_lsp::LspService::new(Backend::new);
        let backend = service.inner();
        let uri = Url::parse("file:///formatting-revision.cha").unwrap();
        let source: Arc<str> = SOURCE.replacen("I want", "I  want", 1).into();
        for replacement in [
            Replacement::Changed,
            Replacement::Reopened,
            Replacement::Closed,
            Replacement::Unsynchronized,
        ] {
            backend.documents.insert(
                uri.clone(),
                DocumentSession::Synchronized(Arc::new(OpenDocument {
                    source: Arc::clone(&source),
                    version: 1,
                })),
            );
            let current = prepare_formatting(backend, &uri).unwrap().unwrap();
            let edits = current.finish(backend).unwrap().unwrap();
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0].new_text, SOURCE);
            let stale = prepare_formatting(backend, &uri).unwrap().unwrap();
            match replacement {
                Replacement::Changed => {
                    backend.documents.insert(
                        uri.clone(),
                        DocumentSession::Synchronized(Arc::new(OpenDocument {
                            source: Arc::from(SOURCE),
                            version: 2,
                        })),
                    );
                }
                Replacement::Reopened => {
                    backend.documents.remove(&uri);
                    backend.documents.insert(
                        uri.clone(),
                        DocumentSession::Synchronized(Arc::new(OpenDocument {
                            source: Arc::clone(&source),
                            version: 1,
                        })),
                    );
                }
                Replacement::Closed => {
                    backend.documents.remove(&uri);
                }
                Replacement::Unsynchronized => {
                    backend.documents.insert(
                        uri.clone(),
                        DocumentSession::AwaitingReplacement { version: 2 },
                    );
                }
            }
            assert_eq!(
                stale.finish(backend).unwrap_err().code,
                tower_lsp::jsonrpc::ErrorCode::ContentModified,
                "{replacement:?}"
            );
        }
    }
}
