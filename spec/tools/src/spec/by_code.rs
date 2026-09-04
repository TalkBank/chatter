//! Every loaded spec, grouped by the code it is FOR.
//!
//! # The defect this type exists to make unrepresentable
//!
//! Eleven codes are claimed by more than one spec file, and the published
//! documentation used to key a `BTreeMap` on the code, so the last spec in
//! filename order silently replaced every earlier one. Twelve specs were loaded,
//! parsed, validated, and then dropped on the floor: they appeared in no page
//! and in no index row, and nothing anywhere said so.
//!
//! The map's TYPE was the lie. `BTreeMap<&str, &ErrorSpec>` asserts one spec per
//! code, and the only way to satisfy it from a corpus that has more is to throw
//! specs away. Last-wins was not a decision anybody made; it was what the type
//! required, arrived at by `_auto` sorting after most other suffixes.
//!
//! # Why there is no winner to pick, for most of them
//!
//! The obvious repair, adjudicate each pair and keep the better spec, is wrong
//! for SEVEN of the eleven, and the data says so: their specs disagree about
//! `Level` or `Layer`, and the disagreements are correct.
//! `E519_l1_of_language_code` is a header-level rule and
//! `E519_word_level_language_code` (word-position language codes, whose fault
//! site is the utterance) is an utterance-level rule, both genuinely reported
//! as E519. A code names a diagnostic, not a single triggering construct, so
//! several specs under one code is a legitimate state of the world.
//!
//! What was illegitimate was a data model that could not say it.
//!
//! # What this doc claimed, twice, and what measurement says
//!
//! Until 2026-08-19 this said EVERY contested code disagreed about `Level`,
//! `Category` or `Layer`. That was true and misleading: `Category` was the only
//! field splitting E202, E241, E243 and two of E519's three files, which are
//! otherwise identical in `Level` and `Layer`. Since `Category` was a published
//! grouping string that no generation decision read, "they disagree" there meant
//! only that two authors had named the same site differently.
//!
//! **The replacement sentence, that the other seven are structural, was ALSO
//! too strong, and its error is the more interesting one.** When measured, a
//! `Level` was declared per file (it moved onto each example in Phase 2), and
//! for an unedited `_auto` stub a GENERATOR wrote it by running the parser.
//! So a stub disagreeing with its sibling about `Level` is
//! not evidence of a second triggering site; it is a machine observation being
//! read as an authored claim, which is the reverse arrow R5 deleted.
//!
//! Measured 2026-08-19 over `spec/errors/`, with the generator's
//! "Review and enhance this specification as needed" note as the discriminator
//! (91 files carry it, all `_auto`, none hand-named):
//!
//! - NINE of the eleven contested codes carry an unedited stub beside an
//!   authored spec: E202, E241, E243, E316, E342, E375, E519, E522, E604.
//! - TWO have no stub at all, so both their specs are authored: E360 and E502.
//!   E519's other two files are likewise authored, and genuinely are one rule
//!   at a header site and an utterance site.
//!
//! So the honestly authored-versus-authored disagreements are E360, E502 and
//! the E519 pair. The remaining five are stub-versus-authored, and whether the
//! stub's example covers a real second site is R8's adjudication, which the
//! stub's own `Level` cannot answer.
//!
//! `Category` was deleted on 2026-08-19, so nothing in their metadata separates
//! them any more. **What that means was then measured by RUNNING them**
//! and comparing their DIAGNOSTICS rather than their declarations. Metadata
//! agreement turned out to be a poor proxy in both directions: E202 and E241
//! really are one rule written twice, E243's `_auto` emits E202 so it is
//! MISFILED rather than duplicated, E519's is a different rule entirely, and
//! E604, which this predicate cannot see, emits diagnostics identical to its
//! sibling's. This type keeps publishing all of them regardless, because
//! dropping one silently is the defect it exists to prevent.
//!
//! Only E202 renders as two BYTE-IDENTICAL index rows; the other three are still
//! told apart by the `Name` column, which for the stubs reads "Auto-generated
//! from corpus". Verified against the regenerated index with
//! `awk '/^\| \[E(202|241|243|519)\]/' docs/errors/index.md`.

use std::collections::BTreeMap;
use std::fmt;

use crate::spec::error::ErrorSpec;
use crate::spec::metadata::SpecErrorCode;

/// The specs that claim ONE code, of which there is always at least one.
///
/// Non-emptiness is STRUCTURAL, not asserted: the first spec is a field, so
/// there is no empty value to construct and no `unreachable` arm to write when
/// matching. A `Vec` plus "invariant: never empty" in a doc comment would be
/// the same shape this module exists to delete.
#[derive(Debug)]
pub struct CodeSpecs {
    /// The spec that claimed the code first, in load order (filename order).
    first: ErrorSpec,
    /// Any further specs claiming the same code, in load order.
    rest: Vec<ErrorSpec>,
}

/// How many specs claim a code, as something a consumer must MATCH on.
///
/// A borrowed view rather than a shape [`CodeSpecs`] is stored in, because the
/// owner's job is to hold the specs and this type's job is to make a renderer
/// handle both cases. Returned by [`CodeSpecs::view`].
#[derive(Debug)]
pub enum CodeSpecsView<'a> {
    /// Exactly one spec claims this code, which is true of most of them.
    Sole(&'a ErrorSpec),
    /// Several distinct rules are reported under one code.
    Several {
        /// The first, in load order.
        first: &'a ErrorSpec,
        /// The others: never empty in this variant, by construction of `view`.
        rest: &'a [ErrorSpec],
    },
}

impl CodeSpecs {
    /// Start a code's list with the first spec that claims it.
    fn new(first: ErrorSpec) -> Self {
        Self {
            first,
            rest: Vec::new(),
        }
    }

    /// Record a further spec claiming the same code.
    fn push(&mut self, spec: ErrorSpec) {
        self.rest.push(spec);
    }

    /// One spec, or several, as an exhaustive match.
    #[must_use]
    pub fn view(&self) -> CodeSpecsView<'_> {
        match self.rest.as_slice() {
            [] => CodeSpecsView::Sole(&self.first),
            rest => CodeSpecsView::Several {
                first: &self.first,
                rest,
            },
        }
    }

    /// Every spec claiming this code, in load order.
    pub fn iter(&self) -> impl Iterator<Item = &ErrorSpec> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    /// The groups of specs under this code that NOTHING distinguishes.
    ///
    /// Two specs are reported here when they agree on the DERIVED set of
    /// their examples' levels ([`ErrorSpec::levels`]), the one per-input fact
    /// left since `layer` fell to R4 and `level` moved onto the example, so
    /// nothing in the model tells them apart.
    ///
    /// **It does NOT decide residue, and it must not be read as doing so.**
    /// Checked on 2026-08-20 by running every contested spec's examples through
    /// the validator: of the four this reports, two really are one rule written
    /// twice (E202, E241), one is MISFILED rather than duplicated (E243's stub
    /// emits E202), and one is a FALSE POSITIVE (E519's stub is a different
    /// rule). It also misses E604, whose two specs emit identical diagnostics.
    /// Declared metadata is weak evidence about meaning in both directions,
    /// because a generator wrote a stub's `Level` by running the parser.
    ///
    /// The coverage question that IS decidable from the specs is
    /// [`ErrorSpec::demonstration`](crate::spec::error::ErrorSpec::demonstration),
    /// enforced structurally since R2 (an example must claim). This
    /// stays as a report an author may find suggestive.
    ///
    /// Linear search rather than a map: the largest contested code has three
    /// specs, so this is at worst nine comparisons and a map would cost more
    /// to read than it saves.
    fn indistinguishable<'a>(&'a self, code: &'a SpecErrorCode) -> Vec<Indistinguishable<'a>> {
        // A code with one spec has nothing to compare, and `view` is already
        // the type that knows. This also keeps the allocation below off the
        // ~225 codes that are `Sole`.
        let CodeSpecsView::Several { .. } = self.view() else {
            return Vec::new();
        };

        let mut groups: Vec<(&ErrorSpec, Vec<&ErrorSpec>)> = Vec::new();
        for spec in self.iter() {
            // Levels alone since R4: `layer` was the other component, and it
            // was an authored field the standing ruling already barred from
            // answering spec sameness (a generator wrote it for unedited
            // stubs). Fewer discriminators means MORE pairs read as
            // indistinguishable here, which is the honest direction: the real
            // discriminator is what the examples EMIT, measured by
            // `adjudicate_contested_spec.sh` against the snapshot.
            let levels = spec.levels();
            match groups.iter_mut().find(|(head, _)| head.levels() == levels) {
                Some((_, tail)) => tail.push(spec),
                None => groups.push((spec, Vec::new())),
            }
        }

        groups
            .into_iter()
            .filter_map(|(first, rest)| {
                let (second, rest) = rest.split_first()?;
                Some(Indistinguishable {
                    code,
                    first,
                    second,
                    rest: rest.to_vec(),
                })
            })
            .collect()
    }
}

/// Specs under one code that agree on the derived set of example levels.
///
/// Not a defect the loader can refuse: the files exist, and publishing both is
/// deliberate, because silently keeping one is what [`SpecsByCode`] replaced.
/// It is a POINTER, and this type is what lets a report name the files instead
/// of a paragraph asserting there are four. It is deliberately not a gate: one
/// was built on this predicate on 2026-08-19 and withdrawn the next morning,
/// for the reason [`CodeSpecs::indistinguishable`] records.
///
/// TWO specs minimum, structurally. This started as a `Vec` with "always two or
/// more" in a doc comment, which is the shape [`CodeSpecs`] forty lines above
/// exists to delete; it cost a `retain`, a runtime assertion in the test, and
/// three panicking index expressions, all of which went with it.
#[derive(Debug)]
pub struct Indistinguishable<'a> {
    /// The code every spec in this group claims.
    pub code: &'a SpecErrorCode,
    /// The first, in load order.
    pub first: &'a ErrorSpec,
    /// The second: what makes this a group at all.
    pub second: &'a ErrorSpec,
    /// Any further specs agreeing with them. Usually empty.
    pub rest: Vec<&'a ErrorSpec>,
}

impl<'a> Indistinguishable<'a> {
    /// Every spec in the group, in load order.
    pub fn specs(&self) -> impl Iterator<Item = &'a ErrorSpec> + '_ {
        std::iter::once(self.first)
            .chain(std::iter::once(self.second))
            .chain(self.rest.iter().copied())
    }
}

/// One line naming the files, for a report AND for a gate's failure message.
///
/// One owner, because those are the two texts a person compares when the gate
/// fires and they had already drifted apart in separator by the time this was
/// written.
impl fmt::Display for Indistinguishable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.code)?;
        let mut first = true;
        for spec in self.specs() {
            if !first {
                f.write_str(" + ")?;
            }
            f.write_str(spec.source_file())?;
            first = false;
        }
        Ok(())
    }
}

/// Every loaded spec, grouped by code, with nothing discarded.
///
/// Sorted by code, because [`SpecErrorCode`] orders `E001 < E002 < ... < W101`
/// and every consumer of this wants that order.
#[derive(Debug, Default)]
pub struct SpecsByCode {
    /// Code to the specs claiming it.
    by_code: BTreeMap<SpecErrorCode, CodeSpecs>,
}

impl SpecsByCode {
    /// Group loaded specs by the code each is for.
    ///
    /// Takes the specs by value: this is the owner afterwards, so there is no
    /// second collection for a later reader to consult and disagree with. That
    /// second collection is precisely what the page loop and the index used to
    /// be, held together by a doc comment promising their rules matched.
    #[must_use]
    pub fn group(specs: Vec<ErrorSpec>) -> Self {
        let mut by_code: BTreeMap<SpecErrorCode, CodeSpecs> = BTreeMap::new();
        for spec in specs {
            let code = spec.error.code.clone();
            match by_code.get_mut(&code) {
                Some(existing) => existing.push(spec),
                None => {
                    by_code.insert(code, CodeSpecs::new(spec));
                }
            }
        }
        Self { by_code }
    }

    /// Each code and the specs claiming it, in code order.
    pub fn codes(&self) -> impl Iterator<Item = (&SpecErrorCode, &CodeSpecs)> {
        self.by_code.iter()
    }

    /// Every spec, in code order and then load order.
    ///
    /// The count of this equals the count that went in, which is the whole
    /// point of the type and is asserted by `grouping_discards_nothing`.
    pub fn specs(&self) -> impl Iterator<Item = &ErrorSpec> {
        self.by_code.values().flat_map(CodeSpecs::iter)
    }

    /// Every group of specs, across all codes, that nothing distinguishes.
    ///
    /// Derived from the specs rather than listed by hand, which is the one
    /// property worth keeping from the withdrawn gate: this module's own
    /// history is what happens when a fact about these codes lives only in
    /// prose.
    #[must_use]
    pub fn indistinguishable(&self) -> Vec<Indistinguishable<'_>> {
        self.by_code
            .iter()
            .flat_map(|(code, specs)| specs.indistinguishable(code))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::error::ErrorSpec;

    /// Load the real spec corpus, which is where the contested codes live.
    fn real_specs() -> Vec<ErrorSpec> {
        let root = crate::repo_paths::RepoRoot::resolve(None).expect("a chatter checkout");
        ErrorSpec::load_for_repo(&root).expect("the committed spec corpus loads")
    }

    /// The property the old `BTreeMap<&str, &ErrorSpec>` could not have.
    ///
    /// A MEASUREMENT of the whole corpus rather than an invariant a type can
    /// hold: it says the grouping is lossless for the specs that actually
    /// exist, which is the claim `deduplicate_by_code` could not make.
    #[test]
    fn grouping_discards_nothing() {
        let specs = real_specs();
        let before = specs.len();
        let grouped = SpecsByCode::group(specs);
        assert_eq!(
            grouped.specs().count(),
            before,
            "grouping must not drop a spec; that is the bug this type replaced"
        );
    }
}
