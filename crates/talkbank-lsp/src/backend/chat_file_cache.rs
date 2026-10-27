//! Single source of truth for "load a parsed `ChatFile` for a URI".
//!
//! All feature handlers that need a parsed document route through
//! `load_chat_file`; those that also need the document text use
//! `load_document_and_chat_file`. Adding a second copy of this
//! cache-then-reparse shape is banned, extend the helpers here
//! instead. The chat_ops submodule re-exports the pair-variant under
//! the name `get_document_and_chat_file` for historical uniformity.

use std::sync::Arc;

use talkbank_model::ParseError;
use talkbank_model::model::ChatFile;
use tower_lsp::lsp_types::Url;

use crate::backend::LspBackendError;
use crate::backend::documents;
use crate::backend::state::Backend;

/// Return a parsed [`ChatFile`] for `uri`, reparsing `doc` on cache miss.
///
/// Cached models are admitted only for identical source. During debounce or
/// recovery, reparsing the requested source preserves current byte positions.
///
/// A [`talkbank_parser::ParseProduct::Built`] is returned as `Ok` even
/// when it carries diagnostics: per this crate's degrade-gracefully
/// doctrine, a document with one bad region should still serve feature
/// requests (hover, completion, alignment) for the healthy rest of the
/// file, rather than failing the whole request. Only
/// [`talkbank_parser::ParseProduct::Unbuildable`] maps to
/// [`LspBackendError::ParseFailure`].
pub(crate) fn load_chat_file(
    backend: &Backend,
    uri: &Url,
    doc: &str,
) -> Result<Arc<ChatFile>, LspBackendError> {
    if let Some(cached) = backend.analyses.get(uri)
        && let Some(current) = cached.for_source(doc)
    {
        return Ok(current.file());
    }

    match backend
        .language_services
        .with_parser(|parser| parser.parse_chat_file(doc))
    {
        Ok(talkbank_parser::ParseProduct::Built { file, .. }) => Ok(Arc::new(file)),
        Ok(talkbank_parser::ParseProduct::Unbuildable { diagnostics }) => {
            Err(parse_failure_from(&diagnostics))
        }
        Err(init_error) => Err(LspBackendError::LanguageServicesUnavailable(init_error)),
    }
}

/// Load both the current document text and its parsed [`ChatFile`].
///
/// [`LspBackendError::DocumentNotFound`] fires when no cached text
/// exists for the URI (most commonly, the client never opened the
/// file). [`load_chat_file`] supplies the other failure variants.
pub(crate) fn load_document_and_chat_file(
    backend: &Backend,
    uri: &Url,
) -> Result<(String, Arc<ChatFile>), LspBackendError> {
    let text =
        documents::get_document_text(backend, uri).ok_or(LspBackendError::DocumentNotFound)?;
    let chat_file = load_chat_file(backend, uri, &text)?;
    Ok((text, chat_file))
}

/// Convert a [`talkbank_parser::ParseProduct::Unbuildable`]'s diagnostics
/// into [`LspBackendError::ParseFailure`].
///
/// Kept private: every call that needs a [`ChatFile`] should route
/// through [`load_chat_file`] rather than recreate the error-mapping
/// shape. New code that needs a different classification should add
/// a variant to [`LspBackendError`], not a second copy of this
/// function.
fn parse_failure_from(diagnostics: &[ParseError]) -> LspBackendError {
    LspBackendError::ParseFailure {
        count: diagnostics.len(),
        first_message: diagnostics.first().map(|e| e.message.clone()),
    }
}

/// Run a feature against one current source-owned analysis. The callback cannot
/// receive an independently supplied model and source. A transient parse during
/// debounce does not replace the authoritative analysis baseline.
pub(crate) fn with_current_analysis<T>(
    backend: &Backend,
    uri: &Url,
    feature: impl FnOnce(&crate::backend::diagnostics::DocumentAnalysis) -> T,
) -> Result<Option<T>, LspBackendError> {
    let Some(source) = documents::get_document_text(backend, uri) else {
        return Ok(None);
    };
    if let Some(cached) = backend.analyses.get(uri)
        && let Some(current) = cached.for_source(&source)
    {
        return Ok(Some(feature(current)));
    }
    let analysis = backend
        .language_services
        .with_parser(|parser| {
            crate::backend::diagnostics::DocumentAnalysis::parse(
                parser,
                uri,
                Arc::from(source),
                None,
            )
        })
        .map_err(LspBackendError::LanguageServicesUnavailable)?;
    Ok(Some(feature(&analysis)))
}
