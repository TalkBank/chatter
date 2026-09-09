//! Shared test-fixture helpers for the LSP crate.
//!
//! Before this module existed, ~15 `#[cfg(test)] mod tests` blocks each
//! redefined their own `parse_chat` / `parse_tree` /
//! `parse_chat_with_alignments` helpers with identical bodies. That
//! pattern is banned going forward, add a helper here and call it from
//! the test module.
//!
//! Two distinct tree-sitter builders exist and both are still
//! needed: [`parse_tree`] spins up a fresh `tree_sitter::Parser`
//! directly (the common case, fine when the test does not care about
//! incremental edit semantics), and [`parse_tree_incremental`]
//! routes through [`TreeSitterParser`] the way the live backend does
//! (for tests that exercise incremental-parse behavior). They return
//! the same `Tree` shape but exercise different code paths.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{ChatFile, Line, TranscriptName, Utterance};
use talkbank_parser::{ParseProduct, TreeSitterParser};
use tree_sitter::Tree;

/// Parse a CHAT source string into a `ChatFile`, panicking on failure.
///
/// Callers get a sharp panic (`ParseProduct::expect_built`) when no model
/// could be built, because a test that can't parse its own fixture has
/// nothing useful to say.
pub(crate) fn parse_chat(content: &str) -> ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    parser.parse_chat_file(content).expect_built()
}

/// [`parse_chat`] + compute per-utterance alignment metadata.
///
/// Default `parse_chat` leaves `utterance.alignments = None`. Tests
/// that need main↔mor / %mor↔%gra / main↔%pho / main↔%sin /
/// main↔`WorTimingSidecar` populated must call this variant so
/// `AlignmentSet` is built on each utterance.
pub(crate) fn parse_chat_with_alignments(content: &str) -> ChatFile {
    let mut chat_file = parse_chat(content);
    compute_all_alignments(&mut chat_file);
    chat_file
}

/// Compute every utterance's alignment metadata in place, as
/// `ChatFile::validate_with_alignment` does before validating.
fn compute_all_alignments(chat_file: &mut ChatFile) {
    for line in &mut chat_file.lines {
        if let Line::Utterance(utterance) = line {
            utterance.compute_alignments_default();
        }
    }
}

/// Parse `content`, having first checked that it reports nothing at all: no
/// parse diagnostic, and no validation diagnostic of any severity under the
/// default rules with alignment computed and no transcript name. That is
/// stricter than `chatter validate`'s verdict, which calls a warning-only
/// file valid; a fixture here is refused for a warning too, so a test that
/// means to exercise a warning says so by not using this helper. A feature
/// is never exercised over a transcript the validator has anything to say
/// about.
///
/// The returned file is the fresh parse: no alignment computed and no
/// validator run over it, which is the state the backend's own parse leaves
/// an utterance in before analysis. [`valid_chat_with_alignments`] is the
/// state after.
pub(crate) fn valid_chat(content: &str) -> Result<ChatFile, String> {
    let parser = TreeSitterParser::new().map_err(|err| format!("parser init: {err}"))?;
    let file = match parser.parse_chat_file(content) {
        ParseProduct::Built { file, diagnostics } if diagnostics.is_empty() => file,
        ParseProduct::Built { diagnostics, .. } | ParseProduct::Unbuildable { diagnostics } => {
            return Err(format!(
                "fixture {content:?} did not parse cleanly: {diagnostics:?}"
            ));
        }
    };
    let verdict = ErrorCollector::new();
    let mut checked = file.clone();
    checked.validate_with_alignment(&verdict, TranscriptName::Anonymous);
    let reported = verdict.into_vec();
    if !reported.is_empty() {
        return Err(format!(
            "fixture {content:?} is not valid CHAT: {}",
            reported
                .iter()
                .map(|e| format!("{} {}", e.code.as_str(), e.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    Ok(file)
}

/// [`valid_chat`] with every utterance's alignment metadata computed.
pub(crate) fn valid_chat_with_alignments(content: &str) -> Result<ChatFile, String> {
    let mut chat_file = valid_chat(content)?;
    compute_all_alignments(&mut chat_file);
    Ok(chat_file)
}

/// The first utterance of a parsed file, for a fixture written around one.
pub(crate) fn first_utterance(chat_file: ChatFile) -> Result<Utterance, String> {
    chat_file
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(*utterance),
            _ => None,
        })
        .ok_or_else(|| "the fixture built no utterance".to_string())
}

/// Build a `tree_sitter::Tree` via a direct `tree_sitter::Parser`.
///
/// Use when the test only needs a parsed syntax tree (for handlers
/// that walk the CST) and does not care which path the backend
/// would have taken. For tests that specifically exercise
/// incremental-parse behavior, use [`parse_tree_incremental`].
pub(crate) fn parse_tree(input: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_talkbank::LANGUAGE;
    parser.set_language(&language.into()).unwrap();
    parser.parse(input, None).unwrap()
}

/// Build a `tree_sitter::Tree` through [`TreeSitterParser`], the
/// same entry point the LSP backend uses.
///
/// Prefer this over [`parse_tree`] when the test exercises the
/// incremental-parse code path (e.g. `references`, `rename`).
pub(crate) fn parse_tree_incremental(input: &str) -> Tree {
    let parser = TreeSitterParser::new().unwrap();
    parser.parse_tree_incremental(input, None).unwrap()
}
