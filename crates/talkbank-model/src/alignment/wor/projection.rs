//! Typed main-tier projection for the current `%wor` policy.

use super::{
    CountMatchedWorTimings, MainWorSlotCount, MissingWorTimings, WorTierSlotCount,
    WorTimingBinding, WorTimingDrift,
};
use crate::alignment::helpers::{
    TierDomain, WordItem, admit_text_for_tier, is_tag_marker_separator, walk_words,
};
use crate::model::dependent_tier::WorItem;
use crate::model::{MainTier, Separator, UtteranceContent, WorTier, Word, WriteChat};
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

impl WorSlotMembershipPolicy {
    /// Whether one main-tier word is a `%wor` slot under this policy.
    ///
    /// The ONE owner of `%wor` membership, and public on purpose.
    /// `WorMainTierProjection::from_main` admits its slots through it, and
    /// a consumer that needs the answer for a single word asks
    /// here instead of restating `counts_for_tier(word, TierDomain::Wor)`
    /// beside its own walk. Whole-main-tier consumers use the projection,
    /// which also retains the selected words' producing root content; split
    /// consumers no longer reconstruct membership beside an independent walk.
    /// A replaced word is admitted by its ORIGINAL, as the projection admits it.
    pub fn admits(self, word: &Word) -> bool {
        self.admit_text(word).is_some()
    }

    fn admit_text(self, word: &Word) -> Option<crate::model::NonEmptyStr<'_>> {
        match self {
            WorSlotMembershipPolicy::FilteredLexicalV1 => {
                admit_text_for_tier(word, TierDomain::Wor)
            }
        }
    }
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

/// The producing top-level content of one canonical `%wor` word slot.
///
/// Only projection constructs this association. The index belongs to the
/// borrowed main tier, not a caller-supplied second model. Nested words share
/// their indivisible root's position; replacements retain their original word.
/// No timing or whole-document validity is claimed by this capability.
/// Equality means the same producing main-tier object and root position,
/// not equal CHAT content in a different model. Comparison and debug output
/// do not traverse the producing main tier.
///
/// A caller cannot invent a position or substitute its producing main tier:
///
/// ```compile_fail,E0451
/// use talkbank_model::alignment::WorSourceContent;
/// use talkbank_model::model::MainTier;
/// fn forge(main: &MainTier) -> WorSourceContent<'_> {
///     WorSourceContent { main, index: 0 }
/// }
/// ```
///
/// Its borrow also excludes changing the source before the location is used:
///
/// ```compile_fail,E0506
/// use talkbank_model::model::MainTier;
/// fn replace(main: &mut MainTier) {
///     let projection = main.wor_projection();
///     if let Some(location) = projection.source_content().next() {
///         main.content.content = Vec::new().into();
///         let _ = location.content();
///     }
/// }
/// ```
#[derive(Clone, Copy)]
pub struct WorSourceContent<'main> {
    main: &'main MainTier,
    index: usize,
}

impl PartialEq for WorSourceContent<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.main, other.main) && self.index == other.index
    }
}

impl Eq for WorSourceContent<'_> {}

impl std::fmt::Debug for WorSourceContent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorSourceContent")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<'main> WorSourceContent<'main> {
    /// The exact main tier from whose projection this location was issued.
    pub fn source(self) -> &'main MainTier {
        self.main
    }

    /// Position within the producing main tier's top-level content.
    pub fn index(self) -> usize {
        self.index
    }

    /// The borrowed root content item, including its surrounding annotations.
    pub fn content(self) -> &'main UtteranceContent {
        &self.main.content.content[self.index]
    }
}

/// A selected lexical word retains the root association issued by its producer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ProjectedWorWord<'main> {
    word: &'main Word,
    display_text: crate::model::NonEmptyStr<'main>,
    source_content: WorSourceContent<'main>,
}

impl<'main> ProjectedWorWord<'main> {
    pub(super) fn display_text(self) -> crate::model::NonEmptyStr<'main> {
        self.display_text
    }

    pub(super) fn word(self) -> &'main Word {
        self.word
    }

    pub(super) fn source_content(self) -> WorSourceContent<'main> {
        self.source_content
    }
}

#[derive(Clone, Copy, Debug)]
enum WorMainTierProjectionItem<'main> {
    Slot(ProjectedWorWord<'main>),
    Separator(&'main Separator),
}

impl<'main> WorMainTierProjection<'main> {
    pub(crate) fn from_main(main: &'main MainTier) -> Self {
        let policy = WorSlotMembershipPolicy::FilteredLexicalV1;
        let mut items = Vec::new();
        for (index, root) in main.content.content.iter().enumerate() {
            let source_content = WorSourceContent { main, index };
            walk_words(
                std::slice::from_ref(root),
                Some(TierDomain::Wor),
                &mut |item| match item {
                    WordItem::Word(word) => {
                        if let Some(display_text) = policy.admit_text(word) {
                            items.push(WorMainTierProjectionItem::Slot(ProjectedWorWord {
                                word,
                                display_text,
                                source_content,
                            }));
                        }
                    }
                    WordItem::ReplacedWord(replaced) => {
                        if let Some(display_text) = policy.admit_text(&replaced.word) {
                            items.push(WorMainTierProjectionItem::Slot(ProjectedWorWord {
                                word: &replaced.word,
                                display_text,
                                source_content,
                            }));
                        }
                    }
                    WordItem::Separator(separator) => {
                        if is_tag_marker_separator(separator) {
                            items.push(WorMainTierProjectionItem::Separator(separator));
                        }
                    }
                },
            );
        }
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

    /// Main-tier word slots selected by this projection. Crate-private on
    /// purpose: a consumer outside the model that wants the slots wants to
    /// pair them with a `%wor` tier, and that pairing is `bind_timing`'s
    /// (a hand zip of these slots was one of three reviewed drafts of the
    /// sanitizer on 2026-09-08).
    pub(crate) fn slots(&self) -> impl Iterator<Item = &'main Word> + '_ {
        self.items.iter().filter_map(|item| match item {
            WorMainTierProjectionItem::Slot(word) => Some(word.word()),
            WorMainTierProjectionItem::Separator(_) => None,
        })
    }

    /// Producing root content for every selected word, in canonical slot order.
    ///
    /// This exposes content locations, not uncorroborated timing or an
    /// independently pairable lexical word sequence. No second traversal is
    /// needed to assign the selected words to structural groups.
    pub fn source_content(&self) -> impl Iterator<Item = WorSourceContent<'main>> + '_ {
        self.items.iter().filter_map(|item| match item {
            WorMainTierProjectionItem::Slot(word) => Some(word.source_content()),
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
                    WorItem::Word(Box::new(wor_display_word_from_main(*word)))
                }
                WorMainTierProjectionItem::Separator(separator) => WorItem::Separator {
                    text: separator.to_chat_string(),
                    location: separator.location(),
                },
            })
            .collect();

        WorTier {
            language_code: self
                .main
                .content
                .language_code
                .as_ref()
                .map(|precode| precode.code.clone()),
            items,
            terminator: self.main.content.terminator.clone(),
            // A projection is generated from the main tier, not read from source.
            location: crate::Location::Unlocated,
        }
    }

    /// Bind a parsed `%wor` timing sidecar to this exact main-tier projection.
    ///
    /// Consuming the projection preserves the relationship between the policy,
    /// selected lexical slots, and the resulting binding state. Use
    /// [`super::bind_wor_timing`] when the projection itself is not otherwise
    /// needed.
    pub fn bind_timing(self, wor: Option<&'main WorTier>) -> WorTimingBinding<'main> {
        let Some(wor) = wor else {
            return WorTimingBinding::Missing(MissingWorTimings {
                policy: self.policy,
                main_count: self.slot_count(),
            });
        };
        match self.bind_present_timing(wor) {
            Ok(matched) => WorTimingBinding::CountMatched(matched),
            Err(drift) => WorTimingBinding::Drifted(drift),
        }
    }

    /// Bind a known-present `%wor` tier without introducing an absence state.
    ///
    /// Equal counts admit the correspondence check; they do not prove lexical
    /// correspondence or timing validity. The optional-tier entry point delegates
    /// here so both routes share the same membership and count comparison.
    pub fn bind_present_timing(
        self,
        wor: &'main WorTier,
    ) -> Result<CountMatchedWorTimings<'main>, WorTimingDrift> {
        let main_count = self.slot_count();
        let policy = self.policy;
        let wor_count = WorTierSlotCount(wor.word_count());
        if main_count.get() != wor_count.get() {
            return Err(WorTimingDrift {
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

        Ok(CountMatchedWorTimings {
            policy,
            main_slots,
            wor_slots,
        })
    }
}

fn wor_display_word_from_main(projected: ProjectedWorWord<'_>) -> Word {
    // A display projection is generated, not read from source.
    let mut display_word = Word::new(
        crate::Location::Unlocated,
        crate::model::WordText::from(projected.display_text.into_owned()),
    );
    display_word
        .inline_bullet
        .clone_from(&projected.word.inline_bullet);
    display_word
}

pub(super) fn canonical_wor_display_text(word: &Word) -> &str {
    word.cleaned_text()
}
