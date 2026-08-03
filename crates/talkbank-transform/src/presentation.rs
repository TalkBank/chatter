//! How computed diagnostics are SHOWN. Never what the validator computes.
//!
//! [`PresentationPolicy`] carries the suppression and severity-remapping
//! choices a user expresses with `--suppress` and friends. It is applied to
//! diagnostics that already exist, at the boundary where a run hands its
//! results to a renderer, so it can change what a reader sees and never what
//! the validator did.
//!
//! # Why this lives in `talkbank-transform` and not in `talkbank-model`
//!
//! It is deliberately UNREACHABLE from `talkbank-cache`. The validation cache
//! key is derived from [`talkbank_model::RuleSelection`] alone; v0.6.0 derived
//! it from a struct that also held these presentation settings, which gave
//! every distinct `--suppress` list its own private cache and re-validated a
//! 106,000-file corpus from cold on the second run. A doc comment saying
//! "don't fold presentation into the key" is the kind of rule that loses to an
//! affordance. `talkbank-transform` depends on `talkbank-cache`, so the cache
//! crate cannot name this type without a dependency cycle: a future attempt to
//! fold a presentation setting into the cache key is a compile error about an
//! unreachable type rather than a silent 200x slowdown.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::collections::HashMap;

use talkbank_model::{ErrorCode, ErrorSink, ParseError, Severity};

/// What a reader is shown, and at what severity.
///
/// # The defining property
///
/// **Nothing here can change which diagnostics the validator computes.** Every
/// setting is applied to an already-computed diagnostic: dropped, or
/// re-labelled with a different severity. A setting that would change what gets
/// computed is rule SELECTION and belongs in
/// [`talkbank_model::RuleSelection`], which is the type the cache key is
/// derived from.
///
/// # Precedence
///
/// 1. An explicit per-code override from
///    [`Self::set_severity`] / [`Self::upgrade`] / [`Self::downgrade`] /
///    [`Self::disable`].
/// 2. Global escalation of unmapped warnings ([`Self::strict`]).
/// 3. The severity the validator assigned.
///
/// # Example
///
/// ```
/// use talkbank_transform::PresentationPolicy;
/// use talkbank_model::{ErrorCode, Severity};
///
/// let policy = PresentationPolicy::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
///     .disable(ErrorCode::InvalidOverlapIndex)
///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
/// ```
#[derive(Clone, Debug, Default)]
pub struct PresentationPolicy {
    /// Per-code severity override; `None` means the diagnostic is not shown.
    severity_overrides: HashMap<ErrorCode, Option<Severity>>,
    /// Show warnings without an explicit per-code override as errors.
    upgrade_unmapped_warnings: bool,
}

impl PresentationPolicy {
    /// Show every computed diagnostic exactly as the validator labelled it.
    pub fn new() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: false,
        }
    }

    /// Show `code` at a lower severity than the validator assigned.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let policy = PresentationPolicy::new()
    ///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);
    /// ```
    pub fn downgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Show `code` at a higher severity than the validator assigned.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let policy = PresentationPolicy::new()
    ///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
    /// ```
    pub fn upgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Stop showing `code` entirely.
    ///
    /// The diagnostic is still COMPUTED; it is dropped on the way to the
    /// reader. That distinction is what lets one cache serve suppressed and
    /// unsuppressed runs alike.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    /// use talkbank_model::ErrorCode;
    ///
    /// let policy = PresentationPolicy::new().disable(ErrorCode::InvalidOverlapIndex);
    /// ```
    pub fn disable(mut self, code: ErrorCode) -> Self {
        self.severity_overrides.insert(code, None);
        self
    }

    /// Set an explicit severity for `code`, or `None` to stop showing it.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let policy = PresentationPolicy::new()
    ///     .set_severity(ErrorCode::IllegalUntranscribed, Some(Severity::Warning))
    ///     .set_severity(ErrorCode::InvalidOverlapIndex, None);
    /// ```
    pub fn set_severity(mut self, code: ErrorCode, severity: Option<Severity>) -> Self {
        self.severity_overrides.insert(code, severity);
        self
    }

    /// Show every unmapped warning as an error.
    ///
    /// Explicit per-code overrides still win, so a caller can opt individual
    /// codes back out by setting them to [`Severity::Warning`].
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    ///
    /// let policy = PresentationPolicy::strict();
    /// ```
    pub fn strict() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: true,
        }
    }

    /// A gentler view for legacy corpora: two common hard errors shown as
    /// warnings.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_transform::PresentationPolicy;
    ///
    /// let policy = PresentationPolicy::lenient();
    /// ```
    pub fn lenient() -> Self {
        Self::new()
            .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
            .downgrade(ErrorCode::InvalidOverlapIndex, Severity::Warning)
    }

    /// The severity to display `code` at, or `None` when it is not shown.
    pub fn effective_severity(&self, code: ErrorCode, original: Severity) -> Option<Severity> {
        match self.severity_overrides.get(&code) {
            Some(override_severity) => *override_severity,
            None if self.upgrade_unmapped_warnings && original == Severity::Warning => {
                Some(Severity::Error)
            }
            None => Some(original),
        }
    }

    /// Whether `code` is hidden from the reader.
    pub fn is_disabled(&self, code: ErrorCode) -> bool {
        matches!(self.severity_overrides.get(&code), Some(None))
    }

    /// Every per-code override, for callers that report the active policy.
    pub fn overrides(&self) -> &HashMap<ErrorCode, Option<Severity>> {
        &self.severity_overrides
    }

    /// Whether this policy shows every computed diagnostic unchanged.
    ///
    /// Lets a caller skip the mapping pass entirely in the common case rather
    /// than each call site re-deriving "is anything set here".
    pub fn shows_everything(&self) -> bool {
        self.severity_overrides.is_empty() && !self.upgrade_unmapped_warnings
    }

    /// Apply this policy to one computed diagnostic, yielding what the reader
    /// should see, or `None` when the diagnostic is hidden.
    pub fn apply(&self, mut diagnostic: ParseError) -> Option<ParseError> {
        match self.effective_severity(diagnostic.code, diagnostic.severity) {
            Some(severity) => {
                diagnostic.severity = severity;
                Some(diagnostic)
            }
            None => None,
        }
    }

    /// Apply this policy to a whole computed diagnostic set.
    ///
    /// Consumes its input rather than borrowing it: the caller has already
    /// derived everything only the COMPLETE set can answer (the cached fact,
    /// the run's tallies), and what remains is the reader's view. Keeping both
    /// alive would invite a later reader to ask the presented set a question it
    /// cannot answer.
    pub fn apply_all(&self, diagnostics: Vec<ParseError>) -> Vec<ParseError> {
        if self.shows_everything() {
            return diagnostics;
        }
        diagnostics
            .into_iter()
            .filter_map(|diagnostic| self.apply(diagnostic))
            .collect()
    }
}

/// Error sink that applies a [`PresentationPolicy`] on the way through.
///
/// For surfaces that stream diagnostics to a reader as they arrive and so
/// cannot post-process a collected `Vec`. It must never wrap a sink whose
/// output feeds a cache write or a run tally: those consume the COMPLETE set,
/// and confusing the two is exactly what put a display preference into the
/// cache key.
///
/// # Example
///
/// ```
/// use talkbank_transform::{ConfigurableErrorSink, PresentationPolicy};
/// use talkbank_model::{ErrorCode, ErrorCollector, Severity};
///
/// let policy = PresentationPolicy::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
///     .disable(ErrorCode::InvalidOverlapIndex);
///
/// let inner = ErrorCollector::new();
/// let sink = ConfigurableErrorSink::new(&inner, policy);
/// ```
pub struct ConfigurableErrorSink<'a, S: ErrorSink> {
    inner: &'a S,
    policy: PresentationPolicy,
}

impl<'a, S: ErrorSink> ConfigurableErrorSink<'a, S> {
    /// Wrap `inner` so everything reaching it has been through `policy`.
    pub fn new(inner: &'a S, policy: PresentationPolicy) -> Self {
        Self { inner, policy }
    }

    /// The wrapped sink, which receives post-policy diagnostics.
    pub fn inner(&self) -> &S {
        self.inner
    }

    /// The policy being applied.
    pub fn policy(&self) -> &PresentationPolicy {
        &self.policy
    }
}

impl<S: ErrorSink> ErrorSink for ConfigurableErrorSink<'_, S> {
    /// Forward one diagnostic, unless the policy hides it.
    fn report(&self, error: ParseError) {
        if let Some(shown) = self.policy.apply(error) {
            self.inner.report(shown);
        }
    }

    /// Forward a batch, dropping whatever the policy hides.
    fn report_all(&self, errors: Vec<ParseError>) {
        let shown = self.policy.apply_all(errors);
        if !shown.is_empty() {
            self.inner.report_all(shown);
        }
    }

    /// Forward a smallvec-backed batch.
    fn report_vec(&self, errors: talkbank_model::ErrorVec) {
        self.report_all(errors.into_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{ErrorCollector, ErrorContext, SourceLocation};

    /// Builds a diagnostic to push through a policy.
    fn make_test_error(code: ErrorCode, severity: Severity) -> ParseError {
        ParseError::new(
            code,
            severity,
            SourceLocation::at_offset(0),
            ErrorContext::new("test", 0..4, "test"),
            "Test error",
        )
    }

    #[test]
    fn downgraded_code_is_shown_at_the_lower_severity() {
        let policy =
            PresentationPolicy::new().downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);
        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, policy);

        sink.report(make_test_error(
            ErrorCode::IllegalUntranscribed,
            Severity::Error,
        ));

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Warning);
    }

    #[test]
    fn disabled_code_is_not_shown() {
        let policy = PresentationPolicy::new().disable(ErrorCode::InvalidOverlapIndex);
        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, policy);

        sink.report(make_test_error(
            ErrorCode::InvalidOverlapIndex,
            Severity::Error,
        ));

        assert_eq!(inner.into_vec().len(), 0, "a disabled code is not shown");
    }

    #[test]
    fn upgraded_warning_is_shown_as_an_error() {
        let policy =
            PresentationPolicy::new().upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, policy);

        sink.report(make_test_error(
            ErrorCode::UnknownAnnotation,
            Severity::Warning,
        ));

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn an_unmapped_code_keeps_the_validators_severity() {
        let policy = PresentationPolicy::new();
        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, policy);

        sink.report(make_test_error(
            ErrorCode::IllegalUntranscribed,
            Severity::Error,
        ));

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn a_batch_keeps_everything_the_policy_does_not_hide() {
        let policy = PresentationPolicy::new().disable(ErrorCode::InvalidOverlapIndex);
        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, policy);

        sink.report_all(vec![
            make_test_error(ErrorCode::IllegalUntranscribed, Severity::Error),
            make_test_error(ErrorCode::InvalidOverlapIndex, Severity::Error),
            make_test_error(ErrorCode::UnknownAnnotation, Severity::Warning),
        ]);

        let shown = inner.into_vec();
        assert_eq!(shown.len(), 2, "only the disabled code is dropped");
        assert_eq!(shown[0].code, ErrorCode::IllegalUntranscribed);
        assert_eq!(shown[1].code, ErrorCode::UnknownAnnotation);
    }

    #[test]
    fn strict_shows_unmapped_warnings_as_errors() {
        let policy = PresentationPolicy::strict();
        assert_eq!(
            policy.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Error)
        );
    }

    #[test]
    fn an_explicit_override_beats_strict_escalation() {
        let policy = PresentationPolicy::strict()
            .set_severity(ErrorCode::UnknownAnnotation, Some(Severity::Warning));
        assert_eq!(
            policy.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Warning)
        );
    }

    #[test]
    fn lenient_downgrades_the_two_legacy_codes() {
        let policy = PresentationPolicy::lenient();
        assert_eq!(
            policy.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Warning)
        );
    }

    /// The default policy is the identity, and says so, so callers can skip
    /// the mapping pass rather than each re-deriving "is anything set".
    #[test]
    fn the_default_policy_shows_everything() {
        assert!(PresentationPolicy::new().shows_everything());
        assert!(!PresentationPolicy::lenient().shows_everything());
        assert!(!PresentationPolicy::strict().shows_everything());
    }
}
