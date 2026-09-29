use talkbank_model::ErrorCode;
use tower_lsp::lsp_types::*;

use super::builders::{delete_diagnostic_line, insert_at, replace_diagnostic_range};

/// Build the quick-fix actions offered for one diagnostic, if its code has
/// a verified fix.
///
/// Every arm here was checked against a real `chatter validate` run on a
/// fixture that actually triggers the code. A code with no arm here,
/// whether never considered or found (2026-07-31 audit) to offer the wrong
/// fix, falls through to `Vec::new()`; nothing here invents a default
/// action for a code it does not recognize. `test_removed_codes_offer_no_action`
/// in the sibling test module names every code that was removed and why.
/// The rationale for several of the same exclusions is written out in more
/// detail in `talkbank_transform::splice::catalog`'s module docs, the
/// shared fix catalog this LSP module has not yet migrated onto.
///
/// This routes on [`ErrorCode`] variants, not the wire-protocol code
/// string, specifically so that renumbering an `ErrorCode` (changing its
/// `#[code("E...")]` attribute) breaks this match at compile time instead
/// of silently detaching a case from the diagnostic it used to fire for.
/// The string comparison this replaced (`"E501" => ...`) was exactly how
/// several of the removed cases went stale: a code's meaning moved to a
/// different number and nothing here noticed.
pub(super) fn actions_for_diagnostic(
    uri: &Url,
    diagnostic: &Diagnostic,
    doc: Option<&str>,
) -> Vec<CodeAction> {
    let Some(code) = diagnostic_code(diagnostic) else {
        return Vec::new();
    };

    match code {
        ErrorCode::MissingTerminator => missing_terminator_actions(uri, diagnostic),
        // A diagnostic establishes missing facts, not values for those facts.
        // Keep the same refusal policy as the shared fix catalog: no role,
        // participant or language may be constructed from diagnostic prose.
        ErrorCode::UndeclaredSpeaker
        | ErrorCode::MissingRequiredHeader
        | ErrorCode::EmptyLanguagesHeader => Vec::new(),
        ErrorCode::MissingEndHeader => doc
            .map(|text| insert_at_end(uri, text, "@End\n", "Insert '@End' at end of file"))
            .into_iter()
            .collect(),
        ErrorCode::MissingUTF8Header => vec![insert_at_start(
            uri,
            "@UTF8\n",
            "Insert '@UTF8' at start of file",
        )],
        ErrorCode::EmptyUtterance => vec![delete_diagnostic_line(
            uri,
            diagnostic,
            "Delete empty utterance",
        )],
        ErrorCode::CommaAfterNonSpokenContent => vec![replace_diagnostic_range(
            uri,
            diagnostic,
            "",
            "Remove comma after non-spoken content",
        )],
        _ => Vec::new(),
    }
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<ErrorCode> {
    match &diagnostic.code {
        Some(NumberOrString::String(code)) => ErrorCode::parse_exact(code),
        _ => None,
    }
}

fn missing_terminator_actions(uri: &Url, diagnostic: &Diagnostic) -> Vec<CodeAction> {
    [
        (".", "Add '.' (declarative/default)"),
        ("?", "Add '?' (question)"),
        ("!", "Add '!' (exclamation)"),
    ]
    .into_iter()
    .map(|(terminator, title)| {
        insert_at(
            uri,
            diagnostic.range.end,
            format!(" {terminator}"),
            title,
            Some(diagnostic),
        )
    })
    .collect()
}

fn insert_at_end(uri: &Url, doc: &str, text: &str, title: &str) -> CodeAction {
    let insert_text = if doc.ends_with('\n') {
        text.to_string()
    } else {
        format!("\n{text}")
    };

    insert_at(
        uri,
        crate::backend::utils::offset_to_position(doc, doc.len() as u32),
        insert_text,
        title,
        None,
    )
}

fn insert_at_start(uri: &Url, text: &str, title: &str) -> CodeAction {
    insert_at(
        uri,
        Position {
            line: 0,
            character: 0,
        },
        text,
        title,
        None,
    )
}
