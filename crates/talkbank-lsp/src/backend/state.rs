//! Shared backend state for document caches, language services, and validation bookkeeping.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tower_lsp::Client;
use tower_lsp::lsp_types::Url;

use super::diagnostics::DocumentAnalysis;
use super::language_services::LanguageServices;

/// Initialization failures for backend subsystems.
#[derive(Clone, Debug, thiserror::Error)]
pub enum BackendInitError {
    /// Tree-sitter parser failed to initialize.
    #[error("failed to initialize tree-sitter parser: {0}")]
    Parser(String),
    /// Semantic tokens provider failed to initialize.
    #[error("failed to initialize semantic tokens provider: {0}")]
    SemanticTokens(String),
}

/// Mutable server-wide state shared across LSP request/notification handlers.
#[derive(Clone)]
pub struct Backend {
    /// LSP client for sending notifications
    pub client: Client,

    /// Cache of document contents (URI -> text)
    pub(crate) documents: Arc<DashMap<Url, Arc<super::documents::OpenDocument>>>,

    /// Thread-local parser and semantic-token services.
    pub(crate) language_services: LanguageServices,

    /// Pending validation IDs per document (for debouncing)
    pub pending_validations: Arc<DashMap<Url, u64>>,

    /// Counter for generating unique validation IDs
    pub validation_counter: Arc<AtomicU64>,

    /// Atomically replaced source-bound syntax, model, and diagnostic snapshots.
    pub(crate) analyses: Arc<DashMap<Url, Arc<DocumentAnalysis>>>,
}

/// Informational status of the backend's cached analysis.
///
/// `StaleBaseline` means analysis is pending or the last parse recovered errors.
/// Feature requests still parse their exact current source; this classification
/// only controls the existing caution annotation, never stale-model admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseState {
    /// Last parse succeeded; baseline is authoritative.
    Clean,
    /// Current analysis is pending or contains parser recovery.
    StaleBaseline,
    /// No baseline exists.
    Absent,
}

impl Backend {
    /// Create a backend with initialized language services and empty caches.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(DashMap::new()),
            language_services: LanguageServices::new(),
            pending_validations: Arc::new(DashMap::new()),
            validation_counter: Arc::new(AtomicU64::new(0)),
            analyses: Arc::new(DashMap::new()),
        }
    }

    /// Classify the current cache for informational feature annotations.
    /// Feature reads independently require an exact source match.
    pub fn parse_state(&self, uri: &Url) -> ParseState {
        let Some(source) = self.documents.get(uri) else {
            return ParseState::Absent;
        };
        let Some(analysis) = self.analyses.get(uri) else {
            return ParseState::Absent;
        };
        if analysis.for_source(&source.source).is_some() && analysis.is_parsed() {
            ParseState::Clean
        } else {
            ParseState::StaleBaseline
        }
    }
}
