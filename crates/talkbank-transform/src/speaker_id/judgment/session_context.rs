//! Corpus-agnostic per-session context for the holistic judgment.
//!
//! This module is the typed read boundary for the CLI's
//! `--session-context <file.json>` input: a JSON object mapping session
//! IDs to records of optional, free-vocabulary context labels that are
//! surfaced verbatim into the LLM judgment prompt.
//!
//! ```json
//! {
//!   "NF201-3": {
//!     "sample_type": "clinician interview",
//!     "declared_roles": ["Investigator"],
//!     "consent_tier": "video+audio",
//!     "age_months": 52
//!   }
//! }
//! ```
//!
//! All four record fields are optional. The labels are deliberately NOT
//! closed enums: each corpus brings its own vocabulary, and the labels'
//! only consumer is the LLM prompt, which quotes them verbatim. The seam
//! therefore stays generic; corpus-specific conversion (for example from
//! a contributor records system to this JSON) lives outside this
//! repository.
//!
//! Reading is fail-loud: a missing file, malformed JSON, a record that
//! does not match the documented shape, or a blank (empty or
//! whitespace-only) label is a typed [`SessionContextError`], never a
//! silent fallback to "no context".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::request::SessionId;

/// Which session-context label newtype rejected a blank value. Carried
/// by [`BlankLabelError`] so the error message names the offending JSON
/// field without stringly-typed provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    /// The record's `sample_type` field ([`SampleTypeLabel`]).
    SampleType,
    /// An entry of the record's `declared_roles` array ([`RoleLabel`]).
    Role,
    /// The record's `consent_tier` field ([`ConsentTierLabel`]).
    ConsentTier,
}

impl std::fmt::Display for LabelKind {
    /// Render the JSON field name the label kind corresponds to.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field = match self {
            Self::SampleType => "sample_type",
            Self::Role => "declared_roles",
            Self::ConsentTier => "consent_tier",
        };
        f.write_str(field)
    }
}

/// A context label was empty or whitespace-only. Raised by the
/// `TryFrom<String>` constructors of the label newtypes; at the JSON
/// read boundary serde folds this `Display` text into the
/// deserialization error, so the caller observes it as a
/// [`SessionContextError::Shape`]. A blank label would render a blank
/// prompt line, silently conveying nothing while looking configured.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind} label must contain at least one non-whitespace character; got {raw:?}")]
pub struct BlankLabelError {
    /// Which label field rejected the value.
    kind: LabelKind,
    /// The rejected raw string (kept for the error message).
    raw: String,
}

/// Shared constructor guard for the label newtypes: accept the raw
/// string verbatim only when it contains at least one non-whitespace
/// character, else report a [`BlankLabelError`] for `kind`.
fn non_blank_label(raw: String, kind: LabelKind) -> Result<String, BlankLabelError> {
    if raw.trim().is_empty() {
        Err(BlankLabelError { kind, raw })
    } else {
        Ok(raw)
    }
}

/// Free-vocabulary label describing what kind of speech sample the
/// session is (e.g. `"clinician interview"`, `"narrative retell"`).
/// Surfaced verbatim into the judgment prompt's `sample_type:` line; no
/// chatter-side vocabulary is imposed.
///
/// Invariant: contains at least one non-whitespace character. Enforced
/// at the JSON read boundary via `TryFrom<String>`
/// (`#[serde(try_from = "String")]`); a blank label fails the file load
/// as a [`SessionContextError::Shape`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct SampleTypeLabel(pub String);

impl TryFrom<String> for SampleTypeLabel {
    type Error = BlankLabelError;

    /// Accept any string with at least one non-whitespace character,
    /// verbatim; reject blank labels (see [`BlankLabelError`]).
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        non_blank_label(raw, LabelKind::SampleType).map(Self)
    }
}

impl SampleTypeLabel {
    /// Borrow the label text (the exact string from the context file).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SampleTypeLabel {
    /// Render the label verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Free-vocabulary label naming an adult role declared present in the
/// session (e.g. `"Investigator"`, `"Mother"`). Surfaced verbatim into
/// the judgment prompt's `declared_adult_roles:` line.
///
/// Invariant: contains at least one non-whitespace character. Enforced
/// at the JSON read boundary via `TryFrom<String>`
/// (`#[serde(try_from = "String")]`); a blank label fails the file load
/// as a [`SessionContextError::Shape`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct RoleLabel(pub String);

impl TryFrom<String> for RoleLabel {
    type Error = BlankLabelError;

    /// Accept any string with at least one non-whitespace character,
    /// verbatim; reject blank labels (see [`BlankLabelError`]).
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        non_blank_label(raw, LabelKind::Role).map(Self)
    }
}

impl RoleLabel {
    /// Borrow the label text (the exact string from the context file).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoleLabel {
    /// Render the label verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Free-vocabulary label for the media-consent tier governing what may
/// be shared for this session (e.g. `"video+audio"`, `"transcript"`).
/// Surfaced verbatim into the judgment prompt's `consent_tier:` line.
///
/// Invariant: contains at least one non-whitespace character. Enforced
/// at the JSON read boundary via `TryFrom<String>`
/// (`#[serde(try_from = "String")]`); a blank label fails the file load
/// as a [`SessionContextError::Shape`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ConsentTierLabel(pub String);

impl TryFrom<String> for ConsentTierLabel {
    type Error = BlankLabelError;

    /// Accept any string with at least one non-whitespace character,
    /// verbatim; reject blank labels (see [`BlankLabelError`]).
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        non_blank_label(raw, LabelKind::ConsentTier).map(Self)
    }
}

impl ConsentTierLabel {
    /// Borrow the label text (the exact string from the context file).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConsentTierLabel {
    /// Render the label verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Child age in months. Non-negative by construction (`u32`); JSON
/// values that are negative, fractional, or non-numeric are shape
/// errors at the read boundary. Rendered as a plain integer (e.g.
/// `"52"`) in the judgment prompt's `child_age_months:` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct AgeMonths(pub u32);

impl std::fmt::Display for AgeMonths {
    /// Render the raw month count as a plain integer so the LLM sees
    /// the exact numeric value alongside the coarse stage hint.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One session's declared context. Every field is optional: absent
/// fields are surfaced to the judgment as unknown, never guessed.
///
/// Unknown JSON keys are rejected (`deny_unknown_fields`) so a typo'd
/// field name fails loudly at the read boundary instead of silently
/// dropping context the operator intended to supply.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionContextRecord {
    /// What kind of speech sample this session is, if declared.
    #[serde(default)]
    pub sample_type: Option<SampleTypeLabel>,
    /// Adult roles declared present in the session; empty when none
    /// are declared (the LLM then infers roles from the transcript).
    #[serde(default)]
    pub declared_roles: Vec<RoleLabel>,
    /// Media-consent tier for the session, if declared.
    #[serde(default)]
    pub consent_tier: Option<ConsentTierLabel>,
    /// Child age in months at the session, if declared. When absent,
    /// the resolver falls back to the donor's CHAT `@ID` age.
    #[serde(default)]
    pub age_months: Option<AgeMonths>,
}

/// The parsed session-context file: session ID to context record.
/// `BTreeMap` keeps iteration deterministic for tests and debugging.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(transparent)]
pub struct SessionContextFile(pub BTreeMap<SessionId, SessionContextRecord>);

impl SessionContextFile {
    /// Read and parse a session-context JSON file.
    ///
    /// Fail-loud contract: I/O failures and any JSON that does not
    /// match the documented session-ID-to-record shape return a typed
    /// [`SessionContextError`]. There is no lenient mode; the caller
    /// configured a file it intended to use, so a partial or silent
    /// read would corrupt the judgment context.
    pub fn read_json(path: &Path) -> Result<Self, SessionContextError> {
        let text = std::fs::read_to_string(path).map_err(|source| SessionContextError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| SessionContextError::Shape {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Look up the record for `session_id` (the donor basename stem).
    /// `None` means the file has no record for this session; the
    /// resolver then falls back to the CHAT `@ID` age and unknowns.
    pub fn get(&self, session_id: &str) -> Option<&SessionContextRecord> {
        // `SessionId: Borrow<str>` (impl next to the type in
        // `request.rs`) lets the BTreeMap look up by the raw stem
        // without allocating a temporary key.
        self.0.get(session_id)
    }
}

/// Why a session-context file could not be read into a typed model.
#[derive(Debug, thiserror::Error)]
pub enum SessionContextError {
    /// The file could not be opened or read.
    #[error("session-context file {path}: {source}", path = .path.display())]
    Io {
        /// The path the caller asked to read.
        path: PathBuf,
        /// The underlying I/O failure.
        source: std::io::Error,
    },
    /// The file's bytes are not valid JSON, or the JSON does not have
    /// the documented session-ID-to-record shape (unknown field, wrong
    /// type, negative age, blank label, ...). Blank-label rejections
    /// originate as [`BlankLabelError`] in the label newtypes'
    /// `TryFrom<String>` constructors and surface here via serde.
    #[error("session-context file {path}: invalid JSON: {source}", path = .path.display())]
    Shape {
        /// The path the caller asked to read.
        path: PathBuf,
        /// The serde-level parse or shape failure.
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write `text` into a fresh temp file and return the handle.
    fn temp_json(text: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), text).expect("write temp json");
        file
    }

    #[test]
    fn parses_full_record() {
        let file = temp_json(
            r#"{
              "NF201-3": {
                "sample_type": "clinician interview",
                "declared_roles": ["Investigator", "Mother"],
                "consent_tier": "video+audio",
                "age_months": 52
              }
            }"#,
        );
        let parsed = SessionContextFile::read_json(file.path()).expect("parse full record");
        let record = parsed.get("NF201-3").expect("record present");
        assert_eq!(
            record.sample_type,
            Some(SampleTypeLabel("clinician interview".to_string()))
        );
        assert_eq!(
            record.declared_roles,
            vec![
                RoleLabel("Investigator".to_string()),
                RoleLabel("Mother".to_string())
            ]
        );
        assert_eq!(
            record.consent_tier,
            Some(ConsentTierLabel("video+audio".to_string()))
        );
        assert_eq!(record.age_months, Some(AgeMonths(52)));
    }

    #[test]
    fn parses_empty_record_as_all_unknown() {
        let file = temp_json(r#"{ "NF201-3": {} }"#);
        let parsed = SessionContextFile::read_json(file.path()).expect("parse empty record");
        let record = parsed.get("NF201-3").expect("record present");
        assert_eq!(record, &SessionContextRecord::default());
    }

    #[test]
    fn absent_session_returns_none() {
        let file = temp_json(r#"{ "NF201-3": {} }"#);
        let parsed = SessionContextFile::read_json(file.path()).expect("parse");
        assert!(parsed.get("OTHER-9").is_none());
    }

    #[test]
    fn malformed_json_is_shape_error() {
        let file = temp_json("{ this is not JSON");
        let err = SessionContextFile::read_json(file.path())
            .expect_err("malformed JSON must be an error");
        assert!(
            matches!(err, SessionContextError::Shape { .. }),
            "expected Shape error; got: {err}"
        );
    }

    #[test]
    fn unknown_record_field_is_shape_error() {
        let file = temp_json(r#"{ "NF201-3": { "sample_typ": "typo" } }"#);
        let err = SessionContextFile::read_json(file.path())
            .expect_err("unknown field must be a shape error, not silently dropped");
        assert!(matches!(err, SessionContextError::Shape { .. }));
    }

    #[test]
    fn negative_age_is_shape_error() {
        let file = temp_json(r#"{ "NF201-3": { "age_months": -3 } }"#);
        let err = SessionContextFile::read_json(file.path())
            .expect_err("negative age must be a shape error");
        assert!(matches!(err, SessionContextError::Shape { .. }));
    }

    #[test]
    fn wrong_field_type_is_shape_error() {
        let file = temp_json(r#"{ "NF201-3": { "age_months": "fifty-two" } }"#);
        let err = SessionContextFile::read_json(file.path())
            .expect_err("string age must be a shape error");
        assert!(matches!(err, SessionContextError::Shape { .. }));
    }

    #[test]
    fn empty_sample_type_label_is_shape_error() {
        let file = temp_json(r#"{ "NF201-3": { "sample_type": "" } }"#);
        let err = SessionContextFile::read_json(file.path())
            .expect_err("empty sample_type label must be a shape error");
        assert!(
            matches!(err, SessionContextError::Shape { .. }),
            "expected Shape error; got: {err}"
        );
    }

    #[test]
    fn whitespace_only_declared_role_is_shape_error() {
        let file = temp_json(r#"{ "NF201-3": { "declared_roles": ["   "] } }"#);
        let err = SessionContextFile::read_json(file.path())
            .expect_err("whitespace-only role label must be a shape error");
        assert!(
            matches!(err, SessionContextError::Shape { .. }),
            "expected Shape error; got: {err}"
        );
    }

    #[test]
    fn whitespace_only_consent_tier_is_shape_error() {
        let file = temp_json("{ \"NF201-3\": { \"consent_tier\": \" \\t \" } }");
        let err = SessionContextFile::read_json(file.path())
            .expect_err("whitespace-only consent tier label must be a shape error");
        assert!(
            matches!(err, SessionContextError::Shape { .. }),
            "expected Shape error; got: {err}"
        );
    }

    #[test]
    fn missing_file_is_io_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.json");
        let err =
            SessionContextFile::read_json(&path).expect_err("missing file must be an Io error");
        assert!(matches!(err, SessionContextError::Io { .. }));
    }
}
