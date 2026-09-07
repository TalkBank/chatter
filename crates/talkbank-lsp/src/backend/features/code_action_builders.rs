use tower_lsp::lsp_types::*;

fn quick_fix(
    uri: &Url,
    title: impl Into<String>,
    range: Range,
    new_text: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> CodeAction {
    CodeAction {
        title: title.into(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: diagnostic.map(|item| vec![item.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    uri.clone(),
                    vec![TextEdit {
                        range,
                        new_text: new_text.into(),
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub(super) fn replace_range(
    uri: &Url,
    range: Range,
    replacement: impl Into<String>,
    title: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> CodeAction {
    quick_fix(uri, title, range, replacement, diagnostic)
}

pub(super) fn replace_diagnostic_range(
    uri: &Url,
    diagnostic: &Diagnostic,
    replacement: impl Into<String>,
    title: impl Into<String>,
) -> CodeAction {
    replace_range(uri, diagnostic.range, replacement, title, Some(diagnostic))
}

pub(super) fn insert_at(
    uri: &Url,
    position: Position,
    text: impl Into<String>,
    title: impl Into<String>,
    diagnostic: Option<&Diagnostic>,
) -> CodeAction {
    replace_range(
        uri,
        Range {
            start: position,
            end: position,
        },
        text,
        title,
        diagnostic,
    )
}

pub(super) fn delete_diagnostic_line(
    uri: &Url,
    diagnostic: &Diagnostic,
    title: impl Into<String>,
) -> CodeAction {
    let line = diagnostic.range.start.line;
    replace_range(
        uri,
        Range {
            start: Position { line, character: 0 },
            end: Position {
                line: line + 1,
                character: 0,
            },
        },
        String::new(),
        title,
        Some(diagnostic),
    )
}
