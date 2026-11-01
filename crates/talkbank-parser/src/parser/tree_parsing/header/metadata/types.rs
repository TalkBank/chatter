//! Parsing for `@Types` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use crate::generated_traversal::{
    AsRawNode, KindAdmitted, NamedKind, NarrowedKindSlot, NoChild, Raw, SourceBound,
    SourceBoundKind, SourceField, SourceSlotView, TypesActivityNode, TypesDesignNode,
    TypesGroupNode, TypesHeaderNode,
};

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::node_types::TYPES_HEADER;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{
    ContentReadError, HeaderSite, read_empty_body, surface_displaced,
};
use crate::parser::typed_cst::{ExtractCanonical, GeneratedPosition, report_cst_failure};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{
    ActivityType, DesignType, GroupType, Header, InvalidTypesField, PlainKind, StructuredKind,
    TypesHeader,
};

/// The field's generated kind owns its position; callers supply no selector.
trait RequiredTypesField<'tree>: SourceBoundKind<'tree> + NamedKind {
    /// The generated carrier and field of this required position, named in
    /// a producer-fault report when the walk leaves it empty.
    const POSITION: GeneratedPosition;
    type Value;
    fn admit(text: &str) -> Result<Self::Value, InvalidTypesField>;
}

impl<'tree> RequiredTypesField<'tree> for TypesDesignNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChild2Children.child_0");
    type Value = DesignType;
    fn admit(text: &str) -> Result<Self::Value, InvalidTypesField> {
        DesignType::from_text(text)
    }
}

impl<'tree> RequiredTypesField<'tree> for TypesActivityNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChild2Children.child_4");
    type Value = ActivityType;
    fn admit(text: &str) -> Result<Self::Value, InvalidTypesField> {
        ActivityType::from_text(text)
    }
}

impl<'tree> RequiredTypesField<'tree> for TypesGroupNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("TypesHeaderChild2Children.child_8");
    type Value = GroupType;
    fn admit(text: &str) -> Result<Self::Value, InvalidTypesField> {
        GroupType::from_text(text)
    }
}

/// Parse Types header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// types_header: $ => seq(
///     types_prefix, header_sep,         // typed child_0, child_1
///     optional(seq(                     // typed child_2: the three fields
///         types_design,                 //   group child_0
///         optional(whitespaces), comma, optional(whitespaces),
///         types_activity,               //   group child_4
///         optional(whitespaces), comma, optional(whitespaces),
///         types_group,                  //   group child_8
///     )),
///     newline                           // typed child_3
/// )
/// ```
///
/// The @Types header has three mandatory fields: design, activity, group.
/// The whole field group is optional in the grammar so that a header with
/// nothing after its separator parses: it is `Header::EmptyContent`, which
/// validation refuses (E553). A partial group is still malformed.
pub fn parse_types_header<'tree>(
    typed: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Header>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();

    // The design->activity->group order and short-circuit on the first
    // missing field are preserved. Present fields separately admit readable
    // source ranges; failures remain producer faults. Extracted under the
    // canonical grammar's admitted kind proof: the three field kinds are
    // narrowed, so none of them can be MISSING here.
    let children = typed.extract_canonical()?;
    let finish = |header: Result<ParseOutcome<Header>, crate::CstFailure>| {
        surface_displaced(
            &children.children().unexpected,
            TYPES_HEADER,
            source,
            errors,
        );
        header
    };
    let refuse = |reason: &str| {
        super::super::unknown_header(
            node,
            source,
            "Expected @Types:\tdesign, activity, group",
            reason,
        )
        .map(ParseOutcome::Parsed)
    };

    let site = HeaderSite::bound(typed);
    let group = match children.field_child_2().slot().optional() {
        // The field group is the body; an omitted one is the empty state
        // when the separator was written (the same rule every header's body
        // follows), and a malformed header otherwise.
        None => {
            return finish(Ok(match read_empty_body(&site) {
                Ok(separator) => ParseOutcome::Parsed(
                    PlainKind::Structured(StructuredKind::Types).empty(separator),
                ),
                Err(failure) => failure.into_outcome(&site, errors),
            }));
        }
        Some(slot) => match slot.view() {
            SourceSlotView::Present(group) => group,
            // An ERROR where the field group belongs is unparsable text: the
            // whole-tree backstop reports it, and the line is kept as an
            // unknown header with its text, as every unparsable header body
            // is under both parsers.
            SourceSlotView::Error(_) => {
                return finish(Ok(ContentReadError::Unparsable.into_outcome(&site, errors)));
            }
        },
    };
    for displaced in group.field_unexpected().iter() {
        surface_displaced(&[displaced.raw_node()], TYPES_HEADER, source, errors);
    }
    // An ERROR in or beside the fields (`long, toy play, TD` parses `toy`
    // as the activity and `play` as an ERROR) means the fields are not all
    // that was written: the header is kept as unknown with its text, by the
    // same decision every header body takes, never a @Types built from the
    // fields around the ERROR.
    let group = match site.complete_body(group) {
        Ok(group) => group.into_inner(),
        Err(failure) => return finish(Ok(failure.into_outcome(&site, errors))),
    };

    let ParseOutcome::Parsed(design) =
        read_types_field(group.field_child_0().slot(), typed, errors)?
    else {
        return finish(refuse("Invalid or missing design field in @Types header"));
    };

    let ParseOutcome::Parsed(activity) =
        read_types_field(group.field_child_4().slot(), typed, errors)?
    else {
        return finish(refuse("Invalid or missing activity field in @Types header"));
    };

    let ParseOutcome::Parsed(group) =
        read_types_field(group.field_child_8().slot(), typed, errors)?
    else {
        return finish(refuse("Invalid or missing group field in @Types header"));
    };

    finish(Ok(ParseOutcome::Parsed(Header::Types(TypesHeader::new(
        design, activity, group,
    )))))
}

/// Read one mandatory `@Types` field from its source-bound positional slot.
///
/// `slot` is the field's `child_N` slot, narrowed under the admitted kind
/// proof (e.g. `NarrowedKindSlot<TypesDesignNode, Raw, KindAdmitted>`), so it
/// has no `Missing` state;
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
/// level as E316 before this header is built). `Missing` is now uninhabited
/// under the admitted proof; the "Missing <field>" report stays for the
/// `Error` state the slot type still admits.
fn read_types_field<'tree, T: RequiredTypesField<'tree>>(
    slot: SourceField<'_, 'tree, '_, NarrowedKindSlot<'tree, T, Raw, KindAdmitted>>,
    header: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<T::Value>, crate::CstFailure> {
    let node = header.raw_node();
    let source = header.source();
    match slot.view() {
        SourceSlotView::Present(field) => {
            let field = field.read()?;
            match T::admit(field.text()) {
                Ok(value) => Ok(ParseOutcome::parsed(value)),
                Err(reason) => {
                    let span = span_of(field.raw_node());
                    errors.report(ParseError::new(
                        ErrorCode::UnparsableContent,
                        Severity::Error,
                        DiagnosticLocation::new(span),
                        ErrorContext::new(source, span, T::KIND.name()),
                        reason.to_string(),
                    ));
                    Ok(ParseOutcome::rejected())
                }
            }
        }
        // An ERROR node where the field belongs takes the "Missing <label>
        // in @Types header" report at the HEADER NODE span; an absent
        // position is the fault below.
        SourceSlotView::Error(_) => {
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
        SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
    }
}
