//! Source, admitted model and selected private mapping have one owner.

use std::fmt;

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidChatFile, ValidationPolicy};
use talkbank_model::{ErrorCollector, RuleSelection};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::ParsedSource;

use super::TranscriptNames;

/// A source document admitted for selective-pseudonymization planning.
///
/// Construction parses this source exactly once, requires alignment-aware
/// validation with explicit filename/rule context, and binds the immutable
/// result to both source and selected map. Callers cannot substitute a different
/// model or mutate it while retaining this evidence. A rewrite remains bound
/// to this input and must establish a separate output proof.
///
/// Admission does not imply that any name occurs, or that a safe rewrite exists.
/// The accessors expose sensitive **input**, not de-identified output; this type
/// deliberately implements neither `WriteChat` nor `Serialize`.
pub struct PseudonymizationInput<'source, 'map> {
    parsed: ParsedSource<'source>,
    document: ValidChatFile,
    names: &'map TranscriptNames,
}

/// Source admission failed. Error text and chains never contain transcript text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InputRefusal {
    /// Parsing rejected or recovered malformed input.
    #[error("pseudonymization input could not be parsed without recovery")]
    Parsing,
    /// Model or alignment validation failed under the selected context.
    #[error("pseudonymization input failed validation")]
    Validation,
    /// Warnings remained even though no error was emitted.
    #[error("pseudonymization input requires diagnostic review")]
    DiagnosticReview,
}

impl TranscriptNames {
    /// Bind a private name selection to a source admitted for planning.
    ///
    /// Alignment checks cannot be disabled. The caller explicitly supplies the
    /// transcript name used for filename-dependent rules (or explicit anonymity)
    /// and optional rule selection. Warnings also refuse admission: the planner
    /// must not silently proceed past uncertain or unsupported input.
    ///
    /// Detailed parser/validator diagnostics contain protected transcript text.
    /// They are retained only inside this call and never attached as an error
    /// source, printed, or passed to a caller-provided logging sink.
    pub fn admit_document<'source, 'map>(
        &'map self,
        source: &'source str,
        name: TranscriptName<'_>,
        rules: RuleSelection,
        parser: &TreeSitterParser,
    ) -> Result<PseudonymizationInput<'source, 'map>, InputRefusal> {
        let diagnostics = ErrorCollector::new();
        let policy = ValidationPolicy::new(rules, AlignmentValidation::IncludeTierAlignment);
        let (file, parsed) = parser.parse_chat_file_with_source(source, &diagnostics);
        let parsed = parsed.ok_or(InputRefusal::Parsing)?;
        if diagnostics.has_errors() {
            return Err(InputRefusal::Parsing);
        }
        let document = file
            .validate_with_policy(policy, &diagnostics, name)
            .map_err(|_| InputRefusal::Validation)?;
        if !diagnostics.is_empty() {
            return Err(InputRefusal::DiagnosticReview);
        }
        Ok(PseudonymizationInput {
            parsed,
            document,
            names: self,
        })
    }
}

impl<'source, 'map> PseudonymizationInput<'source, 'map> {
    /// Consume retained parse ownership after the output stability review.
    pub(super) fn into_document(self) -> ValidChatFile {
        self.document
    }
    /// Original protected source, associated with the model at construction.
    pub fn source(&self) -> &'source str {
        self.parsed.source()
    }

    /// Exact producer-owned CST for source-bound generated field traversal.
    /// It is immutable and was lowered in the same parse as this input's model.
    pub fn parsed_source(&self) -> &ParsedSource<'source> {
        &self.parsed
    }

    /// Immutable accepted model and its exact validation context.
    pub fn document(&self) -> &ValidChatFile {
        &self.document
    }

    /// Private mapping selected before this source was admitted.
    pub fn names(&self) -> &'map TranscriptNames {
        self.names
    }
}

impl fmt::Debug for PseudonymizationInput<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PseudonymizationInput(<private>)")
    }
}
