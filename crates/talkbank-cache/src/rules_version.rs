//! The cache-compatibility version folded into every cache row.
//!
//! # The seam this guards
//!
//! A cached pass/fail verdict is only reusable if the *validation rules* that
//! produced it are still in force. The cache's `version` column historically
//! held only the `talkbank-cache` crate's package version, which does NOT
//! change when validation rules change (rules live in `talkbank-model`). That
//! let stale "Valid" verdicts outlive the addition of a rule like E370, so
//! `chatter validate` returned a wrong "Valid" while a fresh validation
//! rejected the file.
//!
//! [`RulesVersion`] is the typed value stored in the `version` column. The
//! [`RulesVersion::current`] constructor combines the crate package version
//! with [`talkbank_model::validation_rules_fingerprint`], so any rule-set
//! change yields a different `RulesVersion`, which makes prior rows a cache
//! MISS (they remain on disk for selective re-testing under their old version,
//! but are never served to a query carrying the new version).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

/// Cache-compatibility version stored in the `version` column of every cache
/// row, and matched on every read.
///
/// # Invariant
///
/// Two builds that share the same `talkbank-cache` package version AND the
/// same validation rule set produce equal `RulesVersion` values; any
/// difference in either dimension produces a different value. This is what
/// makes the cache self-invalidate across rule changes.
///
/// The wrapped string is opaque: callers must not parse or depend on its
/// internal shape. It is only ever compared for equality (inside SQL `WHERE
/// version = ?`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesVersion(String);

/// The `talkbank-cache` crate's package version. Bumping the crate (e.g. a
/// cache schema/serialization change) still invalidates the cache, independent
/// of the validation rules, so both dimensions are folded into the version.
const CACHE_CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Separator between the crate version and the rule-set fingerprint in the
/// composed version string. A `+` keeps the two dimensions visually distinct in
/// any diagnostic dump without colliding with hex fingerprint or semver
/// characters.
const VERSION_PART_SEPARATOR: &str = "+rules.";

impl RulesVersion {
    /// Build the version for the rule set compiled into this binary.
    ///
    /// Combines the cache crate's package version with the active validation
    /// rule-set fingerprint from `talkbank-model`. This is what production
    /// callers ([`crate::CachePool::new`], [`crate::CachePool::with_directory`])
    /// use.
    pub fn current() -> Self {
        let fingerprint = talkbank_model::validation_rules_fingerprint();
        Self(format!(
            "{CACHE_CRATE_VERSION}{VERSION_PART_SEPARATOR}{fingerprint}"
        ))
    }

    /// Construct a `RulesVersion` from an explicit label, for tests that need
    /// to drive two distinct rule-set versions without recompiling against a
    /// different rule set.
    ///
    /// This is a test-support seam, not a production constructor: production
    /// code derives the version from the real rule set via [`Self::current`].
    /// It is exposed (not `#[cfg(test)]`) so integration tests in dependent
    /// crates can stand up "before rule X" / "after rule X" caches.
    pub fn for_testing(label: &str) -> Self {
        Self(label.to_owned())
    }

    /// Borrow the underlying string for binding into a SQL parameter.
    ///
    /// Kept crate-internal: outside code has no business reading the raw
    /// version string, only comparing `RulesVersion` values.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `current()` is stable within a build: the same rule set yields the same
    /// version every time.
    #[test]
    fn current_is_stable_within_a_build() {
        assert_eq!(RulesVersion::current(), RulesVersion::current());
    }

    /// The composed version embeds the crate version, so a pure crate bump
    /// (with rules unchanged) would still alter the stored version.
    #[test]
    fn current_embeds_the_crate_version() {
        let version = RulesVersion::current();
        assert!(
            version.as_str().starts_with(CACHE_CRATE_VERSION),
            "version {:?} should begin with the crate version {:?}",
            version.as_str(),
            CACHE_CRATE_VERSION
        );
    }

    /// Distinct test labels produce distinct versions, the property the
    /// integration test relies on to model a rules change.
    #[test]
    fn distinct_testing_labels_are_distinct_versions() {
        assert_ne!(
            RulesVersion::for_testing("a"),
            RulesVersion::for_testing("b")
        );
    }
}
