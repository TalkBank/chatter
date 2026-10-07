//! Owned validation evidence. Mutable models and accepted models have distinct APIs.

use crate::model::{OwnedTranscriptName, TranscriptName};
use crate::{
    ChatFile, CompletedDiagnostics, ErrorCollector, ErrorSink, ParseError, RuleSelection, WriteChat,
};

/// Whether validation also computes and checks dependent-tier alignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Immutable model accepted by the recorded validation policy.
///
/// This proves model validation, not that a source recording agrees with its
/// transcription. Source parsing diagnostics must be handled before this boundary.
/// It cannot be deserialized or constructed from a raw model. Editing consumes
/// the proof through [`Self::into_unchecked`]. Serialization preserves the existing
/// CHAT/JSON representation without serializing the evidence as transcript content.
///
/// ```compile_fail,E0594
/// # fn edit(file: &mut talkbank_model::validation::ValidChatFile) {
/// file.document().lines = Vec::new().into();
/// # }
/// ```
///
/// ```compile_fail,E0277
/// # use talkbank_model::validation::ValidChatFile;
/// let forged: ValidChatFile = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ValidChatFile {
    document: ChatFile,
    policy: ValidationPolicy,
    name: OwnedTranscriptName,
    diagnostics: Vec<ParseError>,
}

impl ValidChatFile {
    /// Borrow the accepted payload without mutation authority.
    pub fn document(&self) -> &ChatFile {
        &self.document
    }

    /// Discard validity evidence before editing or transforming the payload.
    pub fn into_unchecked(self) -> ChatFile {
        let mut document = self.document;
        for line in &mut document.lines {
            if let crate::model::Line::Utterance(utterance) = line {
                utterance.forget_construction_admission();
            }
        }
        document
    }

    /// The policy under which this payload was accepted.
    pub fn policy(&self) -> ValidationPolicy {
        self.policy
    }

    /// The name used for filename-dependent checks, or explicit anonymity.
    pub fn name(&self) -> TranscriptName<'_> {
        self.name.borrow()
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

/// An outstanding linked-media requirement, issued only by regeneration
/// validation. This is not document validity or output permission.
///
/// It is discharged only through the [`PendingTimingChatFile`] that carries
/// it, against the document that payload holds and hands back: the
/// obligation alone has no discharge, so its verdict cannot concern a
/// document other than the one a consumer goes on to admit.
///
/// ```compile_fail,E0599
/// # fn judge(
/// #     pending: &talkbank_model::validation::PendingTimingChatFile,
/// #     other: &talkbank_model::ChatFile,
/// # ) {
/// let _ = pending.obligation().clone().discharge(other);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MediaTimingObligation {
    header_span: crate::Span,
}

impl MediaTimingObligation {
    /// The retained declaration that still requires actual timing evidence.
    pub fn header_span(&self) -> crate::Span {
        self.header_span
    }
}

/// Retained structure accepted for timing regeneration, not complete CHAT.
/// It cannot be serialized as an accepted document or converted to ValidChatFile.
///
/// ```compile_fail,E0599
/// # fn certify(pending: talkbank_model::validation::PendingTimingChatFile) {
/// let _ = pending.into_valid_file();
/// # }
/// ```
#[derive(Debug)]
pub struct PendingTimingChatFile {
    document: ChatFile,
    obligation: MediaTimingObligation,
    diagnostics: Vec<ParseError>,
}

impl PendingTimingChatFile {
    /// Inspect the retained working structure without asserting validity.
    pub fn document(&self) -> &ChatFile {
        &self.document
    }
    /// Mutate the working document, for instance to write regenerated timing
    /// into it, while the obligation stays attached to this same document.
    pub fn document_mut(&mut self) -> &mut ChatFile {
        &mut self.document
    }
    /// Outstanding requirement associated with this payload.
    pub fn obligation(&self) -> &MediaTimingObligation {
        &self.obligation
    }
    /// Warnings from retained-structure admission, not filtered errors.
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.diagnostics
    }
    /// Discharge the obligation against this payload's document, through the
    /// very check that issued it (E544's): it is discharged exactly when that
    /// check no longer applies, because the document now carries timing
    /// evidence or no longer declares linked media without a status. The
    /// document judged is the document handed back, so the verdict always
    /// concerns the document a consumer goes on to admit. It does not prove
    /// that document derives from the original: [`Self::document_mut`]
    /// permits any edit, a whole replacement included.
    ///
    /// The retained-structure warnings ([`Self::diagnostics`]) are dropped on
    /// success: complete admission of the returned document produces its own.
    ///
    /// # Errors
    /// Hands the payload back, unchanged and boxed (it holds a whole
    /// document), while E544 would still fire. Complete admission of the
    /// returned document still re-runs every rule; this is the obligation's
    /// own verdict, not output permission.
    pub fn discharge(self) -> Result<ChatFile, Box<Self>> {
        match self.document.untimed_media_declaration() {
            None => Ok(self.document),
            Some(_) => Err(Box::new(self)),
        }
    }
}

/// Regeneration may start from a complete retained document or from a checked
/// working document with an explicit remaining media/timing obligation.
#[derive(Debug)]
pub enum TimingRegenerationAdmission {
    /// All requirements are already satisfied.
    Ready(ValidChatFile),
    /// Timing must be established before complete output admission.
    Pending(PendingTimingChatFile),
}

enum AdmissionPhase {
    Complete,
    RegeneratingTiming,
}

/// Rejected model and its evidence, retained for inspection or repair.
#[derive(Debug)]
pub struct ValidationFailure {
    document: Box<ChatFile>,
    diagnostics: Vec<ParseError>,
    policy: ValidationPolicy,
    name: OwnedTranscriptName,
    reason: ValidationFailureReason,
}

/// Why no accepted-model evidence could be issued. Internal failure takes
/// precedence over input findings and does not assert invalid CHAT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationFailureReason {
    InternalFailure,
    IncompleteParse,
    Invalidity,
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

    /// Recover the rejected model together with the diagnostics that rejected
    /// it, both moved rather than copied, for a repair that must keep the
    /// evidence bound to what it removes. A tool failure is not evidence
    /// about the model, so it is handed back unchanged as `Err`.
    pub fn into_rejection(self) -> Result<(ChatFile, Vec<ParseError>), Self> {
        match self.reason {
            ValidationFailureReason::InternalFailure => Err(self),
            ValidationFailureReason::IncompleteParse | ValidationFailureReason::Invalidity => {
                Ok((*self.document, self.diagnostics))
            }
        }
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
        self.name.borrow()
    }

    /// Whether unknown or recovered tier provenance prevented full checking.
    pub fn has_incomplete_parse(&self) -> bool {
        self.reason == ValidationFailureReason::IncompleteParse
    }

    /// Whether the tool failed instead of completing a validity determination.
    pub fn has_internal_failure(&self) -> bool {
        self.reason == ValidationFailureReason::InternalFailure
    }
}

impl std::fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.reason {
            ValidationFailureReason::InternalFailure => {
                write!(f, "internal tool failure; CHAT validity was not determined")?;
            }
            ValidationFailureReason::IncompleteParse => {
                write!(
                    f,
                    "model validation incomplete: unknown or recovered parse provenance"
                )?;
            }
            ValidationFailureReason::Invalidity => write!(f, "model validation failed")?,
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
    /// Validate an explicitly assembled typed document without reparsing CHAT.
    ///
    /// This certifies only the supplied structure under `policy`, never its
    /// completeness relative to original source text. Recorded parser recovery
    /// still rejects; it is not erased by this operation. Unknown utterances are
    /// checked as new constructions, including alignment when selected. Only
    /// success returns construction admission, inside an immutable proof.
    /// Failure returns the payload without newly granted runtime authority.
    pub fn validate_construction_with_policy(
        mut self,
        policy: ValidationPolicy,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> Result<ValidChatFile, ValidationFailure> {
        for line in &mut self.lines {
            if let crate::model::Line::Utterance(utterance) = line {
                utterance.prepare_construction_validation();
            }
        }
        self.validate_with_policy(policy, errors, name)
            .map_err(|mut failure| {
                for line in &mut failure.document.lines {
                    if let crate::model::Line::Utterance(utterance) = line {
                        utterance.forget_construction_admission();
                    }
                }
                failure
            })
    }

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
        self,
        policy: ValidationPolicy,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> Result<ValidChatFile, ValidationFailure> {
        let (document, diagnostics, _) =
            self.validate_owned(policy, errors, name, AdmissionPhase::Complete)?;
        Ok(ValidChatFile {
            document,
            policy,
            name: name.to_owned_name(),
            diagnostics,
        })
    }

    /// Admit retained structure for a timing-producing operation, without
    /// treating an outstanding linked-media requirement as complete validity.
    /// All default structure, alignment, provenance and internal-failure checks
    /// remain mandatory. Source-dependent eligibility belongs to the parser's
    /// source-bound removal plan, not to this generic model operation.
    pub fn validate_for_timing_regeneration(
        self,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> Result<TimingRegenerationAdmission, ValidationFailure> {
        let policy = ValidationPolicy::new(
            RuleSelection::new(),
            AlignmentValidation::IncludeTierAlignment,
        );
        let (document, diagnostics, pending) =
            self.validate_owned(policy, errors, name, AdmissionPhase::RegeneratingTiming)?;
        Ok(match pending {
            Some(header_span) => TimingRegenerationAdmission::Pending(PendingTimingChatFile {
                document,
                diagnostics,
                obligation: MediaTimingObligation { header_span },
            }),
            None => TimingRegenerationAdmission::Ready(ValidChatFile {
                document,
                diagnostics,
                policy,
                name: name.to_owned_name(),
            }),
        })
    }

    fn validate_owned(
        mut self,
        policy: ValidationPolicy,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
        phase: AdmissionPhase,
    ) -> Result<(ChatFile, Vec<ParseError>, Option<crate::Span>), ValidationFailure> {
        let collected = ErrorCollector::new();
        let sink = RecordingSink {
            collected: &collected,
            target: errors,
        };
        let incomplete_parse = self
            .utterances()
            .any(|u| !u.parse_health().permits_validation());
        let pending = match phase {
            AdmissionPhase::Complete => {
                self.validate_at(policy, &sink, name);
                None
            }
            AdmissionPhase::RegeneratingTiming => self.validate_regenerating_timing(&sink, name),
        };
        let has_errors = collected.has_errors();
        let (diagnostics, reason) = match CompletedDiagnostics::admit(collected.into_vec()) {
            Err(failure) => (
                failure.into_diagnostics(),
                Some(ValidationFailureReason::InternalFailure),
            ),
            Ok(completed) => {
                let reason = if incomplete_parse {
                    Some(ValidationFailureReason::IncompleteParse)
                } else if has_errors {
                    Some(ValidationFailureReason::Invalidity)
                } else {
                    None
                };
                (completed.into_diagnostics(), reason)
            }
        };
        let name = name.to_owned_name();
        if let Some(reason) = reason {
            Err(ValidationFailure {
                document: Box::new(self),
                diagnostics,
                policy,
                name,
                reason,
            })
        } else {
            Ok((self, diagnostics, pending))
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
