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
//! `validation_rules_fingerprint` does NOT merely enumerate which error
//! codes exist: it also folds in a build-time hash of `talkbank-model`'s
//! entire `src/` tree, so a change to what a rule DOES (a predicate that
//! tightens or loosens, with no code added or removed) also produces a
//! different `RulesVersion`. That whole-tree hash deliberately
//! over-invalidates rather than trying to isolate exactly which files are
//! "validation-relevant"; see `talkbank-model`'s
//! `errors::codes::rules_fingerprint` module for why. The accepted tradeoff:
//! an occasional spurious cache miss costs seconds of re-validation, while a
//! stale verdict costs correctness, silently.
//!
//! # PARSE behaviour: the fourth dimension
//!
//! Rule behaviour alone is not enough: the grammar lives in a separate crate
//! (`tree-sitter-talkbank`), and this crate (`talkbank-cache`) deliberately
//! depends only on `talkbank-model`, not on the parser, so a grammar-level
//! fingerprint cannot be computed here without inverting that layering
//! (banned; see `talkbank-cache`'s own `CLAUDE.md`/crate docs on the
//! dependency direction). A grammar change alters what parses, which alters
//! what validates, so a cached verdict from before a grammar change is not a
//! valid answer after one.
//!
//! [`RulesVersion::current_with_config`] closes this without inverting the
//! layering: it takes the parser fingerprint as a mandatory `&str`
//! parameter rather than deriving it internally. `talkbank-cache` never
//! reads or interprets that string; it only folds the bytes it is handed
//! into the composed version. The caller supplying it (the CLI, the desktop
//! app) already depends on both `talkbank-parser` (transitively or
//! directly) and `talkbank-cache`, which is exactly the seam
//! `crate::CachePool::with_directory_and_rules_version` exists for. Making
//! the parameter mandatory, rather than optional or defaulted, is
//! deliberate: a caller cannot construct a production `RulesVersion` while
//! forgetting the parser dimension, because there is no overload that
//! allows it. See `talkbank_parser::GRAMMAR_FINGERPRINT` for how the value
//! itself is computed.
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
/// version = ?`), or, for callers that memoize one [`crate::CachePool`] per
/// active config (e.g. a long-lived app session), used as a map key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// Separator preceding the caller-supplied parser/grammar fingerprint folded
/// in by [`RulesVersion::current_with_config`]. Placed immediately after the
/// rule-set fingerprint (before the config fragment appended after it)
/// because, unlike the config fragment, the parser fingerprint is never
/// optional: every `current_with_config` call carries one.
const PARSER_PART_SEPARATOR: &str = "+parser.";

impl RulesVersion {
    /// Build the version for the rule set compiled into this binary,
    /// deliberately WITHOUT a parser/grammar dimension.
    ///
    /// Combines the cache crate's package version with the active validation
    /// rule-set fingerprint from `talkbank-model`. Production callers use
    /// this only for operations that never serve a validation VERDICT and so
    /// are not exposed to the PARSE-behaviour gap described in the module
    /// doc comment: [`crate::CachePool::new`] and
    /// [`crate::CachePool::with_directory`] back administrative commands
    /// (`chatter cache stats`, `chatter cache clear`) whose queries do not
    /// filter by `version` at all. Any caller that serves a pass/fail
    /// verdict back to a user MUST use [`Self::current_with_config`]
    /// instead, which requires a parser fingerprint.
    pub fn current() -> Self {
        let fingerprint = talkbank_model::validation_rules_fingerprint();
        Self(format!(
            "{CACHE_CRATE_VERSION}{VERSION_PART_SEPARATOR}{fingerprint}"
        ))
    }

    /// Build the version for the rule set compiled into this binary,
    /// further scoped to the CACHE-RELEVANT surface of an active
    /// [`talkbank_model::ValidationConfig`] AND an explicit PARSE-behaviour
    /// fingerprint.
    ///
    /// A `ValidationConfig` now joins the rule set upstream of validation
    /// (see `ChatFile::validate_with_config` /
    /// `validate_with_alignment_and_config`), so whether a file counts as
    /// Valid depends on the FULL active config: a disabled code is never
    /// emitted at all, a downgraded/upgraded code changes severity, strict
    /// linker mode runs additional checks (E351-E355) a lenient run never
    /// reaches, and `upgrade_unmapped_warnings` can turn any unmapped
    /// warning into an error. A cache row produced under one active config
    /// is not a valid answer for a different one.
    ///
    /// The CACHE-RELEVANT surface is computed by
    /// [`talkbank_model::ValidationConfig::cache_key_fragment`], not
    /// enumerated here: that method destructures the whole struct with no
    /// `..` rest pattern, so a field added to `ValidationConfig` in the
    /// future is a compile error there until someone decides whether it
    /// belongs in the cache key, rather than a silent omission in a
    /// hand-picked field list on this side of the crate boundary. This
    /// crate cannot do that destructuring itself: `ValidationConfig`'s
    /// fields are private, and even if they were public, re-enumerating
    /// them here would recreate exactly the "someone has to remember"
    /// failure mode that shipped two real gaps (`upgrade_unmapped_warnings`
    /// never folded in at all, and a separate CLI cache-key builder that
    /// folded `strict_linkers` but not `--suppress`; see that method's doc
    /// comment for the full incident).
    ///
    /// # `parser_fingerprint` is mandatory, not optional
    ///
    /// This crate cannot compute a parser fingerprint itself (see the module
    /// doc comment for why depending on the parser crate is banned), so the
    /// caller supplies one. It is a required positional parameter rather
    /// than an `Option` or a builder method with a `Default`, on purpose:
    /// the whole point is that a caller CANNOT build a production
    /// `RulesVersion` while forgetting the parser dimension, the exact
    /// defect this type exists to make unrepresentable. Production callers
    /// pass `talkbank_parser::GRAMMAR_FINGERPRINT` (or its re-export through
    /// `talkbank-transform`); this crate treats the string as opaque bytes
    /// to fold in, never parsing or interpreting it. Tests that only need to
    /// model rule-set changes, independent of parse behaviour, pass a fixed
    /// literal (see this module's own tests below).
    pub fn current_with_config(
        config: &talkbank_model::ValidationConfig,
        parser_fingerprint: &str,
    ) -> Self {
        let fingerprint = talkbank_model::validation_rules_fingerprint();
        Self(format!(
            "{CACHE_CRATE_VERSION}{VERSION_PART_SEPARATOR}{fingerprint}\
             {PARSER_PART_SEPARATOR}{parser_fingerprint}{}",
            config.cache_key_fragment()
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

    /// A fixed stand-in parser/grammar fingerprint for tests that exercise
    /// the CONFIG dimension and want the parser dimension held constant.
    /// Real callers pass `talkbank_parser::GRAMMAR_FINGERPRINT`; this crate
    /// cannot depend on the parser crate (see the module doc comment), so
    /// tests here model the caller-supplied string with a literal.
    const TEST_PARSER_FINGERPRINT: &str = "grammar-fp-test";

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

    /// `current_with_config` is deterministic: the same config and the same
    /// parser fingerprint yield the same version every time.
    #[test]
    fn current_with_config_is_stable_within_a_build() {
        let config = talkbank_model::ValidationConfig::new();
        assert_eq!(
            RulesVersion::current_with_config(&config, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&config, TEST_PARSER_FINGERPRINT)
        );
    }

    /// A default (no overrides, no strict-linkers) config combined with a
    /// parser fingerprint is now a DIFFERENT version from bare `current()`,
    /// which carries no parser dimension at all. Before the parser
    /// dimension existed, the default config was equivalent to `current()`;
    /// that equivalence is gone on purpose, because `current_with_config`
    /// always folds in a parser fingerprint and `current()` never does. The
    /// two constructors now serve genuinely different callers (see each
    /// method's doc comment), not lenient/strict variants of the same
    /// value.
    #[test]
    fn default_config_with_parser_fingerprint_differs_from_current() {
        let config = talkbank_model::ValidationConfig::new();
        assert_ne!(
            RulesVersion::current_with_config(&config, TEST_PARSER_FINGERPRINT),
            RulesVersion::current()
        );
    }

    /// Two `RulesVersion` values built from the SAME config but DIFFERENT
    /// parser fingerprints must not be equal. This is the property that
    /// lets the cache detect a grammar change: a grammar edit alters what
    /// parses, which alters what validates, even when the compiled-in
    /// validation rule set and the active config are byte-identical. Before
    /// `current_with_config` took a parser fingerprint at all, two builds
    /// compiled against different grammars produced the SAME `RulesVersion`
    /// here, and a stale verdict from before the grammar change could be
    /// served after it.
    #[test]
    fn distinct_parser_fingerprints_are_distinct_versions() {
        let config = talkbank_model::ValidationConfig::new();
        let a = RulesVersion::current_with_config(&config, "grammar-fp-a");
        let b = RulesVersion::current_with_config(&config, "grammar-fp-b");
        assert_ne!(
            a, b,
            "a config held constant across two different parser fingerprints \
             must not collide to the same cache key"
        );
    }

    /// Disabling a code changes the version: this is the property that lets
    /// the cache distinguish a suppressed verdict from an unsuppressed one.
    /// The parser fingerprint is held constant so this test isolates the
    /// CONFIG dimension, not the parser dimension `distinct_parser_fingerprints_are_distinct_versions` already covers.
    #[test]
    fn disabled_codes_change_the_version() {
        let default_config = talkbank_model::ValidationConfig::new();
        let config = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        assert_ne!(
            RulesVersion::current_with_config(&config, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&default_config, TEST_PARSER_FINGERPRINT)
        );
    }

    /// Different disabled-code sets produce different versions.
    #[test]
    fn distinct_disabled_code_sets_are_distinct_versions() {
        let a = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        let b = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::UnparsableContent);
        assert_ne!(
            RulesVersion::current_with_config(&a, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&b, TEST_PARSER_FINGERPRINT)
        );
    }

    /// Order must not matter: two configs that disable the same set of
    /// codes in a different order (as a `HashMap` iteration would) must
    /// produce the SAME version, or the cache would miss every time despite
    /// an identical active rule set. This is the trap the `RulesVersion`
    /// doc comment on `current_with_config` calls out explicitly.
    #[test]
    fn disabled_code_order_does_not_affect_the_version() {
        let forward = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex)
            .disable(talkbank_model::ErrorCode::UnparsableContent);
        let backward = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::UnparsableContent)
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        assert_eq!(
            RulesVersion::current_with_config(&forward, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&backward, TEST_PARSER_FINGERPRINT)
        );
    }

    /// Disabling the same code twice does not change the version (a
    /// `HashMap` entry cannot literally duplicate, but two configs built by
    /// different code paths that converge on the same disabled set must
    /// still compare equal).
    #[test]
    fn duplicate_disabled_codes_do_not_affect_the_version() {
        let once = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        let twice = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex)
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        assert_eq!(
            RulesVersion::current_with_config(&once, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&twice, TEST_PARSER_FINGERPRINT)
        );
    }

    /// Item 4's confirmation: strict-linkers-only and suppression-only
    /// configs must produce DIFFERENT cache keys from each other AND from
    /// the default. Before this test existed, only the disabled-code set
    /// was folded into the version, so a `--strict-linkers` run and a plain
    /// unsuppressed run shared a key even though strict mode can flip a
    /// file from Valid to Invalid: the same stale-verdict bug this whole
    /// module exists to close, reopened one dimension over. The parser
    /// fingerprint is held constant so this test isolates the CONFIG
    /// dimension.
    #[test]
    fn strict_linkers_and_suppression_produce_distinct_cache_keys() {
        let default_config = talkbank_model::ValidationConfig::new();
        let strict_only = talkbank_model::ValidationConfig::new().with_strict_linkers();
        let suppress_only = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex);
        let both = talkbank_model::ValidationConfig::new()
            .disable(talkbank_model::ErrorCode::InvalidOverlapIndex)
            .with_strict_linkers();

        let default_version =
            RulesVersion::current_with_config(&default_config, TEST_PARSER_FINGERPRINT);
        let strict_version =
            RulesVersion::current_with_config(&strict_only, TEST_PARSER_FINGERPRINT);
        let suppress_version =
            RulesVersion::current_with_config(&suppress_only, TEST_PARSER_FINGERPRINT);
        let both_version = RulesVersion::current_with_config(&both, TEST_PARSER_FINGERPRINT);

        assert_ne!(
            strict_version, default_version,
            "strict-linkers-only must differ from the default"
        );
        assert_ne!(
            strict_version, suppress_version,
            "strict-linkers-only must differ from suppression-only"
        );
        assert_ne!(
            suppress_version, default_version,
            "suppression-only must differ from the default"
        );
        assert_ne!(
            both_version, strict_version,
            "both active must differ from strict-linkers alone"
        );
        assert_ne!(
            both_version, suppress_version,
            "both active must differ from suppression alone"
        );
    }

    /// `ValidationConfig::strict()`'s `upgrade_unmapped_warnings` flag can
    /// flip a file from Valid to Invalid (any warning lacking an explicit
    /// override becomes an error), so it must be folded into the version.
    ///
    /// This is the gap `current_with_config`'s old hand-picked field list
    /// left open: `strict()` differs from `new()` in exactly this one
    /// field, and nothing folded it in until `ValidationConfig` grew
    /// `cache_key_fragment` (an exhaustive destructure with no `..` rest
    /// pattern) and this function started delegating to it instead of
    /// enumerating fields itself.
    #[test]
    fn upgrade_unmapped_warnings_changes_the_version() {
        let default_config = talkbank_model::ValidationConfig::new();
        let strict_config = talkbank_model::ValidationConfig::strict();
        assert_ne!(
            RulesVersion::current_with_config(&default_config, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_config(&strict_config, TEST_PARSER_FINGERPRINT),
            "upgrade_unmapped_warnings must be folded into the cache key: \
             ValidationConfig::strict() differs from ValidationConfig::new() \
             only in that flag"
        );
    }
}
