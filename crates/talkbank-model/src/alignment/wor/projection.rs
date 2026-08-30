//! Typed main-tier projection for the current `%wor` policy.

use super::{
    CountMatchedWorTimings, MainWorSlotCount, MissingWorTimings, WorTierSlotCount,
    WorTimingBinding, WorTimingDrift,
};
use crate::alignment::helpers::{
    TierDomain, WordItem, counts_for_tier, is_tag_marker_separator, walk_words,
};
use crate::model::dependent_tier::WorItem;
use crate::model::{MainTier, Separator, WorTier, Word, WriteChat};
use schemars::JsonSchema;

/// Main-tier membership rule used by a `%wor` timing binding.
///
/// The policy is explicit because a slot-count claim is meaningless without
/// saying which main-tier items were eligible. Chatter currently supports one
/// canonical policy. Future policies must be added as named variants and
/// evaluated explicitly rather than silently changing this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorSlotMembershipPolicy {
    /// Version 1 of the filtered lexical policy implemented by
    /// [`TierDomain::Wor`]. This variant's membership must never change.
    FilteredLexicalV1,
}

/// Main-tier content projected through one named `%wor` membership policy.
///
/// This capability is the single owner of current `%wor` selection. Both
/// [`MainTier::generate_wor_tier`] and [`super::bind_wor_timing`] travel through
/// it, so generation and timing recovery cannot silently use different
/// walkers. Construction is available only from a typed [`MainTier`].
#[derive(Debug)]
#[must_use = "a `%wor` projection must be generated, inspected, or bound"]
pub struct WorMainTierProjection<'main> {
    main: &'main MainTier,
    policy: WorSlotMembershipPolicy,
    items: Vec<WorMainTierProjectionItem<'main>>,
}

#[derive(Clone, Copy, Debug)]
enum WorMainTierProjectionItem<'main> {
    Slot(&'main Word),
    Separator(&'main Separator),
}

impl<'main> WorMainTierProjection<'main> {
    pub(crate) fn from_main(main: &'main MainTier) -> Self {
        let policy = WorSlotMembershipPolicy::FilteredLexicalV1;
        let mut items = Vec::new();
        walk_words(
            &main.content.content,
            Some(TierDomain::Wor),
            &mut |item| match item {
                WordItem::Word(word) => {
                    if counts_for_tier(word, TierDomain::Wor) {
                        items.push(WorMainTierProjectionItem::Slot(word));
                    }
                }
                WordItem::ReplacedWord(replaced) => {
                    if counts_for_tier(&replaced.word, TierDomain::Wor) {
                        items.push(WorMainTierProjectionItem::Slot(&replaced.word));
                    }
                }
                WordItem::Separator(separator) => {
                    if is_tag_marker_separator(separator) {
                        items.push(WorMainTierProjectionItem::Separator(separator));
                    }
                }
            },
        );
        Self {
            main,
            policy,
            items,
        }
    }

    /// Membership policy that produced this projection.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Main-tier word slots selected by this projection.
    pub fn slots(&self) -> impl Iterator<Item = &'main Word> + '_ {
        self.items.iter().filter_map(|item| match item {
            WorMainTierProjectionItem::Slot(word) => Some(*word),
            WorMainTierProjectionItem::Separator(_) => None,
        })
    }

    /// Number of selected main-tier word slots.
    pub fn slot_count(&self) -> MainWorSlotCount {
        MainWorSlotCount(self.slots().count())
    }

    /// Derive a serializable `%wor` tier from this exact projection.
    ///
    /// The visible word text is display material. Lexical identity remains on
    /// the borrowed main-tier word and the inline bullet carries timing.
    pub fn generate_tier(&self) -> WorTier {
        let items = self
            .items
            .iter()
            .map(|item| match item {
                WorMainTierProjectionItem::Slot(word) => {
                    WorItem::Word(Box::new(wor_display_word_from_main(word)))
                }
                WorMainTierProjectionItem::Separator(separator) => WorItem::Separator {
                    text: separator.to_chat_string(),
                    span: separator.span(),
                },
            })
            .collect();

        WorTier {
            language_code: self.main.content.language_code.clone(),
            items,
            terminator: self.main.content.terminator.clone(),
            span: crate::Span::DUMMY,
        }
    }

    /// Bind a parsed `%wor` timing sidecar to this exact main-tier projection.
    ///
    /// Consuming the projection preserves the relationship between the policy,
    /// selected lexical slots, and the resulting binding state. Use
    /// [`super::bind_wor_timing`] when the projection itself is not otherwise
    /// needed.
    pub fn bind_timing(self, wor: Option<&'main WorTier>) -> WorTimingBinding<'main> {
        let main_count = self.slot_count();
        let policy = self.policy;

        let Some(wor) = wor else {
            return WorTimingBinding::Missing(MissingWorTimings { policy, main_count });
        };

        let wor_count = WorTierSlotCount(wor.word_count());
        if main_count.get() != wor_count.get() {
            return WorTimingBinding::Drifted(WorTimingDrift {
                policy,
                main_count,
                wor_count,
            });
        }

        let main_slots = self
            .items
            .into_iter()
            .filter_map(|item| match item {
                WorMainTierProjectionItem::Slot(word) => Some(word),
                WorMainTierProjectionItem::Separator(_) => None,
            })
            .collect();
        let wor_slots = wor.words().collect();

        WorTimingBinding::CountMatched(CountMatchedWorTimings {
            policy,
            main_slots,
            wor_slots,
        })
    }
}

fn wor_display_word_from_main(word: &Word) -> Word {
    let cleaned = canonical_wor_display_text(word);
    let mut display_word = Word::new_unchecked(cleaned, cleaned);
    display_word.inline_bullet.clone_from(&word.inline_bullet);
    display_word
}

pub(super) fn canonical_wor_display_text(word: &Word) -> &str {
    word.cleaned_text()
}
