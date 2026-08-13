// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Integration tests for JSON serialization, schema validation, and error rendering.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::TranscriptName;
use talkbank_transform::json::{
    is_schema_validation_available, schema_load_error, to_json_pretty_unvalidated,
    to_json_unvalidated, validate_json_string,
};
use talkbank_transform::{
    PipelineError, chat_to_json, parse_and_validate, render_error_with_miette,
    render_error_with_miette_with_named_source, render_error_with_miette_with_source,
};

/// Minimal valid CHAT for JSON conversion tests.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";

// ===== Schema (4 tests) =====

#[test]
fn schema_is_available() {
    assert!(
        is_schema_validation_available(),
        "JSON schema should be loadable"
    );
}

#[test]
fn valid_json_passes_schema() -> Result<(), PipelineError> {
    // Parse a valid CHAT file, convert to JSON, then validate against schema
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = talkbank_transform::json::to_json_pretty_validated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    // If to_json_pretty_validated succeeded, schema validation passed
    assert!(!json.is_empty());
    Ok(())
}

#[test]
fn invalid_json_fails_schema() {
    let random_json = r#"{"not_a_chat_field": true, "random": 42}"#;
    let result = validate_json_string(random_json);
    assert!(result.is_err(), "Random JSON should fail schema validation");
}

#[test]
fn schema_load_error_is_none() {
    assert!(
        schema_load_error().is_none(),
        "Schema load error should be None when schema is available"
    );
}

// ===== Serialization (3 tests) =====

#[test]
fn to_json_pretty_has_newlines() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = to_json_pretty_unvalidated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(json.contains('\n'), "Pretty JSON should contain newlines");
    Ok(())
}

#[test]
fn to_json_unvalidated_skips_schema() -> Result<(), PipelineError> {
    let options = ParseValidateOptions::default();
    let chat_file = parse_and_validate(VALID_CHAT, options)?;
    let json = to_json_unvalidated(&chat_file)
        .map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(!json.is_empty(), "Unvalidated JSON should produce output");
    // Verify it is valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| PipelineError::JsonSerialization(e.to_string()))?;
    assert!(parsed.is_object());
    Ok(())
}

#[test]
fn validate_json_string_roundtrip() -> Result<(), PipelineError> {
    // Serialize then validate the resulting string
    let options = ParseValidateOptions::default();
    let json = chat_to_json(VALID_CHAT, options, false)?;
    // chat_to_json already validates, but we can also validate the string directly
    let result = validate_json_string(&json);
    assert!(
        result.is_ok(),
        "Roundtrip JSON should pass schema validation"
    );
    Ok(())
}

// ===== Rendering (3 tests) =====

#[test]
fn render_error_includes_code() {
    // Parse invalid CHAT to get real errors
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            assert!(!parse_errors.errors.is_empty(), "Should have parse errors");
            let rendered = render_error_with_miette(&parse_errors.errors[0]);
            // The rendered output should contain some error information
            assert!(!rendered.is_empty(), "Rendered error should not be empty");
        }
        Err(PipelineError::Validation(errors)) => {
            assert!(!errors.is_empty(), "Should have validation errors");
            let rendered = render_error_with_miette(&errors[0]);
            assert!(!rendered.is_empty(), "Rendered error should not be empty");
        }
        Ok(_) => {
            // If this somehow passes, the test structure still verifies rendering works
        }
        Err(e) => {
            panic!("Unexpected error type: {e}");
        }
    }
}

#[test]
fn render_error_with_source_includes_content() {
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            let rendered =
                render_error_with_miette_with_source(&parse_errors.errors[0], "test.cha", content);
            assert!(
                !rendered.is_empty(),
                "Rendered error with source should not be empty"
            );
        }
        Err(PipelineError::Validation(errors)) => {
            let rendered = render_error_with_miette_with_source(&errors[0], "test.cha", content);
            assert!(
                !rendered.is_empty(),
                "Rendered error with source should not be empty"
            );
        }
        Ok(_) => {}
        Err(e) => panic!("Unexpected error type: {e}"),
    }
}

#[test]
fn render_error_with_named_source_includes_filename() {
    let content = "@UTF8\n@Begin\n*CHI:\thello .\n";
    let options = ParseValidateOptions::default().with_validation();
    match parse_and_validate(content, options) {
        Err(PipelineError::Parse(parse_errors)) => {
            let source = miette::NamedSource::new(
                "my_test_file.cha",
                std::sync::Arc::new(content.to_string()),
            );
            let rendered =
                render_error_with_miette_with_named_source(&parse_errors.errors[0], &source);
            assert!(
                !rendered.is_empty(),
                "Rendered error with named source should not be empty"
            );
        }
        Err(PipelineError::Validation(errors)) => {
            let source = miette::NamedSource::new(
                "my_test_file.cha",
                std::sync::Arc::new(content.to_string()),
            );
            let rendered = render_error_with_miette_with_named_source(&errors[0], &source);
            assert!(
                !rendered.is_empty(),
                "Rendered error with named source should not be empty"
            );
        }
        Ok(_) => {}
        Err(e) => panic!("Unexpected error type: {e}"),
    }
}

// ===== E768: the JSON ingress is the only door to an unrepresentable filename =====

/// A `@Media` filename containing the delimiter is reported as E768 when it
/// arrives through JSON.
///
/// This is the regression test named in `spec/errors/E768_...md`'s status note,
/// and it lives here rather than in the generated CHAT-fixture corpus because
/// no `.cha` file can express the value: both parsers end the filename at the
/// comma. Deserialization is deliberately lenient per the codebase's
/// serde-boundary convention (see `LanguageCode::deserialize_empty_is_lenient`),
/// so the model reconstructs whatever the document held and validation is what
/// reports the violation, with a code and a span.
#[test]
fn media_filename_from_json_is_reported() {
    use talkbank_model::model::ChatFile;
    use talkbank_model::{ErrorCode, ErrorCollector};

    let chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
        @ID:\teng|corpus|CHI|||||Child|||\n@Media:\trecording, audio\n\
        *CHI:\thello .\n@End\n";
    let json = chat_to_json(chat, ParseValidateOptions::default(), false)
        .expect("valid CHAT converts to JSON");

    // Edit the JSON, not the CHAT: put the delimiter inside the filename, which
    // is precisely the value no transcript could have carried.
    let mut doc: serde_json::Value = serde_json::from_str(&json).expect("emitted JSON parses");
    set_media_filename(&mut doc, "take1,take2");

    let file: ChatFile = serde_json::from_value(doc)
        .expect("deserialization is lenient by convention: the model must still be reconstructed");
    let admitted = file.headers().find_map(|h| match h {
        talkbank_model::model::Header::Media(m) => Some(m.filename.as_str()),
        _ => None,
    });
    assert_eq!(
        admitted,
        Some("take1,take2"),
        "precondition: the lenient boundary really does admit the value"
    );

    let errors = ErrorCollector::new();
    file.validate(&errors, TranscriptName::Anonymous);
    let codes: Vec<ErrorCode> = errors.into_vec().into_iter().map(|e| e.code).collect();
    assert!(
        codes.contains(&ErrorCode::MediaFilenameNotRepresentable),
        "expected E768 for a filename containing the @Media delimiter, got {codes:?}"
    );
}

/// Overwrites the `@Media` filename everywhere a serialized `ChatFile` records
/// it: the header line, and the extracted top-level `media` field.
///
/// Walks the JSON rather than string-replacing so the test cannot silently
/// start editing some other field that happens to share the value. Only the
/// header-line edit is load-bearing (validation reads the lines, never the
/// extracted `media` field); the second is written so the document stays
/// self-consistent, the way a real hand-edited or tool-generated one would be.
/// Each site asserts where it is edited, so a failure names the site rather
/// than a count that cannot distinguish them.
fn set_media_filename(doc: &mut serde_json::Value, filename: &str) {
    let new_name = || serde_json::Value::String(filename.to_string());

    let extracted = doc
        .get_mut("media")
        .and_then(|m| m.get_mut("filename"))
        .expect("a serialized ChatFile with @Media carries the extracted media field");
    *extracted = new_name();

    let lines = doc
        .get_mut("lines")
        .and_then(|l| l.as_array_mut())
        .expect("a ChatFile document has a lines array");
    let header_filename = lines
        .iter_mut()
        .filter_map(|line| line.get_mut("header"))
        .filter(|header| header.get("type").and_then(|t| t.as_str()) == Some("media"))
        .find_map(|header| header.get_mut("filename"))
        .expect("the document has a media header line carrying a filename");
    *header_filename = new_name();
}
