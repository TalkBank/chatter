//! Parsing for `@Types` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use crate::generated_traversal::{
    AsRawNode, KindSlot, NamedKind, NoChild, SourceBound, SourceBoundKind, SourceField,
    SourceSlotView, TypesActivityNode, TypesDesignNode, TypesGroupNode, TypesHeaderNode,
};

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::node_types::TYPES_HEADER;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::{GeneratedPosition, report_cst_failure};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, TypesHeader};

/// The field's generated kind owns its position; callers supply no selector.
trait RequiredTypesField<'tree>: SourceBoundKind<'tree> + NamedKind {
    /// The generated carrier and field of this required position, named in
    /// a producer-fault report when the walk leaves it empty.
    const POSITION: GeneratedPosition;
}

impl<'tree> RequiredTypesField<'tree> for TypesDesignNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChildren.child_2");
}

impl<'tree> RequiredTypesField<'tree> for TypesActivityNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChildren.child_6");
}

impl<'tree> RequiredTypesField<'tree> for TypesGroupNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChildren.child_10");
}

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
    let children = typed.extract();

    let ParseOutcome::Parsed(design) =
        read_types_field(children.field_child_2().slot(), typed, errors)?
    else {
        surface_displaced(
            &children.children().unexpected,
            TYPES_HEADER,
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "Expected @Types:\tdesign, activity, group",
            "Missing design field in @Types header",
        );
    };

    let ParseOutcome::Parsed(activity) =
        read_types_field(children.field_child_6().slot(), typed, errors)?
    else {
        surface_displaced(
            &children.children().unexpected,
            TYPES_HEADER,
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "Expected @Types:\tdesign, activity, group",
            "Missing activity field in @Types header",
        );
    };

    let ParseOutcome::Parsed(group) =
        read_types_field(children.field_child_10().slot(), typed, errors)?
    else {
        surface_displaced(
            &children.children().unexpected,
            TYPES_HEADER,
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "Expected @Types:\tdesign, activity, group",
            "Missing group field in @Types header",
        );
    };

    surface_displaced(
        &children.children().unexpected,
        TYPES_HEADER,
        source,
        errors,
    );
    let types_header = TypesHeader::new(design, activity, group);

    Ok(Header::Types(types_header))
}

/// Read one mandatory `@Types` field from its source-bound positional slot.
///
/// `slot` is the field's `child_N` slot (e.g. `KindSlot<TypesDesignNode>`);
/// its kind's `RequiredTypesField::POSITION` names the generated carrier
/// and field in a producer-fault report; `header` is the bound `@Types`
/// header, whose node locates both the missing-field diagnostic and the
/// producer-fault report. The slot match is EXHAUSTIVE over
/// every `NodeSlot` variant; there is deliberately no `_` catch-all that
/// could silently drop a recovery slot. Source-read failures propagate
/// separately from authored missing-field errors.
///
/// Reachability (coverage drive batch 4, 2026-10-19): a sweep of 1,345
/// single-edit mutations of a `@Types` line put none of the three field
/// positions in `Missing` or `Error` (a broken line is refused at the file
/// level as E316 before this header is built), so the "Missing <field>"
/// report is entered by no authored file and stays for the states the slot
/// type admits.
fn read_types_field<'tree, T: RequiredTypesField<'tree>>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, T>>,
    header: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<String>, crate::CstFailure> {
    let node = header.raw_node();
    let source = header.source();
    match slot.view() {
        SourceSlotView::Present(field) => Ok(ParseOutcome::parsed(field.read()?.text().to_owned())),
        // The pre-migration `find_child_text` returned `None` for any
        // non-present field child (absent, missing, error or unexpected),
        // funnelling all of them to the SAME "Missing <label> in @Types
        // header" diagnostic at the HEADER NODE span. The MISSING and ERROR
        // states keep that report; an absent position is the fault below.
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(source, span_of(node), TYPES_HEADER),
                format!("Missing {} in @Types header", T::KIND.name()),
            ));
            Ok(ParseOutcome::rejected())
        }
        // A required field the walk left empty is a generator or grammar
        // fault, not a field the header lacks. Until 2026-10-19 it took the
        // "Missing <field>" report for content never read.
        SourceSlotView::Absent(NoChild) => {
            report_cst_failure(
                node,
                source,
                crate::CstFailure::AbsentRequiredPosition {
                    rule: TYPES_HEADER,
                    position: T::POSITION,
                },
                errors,
            );
            Ok(ParseOutcome::rejected())
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
}
