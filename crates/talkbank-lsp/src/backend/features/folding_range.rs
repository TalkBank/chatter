//! Folding range provider for CHAT files.
//!
//! Creates fold regions so annotators can collapse the annotation layer
//! (`%mor:`, `%gra:`, `%pho:`, … dependent tiers) and focus on transcription.
//!
//! Each fold corresponds to one utterance *block*:
//! - `startLine` = the `*SPEAKER:` main-tier line.
//! - `endLine`   = the last `%xxx:` dependent-tier line of that utterance.
//!
//! An utterance with no dependent tiers does not produce a fold (a single-line
//! fold adds no value to the editor UI).
//!
//! Optionally, the header block (@Begin … first utterance) is folded as well,
//! using a separate `FoldingRangeKind::Region` marker.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::Span;
use tower_lsp::lsp_types::*;

use super::gem_navigation::GemNavigation;
use crate::backend::diagnostics::DocumentAnalysis;
use crate::backend::utils::LineIndex;

/// Build the list of folding ranges for a CHAT file.
///
/// The analysis owns both the parsed model and its exact source text.
///
/// # Returns
/// A list of `FoldingRange` values, one per utterance block that spans more
/// than one line (i.e., has at least one dependent tier), plus an optional
/// header-block fold if a header section exists.
pub(crate) fn folding_range(analysis: &DocumentAnalysis) -> Vec<FoldingRange> {
    let chat_file = analysis.file();
    let document = analysis.source();
    let mut ranges: Vec<FoldingRange> = Vec::new();
    let index = LineIndex::new(document);

    // Track the byte offset of the first utterance for the header fold.
    let mut first_utterance_start: Option<usize> = None;

    for utterance in chat_file.utterances() {
        // An utterance with no source location has nothing to fold.
        let Some(main_span) = utterance.main.location.span() else {
            continue;
        };
        let main_start = main_span.start();

        if first_utterance_start.is_none() {
            first_utterance_start = Some(main_start);
        }

        // Only fold if the utterance has at least one dependent tier.
        if utterance.dependent_tiers.is_empty() {
            continue;
        }

        let main_line = index.offset_to_position(main_start).line;
        let block_end = utterance_block_end(utterance, main_span);
        let end_line = index.offset_to_position(block_end).line;

        if end_line > main_line {
            ranges.push(FoldingRange {
                start_line: main_line,
                start_character: None,
                end_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    // Header block fold: line 0 → line before first utterance.
    if let Some(first_start) = first_utterance_start {
        let first_utt_line = index.offset_to_position(first_start).line;
        if first_utt_line > 1 {
            ranges.push(FoldingRange {
                start_line: 0,
                start_character: None,
                end_line: first_utt_line.saturating_sub(1),
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    ranges.extend(
        GemNavigation::from_analysis(analysis)
            .into_iter()
            .filter_map(GemNavigation::into_fold),
    );

    ranges
}

/// Returns the byte offset of the end of an utterance block (main tier +
/// all dependent tiers). Falls back to the main tier end when no dependent
/// tiers are present.
fn utterance_block_end(utterance: &talkbank_model::model::Utterance, main_span: Span) -> usize {
    utterance
        .dependent_tiers
        .iter()
        .filter_map(|t| t.tier.location().span())
        .map(|span| span.end())
        .max()
        .unwrap_or(main_span.end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::document_analysis;
    const BASIC: &str = include_str!("../../../../../corpus/reference/core/basic-conversation.cha");
    const MOR_GRA: &str = include_str!("../../../../../corpus/reference/tiers/mor-gra.cha");
    const GEMS: &str =
        include_str!("../../../../../corpus/reference/edge-cases/multiline-paired-gems.cha");

    #[test]
    fn fold_utterance_with_dependent_tiers() {
        let input = MOR_GRA;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let ranges = folding_range(&analysis);

        // Should have at least one utterance fold and a header fold
        assert!(ranges.len() >= 2);

        // Find the utterance fold (starts at the *CHI: line)
        let utt_fold = ranges.iter().find(|r| r.start_line == 11).unwrap();
        assert!(utt_fold.end_line > utt_fold.start_line);
    }

    #[test]
    fn no_fold_for_single_line_utterance() {
        let input = BASIC;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let ranges = folding_range(&analysis);

        // No utterance fold, only header fold (if any)
        assert!(ranges.iter().all(|r| r.start_line == 0));
    }

    #[test]
    fn header_block_fold() {
        let input = BASIC;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let ranges = folding_range(&analysis);

        // Header fold starts at line 0
        let header_fold = ranges.iter().find(|r| r.start_line == 0);
        assert!(header_fold.is_some());
        assert!(header_fold.unwrap().end_line >= 3);
    }

    #[test]
    fn gem_block_fold() {
        let input = GEMS;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let ranges = folding_range(&analysis);

        // Should have a gem fold from @Bg to @Eg
        let gem_fold = ranges.iter().find(|r| r.start_line == 6).unwrap();
        assert_eq!(gem_fold.end_line, 11);
    }
}
