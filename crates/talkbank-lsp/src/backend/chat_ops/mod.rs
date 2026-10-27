//! CHAT document operations for the LSP.
//!
//! Provides execute-command handlers that replace ad-hoc string/regex parsing
//! in the TypeScript extension with proper model-based operations:
//!
//! - `talkbank/getSpeakers`, extract declared speaker codes from `@Participants`
//! - `talkbank/filterDocument`, filter a document to selected speakers
//! - `talkbank/getUtterances`, list utterances with speaker, line, and coded status
//! - `talkbank/formatBulletLine`, construct a timing bullet and new utterance line
//! - `talkbank/scopedFind`, semantically scoped search across tiers and speakers

use super::LspBackendError;
use serde_json::Value;

use super::execute_commands::ChatOpsCommandRequest;
use super::state::Backend;

mod filter_document;
mod format_bullet;
mod scoped_find;
mod shared;
mod speakers;
#[cfg(test)]
mod tests;
mod utterances;

pub(crate) use filter_document::handle_filter_document;
pub(crate) use format_bullet::handle_format_bullet_line;
pub(crate) use scoped_find::handle_scoped_find;
pub(crate) use speakers::handle_get_speakers;
pub(crate) use utterances::handle_get_utterances;

/// Feature-oriented execute-command service for CHAT document operations.
pub(crate) struct ChatOpsCommandService;

impl ChatOpsCommandService {
    /// Dispatch one chat-ops-family execute-command request.
    pub(crate) fn dispatch(
        &self,
        backend: &Backend,
        request: ChatOpsCommandRequest,
    ) -> Result<Value, LspBackendError> {
        match request {
            ChatOpsCommandRequest::GetSpeakers(request) => handle_get_speakers(backend, &request),
            ChatOpsCommandRequest::FilterDocument(request) => {
                handle_filter_document(backend, &request)
            }
            ChatOpsCommandRequest::GetUtterances(request) => {
                handle_get_utterances(backend, &request)
            }
            ChatOpsCommandRequest::FormatBulletLine(request) => handle_format_bullet_line(&request),
            ChatOpsCommandRequest::ScopedFind(request) => handle_scoped_find(backend, &request),
        }
    }
}
