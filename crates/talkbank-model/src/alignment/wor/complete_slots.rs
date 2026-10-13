//! The nonempty slot population retained by complete timing admission.
use super::CompleteWorTimingSlot;

/// Positive, corroborated timing slots in source-word order.
///
/// The first slot is mandatory, not recovered from a possibly empty vector.
/// Only timing assessment constructs this population; its source borrow and
/// positive intervals remain attached to every slot.
///
/// An empty population cannot be substituted for admitted complete timing:
///
/// ```compile_fail,E0277
/// use talkbank_model::alignment::CompleteWorTimingSlots;
/// let empty: CompleteWorTimingSlots<'_> = Vec::new().into();
/// ```
///
/// Even an existing slot cannot be used to replace the admitted population:
///
/// ```compile_fail,E0451
/// use talkbank_model::alignment::{CompleteWorTimingSlot, CompleteWorTimingSlots};
/// fn substitute<'a>(first: CompleteWorTimingSlot<'a>) -> CompleteWorTimingSlots<'a> {
///     CompleteWorTimingSlots { first, following: Vec::new() }
/// }
/// ```
#[derive(Debug, PartialEq)]
pub struct CompleteWorTimingSlots<'main> {
    pub(super) first: CompleteWorTimingSlot<'main>,
    pub(super) following: Vec<CompleteWorTimingSlot<'main>>,
}

impl<'main> CompleteWorTimingSlots<'main> {
    /// The first positive source-word timing, without an empty alternative.
    pub fn first(&self) -> &CompleteWorTimingSlot<'main> {
        &self.first
    }

    /// The remaining source-word slots; a singleton has an empty tail.
    pub fn following(&self) -> &[CompleteWorTimingSlot<'main>] {
        &self.following
    }

    /// Number of source words carrying complete positive timing.
    pub fn len(&self) -> usize {
        1 + self.following.len()
    }

    /// Complete timing always retains at least one source word.
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Traverse every slot exactly once in source order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &CompleteWorTimingSlot<'main>> {
        std::iter::once(&self.first).chain(&self.following)
    }

    /// Adjacent source-word pairs, including the first slot and its successor.
    pub fn adjacent_pairs(
        &self,
    ) -> impl Iterator<Item = (&CompleteWorTimingSlot<'main>, &CompleteWorTimingSlot<'main>)> {
        self.iter().zip(&self.following)
    }
}

impl<'slots, 'main> IntoIterator for &'slots CompleteWorTimingSlots<'main> {
    type Item = &'slots CompleteWorTimingSlot<'main>;
    type IntoIter = std::iter::Chain<
        std::iter::Once<Self::Item>,
        std::slice::Iter<'slots, CompleteWorTimingSlot<'main>>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.first).chain(&self.following)
    }
}
