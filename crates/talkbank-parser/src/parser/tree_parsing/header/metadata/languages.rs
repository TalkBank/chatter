//! Parsing for `@Languages` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::generated_traversal::{
    AsRawNode, KindSlot, LanguageCodeNode, LanguagesHeaderNode, NoChild, SlotView,
    extract_languages_contents, extract_languages_header,
};
use crate::node_types::LANGUAGES_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_structure, present, surface_displaced,
};
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, LanguageCode};

/// A list fault retains its grammatical role and offending node together;
/// callers cannot independently choose a diagnostic message for that role.
enum LanguageListFault<'tree> {
    UnexpectedCode {
        node: Node<'tree>,
        position: LanguagePosition,
    },
    ExpectedComma(Node<'tree>),
    ExpectedWhitespace(Node<'tree>),
    UnparsableGroup(Node<'tree>),
}

impl LanguageListFault<'_> {
    fn report(self, source: &str, errors: &impl ErrorSink) {
        let (bad, message) = match self {
            Self::UnexpectedCode { node, position } => {
                let expectation = match position {
                    LanguagePosition::First => "as the first @Languages code",
                    LanguagePosition::Subsequent => "in @Languages code list",
                };
                (
                    node,
                    format!(
                        "Expected 'language_code' {expectation}, got: {}",
                        node.kind()
                    ),
                )
            }
            Self::UnparsableGroup(node) => (
                node,
                "Unparsable content in @Languages code list".to_owned(),
            ),
            Self::ExpectedComma(node) => (
                node,
                format!("Expected ',' in @Languages code list, got: {}", node.kind()),
            ),
            Self::ExpectedWhitespace(node) => (
                node,
                format!(
                    "Expected whitespace after ',' in @Languages code list, got: {}",
                    node.kind()
                ),
            ),
        };
        errors.report(ParseError::new(
            ErrorCode::EmptyLanguagesHeader,
            Severity::Error,
            SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
            ErrorContext::new(source, bad.byte_range(), "languages_contents"),
            message,
        ));
    }
}

/// Admit one typed language-code node to the model. Text cannot be supplied
/// independently of the node used for its diagnostic; recovery slots never
/// enter this transition.
fn parse_language_code(
    typed: &LanguageCodeNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<LanguageCode> {
    let ParseOutcome::Parsed(text) =
        decode_present_child(typed, source, errors, "language_code", |err| {
            format!("Failed to extract UTF-8 text from language_code: {err}")
        })
    else {
        return ParseOutcome::rejected();
    };
    match LanguageCode::new(text) {
        Ok(code) => ParseOutcome::parsed(code),
        Err(error) => {
            let node = typed.raw_node();
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.byte_range(), "language_code"),
                format!("Invalid @Languages code: {error}"),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// The grammatical role fixes the wording; callers cannot provide arbitrary
/// expected-node text independently of the language-code slot.
enum LanguagePosition {
    First,
    Subsequent,
}

/// One admission policy for both required and repeated language-code slots.
/// Recovery never enters the validated model constructor.
fn parse_language_slot(
    slot: &KindSlot<'_, LanguageCodeNode<'_>>,
    position: LanguagePosition,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<LanguageCode> {
    match slot.view() {
        SlotView::Present(code) => parse_language_code(code, source, errors),
        SlotView::Missing(node) => {
            check_not_missing(node, source, errors, "languages_contents");
            ParseOutcome::rejected()
        }
        SlotView::Error(bad) => {
            LanguageListFault::UnexpectedCode {
                node: bad,
                position,
            }
            .report(source, errors);
            ParseOutcome::rejected()
        }
        SlotView::Absent(NoChild) => ParseOutcome::rejected(),
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
/// `NodeSlot` is matched exhaustively over the states its position can
/// produce (`Present`/`Missing`/`Error`/`Absent`, no `_ =>`), so a tree-sitter
/// MISSING `language_code`
/// placeholder is now a TYPE-DISTINCT `NodeSlot::Missing` value that can never
/// reach [`LanguageCode::new`]: it is reported as a `MissingRequiredElement`
/// diagnostic instead, matching the diagnostic `check_not_missing` emits for
/// the analogous first-entry position in `@Participants`
/// (`tree_parsing/header/participants.rs`). This fixes a pre-existing panic
/// (the older `LanguageCode::new` asserted non-empty; the prior raw-node walk
/// distinguished MISSING from Present only by `.kind()`, which both share for
/// a MISSING `language_code` placeholder, and read the MISSING node's empty
/// text straight into the constructor). The current model constructor is
/// fallible; slot admission still keeps recovery distinct from invalid text.
/// Empirically confirmed reachable via
/// `@Languages:\t\n` (an entirely empty `languages_contents`, which
/// tree-sitter fills with a zero-width MISSING `language_code` at `child_0`);
/// see `tests/header_internals_migration.rs`'s `LANGUAGES_EMPTY_CONTENTS`.
pub fn parse_languages_header(
    typed: LanguagesHeaderNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Header {
    let node = typed.raw_node();
    let mut codes = Vec::new();

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
    // `present` keeps only a Present node and funnels every
    // non-Present recovery state to the same "Missing languages_contents"
    // diagnostic.
    let children = extract_languages_header(typed);
    let Some(contents_node) = present(children.child_2.slot()) else {
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
        surface_displaced(&children.unexpected, "languages_header", source, errors);
        return super::super::unknown_header(
            node,
            source,
            "@Languages",
            "Expected @Languages:\t<code>[, <code>...]",
            "Missing languages_contents in @Languages header",
        );
    };
    surface_displaced(&children.unexpected, "languages_header", source, errors);

    // Decompose `languages_contents` into its typed child slots: `child_0` is
    // the required first `language_code`; `child_1` is the typed repeat of
    // `(optional(whitespaces), comma, whitespaces, language_code)` groups.
    let contents_children = extract_languages_contents(*contents_node);

    codes.extend(
        parse_language_slot(
            contents_children.child_0.slot(),
            LanguagePosition::First,
            source,
            errors,
        )
        .into_option(),
    );

    // Subsequent language codes (optional typed repeat, child_1): each
    // element is a `LanguagesContentsChild1Children` group of
    // `(optional(whitespaces), comma, whitespaces, language_code)`. The
    // generator's recovery-aware repeat classifies each OUTER item as
    // `Present`/`Error`/`Absent` only (never `Missing`/`Unexpected` at the
    // item level; see `generated_traversal.rs`'s
    // `extract_languages_contents`), but every state is still matched
    // exhaustively per the project rule against `_ =>` on typed enums.
    for item in contents_children.child_1.slot() {
        match item.slot().view() {
            SlotView::Present(group) => {
                // Optional leading whitespace has no model effect. Structural
                // recovery reporting remains independent of code admission.

                // Comma, then the whitespace after it: structural, required
                // within a Present group, and reported through the shared
                // verb with this list's own words.
                expect_structure(
                    group.child_1.slot(),
                    "languages_contents",
                    source,
                    errors,
                    |bad| {
                        LanguageListFault::ExpectedComma(bad).report(source, errors);
                    },
                );
                expect_structure(
                    group.child_2.slot(),
                    "languages_contents",
                    source,
                    errors,
                    |bad| {
                        LanguageListFault::ExpectedWhitespace(bad).report(source, errors);
                    },
                );

                codes.extend(
                    parse_language_slot(
                        group.child_3.slot(),
                        LanguagePosition::Subsequent,
                        source,
                        errors,
                    )
                    .into_option(),
                );

                // The group's own unexpected sink (spec Section 7): surfaced
                // independently of the outer `languages_contents` sink below,
                // matching the B2 `Bg`/`Eg`/`@Media`-status nested-group
                // precedent.
                surface_displaced(&group.unexpected, "languages_contents", source, errors);
            }
            SlotView::Error(bad) => {
                LanguageListFault::UnparsableGroup(bad).report(source, errors);
            }
            SlotView::Missing(never) => match never {},
            SlotView::Absent(NoChild) => {}
        }
    }

    surface_displaced(
        &contents_children.unexpected,
        "languages_contents",
        source,
        errors,
    );

    Header::Languages {
        codes: codes.into(),
    }
}
