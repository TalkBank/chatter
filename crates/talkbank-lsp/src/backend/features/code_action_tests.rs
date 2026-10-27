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

// These unit tests exercise fix policy and provisional geometry only. Real
// revision admission, capability negotiation and wire edits live in stdio tests.
fn proposals(diagnostics: Vec<Diagnostic>) -> Option<Vec<ProposedQuickFix>> {
    let actions: Vec<_> = diagnostics
        .iter()
        .flat_map(fixes::actions_for_diagnostic)
        .collect();
    (!actions.is_empty()).then_some(actions)
}

/// Extract the single provisional text edit, not an admitted workspace edit.
fn extract_edit(actions: &[ProposedQuickFix]) -> TextEdit {
    assert_eq!(actions.len(), 1);
    actions[0].edit.clone()
}

#[test]
fn test_undeclared_speaker_requires_explicit_participant_facts() {
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        5,
        1,
        4,
    );

    assert!(proposals(vec![diag]).is_none());
}

#[test]
fn header_fixes_require_catalog_evidence() {
    for code in ["E502", "E503"] {
        assert!(proposals(vec![make_diagnostic(code, "missing header", 0, 0, 0)]).is_none());
    }
}

#[test]
fn test_fix_undeclared_speaker_no_participants_line() {
    let diag = make_diagnostic(
        "E308",
        "Speaker 'FOO' is not in the participant list",
        2,
        1,
        4,
    );

    let actions = proposals(vec![diag]);
    assert!(actions.is_none());
}

#[test]
fn empty_utterance_requires_catalog_evidence() {
    let diag = make_diagnostic("E306", "Empty utterance", 2, 0, 5);

    assert!(proposals(vec![diag]).is_none());
}

#[test]
fn test_empty_languages_requires_explicit_language_facts() {
    let diag = make_diagnostic("E507", "Empty @Languages header", 3, 0, 11);

    assert!(proposals(vec![diag]).is_none());
}

#[test]
fn missing_terminator_requires_catalog_evidence() {
    let diag = make_diagnostic("E305", "Missing terminator in main tier", 1, 0, 10);

    assert!(proposals(vec![diag]).is_none());
}

#[test]
fn test_no_actions_for_unknown_code() {
    let diag = make_diagnostic("E999", "Unknown error", 0, 0, 5);

    let actions = proposals(vec![diag]);
    assert!(actions.is_none());
}

#[test]
fn test_fix_comma_after_non_spoken_removes_it() {
    let diag = make_diagnostic("E259", "Comma after non-spoken", 0, 15, 16);

    let actions = proposals(vec![diag]).unwrap();
    let edit = extract_edit(&actions);
    assert_eq!(edit.new_text, "");
}

#[test]
fn test_missing_participants_requires_explicit_participant_facts() {
    let diag = make_diagnostic("E504", "Missing required header: @Participants", 0, 0, 0);

    assert!(proposals(vec![diag]).is_none());
}

#[test]
fn test_fix_e504_non_participants_ignored() {
    let diag = make_diagnostic("E504", "Missing required header: @Languages", 0, 0, 0);

    let actions = proposals(vec![diag]);
    assert!(actions.is_none());
}

/// E241 joined this list on 2026-08-15. Its arm spliced the literal `"xxx"`
/// under the title "Replace 'xx' with 'xxx'", so it ALREADY rewrote a `YYY` or a
/// `WWW` to `xxx`, turning "requires phonetic transcription" into
/// "unintelligible" on the user's behalf. That was wrong before the rule moved;
/// widening E241 to the shortened forms only added `ww` to the set it damages.
/// (An earlier draft of this note said the arm "became" wrong when the rule
/// widened. It did not, and the distinction matters: this is a defect the audit
/// missed, not one a later change introduced.)
///
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
    let removed_codes = [
        "E241", // IllegalUntranscribed: hardcoded "xxx", so it rewrote a `YYY` or `ww` to `xxx`.
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
        let actions = proposals(vec![diag]);
        assert!(
            actions.is_none(),
            "{code} must offer no code action (see audit report); got {actions:?}"
        );
    }
}

#[test]
fn raw_line_diagnostics_cannot_issue_deletion() {
    let overflow = make_diagnostic("E306", "Empty utterance", u32::MAX, 0, 5);
    assert!(proposals(vec![overflow]).is_none());
    let final_line = make_diagnostic("E306", "Empty utterance", 2, 0, 6);
    assert!(proposals(vec![final_line]).is_none());
}
