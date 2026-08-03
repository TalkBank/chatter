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
//! [`RulesVersion::current_with_rule_selection`] closes this without inverting the
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
/// in by [`RulesVersion::current_with_rule_selection`]. Placed immediately
/// after the rule-set fingerprint (before the rule-selection fragment appended
/// after it) because, unlike that fragment, the parser fingerprint is never
/// optional: every `current_with_rule_selection` call carries one.
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
    /// verdict back to a user MUST use [`Self::current_with_rule_selection`]
    /// instead, which requires a parser fingerprint.
    pub fn current() -> Self {
        let fingerprint = talkbank_model::validation_rules_fingerprint();
        Self(format!(
            "{CACHE_CRATE_VERSION}{VERSION_PART_SEPARATOR}{fingerprint}"
        ))
    }

    /// Build the version for the rule set compiled into this binary, further
    /// scoped to an active [`talkbank_model::RuleSelection`] AND an explicit
    /// PARSE-behaviour fingerprint.
    ///
    /// # Why the parameter is a `RuleSelection` and nothing wider
    ///
    /// A cache row records what validation FOUND, so the key must cover
    /// everything that can change what validation DOES, and nothing else.
    /// `RuleSelection`'s defining property is exactly that: every field in it
    /// changes what is computed (today, whether E351-E355 run at all). Anything
    /// that merely changes what a reader is shown is a
    /// `talkbank_transform::PresentationPolicy`, and folding one of those in
    /// here is the v0.6.0 regression this signature exists to prevent: a
    /// `--suppress` list reached the key, so every distinct suppression set got
    /// its own private cache and a second pass over a 106,000-file corpus
    /// re-validated all of it from cold.
    ///
    /// That is enforced by the crate graph rather than by this comment.
    /// `talkbank-transform` depends on `talkbank-cache`, so this crate cannot
    /// name `PresentationPolicy` at all: a future attempt to fold a
    /// presentation setting into the key is a compile error about an
    /// unreachable type, not a passing test and a slowdown a user finds three
    /// releases later.
    ///
    /// The fragment itself is computed by
    /// [`talkbank_model::RuleSelection::cache_key_fragment`], not enumerated
    /// here: that method destructures the whole struct with no `..` rest
    /// pattern, so a field added to `RuleSelection` is a compile error there
    /// until someone folds it in. This crate cannot do that destructuring
    /// itself (the fields are private), and re-enumerating them here would
    /// recreate the "someone has to remember" failure mode that shipped two
    /// real gaps: a flag never folded in at all, and a second hand-rolled
    /// cache-key builder in the CLI that folded strict-linkers but not the
    /// suppression list.
    ///
    /// # `parser_fingerprint` is mandatory, not optional
    ///
    /// This crate cannot compute a parser fingerprint itself (see the module
    /// doc comment for why depending on the parser crate is banned), so the
    /// caller supplies one. It is a required positional parameter rather than
    /// an `Option` or a builder method with a `Default`, on purpose: a caller
    /// CANNOT build a production `RulesVersion` while forgetting the parser
    /// dimension, the exact defect this type exists to make unrepresentable.
    /// Production callers pass `talkbank_parser::GRAMMAR_FINGERPRINT` (or its
    /// re-export through `talkbank-transform`); this crate treats the string as
    /// opaque bytes to fold in, never parsing or interpreting it. Tests that
    /// only need to model rule-set changes, independent of parse behaviour,
    /// pass a fixed literal (see this module's own tests below).
    pub fn current_with_rule_selection(
        rules: &talkbank_model::RuleSelection,
        parser_fingerprint: &str,
    ) -> Self {
        let fingerprint = talkbank_model::validation_rules_fingerprint();
        Self(format!(
            "{CACHE_CRATE_VERSION}{VERSION_PART_SEPARATOR}{fingerprint}\
             {PARSER_PART_SEPARATOR}{parser_fingerprint}{}",
            rules.cache_key_fragment()
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

    /// Adopt a version string read back out of the `version` column.
    ///
    /// The only values in that column were written by some build's composed
    /// version, so this is a round trip rather than a construction: it is how
    /// maintenance code (the reachability prune) names a version it found on
    /// disk without inventing one. Crate-internal on purpose; outside callers
    /// derive a version from a rule set, never from a string.
    pub(crate) fn from_stored(stored: String) -> Self {
        Self(stored)
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
    use talkbank_model::RuleSelection;

    /// A fixed stand-in parser/grammar fingerprint for tests that exercise the
    /// RULE-SELECTION dimension and want the parser dimension held constant.
    /// Real callers pass `talkbank_parser::GRAMMAR_FINGERPRINT`; this crate
    /// cannot depend on the parser crate (see the module doc comment), so tests
    /// here model the caller-supplied string with a literal.
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
    /// integration tests rely on to model a rules change.
    #[test]
    fn distinct_testing_labels_are_distinct_versions() {
        assert_ne!(
            RulesVersion::for_testing("a"),
            RulesVersion::for_testing("b")
        );
    }

    /// `current_with_rule_selection` is deterministic: the same rule selection
    /// and the same parser fingerprint yield the same version every time.
    #[test]
    fn current_with_rule_selection_is_stable_within_a_build() {
        let rules = RuleSelection::new();
        assert_eq!(
            RulesVersion::current_with_rule_selection(&rules, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_rule_selection(&rules, TEST_PARSER_FINGERPRINT)
        );
    }

    /// A default rule selection combined with a parser fingerprint is a
    /// DIFFERENT version from bare `current()`, which carries no parser
    /// dimension at all. The two constructors serve genuinely different callers
    /// (see each method's doc comment), not lenient/strict variants of one
    /// value.
    #[test]
    fn default_rule_selection_with_parser_fingerprint_differs_from_current() {
        let rules = RuleSelection::new();
        assert_ne!(
            RulesVersion::current_with_rule_selection(&rules, TEST_PARSER_FINGERPRINT),
            RulesVersion::current()
        );
    }

    /// Two versions built from the SAME rule selection but DIFFERENT parser
    /// fingerprints must not be equal. This is what lets the cache detect a
    /// grammar change: a grammar edit alters what parses, which alters what
    /// validates, even when the compiled-in rule set and the active rule
    /// selection are byte-identical.
    #[test]
    fn distinct_parser_fingerprints_are_distinct_versions() {
        let rules = RuleSelection::new();
        let a = RulesVersion::current_with_rule_selection(&rules, "grammar-fp-a");
        let b = RulesVersion::current_with_rule_selection(&rules, "grammar-fp-b");
        assert_ne!(
            a, b,
            "a rule selection held constant across two different parser fingerprints \
             must not collide to the same cache key"
        );
    }

    /// Strict-linker mode runs checks (E351-E355) a lenient run never reaches,
    /// so it must not share a cache row with one. The parser fingerprint is
    /// held constant to isolate the rule-selection dimension.
    #[test]
    fn strict_linkers_changes_the_version() {
        let lenient = RuleSelection::new();
        let strict = RuleSelection::new().with_strict_linkers();
        assert_ne!(
            RulesVersion::current_with_rule_selection(&strict, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_rule_selection(&lenient, TEST_PARSER_FINGERPRINT)
        );
    }

    /// The other direction, and the regression this whole split exists for:
    /// two runs that select the same rules land on the SAME key, however they
    /// intend to DISPLAY the result. There is no presentation parameter here to
    /// vary, which is the point; this test pins that the remaining dimensions
    /// leave equal rule selections equal.
    #[test]
    fn equal_rule_selections_produce_one_shared_key() {
        let built_one_way = RuleSelection::new().with_strict_linkers();
        let built_another = RuleSelection::new()
            .with_strict_linkers()
            .with_strict_linkers();
        assert_eq!(
            RulesVersion::current_with_rule_selection(&built_one_way, TEST_PARSER_FINGERPRINT),
            RulesVersion::current_with_rule_selection(&built_another, TEST_PARSER_FINGERPRINT),
            "two runs selecting the same rules must share one cache"
        );
    }
}
