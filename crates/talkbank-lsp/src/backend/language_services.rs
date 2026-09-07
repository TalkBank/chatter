//! Thread-local parser and semantic-token services for the LSP backend.
//!
//! The backend previously shared a single parser and semantic-tokens provider
//! behind global mutexes. Both resources are inherently thread-confined
//! (`tree_sitter::Parser` is not `Sync`, and the highlighter keeps mutable
//! internal state), so this module replaces those mutexes with lazily
//! initialized thread-local instances and a small explicit service API.

use std::cell::{OnceCell, RefCell};

use talkbank_parser::TreeSitterParser;

use crate::semantic_tokens::SemanticTokensProvider;

use super::LspBackendError;
use super::state::BackendInitError;

thread_local! {
    /// Thread-local tree-sitter parser used by synchronous parser entry points.
    static CHAT_PARSER: OnceCell<Result<TreeSitterParser, BackendInitError>> = const {
        OnceCell::new()
    };
    /// Thread-local semantic-tokens provider used by token requests.
    static SEMANTIC_TOKENS: OnceCell<Result<RefCell<SemanticTokensProvider>, BackendInitError>> = const {
        OnceCell::new()
    };
}

/// Lazily initialized thread-local language services used by the LSP backend.
#[derive(Clone, Default)]
pub(crate) struct LanguageServices;

impl LanguageServices {
    /// Create a new language-services façade.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Execute a closure with the current thread's CHAT parser.
    pub(crate) fn with_parser<T>(
        &self,
        callback: impl FnOnce(&TreeSitterParser) -> T,
    ) -> Result<T, BackendInitError> {
        CHAT_PARSER.with(|slot| {
            match slot.get_or_init(|| {
                TreeSitterParser::new()
                    .map_err(|error| BackendInitError::Parser(format!("{error:?}")))
            }) {
                Ok(parser) => Ok(callback(parser)),
                Err(error) => Err(error.clone()),
            }
        })
    }

    /// Execute a closure with the current thread's semantic-tokens provider.
    ///
    /// The closure returns the typed `LspBackendError` so call sites can
    /// `match` on variants. The init-failure branch surfaces through the
    /// same error type via [`LspBackendError::HighlightFailed`] so callers
    /// see a single error surface regardless of whether the failure came
    /// from grammar init or token extraction.
    pub(crate) fn with_semantic_tokens_provider<T>(
        &self,
        callback: impl FnOnce(&mut SemanticTokensProvider) -> Result<T, LspBackendError>,
    ) -> Result<T, LspBackendError> {
        SEMANTIC_TOKENS.with(|slot| {
            match slot.get_or_init(|| {
                SemanticTokensProvider::new()
                    .map(RefCell::new)
                    .map_err(|error| BackendInitError::SemanticTokens(error.to_string()))
            }) {
                Ok(provider) => {
                    let mut provider = provider.try_borrow_mut().map_err(|_| {
                        LspBackendError::HighlightFailed {
                            reason: "semantic-token service is already in use on this thread"
                                .into(),
                        }
                    })?;
                    callback(&mut provider)
                }
                Err(init_error) => Err(LspBackendError::HighlightFailed {
                    reason: init_error.to_string(),
                }),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for thread-local language services.

    use super::LanguageServices;

    /// Parser access should succeed repeatedly on the same thread.
    #[test]
    fn parser_service_parses_repeatedly() {
        let services = LanguageServices::new();
        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";

        let first = services
            .with_parser(|parser| parser.parse_chat_file(text).is_built())
            .expect("parser should initialize");
        let second = services
            .with_parser(|parser| parser.parse_chat_file(text).is_built())
            .expect("parser should remain available");

        assert!(first);
        assert!(second);
    }

    /// Semantic-token access should succeed repeatedly on the same thread.
    #[test]
    fn semantic_tokens_service_highlights_repeatedly() {
        let services = LanguageServices::new();
        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";

        let first = services
            .with_semantic_tokens_provider(|provider| provider.semantic_tokens_full(text))
            .expect("semantic-tokens provider should initialize");
        let second = services
            .with_semantic_tokens_provider(|provider| {
                let nested = services
                    .with_semantic_tokens_provider(|nested| nested.semantic_tokens_full(text));
                assert!(nested.is_err(), "nested access must fail without panicking");
                provider.semantic_tokens_range(text, 0, text.len())
            })
            .expect("semantic-tokens provider should remain available");

        assert!(!first.is_empty());
        assert!(!second.is_empty());
    }
}
