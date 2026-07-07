//! Parsing for `@Languages` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::generated_traversal::{
    AsRawNode, LanguagesContentsNode, LanguagesHeaderNode, NodeSlot, extract_languages_contents,
    extract_languages_header,
};
use crate::node_types::LANGUAGES_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, LanguageCode, WarningText};

/// Build `Header::Unknown` for malformed `@Languages` input.
fn unknown_languages_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Languages".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Languages:\t<code>[, <code>...]".to_string()),
    }
}

/// Decode UTF-8 child text for a language-code field, delegating to the
/// shared `decode_present_child` helper.
///
/// The CHAT source handed to the parser is already valid UTF-8 in practice
/// (it is a `&str`), so the error arm is defensive only; kept for parity with
/// the same pattern used by the other migrated header-internals files (e.g.
/// `metadata/media.rs`).
fn decode_child_text(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<String> {
    decode_present_child(child, source, errors, context, |err| {
        format!("Failed to extract UTF-8 text from {}: {}", context, err)
    })
}

/// Push a decoded language-code text onto `codes`, reporting (rather than
/// panicking on) the grammar-impossible empty-text case.
///
/// A `Present` `language_code` node always matches the grammar's
/// `/[a-z]{2,4}/`, so its text is non-empty in practice (a zero-width
/// placeholder is classified `NodeSlot::Missing` by the generated extractor,
/// never `Present`). The `Err` arm exists so the parser stays panic-free and
/// non-silent even if that invariant were ever violated, per the no-panic /
/// no-silent-drop policy.
fn push_language_code(
    codes: &mut Vec<LanguageCode>,
    node: Node,
    text: String,
    source: &str,
    errors: &impl ErrorSink,
) {
    match LanguageCode::new(text) {
        Ok(code) => codes.push(code),
        Err(error) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "language_code"),
                format!("Invalid @Languages code: {error}"),
            ));
        }
    }
}

/// Parse Languages header from tree-sitter node
///
/// **Grammar Rule**:
/// ```javascript
/// languages_header: $ => seq(
///     token('@Languages:\t'),
///     $.languages_contents,
///     $.newline
/// )
///
/// languages_contents: $ => seq(
///     $.language_code,
///     repeat(seq(optional($.whitespaces), $.comma, $.whitespaces, $.language_code))
/// )
/// ```
///
/// **Migration note (Task B2-followup, fully migrated).** Both the OUTER
/// access to `languages_contents` (`child_2` of `languages_header`) and the
/// INNER language-code list are typed via the NEW backend's free
/// `extract_languages_header` / `extract_languages_contents`. The first
/// (non-repeated) `language_code` is typed `child_0`; the remaining
/// comma-separated codes are the typed repeat `child_1`, each element a
/// `LanguagesContentsChild1Children` group of
/// `(optional(whitespaces), comma, whitespaces, language_code)`. Every
/// `NodeSlot` is matched exhaustively (`Present`/`Missing`/`Error`/
/// `Unexpected`/`Absent`, no `_ =>`), so a tree-sitter MISSING `language_code`
/// placeholder is now a TYPE-DISTINCT `NodeSlot::Missing` value that can never
/// reach [`LanguageCode::new`]: it is reported as a `MissingRequiredElement`
/// diagnostic instead, matching the diagnostic `check_not_missing` emits for
/// the analogous first-entry position in `@Participants`
/// (`tree_parsing/header/participants.rs`). This fixes a pre-existing panic
/// (`LanguageCode::new` asserts non-empty; the prior un-migrated raw-node walk
/// distinguished MISSING from Present only by `.kind()`, which both share for
/// a MISSING `language_code` placeholder, and read the MISSING node's empty
/// text straight into the constructor). Empirically confirmed reachable via
/// `@Languages:\t\n` (an entirely empty `languages_contents`, which
/// tree-sitter fills with a zero-width MISSING `language_code` at `child_0`);
/// see `tests/header_internals_migration.rs`'s `LANGUAGES_EMPTY_CONTENTS`.
pub fn parse_languages_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    let mut codes = Vec::new();

    // Verify this is a languages_header node
    if node.kind() != LANGUAGES_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected languages_header node, got: {}", node.kind()),
        ));
        return unknown_languages_header(
            node,
            source,
            "Languages header CST node had unexpected kind",
        );
    }

    // The language-list parsing below only descends into `languages_contents`;
    // the shared header scan reports any structural ERROR/MISSING node
    // tree-sitter parked elsewhere under the header (e.g. a trailing comma) so
    // it is never silently swallowed. Like `@Participants`, `@Languages` is a
    // comma-separated list, so a dangling comma becomes an `(ERROR (comma))`
    // sibling of `languages_contents`.
    super::super::report_header_structural_errors(node, LANGUAGES_HEADER, source, errors);

    // Extract `languages_contents` via typed slot `child_2` of the
    // `languages_header` (unchanged index from the OLD module).
    // `extract_languages_header` strips structural nodes (prefix, header_sep,
    // newline) and exposes `languages_contents` as a `NodeSlot`;
    // `present_or_recover().ok()` keeps only a Present node and funnels every
    // non-Present recovery state to the same "Missing languages_contents"
    // diagnostic.
    let children = extract_languages_header(LanguagesHeaderNode(node));
    let Some(contents_node) = children.child_2.slot.present_or_recover().ok() else {
        errors.report(ParseError::new(
            ErrorCode::EmptyLanguagesHeader,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                "languages_header",
            ),
            "Missing languages_contents in @Languages header",
        ));
        surface_unexpected(&children.unexpected, source, errors);
        return unknown_languages_header(
            node,
            source,
            "Missing languages_contents in @Languages header",
        );
    };
    let contents = contents_node.raw_node();
    surface_unexpected(&children.unexpected, source, errors);

    // Decompose `languages_contents` into its typed child slots: `child_0` is
    // the required first `language_code`; `child_1` is the typed repeat of
    // `(optional(whitespaces), comma, whitespaces, language_code)` groups.
    let contents_children = extract_languages_contents(LanguagesContentsNode(contents));

    // First language code (required, typed child_0). This is the exact
    // position the pre-existing panic bug lived at: a MISSING placeholder here
    // is now a type-distinct `NodeSlot::Missing`, never reaching
    // `LanguageCode::new`.
    match contents_children.child_0.slot {
        NodeSlot::Present(code_node) => {
            if let ParseOutcome::Parsed(text) =
                decode_child_text(code_node.raw_node(), source, errors, "language_code")
            {
                push_language_code(&mut codes, code_node.raw_node(), text, source, errors);
            }
        }
        NodeSlot::Missing(missing_node) => {
            // The fix: report the same "MISSING required element" diagnostic
            // `check_not_missing` emits for the analogous first-@Participants
            // position, instead of ever calling `LanguageCode::new` on the
            // placeholder's empty text.
            check_not_missing(missing_node, source, errors, "languages_contents");
        }
        NodeSlot::Error(bad) | NodeSlot::Unexpected(bad) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                ErrorContext::new(
                    source,
                    bad.start_byte()..bad.end_byte(),
                    "languages_contents",
                ),
                format!(
                    "Expected 'language_code' as the first @Languages code, got: {}",
                    bad.kind()
                ),
            ));
        }
        NodeSlot::Absent => {
            // No node at all: mirrors the OLD raw-node walk's silent
            // `if idx < child_count` short-circuit when `languages_contents`
            // has no children whatsoever. Not observed in practice (an empty
            // `languages_contents` yields a MISSING `language_code`, above,
            // not a childless node), kept for exhaustive `NodeSlot` coverage.
        }
    }

    // Subsequent language codes (optional typed repeat, child_1): each
    // element is a `LanguagesContentsChild1Children` group of
    // `(optional(whitespaces), comma, whitespaces, language_code)`. The
    // generator's recovery-aware repeat classifies each OUTER item as
    // `Present`/`Error`/`Absent` only (never `Missing`/`Unexpected` at the
    // item level; see `generated_traversal.rs`'s
    // `extract_languages_contents`), but every state is still matched
    // exhaustively per the project rule against `_ =>` on typed enums.
    for item in &contents_children.child_1.slot {
        match &item.slot {
            NodeSlot::Present(group) => {
                // Leading whitespace before the comma is purely structural
                // (matches the OLD walk's silent skip loop): every state is a
                // no-op, matched explicitly so it is never silently dropped.
                match group.child_0.slot.clone() {
                    None => {}
                    Some(
                        NodeSlot::Present(_)
                        | NodeSlot::Missing(_)
                        | NodeSlot::Error(_)
                        | NodeSlot::Unexpected(_)
                        | NodeSlot::Absent,
                    ) => {}
                }

                // Comma (structural, required within a Present group).
                match group.child_1.slot.clone() {
                    NodeSlot::Present(_) => {}
                    NodeSlot::Missing(missing_node) => {
                        check_not_missing(missing_node, source, errors, "languages_contents");
                    }
                    NodeSlot::Error(bad) | NodeSlot::Unexpected(bad) => {
                        errors.report(ParseError::new(
                            ErrorCode::EmptyLanguagesHeader,
                            Severity::Error,
                            SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                            ErrorContext::new(
                                source,
                                bad.start_byte()..bad.end_byte(),
                                "languages_contents",
                            ),
                            format!("Expected ',' in @Languages code list, got: {}", bad.kind()),
                        ));
                    }
                    NodeSlot::Absent => {}
                }

                // Whitespace after the comma (structural, required within a
                // Present group).
                match group.child_2.slot.clone() {
                    NodeSlot::Present(_) => {}
                    NodeSlot::Missing(missing_node) => {
                        check_not_missing(missing_node, source, errors, "languages_contents");
                    }
                    NodeSlot::Error(bad) | NodeSlot::Unexpected(bad) => {
                        errors.report(ParseError::new(
                            ErrorCode::EmptyLanguagesHeader,
                            Severity::Error,
                            SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                            ErrorContext::new(
                                source,
                                bad.start_byte()..bad.end_byte(),
                                "languages_contents",
                            ),
                            format!(
                                "Expected whitespace after ',' in @Languages code list, got: {}",
                                bad.kind()
                            ),
                        ));
                    }
                    NodeSlot::Absent => {}
                }

                // The repeated language code: the SAME fix as `child_0` above
                // applies here (a MISSING placeholder is type-distinct and
                // never reaches `LanguageCode::new`).
                match group.child_3.slot.clone() {
                    NodeSlot::Present(code_node) => {
                        if let ParseOutcome::Parsed(text) =
                            decode_child_text(code_node.raw_node(), source, errors, "language_code")
                        {
                            push_language_code(
                                &mut codes,
                                code_node.raw_node(),
                                text,
                                source,
                                errors,
                            );
                        }
                    }
                    NodeSlot::Missing(missing_node) => {
                        check_not_missing(missing_node, source, errors, "languages_contents");
                    }
                    NodeSlot::Error(bad) | NodeSlot::Unexpected(bad) => {
                        errors.report(ParseError::new(
                            ErrorCode::EmptyLanguagesHeader,
                            Severity::Error,
                            SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                            ErrorContext::new(
                                source,
                                bad.start_byte()..bad.end_byte(),
                                "languages_contents",
                            ),
                            format!(
                                "Expected 'language_code' in @Languages code list, got: {}",
                                bad.kind()
                            ),
                        ));
                    }
                    NodeSlot::Absent => {}
                }

                // The group's own unexpected sink (spec Section 7): surfaced
                // independently of the outer `languages_contents` sink below,
                // matching the B2 `Bg`/`Eg`/`@Media`-status nested-group
                // precedent.
                surface_unexpected(&group.unexpected, source, errors);
            }
            NodeSlot::Error(bad) => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyLanguagesHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                    ErrorContext::new(
                        source,
                        bad.start_byte()..bad.end_byte(),
                        "languages_contents",
                    ),
                    "Unparsable content in @Languages code list".to_string(),
                ));
            }
            NodeSlot::Missing(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
                // Unreachable for this repeat shape: the generator classifies
                // a whole repeat item as `Present`/`Error`/`Absent` only (see
                // `generated_traversal.rs`'s `extract_languages_contents`
                // repeat-count loop), never `Missing`/`Unexpected` at the item
                // level. Matched exhaustively for type-safety regardless.
            }
        }
    }

    surface_unexpected(&contents_children.unexpected, source, errors);

    Header::Languages {
        codes: codes.into(),
    }
}
