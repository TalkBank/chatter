//! `@Participants` header parsing, over the generated typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>
//!
//! **Grammar Rule**:
//! ```javascript
//! participants_header: $ => seq(
//!     $.participants_prefix, $.header_sep,
//!     $.participants_contents,
//!     $.newline
//! )
//!
//! participants_contents: $ => seq(
//!     $.participant,
//!     repeat(seq($.comma, $.whitespaces, $.participant))
//! )
//!
//! participant: $ => seq(
//!     field('code', $.speaker),
//!     repeat(seq($.whitespaces, $.participant_word)),
//!     optional($.whitespaces)
//! )
//! ```
//!
//! `extract_participants_contents` places the first participant in `child_0`
//! and every `(comma, whitespaces, participant)` group of the repeat in
//! `child_1`; `extract_participant` places the speaker code in `code`, every
//! `(whitespaces, participant_word)` group in `child_1` and the optional
//! trailing whitespace in `child_2`. Each slot is matched exhaustively, so a
//! tree-sitter MISSING placeholder or ERROR node at a position is a
//! type-distinct value with its own arm, and nothing here reads
//! `node.kind()`.
//!
//! Fixed-kind slots retain Present, Missing, Error and Absent; their
//! Unexpected payload is uninhabited. Whole sequence-repeat items also
//! have an uninhabited Missing payload. Other recovery states remain even
//! when finite fixtures have not witnessed them. The retained corpus does
//! witness a MISSING speaker and a doubled-comma ERROR repeat item.
//!
//! Until 2026-09-08 this file hand-walked both repeats with an index
//! (`child.kind() == COMMA`, `idx += 1`) with duplicated structural diagnostics.
//! Source-bound extraction now also keeps tree/source identity through every
//! participant group and leaf. Canonical range admission remains fallible at
//! the shared read boundary; participant lowering cannot accept a separately
//! supplied string. No recovery state is removed because a finite corpus has
//! not reached it.

use crate::generated_traversal::{
    AsRawNode, KindSlot, NoChild, ParticipantNode, ParticipantsHeaderNode, SourceBound,
    SourceField, SourceSlotView,
};
use crate::node_types::{PARTICIPANT, PARTICIPANTS_CONTENTS, PARTICIPANTS_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};
use crate::parser::typed_cst::read_source_field;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{
    Header, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};

/// Parse a `@Participants` header into [`Header::Participants`].
///
/// Every participant entry that parses is kept; an entry that does not
/// (a missing code, a missing role) is reported and dropped, so the header
/// still names the speakers it can, and the cross-header checks (E522,
/// E523) work from those.
pub fn parse_participants_header<'tree>(
    typed: SourceBound<'tree, '_, ParticipantsHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Header {
    let node = typed.raw_node();
    let source = typed.source();

    // The list parsing below only descends into `participants_contents`; the
    // shared header scan reports any structural ERROR/MISSING node
    // tree-sitter parked elsewhere under the header (a trailing comma, say)
    // so it is never silently swallowed.
    super::report_header_structural_errors(node, PARTICIPANTS_HEADER, source, errors);

    let header_children = typed.extract();
    let contents_node = match header_children.field_child_2().slot().view() {
        SourceSlotView::Present(node) => read_source_field(node, errors),
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => None,
    };
    let Some(contents_node) = contents_node else {
        errors.report(ParseError::new(
            ErrorCode::EmptyParticipantsHeader,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                PARTICIPANTS_HEADER,
            ),
            "Missing participants_contents in @Participants header",
        ));
        surface_displaced(
            &header_children.children().unexpected,
            PARTICIPANTS_HEADER,
            source,
            errors,
        );
        return super::unknown_header(
            node,
            source,
            "@Participants",
            "Expected @Participants:\tCODE [NAME] ROLE[, ...]",
            "Missing participants_contents in @Participants header",
        );
    };
    surface_displaced(
        &header_children.children().unexpected,
        PARTICIPANTS_HEADER,
        source,
        errors,
    );

    let contents = contents_node.extract();
    let rest = contents.field_child_1().slot().iter();
    let mut entries = Vec::with_capacity(rest.len() + 1);

    entries.extend(
        parse_participant_slot(
            contents.field_child_0().slot(),
            ParticipantPosition::First(typed.node()),
            errors,
        )
        .into_option(),
    );

    // Every further participant arrives as a `(comma, whitespaces,
    // participant)` group. The comma and the whitespace are structure: a
    // Present one needs nothing, a MISSING one is reported as the recovery
    // it is, an ERROR or displaced node at either position is the list
    // losing its shape.
    for item in rest {
        match item.slot().view() {
            SourceSlotView::Present(group) => {
                expect_source_structure(
                    group.field_child_0().slot(),
                    PARTICIPANTS_CONTENTS,
                    errors,
                    |bad| {
                        ParticipantFault::Comma(bad).report(source, errors);
                    },
                );
                expect_source_structure(
                    group.field_child_1().slot(),
                    PARTICIPANTS_CONTENTS,
                    errors,
                    |bad| {
                        ParticipantFault::Whitespace(bad).report(source, errors);
                    },
                );
                entries.extend(
                    parse_participant_slot(
                        group.field_child_2().slot(),
                        ParticipantPosition::Subsequent,
                        errors,
                    )
                    .into_option(),
                );
                surface_source_displaced(group.field_unexpected(), PARTICIPANTS_CONTENTS, errors);
            }
            // A doubled comma (`CHI Target_Child,, MOT Mother`) lands here:
            // the ERROR node holding the stray comma fills a whole repeat
            // item. Spec: `E506.md#3`.
            SourceSlotView::Error(bad) => {
                ParticipantFault::Comma(bad.raw_node()).report(bad.source(), errors);
            }
            SourceSlotView::Absent(NoChild) => {}
        }
    }
    surface_displaced(
        &contents.children().unexpected,
        PARTICIPANTS_CONTENTS,
        source,
        errors,
    );

    Header::Participants {
        entries: entries.into(),
    }
}

/// Structural positions need no text read, but retain their recovery policy
/// and derive diagnostic source identity from the generated field itself.
fn expect_source_structure<'tree, T: AsRawNode<'tree> + Copy>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, T>>,
    context: &str,
    errors: &impl ErrorSink,
    on_bad: impl FnOnce(Node<'tree>),
) {
    match slot.view() {
        SourceSlotView::Present(_) | SourceSlotView::Absent(_) => {}
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), missing.source(), errors, context);
        }
        SourceSlotView::Error(bad) => on_bad(bad.raw_node()),
    }
}

fn surface_source_displaced<'tree>(
    nodes: SourceField<'_, 'tree, '_, Vec<Node<'tree>>>,
    context: &str,
    errors: &impl ErrorSink,
) {
    for node in nodes.iter() {
        surface_displaced(
            std::slice::from_ref(&node.raw_node()),
            context,
            node.source(),
            errors,
        );
    }
}

/// A recovery node with the grammatical role that determines its diagnostic.
/// List and entry faults share location construction, not their error policy.
enum ParticipantFault<'tree> {
    Comma(Node<'tree>),
    Whitespace(Node<'tree>),
    Entry {
        node: Node<'tree>,
        position: ParticipantPosition<'tree>,
    },
    EntryShape(Node<'tree>),
}

impl ParticipantFault<'_> {
    fn report(self, source: &str, errors: &impl ErrorSink) {
        let (node, expected) = match self {
            Self::Comma(node) => (node, Some("',' between participants")),
            Self::Whitespace(node) => (node, Some("whitespace after ','")),
            Self::Entry { node, position } => (
                node,
                Some(match position {
                    ParticipantPosition::First(_) => "a participant entry",
                    ParticipantPosition::Subsequent => "a participant entry after ','",
                }),
            ),
            Self::EntryShape(node) => (node, None),
        };
        let (code, context, message) = match expected {
            Some(expected) => (
                ErrorCode::EmptyParticipantsHeader,
                PARTICIPANTS_CONTENTS,
                format!("Expected {expected} in @Participants, got: {}", node.kind()),
            ),
            None => (
                ErrorCode::UnparsableContent,
                PARTICIPANT,
                format!("Unparsable content in participant entry: {}", node.kind()),
            ),
        };
        errors.report(ParseError::new(
            code,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.byte_range(), context),
            message,
        ));
    }
}

/// Only the first position needs the enclosing header for an absence report.
/// The repeated position cannot accidentally report that the header is empty.
enum ParticipantPosition<'tree> {
    First(ParticipantsHeaderNode<'tree>),
    Subsequent,
}

/// Shared admission for required and repeated participant slots. Recovery
/// stays outside entry conversion, and positional policy remains explicit.
fn parse_participant_slot<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, ParticipantNode<'tree>>>,
    position: ParticipantPosition<'_>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(participant) => match read_source_field(participant, errors) {
            Some(participant) => parse_participant_entry(participant, errors),
            None => ParseOutcome::rejected(),
        },
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, PARTICIPANTS_CONTENTS);
            ParseOutcome::rejected()
        }
        SourceSlotView::Error(bad) => {
            ParticipantFault::Entry {
                node: bad.raw_node(),
                position,
            }
            .report(source, errors);
            ParseOutcome::rejected()
        }
        SourceSlotView::Absent(NoChild) => {
            if let ParticipantPosition::First(header) = position {
                let node = header.raw_node();
                errors.report(ParseError::new(
                    ErrorCode::EmptyParticipantsHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.byte_range(), PARTICIPANTS_CONTENTS),
                    "@Participants header declares no participant",
                ));
            }
            ParseOutcome::rejected()
        }
    }
}

/// Parse a single participant entry: `CODE [NAME ...] ROLE`.
///
/// The last word is the role and every word before it is the name. The
/// `speaker` and `participant_word` tokens are non-empty by their regexes,
/// so the only ways an entry fails are a code the parser had to invent
/// (E342 from the recovery report; the entry is dropped) and no word at all
/// after the code (E513, `@Participants:\tCHI`). E512 has no route here: the
/// first word of an entry is always its code, which `E512.md` records.
fn parse_participant_entry<'tree>(
    typed: SourceBound<'tree, '_, ParticipantNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    let node = typed.raw_node();
    let source = typed.source();
    let children = typed.extract();
    let unexpected = &children.children().unexpected;

    let speaker_code = match children.field_code().slot().view() {
        SourceSlotView::Present(speaker) => {
            let Some(speaker) = read_source_field(speaker, errors) else {
                surface_displaced(unexpected, PARTICIPANT, source, errors);
                return ParseOutcome::rejected();
            };
            speaker.text()
        }
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, PARTICIPANT);
            surface_displaced(unexpected, PARTICIPANT, source, errors);
            return ParseOutcome::rejected();
        }
        SourceSlotView::Error(bad) => {
            let bad = bad.raw_node();
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantCode,
                Severity::Error,
                SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
                ErrorContext::new(source, bad.start_byte()..bad.end_byte(), PARTICIPANT),
                format!(
                    "Expected a speaker code to start the participant entry, got: {}",
                    bad.kind()
                ),
            ));
            surface_displaced(unexpected, PARTICIPANT, source, errors);
            return ParseOutcome::rejected();
        }
        SourceSlotView::Absent(NoChild) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantCode,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), PARTICIPANT),
                "Participant entry missing speaker code",
            ));
            surface_displaced(unexpected, PARTICIPANT, source, errors);
            return ParseOutcome::rejected();
        }
    };

    // Name words and the role, each preceded by whitespace the grammar
    // requires; the whitespace is structure and needs nothing when Present.
    let mut words: Vec<String> = Vec::with_capacity(children.field_child_1().slot().iter().len());
    for item in children.field_child_1().slot().iter() {
        match item.slot().view() {
            SourceSlotView::Present(group) => {
                expect_source_structure(group.field_child_0().slot(), PARTICIPANT, errors, |bad| {
                    ParticipantFault::EntryShape(bad).report(source, errors);
                });
                match group.field_child_1().slot().view() {
                    SourceSlotView::Present(word) => {
                        if let Some(word) = read_source_field(word, errors) {
                            words.push(word.text().to_owned());
                        }
                    }
                    SourceSlotView::Absent(NoChild) => {}
                    SourceSlotView::Missing(missing) => {
                        check_not_missing(missing.raw_node(), source, errors, PARTICIPANT);
                    }
                    SourceSlotView::Error(bad) => {
                        ParticipantFault::EntryShape(bad.raw_node()).report(source, errors);
                    }
                }
                surface_source_displaced(group.field_unexpected(), PARTICIPANT, errors);
            }
            SourceSlotView::Absent(NoChild) => {}
            SourceSlotView::Error(bad) => {
                ParticipantFault::EntryShape(bad.raw_node()).report(source, errors);
            }
        }
    }
    // Trailing whitespace before the comma or newline is tolerated by the
    // grammar and carries nothing, but a node that is not whitespace there
    // is still the entry losing its shape.
    if let Some(trailing) = children.field_child_2().slot().optional() {
        expect_source_structure(trailing, PARTICIPANT, errors, |bad| {
            ParticipantFault::EntryShape(bad).report(source, errors);
        });
    }
    surface_displaced(unexpected, PARTICIPANT, source, errors);

    let Some(role) = words.pop() else {
        errors.report(ParseError::new(
            ErrorCode::EmptyParticipantRole,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), PARTICIPANT),
            "Participant role cannot be empty",
        ));
        return ParseOutcome::rejected();
    };
    let name = if words.is_empty() {
        None
    } else {
        Some(ParticipantName::new(words.join(" ")))
    };

    ParseOutcome::parsed(ParticipantEntry {
        speaker_code: SpeakerCode::new(speaker_code),
        name,
        role: ParticipantRole::new(role),
    })
}
