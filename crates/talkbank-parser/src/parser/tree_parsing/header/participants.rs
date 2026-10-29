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
//! Compiled-language admission excludes Missing for eligible composite kinds;
//! lexical and alias-target slots retain it. The ordinary header additionally
//! uses a per-rule required-position proof, so its contents slot is Present.
//! Nested contents and entries retain Error and fixed-slot Absent; their
//! Unexpected payload is uninhabited. Whole sequence-repeat items also
//! have an uninhabited Missing payload. Other recovery states remain even
//! when finite fixtures have not witnessed them. The retained corpus does
//! witness a MISSING speaker and a doubled-comma ERROR repeat item.
//!
//! **Reachability of the recovery arms (measured, not reasoned).** The
//! slot-state census (`cargo run -p talkbank-parser-tests --example
//! slot_state_census`) over every CHAT file in the repository (2026-09-09)
//! and over 7,206 generated files (7,158 single-edit mutations of three
//! `@Participants` lines plus 48 hand-written variants, 2026-10-19, coverage
//! drive batch 1) put exactly two positions here into a recovery state:
//! `participant.code` is `Missing` (`@Participants:\t`, E342) and an element
//! of `participants_contents.child_1` is `Error` (a doubled comma,
//! `E506.md#3`). Every other arm below (an ERROR or MISSING comma or
//! whitespace inside a recognised group, an ERROR entry, an ERROR code, an
//! ERROR or MISSING name word, displaced nodes inside a group or an entry)
//! was reached by no authored malformed file. Those arms stay because their
//! position's type admits the state, and each reports the recovery node it
//! was given, under the code of the construct it broke, rather than
//! inventing a value; none re-derives a model rule from parsed content. An
//! `Absent` required position is different in kind: the grammar has no
//! fail-closed partials, so the walk cannot leave one empty, and the two
//! arms for it report [`CstFailure::AbsentRequiredPosition`] instead of a
//! CHAT code.
//!
//! Until 2026-09-08 this file hand-walked both repeats with an index
//! (`child.kind() == COMMA`, `idx += 1`) with duplicated structural diagnostics.
//! Source-bound extraction now also keeps tree/source identity through every
//! participant group and leaf. Each entry admits its selected ranges once
//! before payload decoding; retained leaf reads cannot fail independently.
//! Admission remains fallible; participant lowering cannot accept a separately
//! supplied string. No recovery state is removed because a finite corpus has
//! not reached it.

use crate::generated_traversal::{Absence, KindMissing, Never, NodeSlot};
use crate::generated_traversal::{
    AsRawNode, ParticipantNode, ParticipantsHeaderNode, ReadableSlot, SourceBound, SourceField,
    SourceSlice, SourceSlotView,
};
use crate::node_types::{PARTICIPANT, PARTICIPANTS_CONTENTS, PARTICIPANTS_HEADER};
use crate::parser::typed_cst::ExtractCanonical;
use tree_sitter::Node;

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};
use crate::parser::typed_cst::{
    CstFailure, GeneratedPosition, read_source_field, report_cst_failure,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{
    Header, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};

/// Parse a `@Participants` header into [`Header::Participants`].
///
/// An observed speaker without a role remains an explicit shared recovery
/// entry. Missing speaker codes still reject: a missing CST token cannot
/// supply a declaration identity.
pub fn parse_participants_header<'tree>(
    typed: SourceBound<'tree, '_, ParticipantsHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, crate::parser::typed_cst::CstFailure> {
    // A recovered CST may expose a truncated code or promote a role to a code.
    // Keep the complete header as rejected evidence before those leaves acquire
    // declaration semantics. A plain missing role has no CST recovery nodes.
    let rejected = typed.raw_node().has_error();
    let raw = talkbank_model::model::UnparsedParticipantHeader::new(
        span_of(typed.raw_node()),
        typed.text(),
    );
    // An actually empty source body is an empty declaration, not failed
    // lowering disguised as an empty list. The exact prefix establishes which
    // bytes are the body; no ERROR-node text is reclassified here.
    let empty_body = typed
        .text()
        .strip_prefix("@Participants:\t")
        .is_some_and(|body| body.trim().is_empty());
    let findings = talkbank_model::ErrorCollector::new();
    let result = parse_participants_payload(typed, &findings);
    errors.report_all(
        findings
            .into_vec()
            .into_iter()
            .filter(|finding| !rejected || finding.code != ErrorCode::EmptyParticipantRole)
            .collect(),
    );
    let result = result?;
    if rejected && !empty_body {
        Ok(Header::UnparsedParticipants(raw))
    } else {
        Ok(result)
    }
}

fn parse_participants_payload<'tree>(
    typed: SourceBound<'tree, '_, ParticipantsHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, crate::parser::typed_cst::CstFailure> {
    let node = typed.raw_node();
    let source = typed.source();

    // The list parsing below only descends into `participants_contents`; the
    // shared header scan reports any structural ERROR/MISSING node
    // tree-sitter parked elsewhere under the header (a trailing comma, say)
    // so it is never silently swallowed.
    super::report_header_structural_errors(node, PARTICIPANTS_HEADER, source, errors);

    let rule = crate::parser::typed_cst::participants_rule()?;
    let header_children = typed.extract_required(rule)?;
    let SourceSlotView::Present(contents_node) = header_children.field_child_2().slot().view();
    surface_displaced(
        &header_children.children().unexpected,
        PARTICIPANTS_HEADER,
        source,
        errors,
    );

    // Structural presence and readable source admission are distinct states.
    // A producer failure must not become an empty-header CHAT diagnostic.
    let contents = contents_node.read()?;
    let contents_raw = contents.raw_node();
    let contents = contents.extract_canonical()?;
    let rest = contents.field_child_1().slot().iter();
    let mut entries = Vec::with_capacity(rest.len() + 1);

    entries.extend(
        parse_participant_slot(
            contents.field_child_0().slot(),
            ParticipantPosition::First,
            contents_raw,
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
                // The comma and whitespace arms inside a recognised group
                // are unreached by any authored malformed file (module doc):
                // tree-sitter has only ever parked a list fault as a whole
                // ERROR element, the arm below.
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
                        contents_raw,
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
            SourceSlotView::Absent(never) => match never {},
        }
    }
    surface_displaced(
        &contents.children().unexpected,
        PARTICIPANTS_CONTENTS,
        source,
        errors,
    );

    Ok(Header::Participants {
        entries: entries.into(),
    })
}

/// Structural positions need no text read, but retain their recovery policy
/// and derive diagnostic source identity from the generated field itself.
fn expect_source_structure<'tree, T: AsRawNode<'tree> + Copy, A: Absence>(
    slot: SourceField<'_, 'tree, '_, NodeSlot<T, KindMissing<T>, Node<'tree>, Never, A>>,
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

/// Structural recovery after range admission; readability does not imply syntax validity.
fn expect_readable_structure<'tree, T, M: AsRawNode<'tree>, E: AsRawNode<'tree>, A>(
    slot: &ReadableSlot<T, M, E, Never, A>,
    source: &str,
    errors: &impl ErrorSink,
    on_bad: impl FnOnce(Node<'tree>),
) {
    match slot {
        ReadableSlot::Present(_) | ReadableSlot::Absent(_) => {}
        ReadableSlot::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, PARTICIPANT);
        }
        ReadableSlot::Error(bad) => on_bad(bad.raw_node()),
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

fn surface_readable_displaced(nodes: &[SourceSlice<'_, '_>], errors: &impl ErrorSink) {
    for node in nodes {
        surface_displaced(
            std::slice::from_ref(&node.raw_node()),
            PARTICIPANT,
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
        position: ParticipantPosition,
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
                    ParticipantPosition::First => "a participant entry",
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
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), context),
            message,
        ));
    }
}

/// Where an entry sits in the list, which words its fault report. It no
/// longer carries the header node: until 2026-10-19 the first position used
/// it to report E506 for an `Absent` first entry, a state the walk cannot
/// produce (module doc), so that report invented a CHAT diagnostic for
/// content never read. Both positions now report the producer fault.
#[derive(Clone, Copy)]
enum ParticipantPosition {
    First,
    Subsequent,
}

impl ParticipantPosition {
    /// The generated carrier and field of the participant slot, for the
    /// fault. The repeat group is an inline sequence with no node of its
    /// own, so its element index is not part of the name.
    fn generated_position(self) -> GeneratedPosition {
        match self {
            Self::First => GeneratedPosition::new("ParticipantsContentsChildren.child_0"),
            Self::Subsequent => {
                GeneratedPosition::new("ParticipantsContentsChild1Children.child_2")
            }
        }
    }
}

/// Shared admission for required and repeated participant slots. Recovery
/// stays outside entry conversion, and positional policy remains explicit.
/// `contents` is the `participants_contents` node, where a producer fault
/// with no node of its own is located.
fn parse_participant_slot<'tree, A: crate::generated_traversal::Absence>(
    slot: SourceField<
        '_,
        'tree,
        '_,
        crate::generated_traversal::NonMissingKindSlot<
            'tree,
            ParticipantNode<'tree>,
            crate::generated_traversal::Raw,
            A,
        >,
    >,
    position: ParticipantPosition,
    contents: Node<'tree>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(participant) => match read_source_field(participant, errors) {
            Some(participant) => parse_participant_entry(participant, errors),
            None => ParseOutcome::rejected(),
        },
        // An ERROR node standing for a whole entry: unreached by any authored
        // malformed file (module doc); a list fault arrives as an ERROR
        // repeat element instead.
        SourceSlotView::Error(bad) => {
            ParticipantFault::Entry {
                node: bad.raw_node(),
                position,
            }
            .report(source, errors);
            ParseOutcome::rejected()
        }
        // A required position the walk left empty is a generator or grammar
        // fault (module doc), never an empty header: the model reports E506
        // from the entry list it is given.
        SourceSlotView::Absent(_) => {
            report_cst_failure(
                contents,
                source,
                CstFailure::AbsentRequiredPosition {
                    rule: PARTICIPANTS_CONTENTS,
                    position: position.generated_position(),
                },
                errors,
            );
            ParseOutcome::rejected()
        }
    }
}

/// Parse a single participant entry: `CODE [NAME ...] ROLE`.
///
/// The last word is the role and every word before it is the name. The
/// `speaker` and `participant_word` tokens are non-empty by their regexes,
/// so a missing code is a code the parser had to invent
/// (E342 from the recovery report; the entry rejects). No word after the code
/// is explicit missing-role recovery (E513, `@Participants:\tCHI`). E512 has no route here: the
/// first word of an entry is always its code, which `E512.md` records.
fn parse_participant_entry<'tree>(
    typed: SourceBound<'tree, '_, ParticipantNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    let node = typed.raw_node();
    let source = typed.source();
    let children = typed.extract();
    // Keep only displaced recovery evidence across a failed consuming
    // admission. Do not clone the complete selected carrier.
    let displaced = children.children().unexpected.clone();
    let admitted = match children.admit_ranges() {
        Ok(admitted) => admitted,
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(node, source, failure, errors);
            surface_displaced(&displaced, PARTICIPANT, source, errors);
            return ParseOutcome::Rejected;
        }
    };
    drop(displaced);
    let children = admitted.children();
    let unexpected = &children.unexpected;

    let speaker_code = match &children.code.slot {
        ReadableSlot::Present(speaker) => speaker.text(),
        ReadableSlot::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, PARTICIPANT);
            surface_readable_displaced(unexpected, errors);
            return ParseOutcome::rejected();
        }
        // An ERROR node at the code position: unreached by any authored
        // malformed file (module doc); garbage before a code has only ever
        // become an ERROR element of the list or a MISSING speaker.
        ReadableSlot::Error(bad) => {
            let bad = bad.raw_node();
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantCode,
                Severity::Error,
                DiagnosticLocation::new(span_of(bad)),
                ErrorContext::new(source, span_of(bad), PARTICIPANT),
                format!(
                    "Expected a speaker code to start the participant entry, got: {}",
                    bad.kind()
                ),
            ));
            surface_readable_displaced(unexpected, errors);
            return ParseOutcome::rejected();
        }
        ReadableSlot::Unexpected(never) => match *never {},
        // The code is a required position; the walk cannot leave it empty
        // (module doc). Until 2026-10-19 this arm reported E512 for a code
        // nobody read; it reports the producer fault instead.
        ReadableSlot::Absent(_) => {
            report_cst_failure(
                node,
                source,
                CstFailure::AbsentRequiredPosition {
                    rule: PARTICIPANT,
                    position: GeneratedPosition::new("ParticipantChildren.code"),
                },
                errors,
            );
            surface_readable_displaced(unexpected, errors);
            return ParseOutcome::rejected();
        }
    };

    // Name words and the role, each preceded by whitespace the grammar
    // requires; the whitespace is structure and needs nothing when Present.
    // Every recovery arm in this loop and the trailing-whitespace check
    // after it is unreached by any authored malformed file (module doc):
    // a stray token between words has only ever split the entry or become
    // an ERROR element of the list.
    let mut words: Vec<String> = Vec::with_capacity(children.child_1.slot.len());
    for item in &children.child_1.slot {
        match &item.slot {
            ReadableSlot::Present(group) => {
                expect_readable_structure(&group.child_0.slot, source, errors, |bad| {
                    ParticipantFault::EntryShape(bad).report(source, errors);
                });
                match &group.child_1.slot {
                    ReadableSlot::Present(word) => {
                        words.push(word.text().to_owned());
                    }
                    ReadableSlot::Unexpected(never) => match *never {},
                    ReadableSlot::Absent(_) => {}
                    ReadableSlot::Missing(missing) => {
                        check_not_missing(missing.raw_node(), source, errors, PARTICIPANT);
                    }
                    ReadableSlot::Error(bad) => {
                        ParticipantFault::EntryShape(bad.raw_node()).report(source, errors);
                    }
                }
                surface_readable_displaced(&group.unexpected, errors);
            }
            ReadableSlot::Absent(never)
            | ReadableSlot::Missing(never)
            | ReadableSlot::Unexpected(never) => match *never {},
            ReadableSlot::Error(bad) => {
                ParticipantFault::EntryShape(bad.raw_node()).report(source, errors);
            }
        }
    }
    // Trailing whitespace before the comma or newline is tolerated by the
    // grammar and carries nothing, but a node that is not whitespace there
    // is still the entry losing its shape.
    if let Some(trailing) = &children.child_2.slot {
        expect_readable_structure(trailing, source, errors, |bad| {
            ParticipantFault::EntryShape(bad).report(source, errors);
        });
    }
    surface_readable_displaced(unexpected, errors);

    let Ok(words) = crate::ParticipantWordRoles::from_words(words) else {
        errors.report(ParseError::new(
            ErrorCode::EmptyParticipantRole,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), PARTICIPANT),
            "Participant role cannot be empty",
        ));
        return ParseOutcome::parsed(ParticipantEntry::missing_role(SpeakerCode::new(
            speaker_code,
        )));
    };
    let name = if words.names().is_empty() {
        None
    } else {
        Some(ParticipantName::new(words.names().join(" ")))
    };

    ParseOutcome::parsed(ParticipantEntry::new(
        SpeakerCode::new(speaker_code),
        name,
        ParticipantRole::new(words.role().as_str()),
    ))
}
