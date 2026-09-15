//! Forward-only source admission and assembly. Headers remain source events,
//! and an utterance's dependent tiers never leave their owning AST node.

use super::*;
use std::collections::{VecDeque, vec_deque::IntoIter};
use std::iter::Peekable;
use talkbank_model::{Span, TierSeparator};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Placement {
    Timed,
    SourceOrder,
}

/// Bounds are copied only from selected source AST time anchors. Missing bounds
/// remain unknown; neither endpoint is a fabricated utterance timestamp.
struct OrderBounds {
    lower: Option<u64>,
    upper: Option<u64>,
}

impl OrderBounds {
    fn precedes(&self, other: &Self) -> Option<bool> {
        if matches!((self.upper, other.lower), (Some(left), Some(right)) if left < right) {
            Some(true)
        } else if matches!((other.upper, self.lower), (Some(left), Some(right)) if left < right) {
            Some(false)
        } else {
            None
        }
    }
}

struct HeaderEvent {
    header: Box<Header>,
    span: Span,
    separator: TierSeparator,
}

impl HeaderEvent {
    fn into_line(self) -> Line {
        Line::Header {
            header: self.header,
            span: self.span,
            separator: self.separator,
        }
    }
}

/// Position is admitted once while the immutable utterance enters the source.
/// Neither the AST nor its position can be edited through an admitted cursor.
struct PositionedUtterance {
    utterance: Box<Utterance>,
    origin: MergeOrigin,
    start: Option<u64>,
    order: OrderBounds,
    // Enclosing observed intervals, used only with explicit correspondence.
    // Unlike start-order bounds, overlap does not establish precedence.
    interval_order: CorrespondenceBounds,
}

struct CorrespondenceBounds(OrderBounds);

impl CorrespondenceBounds {
    fn precedes(&self, other: &Self) -> Option<bool> {
        self.0.precedes(&other.0)
    }
}

enum SourceEvent {
    Header(HeaderEvent),
    Section(SectionBoundary),
    Utterance(PositionedUtterance),
    End(HeaderEvent),
}

/// A section marker's source-supplied timing bracket, never an invented instant.
struct SectionBoundary {
    header: HeaderEvent,
    previous_end: Option<u64>,
    next_start: Option<u64>,
    order: OrderBounds,
}

/// Exactly one direction is supported by the two section brackets.
enum SectionOrder {
    Before,
    After,
}

impl SectionBoundary {
    fn has_ordered_neighbors(&self) -> bool {
        !matches!((self.previous_end, self.next_start), (Some(before), Some(after)) if before > after)
    }

    fn precedes(&self, utterance: &PositionedUtterance) -> Result<bool, MergeError> {
        if self.has_ordered_neighbors() {
            if matches!((self.next_start, utterance.start), (Some(next), Some(start)) if start >= next)
            {
                return Ok(true);
            }
            if self
                .previous_end
                .is_some_and(|previous| utterance.start.is_some_and(|start| start < previous))
            {
                return Ok(false);
            }
        }
        Err(MergeError::AmbiguousSectionPlacement {
            header: self.header.header.to_chat_string(),
            previous_end: self.previous_end,
            next_start: self.next_start,
            competing: utterance.origin,
        })
    }

    fn precedes_section(&self, other: &Self) -> Result<SectionOrder, MergeError> {
        let before = self.has_ordered_neighbors()
            && other.has_ordered_neighbors()
            && matches!((self.next_start, other.previous_end), (Some(left), Some(right)) if left <= right);
        let after = self.has_ordered_neighbors()
            && other.has_ordered_neighbors()
            && matches!((other.next_start, self.previous_end), (Some(left), Some(right)) if left <= right);
        match (before, after) {
            (true, false) => return Ok(SectionOrder::Before),
            (false, true) => return Ok(SectionOrder::After),
            // Neither proof, or mutually contradictory proofs at a shared
            // instant. Utterance tie policy does not order section headers.
            (false, false) | (true, true) => {}
        }
        Err(MergeError::AmbiguousSectionOrder {
            reference: self.header.header.to_chat_string(),
            donor: other.header.header.to_chat_string(),
        })
    }
}

/// During admission, a section has its preceding neighbor but not yet its
/// following neighbor. Only `finish` can issue the fully bracketed stream.
enum PendingEvent {
    BoundSection(SectionBoundary),
    Header(HeaderEvent),
    Section {
        header: HeaderEvent,
        previous_end: Option<u64>,
        previous_start: Option<u64>,
    },
    Utterance(PositionedUtterance),
    End(HeaderEvent),
}

/// Only the admission walk can construct this queue. Consumers can inspect the
/// frontier and consume it; there is no sorting or random-access mutation seam.
struct OrderedSource {
    events: Peekable<IntoIter<SourceEvent>>,
}

struct SourceAdmission {
    events: Vec<PendingEvent>,
    previous: Option<(u64, MergeOrigin)>,
    previous_end: Option<u64>,
    placement: Placement,
}

impl SourceAdmission {
    fn new(placement: Placement) -> Self {
        Self {
            events: Vec::new(),
            previous: None,
            previous_end: None,
            placement,
        }
    }

    fn header(&mut self, header: HeaderEvent) {
        let event = if matches!(header.header.as_ref(), Header::End) {
            PendingEvent::End(header)
        } else if matches!(
            header.header.as_ref(),
            Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
        ) {
            PendingEvent::Section {
                header,
                previous_end: self.previous_end,
                previous_start: self.previous.map(|(start, _)| start),
            }
        } else {
            PendingEvent::Header(header)
        };
        self.events.push(event);
    }

    fn utterance(
        &mut self,
        utterance: Box<Utterance>,
        origin: MergeOrigin,
    ) -> Result<(), MergeError> {
        let timing = utterance
            .main
            .content
            .bullet
            .as_ref()
            .map(|bullet| &bullet.timing);
        if self.placement == Placement::Timed && timing.is_none() {
            return Err(MergeError::UnpositionedUtterance { origin });
        }
        let start = timing.map(|timing| timing.start_ms);
        let end = timing.map(|timing| timing.end_ms);
        if let (Some((previous_start, previous)), Some(start)) = (self.previous, start)
            && start < previous_start
        {
            return Err(MergeError::SourceTimelineReversal {
                previous,
                current: origin,
            });
        }
        let lower = start.or(self.previous.map(|(start, _)| start));
        if let Some(timing) = timing {
            self.previous = Some((timing.start_ms, origin));
            self.previous_end = Some(timing.end_ms);
        }
        self.events
            .push(PendingEvent::Utterance(PositionedUtterance {
                utterance,
                origin,
                start,
                order: OrderBounds {
                    lower,
                    upper: start,
                },
                interval_order: CorrespondenceBounds(OrderBounds { lower, upper: end }),
            }));
        Ok(())
    }

    fn finish(self) -> OrderedSource {
        let mut events = VecDeque::with_capacity(self.events.len());
        let mut next_start = None;
        let mut next_end = None;
        for event in self.events.into_iter().rev() {
            let admitted = match event {
                PendingEvent::BoundSection(boundary) => SourceEvent::Section(boundary),
                PendingEvent::Header(header) => SourceEvent::Header(header),
                PendingEvent::End(header) => SourceEvent::End(header),
                PendingEvent::Section {
                    header,
                    previous_end,
                    previous_start,
                } => SourceEvent::Section(SectionBoundary {
                    header,
                    previous_end,
                    next_start,
                    order: OrderBounds {
                        lower: previous_start,
                        upper: next_start,
                    },
                }),
                PendingEvent::Utterance(mut utterance) => {
                    utterance.order.upper = utterance.start.or(next_start);
                    let observed_end = utterance.interval_order.0.upper;
                    utterance.interval_order.0.upper = observed_end.or(next_end);
                    next_end = observed_end.or(next_end);
                    next_start = utterance.start.or(next_start);
                    SourceEvent::Utterance(utterance)
                }
            };
            events.push_front(admitted);
        }
        OrderedSource {
            events: events.into_iter().peekable(),
        }
    }
}

/// Both sources have passed placement admission before this type is born.
/// Assembly consumes only source frontiers, so source-relative order and
/// one-origin-per-utterance survive by construction.
struct AdmittedMerge {
    reference: OrderedSource,
    donor: OrderedSource,
    reference_fates: Vec<ReferenceFate>,
    donor_fates: Vec<DonorFate>,
    placement: Placement,
    relative_order: Option<super::relative_order::OrderConstraints>,
    timed_gem: Option<super::gem_exterior::TimedGem>,
    draft_order: super::draft_order::DraftOrderPolicy,
}

impl AdmittedMerge {
    fn assemble(mut self) -> Result<MergeDraft, MergeError> {
        let mut lines = Vec::new();
        let mut origins = Vec::new();
        let mut gem_exterior_placements = Vec::new();
        let mut draft_order_reviews = Vec::new();
        // Donor utterances whose placement the gem exterior policy decided: each
        // was compared against a reference gem marker and the policy answered.
        // Only these carry gem exterior evidence; a donor utterance ordered by an
        // ordinary comparison is not evidence of the policy.
        let mut gem_decided: Vec<MergeOrigin> = Vec::new();
        loop {
            let ordering = (|| -> Result<Option<bool>, MergeError> {
                Ok(Some(
                    match (self.reference.events.peek(), self.donor.events.peek()) {
                        (None, None) => return Ok(None),
                        (Some(_), None) => true,
                        (None, Some(_)) => false,
                        // End belongs after the donor's remaining source events.
                        (Some(SourceEvent::End(_)), Some(_)) => false,
                        (Some(SourceEvent::Header(_)), Some(_)) => true,
                        (
                            Some(SourceEvent::Section(_)),
                            Some(SourceEvent::Header(_) | SourceEvent::End(_)),
                        ) => false,
                        (
                            Some(SourceEvent::Section(reference)),
                            Some(SourceEvent::Section(donor)),
                        ) => match self.placement {
                            Placement::Timed => {
                                matches!(reference.precedes_section(donor)?, SectionOrder::Before)
                            }
                            Placement::SourceOrder => reference
                                .order
                                .precedes(&donor.order)
                                .ok_or_else(|| MergeError::AmbiguousSectionOrder {
                                    reference: reference.header.header.to_chat_string(),
                                    donor: donor.header.header.to_chat_string(),
                                })?,
                        },
                        (
                            Some(SourceEvent::Section(header)),
                            Some(SourceEvent::Utterance(utterance)),
                        ) => {
                            let exterior =
                                self.timed_gem.as_ref().and_then(|gem| {
                                    utterance.utterance.main.content.bullet.as_ref().and_then(
                                        |bullet| gem.precedes(&header.header.header, bullet),
                                    )
                                });
                            if let Some(precedes) = exterior {
                                precedes
                            } else {
                                match self.placement {
                                    Placement::Timed => header.precedes(utterance)?,
                                    Placement::SourceOrder => header
                                        .order
                                        .precedes(&utterance.order)
                                        .ok_or_else(|| MergeError::AmbiguousSectionPlacement {
                                            header: header.header.header.to_chat_string(),
                                            previous_end: header.previous_end,
                                            next_start: header.next_start,
                                            competing: utterance.origin,
                                        })?,
                                }
                            }
                        }
                        (
                            Some(SourceEvent::Utterance(utterance)),
                            Some(SourceEvent::Section(header)),
                        ) => match self.placement {
                            Placement::Timed => !header.precedes(utterance)?,
                            Placement::SourceOrder => !header
                                .order
                                .precedes(&utterance.order)
                                .ok_or_else(|| MergeError::AmbiguousSectionPlacement {
                                    header: header.header.header.to_chat_string(),
                                    previous_end: header.previous_end,
                                    next_start: header.next_start,
                                    competing: utterance.origin,
                                })?,
                        },
                        (
                            Some(SourceEvent::Utterance(_)),
                            Some(SourceEvent::Header(_) | SourceEvent::End(_)),
                        ) => false,
                        (
                            Some(SourceEvent::Utterance(reference)),
                            Some(SourceEvent::Utterance(donor)),
                        ) => {
                            let attested = self
                                .relative_order
                                .as_ref()
                                .and_then(|order| order.precedes(reference.origin, donor.origin));
                            let observed =
                                if reference.start.is_some() && reference.start == donor.start {
                                    Some(true)
                                } else {
                                    reference.order.precedes(&donor.order)
                                };
                            let disjoint = reference.interval_order.precedes(&donor.interval_order);
                            if matches!((attested,disjoint),(Some(a),Some(b)) if a != b) {
                                return Err(match (reference.origin, donor.origin) {
                                    (
                                        MergeOrigin::Retained(reference),
                                        MergeOrigin::Inserted(donor),
                                    ) => {
                                        MergeError::RelativeOrderTimingConflict { reference, donor }
                                    }
                                    _ => MergeError::InvalidDonorSelection,
                                });
                            }
                            match self.placement {
                                Placement::Timed => reference.start <= donor.start,
                                // Stable serialization convention, not acoustic precedence.
                                // Missing starts and header brackets are not time ties.
                                Placement::SourceOrder => attested.or(observed).ok_or(
                                    MergeError::AmbiguousUtteranceOrder {
                                        reference: reference.origin,
                                        donor: donor.origin,
                                    },
                                )?,
                            }
                        }
                    },
                ))
            })();
            if ordering.is_ok()
                && let (
                    Some(gem),
                    Some(SourceEvent::Section(header)),
                    Some(SourceEvent::Utterance(utterance)),
                ) = (
                    &self.timed_gem,
                    self.reference.events.peek(),
                    self.donor.events.peek(),
                )
            {
                let answered = utterance
                    .utterance
                    .main
                    .content
                    .bullet
                    .as_ref()
                    .is_some_and(|bullet| gem.precedes(&header.header.header, bullet).is_some());
                if answered && !gem_decided.contains(&utterance.origin) {
                    gem_decided.push(utterance.origin);
                }
            }
            let reference_next = match ordering {
                Ok(None) => break,
                Ok(Some(order)) => order,
                Err(error) => {
                    let review = self.draft_order.resolve(error, origins.len())?;
                    lines.push(Line::header(Header::Comment {
                        content: talkbank_model::model::BulletContent::from_text(review.comment()),
                    }));
                    draft_order_reviews.push(review);
                    true
                }
            };
            let source = if reference_next {
                &mut self.reference
            } else {
                &mut self.donor
            };
            if let Some(event) = source.events.next() {
                match event {
                    SourceEvent::Header(header) | SourceEvent::End(header) => {
                        lines.push(header.into_line())
                    }
                    SourceEvent::Section(boundary) => lines.push(boundary.header.into_line()),
                    SourceEvent::Utterance(positioned) => {
                        if let (Some(gem), MergeOrigin::Inserted(donor), Some(bullet)) = (
                            &self.timed_gem,
                            positioned.origin,
                            positioned.utterance.main.content.bullet.as_ref(),
                        ) && gem_decided.contains(&positioned.origin)
                            && let Some(evidence) = gem.evidence(donor, bullet)
                        {
                            gem_exterior_placements.push(evidence);
                        }
                        lines.push(Line::Utterance(positioned.utterance));
                        origins.push(positioned.origin);
                    }
                }
            }
        }
        if let Some(order) = &self.relative_order {
            order.verify(&origins)?;
        }
        let errors = talkbank_model::ErrorCollector::new();
        let participants =
            talkbank_model::model::participant::join::build_participants_from_lines(&lines)
                .report_into(&errors);
        let diagnostics = errors.into_vec();
        if !diagnostics.is_empty() {
            return Err(MergeError::InvalidParticipantJoin { diagnostics });
        }
        // Validation is the draft's own transition (`MergeDraft::validate`), so a
        // caller may repair timing between assembly and validation.
        Ok(MergeDraft {
            file: ChatFile::with_participants(lines, participants),
            origins,
            reference_fates: self.reference_fates,
            donor_fates: self.donor_fates,
            gem_exterior_placements,
            draft_order_reviews,
            bullet_edits: std::collections::BTreeMap::new(),
        })
    }
}

fn copied_header(header: &Header, span: Span, separator: TierSeparator) -> HeaderEvent {
    HeaderEvent {
        header: Box::new(header.clone()),
        span,
        separator,
    }
}

/// Reconciled donor metadata is an explicit opening-header exception. Body
/// headers are never admitted through this channel.
pub(super) struct OpeningAdditions {
    participants: Vec<ParticipantEntry>,
    events: VecDeque<HeaderEvent>,
}

impl OpeningAdditions {
    pub(super) fn from_donor(
        donor: &ChatFile,
        retain: &[SpeakerCode],
        deduped: &std::collections::HashSet<SpeakerCode>,
        participants: Vec<ParticipantEntry>,
    ) -> Result<Self, MergeError> {
        let mut additions = Self {
            participants,
            events: VecDeque::new(),
        };
        let mut comment_seen = false;
        for line in donor.lines.iter().take_while(|line| !starts_body(line)) {
            if let Line::Header {
                header,
                span,
                separator,
            } = line
            {
                if let Header::ID(id) = header.as_ref() {
                    if comment_seen {
                        return Err(MergeError::DonorMetadataOrder);
                    }
                    if !retain.contains(&id.speaker) && !deduped.contains(&id.speaker) {
                        additions
                            .events
                            .push_back(copied_header(header, *span, *separator));
                    }
                } else if matches!(header.as_ref(), Header::Comment { .. }) {
                    comment_seen = true;
                    additions
                        .events
                        .push_back(copied_header(header, *span, *separator));
                }
            }
        }
        Ok(additions)
    }
}

fn starts_body(line: &Line) -> bool {
    match line {
        Line::Utterance(_) => true,
        Line::Header { header, .. } => matches!(
            header.as_ref(),
            Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
        ),
    }
}

fn is_end(line: &Line) -> bool {
    matches!(line, Line::Header { header, .. } if matches!(header.as_ref(), Header::End))
}

fn inject_headers(admission: &mut SourceAdmission, additions: &mut VecDeque<HeaderEvent>) {
    for event in additions.drain(..) {
        admission.header(event);
    }
}

pub(super) fn merge(
    reference: &ChatFile,
    donor: &ChatFile,
    retain: &[SpeakerCode],
    strip_tiers: &[String],
    mut additions: OpeningAdditions,
    placement: Placement,
    selection: Option<&SourceBoundDonorSelection<'_>>,
) -> Result<MergeDraft, MergeError> {
    let retained = |speaker: &SpeakerCode| retain.contains(speaker);
    // Where donor opening metadata joins the reference: before the first body
    // line, or before `@End` when the reference has no body (a header-only
    // reference). `None` only for a document with neither, which gets the
    // metadata after its last line. A count of opening lines cannot express the
    // header-only case: it equals the line count, and the injection was skipped.
    let opening_end = reference
        .lines
        .iter()
        .position(|line| starts_body(line) || is_end(line));
    let last_id = reference
        .lines
        .iter()
        .take(opening_end.unwrap_or(reference.lines.len()))
        .enumerate()
        .rev()
        .find_map(|(index, line)| {
            matches!(line, Line::Header { header, .. } if matches!(header.as_ref(), Header::ID(_)))
                .then_some(index)
        });
    let mut reference_admission = SourceAdmission::new(placement);
    let mut reference_fates = Vec::new();
    for (index, line) in reference.lines.iter().enumerate() {
        if Some(index) == opening_end {
            inject_headers(&mut reference_admission, &mut additions.events);
        }
        match line {
            Line::Header {
                header,
                span,
                separator,
            } => {
                let mut event = copied_header(header, *span, *separator);
                if let Header::Participants { entries } = event.header.as_mut() {
                    let mut combined: Vec<_> = entries.iter().cloned().collect();
                    combined.extend(additions.participants.iter().cloned());
                    *entries = ParticipantEntries::new(combined);
                }
                reference_admission.header(event);
            }
            Line::Utterance(utterance) => {
                let origin =
                    MergeOrigin::Retained(ReferenceIdx(UtteranceIdx::new(reference_fates.len())));
                let fate = if retained(&utterance.main.speaker) {
                    reference_admission.utterance(utterance.clone(), origin)?;
                    ReferenceFate::Retained
                } else {
                    ReferenceFate::DroppedNotRetained {
                        speaker: utterance.main.speaker.clone(),
                    }
                };
                reference_fates.push(fate);
            }
        }
        if Some(index) == last_id {
            // Consume only the leading IDs from the one ordered metadata
            // queue. The constructor refuses an ID after a comment, so this
            // preserves donor order while satisfying CHAT's contiguous ID block.
            while additions
                .events
                .front()
                .is_some_and(|event| matches!(event.header.as_ref(), Header::ID(_)))
            {
                if let Some(event) = additions.events.pop_front() {
                    reference_admission.header(event);
                }
            }
        }
    }
    if opening_end.is_none() {
        inject_headers(&mut reference_admission, &mut additions.events);
    }
    let mut donor_admission = SourceAdmission::new(placement);
    let mut donor_fates = Vec::new();
    let donor_opening_end = donor
        .lines
        .iter()
        .take_while(|line| !starts_body(line))
        .count();
    let mut source_headers = selection.map(|selection| selection.headers().iter());
    for (index, line) in donor.lines.iter().enumerate() {
        match line {
            Line::Header {
                header,
                span,
                separator,
            } => {
                // The first utterance ends opening-metadata reconciliation,
                // even when that utterance is excluded by speaker authority.
                if let Some(contexts) = &mut source_headers {
                    let context = contexts.next().ok_or(MergeError::InvalidDonorSelection)?;
                    match context {
                        selection::HeaderContext::Opening | selection::HeaderContext::End => {}
                        selection::HeaderContext::Body(bracket) => {
                            donor_admission.events.push(PendingEvent::BoundSection(
                                SectionBoundary {
                                    header: copied_header(header, *span, *separator),
                                    previous_end: bracket.previous_end(),
                                    next_start: bracket.next_start(),
                                    order: OrderBounds {
                                        lower: bracket.previous_start(),
                                        upper: bracket.next_start(),
                                    },
                                },
                            ));
                        }
                    }
                } else if index >= donor_opening_end && !matches!(header.as_ref(), Header::End) {
                    donor_admission.header(copied_header(header, *span, *separator));
                }
            }
            Line::Utterance(utterance) => {
                let origin = MergeOrigin::Inserted(DonorIdx(UtteranceIdx::new(donor_fates.len())));
                let fate = if retained(&utterance.main.speaker) {
                    DonorFate::ExcludedByRetain
                } else {
                    let mut utterance = utterance.clone();
                    let before = utterance.dependent_tiers.len();
                    utterance
                        .dependent_tiers
                        .retain(|tier| !strip_tiers.iter().any(|kind| kind == tier.kind()));
                    let tiers_stripped = before - utterance.dependent_tiers.len();
                    donor_admission.utterance(utterance, origin)?;
                    DonorFate::Inserted { tiers_stripped }
                };
                donor_fates.push(fate);
            }
        }
    }
    AdmittedMerge {
        reference: reference_admission.finish(),
        donor: donor_admission.finish(),
        reference_fates,
        donor_fates,
        placement,
        relative_order: selection
            .map(|selected| selected.order_for(reference))
            .transpose()?
            .flatten(),
        timed_gem: selection
            .map(|selected| selected.gem_for(reference))
            .transpose()?
            .flatten(),
        draft_order: match selection {
            Some(selected) => selected.draft_policy_for(reference)?,
            None => super::draft_order::DraftOrderPolicy::Strict,
        },
    }
    .assemble()
}

#[cfg(test)]
mod section_order_tests {
    use super::*;

    #[test]
    fn section_order_requires_one_direction_not_two() {
        let fixture = crate::parse_and_validate(
            include_str!("../../../../corpus/reference/edge-cases/postcodes-and-gems.cha"),
            Default::default(),
        )
        .unwrap();
        let header = fixture.lines.iter().find(|line| {
            matches!(line, Line::Header { header, .. } if matches!(header.as_ref(), Header::BeginGem { .. }))
        }).unwrap();
        let boundary = |previous_end, next_start| {
            let Line::Header {
                header,
                span,
                separator,
            } = header.clone()
            else {
                panic!("selected fixture section header");
            };
            SectionBoundary {
                header: HeaderEvent {
                    header,
                    span,
                    separator,
                },
                previous_end: Some(previous_end),
                next_start: Some(next_start),
                order: OrderBounds {
                    lower: Some(0),
                    upper: Some(next_start),
                },
            }
        };
        let earlier = boundary(50, 50);
        let later = boundary(100, 100);
        assert!(matches!(
            earlier.precedes_section(&later),
            Ok(SectionOrder::Before)
        ));
        assert!(matches!(
            later.precedes_section(&earlier),
            Ok(SectionOrder::After)
        ));
        assert!(matches!(
            later.precedes_section(&boundary(100, 100)),
            Err(MergeError::AmbiguousSectionOrder { .. })
        ));
        assert!(matches!(
            boundary(40, 120).precedes_section(&later),
            Err(MergeError::AmbiguousSectionOrder { .. })
        ));
        assert!(matches!(
            boundary(120, 80).precedes_section(&later),
            Err(MergeError::AmbiguousSectionOrder { .. })
        ));
    }
}
