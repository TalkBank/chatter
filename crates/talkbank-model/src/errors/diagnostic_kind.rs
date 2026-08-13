//! Diagnostic KIND and validation PROFILE: the two orthogonal axes that
//! replace a single, overloaded [`Severity`](super::Severity) bucket.
//!
//! # Background
//!
//! Before this module, `Severity` had exactly two variants (`Error`,
//! `Warning`) and the `Warning` bucket silently mashed together five
//! unrelated concerns (policy-downgraded invalidity, model incompleteness,
//! parse-mode recovery, deprecation, and style), decided ad hoc at roughly
//! three dozen emit sites with no central registry recording the decision.
//! Full history and rationale:
//! `docs/design/2026-07-13-diagnostic-kind-and-profile-refactor.md`
//! (private meta-repo).
//!
//! This module separates the two axes a diagnostic actually varies along:
//!
//! - [`DiagnosticKind`] (Axis 1): what the diagnostic intrinsically IS, a
//!   property of the *rule*. Looked up per [`ErrorCode`] via [`kind_of`],
//!   an EXHAUSTIVE match with no wildcard arm, so a new `ErrorCode` variant
//!   that nobody assigned a kind is a compile error, not a silent gap.
//! - [`ValidationProfile`] (Axis 2): who is asking, a property of the
//!   *consumer* (never of the file, never of the rule).
//!
//! [`Severity`](super::Severity) is DERIVED from the two, via [`severity`],
//! and is never stored directly on a diagnostic. `None` means the finding
//! is not surfaced as an error or a warning under that profile at all (for
//! example an [`Unmodeled`](DiagnosticKind::Unmodeled) finding, which is a
//! chatter coverage gap, never a file fault, and so never renders as
//! [`Severity::Error`](super::Severity::Error) or
//! [`Severity::Warning`](super::Severity::Warning) under any profile).
//!
//! # Landing state (2026-07-31)
//!
//! [`kind_of`] is now GENERATED from `spec/errors/*.md`'s required
//! `- **Kind**:` metadata field (`gen_diagnostic_kind`, in
//! `spec/runtime-tools`), not hand-written: the per-code adjudication this
//! module used to defer to a hand-curated proposal table now lives as
//! ordinary spec content, read directly off the same file that already
//! documents the code's `## CHAT Rule` / `## Notes`, so the two cannot
//! independently drift. The generated match is in
//! `generated_diagnostic_kind.rs`; do not hand-edit it, and see that
//! file's header for the regeneration command.
//!
//! `ErrorCode` and `spec/errors/` are two independently hand-maintained
//! sets, and the generator FAILS CLOSED on any divergence between them
//! (a variant with no spec file, or a spec-named code with no matching
//! variant) instead of defaulting a gap to `Invalidity`: see that
//! generator's module docs. The two sets are exhaustively reconciled as of
//! this landing: of the 221 `ErrorCode` variants (226 minus 5 retired the
//! same day: `E366`, `E369`, `E700`, `E703`, `W999`/`LegacyWarning`, none
//! of which had any emit site anywhere in the workspace), 218 are
//! `Invalidity`, 2 (`CodeGluedToFollowingContent` / E757,
//! `PrefixedFormGluedToPrecedingWord` / E764) are `Style`, and 1
//! (`InvalidTimTierFormat` / E603) is `Unmodeled`. Every variant has
//! exactly one spec file naming it.
//!
//! No [`ValidationProfile`] is wired into any consumer: this module is
//! still purely additive and changes nothing about `chatter validate`'s
//! behaviour, because nothing calls [`kind_of`] or [`severity`] outside
//! this module's own tests.
//!
//! A caveat on the "warnings don't matter" argument for why the exact
//! current classification is safe to land inert: [`Severity::Warning`](super::Severity::Warning)
//! is NOT structurally unreachable through `chatter validate`. A handful of
//! production call sites construct it directly today (e.g.
//! `ErrorCode::InvalidTimTierFormat` in
//! `model/file/utterance/validate.rs`, `ErrorCode::SpeakerNotFoundInParticipants`
//! in `model/content/main_tier/mod.rs`, and `ErrorCode::TierValidationError`'s
//! two alignment-diagnostic helpers). What IS true, checked against each
//! of those sites: none of them has been observed to independently flip a
//! file's overall pass/fail verdict, either because the same call site
//! that reports the warning also reports a `Severity::Error` for the same
//! condition at the same span, or because the triggering condition
//! (`ParseHealthState::Unknown`, the default for content never touched by
//! a parser) does not occur on content that reached `chatter validate`
//! through its real parse path. See the adjudication table's "Notable
//! Findings" section for the full citations; do not repeat the flatter
//! "unreachable" claim elsewhere without that caveat.
//!
//! `Style`'s derived [`severity`] is silent under `Strict`/`Editor`/
//! `Pipeline` (surfacing only under an opt-in `Lint` profile), which is in
//! LATENT TENSION with `E757`/`E764` each being `Layer: validation, Status:
//! implemented` today and with the project rule that CHECK-parity style
//! rules follow CHECK as hard errors. It is inert tension only: no
//! consumer reads `severity()` yet, so `chatter validate` is unaffected.
//! Resolving it (either by giving `Style` a different derivation under
//! `Strict`, or by accepting that a future wiring-up would change these two
//! codes' behaviour) is the maintainer's call, not this generator's.
//!
//! Reclassifying a code's `Kind`, or wiring a non-`Strict` profile into a
//! consumer, is the maintainer's call: change the code's spec file, then
//! regenerate.

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
    /// Violates the spec, or the construct does not make sense. The
    /// CHECK-equivalent axis, and the ONLY kind that bears on "is this
    /// valid CHAT". Most codes are classified this way; see the module
    /// docs' "Landing state" section for the current counts and the small
    /// set of exceptions.
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
/// of these. As of the current landing only [`Strict`](Self::Strict) is
/// wired into any consumer; the others are part of the type shape this
/// module establishes, not yet exercised behaviour.
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
/// Delegates to the GENERATED [`generated_diagnostic_kind::kind_of_from_spec`],
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
/// Severity is deliberately never stored on a diagnostic alongside its
/// kind; storing a mirrored, independently-mutable copy is exactly the
/// ad-hoc-decision problem this module exists to remove. Compute it here,
/// at the point where a profile is actually known.
///
/// This is an exhaustive match over every `(DiagnosticKind, ValidationProfile)`
/// pair, so a newly added variant on either enum is a compile error here
/// too. Only the `Invalidity` row under `Strict` is exercised by any
/// consumer as of the current landing (see the module docs); the remaining
/// entries follow directly from the design doc's stated intent for each
/// profile but are UNVALIDATED by any running consumer and are provisional
/// pending the maintainer's adjudication of the open profile-semantics
/// questions in that doc.
pub fn severity(kind: DiagnosticKind, profile: ValidationProfile) -> Option<Severity> {
    match (kind, profile) {
        // Invalidity is the CHECK-equivalent axis: Strict is the
        // publication/roundtrip gate, so it blocks. This is the only
        // (kind, profile) pair any consumer exercises today.
        (DiagnosticKind::Invalidity, ValidationProfile::Strict) => Some(Severity::Error),
        // Editor/LSP recovers from invalidity rather than rejecting the
        // document outright, so the same finding renders as a warning.
        (DiagnosticKind::Invalidity, ValidationProfile::Editor) => Some(Severity::Warning),
        // Pipeline profiles block on "hard" invalidity; every code is
        // currently classified Invalidity (nothing has been adjudicated
        // into a softer kind yet), so Pipeline agrees with Strict for now.
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
        // kind to Invalidity (see the design doc's open question 3).
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

    /// Pins the exact spec-derived classification: every code is
    /// `Invalidity` EXCEPT the three named here. A change to this test is a
    /// deliberate reclassification (edit the code's spec file's `Kind`
    /// bullet, regenerate, then update this list to match) and must never
    /// be a silent drive-by edit made only to turn the test green.
    #[test]
    fn kind_of_matches_the_spec_derived_classification() {
        let non_invalidity: &[(ErrorCode, DiagnosticKind)] = &[
            (ErrorCode::InvalidTimTierFormat, DiagnosticKind::Unmodeled), // E603
            (
                ErrorCode::CodeGluedToFollowingContent,
                DiagnosticKind::Style,
            ), // E757
            (
                ErrorCode::PrefixedFormGluedToPrecedingWord,
                DiagnosticKind::Style,
            ), // E764
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
