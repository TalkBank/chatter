use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::backend::chat_file_cache::with_current_analysis;
use crate::backend::features;
use crate::backend::state::Backend;

pub(super) async fn handle_document_symbol(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> Result<Option<DocumentSymbolResponse>> {
    with_current_analysis(
        backend,
        &params.text_document.uri,
        features::document_symbol,
    )
    .map(Option::flatten)
    .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))
}

pub(super) async fn handle_folding_range(
    backend: &Backend,
    params: FoldingRangeParams,
) -> Result<Option<Vec<FoldingRange>>> {
    with_current_analysis(backend, &params.text_document.uri, features::folding_range)
        .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))
}

pub(super) async fn handle_code_lens(
    backend: &Backend,
    params: CodeLensParams,
) -> Result<Option<Vec<CodeLens>>> {
    with_current_analysis(
        backend,
        &params.text_document.uri,
        features::code_lens::code_lens,
    )
    .map(Option::flatten)
    .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))
}
