//! Generic dependent-tier replacement helpers.
//!
//! When a transform regenerates a dependent tier (`%mor`, `%gra`, `%wor`, or a
//! user-defined `%x...` tier), it must replace the existing tier of the same
//! variant in place rather than append a duplicate. [`replace_or_add_tier`]
//! centralizes that "upsert" so callers do not reimplement the variant-matching
//! logic (user-defined tiers are matched on their label).

use smallvec::SmallVec;
use talkbank_model::model::{DependentTier, DependentTierEntry};

/// Replace an existing tier of the same variant or append a new one.
///
/// Takes the utterance's own `SmallVec<[DependentTierEntry; 3]>`, so this can
/// be called directly on `utterance.dependent_tiers`. Callers still pass a
/// plain [`DependentTier`]: the separator is source provenance, not something
/// a transform should have to invent.
///
/// **Separator handling.** On REPLACE the existing entry's [`TierSeparator`]
/// is preserved, because only the tier's payload is being regenerated: the
/// line's source spacing is unchanged, and discarding it would erase the
/// provenance E758 is detected from. On APPEND there is no source line, so the
/// new entry gets `TierSeparator::CLEAN`. Serialization canonicalizes to a
/// single tab in both cases, so this choice affects diagnostics, not output.
///
/// [`TierSeparator`]: talkbank_model::model::TierSeparator
pub fn replace_or_add_tier(tiers: &mut SmallVec<[DependentTierEntry; 3]>, new_tier: DependentTier) {
    let variant_matches = |existing: &DependentTier, new: &DependentTier| -> bool {
        match (existing, new) {
            (DependentTier::Mor(_), DependentTier::Mor(_)) => true,
            (DependentTier::Gra(_), DependentTier::Gra(_)) => true,
            (DependentTier::Wor(_), DependentTier::Wor(_)) => true,
            (DependentTier::UserDefined(a), DependentTier::UserDefined(b)) => a.label == b.label,
            _ => false,
        }
    };

    for entry in tiers.iter_mut() {
        if variant_matches(&entry.tier, &new_tier) {
            entry.tier = new_tier;
            return;
        }
    }
    tiers.push(DependentTierEntry::new(new_tier));
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::{NonEmptyString, UserDefinedDependentTier, WorTier};

    #[test]
    fn replace_or_add_tier_user_defined_matches_by_label() {
        let mut tiers = smallvec::smallvec![];

        let xtra1 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: Some(NonEmptyString::new("first").unwrap()),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra1);
        assert_eq!(tiers.len(), 1);

        let xtra2 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: Some(NonEmptyString::new("second").unwrap()),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra2);
        assert_eq!(tiers.len(), 1);

        let DependentTier::UserDefined(ud) = &tiers[0].tier else {
            panic!("expected UserDefined tier");
        };
        assert_eq!(ud.content.as_deref(), Some("second"));

        let xcod = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xcod").unwrap(),
            content: Some(NonEmptyString::new("code").unwrap()),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xcod);
        assert_eq!(tiers.len(), 2);
    }

    #[test]
    fn replace_or_add_tier_replaces_existing_wor() {
        let mut tiers = smallvec::smallvec![DependentTierEntry::new(DependentTier::Wor(
            WorTier::default()
        ))];
        let replacement = DependentTier::Wor(WorTier::from_words(vec![
            talkbank_model::model::Word::simple("hello"),
        ]));

        replace_or_add_tier(&mut tiers, replacement);

        assert_eq!(tiers.len(), 1);
        let DependentTier::Wor(wor) = &tiers[0].tier else {
            panic!("expected %wor tier");
        };
        assert_eq!(wor.words().count(), 1);
    }

    /// Compile-time proof that the helper accepts an utterance's OWN storage.
    ///
    /// This is the regression that matters. `DependentTierEntry` landed on
    /// 2026-07-22 and `Utterance::dependent_tiers` became
    /// `SmallVec<[DependentTierEntry; 3]>`, but this helper kept taking
    /// `SmallVec<[DependentTier; 3]>` and so could no longer be called on any
    /// real utterance. It still compiled, because it was internally consistent
    /// and has no callers inside this repository; the only consumer is the
    /// downstream ML pipeline, which was pinned to an older tag and therefore
    /// never exercised it. Shipped unusable in v0.3.6 and v0.4.0.
    ///
    /// Deliberately a compile-time guard rather than a runtime assertion: the
    /// defect was a TYPE mismatch, so the check that catches it is one that
    /// fails to build. If `Utterance::dependent_tiers` and this signature ever
    /// drift apart again, this function stops compiling.
    #[allow(dead_code)]
    fn accepts_an_utterances_own_tier_storage(utterance: &mut talkbank_model::Utterance) {
        replace_or_add_tier(
            &mut utterance.dependent_tiers,
            DependentTier::Wor(WorTier::default()),
        );
    }

    /// Replacing a tier keeps the source line's separator provenance.
    #[test]
    fn replace_preserves_the_existing_separator_and_append_is_clean() {
        use talkbank_model::Span;
        use talkbank_model::model::TierSeparator;

        let dirty = TierSeparator::with_trailing_space(Span::new(3, 5));
        let mut tiers = smallvec::smallvec![DependentTierEntry::with_separator(
            DependentTier::Wor(WorTier::default()),
            dirty,
        )];

        replace_or_add_tier(&mut tiers, DependentTier::Wor(WorTier::default()));
        assert_eq!(
            tiers[0].separator, dirty,
            "replacing a tier's payload must not discard its source spacing"
        );

        let appended = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: Some(NonEmptyString::new("new").unwrap()),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, appended);
        assert_eq!(
            tiers[1].separator,
            TierSeparator::CLEAN,
            "an appended tier has no source line, so it is CLEAN"
        );
    }
}
