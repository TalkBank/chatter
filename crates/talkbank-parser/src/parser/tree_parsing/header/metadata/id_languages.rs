//! Typed `@ID` language components, including compact commas and opaque values.
//! Delimiters belong to the grammar; no comma-splitting decoder mirrors it here.

use crate::CstFailure;
use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, IdLanguageChoiceSourceView, IdLanguageNode, IdLanguagesChild0ChoiceSourceView,
    IdLanguagesChild1ChoiceSourceView, IdLanguagesNode, NoChild, SourceBound, SourceSlotView,
};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::{
    ParseOutcome,
    model::{LanguageCode, LanguageCodes},
};

/// The accumulator cannot recover after a refused component. Continue walking
/// to report subsequent faults, but never certify the surviving subset as a list.
fn include_code(
    list: &mut ParseOutcome<Vec<LanguageCode>>,
    code: ParseOutcome<Option<LanguageCode>>,
) {
    match code {
        ParseOutcome::Parsed(Some(code)) => {
            if let ParseOutcome::Parsed(codes) = list {
                codes.push(code);
            }
        }
        ParseOutcome::Parsed(None) => {}
        ParseOutcome::Rejected => *list = ParseOutcome::Rejected,
    }
}

fn component<'tree>(
    typed: SourceBound<'tree, '_, IdLanguageNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Option<LanguageCode>>, CstFailure> {
    let children = typed.extract();
    surface_displaced(
        &children.children().unexpected,
        "id_language",
        typed.source(),
        errors,
    );
    if !children.children().unexpected.is_empty() {
        return Ok(ParseOutcome::Rejected);
    }
    Ok(match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => match choice.view() {
            IdLanguageChoiceSourceView::LanguageCode(code) => {
                super::languages::parse_language_code(code.read()?, errors).map(Some)
            }
            IdLanguageChoiceSourceView::GenericIdLanguage(value) => {
                let value = value.read()?;
                // Preserve the existing trim/empty-component policy. Unicode
                // whitespace can still occupy a nonempty generic token.
                let text = value.text().trim();
                if text.is_empty() {
                    ParseOutcome::parsed(None)
                } else {
                    match LanguageCode::new(text) {
                        Ok(code) => ParseOutcome::parsed(Some(code)),
                        Err(_) => {
                            report_recovery(value.raw_node(), value.source(), errors);
                            ParseOutcome::Rejected
                        }
                    }
                }
            }
        },
        SourceSlotView::Missing(node) | SourceSlotView::Error(node) => {
            report_recovery(node.raw_node(), typed.source(), errors);
            ParseOutcome::Rejected
        }
        SourceSlotView::Absent(NoChild) => {
            report_recovery(typed.raw_node(), typed.source(), errors);
            ParseOutcome::Rejected
        }
        SourceSlotView::Unexpected(never) => match never {},
    })
}

fn report_recovery(node: tree_sitter::Node<'_>, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::EmptyIDLanguage,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.byte_range(), "id_languages"),
        "Malformed language component in @ID header",
    ));
}

pub(crate) fn parse_id_languages<'tree>(
    typed: SourceBound<'tree, '_, IdLanguagesNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<LanguageCodes>, CstFailure> {
    let children = typed.extract();
    let mut codes = ParseOutcome::parsed(Vec::new());
    match children.field_child_0().slot().view() {
        SourceSlotView::Present(choice) => match choice.view() {
            IdLanguagesChild0ChoiceSourceView::IdLanguage(value) => {
                include_code(&mut codes, component(value.read()?, errors)?);
            }
            IdLanguagesChild0ChoiceSourceView::Comma(_)
            | IdLanguagesChild0ChoiceSourceView::Space(_)
            | IdLanguagesChild0ChoiceSourceView::Tab(_) => {}
        },
        SourceSlotView::Missing(node) | SourceSlotView::Error(node) => {
            report_recovery(node.raw_node(), typed.source(), errors);
            codes = ParseOutcome::Rejected;
        }
        SourceSlotView::Absent(NoChild) => {
            report_recovery(typed.raw_node(), typed.source(), errors);
            codes = ParseOutcome::Rejected;
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
    for item in children.field_child_1().slot().iter() {
        match item.slot().view() {
            SourceSlotView::Present(choice) => match choice.view() {
                IdLanguagesChild1ChoiceSourceView::IdLanguage(value) => {
                    include_code(&mut codes, component(value.read()?, errors)?);
                }
                IdLanguagesChild1ChoiceSourceView::Comma(_)
                | IdLanguagesChild1ChoiceSourceView::Space(_)
                | IdLanguagesChild1ChoiceSourceView::Tab(_) => {}
            },
            SourceSlotView::Missing(node) | SourceSlotView::Error(node) => {
                report_recovery(node.raw_node(), typed.source(), errors);
                codes = ParseOutcome::Rejected;
            }
            SourceSlotView::Absent(never) | SourceSlotView::Unexpected(never) => match never {},
        }
    }
    surface_displaced(
        &children.children().unexpected,
        "id_languages",
        typed.source(),
        errors,
    );
    if !children.children().unexpected.is_empty() {
        codes = ParseOutcome::Rejected;
    }
    Ok(codes.map(LanguageCodes::new))
}
