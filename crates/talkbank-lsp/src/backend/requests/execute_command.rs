//! Execute-command request dispatch for the LSP backend.

use crate::backend::LspBackendError;
use serde_json::Value;
use tower_lsp::jsonrpc::{Error, ErrorCode, Result as RpcResult};
use tower_lsp::lsp_types::ExecuteCommandParams;

use crate::backend::execute_commands::{
    DocumentCommandRequest, DocumentPositionRequest, DocumentUriRequest, ExecuteCommandName,
    ExecuteCommandRequest,
};
use crate::backend::state::Backend;
use crate::backend::utils;
use crate::backend::{chat_ops::ChatOpsCommandService, participants::ParticipantCommandService};

use super::alignment_sidecar;
use super::context::{document_text, get_chat_file};

/// Composition root for execute-command feature services.
struct ExecuteCommandServices {
    documents: DocumentCommandService,
    participants: ParticipantCommandService,
    chat_ops: ChatOpsCommandService,
}

impl ExecuteCommandServices {
    /// Create the stateless service set used to route execute-command requests.
    fn new() -> Self {
        Self {
            documents: DocumentCommandService,
            participants: ParticipantCommandService,
            chat_ops: ChatOpsCommandService,
        }
    }

    /// Route one typed execute-command request to its feature service.
    fn dispatch(
        &self,
        backend: &Backend,
        request: ExecuteCommandRequest,
    ) -> Result<Value, LspBackendError> {
        match request {
            ExecuteCommandRequest::Documents(request) => self.documents.dispatch(backend, request),
            ExecuteCommandRequest::Participants(request) => {
                self.participants.dispatch(backend, request)
            }
            ExecuteCommandRequest::ChatOps(request) => self.chat_ops.dispatch(backend, request),
        }
    }
}

/// Service object for document-local graph and alignment commands.
struct DocumentCommandService;

impl DocumentCommandService {
    /// Dispatch one document-command request.
    fn dispatch(
        &self,
        backend: &Backend,
        request: DocumentCommandRequest,
    ) -> Result<Value, LspBackendError> {
        match request {
            DocumentCommandRequest::ShowDependencyGraph(request) => {
                self.handle_dependency_graph_command(backend, &request)
            }
            DocumentCommandRequest::GetAlignmentSidecar(request) => {
                self.handle_alignment_sidecar_command(backend, &request)
            }
        }
    }

    /// Resolve an alignment-sidecar request against one open document.
    fn handle_alignment_sidecar_command(
        &self,
        backend: &Backend,
        request: &DocumentUriRequest,
    ) -> Result<Value, LspBackendError> {
        let text = document_text(backend, &request.uri).ok_or(LspBackendError::DocumentNotFound)?;
        let chat_file = get_chat_file(backend, &request.uri, &text)?;
        let sidecar = alignment_sidecar::build_alignment_sidecar(&request.uri, &text, &chat_file);
        serde_json::to_value(sidecar).map_err(Into::into)
    }

    /// Resolve a dependency-graph request at one cursor position.
    fn handle_dependency_graph_command(
        &self,
        backend: &Backend,
        request: &DocumentPositionRequest,
    ) -> Result<Value, LspBackendError> {
        let text = document_text(backend, &request.uri).ok_or(LspBackendError::DocumentNotFound)?;
        let chat_file = get_chat_file(backend, &request.uri, &text)?;
        let utterance = utils::find_utterance_at_position(&chat_file, request.position, &text)
            .ok_or(LspBackendError::NoUtteranceAtPosition)?;
        let response = crate::graph::build_dependency_graph_response(
            utterance,
            backend.parse_state(&request.uri),
        );
        serde_json::to_value(response).map_err(Into::into)
    }
}

/// Admission and execution failures have distinct protocol meanings. Neither
/// state can be serialized as a successful command payload by a feature service.
enum CommandFailure {
    Admission(LspBackendError),
    Execution {
        command: ExecuteCommandName,
        source: LspBackendError,
    },
}

impl CommandFailure {
    fn into_rpc(self) -> Error {
        match self {
            Self::Admission(source) => Error::invalid_params(source.to_string()),
            Self::Execution { command, source } => {
                let context = match command {
                    ExecuteCommandName::ShowDependencyGraph => "Dependency graph error",
                    ExecuteCommandName::GetAlignmentSidecar => "Sidecar error",
                    ExecuteCommandName::GetParticipants => "Participant error",
                    ExecuteCommandName::FormatIdLine | ExecuteCommandName::FormatBulletLine => {
                        "Format error"
                    }
                    ExecuteCommandName::GetSpeakers => "Speaker error",
                    ExecuteCommandName::FilterDocument => "Filter error",
                    ExecuteCommandName::GetUtterances => "Utterance error",
                    ExecuteCommandName::ScopedFind => "Search error",
                };
                Error {
                    code: ErrorCode::ServerError(-32001),
                    message: format!("{context}: {source}").into(),
                    data: None,
                }
            }
        }
    }
}

/// Decode and dispatch one `workspace/executeCommand` request. This is the only
/// wire projection: feature services return typed domain results all the way here.
pub(super) async fn handle_execute_command(
    backend: &Backend,
    params: ExecuteCommandParams,
) -> RpcResult<Option<Value>> {
    let request = ExecuteCommandRequest::parse(params)
        .map_err(|source| CommandFailure::Admission(source).into_rpc())?;
    let command = request.name();
    ExecuteCommandServices::new()
        .dispatch(backend, request)
        .map(Some)
        .map_err(|source| CommandFailure::Execution { command, source }.into_rpc())
}
