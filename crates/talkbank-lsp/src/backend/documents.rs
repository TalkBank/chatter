//! Versioned document lifecycle and debounced analysis scheduling.

use super::{diagnostics, state::Backend};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tower_lsp::lsp_types::*;

mod revision;
pub(crate) use revision::{
    CurrentDocumentRevision, DocumentRevision, RevisionResponse, content_modified,
};

const DEBOUNCE_MS: u64 = 250;

/// One editor revision. Arc identity also distinguishes close/reopen cycles.
pub(crate) struct OpenDocument {
    pub(crate) source: Arc<str>,
    pub(crate) version: i32,
}

/// A lost synchronization state carries no source that features could consume.
pub(crate) enum DocumentSession {
    Synchronized(Arc<OpenDocument>),
    AwaitingReplacement { version: i32 },
}

impl DocumentSession {
    pub(crate) fn current(&self) -> Option<&Arc<OpenDocument>> {
        match self {
            Self::Synchronized(document) => Some(document),
            Self::AwaitingReplacement { .. } => None,
        }
    }

    fn version(&self) -> i32 {
        match self {
            Self::Synchronized(document) => document.version,
            Self::AwaitingReplacement { version } => *version,
        }
    }

    /// Every ranged edit requires a source from a synchronized revision or an
    /// earlier full replacement in this batch. No partial batch is published.
    fn revised(
        &self,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<OpenDocument, DocumentChangeError> {
        let mut source = self.current().map(|document| document.source.to_string());
        for change in changes {
            if let Some(range) = change.range {
                let source = source
                    .as_mut()
                    .ok_or(DocumentChangeError::ReplacementRequired)?;
                AdmittedDocumentEdit::new(source, range, &change.text)?.apply();
            } else {
                source = Some(change.text);
            }
        }
        Ok(OpenDocument {
            source: source
                .ok_or(DocumentChangeError::ReplacementRequired)?
                .into(),
            version,
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum DocumentChangeError {
    #[error("document edit range is reversed")]
    ReversedRange,
    #[error("a full document replacement is required")]
    ReplacementRequired,
    #[error(transparent)]
    Position(#[from] super::utils::EditPositionError),
}

/// The mutable revision and its admitted UTF-8 edit range travel together.
/// No caller can apply an unadmitted or differently sourced byte range.
struct AdmittedDocumentEdit<'source, 'replacement> {
    source: &'source mut String,
    range: std::ops::Range<usize>,
    replacement: &'replacement str,
}

impl<'source, 'replacement> AdmittedDocumentEdit<'source, 'replacement> {
    fn new(
        source: &'source mut String,
        range: Range,
        replacement: &'replacement str,
    ) -> Result<Self, DocumentChangeError> {
        if range.start > range.end {
            return Err(DocumentChangeError::ReversedRange);
        }
        let start = super::utils::edit_position_to_offset(source, range.start)?;
        let end = super::utils::edit_position_to_offset(source, range.end)?;
        Ok(Self {
            source,
            range: start..end,
            replacement,
        })
    }

    fn apply(self) {
        self.source.replace_range(self.range, self.replacement);
    }
}

pub(super) fn get_document_text(backend: &Backend, uri: &Url) -> Option<String> {
    backend
        .documents
        .get(uri)
        .and_then(|entry| entry.current().map(|document| document.source.to_string()))
}

pub(super) async fn handle_did_open(backend: &Backend, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let document = Arc::new(OpenDocument {
        source: params.text_document.text.into(),
        version: params.text_document.version,
    });
    backend.pending_validations.remove(&uri);
    backend.documents.insert(
        uri.clone(),
        DocumentSession::Synchronized(Arc::clone(&document)),
    );
    diagnostics::validate_and_publish(backend, uri, document).await;
}

pub(super) async fn handle_did_change(backend: &Backend, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    let outcome = {
        let Some(mut current) = backend.documents.get_mut(&uri) else {
            return;
        };
        if version <= current.version() {
            return;
        }
        match current.revised(version, params.content_changes) {
            Ok(document) => {
                let document = Arc::new(document);
                *current = DocumentSession::Synchronized(Arc::clone(&document));
                Ok(document)
            }
            Err(error) => {
                *current = DocumentSession::AwaitingReplacement { version };
                backend.pending_validations.remove(&uri);
                backend.analyses.remove(&uri);
                Err(error)
            }
        }
    };
    let document = match outcome {
        Ok(document) => document,
        Err(error) => {
            backend
                .client
                .publish_diagnostics(uri.clone(), vec![], Some(version))
                .await;
            backend.client.log_message(MessageType::WARNING, format!(
                "Document synchronization lost for {uri}: {error}. Send full text or reopen the document."
            )).await;
            return;
        }
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
        .and_then(|entry| entry.current().cloned());
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
