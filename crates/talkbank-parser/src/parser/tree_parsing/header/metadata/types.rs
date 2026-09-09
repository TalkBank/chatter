//! Parsing for `@Types` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use crate::generated_traversal::{
    AsRawNode, ChildSlot, NoChild, SlotView, TypesHeaderNode, extract_types_header,
};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::decode_present_child;
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
pub fn parse_types_header(
    typed: TypesHeaderNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Header {
    let node = typed.raw_node();

    // Grammar: seq(types_prefix, header_sep, types_design, comma, whitespaces?,
    // types_activity, comma, whitespaces?, types_group, newline). The NEW backend
    // models the interstitial whitespace as its own position (no longer skipped),
    // so the three mandatory fields sit at `child_2` (design, UNCHANGED),
    // `child_6` (activity, was `child_4`), `child_10` (group, was `child_6`); read
    // through the NEW backend's free, typed `extract_types_header`, each field
    // matched EXHAUSTIVELY. The per-field diagnostics + the caller's
    // `unknown_types_header` recovery are byte-identical to the pre-migration
    // `find_child_text` behaviour; the design->activity->group order (and its
    // short-circuit on the first missing field) is preserved.
    let children = extract_types_header(typed);

    let ParseOutcome::Parsed(design) = read_types_field(
        children.child_2.slot(),
        node,
        source,
        errors,
        "types_design",
    ) else {
        surface_displaced(&children.unexpected, "types_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing design field in @Types header",
        );
    };

    let ParseOutcome::Parsed(activity) = read_types_field(
        children.child_6.slot(),
        node,
        source,
        errors,
        "types_activity",
    ) else {
        surface_displaced(&children.unexpected, "types_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing activity field in @Types header",
        );
    };

    let ParseOutcome::Parsed(group) = read_types_field(
        children.child_10.slot(),
        node,
        source,
        errors,
        "types_group",
    ) else {
        surface_displaced(&children.unexpected, "types_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Types",
            "Expected @Types:\tdesign, activity, group",
            "Missing group field in @Types header",
        );
    };

    surface_displaced(&children.unexpected, "types_header", source, errors);
    let types_header = TypesHeader::new(design, activity, group);

    Header::Types(types_header)
}

/// Read one mandatory `@Types` field from its typed positional slot, reproducing
/// the pre-migration `find_child_text` text + diagnostic handling EXACTLY.
///
/// `slot` is the field's `child_N` slot (e.g. `ChildSlot<TypesDesignNode>`);
/// `node` is the `@Types` header node (used for the missing-field diagnostic
/// span); `label` is the field name (`types_design` / `types_activity` /
/// `types_group`) used to build the preserved diagnostic messages and context.
/// The slot match is EXHAUSTIVE over every `NodeSlot` variant; there is
/// deliberately no `_` catch-all that could silently drop a recovery slot.
fn read_types_field<'tree, T: AsRawNode<'tree>>(
    slot: &ChildSlot<'tree, T>,
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    label: &str,
) -> ParseOutcome<String> {
    match slot.view() {
        SlotView::Present(field) => {
            // Decode through the shared `decode_present_child` helper, which reads
            // from the RAW node's `utf8_text` (NOT the wrapper's `.text()`
            // accessor, which swallows UTF-8 errors via `unwrap_or("")`),
            // reproducing the pre-migration `find_child_text` Ok/Err handling. The
            // per-field diagnostic (context = `label`, the "text from {label}"
            // wording) is supplied here, so it stays byte-identical.
            decode_present_child(field.raw_node(), source, errors, label, |e| {
                format!("Failed to extract UTF-8 text from {}: {}", label, e)
            })
        }
        // The pre-migration `find_child_text` returned `None` for an absent /
        // missing / error / unexpected field child, funnelling to the SAME
        // "Missing <label> in @Types header" diagnostic at the HEADER NODE span.
        // Preserve that exactly.
        SlotView::Missing(_) | SlotView::Absent(NoChild) | SlotView::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "types_header"),
                format!("Missing {} in @Types header", label),
            ));
            ParseOutcome::rejected()
        }
    }
}
