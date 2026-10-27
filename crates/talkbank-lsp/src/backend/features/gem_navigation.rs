//! Source-bound editor projection of model-owned gem pairs.

use talkbank_model::model::{GemEvent, GemState};
use tower_lsp::lsp_types::{DocumentSymbol, FoldingRange, FoldingRangeKind, Range, SymbolKind};

use crate::backend::diagnostics::DocumentAnalysis;
use crate::backend::utils::LineIndex;

/// A paired, located gem with coordinates derived from its analysis source.
pub(super) struct GemNavigation {
    name: String,
    range: Range,
    folding_end_line: u32,
}

impl GemNavigation {
    pub(super) fn from_analysis(analysis: &DocumentAnalysis) -> Vec<Self> {
        let file = analysis.file();
        let document = analysis.source();
        let index = LineIndex::new(document);
        let mut state = GemState::new();
        file.headers_with_locations()
            .filter_map(|(header, location)| {
                let Some(GemEvent::Closed(pair)) = state.observe(header, location) else {
                    return None;
                };
                // Unlocated pairs remain semantic facts, but cannot name an editor range.
                let begin = pair.begin().span()?;
                let end = pair.end().span()?;
                let closing_text = document
                    .get(end.start()..end.end())?
                    .trim_end_matches(['\r', '\n']);
                let (last_character, _) = closing_text.char_indices().next_back()?;
                Some(Self {
                    name: pair
                        .label()
                        .map_or_else(|| "Gem".to_owned(), |label| format!("Gem: {label}")),
                    range: Range::new(
                        index.offset_to_position(begin.start()),
                        index.offset_to_position(end.end()),
                    ),
                    folding_end_line: index.offset_to_position(end.start() + last_character).line,
                })
            })
            .collect()
    }

    pub(super) fn into_symbol(self) -> DocumentSymbol {
        #[allow(deprecated)]
        DocumentSymbol {
            name: self.name,
            detail: None,
            kind: SymbolKind::EVENT,
            tags: None,
            deprecated: None,
            range: self.range,
            selection_range: Range::new(self.range.start, self.range.start),
            children: None,
        }
    }

    pub(super) fn into_fold(self) -> Option<FoldingRange> {
        (self.folding_end_line > self.range.start.line).then_some(FoldingRange {
            start_line: self.range.start.line,
            start_character: None,
            end_line: self.folding_end_line,
            end_character: None,
            kind: Some(FoldingRangeKind::Region),
            collapsed_text: None,
        })
    }
}
