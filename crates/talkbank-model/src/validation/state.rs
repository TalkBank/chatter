//! Owned validation evidence. Mutable models and accepted models have distinct APIs.

use crate::model::{FileStem, TranscriptName};
use crate::{ChatFile, ErrorCollector, ErrorSink, ParseError, RuleSelection, WriteChat};

/// Whether validation also computes and checks dependent-tier alignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentValidation {
    /// Check model rules without computing tier alignment.
    Structure,
    /// Also compute and check tier alignments.
    IncludeTierAlignment,
}

/// The exact rule selection and alignment coverage of a validation attempt.
/// Warnings are retained; any error rejects the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationPolicy {
    rules: RuleSelection,
    alignment: AlignmentValidation,
}

impl ValidationPolicy {
    /// Select rules and alignment coverage explicitly.
    pub fn new(rules: RuleSelection, alignment: AlignmentValidation) -> Self {
        Self { rules, alignment }
    }

    /// Rules actually run by this policy.
    pub fn rules(&self) -> RuleSelection {
        self.rules
    }

    /// Alignment coverage actually run by this policy.
    pub fn alignment(&self) -> AlignmentValidation {
        self.alignment
    }
}

#[derive(Debug, Clone)]
enum CheckedName {
    Anonymous,
    Named(String),
}

impl CheckedName {
    fn capture(name: TranscriptName<'_>) -> Self {
        match name {
            TranscriptName::Anonymous => Self::Anonymous,
            TranscriptName::Named(stem) => Self::Named(stem.as_str().to_owned()),
        }
    }

    fn as_name(&self) -> TranscriptName<'_> {
        match self {
            Self::Anonymous => TranscriptName::Anonymous,
            Self::Named(stem) => TranscriptName::Named(FileStem::from_stem(stem)),
        }
    }
}

/// Immutable model accepted by the recorded validation policy.
///
/// This proves model validation, not that a source recording agrees with its
/// transcription. Source parsing diagnostics must be handled before this boundary.
/// It cannot be deserialized or constructed from a raw model. Editing consumes
/// the proof through [`Self::into_unchecked`]. Serialization preserves the existing
/// CHAT/JSON representation without serializing the evidence as transcript content.
///
/// ```compile_fail
/// use talkbank_model::validation::ValidChatFile;
/// fn edit(file: &mut ValidChatFile) { file.document().lines = Vec::new().into(); }
/// ```
///
/// ```compile_fail
/// use talkbank_model::validation::ValidChatFile;
/// let forged: ValidChatFile = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ValidChatFile {
    document: ChatFile,
    policy: ValidationPolicy,
    name: CheckedName,
    diagnostics: Vec<ParseError>,
}

impl ValidChatFile {
    /// Borrow the accepted payload without mutation authority.
    pub fn document(&self) -> &ChatFile {
        &self.document
    }

    /// Discard validity evidence before editing or transforming the payload.
    pub fn into_unchecked(self) -> ChatFile {
        self.document
    }

    /// The policy under which this payload was accepted.
    pub fn policy(&self) -> ValidationPolicy {
        self.policy
    }

    /// The name used for filename-dependent checks, or explicit anonymity.
    pub fn name(&self) -> TranscriptName<'_> {
        self.name.as_name()
    }

    /// Diagnostics retained from the successful attempt (warnings only).
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.diagnostics
    }
}

impl WriteChat for ValidChatFile {
    fn write_chat<W: std::fmt::Write>(&self, writer: &mut W) -> std::fmt::Result {
        self.document.write_chat(writer)
    }
}

impl serde::Serialize for ValidChatFile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.document.serialize(serializer)
    }
}

/// Rejected model and its evidence, retained for inspection or repair.
#[derive(Debug)]
pub struct ValidationFailure {
    document: Box<ChatFile>,
    diagnostics: Vec<ParseError>,
    policy: ValidationPolicy,
    name: CheckedName,
    incomplete_parse: bool,
}

impl ValidationFailure {
    /// Inspect the original rejected model.
    pub fn document(&self) -> &ChatFile {
        &self.document
    }

    /// Recover ownership of the rejected model for repair.
    pub fn into_unchecked(self) -> ChatFile {
        *self.document
    }

    /// All diagnostics, including warnings, emitted during the attempt.
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.diagnostics
    }

    /// Rule selection and alignment coverage of the failed attempt.
    pub fn policy(&self) -> ValidationPolicy {
        self.policy
    }

    /// Name against which the model was checked.
    pub fn name(&self) -> TranscriptName<'_> {
        self.name.as_name()
    }

    /// Whether unknown or recovered tier provenance prevented full checking.
    pub fn has_incomplete_parse(&self) -> bool {
        self.incomplete_parse
    }
}

impl std::fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "model validation failed")?;
        if self.incomplete_parse {
            write!(f, ": unknown or recovered parse provenance")?;
        }
        for diagnostic in &self.diagnostics {
            write!(f, "\n  {} {}", diagnostic.code.as_str(), diagnostic.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationFailure {}

/// Record each diagnostic before forwarding it to the caller's presentation sink.
struct RecordingSink<'a, S> {
    collected: &'a ErrorCollector,
    target: &'a S,
}

impl<S: ErrorSink> ErrorSink for RecordingSink<'_, S> {
    fn report(&self, diagnostic: ParseError) {
        self.collected.report(diagnostic.clone());
        self.target.report(diagnostic);
    }
}

impl ChatFile {
    /// Validate with default model rules, accepting warnings and rejecting errors.
    pub fn validate_into(
        self,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> Result<ValidChatFile, ValidationFailure> {
        self.validate_with_policy(
            ValidationPolicy::new(RuleSelection::new(), AlignmentValidation::Structure),
            errors,
            name,
        )
    }

    /// Consume a mutable model and retain a proof only if the selected checks pass.
    /// The internal collector owns the verdict: even a sink that discards errors
    /// cannot change rejection into success. Unknown/recovered tier provenance
    /// also rejects, because validation may have skipped checks on those tiers.
    pub fn validate_with_policy(
        mut self,
        policy: ValidationPolicy,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> Result<ValidChatFile, ValidationFailure> {
        let collected = ErrorCollector::new();
        let sink = RecordingSink {
            collected: &collected,
            target: errors,
        };
        let incomplete_parse = self.utterances().any(|u| !u.parse_health.is_clean());
        match policy.alignment {
            AlignmentValidation::Structure => self.validate_with_rules(policy.rules, &sink, name),
            AlignmentValidation::IncludeTierAlignment => {
                self.validate_with_alignment_and_rules(policy.rules, &sink, name);
            }
        }
        let rejected = incomplete_parse || collected.has_errors();
        let diagnostics = collected.into_vec();
        let name = CheckedName::capture(name);
        if rejected {
            Err(ValidationFailure {
                document: Box::new(self),
                diagnostics,
                policy,
                name,
                incomplete_parse,
            })
        } else {
            Ok(ValidChatFile {
                document: self,
                policy,
                name,
                diagnostics,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::TranscriptName;
    use crate::{ChatFile, NullErrorSink};

    /// Discarding diagnostics must not turn invalid input into a validity proof.
    #[test]
    fn discarded_errors_cannot_authorize_valid_output() {
        let result = ChatFile::new(vec![]).validate_into(&NullErrorSink, TranscriptName::Anonymous);
        assert!(result.is_err());
        let rejected = result.unwrap_err();
        assert!(!rejected.diagnostics().is_empty());
        assert!(rejected.into_unchecked().lines.is_empty());
    }
}
