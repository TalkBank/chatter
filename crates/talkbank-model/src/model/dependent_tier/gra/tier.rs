//! `%gra` tier model and structural validation helpers.
//!
//! CHAT reference anchors:
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)

use super::super::WriteChat;
use super::relation::GrammaticalRelation;
use super::tier_type::GraTierType;
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
/// use talkbank_model::Location;
/// use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation};
///
/// // Create a %gra tier
/// let gra = GraTier::new_gra(Location::Unlocated, vec![
///     GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap()),   // Word 1 is subject of word 2
///     GrammaticalRelation::new(2, 0, "ROOT".try_into().unwrap()),   // Word 2 is root
///     GrammaticalRelation::new(3, 2, "OBJ".try_into().unwrap()),    // Word 3 is object of word 2
///     GrammaticalRelation::new(4, 2, "PUNCT".try_into().unwrap()),  // Terminator
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
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
}

impl GraTier {
    /// Constructs a grammatical-relations tier from parsed relations.
    ///
    /// This builder does not certify source origin or validity. Parsers record
    /// observed outcomes with [`super::GraLowering`], which rejects the entire
    /// construction if any relation was rejected.
    pub fn new(
        location: impl Into<crate::Location>,
        tier_type: GraTierType,
        relations: Vec<GrammaticalRelation>,
    ) -> Self {
        Self {
            tier_type,
            relations: relations.into(),
            location: location.into(),
        }
    }

    /// Convenience constructor for standard `%gra`.
    pub fn new_gra(
        location: impl Into<crate::Location>,
        relations: Vec<GrammaticalRelation>,
    ) -> Self {
        Self::new(location, GraTierType::Gra, relations)
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
    /// This is the raw model, so mutations may leave invalid indices or heads.
    /// Shared validation analyzes the resulting tier. An existing structure
    /// analysis holds an immutable borrow and prevents concurrent mutation.
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

    /// Serialize full `%gra` line to an owned string.
    pub fn to_chat(&self) -> String {
        crate::model::ChatString::render(|w| self.write_chat(w))
    }

    /// Write tier content only (relations), without the tier prefix (%gra:\t).
    ///
    /// This is used for roundtrip testing against golden data that contains
    /// content-only, and for the TreeSitterParser API which expects content-only input.
    pub fn write_content<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
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
        crate::model::ChatString::render(|w| self.write_content(w))
    }
}

impl WriteChat for GraTier {
    /// Serializes one full `%gra` line.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
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
    use super::super::GraLowering;
    use super::*;
    use crate::{ErrorCode, ErrorCollector, ParseOutcome, Severity};

    /// Repeated indices are E721, but every authored dependency edge still
    /// contributes cycle evidence. Their order must not select a parent.
    #[test]
    fn duplicate_indices_do_not_hide_a_cycle_behind_the_first_relation() {
        for edges in [
            [(1, 0, "ROOT"), (1, 2, "DEP"), (2, 1, "DEP")],
            [(1, 2, "DEP"), (1, 0, "ROOT"), (2, 1, "DEP")],
        ] {
            let tier = GraTier::new_gra(
                crate::Location::Unlocated,
                edges
                    .into_iter()
                    .map(|(word, head, label)| {
                        GrammaticalRelation::new(word, head, label.try_into().unwrap())
                    })
                    .collect(),
            );
            let errors = ErrorCollector::new();
            validate_all(&tier, &errors);
            let codes = errors
                .into_vec()
                .into_iter()
                .map(|error| error.code)
                .collect::<Vec<_>>();
            assert!(codes.contains(&ErrorCode::GraNonSequentialIndex));
            assert!(
                codes.contains(&ErrorCode::GraCircularDependency),
                "{edges:?}: {codes:?}"
            );
        }
    }

    /// Both halves, exactly as `Utterance::validate_gra_structure` runs them
    /// on a tier nothing disputes.
    ///
    /// A test of the RULES states its precondition by going through the
    /// witness rather than around it: `&[]` says no alignment diagnostic
    /// disputes this tier. Rejected lowering cannot supply a tier here.
    fn validate_all(tier: &GraTier, errors: &impl crate::ErrorSink) {
        let structure = super::super::structure::GraStructure::analyze(tier);
        structure.report_monotone(errors);
        if let Some(whole) = structure.into_whole(&[]) {
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(2, 0, "ROOT".try_into().unwrap()),
            ],
        );
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

    /// A fully lowered graph with two roots reports E723.
    #[test]
    fn a_lowered_tier_reports_two_roots() {
        let tier = [
            ParseOutcome::Parsed(GrammaticalRelation::new(1, 0, "ROOT".try_into().unwrap())),
            ParseOutcome::Parsed(GrammaticalRelation::new(2, 0, "ROOT".try_into().unwrap())),
            ParseOutcome::Parsed(GrammaticalRelation::new(3, 2, "PUNCT".try_into().unwrap())),
        ]
        .into_iter()
        .collect::<GraLowering>()
        .finish(crate::Location::Unlocated)
        .into_option()
        .expect("all relations lowered");
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

    /// A fully lowered graph with a cycle reports E724.
    #[test]
    fn a_lowered_tier_reports_a_cycle() {
        let tier = [
            ParseOutcome::Parsed(GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap())),
            ParseOutcome::Parsed(GrammaticalRelation::new(2, 1, "OBJ".try_into().unwrap())),
            ParseOutcome::Parsed(GrammaticalRelation::new(3, 2, "PUNCT".try_into().unwrap())),
        ]
        .into_iter()
        .collect::<GraLowering>()
        .finish(crate::Location::Unlocated)
        .into_option()
        .expect("all relations lowered");
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

    /// Rejection before, between or after accepted relations cannot produce a graph.
    #[test]
    fn rejected_lowering_never_constructs_a_partial_tier() {
        for rejected_at in 0..3 {
            let mut lowering = GraLowering::with_capacity(3);
            for index in 0..3 {
                lowering.record(if index == rejected_at {
                    ParseOutcome::Rejected
                } else {
                    ParseOutcome::Parsed(GrammaticalRelation::new(
                        index + 1,
                        0,
                        "ROOT".try_into().unwrap(),
                    ))
                });
            }
            assert!(matches!(
                lowering.finish(crate::Location::Unlocated),
                ParseOutcome::Rejected
            ));
        }
    }

    /// Successful lowering still admits graph-invalid data for validation.
    /// Construction success is not a certificate of CHAT validity.
    #[test]
    fn the_same_tier_reports_both_when_nothing_was_dropped() {
        let tier = [ParseOutcome::Parsed(GrammaticalRelation::new(
            2,
            1,
            "PUNCT".try_into().unwrap(),
        ))]
        .into_iter()
        .collect::<GraLowering>()
        .finish(crate::Location::Unlocated)
        .into_option()
        .expect("all relations lowered");
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(2, 0, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(3, 2, "OBJ".try_into().unwrap()),
            ],
        );

        assert_eq!(tier.len(), 3);
        assert!(!tier.is_empty());
        assert_eq!(tier.relations[0].index, 1);
        assert_eq!(tier.relations[1].index, 2);
        assert_eq!(tier.relations[2].index, 3);
    }

    /// Empty relation input reports empty tier state.
    #[test]
    fn test_gra_tier_empty() {
        let tier = GraTier::new_gra(crate::Location::Unlocated, vec![]);
        assert_eq!(tier.len(), 0);
        assert!(tier.is_empty());
    }

    /// A well-formed dependency set emits no structural diagnostics.
    #[test]
    fn test_validate_structure_valid() {
        // TalkBank convention: ROOT head points to self
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "DET".try_into().unwrap()),
                GrammaticalRelation::new(2, 3, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(3, 3, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(4, 3, "PUNCT".try_into().unwrap()),
            ],
        );
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert_eq!(errors.into_vec().len(), 0);
    }

    /// Non-sequential indices trigger `E721`.
    #[test]
    fn test_validate_structure_non_sequential() {
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 3, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(3, 3, "ROOT".try_into().unwrap()), // gap: expected 2
                GrammaticalRelation::new(2, 3, "OBJ".try_into().unwrap()),
            ],
        );
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(2, 1, "OBJ".try_into().unwrap()), // Circular: 1→2, 2→1
                GrammaticalRelation::new(3, 2, "PUNCT".try_into().unwrap()),
            ],
        );
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 1, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(2, 2, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(3, 1, "PUNCT".try_into().unwrap()),
            ],
        );
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "NSUBJ".try_into().unwrap()),
                GrammaticalRelation::new(2, 0, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(3, 2, "OBJ".try_into().unwrap()),
            ],
        );
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 0); // No errors - head=0 is valid
    }

    /// A self-headed non-ROOT relation is a cycle, not a second ROOT.
    #[test]
    fn test_validate_structure_self_headed_non_root_reports_cycle_not_multiple_roots() {
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 0, "ROOT".try_into().unwrap()),
                GrammaticalRelation::new(2, 1, "DEP".try_into().unwrap()),
                GrammaticalRelation::new(3, 3, "NMOD".try_into().unwrap()),
                GrammaticalRelation::new(4, 1, "PUNCT".try_into().unwrap()),
            ],
        );
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
        let tier = GraTier::new_gra(
            crate::Location::Unlocated,
            vec![
                GrammaticalRelation::new(1, 2, "DEP".try_into().unwrap()),
                GrammaticalRelation::new(2, 1, "PARATAXIS".try_into().unwrap()),
                GrammaticalRelation::new(3, 1, "PUNCT".try_into().unwrap()),
            ],
        );
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
        let tier = GraTier::new_gra(crate::Location::Unlocated, vec![]);
        let errors = ErrorCollector::new();
        validate_all(&tier, &errors);
        assert_eq!(errors.into_vec().len(), 0);
    }
}
