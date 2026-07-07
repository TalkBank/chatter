//! Header-node dispatch driven by the NEW backend's free `extract_header`.
//!
//! LEVEL 1 (which-header-kind dispatch): the concrete header subtype CST node is
//! classified by the free [`extract_header`] function into a typed
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
use crate::generated_traversal::{AsRawNode, HeaderChoice, NodeSlot, extract_header};
use crate::model::Header;
use crate::node_types::THUMBNAIL_HEADER;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a header CST node into a typed `Header`.
///
/// Classifies `header_node` with the free [`extract_header`] and routes a
/// present concrete header to `dispatch_header_choice`.
pub fn parse_header_node(
    header_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_header(header_node);
    let outcome = match children.content.slot {
        NodeSlot::Present(choice) => dispatch_header_choice(choice, input, errors),
        // `dispatch_line` only routes a Present concrete `header` subtype node
        // here, so these slots are unreachable in practice. The pre-migration
        // code (supertypes mode of `resolve_header_node`) always produced a
        // concrete (node, kind) pair and so had no error/missing path at this
        // point either; preserve that by rejecting with NO new diagnostic (the
        // whole-tree recovery backstop plus validation still cover any genuine
        // recovery node).
        NodeSlot::Error(_) | NodeSlot::Missing(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            ParseOutcome::rejected()
        }
    };
    // A self-classifying supertype extraction like `extract_header` never
    // populates `unexpected` in practice (there is no separate grammar
    // position it could fail to consume), but surface it anyway so every
    // migrated carrier uses the SAME mechanism (see `surface_unexpected`).
    surface_unexpected(&children.unexpected, input, errors);
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
/// Each `HeaderChoice` variant now holds the NEW backend's typed leaf wrapper
/// (e.g. `LanguagesHeaderNode`), not a bare `Node` as the OLD `HeaderChoice`
/// did; `.raw_node()` (`AsRawNode`) recovers the same raw node the per-kind
/// functions (unchanged, still `Node`-typed) expect.
fn dispatch_header_choice(
    choice: HeaderChoice,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match choice {
        // Marker-only headers (no node content used).
        HeaderChoice::NewEpisodeHeader(_) => ParseOutcome::parsed(Header::NewEpisode),
        HeaderChoice::BlankHeader(_) => ParseOutcome::parsed(Header::Blank),

        // Structured headers (dedicated sub-parsers in tree_parsing/header/).
        HeaderChoice::LanguagesHeader(n) => structured::languages(n.raw_node(), input, errors),
        HeaderChoice::ParticipantsHeader(n) => {
            structured::participants(n.raw_node(), input, errors)
        }
        HeaderChoice::IdHeader(n) => structured::id(n.raw_node(), input, errors),
        HeaderChoice::MediaHeader(n) => structured::media(n.raw_node(), input, errors),
        HeaderChoice::SituationHeader(n) => structured::situation(n.raw_node(), input, errors),
        HeaderChoice::TypesHeader(n) => structured::types(n.raw_node(), input, errors),

        // Special (mixed-shape) headers.
        HeaderChoice::CommentHeader(n) => special::comment(n.raw_node(), input, errors),
        HeaderChoice::NumberHeader(n) => special::number(n.raw_node(), input, errors),
        HeaderChoice::RecordingQualityHeader(n) => {
            special::recording_quality(n.raw_node(), input, errors)
        }
        HeaderChoice::TranscriptionHeader(n) => special::transcription(n.raw_node(), input, errors),
        HeaderChoice::BirthOfHeader(n) => special::birth_of(n.raw_node(), input, errors),
        HeaderChoice::BirthplaceOfHeader(n) => special::birthplace_of(n.raw_node(), input, errors),
        HeaderChoice::L1OfHeader(n) => special::l1_of(n.raw_node(), input, errors),
        HeaderChoice::OptionsHeader(n) => special::options(n.raw_node(), input, errors),

        // GEM headers.
        HeaderChoice::BgHeader(n) => gem::bg(n.raw_node(), input, errors),
        HeaderChoice::EgHeader(n) => gem::eg(n.raw_node(), input, errors),
        HeaderChoice::GHeader(n) => gem::g(n.raw_node(), input, errors),

        // Simple scalar headers.
        HeaderChoice::DateHeader(n) => simple::date(n.raw_node(), input, errors),
        HeaderChoice::TapeLocationHeader(n) => simple::tape_location(n.raw_node(), input, errors),
        HeaderChoice::TimeDurationHeader(n) => simple::time_duration(n.raw_node(), input, errors),
        HeaderChoice::TimeStartHeader(n) => simple::time_start(n.raw_node(), input, errors),
        HeaderChoice::LocationHeader(n) => simple::location(n.raw_node(), input, errors),
        HeaderChoice::RoomLayoutHeader(n) => simple::room_layout(n.raw_node(), input, errors),
        HeaderChoice::TranscriberHeader(n) => simple::transcriber(n.raw_node(), input, errors),
        HeaderChoice::WarningHeader(n) => simple::warning(n.raw_node(), input, errors),
        HeaderChoice::ActivitiesHeader(n) => simple::activities(n.raw_node(), input, errors),
        HeaderChoice::BckHeader(n) => simple::bck(n.raw_node(), input, errors),
        HeaderChoice::PageHeader(n) => simple::page(n.raw_node(), input, errors),
        HeaderChoice::VideosHeader(n) => simple::videos(n.raw_node(), input, errors),
        HeaderChoice::THeader(n) => simple::t(n.raw_node(), input, errors),
        HeaderChoice::UnsupportedHeader(n) => simple::unsupported(n.raw_node(), input, errors),

        // Gap: `thumbnail_header` is the one `header` subtype with no model
        // variant. Preserve the pre-migration `ends_with("_header")`
        // fall-through, which reported `UnknownHeader` and rejected.
        HeaderChoice::ThumbnailHeader(n) => thumbnail(n.raw_node(), input, errors),
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
