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

use crate::generated_traversal::{
    AsRawNode, IdContentsNode, IdHeaderNode, IdRoleNode, IdSpeakerNode, Never, NoChild,
    ReadableSlot, SourceBound, SourceBoundKind, SourceSlotView,
};

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, LanguageCodes, Sex};

/// The field's generated kind owns its diagnostic; callers supply no policy selector.
trait RequiredIDField<'tree>: SourceBoundKind<'tree> {
    const ERROR: ErrorCode;
    const MESSAGE: &'static str;
}

impl<'tree> RequiredIDField<'tree> for IdSpeakerNode<'tree> {
    const ERROR: ErrorCode = ErrorCode::EmptyIDSpeaker;
    const MESSAGE: &'static str = "Missing id_speaker field in @ID header";
}

impl<'tree> RequiredIDField<'tree> for IdRoleNode<'tree> {
    const ERROR: ErrorCode = ErrorCode::EmptyIDRole;
    const MESSAGE: &'static str =
        "Empty role field in @ID header: the role (8th field) must not be blank";
}

/// Decode only a range-admitted required field; recovery still rejects its payload.
fn required_field<'tree, 'source, T, M>(
    slot: &ReadableSlot<'tree, 'source, SourceBound<'tree, 'source, T>, M, Never, NoChild>,
    contents: SourceBound<'tree, '_, IdContentsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<String>
where
    T: RequiredIDField<'tree>,
{
    let id_contents = contents.raw_node();
    let source = contents.source();
    let ReadableSlot::Present(value) = slot else {
        errors.report(ParseError::new(
            T::ERROR,
            Severity::Error,
            SourceLocation::from_offsets(id_contents.start_byte(), id_contents.end_byte()),
            ErrorContext::new(
                source,
                id_contents.start_byte()..id_contents.end_byte(),
                "id_contents",
            ),
            T::MESSAGE,
        ));
        return ParseOutcome::rejected();
    };
    ParseOutcome::parsed(value.text().to_owned())
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

    // Retain the missing-payload diagnostic for every non-present slot state;
    // source-range admission is a distinct producer obligation.
    let header_children = typed.extract();
    let SourceSlotView::Present(contents) = header_children.field_child_2().slot().view() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "id_header"),
            "Missing id_contents child in id_header",
        ));
        surface_displaced(
            &header_children.children().unexpected,
            "id_header",
            source,
            errors,
        );
        return Ok(unknown_id_header(
            "ID header CST node is missing id_contents",
        ));
    };
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
                "id_header",
                source,
                errors,
            );
            surface_displaced(&displaced, "id_contents", source, errors);
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
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.byte_range(), "id_languages"),
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
        "id_header",
        source,
        errors,
    );
    for displaced in &contents.unexpected {
        surface_displaced(
            std::slice::from_ref(&displaced.raw_node()),
            "id_contents",
            displaced.source(),
            errors,
        );
    }

    let (language, speaker, role) = match (language, speaker, role) {
        (
            ParseOutcome::Parsed(language),
            ParseOutcome::Parsed(speaker),
            ParseOutcome::Parsed(role),
        ) => (language, speaker, role),
        _ => return Ok(unknown_id_header("ID header contains malformed fields")),
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
