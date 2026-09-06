//! An analysis owns the exact source revision from which all its artifacts derive.

use std::sync::Arc;
use talkbank_model::model::{ChatFile, FileStem, TranscriptName};
use talkbank_model::{ErrorCollector, Severity};
use talkbank_parser::TreeSitterParser;
use tower_lsp::lsp_types::{Diagnostic, Url};
use tree_sitter::Tree;

use super::conversion::{to_diagnostics_batch, to_diagnostics_batch_with_context};
use super::text_diff::compute_input_edit;

/// One source revision, with either complete validation or parser diagnostics.
///
/// Construction always parses the owned source. Callers cannot install a tree,
/// model, or diagnostics independently, nor use a previous model's byte spans
/// with edited text. A partial model still serves healthy regions of its source.
pub(crate) struct DocumentAnalysis {
    source: Arc<str>,
    tree: Option<Tree>,
    file: Arc<ChatFile>,
    diagnostics: Vec<Diagnostic>,
    status: AnalysisStatus,
}

#[derive(Clone, Copy)]
enum AnalysisStatus {
    Parsed,
    Recovered,
}

impl DocumentAnalysis {
    /// Analyze a new source, applying its complete edit to the previous CST.
    ///
    /// The old source belongs to the old tree, so skipped debounce revisions
    /// cannot change the edit's baseline. AST lowering and validation run on the
    /// resulting source as a unit; unproven AST/header span reuse is excluded.
    pub(crate) fn parse(
        parser: &TreeSitterParser,
        uri: &Url,
        source: Arc<str>,
        previous: Option<&Self>,
    ) -> Self {
        let mut old_tree = previous.and_then(|old| old.tree.clone());
        if let (Some(old), Some(tree)) = (previous, old_tree.as_mut())
            && let Some(edit) = compute_input_edit(&old.source, &source)
        {
            tree.edit(&edit);
        }
        let sink = ErrorCollector::new();
        let (mut file, tree) =
            parser.parse_chat_file_streaming_incremental(&source, old_tree.as_ref(), &sink);
        let parse_errors = sink.into_vec();
        let (status, diagnostics) = if parse_errors
            .iter()
            .any(|error| error.severity == Severity::Error)
        {
            (
                AnalysisStatus::Recovered,
                to_diagnostics_batch(&parse_errors.iter().collect::<Vec<_>>(), &source),
            )
        } else {
            let name = uri
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .and_then(|filename| filename.strip_suffix(".cha"))
                .map_or(TranscriptName::Anonymous, |stem| {
                    TranscriptName::Named(FileStem::from_stem(stem))
                });
            let validation_errors = ErrorCollector::new();
            file.validate_with_alignment(&validation_errors, name);
            let errors = validation_errors.into_vec();
            (
                AnalysisStatus::Parsed,
                to_diagnostics_batch_with_context(
                    &errors.iter().collect::<Vec<_>>(),
                    &source,
                    Some(uri),
                    Some(&file),
                ),
            )
        };
        Self {
            source,
            tree,
            file: Arc::new(file),
            diagnostics,
            status,
        }
    }

    /// Admit reuse only for identical source bytes.
    pub(crate) fn for_source(&self, source: &str) -> Option<&Self> {
        (self.source.as_ref() == source).then_some(self)
    }

    pub(crate) fn tree(&self) -> Option<Tree> {
        self.tree.clone()
    }
    pub(crate) fn file(&self) -> Arc<ChatFile> {
        Arc::clone(&self.file)
    }
    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub(crate) fn is_parsed(&self) -> bool {
        matches!(self.status, AnalysisStatus::Parsed)
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
