//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::ErrorSink;
use crate::model;
use crate::node_types::{CONTINUATION, REST_OF_LINE};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse optional label text used by `@Bg`, `@Eg`, and `@G` headers.
pub(crate) fn parse_optional_gem_label(
    node: Option<Node>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let Some(node) = node else {
        return ParseOutcome::parsed(None);
    };
    let mut cursor = node.walk();
    let mut label = String::new();
    let mut saw_text = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            REST_OF_LINE => {
                if let Ok(text) = child.utf8_text(input.as_bytes())
                    && !text.is_empty()
                {
                    label.push_str(text);
                    saw_text = true;
                }
            }
            CONTINUATION => {
                if saw_text {
                    label.push(' ');
                }
            }
            _ => errors.report(unexpected_node_error(child, input, "gem label")),
        }
    }

    if label.is_empty() {
        ParseOutcome::parsed(None)
    } else {
        ParseOutcome::parsed(Some(model::GemLabel::new(label)))
    }
}
