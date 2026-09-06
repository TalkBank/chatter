//! Shared document, parse-tree, and `ChatFile` lookup helpers for request handlers.

use std::sync::Arc;

use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use crate::backend::LspBackendError;
use crate::backend::chat_file_cache;
use crate::backend::documents;
use crate::backend::state::Backend;

/// Return cached document text for one URI.
pub(super) fn document_text(backend: &Backend, uri: &Url) -> Option<String> {
    documents::get_document_text(backend, uri)
}

/// Return a parsed `ChatFile`, reparsing the document on cache miss.
///
/// Thin re-export of [`chat_file_cache::load_chat_file`] that keeps
/// the per-module name (`get_chat_file`) stable for existing callers
/// while the actual implementation lives in one shared place.
pub(super) fn get_chat_file(
    backend: &Backend,
    uri: &Url,
    doc: &str,
) -> Result<Arc<ChatFile>, LspBackendError> {
    chat_file_cache::load_chat_file(backend, uri, doc)
}

/// Return a current-source tree; transient request parses never advance the
/// authoritative analysis baseline during a debounce interval.
pub(super) fn get_parse_tree(backend: &Backend, uri: &Url, doc: &str) -> Option<Tree> {
    if let Some(analysis) = backend.analyses.get(uri)
        && let Some(current) = analysis.for_source(doc)
    {
        return current.tree();
    }

    match backend
        .language_services
        .with_parser(|parser| parser.parse_tree_incremental(doc, None))
    {
        Ok(Ok(tree)) => Some(tree),
        _ => None,
    }
}
