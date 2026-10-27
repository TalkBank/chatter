//! Text/document request-family routing for the LSP backend.
//!
//! This module groups the remaining text/document request handlers behind a
//! small service composition root so request routing shares context-resolution
//! helpers instead of open-coding document, CST, and `ChatFile` lookup in
//! separate thin wrapper modules.

use std::sync::Arc;

use talkbank_model::model::ChatFile;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

use crate::backend::documents::RevisionResponse;
use crate::backend::features;
use crate::backend::state::Backend;

use super::context::{document_text, get_chat_file, get_parse_tree};
use super::goto_definition;

/// Composition root for text/document request-family services.
struct TextDocumentServices {
    document_features: DocumentFeatureService,
    interaction_features: InteractionFeatureService,
    navigation: NavigationRequestService,
}

impl TextDocumentServices {
    /// Create the stateless request-family services.
    fn new() -> Self {
        Self {
            document_features: DocumentFeatureService,
            interaction_features: InteractionFeatureService,
            navigation: NavigationRequestService,
        }
    }
}

/// Shared document text resolved for one LSP request.
struct TextDocumentContext {
    uri: Url,
    doc: String,
}

impl TextDocumentContext {
    /// Resolve the current document text for one URI.
    fn resolve(backend: &Backend, uri: Url) -> Option<Self> {
        let doc = document_text(backend, &uri)?;
        Some(Self { uri, doc })
    }
}

/// Document text plus a cached or reparsed tree-sitter CST.
struct SyntaxTextDocumentContext {
    uri: Url,
    doc: String,
    tree: Tree,
}

impl SyntaxTextDocumentContext {
    /// Resolve document text plus parse tree for one URI.
    fn resolve(backend: &Backend, uri: Url) -> Option<Self> {
        let TextDocumentContext { uri, doc } = TextDocumentContext::resolve(backend, uri)?;
        Self::from_source(backend, uri, doc)
    }

    fn from_source(backend: &Backend, uri: Url, doc: String) -> Option<Self> {
        let tree = get_parse_tree(backend, &uri, &doc)?;
        Some(Self { uri, doc, tree })
    }
}

/// Document text plus the parsed `ChatFile` model.
struct ParsedTextDocumentContext {
    doc: String,
    chat_file: Arc<ChatFile>,
}

impl ParsedTextDocumentContext {
    /// Resolve document text plus parsed model for one URI.
    fn resolve(backend: &Backend, uri: Url) -> Result<Option<Self>> {
        let Some(TextDocumentContext { uri, doc }) = TextDocumentContext::resolve(backend, uri)
        else {
            return Ok(None);
        };
        let chat_file = get_chat_file(backend, &uri, &doc)
            .map_err(|err| tower_lsp::jsonrpc::Error::invalid_params(err.to_string()))?;
        Ok(Some(Self { doc, chat_file }))
    }
}

/// Fully resolved text-document context used by features that need text, tree, and model.
struct FullTextDocumentContext {
    uri: Url,
    doc: String,
    tree: Tree,
    chat_file: Arc<ChatFile>,
}

impl FullTextDocumentContext {
    /// Resolve document text, parse tree, and parsed model for one URI.
    fn resolve(backend: &Backend, uri: Url) -> Result<Option<Self>> {
        let Some(TextDocumentContext { uri, doc }) = TextDocumentContext::resolve(backend, uri)
        else {
            return Ok(None);
        };
        Self::from_source(backend, uri, doc)
    }

    fn from_source(backend: &Backend, uri: Url, doc: String) -> Result<Option<Self>> {
        let Some(tree) = get_parse_tree(backend, &uri, &doc) else {
            return Ok(None);
        };
        let chat_file = get_chat_file(backend, &uri, &doc)
            .map_err(|err| tower_lsp::jsonrpc::Error::invalid_params(err.to_string()))?;
        Ok(Some(Self {
            uri,
            doc,
            tree,
            chat_file,
        }))
    }
}

/// Service for text/document features that depend on document text or parsed models.
struct DocumentFeatureService;

impl DocumentFeatureService {
    fn handle_code_action(
        &self,
        backend: &Backend,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let Some(support) = backend.versioned_edits.get().and_then(Option::as_ref) else {
            return Ok(None);
        };
        let Some(revision) =
            crate::backend::diagnostics::document_diagnostics(backend, &params.text_document.uri)?
        else {
            return Ok(None);
        };

        let Some(scope) =
            features::QuickFixScope::admit(&revision, params.range, params.context.only.as_deref())
        else {
            return Ok(None);
        };

        backend
            .language_services
            .with_parser(|parser| {
                let prepared = features::prepare_code_actions(scope, parser);
                revision.with_current(backend, |current| prepared.finish(current, support))
            })
            .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))?
            .ok_or_else(|| {
                tower_lsp::jsonrpc::Error::new(tower_lsp::jsonrpc::ErrorCode::ContentModified)
            })
    }

    fn handle_inlay_hint(
        &self,
        backend: &Backend,
        params: InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        let Some(context) = ParsedTextDocumentContext::resolve(backend, params.text_document.uri)?
        else {
            return Ok(None);
        };

        let hints =
            features::generate_alignment_hints(&context.chat_file, &context.doc, params.range);
        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    fn handle_selection_range(
        &self,
        backend: &Backend,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let Some(context) = TextDocumentContext::resolve(backend, params.text_document.uri) else {
            return Ok(None);
        };

        Ok(Some(features::selection_range(
            &context.doc,
            &params.positions,
        )))
    }

    fn handle_on_type_formatting(
        &self,
        backend: &Backend,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let position = params.text_document_position.position;
        let ch = params.ch;
        let uri = params.text_document_position.text_document.uri;
        let Some(response) = RevisionResponse::prepare(backend, &uri, |revision| {
            Ok(features::on_type_formatting(
                revision.source(),
                position,
                &ch,
            ))
        })?
        else {
            return Ok(None);
        };

        response.finish(backend)
    }

    fn handle_document_link(
        &self,
        backend: &Backend,
        params: DocumentLinkParams,
    ) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        let Some(links) =
            crate::backend::chat_file_cache::with_current_analysis(backend, &uri, |analysis| {
                features::document_links(&uri, analysis)
            })
            .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))?
        else {
            return Ok(None);
        };
        if links.is_empty() {
            Ok(None)
        } else {
            Ok(Some(links))
        }
    }
}

/// Service for interactive text/document features that depend on the current cursor position.
struct InteractionFeatureService;

impl InteractionFeatureService {
    fn handle_hover(&self, backend: &Backend, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let parse_state = backend.parse_state(&uri);
        let Some(context) = FullTextDocumentContext::resolve(backend, uri)? else {
            return Ok(None);
        };

        Ok(features::hover(
            &context.chat_file,
            &context.tree,
            position,
            &context.doc,
            parse_state,
        ))
    }

    fn handle_completion(
        &self,
        backend: &Backend,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        let uri = params.text_document_position.text_document.uri;
        let Some(response) = RevisionResponse::prepare(backend, &uri, |revision| {
            let Some(context) = FullTextDocumentContext::from_source(
                backend,
                revision.uri().clone(),
                revision.source().to_owned(),
            )?
            else {
                return Ok(None);
            };
            Ok(features::completion(
                &context.chat_file,
                &context.tree,
                &context.doc,
                position,
            ))
        })?
        else {
            return Ok(None);
        };
        response.finish(backend)
    }

    fn handle_document_highlight(
        &self,
        backend: &Backend,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let position = params.text_document_position_params.position;
        let Some(context) = FullTextDocumentContext::resolve(
            backend,
            params.text_document_position_params.text_document.uri,
        )?
        else {
            return Ok(None);
        };

        Ok(features::document_highlights(
            &context.chat_file,
            &context.tree,
            position,
            &context.doc,
        ))
    }

    fn handle_linked_editing_range(
        &self,
        backend: &Backend,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let Some(response) = RevisionResponse::prepare(backend, &uri, |revision| {
            let Some(context) = SyntaxTextDocumentContext::from_source(
                backend,
                revision.uri().clone(),
                revision.source().to_owned(),
            ) else {
                return Ok(None);
            };
            Ok(features::linked_editing_ranges(
                &context.tree,
                &context.doc,
                position,
            ))
        })?
        else {
            return Ok(None);
        };
        response.finish(backend)
    }
}

/// Service for navigation and rename features over one text document.
struct NavigationRequestService;

impl NavigationRequestService {
    fn handle_references(
        &self,
        backend: &Backend,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        let Some(context) = SyntaxTextDocumentContext::resolve(
            backend,
            params.text_document_position.text_document.uri,
        ) else {
            return Ok(None);
        };

        Ok(features::references::references(
            &context.tree,
            &context.uri,
            &context.doc,
            position,
            include_declaration,
        ))
    }

    fn handle_prepare_rename(
        &self,
        backend: &Backend,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        if backend
            .versioned_edits
            .get()
            .and_then(Option::as_ref)
            .is_none()
        {
            return Ok(None);
        }
        let Some(revision) =
            crate::backend::diagnostics::document_diagnostics(backend, &params.text_document.uri)?
        else {
            return Ok(None);
        };
        let Some(tree) = get_parse_tree(backend, revision.uri(), revision.source()) else {
            return Ok(None);
        };
        let prepared = features::rename::prepare_rename(&tree, revision.source(), params.position);
        revision.with_current(backend, |_| prepared).ok_or_else(|| {
            tower_lsp::jsonrpc::Error::new(tower_lsp::jsonrpc::ErrorCode::ContentModified)
        })
    }

    fn handle_rename(
        &self,
        backend: &Backend,
        params: RenameParams,
    ) -> Result<Option<WorkspaceEdit>> {
        let Some(support) = backend.versioned_edits.get().and_then(Option::as_ref) else {
            return Ok(None);
        };
        let Some(revision) = crate::backend::diagnostics::document_diagnostics(
            backend,
            &params.text_document_position.text_document.uri,
        )?
        else {
            return Ok(None);
        };
        let Some(tree) = get_parse_tree(backend, revision.uri(), revision.source()) else {
            return Ok(None);
        };
        let prepared = features::rename::prepare_rename_edit(
            &tree,
            &revision,
            params.text_document_position.position,
            &params.new_name,
        )
        .map_err(|error| tower_lsp::jsonrpc::Error::invalid_params(error.to_string()))?;
        revision
            .with_current(backend, |current| {
                prepared.and_then(|prepared| prepared.finish(current, support))
            })
            .ok_or_else(|| {
                tower_lsp::jsonrpc::Error::new(tower_lsp::jsonrpc::ErrorCode::ContentModified)
            })
    }

    fn handle_goto_definition(
        &self,
        backend: &Backend,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let position = params.text_document_position_params.position;
        let Some(context) = FullTextDocumentContext::resolve(
            backend,
            params.text_document_position_params.text_document.uri,
        )?
        else {
            return Ok(None);
        };

        Ok(goto_definition::goto_definition(
            &context.chat_file,
            &context.uri,
            &context.doc,
            &context.tree,
            position,
        ))
    }
}

pub(super) async fn handle_hover(backend: &Backend, params: HoverParams) -> Result<Option<Hover>> {
    TextDocumentServices::new()
        .interaction_features
        .handle_hover(backend, params)
}

pub(super) async fn handle_code_action(
    backend: &Backend,
    params: CodeActionParams,
) -> Result<Option<CodeActionResponse>> {
    TextDocumentServices::new()
        .document_features
        .handle_code_action(backend, params)
}

pub(super) async fn handle_completion(
    backend: &Backend,
    params: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    TextDocumentServices::new()
        .interaction_features
        .handle_completion(backend, params)
}

pub(super) async fn handle_inlay_hint(
    backend: &Backend,
    params: InlayHintParams,
) -> Result<Option<Vec<InlayHint>>> {
    TextDocumentServices::new()
        .document_features
        .handle_inlay_hint(backend, params)
}

pub(super) async fn handle_document_highlight(
    backend: &Backend,
    params: DocumentHighlightParams,
) -> Result<Option<Vec<DocumentHighlight>>> {
    TextDocumentServices::new()
        .interaction_features
        .handle_document_highlight(backend, params)
}

pub(super) async fn handle_references(
    backend: &Backend,
    params: ReferenceParams,
) -> Result<Option<Vec<Location>>> {
    TextDocumentServices::new()
        .navigation
        .handle_references(backend, params)
}

pub(super) async fn handle_prepare_rename(
    backend: &Backend,
    params: TextDocumentPositionParams,
) -> Result<Option<PrepareRenameResponse>> {
    TextDocumentServices::new()
        .navigation
        .handle_prepare_rename(backend, params)
}

pub(super) async fn handle_rename(
    backend: &Backend,
    params: RenameParams,
) -> Result<Option<WorkspaceEdit>> {
    TextDocumentServices::new()
        .navigation
        .handle_rename(backend, params)
}

pub(super) async fn handle_goto_definition(
    backend: &Backend,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    TextDocumentServices::new()
        .navigation
        .handle_goto_definition(backend, params)
}

pub(super) async fn handle_selection_range(
    backend: &Backend,
    params: SelectionRangeParams,
) -> Result<Option<Vec<SelectionRange>>> {
    TextDocumentServices::new()
        .document_features
        .handle_selection_range(backend, params)
}

pub(super) async fn handle_linked_editing_range(
    backend: &Backend,
    params: LinkedEditingRangeParams,
) -> Result<Option<LinkedEditingRanges>> {
    TextDocumentServices::new()
        .interaction_features
        .handle_linked_editing_range(backend, params)
}

pub(super) async fn handle_on_type_formatting(
    backend: &Backend,
    params: DocumentOnTypeFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    TextDocumentServices::new()
        .document_features
        .handle_on_type_formatting(backend, params)
}

pub(super) async fn handle_document_link(
    backend: &Backend,
    params: DocumentLinkParams,
) -> Result<Option<Vec<DocumentLink>>> {
    TextDocumentServices::new()
        .document_features
        .handle_document_link(backend, params)
}
