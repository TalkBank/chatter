//! Parsing for `@Languages` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::CstFailure;
use crate::generated_traversal::{
    AsRawNode, KindSlot, LanguageCodeNode, LanguagesHeaderNode, NoChild, SourceBound, SourceField,
    SourceSlotView,
};
use crate::node_types::LANGUAGES_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};
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
fn parse_language_code<'tree>(
    typed: SourceBound<'tree, '_, LanguageCodeNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<LanguageCode> {
    match LanguageCode::new(typed.text()) {
        Ok(code) => ParseOutcome::parsed(code),
        Err(error) => {
            let node = typed.raw_node();
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(typed.source(), node.byte_range(), "language_code"),
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
fn parse_language_slot<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, LanguageCodeNode<'tree>>>,
    position: LanguagePosition,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<LanguageCode>, CstFailure> {
    let source = slot.source();
    Ok(match slot.view() {
        SourceSlotView::Present(code) => parse_language_code(code.read()?, errors),
        SourceSlotView::Missing(node) => {
            check_not_missing(node.raw_node(), source, errors, "languages_contents");
            ParseOutcome::rejected()
        }
        SourceSlotView::Error(bad) => {
            LanguageListFault::UnexpectedCode {
                node: bad.raw_node(),
                position,
            }
            .report(source, errors);
            ParseOutcome::rejected()
        }
        SourceSlotView::Absent(NoChild) => ParseOutcome::rejected(),
        SourceSlotView::Unexpected(never) => match never {},
    })
}

/// Check a source-associated separator without treating recovery as lexical text.
fn expect_language_separator<'tree, T: AsRawNode<'tree>>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, T>>,
    errors: &impl ErrorSink,
    on_bad: impl FnOnce(Node<'tree>),
) {
    match slot.view() {
        SourceSlotView::Present(_) | SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Missing(node) => {
            check_not_missing(node.raw_node(), node.source(), errors, "languages_contents");
        }
        SourceSlotView::Error(node) => on_bad(node.raw_node()),
        SourceSlotView::Unexpected(never) => match never {},
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
/// The header, contents, and repeated code fields retain their canonical source.
/// Only present, readable code nodes enter the validated model constructor;
/// missing/error slots retain their existing CHAT recovery diagnostics. Producer
/// or source-binding faults propagate separately as internal failures.
pub fn parse_languages_header<'tree>(
    typed: SourceBound<'tree, '_, LanguagesHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, CstFailure> {
    let node = typed.raw_node();
    let source = typed.source();
    let mut codes = Vec::new();

    // The language-list parsing below only descends into `languages_contents`;
    // the shared header scan reports any structural ERROR/MISSING node
    // tree-sitter parked elsewhere under the header (e.g. a trailing comma) so
    // it is never silently swallowed. Like `@Participants`, `@Languages` is a
    // comma-separated list, so a dangling comma becomes an `(ERROR (comma))`
    // sibling of `languages_contents`.
    super::super::report_header_structural_errors(node, LANGUAGES_HEADER, source, errors);

    // Only present contents enter list admission. Preserve the existing
    // missing-contents diagnostic for every recovery state.
    let grammar = crate::parser::typed_cst::canonical_grammar()?;
    let children = typed.extract_admitted(grammar)?;
    let SourceSlotView::Present(contents_node) = children.field_child_2().slot().view() else {
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
        surface_displaced(
            &children.children().unexpected,
            "languages_header",
            source,
            errors,
        );
        return super::super::unknown_header(
            node,
            source,
            "@Languages",
            "Expected @Languages:\t<code>[, <code>...]",
            "Missing languages_contents in @Languages header",
        );
    };
    surface_displaced(
        &children.children().unexpected,
        "languages_header",
        source,
        errors,
    );

    // Decompose `languages_contents` into its typed child slots: `child_0` is
    // the required first `language_code`; `child_1` is the typed repeat of
    // `(optional(whitespaces), comma, whitespaces, language_code)` groups.
    let contents_children = contents_node.read()?.extract_admitted(grammar)?;

    codes.extend(
        parse_language_slot(
            contents_children.field_child_0().slot(),
            LanguagePosition::First,
            errors,
        )?
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
    for item in contents_children.field_child_1().slot().iter() {
        match item.slot().view() {
            SourceSlotView::Present(group) => {
                // Optional leading whitespace has no model effect. Structural
                // recovery reporting remains independent of code admission.

                // Comma, then the whitespace after it: structural, required
                // within a Present group, and reported through the shared
                // verb with this list's own words.
                expect_language_separator(group.field_child_1().slot(), errors, |bad| {
                    LanguageListFault::ExpectedComma(bad).report(source, errors);
                });
                expect_language_separator(group.field_child_2().slot(), errors, |bad| {
                    LanguageListFault::ExpectedWhitespace(bad).report(source, errors);
                });

                codes.extend(
                    parse_language_slot(
                        group.field_child_3().slot(),
                        LanguagePosition::Subsequent,
                        errors,
                    )?
                    .into_option(),
                );

                // The group's own unexpected sink (spec Section 7): surfaced
                // independently of the outer `languages_contents` sink below,
                // matching the B2 `Bg`/`Eg`/`@Media`-status nested-group
                // precedent.
                for displaced in group.field_unexpected().iter() {
                    surface_displaced(
                        &[displaced.raw_node()],
                        "languages_contents",
                        displaced.source(),
                        errors,
                    );
                }
            }
            SourceSlotView::Error(bad) => {
                LanguageListFault::UnparsableGroup(bad.raw_node()).report(bad.source(), errors);
            }
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
            SourceSlotView::Absent(never) => match never {},
        }
    }

    surface_displaced(
        &contents_children.children().unexpected,
        "languages_contents",
        source,
        errors,
    );

    Ok(Header::Languages {
        codes: codes.into(),
    })
}
