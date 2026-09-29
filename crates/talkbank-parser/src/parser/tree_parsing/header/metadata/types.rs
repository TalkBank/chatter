//! Parsing for `@Types` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use crate::generated_traversal::{
    AsRawNode, KindSlot, NoChild, SourceBound, SourceBoundKind, SourceField, SourceSlotView,
    TypesHeaderNode,
};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, TypesHeader};

/// Parse Types header from tree-sitter node
///
/// **Grammar Rule (the NEW backend does NOT skip whitespace, so the field
/// indices are wider than the OLD module's; the FIELDS THEMSELVES are
/// unchanged):**
/// ```javascript
/// types_header: $ => seq(
///     '@', 'Types', $.colon, $.tab,
///     $.types_design,        // typed child_2: design type (cross, long, observ)
///     $.comma,                    // typed child_3
///     optional($.whitespaces),    // typed child_4 (NEW: not skipped, was implicit)
///     $.types_activity,      // typed child_6: activity type (was child_4 pre-B2)
///     $.comma,                    // typed child_7
///     optional($.whitespaces),    // typed child_8 (NEW: not skipped, was implicit)
///     $.types_group,         // typed child_10: group type (was child_6 pre-B2)
///     $.newline                   // typed child_11
/// )
/// ```
///
/// The @Types header has three mandatory fields: design, activity, group.
pub fn parse_types_header<'tree>(
    typed: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();

    // Grammar: seq(types_prefix, header_sep, types_design, comma, whitespaces?,
    // types_activity, comma, whitespaces?, types_group, newline). The NEW backend
    // models the interstitial whitespace as its own position (no longer skipped),
    // so the three mandatory fields sit at `child_2` (design, UNCHANGED),
    // `child_6` (activity, was `child_4`), `child_10` (group, was `child_6`); read
    // through source-bound extraction. The design->activity->group order and
    // short-circuit on the first missing field are preserved. Present fields
    // separately admit readable source ranges; failures remain producer faults.
    let children = typed.extract()?;

    let ParseOutcome::Parsed(design) = read_types_field(
        children.field_child_2().slot(),
        typed,
        errors,
        "types_design",
    )?
    else {
        surface_displaced(
            &children.children().unexpected,
            "types_header",
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing design field in @Types header",
        );
    };

    let ParseOutcome::Parsed(activity) = read_types_field(
        children.field_child_6().slot(),
        typed,
        errors,
        "types_activity",
    )?
    else {
        surface_displaced(
            &children.children().unexpected,
            "types_header",
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing activity field in @Types header",
        );
    };

    let ParseOutcome::Parsed(group) = read_types_field(
        children.field_child_10().slot(),
        typed,
        errors,
        "types_group",
    )?
    else {
        surface_displaced(
            &children.children().unexpected,
            "types_header",
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing group field in @Types header",
        );
    };

    surface_displaced(
        &children.children().unexpected,
        "types_header",
        source,
        errors,
    );
    let types_header = TypesHeader::new(design, activity, group);

    Ok(Header::Types(types_header))
}

/// Read one mandatory `@Types` field from its source-bound positional slot.
///
/// `slot` is the field's `child_N` slot (e.g. `KindSlot<TypesDesignNode>`);
/// `header` is the bound `@Types` header (used for the missing-field diagnostic
/// span); `label` is the field name (`types_design` / `types_activity` /
/// `types_group`) used to build the preserved diagnostic messages and context.
/// The slot match is EXHAUSTIVE over every `NodeSlot` variant; there is
/// deliberately no `_` catch-all that could silently drop a recovery slot.
/// Source-read failures propagate separately from authored missing-field errors.
fn read_types_field<'tree, T: SourceBoundKind<'tree>>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, T>>,
    header: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
    label: &str,
) -> Result<ParseOutcome<String>, crate::CstFailure> {
    let node = header.raw_node();
    let source = header.source();
    match slot.view() {
        SourceSlotView::Present(field) => Ok(ParseOutcome::parsed(field.read()?.text().to_owned())),
        // The pre-migration `find_child_text` returned `None` for an absent /
        // missing / error / unexpected field child, funnelling to the SAME
        // "Missing <label> in @Types header" diagnostic at the HEADER NODE span.
        // Preserve that exactly.
        SourceSlotView::Missing(_) | SourceSlotView::Absent(NoChild) | SourceSlotView::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "types_header"),
                format!("Missing {} in @Types header", label),
            ));
            Ok(ParseOutcome::rejected())
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
}
