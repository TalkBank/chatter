//! Where document lowering puts each line, by plan phase.
//!
//! A decided plan lowers each utterance as the walk meets it
//! ([`DirectLines`]). A plan that waits for the document's headers has no
//! plan to lower utterances with until the last header is lowered, so the
//! walk holds them ([`HeldLines`]) and [`lower_held`] lowers them afterwards
//! under the decided plan, in file order.
use super::recovered_tier::RecoveredTier;
use super::tier_plan::{HeaderSelection, TierRouting};
use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, SourceBound, UtteranceNode};
use crate::model::Line;
use crate::parser::chat_file_parser::utterance_parser::parse_utterance_node;
use talkbank_model::ParseOutcome;

/// A line of a document whose plan waits for its headers: finished, or an
/// utterance held for lowering once the plan is decided.
pub(super) enum LoweringLine<'a> {
    Ready(Line),
    Pending {
        node: SourceBound<'a, 'a, UtteranceNode<'a>>,
        alignment_taint: bool,
        trailing_recovery: Vec<RecoveredTier>,
    },
}

/// Where the walk puts each lowered line, in the shape the plan's phase
/// allows. A decided plan lowers utterances as they are met
/// ([`DirectLines`]); an undecided one holds them until every header is
/// lowered ([`HeldLines`]), and has no plan to lower them with.
pub(super) trait LineSink<'a> {
    /// Append a finished line.
    fn push(&mut self, line: Line);
    /// Lower, or hold, the utterance at this line.
    fn utterance(&mut self, node: SourceBound<'a, 'a, UtteranceNode<'a>>);
    /// Taint the most recent utterance's dependent alignment, because a
    /// recovery region after it may have consumed one of its tiers.
    fn taint_previous_utterance(&mut self);
    /// Attach observed recovery only when its source directly follows the last
    /// utterance; return unowned evidence on refusal.
    fn append_recovered_tier(&mut self, tier: RecoveredTier) -> Result<(), RecoveredTier>;
}

/// Lines lowered under an already decided plan.
pub(super) struct DirectLines<'e, S, P> {
    lines: Vec<Line>,
    errors: &'e S,
    plan: P,
}

impl<'e, S: ErrorSink, P: TierRouting> DirectLines<'e, S, P> {
    pub(super) fn new(lines: Vec<Line>, errors: &'e S, plan: P) -> Self {
        Self {
            lines,
            errors,
            plan,
        }
    }

    /// The lowered lines and the plan that lowered them.
    pub(super) fn into_parts(self) -> (Vec<Line>, P) {
        (self.lines, self.plan)
    }
}

impl<'a, S: ErrorSink, P: TierRouting> LineSink<'a> for DirectLines<'_, S, P> {
    fn push(&mut self, line: Line) {
        self.lines.push(line);
    }

    fn utterance(&mut self, node: SourceBound<'a, 'a, UtteranceNode<'a>>) {
        if let ParseOutcome::Parsed(utterance) =
            parse_utterance_node(node, self.errors, &mut self.plan)
        {
            self.plan.push_utterance(&mut self.lines, utterance);
        }
    }

    fn append_recovered_tier(&mut self, tier: RecoveredTier) -> Result<(), RecoveredTier> {
        match self.lines.last_mut() {
            Some(Line::Utterance(utterance)) => append_to_utterance(utterance, tier),
            Some(Line::Header { .. }) | None => Err(tier),
        }
    }

    fn taint_previous_utterance(&mut self) {
        if let Some(utterance) = self.lines.iter_mut().rev().find_map(|line| match line {
            Line::Utterance(utterance) => Some(utterance),
            Line::Header { .. } => None,
        }) {
            utterance.mark_all_dependent_alignment_taint();
        }
    }
}

/// Lines held until the plan is decided from every lowered header.
pub(super) struct HeldLines<'a>(Vec<LoweringLine<'a>>);

impl<'a> LineSink<'a> for HeldLines<'a> {
    fn push(&mut self, line: Line) {
        self.0.push(LoweringLine::Ready(line));
    }

    fn utterance(&mut self, node: SourceBound<'a, 'a, UtteranceNode<'a>>) {
        self.0.push(LoweringLine::Pending {
            node,
            alignment_taint: false,
            trailing_recovery: Vec::new(),
        });
    }

    fn append_recovered_tier(&mut self, tier: RecoveredTier) -> Result<(), RecoveredTier> {
        match self.0.last_mut() {
            Some(LoweringLine::Ready(Line::Utterance(utterance))) => {
                append_to_utterance(utterance, tier)
            }
            Some(LoweringLine::Pending {
                node,
                trailing_recovery,
                ..
            }) if tier.starts_at(node.raw_node().end_byte()) => {
                trailing_recovery.push(tier);
                Ok(())
            }
            Some(LoweringLine::Ready(_) | LoweringLine::Pending { .. }) | None => Err(tier),
        }
    }

    fn taint_previous_utterance(&mut self) {
        if let Some(line) = self.0.iter_mut().rev().find(|line| {
            matches!(
                line,
                LoweringLine::Ready(Line::Utterance(_)) | LoweringLine::Pending { .. }
            )
        }) {
            match line {
                LoweringLine::Ready(Line::Utterance(utterance)) => {
                    utterance.mark_all_dependent_alignment_taint()
                }
                LoweringLine::Pending {
                    alignment_taint, ..
                } => *alignment_taint = true,
                LoweringLine::Ready(_) => {}
            }
        }
    }
}

impl<'a> HeldLines<'a> {
    pub(super) fn new(lines: Vec<LoweringLine<'a>>) -> Self {
        Self(lines)
    }

    /// Decide the plan from every lowered header, then lower the held
    /// utterances under it, in file order.
    pub(super) fn lower<P: TierRouting>(
        self,
        errors: &impl ErrorSink,
        select: HeaderSelection<'_, P>,
    ) -> (Vec<Line>, P) {
        let lines = self.0;
        // References into the final header values, not a second header parser
        // or cloned model. Held utterances retain their producer.
        let headers: Vec<_> = lines
            .iter()
            .filter_map(|line| match line {
                LoweringLine::Ready(Line::Header { header, .. }) => Some(header.as_ref()),
                LoweringLine::Ready(_) | LoweringLine::Pending { .. } => None,
            })
            .collect();
        let mut plan = select(&headers);
        let mut completed = Vec::with_capacity(lines.len());
        for line in lines {
            match line {
                LoweringLine::Ready(line) => completed.push(line),
                LoweringLine::Pending {
                    node,
                    alignment_taint,
                    trailing_recovery,
                } => match parse_utterance_node(node, errors, &mut plan) {
                    ParseOutcome::Parsed(mut utterance) => {
                        if alignment_taint {
                            utterance.utterance.mark_all_dependent_alignment_taint();
                        }
                        utterance
                            .utterance
                            .dependent_tiers
                            .extend(trailing_recovery.into_iter().map(RecoveredTier::into_entry));
                        plan.push_utterance(&mut completed, utterance);
                    }
                    ParseOutcome::Rejected => {}
                },
            }
        }
        (completed, plan)
    }
}

fn append_to_utterance(
    utterance: &mut talkbank_model::Utterance,
    tier: RecoveredTier,
) -> Result<(), RecoveredTier> {
    let end = match utterance.dependent_tiers.last() {
        Some(previous) => previous.tier.location(),
        None => utterance.main.location,
    }
    .span()
    .map(|span| span.end());
    if end.is_some_and(|end| tier.starts_at(end)) {
        utterance.dependent_tiers.push(tier.into_entry());
        Ok(())
    } else {
        Err(tier)
    }
}
