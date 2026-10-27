//! LSP capability declarations advertised during `initialize`.
//!
//! [`build_initialize_result`] returns the negotiated `InitializeResult` that tells
//! the editor which features the server supports. Centralising this in one module
//! makes it straightforward to audit advertised capabilities against actual handler
//! implementations in [`super::requests`].
//!
//! Currently advertised: text document sync (incremental), hover, code actions,
//! completion (triggers: `*`, `%`, `+`), execute command, inlay hints, document
//! highlights, document symbols, folding ranges, semantic tokens (full + range),
//! formatting, goto-definition, references, code lens, and rename (with prepare).

use tower_lsp::Client;
use tower_lsp::lsp_types::*;

use super::execute_commands::ExecuteCommandName;
use crate::semantic_tokens::SemanticTokensProvider;

/// Evidence that this client accepts revision-bound workspace edits.
pub(crate) struct VersionedEdits(());

impl VersionedEdits {
    pub(super) fn negotiate(client: &ClientCapabilities) -> Option<Self> {
        client
            .workspace
            .as_ref()?
            .workspace_edit
            .as_ref()?
            .document_changes
            .filter(|supported| *supported)
            .map(|_| Self(()))
    }
}

/// Advertise quick fixes and rename only when the client can enforce revision binding.
pub(super) fn build_initialize_result(edits: Option<&VersionedEdits>) -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(PositionEncodingKind::UTF16),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL,
            )),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Simple(edits.is_some())),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![
                    "*".to_string(),
                    "%".to_string(),
                    "+".to_string(),
                    "@".to_string(),
                    "[".to_string(),
                ]),
                ..Default::default()
            }),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: ExecuteCommandName::advertised_commands(),
                work_done_progress_options: Default::default(),
            }),
            inlay_hint_provider: Some(OneOf::Left(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: SemanticTokensProvider::legend(),
                    range: Some(true),
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    ..Default::default()
                }),
            ),
            document_formatting_provider: Some(OneOf::Left(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            code_lens_provider: Some(CodeLensOptions {
                resolve_provider: Some(false),
            }),
            rename_provider: Some(match edits {
                Some(_) => OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                }),
                None => OneOf::Left(false),
            }),
            selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
            linked_editing_range_provider: Some(LinkedEditingRangeServerCapabilities::Simple(true)),
            document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                first_trigger_character: ":".to_string(),
                more_trigger_character: None,
            }),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            document_link_provider: Some(DocumentLinkOptions {
                resolve_provider: Some(false),
                work_done_progress_options: Default::default(),
            }),
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                identifier: Some("talkbank".to_string()),
                inter_file_dependencies: false,
                workspace_diagnostics: true,
                work_done_progress_options: Default::default(),
            })),
            ..Default::default()
        },
        server_info: Some(ServerInfo {
            name: "talkbank-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    }
}

/// Emit a one-time initialization message to the client log.
pub(super) async fn log_initialized(client: &Client) {
    client
        .log_message(MessageType::INFO, "TalkBank LSP server initialized")
        .await;
}
