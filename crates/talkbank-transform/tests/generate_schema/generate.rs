//! Regenerate the canonical CHAT JSON Schema from `talkbank_model::ChatFile`.
//!
//! `schema_for!(ChatFile)` produces the schemars schema; a post-process fixes a
//! schemars bug with internally-tagged enums (see `transform.rs`), then the
//! top-level metadata is added and the result is written to the workspace
//! `schema/chat-file.schema.json`. Run with `--nocapture` to see the summary.

mod io;
mod metadata;
mod transform;

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

    // Fix schemars bug: internally-tagged enums with $ref generate invalid JSON Schema.
    // See transform.rs for details.
    transform::fix_ref_properties_combination(&mut schema_value);

    metadata::add_schema_metadata(
        &mut schema_value,
        "https://talkbank.org/schemas/v0.1/chat-file.json",
        "JSON Schema for TalkBank CHAT format transcript files. \
         This schema defines the structure of CHAT files when serialized to JSON.",
        "modify src/model/*.rs types and run `cargo test --test generate_schema`",
    );

    metadata::to_pretty_json(&schema_value).map_err(|source| TestError::Metadata { source })
}

/// Generates the chat-file JSON schema and writes the canonical file.
#[test]
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
    assert_eq!(
        generated.trim_end(),
        committed.trim_end(),
        "schema/chat-file.schema.json is stale relative to the talkbank-model types. \
         Run `cargo test -p talkbank-transform --test generate_schema` and rebuild, \
         then commit the regenerated schema."
    );
    Ok(())
}
