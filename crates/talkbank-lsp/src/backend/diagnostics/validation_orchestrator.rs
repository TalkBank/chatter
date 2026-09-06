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

/// A pull report names the same revision as its diagnostics.
pub(crate) struct RevisionDiagnostics {
    pub(crate) version: i32,
    pub(crate) items: Vec<Diagnostic>,
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
        version: document.version,
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
