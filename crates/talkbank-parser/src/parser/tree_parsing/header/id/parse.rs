//! Parsing for `@ID` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//!
//! **Grammar Rule (structural pipes stripped by the typed visitor; the NEW
//! backend does NOT skip whitespace, so every `optional($.whitespaces)` between
//! fields is ALSO a real (unused-here) position, which is why the field indices
//! below are wider than the OLD module's -- see the field mapping table):**
//! ```javascript
//! id_header: $ => seq(
//!     id_prefix,    // child_0 (structural)
//!     header_sep,   // child_1 (structural)
//!     id_contents,  // child_2 <-- payload (UNCHANGED index from the OLD module)
//!     newline       // child_3 (structural)
//! )
//!
//! id_contents: $ => seq(
//!     optional(id_languages),                // typed child_0  (Option; validation requires nonempty)
//!     '|',                                   // typed child_1  (pipe)
//!     optional($.whitespaces),               // typed child_2  (NEW: not skipped)
//!     optional($.id_corpus),                 // typed child_3  (Option)
//!     optional($.whitespaces),               // typed child_4  (NEW: not skipped)
//!     '|',                                   // typed child_5  (pipe)
//!     id_speaker,                            // typed child_6  (required)
//!     '|',                                   // typed child_7  (pipe)
//!     optional($.whitespaces),               // typed child_8  (NEW: not skipped)
//!     optional($.id_age),                    // typed child_9  (Option)
//!     optional($.whitespaces),               // typed child_10 (NEW: not skipped)
//!     '|',                                   // typed child_11 (pipe)
//!     optional($.whitespaces),               // typed child_12 (NEW: not skipped)
//!     optional($.id_sex),                    // typed child_13 (Option)
//!     optional($.whitespaces),               // typed child_14 (NEW: not skipped)
//!     '|',                                   // typed child_15 (pipe)
//!     optional($.whitespaces),               // typed child_16 (NEW: not skipped)
//!     optional($.id_group),                  // typed child_17 (Option)
//!     optional($.whitespaces),               // typed child_18 (NEW: not skipped)
//!     '|',                                   // typed child_19 (pipe)
//!     optional($.whitespaces),               // typed child_20 (NEW: not skipped)
//!     optional($.id_ses),                    // typed child_21 (Option)
//!     optional($.whitespaces),               // typed child_22 (NEW: not skipped)
//!     '|',                                   // typed child_23 (pipe)
//!     id_role,                               // typed child_24 (required)
//!     '|',                                   // typed child_25 (pipe)
//!     optional($.whitespaces),               // typed child_26 (NEW: not skipped)
//!     optional($.id_education),              // typed child_27 (Option)
//!     optional($.whitespaces),               // typed child_28 (NEW: not skipped)
//!     '|',                                   // typed child_29 (pipe)
//!     optional($.whitespaces),               // typed child_30 (NEW: not skipped)
//!     optional($.id_custom_field),           // typed child_31 (Option)
//!     optional($.whitespaces),               // typed child_32 (NEW: not skipped)
//!     '|'                                    // typed child_33 (pipe)
//! )
//! ```
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

/// Decode an admitted optional field; recovery omission keeps the whole-tree backstop.
fn optional_field<'tree, 'source, T, M>(
    slot: &Option<ReadableSlot<'tree, 'source, SourceBound<'tree, 'source, T>, M, Never, Never>>,
) -> Option<String>
where
    T: SourceBoundKind<'tree>,
{
    if let Some(ReadableSlot::Present(field)) = slot {
        Some(field.text().to_owned())
    } else {
        None
    }
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

    // Corpus is semantically required but parsed leniently as an optional slot so
    // the model is still built when the corpus is blank; an absent/empty corpus
    // leaves the constructor's empty `CorpusName`, which the Validate trait flags
    // as E514. (Reproduces the pre-migration `parse_optional_text_field` choice.)
    let corpus = optional_field(&contents.child_3.slot);

    let speaker = required_field(&contents.child_6.slot, id_contents, errors);

    let age = optional_field(&contents.child_9.slot);

    // Sex is classified to `Sex` HERE (unlike `ses` below, whose raw text defers
    // to the model constructor): the optional `id_sex` node's text -- known
    // (`male`/`female`) or generic alike -- is mapped through `Sex::from_text`
    // (`Unsupported` for unknown values, flagged as E542 by the validator).
    let sex = optional_field(&contents.child_13.slot).map(|text| Sex::from_text(&text));

    let group = optional_field(&contents.child_17.slot);

    // Ses stays TEXT-based: the raw text is carried through and classified by
    // `SesValue::from_text` at model-construction time below (E546 for unknown).
    let ses = optional_field(&contents.child_21.slot);

    let role = required_field(&contents.child_24.slot, id_contents, errors);

    let education = optional_field(&contents.child_27.slot);

    let custom_field = optional_field(&contents.child_31.slot);

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
