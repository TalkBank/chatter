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
        | UtteranceContent::OrphanAnnotation(_)
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

/// One nonempty, contiguous slice of the producing main-tier content.
/// Only partition admission constructs this; readers cannot pair it with a
/// second source, invent an empty bucket or supply a different affiliation.
pub(super) struct ContentGroup<'source, Affiliation = ()> {
    index: usize,
    content_index: usize,
    first: &'source UtteranceContent,
    remaining: &'source [UtteranceContent],
    affiliation: Affiliation,
}

impl<'source, Affiliation> ContentGroup<'source, Affiliation> {
    fn items(&self) -> impl Iterator<Item = &'source UtteranceContent> {
        std::iter::once(self.first).chain(self.remaining)
    }

    pub(super) fn cloned_content(&self) -> Vec<UtteranceContent> {
        self.items().cloned().collect()
    }

    pub(super) fn affiliation(&self) -> &Affiliation {
        &self.affiliation
    }
}

/// Metadata position in a split that has distinct first and last children.
pub(super) enum ChildPosition {
    First,
    Interior,
    Last,
}

/// Source-associated admission of two or more nonempty content groups.
/// Both endpoint fields are mandatory; optional interior groups cannot erase
/// either endpoint. Count, content and affiliations have this single owner.
pub(super) struct ContentItemGroups<'source, Affiliation = ()> {
    source: &'source talkbank_model::model::Utterance,
    first: ContentGroup<'source, Affiliation>,
    interior: Vec<ContentGroup<'source, Affiliation>>,
    last: ContentGroup<'source, Affiliation>,
}

/// Rebuilt children with the admitted partition's cardinality.
/// Construction and length-changing access stay private to this producer.
pub(super) struct RebuiltChildren(Vec<talkbank_model::model::Utterance>);

impl RebuiltChildren {
    pub(super) fn as_slice(&self) -> &[talkbank_model::model::Utterance] {
        &self.0
    }

    pub(super) fn as_mut_slice(&mut self) -> &mut [talkbank_model::model::Utterance] {
        &mut self.0
    }

    pub(super) fn into_vec(self) -> Vec<talkbank_model::model::Utterance> {
        self.0
    }
}

impl<'source> ContentItemGroups<'source> {
    /// Admit the proposed word labels against their producing content domain.
    /// `None` means no structural split, not an empty rebuilt child list.
    pub(super) fn admit(
        source: &'source talkbank_model::model::Utterance,
        word_to_content: &[usize],
        assignments: &[usize],
    ) -> Result<Option<Self>, super::SplitRefusal> {
        Self::admit_with_affiliation(source, word_to_content, assignments, |_| ())
    }
}

/// A normalized run retains the source-word slot that established it.
#[derive(Clone, Copy)]
struct AssignedRun {
    index: usize,
    first_word_slot: usize,
}

impl<'source, Affiliation> ContentItemGroups<'source, Affiliation> {
    /// Attach evidence at the producing word slot, before content is rebuilt.
    pub(super) fn admit_with_affiliation(
        source: &'source talkbank_model::model::Utterance,
        word_to_content: &[usize],
        assignments: &[usize],
        mut affiliation: impl FnMut(usize) -> Affiliation,
    ) -> Result<Option<Self>, super::SplitRefusal> {
        use super::SplitRefusal;
        use std::collections::HashSet;

        if assignments.len() != word_to_content.len() {
            return Err(SplitRefusal::SlotCount {
                expected: word_to_content.len(),
                actual: assignments.len(),
            });
        }
        // Labels become dense source-ordered runs; their magnitude never sizes
        // a collection. Reappearing labels would reorder content and refuse.
        let mut seen = HashSet::new();
        let mut previous = None;
        let mut run_count = 0usize;
        let mut first_word_slot = 0usize;
        let mut normalized = Vec::with_capacity(assignments.len());
        for (slot, &label) in assignments.iter().enumerate() {
            if previous != Some(label) {
                if !seen.insert(label) {
                    return Err(SplitRefusal::DisjointGroup { group: label });
                }
                run_count += 1;
                previous = Some(label);
                first_word_slot = slot;
            }
            normalized.push(AssignedRun {
                index: run_count - 1,
                first_word_slot,
            });
        }
        let Some((&first_group, _)) = normalized.split_first() else {
            return Ok(None);
        };
        let content_items = &source.main.content.content;
        let mut partial: Vec<Option<AssignedRun>> = vec![None; content_items.len()];
        for (&content_index, &group) in word_to_content.iter().zip(&normalized) {
            match partial[content_index] {
                Some(previous) if previous.index != group.index => {
                    return Err(SplitRefusal::IndivisibleContent { content_index });
                }
                Some(_) | None => partial[content_index] = Some(group),
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
        let by_content = partial.into_iter().map(|slot| {
            if let Some(group) = slot {
                last_group = group;
            }
            last_group
        });
        let mut items = content_items.iter().zip(by_content).enumerate();
        let Some((mut start, (mut head, mut group))) = items.next() else {
            return Ok(None);
        };
        let mut first = None;
        let mut interior = Vec::new();
        for (index, (item, next_group)) in items {
            if group.index != next_group.index {
                let completed = ContentGroup {
                    index: group.index,
                    content_index: start,
                    first: head,
                    remaining: &content_items[start + 1..index],
                    affiliation: affiliation(group.first_word_slot),
                };
                match first {
                    None => first = Some(completed),
                    Some(_) => interior.push(completed),
                }
                start = index;
                head = item;
                group = next_group;
            }
        }
        let Some(first) = first else {
            return Ok(None);
        };
        let admitted = Self {
            source,
            first,
            interior,
            last: ContentGroup {
                index: group.index,
                content_index: start,
                first: head,
                remaining: &content_items[start + 1..],
                affiliation: affiliation(group.first_word_slot),
            },
        };
        for group in admitted.groups() {
            if matches!(group.first, UtteranceContent::Separator(_)) {
                return Err(SplitRefusal::LeadingSeparator {
                    content_index: group.content_index,
                });
            }
        }
        Ok(Some(admitted))
    }

    fn groups(&self) -> impl Iterator<Item = &ContentGroup<'source, Affiliation>> {
        std::iter::once(&self.first)
            .chain(&self.interior)
            .chain(std::iter::once(&self.last))
    }

    pub(super) fn source(&self) -> &'source talkbank_model::model::Utterance {
        self.source
    }

    /// Content and affiliation travel together, in their producing order.
    pub(super) fn items_with_groups(
        &self,
    ) -> impl Iterator<Item = (&'source UtteranceContent, usize)> + '_ {
        self.groups()
            .flat_map(|group| group.items().map(move |item| (item, group.index)))
    }

    /// The admitted first child's source affiliation, including leading markers.
    pub(super) const fn first_group(&self) -> usize {
        self.first.index
    }

    /// Every admitted run owns content, so this also sizes dense tier partitions.
    pub(super) fn group_count(&self) -> usize {
        self.interior.len() + 2
    }

    /// The only route to rebuilt children: one child for each source group.
    pub(super) fn rebuild(
        &self,
        mut make_child: impl FnMut(
            ChildPosition,
            &ContentGroup<'source, Affiliation>,
        ) -> talkbank_model::model::Utterance,
    ) -> RebuiltChildren {
        let mut children = Vec::with_capacity(self.group_count());
        children.push(make_child(ChildPosition::First, &self.first));
        children.extend(
            self.interior
                .iter()
                .map(|group| make_child(ChildPosition::Interior, group)),
        );
        children.push(make_child(ChildPosition::Last, &self.last));
        RebuiltChildren(children)
    }
}
