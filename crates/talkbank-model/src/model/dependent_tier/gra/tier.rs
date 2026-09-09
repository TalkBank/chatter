//! `%gra` tier model and structural validation helpers.
//!
//! CHAT reference anchors:
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)

use super::super::WriteChat;
use super::relation::GrammaticalRelation;
use super::tier_type::GraTierType;
use crate::Span;
use crate::alignment::indices::SemanticWordIndex1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Grammatical relations tier (%gra).
///
/// Contains dependency syntax annotations using Universal Dependencies relations.
/// Each relation specifies how morphological chunks in the %mor tier relate syntactically.
///
/// # Alignment with %mor
///
/// The %gra tier aligns with **morphological chunks**, not individual %mor items:
/// - Clitics in %mor (e.g., `pro|it~v|be`) produce **two** %gra relations
/// - Non-clitic words produce **one** %gra relation each
/// - Terminators get their own %gra relation (typically PUNCT)
///
/// # Dependency Relations
///
/// Uses Universal Dependencies relation types including:
/// - **ROOT**: Main predicate of sentence (head = 0)
/// - **NSUBJ**: Nominal subject
/// - **OBJ**: Direct object
/// - **IOBJ**: Indirect object
/// - **DET**: Determiner
/// - **AMOD**: Adjectival modifier
/// - **ADVMOD**: Adverbial modifier
/// - **PUNCT**: Punctuation
///
/// A label is a UD universal relation, optionally followed by a
/// language-specific subtype (`NMOD-POSS`, `ACL-RELCL`). The head is a
/// closed set of 37 and is enforced by validation (`E761`); subtypes are
/// open by UD's own design and are not checked.
///
/// # CHAT Manual Reference
///
/// - [Grammatical Relations Tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
/// - [Universal Dependencies](https://universaldependencies.org/)
///
/// Note: The legacy CLAN MOR manual has been removed from the TalkBank
/// website. GRA now follows UD-oriented conventions documented in the
/// CHAT manual.
///
/// # Example
///
/// ```
/// use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation};
///
/// // Create a %gra tier
/// let gra = GraTier::new_gra(vec![
///     GrammaticalRelation::new(1, 2, "NSUBJ"),   // Word 1 is subject of word 2
///     GrammaticalRelation::new(2, 0, "ROOT"),   // Word 2 is root
///     GrammaticalRelation::new(3, 2, "OBJ"),    // Word 3 is object of word 2
///     GrammaticalRelation::new(4, 2, "PUNCT"),  // Terminator
/// ]);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct GraTier {
    /// Type of grammatical relations tier.
    pub tier_type: GraTierType,

    /// Dependency relations for each morphological chunk.
    ///
    /// Each relation specifies word_index, head_index, and relation_type.
    /// Relations are in the same order as morphological chunks in the %mor tier.
    pub(crate) relations: GraRelations,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: Span,

    /// Whether every relation the `%gra` LINE declared is in `relations`.
    ///
    /// Not serialized, not compared, not shifted: it is a fact about the PARSE
    /// that produced this value, in the same class as [`Self::span`], and two
    /// tiers holding the same relations are the same tier whatever it took to
    /// build them. A JSON round trip therefore comes back
    /// [`GraCompleteness::Unknown`], which is the honest answer, rather than
    /// claiming a completeness nothing established.
    #[serde(skip, default)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub(crate) completeness: GraCompleteness,
}

/// Whether a `%gra` tier holds every relation its line declared.
///
/// A relation the model cannot represent is REJECTED and dropped, so the tier
/// is shorter than the author wrote it, and the rules that describe it as a
/// dependency graph then describe our recovery instead. Nothing in a
/// `Vec<GrammaticalRelation>` can say that, so this says it.
///
/// # Why `Unknown` is judged rather than withheld
///
/// Only a parser that KNOWS it dropped something sets `Truncated`, so the
/// suppression is exact. Everything else, including a hand-built tier and a
/// tier read back from JSON, is `Unknown` and IS judged. Withholding on
/// `Unknown` would be the fail-closed reading, and it is the wrong one here: a
/// rule that switches itself off wherever the plumbing is incomplete is the
/// gate-that-skips-itself failure, and it would be invisible, whereas a
/// cascade is at least visible in the output. The cost of this direction is
/// stated rather than hidden: a future lowering that drops a relation without
/// saying so reopens the cascade, and no type here can stop it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GraCompleteness {
    /// No parser said. The default, and it is JUDGED; see above.
    #[default]
    Unknown,
    /// Every relation on the line is present.
    Whole,
    /// At least one relation was rejected and is not here.
    Truncated,
}

impl GraTier {
    /// Constructs a grammatical-relations tier from parsed relations.
    ///
    /// Says nothing about completeness: a caller holding a bare `Vec` has not
    /// established whether anything was dropped on the way. A parser that
    /// knows says so with [`Self::lowered_from`].
    pub fn new(tier_type: GraTierType, relations: Vec<GrammaticalRelation>) -> Self {
        Self {
            tier_type,
            relations: relations.into(),
            span: Span::DUMMY,
            completeness: GraCompleteness::Unknown,
        }
    }

    /// Constructs a tier from a lowering that knows what it dropped.
    ///
    /// The one route to a [`GraCompleteness`] other than `Unknown`, so a claim
    /// of completeness can only come from the code that counted.
    #[must_use]
    pub fn lowered_from(
        relations: Vec<GrammaticalRelation>,
        completeness: GraCompleteness,
    ) -> Self {
        Self {
            completeness,
            ..Self::new_gra(relations)
        }
    }

    /// Whether every relation this tier's line declared is here.
    #[must_use]
    pub fn completeness(&self) -> GraCompleteness {
        self.completeness
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Convenience constructor for standard `%gra`.
    pub fn new_gra(relations: Vec<GrammaticalRelation>) -> Self {
        Self::new(GraTierType::Gra, relations)
    }

    /// Returns `true` if this tier serializes as `%gra`.
    pub fn is_gra(&self) -> bool {
        self.tier_type == GraTierType::Gra
    }

    /// Borrows the list of grammatical relations.
    pub fn relations(&self) -> &[GrammaticalRelation] {
        &self.relations.0
    }

    /// Return the relation at a 1-indexed `%gra` semantic position.
    ///
    /// This is the typed accessor for callers that already work in the
    /// author-written `%gra` index/head space and want to avoid ad hoc
    /// `index - 1` conversions.
    pub fn relation_at_semantic_index(
        &self,
        index: SemanticWordIndex1,
    ) -> Option<&GrammaticalRelation> {
        self.relations.get(index.to_chunk_index().as_usize())
    }

    /// Consumes the tier and returns the underlying grammatical relations.
    pub fn into_relations(self) -> Vec<GrammaticalRelation> {
        self.relations.0
    }

    /// Mutably borrows the list of grammatical relations.
    ///
    /// # Invariants
    ///
    /// Callers must ensure that mutations do not break index sequentiality
    /// or other structural invariants.
    pub fn relations_mut(&mut self) -> &mut [GrammaticalRelation] {
        &mut self.relations.0
    }

    /// Number of dependency edges in this tier.
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    /// Returns `true` when no dependency edges are present.
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    /// Report the `%gra` structural rules that hold whatever was dropped.
    ///
    /// E723 and E724 only. E721 and E722 need [`WholeGra`], which explains the
    /// split.
    ///
    /// Eight false assertions went with the rewrite, counted with `rg -i
    /// disabled` and `rg 'as WARNING|W72[23]:'` against this file at the
    /// previous commit: four saying ROOT validation was "disabled", two of
    /// them dating it to 2026-02-14, and four calling E722 and E723 warnings.
    /// The code has passed `Severity::Error` for as long as this repository
    /// has history, which begins at the squashed initial release, so what that
    /// date actually changed is not recoverable here and is not claimed.
    pub(crate) fn validate_monotone_structure(&self, errors: &impl crate::ErrorSink) {
        validate_monotone_gra_structure(self, errors);
    }

    /// Serialize full `%gra` line to an owned string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Write tier content only (relations), without the tier prefix (%gra:\t).
    ///
    /// This is used for roundtrip testing against golden data that contains
    /// content-only, and for the TreeSitterParser API which expects content-only input.
    pub fn write_content<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (i, rel) in self.relations.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            rel.write_chat(w)?;
        }

        Ok(())
    }

    /// Serialize content-only `%gra` payload to an owned string.
    pub fn to_content(&self) -> String {
        let mut s = String::new();
        let _ = self.write_content(&mut s);
        s
    }
}

/// A `%gra` tier established to hold every relation its line declared.
///
/// # What this gates, and what it deliberately does not
///
/// Of the four structural rules, only two describe the WHOLE tier, and only
/// those two are behind this witness:
///
/// - **E721**, indices run 1, 2, ..., N. Dropping any relation but the last
///   breaks that by itself.
/// - **E722**, no relation is a ROOT. Dropping can remove the only one.
///
/// The other two are MONOTONE under dropping, so they are never withheld:
/// dropping a relation cannot create a second ROOT (**E723**) and cannot close
/// a cycle (**E724**), so a violation among the survivors is a violation the
/// author wrote. Withholding those was a real loss, caught in review before
/// this shipped: a tier with a genuine cycle and one unrelated rejected
/// relation reported the rejection and nothing else.
///
/// # The bug this type exists to make unwriteable
///
/// A relation the model cannot hold is rejected and DROPPED, so the tier is
/// shorter than the author wrote it, and E721 and E722 then describe our own
/// recovery. Two mechanisms were meant to prevent that and cancelled each
/// other: the structural rules suppressed themselves when ALIGNMENT had
/// reported E720, E712 or E713, while a `%gra` parse-health taint is precisely
/// what stops alignment running at all. On `spec/errors/E710.md`'s second
/// example tree-sitter tainted, alignment was skipped, nothing suppressed the
/// structural pass, and E722 said "no ROOT relation" about a tier whose one
/// surviving relation is `1|0|ROOT`.
///
/// # Why it reads the TIER and not the utterance's parse health
///
/// The taint bit was the obvious input and it is the wrong one. Tree-sitter
/// sets the `%gra` bit through `taint_all_alignment_dependents` whenever ANY
/// unclassifiable dependent tier on the utterance reports a parse error, so a
/// malformed `%xfoo` line would have withheld every structural rule from a
/// `%gra` tier that parsed perfectly. The bit means "cross-tier alignment
/// involving `%gra` is untrustworthy"; the question here is "did this tier
/// lose a relation", which is [`GraCompleteness`], recorded by the lowering
/// that counted.
pub struct WholeGra<'a> {
    tier: &'a GraTier,
}

impl<'a> WholeGra<'a> {
    /// The only constructor, and the only route to E721 and E722.
    ///
    /// `pub(crate)`, and the public route is
    /// [`Utterance::validate_gra_structure`](crate::model::Utterance). The two
    /// arguments are related only by convention: a tier from one utterance
    /// paired with another utterance's diagnostics type-checks and answers
    /// confidently about neither. The utterance owns both.
    ///
    /// Nothing is reported when the answer is `None`. The fault that caused it
    /// (E710, E713, E720) is itself reported, and on the recovery path E600
    /// already says an alignment was skipped. Saying "and two structural rules
    /// were withheld too" is a real improvement and a separate one: E600's
    /// message would have to name WHICH check it means, and that message is
    /// compared verbatim by the backend-parity harness.
    #[must_use]
    pub(crate) fn of(
        tier: &'a GraTier,
        alignment_diagnostics: &[crate::ParseError],
    ) -> Option<Self> {
        let truncated = match tier.completeness {
            // Only a lowering that COUNTED says `Truncated`, so the
            // suppression is exact. `Unknown` is judged; see
            // [`GraCompleteness`] for why that direction and what it costs.
            GraCompleteness::Truncated => true,
            GraCompleteness::Whole | GraCompleteness::Unknown => false,
        };
        let alignment_disputed = alignment_diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                crate::ErrorCode::MorGraCountMismatch
                    | crate::ErrorCode::GraInvalidWordIndex
                    | crate::ErrorCode::GraInvalidHeadIndex
            )
        });
        match truncated || alignment_disputed {
            true => None,
            false => Some(Self { tier }),
        }
    }

    /// Report the rules that describe the tier as a whole: E721 and E722.
    pub(crate) fn validate_whole_tier(&self, errors: &impl crate::ErrorSink) {
        let relations = self.tier.relations();
        if relations.is_empty() {
            return;
        }
        report_non_sequential_index(relations, self.tier.span, errors);
        report_missing_root(relations, self.tier.span, errors);
    }
}

/// E721: the indices run 1, 2, ..., N.
fn report_non_sequential_index(
    relations: &[GrammaticalRelation],
    span: crate::Span,
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ParseError, Severity};

    for (position, relation) in relations.iter().enumerate() {
        let expected = position + 1;
        if relation.index != expected {
            errors.report(
                ParseError::at_span(
                    ErrorCode::GraNonSequentialIndex,
                    Severity::Error,
                    span,
                    format!(
                        "%gra indices not sequential: expected {expected}, found {}",
                        relation.index
                    ),
                )
                .with_suggestion("Indices must be 1, 2, 3, ..., N"),
            );
            // One is enough: after the first break every later index is
            // reported against an expectation the tier has already left.
            break;
        }
    }
}

/// E722: some relation is the ROOT.
///
/// Suppressed when the graph already has a cycle: the missing root is then a
/// consequence of the cycle, and reporting both is one fault told twice.
fn report_missing_root(
    relations: &[GrammaticalRelation],
    span: crate::Span,
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ParseError, Severity};

    if !roots(relations).is_empty() || has_any_cycle(relations) {
        return;
    }
    errors.report(
        ParseError::at_span(
            ErrorCode::GraNoRoot,
            Severity::Error,
            span,
            "%gra tier has no ROOT relation",
        )
        .with_suggestion("Re-run morphotag to regenerate valid %gra"),
    );
}

/// The rules that hold whatever the lowering dropped: E723 and E724.
///
/// Free functions rather than methods on [`WholeGra`] precisely because they
/// need no witness. See that type for which rule is which and why.
pub(crate) fn validate_monotone_gra_structure(tier: &GraTier, errors: &impl crate::ErrorSink) {
    use crate::{ErrorCode, ParseError, Severity};

    let relations = tier.relations();
    if relations.is_empty() {
        return;
    }

    // E723: more than one ROOT.
    let roots = roots(relations);
    if roots.len() > 1 {
        errors.report(
            ParseError::at_span(
                ErrorCode::GraMultipleRoots,
                Severity::Error,
                tier.span,
                format!("%gra tier has {} ROOT relations, expected 1", roots.len()),
            )
            .with_suggestion("Re-run morphotag to regenerate valid %gra"),
        );
    }

    // E724: a cycle.
    if has_any_cycle(relations) {
        errors.report(
            ParseError::at_span(
                ErrorCode::GraCircularDependency,
                Severity::Error,
                tier.span,
                "%gra tier has circular dependency",
            )
            .with_suggestion("Re-run morphotag to regenerate valid %gra"),
        );
    }
}

/// Every relation that is a ROOT, by POSITION in the tier.
///
/// # The terminator exclusion that used to be here, and why it is gone
///
/// This filtered out a root at `index == relations.len()`, so that a
/// terminator emitted as `N|0|ROOT` would not count as a second root. That is
/// a COUNT standing in for a POSITION, and it cost more than it bought.
///
/// It bought nothing measurable: every `%gra` tier in the reference corpus
/// ends in a `PUNCT` relation, and `is_valid_root_relation` requires the label
/// `ROOT`, so the case it guards against occurs in neither the corpus nor the
/// spec suite. It cost a false E722 on any tier whose genuine root is its last
/// relation, and there is a live instance in this repository's own spec suite:
/// `spec/errors/E604_gra_without_mor.md` example 1 is `1|2|NSUBJ 2|0|ROOT`,
/// with sequential indices, nothing dropped and a clean parse, and chatter
/// reported "%gra tier has no ROOT relation" against it. The observation
/// snapshot recorded that verdict as expected output.
///
/// Rewriting it to exclude the last ELEMENT rather than an index VALUE fixes
/// the arithmetic and not the rule: E604's example has no terminator relation
/// at all, so its root IS the last element and the false positive survives.
/// A terminator's relation is the one aligned with the `%mor` terminator
/// chunk, which this function cannot see and which E604's example does not
/// have. So the guard is deleted rather than repaired, and the day a corpus
/// produces `N|0|ROOT` terminators the cure is a rule that can see `%mor`.
fn roots(relations: &[GrammaticalRelation]) -> Vec<usize> {
    relations
        .iter()
        .enumerate()
        .filter(|(_, relation)| is_valid_root_relation(relation))
        .map(|(position, _)| position)
        .collect()
}

/// Fast O(N) cycle detection using iterative DFS with path tracking.
///
/// Follows each word's head pointer chain to the root using iteration (not recursion).
/// Uses memoization to avoid recomputing paths - each node is visited once.
/// Detects cycles by tracking the current path.
///
/// **Completely stack-safe** - uses heap-allocated Vec for the path instead of
/// call stack recursion. Can handle arbitrarily long chains without stack overflow.
fn has_any_cycle(relations: &[GrammaticalRelation]) -> bool {
    use std::collections::{HashMap, HashSet};

    /// Memoized node state during cycle detection.
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        NoCycle, // Verified no cycle in this subtree
    }

    let mut memo: HashMap<usize, State> = HashMap::new();

    // Check each word for cycles
    for start_rel in relations {
        let start_node = start_rel.index;

        // Skip if we've already verified this node
        if memo.contains_key(&start_node) {
            continue;
        }

        // Follow head chain iteratively with path tracking
        let mut path = HashSet::new();
        let mut current = start_node;

        loop {
            // If this node is already memoized as safe, we're done
            if memo.contains_key(&current) {
                // Mark all nodes in current path as safe
                for &node in &path {
                    memo.insert(node, State::NoCycle);
                }
                break;
            }

            // Cycle detected! Node is in current path
            if path.contains(&current) {
                return true;
            }

            path.insert(current);

            // Find the relation for current node
            if let Some(rel) = relations.iter().find(|r| r.index == current) {
                // Valid roots end a chain. A self-headed non-ROOT relation is a
                // cycle, not a root.
                if rel.head == 0 || is_valid_root_relation(rel) {
                    // Mark all nodes in path as safe
                    for &node in &path {
                        memo.insert(node, State::NoCycle);
                    }
                    break;
                }

                // Continue following the chain
                current = rel.head;
            } else {
                // Invalid index - shouldn't happen, but treat as end of chain
                for &node in &path {
                    memo.insert(node, State::NoCycle);
                }
                break;
            }
        }
    }

    false
}

fn is_valid_root_relation(rel: &GrammaticalRelation) -> bool {
    rel.relation.as_str().eq_ignore_ascii_case("ROOT") && (rel.head == 0 || rel.head == rel.index)
}

impl WriteChat for GraTier {
    /// Serializes one full `%gra` line.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Write tier type prefix
        match self.tier_type {
            GraTierType::Gra => w.write_str("%gra:\t")?,
        }

        // Write space-separated relations
        for (i, rel) in self.relations.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            rel.write_chat(w)?;
        }
        Ok(())
    }
}

/// Ordered list of `%gra` dependency relations.
///
/// # Reference
///
/// - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct GraRelations(pub(crate) Vec<GrammaticalRelation>);

impl GraRelations {
    /// Returns `true` when this relation list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for GraRelations {
    type Target = Vec<GrammaticalRelation>;

    /// Borrows the underlying relation vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<GrammaticalRelation>> for GraRelations {
    /// Wraps an owned relation vector without copying.
    fn from(relations: Vec<GrammaticalRelation>) -> Self {
        Self(relations)
    }
}

impl crate::validation::Validate for GraRelations {
    /// Structural checks run via `validate_gra_structure` with tier-level context.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ErrorCollector, Severity};

    /// Both halves, exactly as `Utterance::validate_gra_structure` runs them
    /// on a tier nothing disputes.
    ///
    /// A test of the RULES states its precondition by going through the
    /// witness rather than around it: `&[]` says no alignment diagnostic
    /// disputes this tier, and the tier's own `GraCompleteness` says whether
    /// anything was dropped.
    fn validate_all(tier: &GraTier, errors: &impl crate::ErrorSink) {
        tier.validate_monotone_structure(errors);
        if let Some(whole) = WholeGra::of(tier, &[]) {
            whole.validate_whole_tier(errors);
        }
    }

    /// A tier whose ROOT is its LAST relation has a root.
    ///
    /// The terminator exclusion used to drop a root at
    /// `index == relations.len()`, so this exact tier, which is
    /// `spec/errors/E604_gra_without_mor.md` example 1, was reported rootless.
    /// The observation snapshot recorded E722 against it as expected output.
    #[test]
    fn a_root_in_the_last_relation_is_still_a_root() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "NSUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert!(
            !errors
                .into_vec()
                .iter()
                .any(|e| e.code == ErrorCode::GraNoRoot),
            "a tier ending in `2|0|ROOT` has a ROOT"
        );
    }

    /// Dropping a relation cannot create a second ROOT, so E723 still fires.
    #[test]
    fn a_truncated_tier_still_reports_two_roots() {
        let tier = GraTier::lowered_from(
            vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 0, "ROOT"),
                GrammaticalRelation::new(3, 2, "PUNCT"),
            ],
            GraCompleteness::Truncated,
        );
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert!(
            errors
                .into_vec()
                .iter()
                .any(|e| e.code == ErrorCode::GraMultipleRoots),
            "two surviving ROOTs are two ROOTs the author wrote"
        );
    }

    /// Dropping a relation cannot close a cycle, so E724 still fires.
    #[test]
    fn a_truncated_tier_still_reports_a_cycle() {
        let tier = GraTier::lowered_from(
            vec![
                GrammaticalRelation::new(1, 2, "NSUBJ"),
                GrammaticalRelation::new(2, 1, "OBJ"),
                GrammaticalRelation::new(3, 2, "PUNCT"),
            ],
            GraCompleteness::Truncated,
        );
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert!(
            errors
                .into_vec()
                .iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency),
            "a cycle among the survivors is a cycle the author wrote"
        );
    }

    /// Dropping a relation DOES break sequentiality and can remove the only
    /// root, so E721 and E722 are withheld on a truncated tier.
    #[test]
    fn a_truncated_tier_withholds_the_whole_tier_rules() {
        // What `%gra: 1|0|ROOT 2|<unrepresentable>|PUNCT` leaves behind, and
        // what `%gra: 0|0|ROOT 1|0|PUNCT` leaves behind: one relation each,
        // shaped so that the whole-tier rules would fire if they ran.
        let rootless = GraTier::lowered_from(
            vec![GrammaticalRelation::new(2, 1, "PUNCT")],
            GraCompleteness::Truncated,
        );
        let errors = ErrorCollector::new();
        validate_all(&rootless, &errors);
        let reported = errors.into_vec();
        assert!(
            !reported.iter().any(|e| e.code == ErrorCode::GraNoRoot),
            "the root may be the relation that was dropped"
        );
        assert!(
            !reported
                .iter()
                .any(|e| e.code == ErrorCode::GraNonSequentialIndex),
            "an index gap is what dropping a relation leaves behind"
        );
    }

    /// The same tier, NOT marked truncated, does report both.
    ///
    /// The pair is the point: without it the test above passes if the rules
    /// stop firing for any reason at all.
    #[test]
    fn the_same_tier_reports_both_when_nothing_was_dropped() {
        let tier = GraTier::lowered_from(
            vec![GrammaticalRelation::new(2, 1, "PUNCT")],
            GraCompleteness::Whole,
        );
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let reported = errors.into_vec();
        assert!(reported.iter().any(|e| e.code == ErrorCode::GraNoRoot));
        assert!(
            reported
                .iter()
                .any(|e| e.code == ErrorCode::GraNonSequentialIndex)
        );
    }

    /// New `%gra` construction preserves relation order and count.
    #[test]
    fn test_gra_tier_new() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "NSUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "OBJ"),
        ]);

        assert_eq!(tier.len(), 3);
        assert!(!tier.is_empty());
        assert_eq!(tier.relations[0].index, 1);
        assert_eq!(tier.relations[1].index, 2);
        assert_eq!(tier.relations[2].index, 3);
    }

    /// Empty relation input reports empty tier state.
    #[test]
    fn test_gra_tier_empty() {
        let tier = GraTier::new_gra(vec![]);
        assert_eq!(tier.len(), 0);
        assert!(tier.is_empty());
    }

    /// A well-formed dependency set emits no structural diagnostics.
    #[test]
    fn test_validate_structure_valid() {
        // TalkBank convention: ROOT head points to self
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "DET"),
            GrammaticalRelation::new(2, 3, "NSUBJ"),
            GrammaticalRelation::new(3, 3, "ROOT"),
            GrammaticalRelation::new(4, 3, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert_eq!(errors.into_vec().len(), 0);
    }

    /// Non-sequential indices trigger `E721`.
    #[test]
    fn test_validate_structure_non_sequential() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 3, "NSUBJ"),
            GrammaticalRelation::new(3, 3, "ROOT"), // gap: expected 2
            GrammaticalRelation::new(2, 3, "OBJ"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();
        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraNonSequentialIndex)
        );
    }

    /// A cycle is E724, at error severity.
    #[test]
    fn a_cycle_is_reported_as_an_error() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "NSUBJ"),
            GrammaticalRelation::new(2, 1, "OBJ"), // Circular: 1→2, 2→1
            GrammaticalRelation::new(3, 2, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();

        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency)
        );
        assert!(errs.iter().all(|e| e.severity == Severity::Error));
    }

    /// Two ROOTs are E723, at error severity.
    #[test]
    fn two_roots_are_reported_as_an_error() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 1, "ROOT"),
            GrammaticalRelation::new(2, 2, "ROOT"),
            GrammaticalRelation::new(3, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();

        assert!(errs.iter().any(|e| e.code == ErrorCode::GraMultipleRoots));
        assert!(errs.iter().all(|e| e.severity == Severity::Error));
    }

    /// `head=0` is accepted as a valid root encoding.
    #[test]
    fn test_validate_structure_root_head_zero_allowed() {
        // UD convention: ROOT head=0 is now allowed (no warning)
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "NSUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "OBJ"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 0); // No errors - head=0 is valid
    }

    /// A self-headed non-ROOT relation is a cycle, not a second ROOT.
    #[test]
    fn test_validate_structure_self_headed_non_root_reports_cycle_not_multiple_roots() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "DEP"),
            GrammaticalRelation::new(3, 3, "NMOD"),
            GrammaticalRelation::new(4, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();

        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency),
            "self-headed non-ROOT relations must be treated as circular dependencies"
        );
        assert!(
            !errs.iter().any(|e| e.code == ErrorCode::GraMultipleRoots),
            "self-headed non-ROOT relations must not be counted as extra ROOTs"
        );
    }

    /// A rootless 2-cycle should report the cycle itself, not cascade into an
    /// additional "no ROOT" diagnostic.
    #[test]
    fn test_validate_structure_rootless_two_cycle_reports_cycle_not_no_root() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "DEP"),
            GrammaticalRelation::new(2, 1, "PARATAXIS"),
            GrammaticalRelation::new(3, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();

        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency),
            "rootless 2-cycles must report E724"
        );
        assert!(
            !errs.iter().any(|e| e.code == ErrorCode::GraNoRoot),
            "rootless 2-cycles must not additionally report E722 when the lack \
             of a root is just a consequence of the cycle"
        );
    }

    /// Empty tiers are accepted by structure validation.
    #[test]
    fn test_validate_structure_empty() {
        let tier = GraTier::new_gra(vec![]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert_eq!(errors.into_vec().len(), 0);
    }
}
