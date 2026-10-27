//! Admit lexical GRA fields directly into shared grammatical relations.
use crate::source_text::SourceText;
use talkbank_model::model::*;
use talkbank_model::{ErrorCode, ErrorSink, ParseError, ParseOutcome, Severity};

pub(super) fn relation(
    index_text: &str,
    head_text: &str,
    label: &str,
    source: SourceText<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> ParseOutcome<GrammaticalRelation> {
    let index = match index_text.parse::<usize>() {
        Ok(0) => {
            report_gra(
                errors,
                source,
                index_text,
                ErrorCode::InvalidGrammarIndex,
                "Index cannot be 0 (indices are 1-indexed)",
            );
            return ParseOutcome::rejected();
        }
        Ok(index) => index,
        Err(_) => {
            report_gra(
                errors,
                source,
                index_text,
                ErrorCode::MalformedGrammarRelation,
                &format!("Invalid index '{}': must be a positive integer", index_text),
            );
            return ParseOutcome::rejected();
        }
    };
    let head = match head_text.parse::<usize>() {
        Ok(head) => head,
        Err(_) => {
            report_gra(
                errors,
                source,
                head_text,
                ErrorCode::UnexpectedGrammarNode,
                &format!(
                    "Invalid head '{}': must be a non-negative integer",
                    head_text
                ),
            );
            return ParseOutcome::rejected();
        }
    };
    match GrammaticalRelationType::new(label) {
        Ok(label) => ParseOutcome::parsed(GrammaticalRelation::new(index, head, label)),
        Err(error) => {
            report_gra(
                errors,
                source,
                label,
                ErrorCode::MalformedGrammarRelation,
                &error.to_string(),
            );
            ParseOutcome::rejected()
        }
    }
}

/// Report at the actual lexical field; a mismatched source is a producer fault.
fn report_gra(
    errors: &(impl ErrorSink + ?Sized),
    source: SourceText<'_>,
    field: &str,
    code: ErrorCode,
    message: &str,
) {
    match source.span_of(field) {
        Some(span) => errors.report(ParseError::at_span(code, Severity::Error, span, message)),
        None => errors.report(ParseError::at_location(
            ErrorCode::InternalError,
            Severity::Error,
            talkbank_model::DiagnosticLocation::Unlocated,
            format!("GRA field does not belong to its source: {message}"),
        )),
    }
}
