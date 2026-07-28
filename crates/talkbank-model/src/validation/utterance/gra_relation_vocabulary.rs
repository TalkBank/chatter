//! Closed-vocabulary validation for `%gra` relation labels.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//! - <https://universaldependencies.org/u/dep/>
//!
//! # What is checked, and what deliberately is not
//!
//! A `%gra` relation label is `HEAD` or `HEAD-SUBTYPE`. Universal
//! Dependencies fixes the HEAD set at 37 universal relations and
//! deliberately defines SUBTYPES as language-specific and open-ended.
//! So the head is the only part of a label that can be checked against a
//! closed vocabulary, and subtypes are never checked. Checking a whole
//! label against a closed list is the over-strict implementation that
//! would reject the 113 legitimate subtyped labels the corpora use.
//!
//! # Why this rule exists
//!
//! Nothing validated relation labels before, in chatter or in CLAN CHECK,
//! so a corrupted label rode silently into every downstream analysis. The
//! motivating case was a real one: `13|3|PUNCTT`, a hand-edit typo for
//! `PUNCT` that both validators passed.
//!
//! # Grounding
//!
//! Full-corpus survey of the TalkBank corpora, 2026-07-26: 106,158 files,
//! 70,802 of them carrying a `%gra` tier, 138,565,864 relation instances,
//! 150 distinct labels, 40 distinct heads. All 37 universal heads are attested. Exactly three
//! heads fall outside the set: `IOB` (146 instances, a truncation of
//! `IOBJ`), `PAD` (5), `PUNCTT` (1). No legacy TalkBank label (`SUBJ`,
//! `JCT`, `COORD`, `INCROOT`) survives anywhere in the corpora, which is
//! what licenses treating the universal set as closed rather than as a
//! recommendation.

use crate::model::dependent_tier::DependentTier;
use crate::model::{GrammaticalRelation, Utterance};
use crate::{ErrorCode, ErrorSink, ParseError, Severity};

/// Separates a Universal Dependencies head from its language-specific subtype.
///
/// Functional data rather than prose punctuation: this is the ASCII hyphen a
/// CHAT `%gra` label literally contains, so it is written as an escape.
const SUBTYPE_SEPARATOR: char = '\u{2D}';

/// The 37 Universal Dependencies universal relations, upper-cased.
///
/// Upper case is the case TalkBank `%gra` tiers are transcribed in;
/// membership is tested case-insensitively anyway, so a lower-case corpus
/// would be accepted rather than flagged wholesale.
///
/// Source: <https://universaldependencies.org/u/dep/>. Ordered as UD's own
/// table orders them (core arguments, non-core dependents, nominal
/// dependents, then coordination and the special relations) so the list can
/// be diffed against that page by eye.
const UNIVERSAL_RELATIONS: [&str; 37] = [
    // Core arguments
    "NSUBJ",
    "OBJ",
    "IOBJ",
    "CSUBJ",
    "CCOMP",
    "XCOMP",
    // Non-core dependents
    "OBL",
    "VOCATIVE",
    "EXPL",
    "DISLOCATED",
    "ADVCL",
    "ADVMOD",
    "DISCOURSE",
    "AUX",
    "COP",
    "MARK",
    // Nominal dependents
    "NMOD",
    "APPOS",
    "NUMMOD",
    "ACL",
    "AMOD",
    "DET",
    "CLF",
    "CASE",
    // Coordination, multiword expressions, headless, loose, special, other
    "CONJ",
    "CC",
    "FIXED",
    "FLAT",
    "COMPOUND",
    "LIST",
    "PARATAXIS",
    "ORPHAN",
    "GOESWITH",
    "REPARANDUM",
    "PUNCT",
    "ROOT",
    "DEP",
];

/// Split a relation label into its head and optional subtype.
///
/// The split is at the FIRST separator only. A subtype may itself contain the
/// separator, and splitting at every occurrence then checking each part is
/// precisely the over-strict reading this rule must avoid.
fn split_label(label: &str) -> (&str, Option<&str>) {
    match label.split_once(SUBTYPE_SEPARATOR) {
        Some((head, subtype)) => (head, Some(subtype)),
        None => (label, None),
    }
}

/// Whether a head names one of the 37 UD universal relations.
fn head_is_universal(head: &str) -> bool {
    UNIVERSAL_RELATIONS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(head))
}

/// Report `E761` for every `%gra` relation whose head is not a UD universal.
///
/// Runs per relation rather than stopping at the first offender: a tier
/// produced by a broken tagger can carry several distinct bad labels, and
/// each one is a separate thing to fix in the data.
///
/// Deliberately independent of `%gra` structural validation
/// (`validate_gra_structure`) and of `%mor`-to-`%gra` alignment. A label's
/// legality does not depend on the tier's tree shape or on its cardinality
/// agreeing with `%mor`, so this check is not suppressed when those fail;
/// suppressing it would hide the label defect behind an unrelated one.
pub(crate) fn check_gra_relation_vocabulary(utterance: &Utterance, errors: &impl ErrorSink) {
    // A parse-tainted `%gra` tier may hold recovery output rather than
    // authored labels, so its contents are not evidence about the source
    // text. The root parse failure is reported elsewhere.
    if utterance
        .parse_health
        .is_tier_tainted(crate::model::ParseHealthTier::Gra)
    {
        return;
    }

    for entry in &utterance.dependent_tiers {
        let DependentTier::Gra(tier) = &entry.tier else {
            continue;
        };
        for relation in tier.relations() {
            report_if_not_universal(relation, tier.span, errors);
        }
    }
}

/// Emit the diagnostic for one relation, when its head is unknown.
fn report_if_not_universal(
    relation: &GrammaticalRelation,
    span: crate::Span,
    errors: &impl ErrorSink,
) {
    let label = relation.relation.as_str();
    let (head, subtype) = split_label(label);
    if head_is_universal(head) {
        return;
    }

    // Name the head separately from the label only when they differ, so the
    // message for a bare label does not repeat itself.
    let what = match subtype {
        Some(_) => format!("\"{label}\" (head \"{head}\")"),
        None => format!("\"{label}\""),
    };

    // The span covers the whole tier, so the message carries the relation's
    // own triple. A tier can hold dozens of relations and a corpus editor
    // fixing these by hand needs to know WHICH one, not just that one of them
    // is wrong.
    let triple = format!("{}|{}|{}", relation.index, relation.head, label);

    errors.report(
        ParseError::at_span(
            ErrorCode::GraRelationHeadNotUniversal,
            Severity::Error,
            span,
            format!(
                "%gra relation {triple}: {what} is not a Universal Dependencies \
                 relation, \"{head}\" is not one of the 37 universal relations"
            ),
        )
        .with_suggestion(
            "Use a UD universal relation as the head, optionally followed by a \
             language-specific subtype (for example NMOD-POSS). Common typos: \
             IOB for IOBJ, PUNCTT for PUNCT.",
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UD fixes the universal set at 37; a miscount silently changes the rule.
    #[test]
    fn universal_set_holds_exactly_thirty_seven_distinct_relations() {
        let mut sorted = UNIVERSAL_RELATIONS;
        sorted.sort_unstable();
        let distinct = sorted.len() - sorted.windows(2).filter(|pair| pair[0] == pair[1]).count();
        assert_eq!(
            distinct, 37,
            "the universal set must hold 37 distinct heads"
        );
    }

    /// `HEAD-SUBTYPE` splits into its two documented parts.
    #[test]
    fn split_label_separates_head_from_subtype() {
        assert_eq!(split_label("NMOD-POSS"), ("NMOD", Some("POSS")));
    }

    /// A label with no separator is all head, and carries no subtype.
    #[test]
    fn split_label_leaves_a_bare_head_without_a_subtype() {
        assert_eq!(split_label("ROOT"), ("ROOT", None));
    }

    /// A multi-part subtype stays whole: the split is at the first separator.
    #[test]
    fn split_label_splits_only_at_the_first_separator() {
        assert_eq!(split_label("ACL-RELCL-EXTRA"), ("ACL", Some("RELCL-EXTRA")));
    }

    /// Membership is case-insensitive, so a lower-case corpus is accepted.
    #[test]
    fn head_membership_ignores_case() {
        assert!(head_is_universal("nsubj"));
        assert!(head_is_universal("NSUBJ"));
    }

    /// The three heads the corpora actually get wrong are all rejected.
    #[test]
    fn the_attested_defective_heads_are_not_universal() {
        assert!(!head_is_universal("IOB"));
        assert!(!head_is_universal("PAD"));
        assert!(!head_is_universal("PUNCTT"));
    }

    /// Retired TalkBank labels are rejected: they occur nowhere in the corpora.
    #[test]
    fn retired_talkbank_labels_are_not_universal() {
        for retired in ["SUBJ", "JCT", "COORD", "INCROOT", "POBJ", "MOD"] {
            assert!(
                !head_is_universal(retired),
                "{retired} is a retired label and must not pass"
            );
        }
    }
}
