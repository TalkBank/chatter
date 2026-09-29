//! Source-bound dispatch for headers with dedicated structured sub-parsers.
//!
//! Every entry retains its parse owner's node/source capability. One wrapper
//! forwards collected diagnostics and distinguishes producer failure from CHAT
//! recovery; callers cannot substitute an unrelated source string.

use crate::error::{ErrorCollector, ErrorSink};
use crate::generated_traversal::{
    AsRawNode, IdHeaderNode, LanguagesHeaderNode, MediaHeaderNode, ParticipantsHeaderNode,
    SituationHeaderNode, SourceBound, SourceBoundKind, TypesHeaderNode,
};
use crate::model::Header;
use talkbank_model::ParseOutcome;

use crate::parser::tree_parsing::header::{
    parse_id_header, parse_languages_header, parse_media_header, parse_participants_header,
    parse_situation_header, parse_types_header,
};

/// Preserve diagnostics already collected before an internal producer failure.
fn call_sub<'tree, 'source, N, F, E>(
    header: SourceBound<'tree, 'source, N>,
    errors: &impl ErrorSink,
    sub: F,
) -> ParseOutcome<Header>
where
    N: SourceBoundKind<'tree>,
    F: FnOnce(SourceBound<'tree, 'source, N>, &ErrorCollector) -> Result<Header, E>,
    E: Into<crate::CstFailure>,
{
    let header_errors = ErrorCollector::new();
    let node = header.raw_node();
    let source = header.source();
    let result = sub(header, &header_errors);
    errors.report_all(header_errors.into_vec());
    match result {
        Ok(header) => ParseOutcome::Parsed(header),
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(node, source, failure, errors);
            ParseOutcome::Rejected
        }
    }
}

/// Lower this header without separating its node from its source.
pub(super) fn languages<'tree>(
    header: SourceBound<'tree, '_, LanguagesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_languages_header)
}

/// Lower this header without separating its node from its source.
pub(super) fn participants<'tree>(
    header: SourceBound<'tree, '_, ParticipantsHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_participants_header)
}

/// Lower this header without separating its node from its source.
pub(super) fn id<'tree>(
    header: SourceBound<'tree, '_, IdHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_id_header)
}

/// Lower this header without separating its node from its source.
pub(super) fn media<'tree>(
    header: SourceBound<'tree, '_, MediaHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_media_header)
}

/// Lower this header without separating its node from its source.
pub(super) fn situation<'tree>(
    header: SourceBound<'tree, '_, SituationHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_situation_header)
}

/// Lower this header without separating its node from its source.
pub(super) fn types<'tree>(
    header: SourceBound<'tree, '_, TypesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    call_sub(header, errors, parse_types_header)
}
