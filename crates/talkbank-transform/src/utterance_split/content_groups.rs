//! A total assignment of top-level content items to the children of a split,
//! and the one owner of "is this content node a retrace".
use talkbank_model::model::{Retrace, UtteranceContent};

/// The retrace this content node is, in EITHER spelling, or `None`.
///
/// One owner for the question, because the answer is what decides whether a
/// node binds forward to its material. It was written inline as
/// `matches!(item, UtteranceContent::Retrace(_))`, and when chatter v0.10.0
/// added `AnnotatedRetrace` that expression silently started answering "no"
/// for every retrace carrying an error code: the crate compiled, and the
/// stranding regression this module documents came back for exactly the
/// utterances a transcriber had annotated. A `matches!` is a catch-all wearing
/// a macro. The exhaustive match below makes a third spelling a compile error.
///
/// Returns the NODE rather than a bool: the two spellings differ only in
/// carrying the annotations that follow the marker, so a caller that later
/// needs the retraced material has it, and a predicate that answers `true`
/// and throws the answer away cannot be extended without being rewritten.
pub(super) fn as_retrace(item: &UtteranceContent) -> Option<&Retrace> {
    match item {
        UtteranceContent::Retrace(retrace) => Some(retrace),
        UtteranceContent::AnnotatedRetrace(annotated) => Some(&annotated.inner),
        UtteranceContent::Word(_)
        | UtteranceContent::AnnotatedWord(_)
        | UtteranceContent::ReplacedWord(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::AnnotatedQuotation(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Action(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => None,
    }
}

/// Which child each top-level content item travels with, for EVERY item.
///
/// This replaced a `Vec<Option<usize>>` that stayed partial after its
/// fills, so both of its readers ended in `unwrap_or(0)`: an item whose
/// group was unknown was quietly handed to the FIRST child, which is a
/// measured-looking answer nobody measured. Here the fill IS the
/// constructor, its seed is the first word's own group rather than a
/// zero, and each stored entry is a plain `usize`, so no unknown survives
/// construction for a reader to default.
pub(super) struct ContentItemGroups {
    /// One group index per content item, in content order.
    by_content: Vec<usize>,
    /// One past the highest group index `by_content` can name. Every entry
    /// indexes a collection sized from this, by construction.
    group_count: usize,
}

impl ContentItemGroups {
    /// Assign every content item to a child, or report that this
    /// assignment splits nothing.
    ///
    /// `None` is the one answer covering all three ways there is nothing
    /// to split: no assignments, no extracted words, or every word
    /// assigned to the same child. A `Some` therefore proves the
    /// assignment names at least two distinct children, and the caller
    /// hands back the parent untouched on `None` rather than rebuilding
    /// it from its own parts.
    pub(super) fn assign(
        content_items: &[UtteranceContent],
        word_to_content: &[usize],
        assignments: &[usize],
    ) -> Option<Self> {
        let (&first_group, rest) = assignments.split_first()?;
        if word_to_content.is_empty() || rest.iter().all(|&group| group == first_group) {
            return None;
        }

        // Direct assignment, first writer wins: a content item holding
        // several extracted words takes the group of its first word.
        let mut partial: Vec<Option<usize>> = vec![None; content_items.len()];
        for (word_idx, &content_idx) in word_to_content.iter().enumerate() {
            if word_idx < assignments.len() && partial[content_idx].is_none() {
                partial[content_idx] = Some(assignments[word_idx]);
            }
        }

        // A retrace marker binds FORWARD to the repeated/corrected material it
        // points at: `<X> [/] Y` is one unit where X was abandoned and Y is the
        // retry, so a retrace content node must travel with the following kept
        // word's group. Retraced words are not counted in the Mor word domain, so
        // retrace nodes get no direct assignment above; the generic back-fill
        // below would attach them to the PRECEDING word, stranding them as a
        // dangling `[/]` when the split boundary falls between the retrace and its
        // material (real utterance-segmentation output stranded retraces this
        // way). Pre-assign each un-grouped retrace node the group of
        // the next already-grouped content item; if none follows (a legitimately
        // utterance-final retrace), leave it for the back-fill.
        // Regression: `utseg_split_does_not_strand_retrace`.
        // One reverse pass carries the next DIRECTLY assigned group, so a
        // run of retraces is linear rather than one forward scan each. A
        // retrace filled here is not a direct assignment and does not
        // become the "next group" of the items before it, which is what
        // the forward scan it replaces saw as well.
        let mut next_direct = None;
        for (slot, item) in partial.iter_mut().zip(content_items).rev() {
            match slot {
                Some(group) => next_direct = Some(*group),
                None if as_retrace(item).is_some() => *slot = next_direct,
                None => {}
            }
        }

        // One back-fill pass, seeded with the first extracted word's own
        // group. The seed is what makes this total, and it is evidence
        // rather than a default: an item sitting before the first grouped
        // one is punctuation or a marker, and it travels with the child
        // holding the utterance's first word, which is the group the model
        // chose for that word. Everything after a grouped item travels
        // with the most recent one, as before. The separate forward-fill
        // pass that used to repair leading `None`s is what the seed
        // replaces.
        let mut last_group = first_group;
        let by_content = partial
            .into_iter()
            .map(|slot| {
                if let Some(group) = slot {
                    last_group = group;
                }
                last_group
            })
            .collect();

        Some(Self {
            by_content,
            // Every stored entry came from `assignments`, so the highest
            // assignment bounds them all.
            group_count: rest.iter().copied().fold(first_group, usize::max) + 1,
        })
    }

    /// Each content item's group, in content order: one entry per item.
    pub(super) fn in_content_order(&self) -> impl Iterator<Item = usize> + '_ {
        self.by_content.iter().copied()
    }

    /// One past the highest group any item names.
    pub(super) fn group_count(&self) -> usize {
        self.group_count
    }
}
