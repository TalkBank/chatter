//! Regenerate the canonical CHAT JSON Schema from `talkbank_model::ChatFile`.
//!
//! `schema_for!(ChatFile)` produces the Draft 2020-12 schema. Only top-level
//! metadata is added before the result is written to the workspace
//! `schema/chat-file.schema.json`. Run with `--nocapture` to see the summary.

mod io;
mod metadata;

use schemars::schema_for;
use talkbank_model::ChatFile;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Metadata error: {source}")]
    Metadata { source: metadata::MetadataError },
    #[error("IO error: {source}")]
    Io { source: io::IoError },
}

/// Build the canonical schema JSON string from the live `ChatFile` model.
///
/// Single source of truth for both the regenerator (`generate_chat_file_schema`)
/// and the staleness guard (`committed_schema_matches_model`), so the two can
/// never drift apart.
fn build_canonical_schema_json() -> Result<String, TestError> {
    let schema = schema_for!(ChatFile);
    let mut schema_value =
        metadata::schema_to_value(schema).map_err(|source| TestError::Metadata { source })?;

    metadata::add_schema_metadata(
        &mut schema_value,
        "https://talkbank.org/schemas/v0.1/chat-file.json",
        "JSON Schema for TalkBank CHAT format transcript files. \
         This schema defines the structure of CHAT files when serialized to JSON.",
        "modify the Rust model and run `just schema-gen`",
    );

    metadata::to_pretty_json(&schema_value).map_err(|source| TestError::Metadata { source })
}

/// Generates the chat-file JSON schema and writes the canonical file.
#[test]
#[ignore = "writes the canonical schema; run just schema-gen explicitly"]
fn generate_chat_file_schema() -> Result<(), TestError> {
    let schema_json = build_canonical_schema_json()?;
    let canonical_path = io::schema_path_for("chat-file.schema");
    io::write_schema_file(&canonical_path, &schema_json)
        .map_err(|source| TestError::Io { source })?;
    io::print_summary(&canonical_path, schema_json.len());

    Ok(())
}

/// Guard against forgetting to regenerate the schema after a model change.
///
/// The committed `schema/chat-file.schema.json` is embedded at compile time as
/// `talkbank_transform::SCHEMA_JSON` (and `chatter to-json` validates its own
/// output against it). If a model type's shape/fields/serde/doc comments change
/// without regenerating, the freshly-built schema diverges from the embedded
/// one. This test fails in that case with a clear instruction, so the staleness
/// can never ship silently.
#[test]
fn committed_schema_matches_model() -> Result<(), TestError> {
    let generated = build_canonical_schema_json()?;
    let committed = talkbank_transform::SCHEMA_JSON;
    assert!(
        generated.trim_end() == committed.trim_end(),
        "schema/chat-file.schema.json is stale relative to the talkbank-model types. \
         Run `just schema-gen` and rebuild, \
         then commit the regenerated schema."
    );
    Ok(())
}

/// Draft 2020-12 evaluates both a referenced payload and its sibling tag.
#[test]
fn generated_ref_siblings_enforce_tag_and_payload() -> Result<(), Box<dyn std::error::Error>> {
    let canonical: serde_json::Value = serde_json::from_str(&build_canonical_schema_json()?)?;
    let schema = serde_json::json!({
        "$schema": canonical["$schema"],
        "$defs": canonical["$defs"],
        "$ref": "#/$defs/BracketedItem",
    });
    let validator = jsonschema::validator_for(&schema)?;
    let mut word = serde_json::to_value(talkbank_model::Word::simple("hello"))?;
    word["type"] = serde_json::json!("word");
    assert!(validator.is_valid(&word));
    word["type"] = serde_json::json!("unknown_tag");
    assert!(
        !validator.is_valid(&word),
        "the sibling tag must be enforced"
    );
    word["type"] = serde_json::json!("word");
    word["raw_text"] = serde_json::json!(42);
    assert!(
        !validator.is_valid(&word),
        "the referenced payload must be enforced"
    );
    Ok(())
}
