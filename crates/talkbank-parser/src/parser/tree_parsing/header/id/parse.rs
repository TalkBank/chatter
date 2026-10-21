//! Parsing for `@ID` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//!
//! The header's generated `child_2` owns `id_contents`. Its generated
//! carrier normalizes each optional text field and adjacent whitespace into
//! a typed repeat: structural whitespace and the concrete field remain
//! distinct alternatives. Languages stay optional, speaker and role stay
//! required, and every pipe retains its own recovery slot. The selectors
//! below name actual generated field types instead of reconstructing fields
//! from delimiter-separated text.
//!
//! Extraction retains the source-bound header, contents and field projections.
//! Contents consume selected range admission before any field decoding.
//! Required and optional field presence retain their recovery policies; an
//! admission failure is an internal producer failure, not an empty CHAT field.
//! Structural pipes and whitespace carry no model payload.
//!
//! **Reachability of the recovery arms (coverage drive batch 4,
//! 2026-10-19).** The slot-state census over 4,707 single-edit mutations of
//! two `@ID` lines put the speaker and role positions only in `Present`, the
//! optional field repeats in `Error` (a stray delimiter inside a field, which
//! `optional_field!` leaves to the whole-tree backstop) and the final pipe in
//! `Missing` (a truncated `@ID`). The languages field was never `Missing` or
//! `Error`, and nothing inside `id_languages` was either: `report_recovery`
//! there is entered by no authored file. Those arms stay for the states the
//! slot types admit; an `Absent` required position reports the producer
//! fault.

use crate::generated_traversal::{
    AsRawNode, IdContentsNode, IdHeaderNode, IdRoleNode, IdSpeakerNode, Never, NoChild,
    ReadableSlot, SourceBound, SourceBoundKind, SourceSlotView,
};

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::node_types::{ID_CONTENTS, ID_HEADER, ID_LANGUAGES};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::{CstFailure, GeneratedPosition, report_cst_failure};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, LanguageCodes, Sex};

/// The field's generated kind owns its position; callers supply no policy selector.
trait RequiredIDField<'tree>: SourceBoundKind<'tree> {
    /// The generated carrier and field of this required position, named in
    /// a producer-fault report when the walk leaves it empty.
    const POSITION: GeneratedPosition;
}

impl<'tree> RequiredIDField<'tree> for IdSpeakerNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("IdContentsChildren.child_4");
}

impl<'tree> RequiredIDField<'tree> for IdRoleNode<'tree> {
    const POSITION: GeneratedPosition = GeneratedPosition::new("IdContentsChildren.child_14");
}

/// Decode only a range-admitted required field; recovery rejects its payload.
///
/// A MISSING or ERROR node at the position is reported by the document's
/// whole-tree backstop, as `optional_field!` already relies on, and rejects
/// the field, which makes the header `Unknown`. The header fragment route
/// (`parse_header_fragment`) lowers a header without that backstop, so
/// there the rejection is unreported, a gap the coverage drive records
/// rather than closes here. Reachability (coverage drive
/// batch 4, 2026-10-19): across 4,707 single-edit mutations of two `@ID`
/// lines the speaker and role positions were only ever `Present`; a
/// truncated `@ID` puts its MISSING node at the final pipe and leaves the
/// role an empty PRESENT token, which the model's `check_id_header` reports
/// as E515. Until 2026-10-19 this arm reported E511 or E515 itself, a mirror
/// of that model rule at a recovery node nobody read, with the model's own
/// wording. An `Absent` required position is the walk's fault.
fn required_field<'tree, 'source, T, M, E>(
    slot: &ReadableSlot<SourceBound<'tree, 'source, T>, M, E, Never, NoChild>,
    contents: SourceBound<'tree, '_, IdContentsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<String>
where
    T: RequiredIDField<'tree>,
{
    match slot {
        ReadableSlot::Present(value) => ParseOutcome::parsed(value.text().to_owned()),
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) => ParseOutcome::rejected(),
        ReadableSlot::Absent(NoChild) => {
            report_cst_failure(
                contents.raw_node(),
                contents.source(),
                CstFailure::AbsentRequiredPosition {
                    rule: ID_CONTENTS,
                    position: T::POSITION,
                },
                errors,
            );
            ParseOutcome::rejected()
        }
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

/// Select the typed field from its generated whitespace/field repeat.
/// Whitespace is structural, not field text; recovery remains distinct from
/// a present field and is retained by the document's whole-tree backstop.
macro_rules! optional_field {
    ($slots:expr, $choice:ident, $field:ident) => {{
        use crate::generated_traversal::$choice;
        $slots.iter().find_map(|element| match &element.slot {
            ReadableSlot::Present($choice::$field(field)) => Some(field.text().to_owned()),
            ReadableSlot::Present($choice::Whitespaces(_))
            | ReadableSlot::Missing(_)
            | ReadableSlot::Error(_) => None,
            ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        })
    }};
}

/// Parse ID header from tree-sitter node.
pub fn parse_id_header<'tree>(
    typed: SourceBound<'tree, '_, IdHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Header, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();

    // Per-rule admission proves this ordinary composite payload present;
    // source-range admission is a distinct producer obligation.
    let header_children = crate::parser::typed_cst::extract_envelope(typed)?;
    let SourceSlotView::Present(contents) = header_children.field_child_2().slot().view();
    let id_contents = contents.read()?;

    // Decompose id_contents into its typed field slots. The NEW backend does NOT
    // skip whitespace, so every `optional($.whitespaces)` between pipe-delimited
    // fields is its OWN position; the field indices below are wider than the OLD
    // module's (see the field-mapping table in the module doc comment) but the
    // FIELDS THEMSELVES are unchanged.
    let contents = id_contents.extract();

    // Retain only displaced evidence across consuming admission, not a cloned
    // payload. Unreadable selected ranges cannot enter field decoding.
    let displaced = contents.children().unexpected.clone();
    let admitted = match contents.admit_ranges() {
        Ok(admitted) => admitted,
        Err(failure) => {
            surface_displaced(
                &header_children.children().unexpected,
                ID_HEADER,
                source,
                errors,
            );
            surface_displaced(&displaced, ID_CONTENTS, source, errors);
            return Err(failure.into());
        }
    };
    drop(displaced);
    let contents = admitted.children();

    let language = match &contents.child_0.slot {
        // Absence is an empty model list, whose validator retains E510.
        None => ParseOutcome::parsed(LanguageCodes::new(Vec::new())),
        Some(slot) => match slot {
            ReadableSlot::Present(field) => {
                super::super::metadata::parse_id_languages(*field, errors)?
            }
            ReadableSlot::Missing(_) | ReadableSlot::Error(_) => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyIDLanguage,
                    Severity::Error,
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, span_of(node), ID_LANGUAGES),
                    "Malformed id_languages field in @ID header",
                ));
                ParseOutcome::Rejected
            }
            ReadableSlot::Absent(never) | ReadableSlot::Unexpected(never) => match *never {},
        },
    };

    // Corpus is semantically required but parsed leniently as an optional field so
    // the model is still built when the corpus is blank; an absent/empty corpus
    // leaves the constructor's empty `CorpusName`, which the Validate trait flags
    // as E514. (Reproduces the pre-migration `parse_optional_text_field` choice.)
    let corpus = optional_field!(contents.child_2.slot, IdContentsChild2Choice, IdCorpus);

    let speaker = required_field(&contents.child_4.slot, id_contents, errors);

    let age = optional_field!(contents.child_6.slot, IdContentsChild6Choice, IdAge);

    // Sex is classified to `Sex` HERE (unlike `ses` below, whose raw text defers
    // to the model constructor): the optional `id_sex` node's text -- known
    // (`male`/`female`) or generic alike -- is mapped through `Sex::from_text`
    // (`Unsupported` for unknown values, flagged as E542 by the validator).
    let sex = optional_field!(contents.child_8.slot, IdContentsChild8Choice, IdSex)
        .map(|text| Sex::from_text(&text));

    let group = optional_field!(contents.child_10.slot, IdContentsChild10Choice, IdGroup);

    // Ses stays TEXT-based: the raw text is carried through and classified by
    // `SesValue::from_text` at model-construction time below (E546 for unknown).
    let ses = optional_field!(contents.child_12.slot, IdContentsChild12Choice, IdSes);

    let role = required_field(&contents.child_14.slot, id_contents, errors);

    let education = optional_field!(contents.child_16.slot, IdContentsChild16Choice, IdEducation);

    let custom_field = optional_field!(
        contents.child_18.slot,
        IdContentsChild18Choice,
        IdCustomField
    );

    surface_displaced(
        &header_children.children().unexpected,
        ID_HEADER,
        source,
        errors,
    );
    for displaced in &contents.unexpected {
        surface_displaced(
            std::slice::from_ref(&displaced.raw_node()),
            ID_CONTENTS,
            displaced.source(),
            errors,
        );
    }

    let (ParseOutcome::Parsed(language), ParseOutcome::Parsed(speaker), ParseOutcome::Parsed(role)) =
        (language, speaker, role)
    else {
        return Ok(unknown_id_header("ID header contains malformed fields"));
    };

    // No Rust-side trimming needed, the grammar's optional($.whitespaces)
    // wrappers and trimming field regexes ensure field content arrives without
    // leading/trailing whitespace.

    let mut id_header = talkbank_model::model::IDHeader::from_languages(language, speaker, role);
    // Absent/empty corpus leaves the constructor's empty `CorpusName`, which the
    // Validate trait reports as E514 (corpus is required).
    if let Some(c) = corpus {
        id_header = id_header.with_corpus(c);
    }
    if let Some(a) = age {
        id_header = id_header.with_age(a);
    }
    if let Some(s) = sex {
        id_header = id_header.with_sex(s);
    }
    if let Some(g) = group {
        id_header = id_header.with_group(g);
    }
    if let Some(ses_val) = ses {
        id_header = id_header.with_ses(talkbank_model::model::SesValue::from_text(&ses_val));
    }
    if let Some(e) = education {
        id_header = id_header.with_education(e);
    }
    if let Some(cf) = custom_field {
        id_header = id_header.with_custom_field(cf);
    }

    Ok(Header::ID(id_header))
}

/// Build `Header::Unknown` for malformed `@ID` input.
fn unknown_id_header(parse_reason: impl Into<String>) -> Header {
    Header::Unknown {
        text: "@ID".into(),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some(
            "Expected @ID format: @ID:\\tlang|corpus|speaker|age|sex|group|ses|role|education|custom|"
                .to_string(),
        ),
    }
}
