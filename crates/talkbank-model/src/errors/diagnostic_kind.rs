//! Intrinsic diagnostic categories and optional presentation profiles.
//!
//! `spec/codes/error-codes.toml` owns the per-code classification; regenerate
//! the exhaustive lookup with `just spec-gen`. Presentation severity is not a
//! validity verdict: an internal failure blocks successful admission but says
//! nothing about whether the source is valid CHAT. Consumers must inspect the
//! category before suppressing or downgrading diagnostics for display.
//!
//! Profiles define presentation policy. Existing diagnostic producers also
//! supply explicit severities; this module does not silently override them.

use super::codes::ErrorCode;
use super::source_location::Severity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a diagnostic intrinsically IS: a property of the *rule* that emits
/// it, never a property of the file being validated and never a property
/// of the consumer asking about it.
///
/// This is Axis 1 of the two-axis model documented in the module docs.
/// Every [`ErrorCode`] has exactly one `DiagnosticKind`, looked up via
/// [`kind_of`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// The tool failed. Blocks admission under every profile without asserting
    /// that the input is invalid CHAT.
    InternalFailure,
    /// Violates the spec, or the construct does not make sense. The
    /// ONLY kind that asserts the input is not valid CHAT.
    Invalidity,
    /// Chatter preserves the construct but does not yet interpret it: a
    /// chatter coverage gap (e.g. an unsupported `@Media` value that is
    /// kept verbatim for roundtrip), never a fault in the file itself.
    Unmodeled,
    /// Valid now, discouraged, on a sunset path toward becoming an
    /// [`Invalidity`](Self::Invalidity) at a future date.
    Deprecation,
    /// Valid, purely stylistic (e.g. inconsistent whitespace CLAN CHECK
    /// would flag but that does not change meaning).
    Style,
}

/// Who is asking: a property of the *consumer* validating a file, never of
/// the file itself and never of the diagnostic rule.
///
/// This is Axis 2 of the two-axis model documented in the module docs.
/// [`severity`] derives a [`Severity`] from a [`DiagnosticKind`] plus one
/// of these. These are library policy projections, not a promise of selectable
/// CLI or editor modes. Existing consumers can retain producer-supplied
/// severities independently; asking for a projection does not mutate a finding
/// or issue a validity certificate. Canonical diagnostic tests exercise the
/// projections on findings produced from specification examples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationProfile {
    /// Publication / roundtrip gate: invalidity blocks. `chatter validate`
    /// runs under this profile today (implicitly; no profile selection
    /// exists yet).
    Strict,
    /// Editor / LSP: invalidity surfaces and the parser recovers from it,
    /// non-blocking, so the user keeps a live document while fixing it.
    Editor,
    /// Batch transform / pipeline (e.g. batchalign3): only hard invalidity
    /// blocks the run.
    Pipeline,
    /// Opt-in style pass: style findings surface only when a consumer asks
    /// for this profile.
    Lint,
}

/// Look up a code's intrinsic [`DiagnosticKind`].
///
/// Delegates to the GENERATED `generated_diagnostic_kind::kind_of_from_spec`,
/// an exhaustive match over every [`ErrorCode`] variant with no `_ =>`
/// wildcard arm: adding a new `ErrorCode` variant without regenerating (or
/// hand-extending) that match is a COMPILE ERROR. That property is
/// deliberate and is the entire point of this registry (see the module
/// docs): the old system let severity be decided ad hoc at roughly three
/// dozen emit sites with nothing forcing the decision to be recorded
/// anywhere; a wildcard arm here would silently rebuild the exact same
/// hole with nicer types.
///
/// See the module docs' "Landing state" section for the current
/// classification counts and how to change one (edit the code's spec file,
/// then regenerate; never hand-edit the generated match).
pub fn kind_of(code: ErrorCode) -> DiagnosticKind {
    super::generated_diagnostic_kind::kind_of_from_spec(code)
}

/// Derive a [`Severity`] from a diagnostic's [`DiagnosticKind`] and the
/// active [`ValidationProfile`]. `None` means the finding is not surfaced
/// as an error or a warning under that profile (it may still be reported
/// on a separate advisory stream elsewhere; that is out of scope for this
/// function, which only ever answers the error/warning/silent question).
///
/// This is an exhaustive match over every `(DiagnosticKind, ValidationProfile)`
/// pair, so a newly added variant on either enum is a compile error here
/// too. Internal failures are visible under every profile; they never become
/// validity claims through presentation policy.
pub fn severity(kind: DiagnosticKind, profile: ValidationProfile) -> Option<Severity> {
    match (kind, profile) {
        (DiagnosticKind::InternalFailure, _) => Some(Severity::Error),
        // Strict validation blocks on invalidity.
        (DiagnosticKind::Invalidity, ValidationProfile::Strict) => Some(Severity::Error),
        // Editor/LSP recovers from invalidity rather than rejecting the
        // document outright, so the same finding renders as a warning.
        (DiagnosticKind::Invalidity, ValidationProfile::Editor) => Some(Severity::Warning),
        // Pipeline profiles also block on invalidity.
        (DiagnosticKind::Invalidity, ValidationProfile::Pipeline) => Some(Severity::Error),
        // Lint is an opt-in STYLE pass layered on top of whichever base
        // profile already reports invalidity; it does not additionally
        // re-report invalidity itself.
        (DiagnosticKind::Invalidity, ValidationProfile::Lint) => None,
        // Unmodeled is a chatter coverage gap, never a file fault: it does
        // not render as Severity::{Error,Warning} under any profile.
        (DiagnosticKind::Unmodeled, ValidationProfile::Strict)
        | (DiagnosticKind::Unmodeled, ValidationProfile::Editor)
        | (DiagnosticKind::Unmodeled, ValidationProfile::Pipeline)
        | (DiagnosticKind::Unmodeled, ValidationProfile::Lint) => None,
        // Deprecation: valid now, discouraged; a warning under every
        // profile until a future sunset mechanically flips the code's
        // kind to Invalidity.
        (DiagnosticKind::Deprecation, ValidationProfile::Strict)
        | (DiagnosticKind::Deprecation, ValidationProfile::Editor)
        | (DiagnosticKind::Deprecation, ValidationProfile::Pipeline) => Some(Severity::Warning),
        // Lint does not additionally re-report deprecation; a Deprecation
        // finding already surfaces under whichever base profile is active.
        (DiagnosticKind::Deprecation, ValidationProfile::Lint) => None,
        // Style findings surface only when a consumer explicitly opts into
        // the Lint profile.
        (DiagnosticKind::Style, ValidationProfile::Lint) => Some(Severity::Warning),
        (DiagnosticKind::Style, ValidationProfile::Strict)
        | (DiagnosticKind::Style, ValidationProfile::Editor)
        | (DiagnosticKind::Style, ValidationProfile::Pipeline) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presentation policy must never suppress or downgrade tool failure.
    #[test]
    fn internal_failure_is_an_error_under_every_profile() {
        for profile in [
            ValidationProfile::Strict,
            ValidationProfile::Editor,
            ValidationProfile::Pipeline,
            ValidationProfile::Lint,
        ] {
            assert_eq!(
                severity(DiagnosticKind::InternalFailure, profile),
                Some(Severity::Error)
            );
        }
    }

    /// Pins the exact spec-derived classification: every code is
    /// `Invalidity` EXCEPT the codes named here. A change to this test is a
    /// deliberate reclassification (edit the code registry's `kind`,
    /// regenerate, then update this list to match) and must never
    /// be a silent drive-by edit made only to turn the test green.
    #[test]
    fn kind_of_matches_the_spec_derived_classification() {
        let non_invalidity: &[(ErrorCode, DiagnosticKind)] = &[
            (ErrorCode::InternalError, DiagnosticKind::InternalFailure),
            (ErrorCode::InvalidTimTierFormat, DiagnosticKind::Unmodeled), // E603
            (
                ErrorCode::CodeGluedToFollowingContent,
                DiagnosticKind::Style,
            ), // E757
            (
                ErrorCode::PrefixedFormGluedToPrecedingWord,
                DiagnosticKind::Style,
            ), // E764
            (
                ErrorCode::MediaFilenameNonCanonicalUnicode,
                DiagnosticKind::Style,
            ), // W109: canonical-equivalent names are a style notice, not E531.
            (ErrorCode::MediaFilenameCaseDiffers, DiagnosticKind::Style), // W110: case-only difference.
        ];

        for code in ErrorCode::iter() {
            let expected = non_invalidity
                .iter()
                .find(|(c, _)| c == code)
                .map(|(_, kind)| *kind)
                .unwrap_or(DiagnosticKind::Invalidity);
            assert_eq!(
                kind_of(*code),
                expected,
                "code {code} does not match the expected spec-derived kind; \
                 if this is a deliberate reclassification, update this \
                 test's `non_invalidity` list to match"
            );
        }
    }

    /// Pins the derivation this module would produce if it were wired into
    /// `chatter validate` under `Strict`: every `Invalidity` code derives
    /// `Severity::Error`, matching the current real behaviour for the vast
    /// majority of codes. Nothing consumes this derivation yet (see the
    /// module docs' "Landing state"), so this test is a statement about
    /// what [`severity`] returns, not a claim that every current emit site
    /// already agrees; a handful of emit sites construct
    /// `Severity::Warning` directly today (cited in the module docs), and
    /// the three non-`Invalidity` codes are asserted separately below.
    #[test]
    fn strict_profile_reproduces_current_severity_for_invalidity_codes() {
        for code in ErrorCode::iter() {
            let kind = kind_of(*code);
            if kind == DiagnosticKind::Invalidity {
                assert_eq!(
                    severity(kind, ValidationProfile::Strict),
                    Some(Severity::Error),
                    "code {code}"
                );
            }
        }
    }

    /// The three non-`Invalidity` codes derive `None` (silent) under
    /// `Strict`, per [`severity`]'s current derivation. This is the LATENT
    /// TENSION documented in the module docs: `E757`/`E764` are real
    /// `Layer: validation, Status: implemented` hard errors today, so this
    /// pins what the derivation says, not a claim that wiring it up would
    /// be behaviour-preserving.
    #[test]
    fn non_invalidity_codes_derive_none_under_strict() {
        assert_eq!(
            severity(
                kind_of(ErrorCode::InvalidTimTierFormat),
                ValidationProfile::Strict
            ),
            None
        );
        assert_eq!(
            severity(
                kind_of(ErrorCode::CodeGluedToFollowingContent),
                ValidationProfile::Strict
            ),
            None
        );
        assert_eq!(
            severity(
                kind_of(ErrorCode::PrefixedFormGluedToPrecedingWord),
                ValidationProfile::Strict
            ),
            None
        );
    }

    /// `Unmodeled` is documented as never surfacing as an error or warning
    /// under any profile; pin that across the whole profile set so a
    /// future profile addition cannot silently start blocking on it.
    #[test]
    fn unmodeled_never_surfaces_as_severity() {
        for profile in [
            ValidationProfile::Strict,
            ValidationProfile::Editor,
            ValidationProfile::Pipeline,
            ValidationProfile::Lint,
        ] {
            assert_eq!(severity(DiagnosticKind::Unmodeled, profile), None);
        }
    }

    /// `Invalidity` under `Editor` recovers rather than blocks: the design
    /// doc's sharpest example of why severity must be derived per-profile
    /// rather than stored once on the diagnostic.
    #[test]
    fn invalidity_downgrades_to_warning_under_editor() {
        assert_eq!(
            severity(DiagnosticKind::Invalidity, ValidationProfile::Editor),
            Some(Severity::Warning)
        );
    }
}
