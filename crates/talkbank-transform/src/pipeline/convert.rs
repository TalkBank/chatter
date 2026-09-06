//! Conversion pipeline helpers (CHAT <-> normalized CHAT/JSON outputs).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::json::{
    to_json_pretty_unvalidated, to_json_pretty_validated, to_json_unvalidated, to_json_validated,
};
use talkbank_model::ParseValidateOptions;

use super::error::PipelineError;
use super::parse::parse_and_validate;
use super::rewrite::Rewrite;
use talkbank_model::model::TranscriptName;

/// Parse, validate, and serialize to JSON with schema validation.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates CHAT structure (if requested via options)
/// 3. Serializes to JSON
/// 4. **Validates JSON against schema** (always)
///
/// Use [`chat_to_json_unvalidated`] to skip JSON schema validation.
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
/// * `pretty` - Pretty-print JSON output
///
/// # Returns
///
/// * `Ok(String)` - JSON string (validated against schema)
/// * `Err(PipelineError)` - Parse, validation, or serialization error
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::chat_to_json;
/// use talkbank_model::ParseValidateOptions;
///
/// # fn convert() -> Result<(), talkbank_transform::PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default();
/// let _json = chat_to_json(content, options, true)?;
/// # Ok(())
/// # }
/// ```
pub fn chat_to_json(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
) -> Result<String, PipelineError> {
    chat_to_json_named(content, options, pretty, TranscriptName::Anonymous)
}

/// JSON Schema checking is independent of CHAT validation and transcript identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonSchemaPolicy {
    /// Check serialized JSON against the embedded CHAT schema.
    Validate,
    /// Serialize without checking JSON Schema; preserve requested CHAT checks.
    Skip,
}

impl JsonSchemaPolicy {
    /// Admit the CLI's schema-only opt-out without changing parse options.
    pub const fn from_skip_flag(skip: bool) -> Self {
        if skip { Self::Skip } else { Self::Validate }
    }
}

/// Convert CHAT to JSON for a transcript whose name is known.
///
/// The name decides whether the rules comparing the transcript against its own
/// file name run, E531 above all. A caller that read the content from a path
/// has a name and should pass it: `chat_to_json` took only content, so
/// `chatter to-json` validated every transcript anonymously and silently
/// skipped E531, which is what the FOLLOW-UP note in `pipeline/parse.rs` was
/// about.
pub fn chat_to_json_named(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
    name: TranscriptName<'_>,
) -> Result<String, PipelineError> {
    chat_to_json_with_schema_policy(content, options, pretty, name, JsonSchemaPolicy::Validate)
}

/// Convert a named transcript, selecting JSON Schema checks after CHAT parsing.
/// Both schema policies use the same transcript identity and CHAT validation.
pub fn chat_to_json_with_schema_policy(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
    name: TranscriptName<'_>,
    schema: JsonSchemaPolicy,
) -> Result<String, PipelineError> {
    let parser = talkbank_parser::TreeSitterParser::new()
        .map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    let chat_file = super::parse::parse_and_validate_named(&parser, content, options, name)?;
    match (schema, pretty) {
        (JsonSchemaPolicy::Validate, true) => to_json_pretty_validated(&chat_file),
        (JsonSchemaPolicy::Validate, false) => to_json_validated(&chat_file),
        (JsonSchemaPolicy::Skip, true) => to_json_pretty_unvalidated(&chat_file),
        (JsonSchemaPolicy::Skip, false) => to_json_unvalidated(&chat_file),
    }
    .map_err(|e| PipelineError::JsonSerialization(e.to_string()))
}

/// Parse, validate, and serialize to JSON WITHOUT schema validation.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates CHAT structure (if requested via options)
/// 3. Serializes to JSON (without schema validation)
///
/// **Use sparingly.** Prefer [`chat_to_json`] which validates JSON against schema.
/// This variant is useful for:
/// - Performance-critical paths where validation is done elsewhere
/// - Testing specific serialization behavior
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
/// * `pretty` - Pretty-print JSON output
///
/// # Returns
///
/// * `Ok(String)` - JSON string (NOT validated against schema)
/// * `Err(PipelineError)` - Parse, validation, or serialization error
pub fn chat_to_json_unvalidated(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
) -> Result<String, PipelineError> {
    chat_to_json_with_schema_policy(
        content,
        options,
        pretty,
        TranscriptName::Anonymous,
        JsonSchemaPolicy::Skip,
    )
}

/// Parse and rewrite CHAT into canonical serialized form.
///
/// This pipeline function:
/// 1. Parses CHAT content
/// 2. Validates (if requested)
/// 3. Serializes back to canonical CHAT format
///
/// # Arguments
///
/// * `content` - The CHAT file content
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(String)` - Canonical CHAT string
/// * `Err(PipelineError)` - Parse or validation error
///
/// # Example
///
/// ```no_run
/// use talkbank_transform::normalize_chat;
/// use talkbank_model::ParseValidateOptions;
///
/// # fn normalize() -> Result<(), talkbank_transform::PipelineError> {
/// let content = "*CHI:\thello world .";
/// let options = ParseValidateOptions::default().with_validation();
/// let _normalized = normalize_chat(content, options)?;
/// # Ok(())
/// # }
/// ```
pub fn normalize_chat(
    content: &str,
    options: ParseValidateOptions,
) -> Result<Rewrite, PipelineError> {
    let chat_file = parse_and_validate(content, options)?;
    Rewrite::of(&chat_file, content).map_err(PipelineError::DroppedContent)
}

#[cfg(test)]
mod tests {
    use super::{chat_to_json, normalize_chat};
    use crate::PipelineError;
    use talkbank_model::ParseValidateOptions;

    #[test]
    fn test_chat_to_json_pretty() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let json = chat_to_json(content, options, true)?;
        assert!(json.contains("{\n")); // Pretty-printed
        Ok(())
    }

    #[test]
    fn test_chat_to_json_compact() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let json = chat_to_json(content, options, false)?;
        assert!(!json.contains("  ")); // Not pretty-printed
        Ok(())
    }

    #[test]
    fn test_normalize_chat() -> Result<(), PipelineError> {
        let content = "@UTF8\n@Begin\n@End\n";
        let options = ParseValidateOptions::default();
        let _ = normalize_chat(content, options)?;
        Ok(())
    }
}
