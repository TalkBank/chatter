//! Header dispatch through the generated source-bound supertype extractor.
//!
//! LEVEL 1 (which-header-kind dispatch): the concrete header subtype CST node is
//! classified by `SourceSlice::extract_header` into an associated
//! `HeaderChoice`, then routed to its per-kind logic by `dispatch_header_choice`,
//! which matches all 34 `HeaderChoice` variants exhaustively (no `_` catch-all
//! that could silently drop a header). This replaces the pre-migration 5-step
//! `node.kind()` string pipeline (`resolve_header_node` + `parse_core_header` +
//! the four `parse_*_header(header_kind, ...)` sub-dispatchers + the
//! `ends_with("_header")` fall-through), and, as of the Task B2 migration, the
//! OLD `TypedTraversal.classify_header` trait-receiver call.
//!
//! LEVEL 2 (the per-header internal parsing inside each per-kind function) is
//! migrated alongside this dispatcher, in the sibling `simple`/`special`/`gem`
//! sub-modules and in `tree_parsing/header/`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

mod gem;
mod simple;
mod special;
mod structured;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, HeaderChoice, HeaderChoiceSourceView, SourceField, SourceSlice, SourceSlotView,
};
use crate::model::Header;
use crate::node_types::THUMBNAIL_HEADER;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a header CST node into a typed `Header`.
///
/// Classifies `header_node` with source-bound extraction and routes a
/// present concrete header to `dispatch_header_choice`.
pub fn parse_header_node(
    header_node: SourceSlice<'_, '_>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let input = header_node.source();
    let children = header_node.extract_header();
    let outcome = match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => dispatch_header_choice(choice, errors),
        // `dispatch_line` only routes a Present concrete `header` subtype node
        // here, so these slots are unreachable in practice. The pre-migration
        // code (supertypes mode of `resolve_header_node`) always produced a
        // concrete (node, kind) pair and so had no error/missing path at this
        // point either; preserve that by rejecting with NO new diagnostic (the
        // whole-tree recovery backstop plus validation still cover any genuine
        // recovery node).
        SourceSlotView::Error(_) | SourceSlotView::Missing(_) | SourceSlotView::Unexpected(_) => {
            ParseOutcome::rejected()
        }
    };
    // A self-classifying supertype extraction like `extract_header` never
    // populates `unexpected` in practice (there is no separate grammar
    // position it could fail to consume), but surface it anyway so every
    // migrated carrier uses the SAME mechanism (see `surface_displaced`).
    surface_displaced(&children.children().unexpected, "header", input, errors);
    outcome
}

/// Route a classified header to its per-kind LEVEL-1 logic.
///
/// Exhaustive over all 34 `HeaderChoice` variants. The variant set is partitioned
/// into: 2 marker-only (inline), 6 structured, 8 special, 3 GEM, 14 simple
/// scalar (each delegating to a per-kind function in the topical sub-module), and
/// 1 `thumbnail_header` gap (the only `header` subtype with no model variant,
/// handled by `thumbnail`). There is deliberately NO `_` catch-all: a future
/// grammar change that adds a `header` subtype must add an arm here, which fails
/// to compile until handled, so no header can be silently dropped.
///
/// Each generated source-view variant retains its producer association.
/// Migrated families receive the admitted `SourceBound` directly; transitional
/// adapters split its node and source only at their existing leaf boundary.
fn dispatch_header_choice<'tree>(
    choice: SourceField<'_, 'tree, '_, HeaderChoice<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // The adapters below keep other header families on their existing lowering
    // API while bound participant, media and scalar lowering retain source identity.
    macro_rules! lower {
        (bound $parse:path, $field:expr) => {
            match crate::parser::typed_cst::read_source_field($field, errors) {
                Some(bound) => $parse(bound, errors),
                None => ParseOutcome::rejected(),
            }
        };
        ($parse:path, $field:expr) => {
            match crate::parser::typed_cst::read_source_field($field, errors) {
                Some(bound) => $parse(bound.node(), bound.source(), errors),
                None => ParseOutcome::rejected(),
            }
        };
    }
    match choice.view() {
        // Marker-only headers (no node content used).
        HeaderChoiceSourceView::NewEpisodeHeader(_) => ParseOutcome::parsed(Header::NewEpisode),
        HeaderChoiceSourceView::BlankHeader(_) => ParseOutcome::parsed(Header::Blank),

        // Structured headers (dedicated sub-parsers in tree_parsing/header/).
        HeaderChoiceSourceView::LanguagesHeader(n) => lower!(structured::languages, n),
        HeaderChoiceSourceView::ParticipantsHeader(n) => {
            match crate::parser::typed_cst::read_source_field(n, errors) {
                Some(bound) => structured::participants(bound, errors),
                None => ParseOutcome::rejected(),
            }
        }
        HeaderChoiceSourceView::IdHeader(n) => lower!(structured::id, n),
        HeaderChoiceSourceView::MediaHeader(n) => {
            match crate::parser::typed_cst::read_source_field(n, errors) {
                Some(bound) => structured::media(bound, errors),
                None => ParseOutcome::rejected(),
            }
        }
        HeaderChoiceSourceView::SituationHeader(n) => lower!(structured::situation, n),
        HeaderChoiceSourceView::TypesHeader(n) => lower!(structured::types, n),

        // Special (mixed-shape) headers.
        HeaderChoiceSourceView::CommentHeader(n) => lower!(bound special::comment, n),
        HeaderChoiceSourceView::NumberHeader(n) => lower!(bound special::number, n),
        HeaderChoiceSourceView::RecordingQualityHeader(n) => {
            lower!(bound special::recording_quality, n)
        }
        HeaderChoiceSourceView::TranscriptionHeader(n) => lower!(bound special::transcription, n),
        HeaderChoiceSourceView::BirthOfHeader(n) => lower!(bound special::birth_of, n),
        HeaderChoiceSourceView::BirthplaceOfHeader(n) => lower!(bound special::birthplace_of, n),
        HeaderChoiceSourceView::L1OfHeader(n) => lower!(bound special::l1_of, n),
        HeaderChoiceSourceView::OptionsHeader(n) => lower!(special::options, n),

        // GEM headers.
        HeaderChoiceSourceView::BgHeader(n) => lower!(gem::bg, n),
        HeaderChoiceSourceView::EgHeader(n) => lower!(gem::eg, n),
        HeaderChoiceSourceView::GHeader(n) => lower!(gem::g, n),

        // Simple scalar headers.
        HeaderChoiceSourceView::DateHeader(n) => lower!(bound simple::date, n),
        HeaderChoiceSourceView::TapeLocationHeader(n) => lower!(bound simple::tape_location, n),
        HeaderChoiceSourceView::TimeDurationHeader(n) => lower!(bound simple::time_duration, n),
        HeaderChoiceSourceView::TimeStartHeader(n) => lower!(bound simple::time_start, n),
        HeaderChoiceSourceView::LocationHeader(n) => lower!(bound simple::location, n),
        HeaderChoiceSourceView::RoomLayoutHeader(n) => lower!(bound simple::room_layout, n),
        HeaderChoiceSourceView::TranscriberHeader(n) => lower!(bound simple::transcriber, n),
        HeaderChoiceSourceView::WarningHeader(n) => lower!(bound simple::warning, n),
        HeaderChoiceSourceView::ActivitiesHeader(n) => lower!(bound simple::activities, n),
        HeaderChoiceSourceView::BckHeader(n) => lower!(bound simple::bck, n),
        HeaderChoiceSourceView::PageHeader(n) => lower!(bound simple::page, n),
        HeaderChoiceSourceView::VideosHeader(n) => lower!(bound simple::videos, n),
        HeaderChoiceSourceView::THeader(n) => lower!(bound simple::t, n),
        HeaderChoiceSourceView::UnsupportedHeader(n) => {
            simple::unsupported(n.raw_node(), n.source(), errors)
        }

        // Gap: `thumbnail_header` is the one `header` subtype with no model
        // variant. Preserve the pre-migration `ends_with("_header")`
        // fall-through, which reported `UnknownHeader` and rejected.
        HeaderChoiceSourceView::ThumbnailHeader(n) => thumbnail(n.raw_node(), n.source(), errors),
    }
}

/// Report the `UnknownHeader` diagnostic for a `@Thumbnail` header and reject.
///
/// Reproduces exactly what the pre-migration `ends_with("_header")`
/// fall-through emitted for the one `header` subtype with no per-kind logic:
/// `ErrorCode::UnknownHeader` at the node span with the node-kind context and
/// the `"Unrecognized header type '<kind>'"` message.
fn thumbnail(node: Node, input: &str, errors: &impl ErrorSink) -> ParseOutcome<Header> {
    errors.report(ParseError::new(
        ErrorCode::UnknownHeader,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(input, node.start_byte()..node.end_byte(), THUMBNAIL_HEADER),
        format!("Unrecognized header type '{}'", THUMBNAIL_HEADER),
    ));
    ParseOutcome::rejected()
}
