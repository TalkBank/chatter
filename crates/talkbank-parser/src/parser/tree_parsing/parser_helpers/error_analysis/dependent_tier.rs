//! Error analysis specialized for dependent-tier failures.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Classifies one dependent-tier error node with optional tier context.
pub(crate) fn analyze_dependent_tier_error_with_context(
    error_node: Node,
    source: &str,
    tier_type: Option<&str>,
) -> ParseError {
    let start = error_node.start_byte();
    let end = error_node.end_byte();
    let error_text = match error_node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(_) => {
            return ParseError::new(
                ErrorCode::InvalidControlCharacter,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, ""),
                "Could not decode dependent tier content as valid UTF-8",
            )
            .with_suggestion("Re-enter using Unicode standard characters");
        }
    };

    // There is no `%gra:` branch here. One fired on that substring anywhere
    // in the ERROR's text until 2026-09-08 and called it E710, "non-numeric
    // index", on an `%eng` body that mentioned `%gra:` and on junk after a
    // well-formed relation alike; this parser's one producer of E710 is the
    // typed relation parser (`tier_parsers/gra/relation.rs`), which knows a
    // head field when it has one (the re2c backend has its own). A `%gra`
    // recovery node is the generic E316 below.

    // E760: %mor item with an EMPTY part-of-speech field (`|we`). More
    // specific than the missing-pipe case below: the pipe is present but
    // the field before it is empty, which is never meaningful %mor
    // content (modern reading of CLAN CHECK error 11). Recognized when the
    // caller supplies mor tier context, when the ERROR sits on a `%mor`
    // line, and when the whole line is the ERROR node (then the text STARTS
    // with the `%mor:` prefix; until 2026-09-08 this tested `contains`, and
    // an `%eng` body that mentioned `%mor:` beside a `|token` was reported
    // as a `%mor` fault). The span is narrowed to the offending item.
    if (tier_type == Some("mor")
        || error_text.starts_with("%mor:")
        || super::dedicated::on_mor_tier_line(source, start))
        && let Some(item) = super::dedicated::mor_item_with_empty_pos(
            error_text,
            super::dedicated::at_item_boundary(source, start),
        )
    {
        // Narrow the span to the item; `find` re-locates the same
        // first occurrence `split_whitespace` matched.
        let (item_start, item_end) = match error_text.find(item) {
            Some(offset) => (start + offset, start + offset + item.len()),
            None => (start, end),
        };
        return ParseError::new(
            ErrorCode::MorItemEmptyPos,
            Severity::Error,
            SourceLocation::from_offsets(item_start, item_end),
            ErrorContext::new(source, item_start..item_end, item),
            format!("MOR item '{item}' has an empty part-of-speech field"),
        )
        .with_suggestion(
            "Every %mor item is pos|stem with a non-empty part of speech before the pipe \
             (e.g., pro|we, v|go)",
        );
    }

    // E702: a recovery ERROR carrying content inside a `%mor` tier.
    //
    // THE MESSAGE USED TO NAME A FAULT THIS CONDITION DOES NOT ESTABLISH. It
    // said "missing pipe separator", and the condition tests only that the tier
    // is `%mor` and that the ERROR node is non-empty. Both `%mor:\tco| .` and
    // `%mor:\tv|go-PAST^v|went .` reach it with their pipes present, and were
    // told a pipe was missing. A diagnostic whose message does not match the
    // input is this project's own stated tell for a chatter defect, so it now
    // reports what it knows and suggests what it cannot know.
    if tier_type == Some("mor") && !error_text.is_empty() && end > start {
        return ParseError::new(
            ErrorCode::InvalidMorphologyFormat,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Unparsable content in a %mor item",
        )
        .with_suggestion("MOR items are pos|stem (e.g., v|hello, n|world), with optional prefix, suffix and translation");
    }

    // Double comma in dependent tier
    if error_text.contains(",,") {
        return ParseError::new(
            ErrorCode::ConsecutiveCommas,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Double comma found in dependent tier",
        )
        .with_suggestion("Use single comma or replace ,, with special character");
    }

    // Generic dependent tier error
    ParseError::new(
        ErrorCode::UnparsableContent,
        Severity::Error,
        SourceLocation::from_offsets(start, end),
        ErrorContext::new(source, start..end, error_text),
        format!(
            "Unparsable content on dependent tier: '{}'",
            match error_text.lines().next() {
                Some(line) => line,
                None => error_text,
            }
        ),
    )
    .with_suggestion("Check dependent tier format, each entry must follow the tier-specific syntax")
}

/// Backward-compatible wrapper without explicit tier context.
pub(crate) fn analyze_dependent_tier_error(error_node: Node, source: &str) -> ParseError {
    analyze_dependent_tier_error_with_context(error_node, source, None)
}
