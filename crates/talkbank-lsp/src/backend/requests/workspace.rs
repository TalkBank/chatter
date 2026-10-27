use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{SymbolInformation, WorkspaceSymbolParams};

use crate::backend::features;
use crate::backend::state::Backend;

#[allow(deprecated)]
pub(super) async fn handle_workspace_symbol(
    backend: &Backend,
    params: WorkspaceSymbolParams,
) -> Result<Option<Vec<SymbolInformation>>> {
    let query = &params.query;
    let mut all_symbols = Vec::new();

    // Drop the map guards before the analysis owner reopens a document.
    let uris: Vec<_> = backend
        .documents
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for uri in uris {
        if let Some(symbols) =
            crate::backend::chat_file_cache::with_current_analysis(backend, &uri, |analysis| {
                features::workspace_symbols_for_document(&uri, analysis, query)
            })
            .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))?
        {
            all_symbols.extend(symbols);
        }
    }

    if all_symbols.is_empty() {
        Ok(None)
    } else {
        Ok(Some(all_symbols))
    }
}
