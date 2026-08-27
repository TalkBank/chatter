//! The per-code facts that are NOT generated: the enforcement axis, and the
//! Phon `%x` group.
//!
//! The enum itself moved to [`super::generated_error_code`] under R1 of the
//! spec-system redesign: its variants, their rustdoc and their
//! `#[status(planned)]` attributes are emitted from
//! `spec/codes/error-codes.toml`, so nothing here restates a per-code fact.
//! What stays is a type ABOUT codes ([`CheckStatus`]), a reading of the
//! generated attributes ([`ErrorCode::check_status`]), and one editorial
//! GROUPING of codes that no per-code field decides.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use super::generated_error_code::ErrorCode;

/// Whether a check is enforced by this binary, or only documented.
///
/// A closed two-state fact about an [`ErrorCode`], living beside the code
/// itself. It used to live in the CLI as a hand-maintained list of 43 code
/// STRINGS whose own doc said it "must be kept in sync" with
/// `spec/errors/*.md`. It was not in sync: 15 of 225 codes were reported
/// wrongly, in both directions, by the command whose only job is telling users
/// which checks run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Enforced: the check fires when its condition is detected.
    Active,
    /// Documented in `spec/errors/` but not yet enforced.
    Planned,
}

/// The Phon `%x` dependent-tier validation codes, as one group.
///
/// Single source of truth for "which error codes are Phon `%x` validation": the
/// word-count cross-checks (E725-E728) plus the content checks (E735-E746). It
/// lives next to the code definitions so the two cannot drift; the CLI exposes
/// it to users under the `xphon` suppress-group name. When you add a Phon `%x`
/// check, add its code here.
pub const XPHON_ERROR_CODES: &[ErrorCode] = &[
    ErrorCode::ModsylModCountMismatch,             // E725
    ErrorCode::PhosylPhoCountMismatch,             // E726
    ErrorCode::PhoalnModCountMismatch,             // E727
    ErrorCode::PhoalnPhoCountMismatch,             // E728
    ErrorCode::SylUnitMalformed,                   // E735
    ErrorCode::SylIllegalConstituentCode,          // E736
    ErrorCode::ModsylReconstructionMismatch,       // E737
    ErrorCode::PhosylReconstructionMismatch,       // E738
    ErrorCode::PhoalnPairMalformed,                // E739
    ErrorCode::PhoalnModReconstructionMismatch,    // E740
    ErrorCode::PhoalnPhoReconstructionMismatch,    // E741
    ErrorCode::XphointBulletInvalid,               // E742
    ErrorCode::XphointIntervalNotMonotonic,        // E743
    ErrorCode::XphointMediaBoundsViolation,        // E744
    ErrorCode::XphointPhoneReconstructionMismatch, // E745
    ErrorCode::XphointGroupCountMismatch,          // E746
];

impl ErrorCode {
    /// Whether this check is enforced or merely documented.
    ///
    /// Derived from the `#[status(planned)]` attributes on the variants,
    /// which are themselves emitted from each code's `status` in
    /// `spec/codes/error-codes.toml`. There is one owner and nothing to keep
    /// in sync: `SpecStatusGate` existed to reconcile the attribute against
    /// the specs in BOTH directions, and R1 deleted it by removing the second
    /// copy rather than by checking it harder.
    pub fn check_status(&self) -> CheckStatus {
        match Self::planned().iter().find(|planned| *planned == self) {
            Some(_) => CheckStatus::Planned,
            None => CheckStatus::Active,
        }
    }
}
