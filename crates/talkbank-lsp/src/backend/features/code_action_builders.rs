use tower_lsp::lsp_types::*;

use super::ProposedQuickFix;

fn quick_fix(
    title: impl Into<String>,
    range: Range,
    new_text: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> ProposedQuickFix {
    ProposedQuickFix {
        title: title.into(),
        diagnostic: diagnostic.cloned(),
        edit: TextEdit {
            range,
            new_text: new_text.into(),
        },
    }
}

pub(super) fn replace_range(
    range: Range,
    replacement: impl Into<String>,
    title: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> ProposedQuickFix {
    quick_fix(title, range, replacement, diagnostic)
}

pub(super) fn replace_diagnostic_range(
    diagnostic: &Diagnostic,
    replacement: impl Into<String>,
    title: impl Into<String>,
) -> ProposedQuickFix {
    replace_range(diagnostic.range, replacement, title, Some(diagnostic))
}

pub(super) fn insert_at(
    position: Position,
    text: impl Into<String>,
    title: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> ProposedQuickFix {
    replace_range(
        Range {
            start: position,
            end: position,
        },
        text,
        title,
        diagnostic,
    )
}
