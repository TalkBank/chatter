//! Parsing for main-tier phonology groups (`‹ ... ›`), over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::ErrorSink;
use crate::generated_traversal::{MainPhoGroupNode, extract_main_pho_group};
use crate::model::UtteranceContent;
use talkbank_model::ParseOutcome;

use super::group::{contents_of, parse_group_contents};
use super::report_tree_shape;
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, surface_displaced};

/// Parse a `main_pho_group` node into `UtteranceContent::PhoGroup`.
///
/// Grammar: `seq(pho_begin_group, contents, pho_end_group)`. The delimiters
/// are structure, the contents go through the one shared walker, and a
/// group with no items is rejected. Phonological groups carry no
/// annotations.
pub(crate) fn parse_pho_group_content(
    typed: MainPhoGroupNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let children = extract_main_pho_group(typed);

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
    let group_items = match contents_of(children.child_1.slot(), |bad| {
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
        Some(contents) => parse_group_contents(&contents, source, errors),
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
    surface_displaced(&children.unexpected, "main_pho_group", source, errors);

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }
    let bracketed = crate::model::BracketedContent::new(group_items);
    ParseOutcome::parsed(UtteranceContent::PhoGroup(crate::model::PhoGroup::new(
        bracketed,
    )))
}
