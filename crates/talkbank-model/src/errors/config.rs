//! Validation policy for remapping or suppressing diagnostics.
//!
//! `ValidationConfig` is applied by [`ConfigurableErrorSink`](crate::ConfigurableErrorSink)
//! before diagnostics are forwarded to downstream consumers.
//!
//! ## Precedence
//!
//! 1. Explicit per-code override from `set_severity`/`upgrade`/`downgrade`/`disable`.
//! 2. Global strict-mode escalation (`strict`) for diagnostics still marked as warnings.
//! 3. Original parser/validator severity.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::{ErrorCode, Severity};
use std::collections::HashMap;

/// Prefix for the sorted per-code severity-override list inside
/// [`ValidationConfig::cache_key_fragment`]'s output.
const OVERRIDES_FRAGMENT_PREFIX: &str = "+overrides.";

/// Label standing in for a fully disabled code inside the overrides list
/// (`Option<Severity>`'s `None` has no `Display`).
const DISABLED_OVERRIDE_LABEL: &str = "disabled";

/// Suffix appended by [`ValidationConfig::cache_key_fragment`] when
/// `enable_quotation_validation` (strict cross-utterance linker validation)
/// is on.
const STRICT_LINKERS_FRAGMENT: &str = "+strict-linkers";

/// Suffix appended by [`ValidationConfig::cache_key_fragment`] when
/// `upgrade_unmapped_warnings` is on.
const UPGRADE_UNMAPPED_WARNINGS_FRAGMENT: &str = "+upgrade-unmapped-warnings";

/// Configuration for validation severity behavior.
///
/// Allows downgrading errors to warnings, disabling specific checks,
/// or upgrading warnings to errors.
///
/// # Example
///
/// ```
/// use talkbank_model::ValidationConfig;
/// use talkbank_model::{ErrorCode, Severity};
///
/// let config = ValidationConfig::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
///     .disable(ErrorCode::InvalidOverlapIndex)
///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ValidationConfig {
    /// Map from error code to overridden severity.
    ///
    /// `None` means the diagnostic is disabled.
    severity_overrides: HashMap<ErrorCode, Option<Severity>>,
    /// If true, warnings without explicit per-code overrides are escalated to errors.
    upgrade_unmapped_warnings: bool,
    /// Enable strict cross-utterance linker validation (E351-E355).
    ///
    /// When true, self-completion (`+,`) and other-completion (`++`) linkers
    /// are checked for correct pairing with preceding terminators (`+/.` and
    /// `+...` respectively). Disabled by default because many existing corpora
    /// do not follow these strict conventions.
    enable_quotation_validation: bool,
}

impl ValidationConfig {
    /// Create a new validation configuration with default behavior.
    pub fn new() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: false,
            enable_quotation_validation: false,
        }
    }

    /// Downgrade an error code to a lower severity
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);
    /// ```
    pub fn downgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Disable a specific error code entirely
    ///
    /// Errors with this code will not be reported.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::ErrorCode;
    ///
    /// let config = ValidationConfig::new()
    ///     .disable(ErrorCode::InvalidOverlapIndex);
    /// ```
    pub fn disable(mut self, code: ErrorCode) -> Self {
        self.severity_overrides.insert(code, None);
        self
    }

    /// Upgrade a warning to an error
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
    /// ```
    pub fn upgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Set a custom severity for an error code.
    ///
    /// Pass `None` to disable the error.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .set_severity(ErrorCode::IllegalUntranscribed, Some(Severity::Warning))
    ///     .set_severity(ErrorCode::InvalidOverlapIndex, None);  // Disable
    /// ```
    pub fn set_severity(mut self, code: ErrorCode, severity: Option<Severity>) -> Self {
        self.severity_overrides.insert(code, severity);
        self
    }

    /// Resolve the severity that should be emitted for a diagnostic.
    ///
    /// Returns `None` when the code is disabled.
    pub fn effective_severity(&self, code: ErrorCode, original: Severity) -> Option<Severity> {
        match self.severity_overrides.get(&code) {
            Some(override_severity) => *override_severity,
            None if self.upgrade_unmapped_warnings && original == Severity::Warning => {
                Some(Severity::Error)
            }
            None => Some(original),
        }
    }

    /// Check if an error code is disabled
    pub fn is_disabled(&self, code: ErrorCode) -> bool {
        matches!(self.severity_overrides.get(&code), Some(None))
    }

    /// Get all severity overrides
    pub fn overrides(&self) -> &HashMap<ErrorCode, Option<Severity>> {
        &self.severity_overrides
    }

    /// Create a strict configuration that escalates unmapped warnings to errors.
    ///
    /// Explicit per-code overrides still take precedence, so callers can opt out
    /// for specific codes by setting them back to `Severity::Warning`.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    ///
    /// let config = ValidationConfig::strict();
    /// // All warnings will be treated as errors
    /// ```
    pub fn strict() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: true,
            enable_quotation_validation: false,
        }
    }

    /// Enable strict cross-utterance linker validation (E351-E355).
    ///
    /// When enabled, self-completion (`+,`) and other-completion (`++`)
    /// linkers are validated against their required preceding terminators.
    /// This is off by default because many real corpora do not follow
    /// strict sequential linker pairing conventions.
    pub fn with_strict_linkers(mut self) -> Self {
        self.enable_quotation_validation = true;
        self
    }

    /// Returns whether strict cross-utterance linker validation is enabled.
    pub fn strict_linkers_enabled(&self) -> bool {
        self.enable_quotation_validation
    }

    /// Create a lenient configuration for legacy corpora.
    ///
    /// Downgrades common strict errors to warnings for gradual migration.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    ///
    /// let config = ValidationConfig::lenient();
    /// // E241 (illegal untranscribed) becomes a warning instead of error
    /// ```
    pub fn lenient() -> Self {
        Self::new()
            .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
            .downgrade(ErrorCode::InvalidOverlapIndex, Severity::Warning)
    }

    /// Render the cache-relevant surface of this config as a deterministic,
    /// canonical text fragment, for folding into a validation cache key
    /// (`talkbank_cache::RulesVersion::current_with_config`).
    ///
    /// # Why this lives here, next to the struct, and not in `talkbank-cache`
    ///
    /// `talkbank-cache` cannot enumerate `ValidationConfig`'s fields itself:
    /// they are private, and even if they were public, a hand-picked field
    /// list in a downstream crate is exactly the shape of bug this method
    /// exists to make unrepresentable. Two real gaps shipped from that
    /// shape: `upgrade_unmapped_warnings` was never folded into the cache
    /// key at all (inert only because `ValidationConfig::strict()`, the
    /// sole constructor that sets it, had no production caller yet), and a
    /// separate CLI cache-key builder folded `strict_linkers` but not its
    /// `--suppress` list. Both were silent: nothing failed to compile, and
    /// nothing failed a test, until a caller finally exercised the gap and
    /// got served a stale verdict from a differently-configured run.
    ///
    /// This method closes the shape, not just the two instances: it
    /// destructures `Self` FIELD BY FIELD with **no `..` rest pattern**, so
    /// adding a field to `ValidationConfig` is a compile error here until
    /// someone decides whether it can change a validation verdict. If it
    /// can, fold it into the returned fragment. If it genuinely cannot,
    /// bind it to `_` with a one-line comment explaining why not; that
    /// comment is the recorded decision, not a silent omission.
    ///
    /// The wrapped fields are the invariant every call site relies on: two
    /// configs with equal `cache_key_fragment()` output must be
    /// interchangeable as far as a cached pass/fail verdict is concerned.
    pub fn cache_key_fragment(&self) -> String {
        let Self {
            severity_overrides,
            upgrade_unmapped_warnings,
            enable_quotation_validation,
        } = self;

        // A disabled code is never emitted and a downgraded/upgraded code
        // changes severity, either of which can flip a file between Valid
        // and Invalid: every override must be folded in. Sorted + deduped
        // so `HashMap` iteration order never affects the fragment.
        let mut overrides: Vec<String> = severity_overrides
            .iter()
            .map(|(code, severity)| {
                let severity_label = severity
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| DISABLED_OVERRIDE_LABEL.to_string());
                format!("{}:{severity_label}", code.as_str())
            })
            .collect();
        overrides.sort_unstable();
        overrides.dedup();

        let mut fragment = String::new();
        if !overrides.is_empty() {
            fragment.push_str(OVERRIDES_FRAGMENT_PREFIX);
            fragment.push_str(&overrides.join(","));
        }
        // Escalates warnings without an explicit override to errors, which
        // can flip a file from Valid to Invalid: must be folded in. This is
        // the gap that shipped inert (`ValidationConfig::strict()` had no
        // production caller) and is closed here.
        if *upgrade_unmapped_warnings {
            fragment.push_str(UPGRADE_UNMAPPED_WARNINGS_FRAGMENT);
        }
        // Turns on E351-E355 (strict cross-utterance linker validation),
        // which can flip a file from Valid to Invalid: must be folded in.
        if *enable_quotation_validation {
            fragment.push_str(STRICT_LINKERS_FRAGMENT);
        }
        fragment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests downgrade error.
    #[test]
    fn test_downgrade_error() {
        let config =
            ValidationConfig::new().downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Warning)
        );
    }

    /// Tests disable error.
    #[test]
    fn test_disable_error() {
        let config = ValidationConfig::new().disable(ErrorCode::InvalidOverlapIndex);

        assert_eq!(
            config.effective_severity(ErrorCode::InvalidOverlapIndex, Severity::Error),
            None
        );
        assert!(config.is_disabled(ErrorCode::InvalidOverlapIndex));
    }

    /// Tests upgrade warning.
    #[test]
    fn test_upgrade_warning() {
        let config = ValidationConfig::new().upgrade(ErrorCode::UnknownAnnotation, Severity::Error);

        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Error)
        );
    }

    /// Tests no override uses original.
    #[test]
    fn test_no_override_uses_original() {
        let config = ValidationConfig::new();

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Error)
        );
    }

    /// Tests lenient config.
    #[test]
    fn test_lenient_config() {
        let config = ValidationConfig::lenient();

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Warning)
        );
    }

    /// Strict mode escalates warnings that do not have explicit overrides.
    #[test]
    fn test_strict_config_upgrades_warnings() {
        let config = ValidationConfig::strict();
        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Error)
        );
    }

    /// `ValidationConfig::strict()` and `ValidationConfig::new()` differ in
    /// exactly one field, `upgrade_unmapped_warnings`, so this is a direct
    /// probe of that one dimension: before `cache_key_fragment` folded it
    /// in, the two configs produced the SAME fragment despite `strict()`
    /// being able to flip a file from Valid to Invalid (any unmapped
    /// warning becomes an error). A downstream `RulesVersion` built from
    /// these two fragments must diverge or a cache row produced under one
    /// could be served to the other.
    #[test]
    fn cache_key_fragment_differs_when_only_upgrade_unmapped_warnings_differs() {
        let default_config = ValidationConfig::new();
        let strict_config = ValidationConfig::strict();
        assert_ne!(
            default_config.cache_key_fragment(),
            strict_config.cache_key_fragment(),
            "upgrade_unmapped_warnings must be folded into the cache key fragment"
        );
    }

    /// Same probe as above, for the other boolean dimension.
    #[test]
    fn cache_key_fragment_differs_when_only_strict_linkers_differs() {
        let default_config = ValidationConfig::new();
        let strict_linkers_config = ValidationConfig::new().with_strict_linkers();
        assert_ne!(
            default_config.cache_key_fragment(),
            strict_linkers_config.cache_key_fragment(),
            "strict_linkers must be folded into the cache key fragment"
        );
    }

    /// Two configs with the same disabled-code SET, built in a different
    /// order, must produce the same fragment: `HashMap` iteration order
    /// must never leak into the cache key.
    #[test]
    fn cache_key_fragment_is_order_independent_over_overrides() {
        let forward = ValidationConfig::new()
            .disable(ErrorCode::InvalidOverlapIndex)
            .disable(ErrorCode::UnparsableContent);
        let backward = ValidationConfig::new()
            .disable(ErrorCode::UnparsableContent)
            .disable(ErrorCode::InvalidOverlapIndex);
        assert_eq!(forward.cache_key_fragment(), backward.cache_key_fragment());
    }

    /// Explicit per-code overrides take precedence over strict-mode escalation.
    #[test]
    fn test_strict_with_explicit_warning_override() {
        let config = ValidationConfig::strict()
            .set_severity(ErrorCode::UnknownAnnotation, Some(Severity::Warning));
        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Warning)
        );
    }
}
