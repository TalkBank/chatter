//! Reading text back out of a comrak AST, once.
//!
//! # Why this module exists
//!
//! Three spec parsers each carried their own copy of these three functions:
//! [`markdown`](super::markdown) for `spec/constructs/`, [`error`](super::error)
//! and [`error_corpus`](super::error_corpus) for `spec/errors/`. Across those
//! three, `normalize_whitespace` and `strip_single_trailing_newline` were
//! byte-identical, and `extract_text_from_children` differed only in whitespace
//! and in whether it spelled `comrak::nodes::` out.
//!
//! Crate-wide the counts are higher and the copies are NOT all equivalent, which
//! is the point of the next section: five `extract_text_from_children` in two
//! behaviours, and five `normalize_whitespace` in two behaviours.
//!
//! # The copies that are NOT here, and what they prove
//!
//! Two more copies of `extract_text_from_children` live in
//! `bin/enhance_specs.rs` and `bin/fix_spec_layers.rs`, and they are a DIFFERENT
//! FUNCTION wearing the same name:
//!
//! ```text
//! if let NodeValue::Text(text) = &child.data.borrow().value { result.push_str(text); }
//! ```
//!
//! Text only. No `Code`, so a metadata value written with backticks reads as
//! empty; no `SoftBreak`, so a value wrapped across two source lines loses its
//! word boundary. Those two binaries are the reverse tools that WRITE into
//! `spec/errors/`, so the readers and the writers of the source of truth do not
//! agree on what a metadata line says.
//!
//! Their `normalize_whitespace` differs too, and less visibly: it collapses runs
//! character by character and then trims only the TRAILING side, where
//! `split_whitespace().join(" ")` trims both. A leading space survives one and
//! not the other.
//!
//! They are deliberately NOT converted here. Pointing them at this module would
//! change what they read and therefore what they write, which is a corpus-level
//! behaviour change in tools the spec-system redesign plans to delete outright
//! (its rules R4 and R5). Recording the divergence is the useful half; the fix
//! is the deletion.
//!
//! That reason covers the reader-versus-writer divergence and NOT a second thing
//! in the same two files: they are near-duplicates OF EACH OTHER. Seven
//! functions are byte-identical between them, about 65 lines, including the two
//! named above plus `display_filename`, `extract_metadata_value` and
//! `parse_value_after_separator`. Factoring those into a shared module is
//! behaviour-preserving and this module's stated reason does not excuse it; it
//! is left undone because R4 and R5 delete both binaries, which is a different
//! argument and is written here so the next reader does not have to reconstruct
//! which one applies.

use comrak::nodes::{AstNode, NodeValue};

/// Concatenate the text a node spans, as the spec format means it.
///
/// Inline code counts as text, because a metadata value may legitimately be
/// written `` `implemented` ``; a soft or hard break becomes one space, because
/// a value wrapped across source lines is still one value.
pub(crate) fn extract_text_from_children<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        match child.data.borrow().value {
            NodeValue::Text(ref text) => result.push_str(text),
            NodeValue::Code(ref code) => result.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => result.push(' '),
            _ => {}
        }
    }
    result
}

/// Collapse every run of whitespace to a single space and trim the ends.
pub(crate) fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove at most one trailing newline (`\n` or `\r\n`) from code block content.
///
/// At most ONE: comrak's fenced-block literal ends with the newline that closes
/// the last content line, and a spec whose example deliberately ends in a blank
/// line means it.
pub(crate) fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}
