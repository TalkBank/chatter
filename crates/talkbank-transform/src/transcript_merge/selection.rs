//! Source coordinates for a caller-selected donor projection.

use super::*;
use talkbank_model::SemanticEq;

/// A selected transcript bound to its original header positions and parents.
///
/// This verifies structural coordinates, not the caller's authority to omit
/// speech or the acoustic correctness of supplied segment timings. Those
/// decisions require external evidence. Output donor origins index `selected`;
/// `parents` translates them back to original utterances.
pub struct SourceBoundDonorSelection<'a> {
    original: &'a ChatFile,
    selected: &'a ChatFile,
    parents: Vec<DonorIdx>,
    headers: Vec<HeaderContext<'a>>,
    relative_order: Option<(&'a ChatFile, super::relative_order::OrderConstraints)>,
    timed_gem: Option<(&'a ChatFile, super::gem_exterior::TimedGem)>,
    draft_reference: Option<&'a ChatFile>,
}

/// Header scope cannot carry a body bracket while claiming to be metadata.
pub(super) enum HeaderContext<'a> {
    Opening,
    Body(SourceHeaderBracket<'a>),
    End,
}

/// Neighbor facts borrow the actual immutable source bullets. A preceding
/// start and end cannot come from different utterances or differ in presence.
pub(super) struct SourceHeaderBracket<'a> {
    previous: Option<&'a talkbank_model::model::Bullet>,
    next: Option<&'a talkbank_model::model::Bullet>,
}

impl SourceHeaderBracket<'_> {
    pub(super) fn previous_start(&self) -> Option<u64> {
        self.previous.map(|b| b.timing.start_ms)
    }
    pub(super) fn previous_end(&self) -> Option<u64> {
        self.previous.map(|b| b.timing.end_ms)
    }
    pub(super) fn next_start(&self) -> Option<u64> {
        self.next.map(|b| b.timing.start_ms)
    }
}

#[derive(Clone, Copy)]
enum HeaderRegion {
    Opening,
    Body,
}

impl<'a> SourceBoundDonorSelection<'a> {
    /// Bind every selected utterance to a nondecreasing original parent.
    /// Headers must survive unchanged, in order and at their source boundary.
    /// Multiple selected children may name one parent; omitted parents have no
    /// selected child. Speaker relabeling is outside this operation's contract.
    pub fn bind(
        original: &'a ChatFile,
        selected: &'a ChatFile,
        parents: Vec<DonorIdx>,
    ) -> Result<Self, MergeError> {
        let originals: Vec<_> = original.utterances().collect();
        let selected_rows: Vec<_> = selected.utterances().collect();
        if parents.len() != selected_rows.len() {
            return Err(MergeError::InvalidDonorSelection);
        }
        let mut previous_parent = None;
        for (parent, child) in parents.iter().zip(&selected_rows) {
            let index = parent.utterance().raw();
            let Some(source) = originals.get(index) else {
                return Err(MergeError::InvalidDonorSelection);
            };
            if previous_parent.is_some_and(|previous| index < previous)
                || source.main.speaker != child.main.speaker
            {
                return Err(MergeError::InvalidDonorSelection);
            }
            // A child replaces part of its parent, so a timed child lies inside a
            // timed parent. Header brackets come from the original timeline; a
            // child timed outside its parent would be placed against brackets
            // that do not describe it.
            if let (Some(parent_bullet), Some(child_bullet)) = (
                source.main.content.bullet.as_ref(),
                child.main.content.bullet.as_ref(),
            ) && (child_bullet.timing.start_ms < parent_bullet.timing.start_ms
                || child_bullet.timing.end_ms > parent_bullet.timing.end_ms)
            {
                return Err(MergeError::InvalidDonorSelection);
            }
            previous_parent = Some(index);
        }
        let mut next_bullets = vec![None; original.lines.len()];
        let mut next = None;
        for (index, line) in original.lines.iter().enumerate().rev() {
            next_bullets[index] = next;
            if let Line::Utterance(row) = line {
                next = row.main.content.bullet.as_ref().or(next);
            }
        }
        let mut projected_headers = selected
            .lines
            .iter()
            .scan(0, |count, line| {
                Some(match line {
                    Line::Utterance(_) => {
                        *count += 1;
                        None
                    }
                    Line::Header { header, .. } => Some((*count, header.as_ref())),
                })
            })
            .flatten();
        let mut headers = Vec::new();
        let (mut original_count, mut selected_count) = (0, 0);
        let mut previous: Option<&talkbank_model::model::Bullet> = None;
        let mut region = HeaderRegion::Opening;
        for (index, line) in original.lines.iter().enumerate() {
            match line {
                Line::Utterance(row) => {
                    region = HeaderRegion::Body;
                    original_count += 1;
                    while parents
                        .get(selected_count)
                        .is_some_and(|p| p.utterance().raw() < original_count)
                    {
                        selected_count += 1;
                    }
                    if let Some(bullet) = &row.main.content.bullet {
                        if previous
                            .is_some_and(|before| bullet.timing.start_ms < before.timing.start_ms)
                        {
                            return Err(MergeError::InvalidDonorSelection);
                        }
                        previous = Some(bullet);
                    }
                }
                Line::Header { header, .. } => {
                    if matches!(
                        header.as_ref(),
                        Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
                    ) {
                        region = HeaderRegion::Body;
                    }
                    if !projected_headers.next().is_some_and(|(count, candidate)| {
                        count == selected_count && header.as_ref().semantic_eq(candidate)
                    }) {
                        return Err(MergeError::InvalidDonorSelection);
                    }
                    let context = match (header.as_ref(), region) {
                        (Header::End, _) => HeaderContext::End,
                        (_, HeaderRegion::Opening) => HeaderContext::Opening,
                        (_, HeaderRegion::Body) => HeaderContext::Body(SourceHeaderBracket {
                            previous,
                            next: next_bullets[index],
                        }),
                    };
                    headers.push(context);
                }
            }
        }
        if projected_headers.next().is_some() {
            return Err(MergeError::InvalidDonorSelection);
        }
        Ok(Self {
            original,
            selected,
            parents,
            headers,
            relative_order: None,
            timed_gem: None,
            draft_reference: None,
        })
    }

    /// Attach externally attested cross-source order without adding timestamps.
    /// Coordinates refer to the exact reference and selected donor populations.
    /// Contradictory proposals refuse; the caller owns their semantic authority.
    pub fn with_relative_order(
        mut self,
        reference: &'a ChatFile,
        proposals: Vec<RelativeOrderConstraint>,
    ) -> Result<Self, MergeError> {
        let constraints = super::relative_order::OrderConstraints::bind(
            reference.utterances().count(),
            self.selected.utterances().count(),
            proposals,
        )?;
        self.relative_order = Some((reference, constraints));
        Ok(self)
    }

    pub(super) fn order_for(
        &self,
        reference: &ChatFile,
    ) -> Result<Option<super::relative_order::OrderConstraints>, MergeError> {
        self.relative_order
            .as_ref()
            .map(|(source, constraints)| {
                if !std::ptr::eq(*source, reference) {
                    return Err(MergeError::InvalidRelativeOrder);
                }
                Ok(constraints.clone())
            })
            .transpose()
    }

    /// Permit complete donor intervals strictly outside this named reference
    /// gem to remain outside its markers. All enclosed reference rows must be
    /// timed; the caller remains responsible for timing reliability and scope.
    /// Does not authorize lexical deletion, speaker changes or new timestamps.
    pub fn with_timed_gem_exterior(
        mut self,
        reference: &'a ChatFile,
        label: &str,
    ) -> Result<Self, MergeError> {
        self.timed_gem = Some((
            reference,
            super::gem_exterior::TimedGem::bind(reference, label)?,
        ));
        Ok(self)
    }

    pub(super) fn gem_for(
        &self,
        reference: &ChatFile,
    ) -> Result<Option<super::gem_exterior::TimedGem>, MergeError> {
        self.timed_gem
            .as_ref()
            .map(|(source, gem)| {
                if !std::ptr::eq(*source, reference) {
                    return Err(MergeError::InvalidGemExterior);
                }
                Ok(gem.clone())
            })
            .transpose()
    }

    /// Explicitly permit unresolved cross-source frontiers to serialize
    /// reference-first with visible review comments and structured decisions.
    /// Known constraints, validity, source order and speech retention still apply.
    pub fn with_flagged_draft_order(mut self, reference: &'a ChatFile) -> Self {
        self.draft_reference = Some(reference);
        self
    }

    pub(super) fn draft_policy_for(
        &self,
        reference: &ChatFile,
    ) -> Result<super::draft_order::DraftOrderPolicy, MergeError> {
        match self.draft_reference {
            None => Ok(super::draft_order::DraftOrderPolicy::Strict),
            Some(source) if std::ptr::eq(source, reference) => {
                Ok(super::draft_order::DraftOrderPolicy::FlaggedReferenceFirst)
            }
            Some(_) => Err(MergeError::InvalidRelativeOrder),
        }
    }

    /// Original parent for each selected-donor ordinal, in selected order.
    pub fn parents(&self) -> &[DonorIdx] {
        &self.parents
    }

    pub(super) fn original(&self) -> &'a ChatFile {
        self.original
    }
    pub(super) fn headers(&self) -> &[HeaderContext<'a>] {
        &self.headers
    }
}

/// Merge a selected donor without rediscovering header scope from its reduced
/// utterance population. Original recorded header brackets constrain ordering;
/// they never become selected-utterance time bullets.
/// An empty retain set is accepted only when the reference has no utterances;
/// its headers remain the metadata base and all selected donor speech survives.
pub fn merge_chat_files_with_donor_selection(
    reference: &ChatFile,
    donor: &SourceBoundDonorSelection<'_>,
    retain: &[SpeakerCode],
    strip_tiers: &[String],
) -> Result<Merged, MergeError> {
    merge_chat_files_with_donor_selection_draft(reference, donor, retain, strip_tiers)
        .and_then(MergeDraft::validate)
}

/// The same merge, stopped before model validation. See [`MergeDraft`] for the
/// one edit a draft admits and for its only route to a validated [`Merged`].
pub fn merge_chat_files_with_donor_selection_draft(
    reference: &ChatFile,
    donor: &SourceBoundDonorSelection<'_>,
    retain: &[SpeakerCode],
    strip_tiers: &[String],
) -> Result<MergeDraft, MergeError> {
    merge_with_placement(
        reference,
        donor.selected,
        retain,
        strip_tiers,
        ordered::Placement::SourceOrder,
        Some(donor),
    )
}
