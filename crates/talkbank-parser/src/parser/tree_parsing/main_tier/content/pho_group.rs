//! Parsing for main-tier phonology groups (`‹ ... ›`), over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, MainPhoGroupNode, SourceBound};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::group::{contents_of, parse_group_contents};
use super::recovery::surface_main_tier_sink;
use super::report_tree_shape;
use crate::parser::tree_parsing::parser_helpers::expect_delimiter;

/// Parse a `main_pho_group` node into `UtteranceContent::PhoGroup`.
///
/// Grammar: `seq(pho_begin_group, contents, pho_end_group)`. The delimiters
/// are structure, the contents go through the one shared walker, and a
/// group with no items is rejected. Phonological groups carry no
/// annotations.
pub(crate) fn parse_pho_group_content<'tree>(
    typed: SourceBound<'tree, '_, MainPhoGroupNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let Ok(associated) = crate::parser::typed_cst::report_reconstruction(
        crate::parser::typed_cst::canonical_grammar()
            .and_then(|grammar| typed.extract_admitted(grammar)),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let children = associated.children();

    expect_delimiter(children.child_0.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected '\u{2039}' at position 0 of main_pho_group, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });
    let group_items = match contents_of(associated.field_child_1().slot(), errors, |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected 'contents' at position 1 of main_pho_group, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    }) {
        Some(contents) => parse_group_contents(&contents, errors),
        None => Vec::new(),
    };
    expect_delimiter(children.child_2.slot(), |bad| {
        report_tree_shape(
            bad,
            format!(
                "Expected '\u{203a}' at position 2 of main_pho_group, found '{}'",
                bad.kind()
            ),
            source,
            errors,
        );
    });
    surface_main_tier_sink(children, source, errors);

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }
    let bracketed = crate::model::BracketedContent::new(group_items);
    ParseOutcome::parsed(UtteranceContent::PhoGroup(crate::model::PhoGroup::new(
        bracketed,
    )))
}
