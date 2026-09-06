//! Versioned document lifecycle and debounced analysis scheduling.

use super::{diagnostics, state::Backend};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tower_lsp::lsp_types::*;

const DEBOUNCE_MS: u64 = 250;

/// One editor revision. Arc identity also distinguishes close/reopen cycles.
pub(crate) struct OpenDocument {
    pub(crate) source: Arc<str>,
    pub(crate) version: i32,
}

pub(super) fn get_document_text(backend: &Backend, uri: &Url) -> Option<String> {
    backend
        .documents
        .get(uri)
        .map(|entry| entry.source.to_string())
}

pub(super) async fn handle_did_open(backend: &Backend, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let document = Arc::new(OpenDocument {
        source: params.text_document.text.into(),
        version: params.text_document.version,
    });
    backend.pending_validations.remove(&uri);
    backend.documents.insert(uri.clone(), Arc::clone(&document));
    diagnostics::validate_and_publish(backend, uri, document).await;
}

pub(super) async fn handle_did_change(backend: &Backend, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let document = {
        let Some(mut current) = backend.documents.get_mut(&uri) else {
            return;
        };
        if params.text_document.version <= current.version {
            return;
        }
        let mut source = current.source.to_string();
        for change in params.content_changes {
            if let Some(range) = change.range {
                let start = super::utils::position_to_offset(&source, range.start);
                let end = super::utils::position_to_offset(&source, range.end);
                if start > end {
                    tracing::warn!(%uri, "ignoring reversed document edit range");
                    return;
                }
                source.replace_range(start..end, &change.text);
            } else {
                source = change.text;
            }
        }
        let document = Arc::new(OpenDocument {
            source: source.into(),
            version: params.text_document.version,
        });
        *current = Arc::clone(&document);
        document
    };
    let validation_id = backend.validation_counter.fetch_add(1, Ordering::SeqCst);
    backend
        .pending_validations
        .insert(uri.clone(), validation_id);
    let backend = backend.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
        let latest = backend
            .pending_validations
            .get(&uri)
            .is_some_and(|id| *id == validation_id);
        if latest {
            diagnostics::validate_and_publish(&backend, uri, document).await;
        }
    });
}

pub(super) async fn handle_did_save(backend: &Backend, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    let document = backend
        .documents
        .get(&uri)
        .map(|entry| Arc::clone(entry.value()));
    if let Some(document) = document {
        // An identical-source analysis is reused by the publisher. An older
        // revision's cache can never suppress validation of the saved source.
        diagnostics::validate_and_publish(backend, uri, document).await;
    }
}

pub(super) async fn handle_did_close(backend: &Backend, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    backend.documents.remove(&uri);
    backend.pending_validations.remove(&uri);
    backend.analyses.remove(&uri);
    backend.client.publish_diagnostics(uri, vec![], None).await;
}
