//! Consuming application phases keep the diagnosis, rather than substitute inputs.

use super::{FixDiagnosis, SelectionSkipped, SpliceEdit};
use crate::splice::{Admission, ByteVerifiedSplice, GateError, Skipped, admit_edits};

/// Selected candidates owned by their complete actual diagnosis.
///
/// ```compile_fail
/// use talkbank_transform::splice::SelectedRepairs;
/// let forged = SelectedRepairs { candidates: Vec::new() };
/// ```
///
/// A finding from another diagnosis cannot be appended, even with equal lifetimes.
/// ```compile_fail
/// use talkbank_transform::splice::{SelectedRepairs, FixFinding};
/// fn mix<'a>(mut selected: SelectedRepairs<'a>, other: FixFinding<'_, 'a>) {
///     selected.append(other);
/// }
/// ```
#[derive(Debug)]
pub struct SelectedRepairs<'source> {
    pub(super) diagnosis: FixDiagnosis<'source>,
    pub(super) candidates: Vec<SpliceEdit>,
    pub(super) skipped: Vec<SelectionSkipped>,
}

impl<'source> SelectedRepairs<'source> {
    /// Actual selection refusals, ready for reporting.
    pub fn skipped(&self) -> &[SelectionSkipped] {
        &self.skipped
    }
    /// Whether this selection contains no candidates.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
    /// Consume into health admission against this owner's model only.
    /// ```compile_fail
    /// use talkbank_transform::splice::SelectedRepairs;
    /// fn substitute(selected: SelectedRepairs<'_>, other: &talkbank_model::ChatFile) {
    ///     selected.admit(other);
    /// }
    /// ```
    pub fn admit(self) -> AdmittedRepairs<'source> {
        let admission = admit_edits(self.diagnosis.document(), self.candidates);
        AdmittedRepairs {
            diagnosis: self.diagnosis,
            admission,
        }
    }
}

/// Health admission cannot be re-associated with another source/model.
#[derive(Debug)]
pub struct AdmittedRepairs<'source> {
    diagnosis: FixDiagnosis<'source>,
    admission: Admission,
}
impl<'source> AdmittedRepairs<'source> {
    /// Immutable admitted candidates.
    pub fn admitted(&self) -> &[SpliceEdit] {
        self.admission.admitted()
    }
    /// Health/utterance-boundary refusal reasons.
    pub fn skipped(&self) -> &[Skipped] {
        self.admission.skipped()
    }
    /// Map once against the diagnosis's input and retain verified bytes/context.
    /// No external source can substitute at this transition.
    /// ```compile_fail
    /// use talkbank_transform::splice::AdmittedRepairs;
    /// fn substitute(admitted: AdmittedRepairs<'_>, other: &str) {
    ///     admitted.apply_verified(other);
    /// }
    /// ```
    pub fn apply_verified(self) -> Result<VerifiedRepairs<'source>, GateError> {
        let verified = self
            .admission
            .into_admitted()
            .map(self.diagnosis.source())?
            .apply_verified()?;
        Ok(VerifiedRepairs {
            diagnosis: self.diagnosis,
            verified,
        })
    }
}

/// Byte-verified candidates carry their actual baseline diagnosis into post-check.
/// This is still not semantic correctness or application permission.
///
/// ```compile_fail
/// use talkbank_transform::splice::VerifiedRepairs;
/// let forged = VerifiedRepairs { verified: todo!(), diagnosis: todo!() };
/// ```
///
/// Raw byte verification does not issue diagnosis-associated repair authority.
/// ```compile_fail
/// use talkbank_transform::splice::{ByteVerifiedSplice, VerifiedRepairs};
/// fn promote(raw: ByteVerifiedSplice<'_>) -> VerifiedRepairs<'_> { raw.into() }
/// ```
#[derive(Debug)]
pub struct VerifiedRepairs<'source> {
    diagnosis: FixDiagnosis<'source>,
    verified: ByteVerifiedSplice<'source>,
}
impl<'source> VerifiedRepairs<'source> {
    /// Actual immutable baseline, including name, policy and all findings.
    pub fn diagnosis(&self) -> &FixDiagnosis<'source> {
        &self.diagnosis
    }
    /// Read-only bytes and their actual mapping; no consuming raw constructor.
    pub fn bytes(&self) -> &ByteVerifiedSplice<'source> {
        &self.verified
    }
}
