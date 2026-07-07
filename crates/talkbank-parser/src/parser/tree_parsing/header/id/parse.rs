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
//!     id_languages,                          // typed child_0  (required)
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
//! This module reads those fields through the NEW backend's free, exhaustive,
//! typed `extract_id_header` / `extract_id_contents` functions, NOT the old flat
//! raw-cursor walk (and, as of Task B2, not the OLD `TypedTraversal` trait
//! receiver either). Required fields are `NodeSlot` slots matched exhaustively;
//! optional fields are `Option<NodeSlot<..>>` slots; the `pipe` and `whitespaces`
//! separators carry no payload and are ignored.

use crate::generated_traversal::{
    AsRawNode, IdContentsNode, IdHeaderNode, NodeSlot, extract_id_contents, extract_id_header,
};
use crate::node_types::ID_HEADER;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use crate::parser::typed_cst::decode_present_child;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, Sex};

/// Decode the UTF-8 text of an `@ID` field node.
///
/// Reproduces the pre-migration `extract_text_with_errors` behaviour: on success
/// the node's text is returned; on invalid UTF-8 a tree-structure diagnostic is
/// reported and the outcome is rejected. The CHAT source is already valid UTF-8,
/// so the error arm is defensive only.
fn decode_field_text(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<String> {
    decode_present_child(node, source, errors, "id_contents", |err| {
        format!("Failed to extract UTF-8 text: {}", err)
    })
}

/// Read a REQUIRED `@ID` field (languages / speaker / role) from its typed slot.
///
/// `Present` reproduces the pre-migration field read
/// (`child(idx).utf8_text()`) exactly, so the VALID path is byte-identical. The
/// non-`Present` arms (`Missing` / `Error` / `Unexpected` / `Absent`) report the
/// field's empty-field diagnostic (`error_code`) and reject. The old flat-cursor
/// walk only rejected a required field when the child list ran out
/// (`idx >= child_count`, i.e. the slot is `Absent`); for a tree-sitter `Missing`
/// placeholder it formerly read the zero-length text as `""` and silently
/// accepted it. Surfacing those malformed-only cases as an explicit diagnostic is
/// the sanctioned 2g-style improvement: it cannot reach a VALID input (a
/// well-formed `id_header` always yields `Present` required fields).
fn required_field<'tree, T: AsRawNode<'tree>>(
    slot: NodeSlot<'tree, T>,
    id_contents: Node,
    source: &str,
    errors: &impl ErrorSink,
    error_code: ErrorCode,
    error_message: &str,
) -> ParseOutcome<String> {
    let Some(node) = slot.present_or_recover().ok() else {
        errors.report(ParseError::new(
            error_code,
            Severity::Error,
            SourceLocation::from_offsets(id_contents.start_byte(), id_contents.end_byte()),
            ErrorContext::new(
                source,
                id_contents.start_byte()..id_contents.end_byte(),
                "id_contents",
            ),
            error_message,
        ));
        return ParseOutcome::rejected();
    };
    decode_field_text(node.raw_node(), source, errors)
}

/// Read an OPTIONAL `@ID` text field (corpus / age / group / ses / education /
/// custom) from its typed `Option<NodeSlot>` slot.
///
/// `Present` reproduces the old present-field read, so the VALID path is
/// byte-identical. The outer `None` reproduces the old "field absent" path (the
/// pre-migration `child.kind() != expected` / `idx >= child_count` miss): no
/// error, the model field is left unset. The remaining `NodeSlot` states
/// (`Missing` / `Error` / `Unexpected`, and the `Absent` classifier value) are
/// malformed-only recovery artifacts a VALID `@ID` never produces; they map to
/// the same no-error "field absent" path. This mirrors `required_field`'s
/// handling and the sanctioned 2g-style stance that a `Missing` placeholder is
/// not silently decoded as a real (empty) field value (the old flat-cursor walk
/// wrapped a `Missing` node and read its zero-length text as `""`). An absent
/// optional NEVER errors, exactly as before. Matched EXHAUSTIVELY, with no `_`
/// catch-all that could silently drop a recovery node.
fn optional_field<'tree, T: AsRawNode<'tree>>(
    slot: Option<NodeSlot<'tree, T>>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<String>> {
    match slot.and_then(|s| s.present_or_recover().ok()) {
        Some(node) => decode_field_text(node.raw_node(), source, errors).map(Some),
        None => ParseOutcome::parsed(None),
    }
}

/// Parse ID header from tree-sitter node.
pub fn parse_id_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is an id_header node.
    if node.kind() != ID_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected id_header node, got: {}", node.kind()),
        ));
        return unknown_id_header("ID header CST node had unexpected kind");
    }

    // Descend to the id_contents payload via the typed `child_2` slot of the
    // id_header (unchanged index from the OLD module: id_header has no
    // interstitial whitespace positions of its own). `present_or_recover().ok()`
    // keeps only a Present id_contents; every non-Present recovery state funnels
    // to the pre-migration `find_child_by_kind(node, ID_CONTENTS) == None` branch.
    let header_children = extract_id_header(IdHeaderNode(node));
    let Some(contents) = header_children.child_2.slot.present_or_recover().ok() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "id_header"),
            "Missing id_contents child in id_header",
        ));
        surface_unexpected(&header_children.unexpected, source, errors);
        return unknown_id_header("ID header CST node is missing id_contents");
    };
    let id_contents = contents.raw_node();

    // Decompose id_contents into its typed field slots. The NEW backend does NOT
    // skip whitespace, so every `optional($.whitespaces)` between pipe-delimited
    // fields is its OWN position; the field indices below are wider than the OLD
    // module's (see the field-mapping table in the module doc comment) but the
    // FIELDS THEMSELVES are unchanged.
    let contents = extract_id_contents(IdContentsNode(id_contents));

    let language = required_field(
        contents.child_0.slot,
        id_contents,
        source,
        errors,
        ErrorCode::EmptyIDLanguage,
        "Missing id_languages field in @ID header",
    );

    // Corpus is semantically required but parsed leniently as an optional slot so
    // the model is still built when the corpus is blank; an absent/empty corpus
    // leaves the constructor's empty `CorpusName`, which the Validate trait flags
    // as E514. (Reproduces the pre-migration `parse_optional_text_field` choice.)
    let corpus = optional_field(contents.child_3.slot, source, errors);

    let speaker = required_field(
        contents.child_6.slot,
        id_contents,
        source,
        errors,
        ErrorCode::EmptyIDSpeaker,
        "Missing id_speaker field in @ID header",
    );

    let age = optional_field(contents.child_9.slot, source, errors);

    // Sex is classified to `Sex` HERE (unlike `ses` below, whose raw text defers
    // to the model constructor): the optional `id_sex` node's text -- known
    // (`male`/`female`) or generic alike -- is mapped through `Sex::from_text`
    // (`Unsupported` for unknown values, flagged as E542 by the validator).
    let sex = match optional_field(contents.child_13.slot, source, errors) {
        ParseOutcome::Parsed(opt) => ParseOutcome::parsed(opt.map(|text| Sex::from_text(&text))),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };

    let group = optional_field(contents.child_17.slot, source, errors);

    // Ses stays TEXT-based: the raw text is carried through and classified by
    // `SesValue::from_text` at model-construction time below (E546 for unknown).
    let ses = optional_field(contents.child_21.slot, source, errors);

    let role = required_field(
        contents.child_24.slot,
        id_contents,
        source,
        errors,
        ErrorCode::EmptyIDRole,
        "Empty role field in @ID header: the role (8th field) must not be blank",
    );

    let education = optional_field(contents.child_27.slot, source, errors);

    let custom_field = optional_field(contents.child_31.slot, source, errors);

    surface_unexpected(&header_children.unexpected, source, errors);
    surface_unexpected(&contents.unexpected, source, errors);

    let (language, corpus, speaker, age, sex, group, ses, role, education, custom_field) = match (
        language,
        corpus,
        speaker,
        age,
        sex,
        group,
        ses,
        role,
        education,
        custom_field,
    ) {
        (
            ParseOutcome::Parsed(language),
            ParseOutcome::Parsed(corpus),
            ParseOutcome::Parsed(speaker),
            ParseOutcome::Parsed(age),
            ParseOutcome::Parsed(sex),
            ParseOutcome::Parsed(group),
            ParseOutcome::Parsed(ses),
            ParseOutcome::Parsed(role),
            ParseOutcome::Parsed(education),
            ParseOutcome::Parsed(custom_field),
        ) => (
            language,
            corpus,
            speaker,
            age,
            sex,
            group,
            ses,
            role,
            education,
            custom_field,
        ),
        _ => return unknown_id_header("ID header contains malformed fields"),
    };

    // No Rust-side trimming needed, the grammar's optional($.whitespaces)
    // wrappers and trimming field regexes ensure field content arrives without
    // leading/trailing whitespace.

    // Parse comma-separated language codes (e.g., "eng, spa" → [eng, spa]).
    // `LanguageCode::new` rejects empty pieces fallibly, so `filter_map(.. .ok())`
    // both constructs the code and drops empty segments (e.g. a malformed
    // "eng,,spa"), exactly like the previous explicit `filter(!is_empty)`.
    let language_codes: Vec<talkbank_model::model::LanguageCode> = language
        .split(',')
        .filter_map(|s| talkbank_model::model::LanguageCode::new(s.trim()).ok())
        .collect();
    let languages = talkbank_model::model::LanguageCodes::new(language_codes);

    let mut id_header = talkbank_model::model::IDHeader::from_languages(languages, speaker, role);
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

    Header::ID(id_header)
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
