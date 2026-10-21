//! Publish a coherent analysis only while its document revision remains current.

use super::super::documents::OpenDocument;
use super::super::state::{Backend, BackendInitError};
use super::DocumentAnalysis;
use std::sync::Arc;
use tower_lsp::lsp_types::*;

/// Analyze one revision. No DashMap guard crosses an await or parser callback.
pub(crate) async fn validate_and_publish(backend: &Backend, uri: Url, document: Arc<OpenDocument>) {
    if let Some(diagnostics) = analyze_revision(backend, &uri, &document) {
        backend
            .client
            .publish_diagnostics(uri, diagnostics, Some(document.version))
            .await;
    }
}

/// Source, URI, version and diagnostics from one admitted editor revision.
pub(crate) struct RevisionDiagnostics {
    uri: Url,
    document: Arc<OpenDocument>,
    items: Vec<Diagnostic>,
}

/// A revision held current for a synchronous feature projection.
pub(crate) struct CurrentRevisionDiagnostics<'a> {
    snapshot: &'a RevisionDiagnostics,
    // Borrowed from the document-map guard, so this admission cannot escape it.
    _current: &'a Arc<OpenDocument>,
}

impl CurrentRevisionDiagnostics<'_> {
    pub(crate) fn snapshot(&self) -> &RevisionDiagnostics {
        self.snapshot
    }
}

impl RevisionDiagnostics {
    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }

    pub(crate) fn version(&self) -> i32 {
        self.document.version
    }

    pub(crate) fn source(&self) -> &str {
        &self.document.source
    }

    pub(crate) fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    pub(crate) fn into_items(self) -> Vec<Diagnostic> {
        self.items
    }

    /// No parse or await in the callback: the read guard pins this open revision.
    pub(crate) fn with_current<T>(
        &self,
        backend: &Backend,
        project: impl FnOnce(CurrentRevisionDiagnostics<'_>) -> T,
    ) -> Option<T> {
        let current = backend.documents.get(&self.uri)?;
        Arc::ptr_eq(current.value(), &self.document).then(|| {
            project(CurrentRevisionDiagnostics {
                snapshot: self,
                _current: current.value(),
            })
        })
    }
}

/// Pull current analysis, including during the push debounce interval.
pub(crate) fn document_diagnostics(
    backend: &Backend,
    uri: &Url,
) -> tower_lsp::jsonrpc::Result<Option<RevisionDiagnostics>> {
    let document = backend
        .documents
        .get(uri)
        .map(|entry| Arc::clone(entry.value()));
    let Some(document) = document else {
        return Ok(None);
    };
    let items = analyze_revision(backend, uri, &document).ok_or_else(|| {
        tower_lsp::jsonrpc::Error::new(tower_lsp::jsonrpc::ErrorCode::ContentModified)
    })?;
    Ok(Some(RevisionDiagnostics {
        uri: uri.clone(),
        document,
        items,
    }))
}

fn analyze_revision(
    backend: &Backend,
    uri: &Url,
    document: &Arc<OpenDocument>,
) -> Option<Vec<Diagnostic>> {
    let previous = backend
        .analyses
        .get(uri)
        .map(|entry| Arc::clone(entry.value()));
    let analysis = if let Some(previous) = previous
        .as_ref()
        .filter(|old| old.for_source(&document.source).is_some())
    {
        Ok(Arc::clone(previous))
    } else {
        backend.language_services.with_parser(|parser| {
            Arc::new(DocumentAnalysis::parse(
                parser,
                uri,
                Arc::clone(&document.source),
                previous.as_deref(),
            ))
        })
    };
    let current = backend.documents.get(uri)?;
    if !Arc::ptr_eq(current.value(), document) {
        return None;
    }
    Some(match analysis {
        Ok(analysis) => {
            let diagnostics = analysis.diagnostics().to_vec();
            backend.analyses.insert(uri.clone(), analysis);
            diagnostics
        }
        Err(error) => {
            backend.analyses.remove(uri);
            vec![initialization_diagnostic(&error)]
        }
    })
}

/// Convert a backend service initialization failure into a single LSP diagnostic.
fn initialization_diagnostic(error: &BackendInitError) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("talkbank-lsp".to_string()),
        message: error.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_projection_refuses_replacement_even_with_equal_source_and_version() {
        let (service, _socket) = tower_lsp::LspService::new(Backend::new);
        let backend = service.inner();
        let uri = Url::parse("file:///revision-admission.cha").unwrap();
        let source: Arc<str> = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/core/basic-conversation.cha"
        ))
        .into();
        backend.documents.insert(
            uri.clone(),
            Arc::new(OpenDocument {
                source: Arc::clone(&source),
                version: 1,
            }),
        );
        let original = document_diagnostics(backend, &uri).unwrap().unwrap();
        assert_eq!(
            original.with_current(backend, |current| current.snapshot().version()),
            Some(1)
        );
        backend.documents.remove(&uri);
        assert!(original.with_current(backend, |_| ()).is_none());
        backend.documents.insert(
            uri.clone(),
            Arc::new(OpenDocument {
                source: Arc::clone(&source),
                version: 1,
            }),
        );
        assert!(
            original.with_current(backend, |_| ()).is_none(),
            "reopen is not the old revision"
        );
        let reopened = document_diagnostics(backend, &uri).unwrap().unwrap();
        assert_eq!(
            reopened.with_current(backend, |current| current.snapshot().version()),
            Some(1)
        );
        backend
            .documents
            .insert(uri.clone(), Arc::new(OpenDocument { source, version: 2 }));
        assert!(reopened.with_current(backend, |_| ()).is_none());
        let updated = document_diagnostics(backend, &uri).unwrap().unwrap();
        assert_eq!(
            updated.with_current(backend, |current| current.snapshot().version()),
            Some(2)
        );
    }
}
