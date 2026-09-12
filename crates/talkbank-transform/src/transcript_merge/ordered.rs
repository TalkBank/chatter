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

impl SectionBoundary {
    fn has_ordered_neighbors(&self) -> bool {
        !matches!((self.previous_end, self.next_start), (Some(before), Some(after)) if before > after)
    }

    fn precedes(&self, utterance: &PositionedUtterance) -> Result<bool, MergeError> {
        if self.has_ordered_neighbors() {
            if matches!((self.next_start, utterance.start), (Some(next), Some(start)) if start >= next) {
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

    fn precedes_section(&self, other: &Self) -> Result<bool, MergeError> {
        if self.has_ordered_neighbors()
            && other.has_ordered_neighbors()
            && matches!((self.next_start, other.previous_end), (Some(left), Some(right)) if left <= right)
        {
            return Ok(true);
        }
        if self.has_ordered_neighbors()
            && other.has_ordered_neighbors()
            && matches!((other.next_start, self.previous_end), (Some(left), Some(right)) if left <= right)
        {
            return Ok(false);
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
        let timing = utterance.main.content.bullet.as_ref().map(|bullet| &bullet.timing);
        if self.placement == Placement::Timed && timing.is_none() {
            return Err(MergeError::UnpositionedUtterance { origin });
        }
        let start = timing.map(|timing| timing.start_ms);
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
                order: OrderBounds { lower, upper: start },
            }));
        Ok(())
    }

    fn finish(self) -> OrderedSource {
        let mut events = VecDeque::with_capacity(self.events.len());
        let mut next_start = None;
        for event in self.events.into_iter().rev() {
            let admitted = match event {
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
                    order: OrderBounds { lower: previous_start, upper: next_start },
                }),
                PendingEvent::Utterance(mut utterance) => {
                    utterance.order.upper = utterance.start.or(next_start);
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
}

impl AdmittedMerge {
    fn assemble(mut self) -> Result<Merged, MergeError> {
        let mut lines = Vec::new();
        let mut origins = Vec::new();
        loop {
            let reference_next = match (self.reference.events.peek(), self.donor.events.peek()) {
                (None, None) => break,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                // End belongs after the donor's remaining source events.
                (Some(SourceEvent::End(_)), Some(_)) => false,
                (Some(SourceEvent::Header(_)), Some(_)) => true,
                (
                    Some(SourceEvent::Section(_)),
                    Some(SourceEvent::Header(_) | SourceEvent::End(_)),
                ) => false,
                (Some(SourceEvent::Section(reference)), Some(SourceEvent::Section(donor))) => {
                    match self.placement {
                        Placement::Timed => reference.precedes_section(donor)?,
                        Placement::SourceOrder => reference.order.precedes(&donor.order).ok_or_else(|| MergeError::AmbiguousSectionOrder {
                            reference: reference.header.header.to_chat_string(),
                            donor: donor.header.header.to_chat_string(),
                        })?,
                    }
                }
                (Some(SourceEvent::Section(header)), Some(SourceEvent::Utterance(utterance))) => {
                    match self.placement {
                        Placement::Timed => header.precedes(utterance)?,
                        Placement::SourceOrder => header.order.precedes(&utterance.order).ok_or_else(|| MergeError::AmbiguousSectionPlacement {
                            header: header.header.header.to_chat_string(), previous_end: header.previous_end,
                            next_start: header.next_start, competing: utterance.origin,
                        })?,
                    }
                }
                (Some(SourceEvent::Utterance(utterance)), Some(SourceEvent::Section(header))) => {
                    match self.placement {
                        Placement::Timed => !header.precedes(utterance)?,
                        Placement::SourceOrder => !header.order.precedes(&utterance.order).ok_or_else(|| MergeError::AmbiguousSectionPlacement {
                            header: header.header.header.to_chat_string(), previous_end: header.previous_end,
                            next_start: header.next_start, competing: utterance.origin,
                        })?,
                    }
                }
                (
                    Some(SourceEvent::Utterance(_)),
                    Some(SourceEvent::Header(_) | SourceEvent::End(_)),
                ) => false,
                (Some(SourceEvent::Utterance(reference)), Some(SourceEvent::Utterance(donor))) => {
                    match self.placement {
                        Placement::Timed => reference.start <= donor.start,
                        Placement::SourceOrder => reference.order.precedes(&donor.order).ok_or(MergeError::AmbiguousUtteranceOrder {
                            reference: reference.origin, donor: donor.origin,
                        })?,
                    }
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
                        lines.push(Line::Utterance(positioned.utterance));
                        origins.push(positioned.origin);
                    }
                }
            }
        }
        let errors = talkbank_model::ErrorCollector::new();
        let participants =
            talkbank_model::model::participant::join::build_participants_from_lines(&lines)
                .report_into(&errors);
        let diagnostics = errors.into_vec();
        if !diagnostics.is_empty() {
            return Err(MergeError::InvalidParticipantJoin { diagnostics });
        }
        let file = ChatFile::with_participants(lines, participants)
            .validate_with_policy(
                talkbank_model::validation::ValidationPolicy::new(
                    talkbank_model::RuleSelection::new(),
                    talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
                ),
                &talkbank_model::NullErrorSink,
                talkbank_model::model::TranscriptName::Anonymous,
            )
            .map_err(|failure| MergeError::InvalidOutput(Box::new(failure)))?;
        Ok(Merged {
            file,
            origins,
            reference_fates: self.reference_fates,
            donor_fates: self.donor_fates,
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
) -> Result<Merged, MergeError> {
    let retained = |speaker: &SpeakerCode| retain.contains(speaker);
    let opening_end = reference
        .lines
        .iter()
        .position(starts_body)
        .unwrap_or(reference.lines.len());
    let last_id = reference
        .lines
        .iter()
        .take(opening_end)
        .enumerate()
        .rev()
        .find_map(|(index, line)| {
            matches!(line, Line::Header { header, .. } if matches!(header.as_ref(), Header::ID(_)))
                .then_some(index)
        });
    let mut reference_admission = SourceAdmission::new(placement);
    let mut reference_fates = Vec::new();
    for (index, line) in reference.lines.iter().enumerate() {
        if index == opening_end {
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
    let mut donor_admission = SourceAdmission::new(placement);
    let mut donor_fates = Vec::new();
    let donor_opening_end = donor
        .lines
        .iter()
        .position(starts_body)
        .unwrap_or(donor.lines.len());
    for (index, line) in donor.lines.iter().enumerate() {
        match line {
            Line::Header {
                header,
                span,
                separator,
            } => {
                // The first utterance ends opening-metadata reconciliation,
                // even when that utterance is excluded by speaker authority.
                if index >= donor_opening_end && !matches!(header.as_ref(), Header::End) {
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
    }
    .assemble()
}
