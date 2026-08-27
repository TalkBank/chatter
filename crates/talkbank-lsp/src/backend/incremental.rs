//! Incremental parsing, re-parse only affected utterances on edits.
//!
//! CHAT files are line-structured: headers (`@Header:…`), then utterances
//! (main tier `*SPK:…` plus dependent tiers `%mor:…`). This module exploits
//! that structure so that a typical 1-3 line edit only re-parses the enclosing
//! utterance rather than the entire file (~100-1000× faster).
//!
//! # Architecture
//!
//! The incremental flow is a set of free functions over the cached source
//! text, parsed `ChatFile`, and tree-sitter CST owned by the backend. On
//! each `didChange`:
//!
//! 1. Apply the text edit and get tree-sitter's `changed_ranges`.
//! 2. [`collect_utterances_and_header_changes`] classifies whether a context-
//!    affecting header (Participants, Languages, Options, ID) was touched,
//!    requiring a full re-validate, or only decorative headers changed.
//! 3. [`detect_utterance_splice`] detects single utterance insertion / deletion
//!    for O(1) array splice instead of O(n) rebuild.
//! 4. [`affected_utterance_indices`] finds which existing utterances overlap the
//!    changed ranges and need re-parsing.
//! 5. [`collect_utterance_line_indices`] maps the surviving utterances back to
//!    their line positions in the document.

use talkbank_parser::DocumentRoot;
use talkbank_parser::generated_traversal::{AsRawNode, FromNodeKind, UtteranceNode};
use tree_sitter::{Range as TsRange, Tree};

use talkbank_model::model::ChatFile;

/// Returns whether header kind.
fn is_header_kind(kind: &str) -> bool {
    use talkbank_parser::node_types::{
        ACTIVITIES_HEADER, BCK_HEADER, BG_HEADER, BIRTH_OF_HEADER, BIRTHPLACE_OF_HEADER,
        BLANK_HEADER, COLOR_WORDS_HEADER, COMMENT_HEADER, DATE_HEADER, EG_HEADER, FONT_HEADER,
        G_HEADER, HEADER, ID_HEADER, L1_OF_HEADER, LANGUAGES_HEADER, LOCATION_HEADER, MEDIA_HEADER,
        NEW_EPISODE_HEADER, NUMBER_HEADER, OPTIONS_HEADER, PAGE_HEADER, PARTICIPANTS_HEADER,
        PID_HEADER, PRE_BEGIN_HEADER, RECORDING_QUALITY_HEADER, ROOM_LAYOUT_HEADER,
        SITUATION_HEADER, T_HEADER, TAPE_LOCATION_HEADER, TIME_DURATION_HEADER, TIME_START_HEADER,
        TRANSCRIBER_HEADER, TRANSCRIPTION_HEADER, TYPES_HEADER, VIDEOS_HEADER, WARNING_HEADER,
        WINDOW_HEADER,
    };

    matches!(
        kind,
        HEADER
            | PRE_BEGIN_HEADER
            | ACTIVITIES_HEADER
            | BCK_HEADER
            | BG_HEADER
            | BIRTH_OF_HEADER
            | BIRTHPLACE_OF_HEADER
            | BLANK_HEADER
            | COLOR_WORDS_HEADER
            | COMMENT_HEADER
            | DATE_HEADER
            | EG_HEADER
            | FONT_HEADER
            | G_HEADER
            | ID_HEADER
            | L1_OF_HEADER
            | LANGUAGES_HEADER
            | LOCATION_HEADER
            | MEDIA_HEADER
            | NEW_EPISODE_HEADER
            | NUMBER_HEADER
            | OPTIONS_HEADER
            | PAGE_HEADER
            | PARTICIPANTS_HEADER
            | PID_HEADER
            | RECORDING_QUALITY_HEADER
            | ROOM_LAYOUT_HEADER
            | SITUATION_HEADER
            | T_HEADER
            | TAPE_LOCATION_HEADER
            | TIME_DURATION_HEADER
            | TIME_START_HEADER
            | TRANSCRIBER_HEADER
            | TRANSCRIPTION_HEADER
            | TYPES_HEADER
            | VIDEOS_HEADER
            | WARNING_HEADER
            | WINDOW_HEADER
    )
}

/// Return whether two half-open byte ranges overlap.
fn ranges_overlap(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> bool {
    start_a < end_b && start_b < end_a
}

/// Whether a header kind affects validation context (participants, languages, options).
///
/// Changes to context-affecting headers require full validation context rebuild.
/// Changes to decorative headers (Comment, Date, Location, etc.) only need
/// header error re-validation, not utterance re-validation.
fn is_context_affecting_header(kind: &str) -> bool {
    use talkbank_parser::node_types::{
        ID_HEADER, LANGUAGES_HEADER, OPTIONS_HEADER, PARTICIPANTS_HEADER,
    };

    matches!(
        kind,
        PARTICIPANTS_HEADER | ID_HEADER | LANGUAGES_HEADER | OPTIONS_HEADER
    )
}

/// Collect utterance CST nodes in order and classify the header change.
pub fn collect_utterances_and_header_changes<'a>(
    tree: &'a Tree,
    changed_ranges: &[TsRange],
) -> (Vec<UtteranceNode<'a>>, HeaderChange) {
    // Typed, because this function is where "is this an utterance?" is DECIDED.
    // It used to push a bare `Node` after testing the kind, so every consumer
    // downstream held a node with no evidence of what it was and the LSP's
    // `parse_utterance_cst` had to take it on trust.
    let mut utterances: Vec<UtteranceNode<'a>> = Vec::new();
    // (start_byte, end_byte, context_affecting)
    let mut header_ranges: Vec<(usize, usize, bool)> = Vec::new();

    // The THIRD copy of this navigation until 2026-08-26, and the one that
    // searched all children rather than taking the first. `DocumentRoot` owns
    // it, and its `node()` is the same fallback-to-root this had, stated once.
    let doc_node = DocumentRoot::classify(tree).node();
    let mut cursor = doc_node.walk();
    for child in doc_node.children(&mut cursor) {
        if child.is_missing() || child.is_error() {
            continue;
        }

        if child.kind() == talkbank_parser::node_types::LINE {
            let mut line_cursor = child.walk();
            for line_child in child.children(&mut line_cursor) {
                if line_child.is_missing() || line_child.is_error() {
                    continue;
                }

                if let Some(utterance) = UtteranceNode::from_node(line_child) {
                    utterances.push(utterance);
                } else {
                    // Only the non-utterance branch needs the kind, so only it
                    // pays for reading it.
                    let kind = line_child.kind();
                    if is_header_kind(kind) {
                        header_ranges.push((
                            line_child.start_byte(),
                            line_child.end_byte(),
                            is_context_affecting_header(kind),
                        ));
                    }
                }
            }
        } else if is_header_kind(child.kind()) {
            header_ranges.push((
                child.start_byte(),
                child.end_byte(),
                is_context_affecting_header(child.kind()),
            ));
        }
    }

    let mut change = HeaderChange::None;
    for range in changed_ranges {
        let start = range.start_byte;
        let end = range.end_byte;
        for &(h_start, h_end, context_affecting) in &header_ranges {
            if ranges_overlap(h_start, h_end, start, end) {
                change = change.max(if context_affecting {
                    HeaderChange::ValidationContext
                } else {
                    HeaderChange::Decorative
                });
            }
        }
    }

    (utterances, change)
}

/// What kind of header changed, if any.
///
/// This was two `bool`s returned side by side, `context_header_changed` and
/// `any_header_changed`, with a doc paragraph explaining what they meant
/// TOGETHER. Context-affecting headers are a subset of headers, so
/// `(context: true, any: false)` describes nothing in the world and was
/// perfectly representable; the invariant lived in the assignment order of two
/// lines. As one ordered value the impossible reading has no spelling, and
/// `max` over the loop replaces the nested `if`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HeaderChange {
    /// No header overlapped a changed range.
    None,
    /// A header changed, but only one validation does not read for context
    /// (`@Comment`, `@Date`, `@Location`, ...).
    Decorative,
    /// A header validation reads for context changed (`@Participants`, `@ID`,
    /// `@Languages`, `@Options`), so cached per-utterance results are void.
    ValidationContext,
}

impl HeaderChange {
    /// Whether any header changed at all.
    #[must_use]
    pub fn is_none(self) -> bool {
        self == Self::None
    }

    /// Whether a header validation reads for context changed.
    #[must_use]
    pub fn affects_validation_context(self) -> bool {
        self == Self::ValidationContext
    }
}

/// Detect a single utterance insertion or deletion by comparing the new CST
/// utterance count against the old count and locating the splice point using
/// the byte position where the text edit begins.
///
/// `diff_start` is the first byte that differs between old and new text,
/// as computed by `compute_text_diff_span`.
///
/// Returns `Some((splice_idx, is_insertion))`:
/// - For insertion: `splice_idx` is the index in the **new** utterance array
/// - For deletion: `splice_idx` is the index in the **old** utterance array
///
/// Returns `None` if the count difference is not ±1.
pub fn detect_utterance_splice(
    utterance_nodes: &[UtteranceNode<'_>],
    diff_start: usize,
    old_utterance_count: usize,
) -> Option<(usize, bool)> {
    let new_count = utterance_nodes.len();
    let diff = new_count as i64 - old_utterance_count as i64;
    if diff.abs() != 1 {
        return None;
    }

    if diff == 1 {
        // Insertion: find the new utterance that contains or starts at diff_start.
        // Note: end_byte() is exclusive, so use strict < for the upper bound.
        for (i, node) in utterance_nodes.iter().enumerate() {
            let raw = node.raw_node();
            if raw.start_byte() <= diff_start && diff_start < raw.end_byte() {
                return Some((i, true));
            }
            if raw.start_byte() > diff_start {
                return Some((i, true));
            }
        }
        // Inserted at the very end
        Some((new_count - 1, true))
    } else {
        // Deletion: the splice point is the first gap in the new array at or
        // after diff_start, i.e., the index where the deleted utterance was.
        let idx = utterance_nodes
            .iter()
            .position(|n| n.raw_node().start_byte() >= diff_start)
            .unwrap_or(new_count);
        Some((idx, false))
    }
}

/// Find utterance indices whose CST nodes overlap changed ranges.
pub fn affected_utterance_indices<'a>(
    utterance_nodes: &[UtteranceNode<'a>],
    changed_ranges: &[TsRange],
) -> Vec<usize> {
    if changed_ranges.is_empty() {
        return Vec::new();
    }

    let mut indices = Vec::new();
    for (idx, node) in utterance_nodes.iter().enumerate() {
        let raw = node.raw_node();
        let (start, end) = (raw.start_byte(), raw.end_byte());
        if changed_ranges
            .iter()
            .any(|range| ranges_overlap(start, end, range.start_byte, range.end_byte))
        {
            indices.push(idx);
        }
    }
    indices
}

/// Collect line indices for utterances in a ChatFile (in utterance order).
pub fn collect_utterance_line_indices(chat_file: &ChatFile) -> Vec<usize> {
    let mut indices = Vec::new();
    for (idx, line) in chat_file.lines.iter().enumerate() {
        if matches!(line, talkbank_model::model::Line::Utterance(_)) {
            indices.push(idx);
        }
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute a tree-sitter InputEdit from an old and new text by finding the
    /// minimal differing region. This mirrors what the LSP does when it receives
    /// a `textDocument/didChange` notification and must edit the cached tree.
    fn compute_input_edit(old_text: &str, new_text: &str) -> tree_sitter::InputEdit {
        let start_byte = old_text
            .bytes()
            .zip(new_text.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(old_text.len().min(new_text.len()));

        let old_remaining = old_text.len() - start_byte;
        let new_remaining = new_text.len() - start_byte;
        let common_suffix = old_text
            .bytes()
            .rev()
            .zip(new_text.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count()
            .min(old_remaining)
            .min(new_remaining);

        let old_end_byte = old_text.len() - common_suffix;
        let new_end_byte = new_text.len() - common_suffix;

        tree_sitter::InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position: byte_to_point(old_text, start_byte),
            old_end_position: byte_to_point(old_text, old_end_byte),
            new_end_position: byte_to_point(new_text, new_end_byte),
        }
    }

    /// Convert byte offset in UTF-8 text to tree-sitter `(row, column)` point.
    fn byte_to_point(text: &str, byte: usize) -> tree_sitter::Point {
        let prefix = &text[..byte];
        let row = prefix.bytes().filter(|&b| b == b'\n').count();
        let col = prefix.len() - prefix.rfind('\n').map(|p| p + 1).unwrap_or(0);
        tree_sitter::Point { row, column: col }
    }

    /// Parse old_text, apply the edit, parse new_text incrementally,
    /// and return (old_tree, new_tree) with correct changed_ranges.
    fn incremental_parse(old_text: &str, new_text: &str) -> (tree_sitter::Tree, tree_sitter::Tree) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_talkbank::LANGUAGE.into())
            .expect("failed to set tree-sitter language");

        let mut old_tree = parser
            .parse(old_text, None)
            .expect("failed to parse old_text");

        let edit = compute_input_edit(old_text, new_text);
        old_tree.edit(&edit);

        let new_tree = parser
            .parse(new_text, Some(&old_tree))
            .expect("failed to parse new_text incrementally");

        (old_tree, new_tree)
    }

    /// Find the first byte position where old and new text differ.
    fn compute_diff_start(old_text: &str, new_text: &str) -> usize {
        old_text
            .bytes()
            .zip(new_text.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(old_text.len().min(new_text.len()))
    }

    /// Minimal valid CHAT preamble for splice tests.
    const PREAMBLE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n";

    #[test]
    fn test_detect_splice_insertion_at_end() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _) = collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 2);
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, Some((1, true))); // insertion at index 1
    }

    #[test]
    fn test_detect_splice_insertion_at_front() {
        let old_text = &format!("{PREAMBLE}*CHI:\tworld .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _) = collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 2);
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, Some((0, true))); // insertion at index 0
    }

    #[test]
    fn test_detect_splice_deletion() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _) = collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 1);
        let result = detect_utterance_splice(&new_utterances, diff_start, 2);
        assert_eq!(result, Some((1, false))); // deletion at old index 1
    }

    #[test]
    fn test_detect_splice_count_diff_too_large() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\ta .\n*CHI:\tb .\n*CHI:\tc .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _) = collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        // Count diff is +2, not ±1, should return None
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, None);
    }
}
