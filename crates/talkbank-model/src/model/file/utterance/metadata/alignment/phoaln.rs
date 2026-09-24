//! One positional owner for `%xphoaln` counts, mappings and reconstruction.
//!
//! Phon's extension specification §2 rule 5 makes a one-sided pause consume
//! a slot only on the tier bearing that pause. A null phone inside an ordinary
//! lexical alignment word does not have that exception.

use crate::Utterance;
use crate::alignment::AlignmentPair;
use crate::alignment::indices::{MainWordIndex, PhoItemIndex};
use crate::model::dependent_tier::{WordAlignment, is_pause_marker};
use crate::model::{PhoItem, PhoTier};

enum Slots {
    Both,
    ModelOnly,
    ActualOnly,
}

impl Slots {
    fn of(word: &WordAlignment) -> Self {
        if let [pair] = word.pairs.as_slice() {
            match (&pair.source, &pair.target) {
                (Some(source), None) if is_pause_marker(source.as_str()) => return Self::ModelOnly,
                (None, Some(target)) if is_pause_marker(target.as_str()) => {
                    return Self::ActualOnly;
                }
                _ => {}
            }
        }
        Self::Both
    }
}

/// A missing source item still consumes a slot; an opposite-side pause does not.
enum SourceSlot<'a, I> {
    Unconsumed,
    Consumed(Option<(I, &'a PhoItem)>),
}

impl<'a, I: Copy> SourceSlot<'a, I> {
    fn consume(
        tier: Option<&'a PhoTier>,
        cursor: &mut usize,
        index: impl FnOnce(usize) -> I,
    ) -> Self {
        let position = *cursor;
        *cursor += 1;
        Self::Consumed(
            tier.and_then(|tier| tier.items.get(position))
                .map(|item| (index(position), item)),
        )
    }

    fn is_consumed(&self) -> bool {
        matches!(self, Self::Consumed(_))
    }

    fn index(&self) -> Option<I> {
        match self {
            Self::Consumed(Some((index, _))) => Some(*index),
            Self::Unconsumed | Self::Consumed(None) => None,
        }
    }

    fn word(&self) -> Option<&'a str> {
        match self {
            Self::Consumed(Some((_, PhoItem::Word(word)))) => Some(word.as_str()),
            Self::Unconsumed
            | Self::Consumed(None)
            | Self::Consumed(Some((_, PhoItem::Group(_)))) => None,
        }
    }
}

/// Producer-issued relationship to the owning utterance's two source tiers.
/// Private fields prevent callers from pairing a word with unrelated indices.
pub(crate) struct PhoalnWordBinding<'a> {
    word: &'a WordAlignment,
    model: SourceSlot<'a, MainWordIndex>,
    actual: SourceSlot<'a, PhoItemIndex>,
}

impl<'a> PhoalnWordBinding<'a> {
    pub(crate) fn word(&self) -> &'a WordAlignment {
        self.word
    }
    pub(crate) fn consumes_model(&self) -> bool {
        self.model.is_consumed()
    }
    pub(crate) fn consumes_actual(&self) -> bool {
        self.actual.is_consumed()
    }
    pub(crate) fn model_word(&self) -> Option<&'a str> {
        self.model.word()
    }
    pub(crate) fn actual_word(&self) -> Option<&'a str> {
        self.actual.word()
    }

    pub(crate) fn into_alignment_pair(self) -> AlignmentPair<MainWordIndex, PhoItemIndex> {
        AlignmentPair::new(self.model.index(), self.actual.index())
    }
}

impl Utterance {
    /// Advance each source cursor according to the alignment word's typed role.
    /// Both tiers are obtained from this utterance, never supplied by a caller.
    pub(crate) fn phoaln_word_bindings(&self) -> impl Iterator<Item = PhoalnWordBinding<'_>> {
        let model = self.mod_tier();
        let actual = self.pho_tier();
        let mut model_cursor = 0;
        let mut actual_cursor = 0;
        self.phoaln_tier()
            .into_iter()
            .flat_map(|tier| tier.words.iter())
            .map(move |word| {
                let (model, actual) = match Slots::of(word) {
                    Slots::Both => (
                        SourceSlot::consume(model, &mut model_cursor, MainWordIndex::new),
                        SourceSlot::consume(actual, &mut actual_cursor, PhoItemIndex::new),
                    ),
                    Slots::ModelOnly => (
                        SourceSlot::consume(model, &mut model_cursor, MainWordIndex::new),
                        SourceSlot::Unconsumed,
                    ),
                    Slots::ActualOnly => (
                        SourceSlot::Unconsumed,
                        SourceSlot::consume(actual, &mut actual_cursor, PhoItemIndex::new),
                    ),
                };
                PhoalnWordBinding {
                    word,
                    model,
                    actual,
                }
            })
    }
}
