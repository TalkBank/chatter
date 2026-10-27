//! Retained editor revisions and short, synchronous current-revision admission.

use std::sync::Arc;
use tower_lsp::{jsonrpc, lsp_types::Url};

use super::OpenDocument;
use crate::backend::state::Backend;

/// One retained open-document identity; it is not necessarily still current.
pub(crate) struct DocumentRevision {
    uri: Url,
    document: Arc<OpenDocument>,
}

/// Issued only while the document-map read guard pins the retained revision.
pub(crate) struct CurrentDocumentRevision<'a> {
    _current: &'a Arc<OpenDocument>,
}

/// A feature response kept private until its producing revision is current.
pub(crate) struct RevisionResponse<T> {
    revision: DocumentRevision,
    response: jsonrpc::Result<T>,
}

impl<T> RevisionResponse<T> {
    pub(crate) fn prepare(
        backend: &Backend,
        uri: &Url,
        project: impl FnOnce(&DocumentRevision) -> jsonrpc::Result<T>,
    ) -> jsonrpc::Result<Option<Self>> {
        let Some(revision) = DocumentRevision::capture(backend, uri)? else {
            return Ok(None);
        };
        let response = project(&revision);
        Ok(Some(Self { revision, response }))
    }

    pub(crate) fn finish(self, backend: &Backend) -> jsonrpc::Result<T> {
        let Self { revision, response } = self;
        revision
            .with_current(backend, |_| response)
            .unwrap_or_else(|| Err(content_modified()))
    }
}

impl DocumentRevision {
    pub(crate) fn retained(uri: Url, document: Arc<OpenDocument>) -> Self {
        Self { uri, document }
    }

    pub(crate) fn capture(backend: &Backend, uri: &Url) -> jsonrpc::Result<Option<Self>> {
        backend
            .documents
            .get(uri)
            .map(|entry| {
                entry
                    .current()
                    .map(|document| Self::retained(uri.clone(), Arc::clone(document)))
                    .ok_or_else(content_modified)
            })
            .transpose()
    }

    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }
    pub(crate) fn document(&self) -> &Arc<OpenDocument> {
        &self.document
    }
    pub(crate) fn source(&self) -> &str {
        &self.document.source
    }
    pub(crate) fn version(&self) -> i32 {
        self.document.version
    }

    /// No parse or await in the callback. Equal text/version after reopen is
    /// deliberately insufficient: the open-document Arc must be identical.
    pub(crate) fn with_current<T>(
        &self,
        backend: &Backend,
        project: impl FnOnce(CurrentDocumentRevision<'_>) -> T,
    ) -> Option<T> {
        let current = backend.documents.get(&self.uri)?;
        let current = current.current()?;
        Arc::ptr_eq(current, &self.document)
            .then(|| project(CurrentDocumentRevision { _current: current }))
    }
}

pub(crate) fn content_modified() -> jsonrpc::Error {
    jsonrpc::Error::new(jsonrpc::ErrorCode::ContentModified)
}
