//! Which validation rules RUN. Not how their diagnostics are shown.
//!
//! [`RuleSelection`] is the input that decides what the validator COMPUTES.
//! Its counterpart, the policy deciding how computed diagnostics are displayed
//! and counted, is `talkbank_transform::PresentationPolicy`, and the two are
//! deliberately in different crates; see the type's own documentation for the
//! defect that separation exists to make unrepresentable.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

/// Suffix appended by [`RuleSelection::cache_key_fragment`] when strict
/// cross-utterance linker validation is on.
const STRICT_LINKERS_FRAGMENT: &str = "+strict-linkers";

/// The set of validation rules that will actually run.
///
/// # The defining property
///
/// **Every field here can change WHAT THE VALIDATOR COMPUTES, which is why
/// this type, and only this type, derives the validation cache key.** A field
/// that cannot change the computed diagnostics does not belong here: it is a
/// presentation preference, and it belongs in
/// `talkbank_transform::PresentationPolicy` instead. That is the mistake to
/// catch in review, and it is the exact mistake that shipped in v0.6.0.
///
/// # Why this is a separate type at all
///
/// v0.6.0 held rule selection and presentation policy in ONE struct and folded
/// the whole struct into the cache key. Suppression (`--suppress`, a pure
/// display preference) therefore partitioned the cache: two runs differing only
/// in which codes they printed shared no entries, and a second pass over a
/// 106,000-file corpus re-validated every file from cold. The cure is not a
/// rule saying "remember to leave presentation out of the key"; a rule loses to
/// an affordance. The cure is that the key-deriving function cannot NAME a
/// presentation policy: `talkbank-cache` takes `&RuleSelection`, and the
/// presentation type lives in a crate that depends on `talkbank-cache`, so
/// reaching for it there is a dependency cycle, not a judgement call.
///
/// # Invariant this buys the cache
///
/// Two runs with equal `RuleSelection` compute the same diagnostics for the
/// same bytes. A cached row therefore records a fact about the file rather
/// than a rendering of it, and no presentation preference can invalidate it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RuleSelection {
    /// Run strict cross-utterance linker validation.
    ///
    /// When true, the quotation linkers (`+"`, `+"/. `, `+".`) and the
    /// completion linkers (`+,`, `++`) are checked for correct pairing with
    /// the terminators they continue. Off by default because many existing
    /// corpora do not follow these strict conventions. This is rule
    /// SELECTION, not presentation: with it off, the checks never execute and
    /// the diagnostics do not exist to be shown or hidden.
    ///
    /// Which codes it turns on is deliberately not written here. This doc said
    /// "(E351-E355)" and omitted E341, E344 and E346, which the same option
    /// enables; the module doc for `validation::cross_utterance` said
    /// something different and also wrong. The list that cannot drift is the
    /// generated error index, where each such code's page states the option it
    /// requires, derived from the spec example that demonstrates it.
    strict_linkers: bool,
}

impl RuleSelection {
    /// The default rule set: every always-on check, and no opt-in check.
    pub fn new() -> Self {
        Self {
            strict_linkers: false,
        }
    }

    /// Enable strict cross-utterance linker validation.
    pub fn with_strict_linkers(mut self) -> Self {
        self.strict_linkers = true;
        self
    }

    /// Whether strict cross-utterance linker validation will run.
    pub fn strict_linkers_enabled(&self) -> bool {
        self.strict_linkers
    }

    /// How many independent options this type carries.
    ///
    /// # Why a domain type publishes a count of its own fields
    ///
    /// The spec system must be able to DEMONSTRATE every rule the validator
    /// can run, and a rule is demonstrable only if some spec example can ask
    /// for it. `talkbank_spec_vocabulary::frontmatter::RuleProfile` is the
    /// list of askable rule sets; its `selection` method returns one of these
    /// by a total match, so a profile cannot name a rule that does not exist.
    /// The converse needs this number: an option no profile selects would be
    /// a rule the binary runs and no example can reach.
    ///
    /// The body destructures `Self` field by field with no `..` rest pattern,
    /// exactly as [`Self::cache_key_fragment`] does, so adding an option is a
    /// compile error here until the count beside the pattern is raised, and
    /// the spec runtime's `every_rule_option_is_askable_by_an_example` is red
    /// until a profile selects it.
    ///
    /// Before this existed the pairing was prose, and the prose was the false
    /// `status = "not_implemented"` on eight codes that fire.
    #[must_use]
    pub fn option_count(&self) -> usize {
        let Self { strict_linkers: _ } = self;
        1
    }

    /// Render this rule set as a deterministic, canonical text fragment for
    /// folding into a validation cache key
    /// (`talkbank_cache::RulesVersion::current_with_rule_selection`).
    ///
    /// # Why the destructure
    ///
    /// The body destructures `Self` FIELD BY FIELD with **no `..` rest
    /// pattern**, so adding a field to `RuleSelection` is a compile error here
    /// until someone folds it in. On this type that forcing is always correct,
    /// because by the type's defining property every field here changes what is
    /// computed and therefore belongs in the key. (The same destructure on the
    /// old combined config was the right instinct on the wrong struct: it
    /// forced presentation settings into the key, which is what broke caching.)
    ///
    /// # Why it lives here rather than in `talkbank-cache`
    ///
    /// The fields are private, and a hand-picked field list in a downstream
    /// crate is precisely the shape of bug this method exists to prevent: two
    /// such lists once drifted apart, one folding in strict-linkers but not
    /// the suppression set. One owner, no mirror to drift.
    ///
    /// The compile error a new field raises here is also the reader's cue to
    /// visit [`Self::option_count`], whose number decides whether the spec
    /// system can demonstrate the new rule at all.
    pub fn cache_key_fragment(&self) -> String {
        let Self { strict_linkers } = self;

        let mut fragment = String::new();
        // Turns on checks a lenient run never reaches at all, so a lenient
        // verdict is not an answer for a strict run.
        if *strict_linkers {
            fragment.push_str(STRICT_LINKERS_FRAGMENT);
        }
        fragment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one dimension this type carries must reach the cache key, or a
    /// lenient verdict could be served to a `--strict-linkers` run that wanted
    /// checks the lenient run never ran.
    #[test]
    fn cache_key_fragment_differs_when_strict_linkers_differs() {
        assert_ne!(
            RuleSelection::new().cache_key_fragment(),
            RuleSelection::new()
                .with_strict_linkers()
                .cache_key_fragment(),
            "strict_linkers changes which checks execute and must reach the key"
        );
    }

    /// Enabling the same dimension twice is the same rule set, so it must
    /// produce the same fragment: two call paths that converge on strict mode
    /// must share a cache.
    #[test]
    fn cache_key_fragment_is_idempotent() {
        assert_eq!(
            RuleSelection::new()
                .with_strict_linkers()
                .cache_key_fragment(),
            RuleSelection::new()
                .with_strict_linkers()
                .with_strict_linkers()
                .cache_key_fragment()
        );
    }

    /// The default rule set contributes nothing to the key, so the common case
    /// composes the shortest possible version string.
    #[test]
    fn default_rule_selection_contributes_no_fragment() {
        assert!(RuleSelection::new().cache_key_fragment().is_empty());
    }
}
