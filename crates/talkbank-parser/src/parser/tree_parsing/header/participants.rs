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
//! Not every arm can fire. Every position here is a single fixed kind, so
//! the generator classifies it Present, Missing, Error or Absent and never
//! `Unexpected` (that state comes only from a choice position); and a whole
//! repeat item is Present, Error or Absent, never Missing. The `Unexpected`
//! patterns and the item-level `Missing` arms are written out because the
//! enum has those states and this crate bans `_` on it, not because an
//! input reaches them; the reachable recovery arms are the ones the
//! before/after probe exercised: a MISSING `speaker` (`@Participants:\t`)
//! and an ERROR item in the list (a doubled comma, `E506.md#3`).
//!
//! Until 2026-09-08 this file hand-walked both repeats with an index
//! (`child.kind() == COMMA`, `idx += 1`), which needed three copies of an
//! "expected X at position N" reporter of which one was ever reached, a
//! "speaker code cannot be empty" arm the `speaker` token regex makes
//! impossible, and UTF-8 decode failures on a `&str` source. A
//! whole-workspace coverage run showed those arms as most of the file; rule
//! 6 of the repository's CLAUDE.md bans the walk that needed them.

use crate::generated_traversal::{
    AsRawNode, NoChild, ParticipantNode, ParticipantsHeaderNode, SlotView, extract_participant,
    extract_participants_contents, extract_participants_header,
};
use crate::node_types::{PARTICIPANT, PARTICIPANTS_CONTENTS, PARTICIPANTS_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_structure, present, surface_displaced,
};
use crate::parser::typed_cst::decode_present_child;
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
pub fn parse_participants_header(
    typed: ParticipantsHeaderNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Header {
    let node = typed.raw_node();

    // The list parsing below only descends into `participants_contents`; the
    // shared header scan reports any structural ERROR/MISSING node
    // tree-sitter parked elsewhere under the header (a trailing comma, say)
    // so it is never silently swallowed.
    super::report_header_structural_errors(node, PARTICIPANTS_HEADER, source, errors);

    let header_children = extract_participants_header(typed);
    let Some(contents_node) = present(header_children.child_2.slot()) else {
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
            &header_children.unexpected,
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
        &header_children.unexpected,
        PARTICIPANTS_HEADER,
        source,
        errors,
    );

    let contents = extract_participants_contents(*contents_node);
    let rest = contents.child_1.slot();
    let mut entries = Vec::with_capacity(rest.len() + 1);

    // The first participant is required by the grammar, so its only
    // non-Present states are recovery states.
    match contents.child_0.slot().view() {
        SlotView::Present(participant) => {
            push_entry(&mut entries, *participant, source, errors);
        }
        SlotView::Missing(missing) => {
            check_not_missing(missing, source, errors, PARTICIPANTS_CONTENTS);
        }
        SlotView::Error(bad) => {
            report_list_shape(bad, "a participant entry", source, errors);
        }
        // A `participants_contents` node with no first child at all: the
        // header declares nobody. `@Participants:\t` does not reach here
        // (tree-sitter fills that with a MISSING `speaker` inside a present
        // participant); the arm states what an empty list would mean.
        SlotView::Absent(NoChild) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantsHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    PARTICIPANTS_CONTENTS,
                ),
                "@Participants header declares no participant",
            ));
        }
    }

    // Every further participant arrives as a `(comma, whitespaces,
    // participant)` group. The comma and the whitespace are structure: a
    // Present one needs nothing, a MISSING one is reported as the recovery
    // it is, an ERROR or displaced node at either position is the list
    // losing its shape.
    for item in rest {
        match item.slot().view() {
            SlotView::Present(group) => {
                expect_structure(
                    group.child_0.slot(),
                    PARTICIPANTS_CONTENTS,
                    source,
                    errors,
                    |bad| {
                        report_list_shape(bad, "',' between participants", source, errors);
                    },
                );
                expect_structure(
                    group.child_1.slot(),
                    PARTICIPANTS_CONTENTS,
                    source,
                    errors,
                    |bad| {
                        report_list_shape(bad, "whitespace after ','", source, errors);
                    },
                );
                match group.child_2.slot().view() {
                    SlotView::Present(participant) => {
                        push_entry(&mut entries, *participant, source, errors);
                    }
                    SlotView::Absent(NoChild) => {}
                    SlotView::Missing(missing) => {
                        check_not_missing(missing, source, errors, PARTICIPANTS_CONTENTS);
                    }
                    SlotView::Error(bad) => {
                        report_list_shape(bad, "a participant entry after ','", source, errors);
                    }
                }
                surface_displaced(&group.unexpected, PARTICIPANTS_CONTENTS, source, errors);
            }
            // A doubled comma (`CHI Target_Child,, MOT Mother`) lands here:
            // the ERROR node holding the stray comma fills a whole repeat
            // item. Spec: `E506.md#3`.
            SlotView::Error(bad) => {
                report_list_shape(bad, "',' between participants", source, errors);
            }
            SlotView::Absent(NoChild) => {}
        }
    }
    surface_displaced(&contents.unexpected, PARTICIPANTS_CONTENTS, source, errors);

    Header::Participants {
        entries: entries.into(),
    }
}

/// E506 at a node that sits where the participant list expected something
/// else: the list has lost its shape there, whatever the node holds.
fn report_list_shape(bad: Node, expected: &str, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::EmptyParticipantsHeader,
        Severity::Error,
        SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
        ErrorContext::new(
            source,
            bad.start_byte()..bad.end_byte(),
            PARTICIPANTS_CONTENTS,
        ),
        format!("Expected {expected} in @Participants, got: {}", bad.kind()),
    ));
}

/// Parse one participant node and keep its entry when it parses.
fn push_entry(
    entries: &mut Vec<ParticipantEntry>,
    participant: ParticipantNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) {
    if let ParseOutcome::Parsed(entry) = parse_participant_entry(participant, source, errors) {
        entries.push(entry);
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
fn parse_participant_entry(
    typed: ParticipantNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    let node = typed.raw_node();
    let children = extract_participant(typed);
    let unexpected = &children.unexpected;

    let speaker_code = match children.code.slot().view() {
        SlotView::Present(speaker) => {
            let ParseOutcome::Parsed(text) =
                decode_present_child(speaker.raw_node(), source, errors, PARTICIPANT, |err| {
                    format!("Failed to extract participant speaker code as UTF-8: {err}")
                })
            else {
                surface_displaced(unexpected, PARTICIPANT, source, errors);
                return ParseOutcome::rejected();
            };
            text
        }
        SlotView::Missing(missing) => {
            check_not_missing(missing, source, errors, PARTICIPANT);
            surface_displaced(unexpected, PARTICIPANT, source, errors);
            return ParseOutcome::rejected();
        }
        SlotView::Error(bad) => {
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
        SlotView::Absent(NoChild) => {
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
    let mut words: Vec<String> = Vec::with_capacity(children.child_1.slot().len());
    for item in children.child_1.slot() {
        match item.slot().view() {
            SlotView::Present(group) => {
                expect_structure(group.child_0.slot(), PARTICIPANT, source, errors, |bad| {
                    report_entry_shape(bad, source, errors);
                });
                match group.child_1.slot().view() {
                    SlotView::Present(word) => {
                        if let ParseOutcome::Parsed(text) = decode_present_child(
                            word.raw_node(),
                            source,
                            errors,
                            PARTICIPANT,
                            |err| {
                                format!(
                                    "Failed to extract participant name or role as UTF-8: {err}"
                                )
                            },
                        ) {
                            words.push(text);
                        }
                    }
                    SlotView::Absent(NoChild) => {}
                    SlotView::Missing(missing) => {
                        check_not_missing(missing, source, errors, PARTICIPANT);
                    }
                    SlotView::Error(bad) => {
                        report_entry_shape(bad, source, errors);
                    }
                }
                surface_displaced(&group.unexpected, PARTICIPANT, source, errors);
            }
            SlotView::Absent(NoChild) => {}
            SlotView::Error(bad) => {
                report_entry_shape(bad, source, errors);
            }
        }
    }
    // Trailing whitespace before the comma or newline is tolerated by the
    // grammar and carries nothing, but a node that is not whitespace there
    // is still the entry losing its shape.
    if let Some(trailing) = children.child_2.slot() {
        expect_structure(trailing, PARTICIPANT, source, errors, |bad| {
            report_entry_shape(bad, source, errors);
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

/// E316 at a node that sits where a participant entry expected whitespace or
/// a word: the entry has lost its shape there.
fn report_entry_shape(bad: Node, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::UnparsableContent,
        Severity::Error,
        SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
        ErrorContext::new(source, bad.start_byte()..bad.end_byte(), PARTICIPANT),
        format!("Unparsable content in participant entry: {}", bad.kind()),
    ));
}
