//! Source-bound decoding of flat and grouped phonology.
//! Whole `pho_words` text retains compound spelling; recovery preserves the
//! entire group rather than inventing a partial token.

use crate::generated_traversal::{
    AdmittedPhoGroupChoiceSourceView, AsRawNode, KindSlot, NoChild, NonMissingKindSlot,
    PhoGroupNode, PhoGroupedContentNode, PhoWordsNode, SourceBound, SourceField, SourceSlotView,
    WhitespacesNode,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::{PhoItem, PhoWord};

use super::cst::{build_group_from_words, fallback_group_as_text};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};

/// Select the generated flat-word or bracketed-group variant while retaining
/// its owning source. Outer Missing/Error preserve the group; Absent emits
/// nothing. Inner Error/Absent preserve the whole-group fallback; compiled
/// grammar admission excludes missing composite content.
/// Internal source faults propagate rather than selecting recovery.
pub(super) fn extract_pho_group_items<'tree>(
    typed: SourceBound<'tree, '_, PhoGroupNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<PhoItem>, crate::CstFailure> {
    let source = typed.source();
    let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    surface_displaced(&children.children().unexpected, "pho_group", source, errors);
    let choice = match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => choice,
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) => {
            return Ok(fallback_group_as_text(typed));
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => return Ok(Vec::new()),
    };
    Ok(match choice.view() {
        AdmittedPhoGroupChoiceSourceView::PhoWords(words) => {
            let text = words.read()?.text();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![PhoItem::Word(PhoWord::new(text))]
            }
        }
        AdmittedPhoGroupChoiceSourceView::PhoBeginGroup(seq) => {
            for node in seq.field_unexpected().iter() {
                surface_displaced(&[node.raw_node()], "pho_group", node.source(), errors);
            }
            match seq.field_child_1().slot().view() {
                SourceSlotView::Present(content) => build_group_from_words(
                    extract_pho_grouped_content_words(content.read()?, errors)?,
                ),
                SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
                    fallback_group_as_text(typed)
                }
            }
        }
    })
}

/// Traverse the first word and generated repeated separator/word pairs.
pub(super) fn extract_pho_grouped_content_words<'tree, 'source>(
    typed: SourceBound<'tree, 'source, PhoGroupedContentNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<&'source str>, crate::CstFailure> {
    let source = typed.source();
    let contents = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let mut words = Vec::with_capacity(contents.field_child_1().slot().iter().len() + 1);
    push_pho_word(contents.field_child_0().slot(), errors, &mut words)?;
    for element in contents.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_pho_separator(pair.field_child_0().slot(), errors, "pho_grouped_content");
                push_pho_word(pair.field_child_1().slot(), errors, &mut words)?;
                for node in pair.field_unexpected().iter() {
                    surface_displaced(
                        &[node.raw_node()],
                        "pho_grouped_content",
                        node.source(),
                        errors,
                    );
                }
            }
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    source,
                    "pho_grouped_content",
                ));
            }
            SourceSlotView::Absent(never) => match never {},
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        }
    }
    surface_displaced(
        &contents.children().unexpected,
        "pho_grouped_content",
        source,
        errors,
    );
    Ok(words)
}

/// Separators carry no model content. Missing/Error retain their diagnostics;
/// absence stays a no-op until the producer's types prove otherwise.
pub(super) fn push_pho_separator<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, WhitespacesNode<'tree>>>,
    errors: &impl ErrorSink,
    context: &str,
) {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(_) | SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Missing(raw) => {
            check_not_missing(raw.raw_node(), source, errors, context);
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, context));
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// Preserve nonempty word text, diagnose Error, and omit absent slots.
/// The composite word carrier cannot be missing after grammar admission.
fn push_pho_word<'tree, 'source>(
    slot: SourceField<'_, 'tree, 'source, NonMissingKindSlot<'tree, PhoWordsNode<'tree>>>,
    errors: &impl ErrorSink,
    words: &mut Vec<&'source str>,
) -> Result<(), crate::CstFailure> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(word) => {
            let text = word.read()?.text();
            if !text.is_empty() {
                words.push(text);
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "pho_grouped_content",
            ));
        }
        SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
    Ok(())
}
