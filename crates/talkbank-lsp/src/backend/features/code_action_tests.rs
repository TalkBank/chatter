use super::*;

fn make_diagnostic(code: &str, message: &str, line: u32, start: u32, end: u32) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line,
                character: start,
            },
            end: Position {
                line,
                character: end,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some("talkbank".to_string()),
        message: message.to_string(),
        ..Default::default()
    }
}

/// Helper: extract the single text edit from a one-action result.
fn extract_edit(actions: &[CodeActionOrCommand], uri: &Url) -> TextEdit {
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        CodeActionOrCommand::CodeAction(action) => {
            let edit = action.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            changes[uri][0].clone()
        }
        _ => panic!("Expected CodeAction"),
    }
}

#[test]
fn test_fix_undeclared_speaker_adds_to_participants() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n*CHI:\thello .\n*FOO:\thi .\n@End\n";
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        5,
        1,
        4,
    );

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, ", FOO Participant");
    assert_eq!(edit.range.start.line, 3);
}

#[test]
fn test_fix_missing_end_inserts_at_eof() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let diag = make_diagnostic("E502", "Missing @End", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    match &actions[0] {
        CodeActionOrCommand::CodeAction(action) => assert!(action.title.contains("@End")),
        _ => panic!("Expected CodeAction"),
    }
}

#[test]
fn test_fix_missing_utf8_inserts_at_start() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E503", "Missing @UTF8", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.new_text, "@UTF8\n");
}

#[test]
fn test_fix_undeclared_speaker_no_participants_line() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*FOO:\thello .\n@End\n";
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        2,
        1,
        4,
    );

    let actions = code_action(uri, vec![diag], Some(doc));
    assert!(actions.is_none());
}

#[test]
fn test_fix_empty_utterance_deletes_line() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E306", "Empty utterance", 2, 0, 5);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.end.line, 3);
}

#[test]
fn test_fix_empty_languages_header() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E507", "Empty @Languages header", 3, 0, 11);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("eng"));
}

#[test]
fn test_fix_missing_terminator_offers_three_options() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E305", "Missing terminator in main tier", 1, 0, 10);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    assert_eq!(actions.len(), 3); // ., ?, !
}

#[test]
fn test_no_actions_for_unknown_code() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E999", "Unknown error", 0, 0, 5);

    let actions = code_action(uri, vec![diag], None);
    assert!(actions.is_none());
}

#[test]
fn test_fix_comma_after_non_spoken_removes_it() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let diag = make_diagnostic("E259", "Comma after non-spoken", 0, 15, 16);

    let actions = code_action(uri.clone(), vec![diag], None).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert_eq!(edit.new_text, "");
}

#[test]
fn test_fix_e504_participants_only() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
    let diag = make_diagnostic("E504", "Missing required header: @Participants", 0, 0, 0);

    let actions = code_action(uri.clone(), vec![diag], Some(doc)).unwrap();
    let edit = extract_edit(&actions, &uri);
    assert!(edit.new_text.contains("@Participants:"));
}

#[test]
fn test_fix_e504_non_participants_ignored() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
    let diag = make_diagnostic("E504", "Missing required header: @Languages", 0, 0, 0);

    let actions = code_action(uri, vec![diag], Some(doc));
    assert!(actions.is_none());
}

/// Regression guard for the 2026-07-31 LSP code-action audit
/// (`docs/investigations/2026-07-31-lsp-code-actions-fix-the-wrong-diagnostic.md`):
/// every one of these codes previously had an `actions_for_diagnostic` arm
/// whose edit was verified (against a real `chatter validate` run) to
/// repair a DIFFERENT diagnostic than the code names, to be a no-op, to be
/// destructive, or to be unreachable via the current parser. All twelve
/// arms were removed rather than repaired (a correct fix belongs in
/// `talkbank-transform`'s shared splice catalog, not a second copy here).
/// This test exists so a future edit cannot silently reintroduce one of
/// them: routing now matches on `ErrorCode` variants (not wire-protocol
/// strings), so reintroducing a wrong arm requires deliberately naming the
/// variant again, and this test still catches it either way.
#[test]
fn test_removed_codes_offer_no_action() {
    let uri = Url::parse("file:///test.cha").unwrap();
    let removed_codes = [
        "E242", // UnbalancedQuotation: "+..." does not balance a quote.
        "E244", // ConsecutiveStressMarkers: whole-span replace deletes the word.
        "E258", // ConsecutiveCommas: span is one comma; replacing it with itself is a no-op.
        "E301", // MissingMainTier is "Empty speaker code"; terminator fix is unrelated.
        "E312", // UnclosedBracket: unreachable, tree-sitter recovery emits E304/E375 instead.
        "E313", // UnclosedParenthesis: unreachable, tree-sitter recovery emits E316 instead.
        "E322", // EmptyColon: unreachable, and deletes a whole line for one missing token.
        "E323", // MissingColonAfterSpeaker: unreachable, tree-sitter recovery emits E316 instead.
        "E362", // TimestampBackwards: one code covers two rules; the swap fixes one, breaks the other.
        "E501", // DuplicateHeader: "insert @Begin after @UTF8" adds a THIRD @Begin.
        "E506", // EmptyParticipantsHeader: real diagnostics carry Span::DUMMY (0,0).
        "E604", // GraWithoutMor: diagnostic anchors the main tier, not the %gra line; deletes the wrong line.
    ];

    for code in removed_codes {
        let diag = make_diagnostic(code, "placeholder message", 0, 0, 5);
        let actions = code_action(uri.clone(), vec![diag], None);
        assert!(
            actions.is_none(),
            "{code} must offer no code action (see audit report); got {actions:?}"
        );
    }
}
