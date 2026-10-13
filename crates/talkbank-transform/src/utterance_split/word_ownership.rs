//! The nonempty measured population carried through speaker splitting.
use crate::rediarize::TrackOwnership;

/// Measured distributions for every original source word, in source order.
///
/// Speaker admission measures the mandatory first timing before constructing
/// this population. Plans, outcomes and consumed parts all retain it unchanged;
/// none can replace its evidence with an empty vector.
///
/// ```compile_fail,E0277
/// use talkbank_transform::utterance_split::WordSpeakerOwnership;
/// let empty: WordSpeakerOwnership = Vec::new().into();
/// ```
///
/// Independent measurements cannot replace the source-admitted population:
///
/// ```compile_fail,E0451
/// use talkbank_transform::{rediarize::TrackOwnership, utterance_split::WordSpeakerOwnership};
/// fn substitute(first: TrackOwnership) -> WordSpeakerOwnership {
///     WordSpeakerOwnership { first, following: Vec::new() }
/// }
/// ```
#[derive(Debug)]
pub struct WordSpeakerOwnership {
    pub(super) first: TrackOwnership,
    pub(super) following: Vec<TrackOwnership>,
}

impl WordSpeakerOwnership {
    /// The first measured source word, without an empty alternative.
    pub fn first(&self) -> &TrackOwnership {
        &self.first
    }

    /// Remaining measured source words; a singleton has an empty tail.
    pub fn following(&self) -> &[TrackOwnership] {
        &self.following
    }

    /// The last measured source word, including the singleton case.
    pub fn last(&self) -> &TrackOwnership {
        self.following.last().unwrap_or(&self.first)
    }

    /// Number of original words with retained measurement evidence.
    pub fn len(&self) -> usize {
        1 + self.following.len()
    }

    /// Admitted word ownership always retains at least one source word.
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Traverse the first word and every following word exactly once.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &TrackOwnership> {
        std::iter::once(&self.first).chain(&self.following)
    }
}

// The external ownership evidence remains the same JSON array, not a new
// object containing implementation-specific first/following fields.
impl serde::Serialize for WordSpeakerOwnership {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl IntoIterator for WordSpeakerOwnership {
    type Item = TrackOwnership;
    type IntoIter =
        std::iter::Chain<std::iter::Once<TrackOwnership>, std::vec::IntoIter<TrackOwnership>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.first).chain(self.following)
    }
}

impl<'ownership> IntoIterator for &'ownership WordSpeakerOwnership {
    type Item = &'ownership TrackOwnership;
    type IntoIter =
        std::iter::Chain<std::iter::Once<Self::Item>, std::slice::Iter<'ownership, TrackOwnership>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.first).chain(&self.following)
    }
}
