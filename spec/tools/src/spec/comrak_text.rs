//! Reading text back out of a comrak AST, once.
//!
//! # Why this module exists
//!
//! Three spec parsers each carried their own copy of these three functions:
//! [`markdown`](super::markdown) for `spec/constructs/`, [`error`](super::error)
//! and the since-deleted `error_corpus`, both for `spec/errors/`. Across those
//! three, `normalize_whitespace` and `strip_single_trailing_newline` were
//! byte-identical, and `extract_text_from_children` differed only in whitespace
//! and in whether it spelled `comrak::nodes::` out.
//!
//! The counts and the divergence they tracked are in the next section; they
//! shrank twice in one day (Phase 1b's sweep, then R4's deletion), which is
//! why they are stated ONCE there with their command, not here as well.
//!
//! # The copies that are NOT here any more
//!
//! `bin/fix_spec_layers.rs` carried a DIFFERENT `extract_text_from_children`
//! under the same name (Text only: no `Code`, no `SoftBreak`) plus its own
//! `normalize_whitespace`, and this header spent five paragraphs recording the
//! divergence because the binary wrote into the source of truth while reading
//! it differently from the loaders. R4 deleted the binary (2026-08-21), which
//! was always the fix the record was waiting for. Measured after the
//! deletion: ONE `extract_text_from_children` (this one) and TWO
//! `normalize_whitespace` (this one, and a structurally different one in
//! `generate_error_words`), counted with `rg -n 'fn <name>'` across both
//! workspaces.

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

/// The VERBATIM markdown of the `## <heading>` section, or `None` if the file
/// has no such section.
///
/// # Why source bytes and not [`extract_text_from_children`]
///
/// A section like `## Description` is prose written in markdown, and it is
/// republished as markdown on the code's page under `docs/errors/`. Reading it
/// back through the text extractor loses exactly the things an author put
/// there on purpose: `` `backticks` `` become bare words, a link keeps its text
/// and drops its URL, and emphasis vanishes. Taking the source is the only
/// reading that survives the round trip it is about to make.
///
/// It also fixes what the two parsers did with a MULTI-paragraph section.
/// `error.rs` took the first paragraph and dropped the rest, silently: measured
/// 2026-08-18, 51 of the 236 specs have more than one, so 51 published pages
/// ended mid-thought. E202's second paragraph is the pointer to where the valid
/// form types are actually declared, which is precisely what a maintainer needs
/// and precisely what was cut. `error_corpus.rs` joined them all with a space,
/// which is a different wrong answer: it flattens paragraph breaks, and the six
/// specs whose description contains a LIST would have had their bullets run
/// together into one line.
///
/// Trailing and leading blank lines are trimmed; everything between is exact.
pub(crate) fn section_source<'a>(
    content: &str,
    root: &'a AstNode<'a>,
    heading: &str,
) -> Option<String> {
    // Top-level children only. `descendants()` would also visit the paragraphs
    // nested inside a list item, which is how a "paragraph" count comes to
    // disagree with what a reader sees.
    let mut start_line: Option<usize> = None;
    let mut end_line: Option<usize> = None;

    for node in root.children() {
        let data = node.data.borrow();
        let NodeValue::Heading(ref h) = data.value else {
            continue;
        };
        if h.level > 2 {
            continue;
        }
        if start_line.is_none() {
            if normalize_whitespace(&extract_text_from_children(node)) == heading {
                start_line = Some(data.sourcepos.end.line + 1);
            }
        } else {
            // The next heading at or above section level closes the section,
            // and there is nothing left to look for.
            end_line = Some(data.sourcepos.start.line.saturating_sub(1));
            break;
        }
    }

    let first = start_line?;
    let lines: Vec<&str> = content.lines().collect();
    let last = match end_line {
        // A heading's own start line is a real line of `content`, so
        // `start.line - 1` is at most `lines.len()`.
        Some(line) => line,
        // No following heading: the section runs to the end of the file.
        None => lines.len(),
    };
    // Both arms are bounded by `lines.len()`, so this covers the slice below.
    if first > last {
        return Some(String::new());
    }

    let body = lines[first - 1..last].join("\n");
    Some(body.trim_matches('\n').trim_end().to_string())
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
