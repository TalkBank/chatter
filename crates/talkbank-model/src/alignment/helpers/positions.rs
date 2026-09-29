//! Typed payloads emitted by the single positional alignment traversal.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::model::{Action, Annotated, Pause, Separator, Word};

use super::count::TierPosition;
use super::descent::AtomicUnit;
use super::domain::PositionalDomain;
use super::measurement::{AtomicFor, Mor, Pho, Sin, TraversalDomain};
use super::rules::is_tag_marker_separator;
use super::to_chat_display_string as to_string;

pub(super) enum PositionLeaf<'a> {
    Separator(&'a Separator),
    Pause(&'a Pause),
    Action(&'a Action),
    AnnotatedAction(&'a Annotated<Action>),
}

pub(super) enum SignAction<'a> {
    Plain(&'a Action),
    Annotated(&'a Annotated<Action>),
}

/// A domain owns both leaf admission and its resulting payload type. In
/// particular, a phonology position has no representation for a sign action.
pub(super) trait PositionDomain: TraversalDomain + Copy {
    type Other<'a>;
    const POSITIONAL: PositionalDomain;
    /// Emit only admitted payloads; the traversal does not re-test a domain's
    /// admission result or manufacture a position for an excluded leaf.
    fn emit_leaf<'a>(leaf: PositionLeaf<'a>, sink: &mut impl FnMut(AlignablePosition<'a, Self>));
    fn render_atomic(atomic: AtomicFor<'_, Self>) -> TierPosition;
    fn render_other(other: Self::Other<'_>) -> TierPosition;
}

impl PositionDomain for Mor {
    type Other<'a> = &'a Separator;
    const POSITIONAL: PositionalDomain = PositionalDomain::Mor;

    fn emit_leaf<'a>(leaf: PositionLeaf<'a>, sink: &mut impl FnMut(AlignablePosition<'a, Self>)) {
        match leaf {
            PositionLeaf::Separator(sep) => {
                if is_tag_marker_separator(sep) {
                    sink(AlignablePosition::Other(sep));
                }
            }
            PositionLeaf::Pause(_) | PositionLeaf::Action(_) | PositionLeaf::AnnotatedAction(_) => {
            }
        }
    }

    fn render_atomic(never: AtomicFor<'_, Self>) -> TierPosition {
        match never {}
    }

    fn render_other(sep: &Separator) -> TierPosition {
        TierPosition {
            text: to_string(sep),
            description: None,
        }
    }
}

impl PositionDomain for Pho {
    type Other<'a> = &'a Pause;
    const POSITIONAL: PositionalDomain = PositionalDomain::Pho;

    fn emit_leaf<'a>(leaf: PositionLeaf<'a>, sink: &mut impl FnMut(AlignablePosition<'a, Self>)) {
        match leaf {
            PositionLeaf::Pause(pause) => sink(AlignablePosition::Other(pause)),
            PositionLeaf::Separator(_)
            | PositionLeaf::Action(_)
            | PositionLeaf::AnnotatedAction(_) => {}
        }
    }

    fn render_atomic(group: AtomicFor<'_, Self>) -> TierPosition {
        render_atomic(AtomicUnit::Pho(group))
    }

    fn render_other(pause: &Pause) -> TierPosition {
        TierPosition {
            text: to_string(pause),
            description: Some("pause".to_string()),
        }
    }
}

impl PositionDomain for Sin {
    type Other<'a> = SignAction<'a>;
    const POSITIONAL: PositionalDomain = PositionalDomain::Sin;

    fn emit_leaf<'a>(leaf: PositionLeaf<'a>, sink: &mut impl FnMut(AlignablePosition<'a, Self>)) {
        match leaf {
            PositionLeaf::Action(action) => {
                sink(AlignablePosition::Other(SignAction::Plain(action)))
            }
            PositionLeaf::AnnotatedAction(action) => {
                sink(AlignablePosition::Other(SignAction::Annotated(action)))
            }
            PositionLeaf::Separator(_) | PositionLeaf::Pause(_) => {}
        }
    }

    fn render_atomic(group: AtomicFor<'_, Self>) -> TierPosition {
        render_atomic(AtomicUnit::Sin(group))
    }

    fn render_other(action: SignAction<'_>) -> TierPosition {
        let text = match action {
            SignAction::Plain(action) => to_string(action),
            SignAction::Annotated(action) => to_string(action),
        };
        TierPosition {
            text,
            description: Some("action".to_string()),
        }
    }
}

fn render_atomic(unit: AtomicUnit<'_>) -> TierPosition {
    TierPosition {
        text: unit.display_text(),
        description: Some(unit.description().to_string()),
    }
}

/// Producer-selected positions; counts and display are sinks on the same walk.
pub(super) enum AlignablePosition<'a, D: PositionDomain> {
    Word(&'a Word),
    Atomic(AtomicFor<'a, D>),
    Other(D::Other<'a>),
}

impl<D: PositionDomain> AlignablePosition<'_, D> {
    pub(super) fn into_tier_position(self) -> TierPosition {
        match self {
            Self::Word(word) => TierPosition {
                text: to_string(word),
                description: None,
            },
            Self::Atomic(atomic) => D::render_atomic(atomic),
            Self::Other(other) => D::render_other(other),
        }
    }
}
