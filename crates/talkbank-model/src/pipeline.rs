//! Pipeline options and helpers for CHAT file processing workflows.
//!
//! This module is a thin configuration layer that sits between the parser crate
//! (`talkbank-parser`) and the orchestration layer (`talkbank-transform`).
//! It defines the option
//! structs that control *what happens after parsing* -- specifically, which
//! validation phases to run.
//!
//! # Architecture
//!
//! ```text
//!   Parser crate              talkbank-model::pipeline  talkbank-transform
//!   ┌──────────────┐         ┌──────────────────┐      ┌──────────────────┐
//!   │ tree-sitter  │─parse──▶│ ParseValidateOpts│─────▶│ parse_and_validate│
//!   │ direct       │         │ validate_chat_... │      │ (orchestrator)   │
//!   └──────────────┘         └──────────────────┘      └──────────────────┘
//! ```
//!
//! ## Builder pattern
//!
//! [`ParseValidateOptions`] uses a builder pattern with `with_*` methods that
//! return `Self`, enabling fluent configuration chains. The default is
//! parse-only (no validation):
//!
//! | Configuration | [`CheckLevel`] | Effect |
//! |---------------|----------------|--------|
//! | `default()` | `ParseOnly` | Parse only, skip all validation |
//! | `with_validation()` | `Validate(Structure)` | Structural validation (headers, speakers) |
//! | `with_alignment()` | `Validate(IncludeTierAlignment)` | Full validation including cross-tier alignment |
//!
//! # Examples
//!
//! Parse-only (fastest, used by roundtrip re-parse):
//!
//! ```
//! use talkbank_model::ParseValidateOptions;
//!
//! let options = ParseValidateOptions::default();
//! assert!(options.validation_policy().is_none());
//! ```
//!
//! Structural validation without alignment (for quick checks):
//!
//! ```
//! use talkbank_model::validation::AlignmentValidation;
//! use talkbank_model::{CheckLevel, ParseValidateOptions};
//!
//! let options = ParseValidateOptions::default().with_validation();
//! assert_eq!(options.level(), CheckLevel::Validate(AlignmentValidation::Structure));
//! ```
//!
//! Full validation with cross-tier alignment (the standard pipeline):
//!
//! ```
//! use talkbank_model::validation::AlignmentValidation;
//! use talkbank_model::{CheckLevel, ParseValidateOptions};
//!
//! // with_alignment() implies with_validation()
//! let options = ParseValidateOptions::default().with_alignment();
//! assert_eq!(
//!     options.level(),
//!     CheckLevel::Validate(AlignmentValidation::IncludeTierAlignment)
//! );
//! ```

use crate::ParseError;

use crate::ChatFile;
use crate::validation::{AlignmentValidation, ValidationPolicy};

/// Minimum structural validity required before a higher-level pipeline should
/// spend additional work on a parsed CHAT file.
///
/// These levels are cumulative and intentionally coarser than the full
/// validator's error taxonomy. They exist so orchestration layers can decide
/// whether a file is safe enough for downstream processing without hard-coding
/// command-specific checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidityLevel {
    /// L0: The file parsed without syntax-level parse errors.
    Parseable = 0,
    /// L1: The file is structurally complete enough for text-oriented passes.
    StructurallyComplete = 1,
    /// L2: The main tier content is well-formed enough for word-sensitive work.
    MainTierValid = 2,
}

/// A coarse-grained preflight validation failure tagged with the minimum gate
/// level it belongs to.
///
/// This is intentionally lighter-weight than the full parser/validator error
/// model. It exists for orchestration layers that need actionable gate failures
/// without depending on command-specific plumbing.
#[derive(Debug, Clone)]
pub struct GateValidationError {
    /// Human-readable description of the failed gate.
    pub message: String,
    /// The gate level at which the failure occurred.
    pub level: ValidityLevel,
}

impl std::fmt::Display for GateValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[L{}] {}", self.level as u8, self.message)
    }
}

/// Which validation runs after parsing: none, or validation at an
/// [`AlignmentValidation`] coverage. "Alignment without validation" is not a
/// value: it was, when the options were two public booleans, and every
/// reader had to remember that `alignment` implied `validate`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckLevel {
    /// Parse only; no validation (the default).
    #[default]
    ParseOnly,
    /// Validate, with or without cross-tier alignment.
    Validate(AlignmentValidation),
}

/// Options for parse-and-validate pipeline.
///
/// Built with the `with_*` methods; the fields are private so the level is
/// always a [`CheckLevel`].
#[derive(Debug, Clone, Default)]
pub struct ParseValidateOptions {
    /// Which validation runs.
    level: CheckLevel,
    /// The rules validation runs, when it runs. Held by a parse-only value
    /// too, so the builder methods compose in any order: strict linkers
    /// select rules but do not enable validation on their own.
    rules: crate::RuleSelection,
}

impl ParseValidateOptions {
    /// Run validation at exactly this level.
    pub fn with_level(mut self, level: CheckLevel) -> Self {
        self.level = level;
        self
    }

    /// Enable structural/data-model validation. Keeps alignment if it was
    /// already requested.
    pub fn with_validation(mut self) -> Self {
        self.level = match self.level {
            CheckLevel::ParseOnly => CheckLevel::Validate(AlignmentValidation::Structure),
            level @ CheckLevel::Validate(_) => level,
        };
        self
    }

    /// Enable alignment validation and its prerequisite structural validation.
    pub fn with_alignment(mut self) -> Self {
        self.level = CheckLevel::Validate(AlignmentValidation::IncludeTierAlignment);
        self
    }

    /// Enable strict cross-utterance linker validation.
    ///
    /// Checks that the quotation linkers (`+"`, `+"/. `, `+".`) and the completion
    /// linkers (`+,`, `++`) pair with the terminators they continue. The codes it
    /// turns on are listed per code in the generated error index; a range written
    /// here said "E351-E355" in nine places and omitted three codes in all nine.
    pub fn with_strict_linkers(mut self) -> Self {
        self.rules = self.rules.with_strict_linkers();
        self
    }

    /// Which validation runs.
    pub fn level(&self) -> CheckLevel {
        self.level
    }

    /// Admit the requested validation phase before handing it to an executor.
    /// `None` means parse-only; strict linkers select rules but do not enable
    /// validation on their own.
    pub fn validation_policy(&self) -> Option<ValidationPolicy> {
        match self.level {
            CheckLevel::ParseOnly => None,
            CheckLevel::Validate(alignment) => Some(ValidationPolicy::new(self.rules, alignment)),
        }
    }
}

/// Validate a parsed ChatFile according to options.
///
/// This helper function validates a ChatFile according to the options,
/// returning validation errors if any.
///
/// # Arguments
///
/// * `chat_file` - Parsed CHAT model.
///   Alignment validation may annotate alignment state, so `&mut` is required.
/// * `options` - Validation options
///
/// # Returns
///
/// * `Ok(())` - Validation passed or was skipped.
/// * `Err(Vec<ParseError>)` - At least one validation error/warning was emitted.
pub fn validate_chat_file_with_options(
    chat_file: &mut ChatFile,
    options: &ParseValidateOptions,
) -> Result<(), Vec<ParseError>> {
    use crate::ErrorCollector;
    use crate::model::TranscriptName;

    let Some(policy) = options.validation_policy() else {
        return Ok(());
    };

    let errors = ErrorCollector::new();
    // This helper takes a parsed file and options, never a path, so the
    // transcript is genuinely anonymous here and E531 does not apply. Callers
    // that read from disk name the transcript themselves. `validate_at` is
    // the one place a coverage becomes a validator call.
    chat_file.validate_at(policy, &errors, TranscriptName::Anonymous);

    let error_vec = errors.into_vec();
    if !error_vec.is_empty() {
        Err(error_vec)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The options name the policy a run validates with: none when parsing
    /// only (strict linkers alone enable nothing), the requested coverage and
    /// linker rules otherwise. `validate_chat_file_with_options` hands that
    /// policy to `ChatFile::validate_at`, the one place a coverage becomes a
    /// validator call.
    #[test]
    fn options_name_the_requested_policy() {
        let parse_only = ParseValidateOptions::default().with_strict_linkers();
        assert_eq!(parse_only.validation_policy(), None);
        for alignment in [
            AlignmentValidation::Structure,
            AlignmentValidation::IncludeTierAlignment,
        ] {
            for (strict, rules) in [
                (false, crate::RuleSelection::new()),
                (true, crate::RuleSelection::new().with_strict_linkers()),
            ] {
                let options =
                    ParseValidateOptions::default().with_level(CheckLevel::Validate(alignment));
                let options = match strict {
                    true => options.with_strict_linkers(),
                    false => options,
                };
                assert_eq!(
                    options.validation_policy(),
                    Some(ValidationPolicy::new(rules, alignment))
                );
            }
        }
    }
}
