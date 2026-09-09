//! Observe every typed position the generated traversal classifies.
//!
//! The generated `extract_*` functions turn a node's flat child list into a
//! `<Rule>Children` carrier whose every position is a `NodeSlot`. This module
//! walks those carriers mechanically (the per-type `Inspect` impls in
//! [`inventory`] are generated from the traversal itself) and records what
//! each position held, as an [`Observation`]. Two consumers read the record:
//!
//! - the generator-conformance test, which asserts that on the valid
//!   reference corpus no position misclassifies a real child
//!   ([`is_violation`] is its policy), and
//! - the slot-state census (`examples/slot_state_census.rs`), which runs the
//!   same walk over every CHAT file in the repository and reports, per
//!   position, which states it has ever been seen in. A recovery arm in the
//!   hand-written parser is reachable from CHAT only if the census has seen
//!   its state at that position; the census is what turns "this arm is
//!   uncovered" into a verdict with evidence.
//!
//! Until 2026-09-09 the harness lived inside the conformance test binary and
//! recorded only violations, so the census could not exist without a second
//! copy of the walk.
//!
//! ## Why every field is inspected the SAME way regardless of shape
//!
//! The backend wraps every field in `Positioned<'tree, S>` where `S` is one
//! of a `NodeSlot` alias (required), `Option<..>` of one (optional), or
//! `Vec<Positioned<..>>` of one (repeat). `T` itself is always exactly one
//! of: a leaf node wrapper (its own node kind is separately visited by
//! [`walk_all`], so nothing to recurse into), a synthetic `*Children` group
//! with no grammar node of its own, or a self-classifying `*Choice` enum
//! whose variant payload is, recursively, one of these same three things.
//! [`InspectField`] dispatches on the CONTAINER shape exactly once and
//! recurses into the payload via [`Inspect`] regardless of which of the three
//! payload kinds it is; `Inspect` is what varies per generated type,
//! mechanically, in [`inventory`].

use crate::generated_traversal::{Absence, LeafSpan, NodeSlot, Positioned, RecoveryNode, SlotView};

pub mod inventory;

pub use inventory::dispatch;

/// How a position holds its child or children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Container {
    /// A required single position: exactly one slot.
    Required,
    /// An optional single position: a slot, or nothing.
    Optional,
    /// A repeat: zero or more slots.
    Repeat,
}

/// What one visit of a position found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Observed {
    /// A well-formed node of the expected type.
    Present,
    /// A tree-sitter MISSING placeholder.
    Missing,
    /// A tree-sitter ERROR node.
    Error,
    /// A present node fitting no matching position.
    Unexpected,
    /// No node where a slot exists.
    Absent,
    /// An optional position with no slot at all, or a repeat with no elements.
    Empty,
}

/// One visit of one position: the record both consumers aggregate.
///
/// It is the aggregation key as well as the per-visit record (deriving `Ord`
/// for that), so a map keys on it with no field-by-field copy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Observation {
    /// The rule (node kind) whose `extract_*` produced the position.
    pub rule_kind: &'static str,
    /// The carrier field (slot) the position is.
    pub slot: &'static str,
    /// How the position holds its children.
    pub container: Container,
    /// What was there.
    pub observed: Observed,
    /// The kind of the node found for a recovery state; empty for `Present`
    /// (the payload is typed, its kind is the position's) and for `Empty`.
    pub actual_child_kind: String,
}

/// The conformance policy: a required position holding anything but a
/// present node misclassified a real child, and so did an optional or repeat
/// position holding an unexpected one. Optional-absent and a recovery node
/// in an optional or repeat position are what error recovery legitimately
/// produces.
#[must_use]
pub fn is_violation(observation: &Observation) -> bool {
    match observation.container {
        Container::Required => observation.observed != Observed::Present,
        Container::Optional | Container::Repeat => observation.observed == Observed::Unexpected,
    }
}

/// Recursive inspection for a generated type: visit every position reachable
/// from `self` and record each. Implemented mechanically in [`inventory`] for
/// every leaf node wrapper (no-op), `*Choice` enum (dispatch into the present
/// variant), and `*Children` struct (recurse into each named field).
pub trait Inspect {
    /// Record every position reachable from `self` under rule `rule`.
    fn inspect(&self, rule: &'static str, out: &mut Vec<Observation>);
}

/// Field-level inspection, dispatched by the field's STATIC (container) type
/// so a `*Children` struct's per-field macro invocation never needs to know
/// whether a given field is required, optional or repeat.
pub trait InspectField {
    /// Record what the field `slot` of rule `rule` holds, and recurse into
    /// any present payload.
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<Observation>);
}

/// A tracked position: delegate straight to the slot or collection it
/// carries. The leading extras a position records are comments and
/// whitespace, never classified content.
impl<'tree, S: InspectField> InspectField for Positioned<'tree, S> {
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<Observation>) {
        self.slot().inspect_field(rule, slot, out);
    }
}

/// Record one slot in `container`, recursing into a present payload.
fn observe<'tree, T, M, U, A>(
    slot: &NodeSlot<'tree, T, M, U, A>,
    container: Container,
    rule: &'static str,
    name: &'static str,
    out: &mut Vec<Observation>,
) where
    T: Inspect,
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
    A: Absence,
{
    let (observed, actual_child_kind) = match slot.view() {
        SlotView::Present(value) => {
            value.inspect(rule, out);
            (Observed::Present, String::new())
        }
        SlotView::Missing(node) => (Observed::Missing, node.node().kind().to_string()),
        SlotView::Error(node) => (Observed::Error, node.kind().to_string()),
        SlotView::Unexpected(node) => (Observed::Unexpected, node.node().kind().to_string()),
        SlotView::Absent(absent) => absent.when_absent((Observed::Absent, String::new())),
    };
    out.push(Observation {
        rule_kind: rule,
        slot: name,
        container,
        observed,
        actual_child_kind,
    });
}

impl<'tree, T, M, U, A> InspectField for NodeSlot<'tree, T, M, U, A>
where
    T: Inspect,
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
    A: Absence,
{
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<Observation>) {
        observe(self, Container::Required, rule, slot, out);
    }
}

impl<'tree, T, M, U, A> InspectField for Option<NodeSlot<'tree, T, M, U, A>>
where
    T: Inspect,
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
    A: Absence,
{
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<Observation>) {
        match self {
            Some(inner) => observe(inner, Container::Optional, rule, slot, out),
            None => out.push(Observation {
                rule_kind: rule,
                slot,
                container: Container::Optional,
                observed: Observed::Empty,
                actual_child_kind: String::new(),
            }),
        }
    }
}

impl<'tree, T, M, U, A> InspectField for Vec<Positioned<'tree, NodeSlot<'tree, T, M, U, A>>>
where
    T: Inspect,
    M: RecoveryNode<'tree>,
    U: RecoveryNode<'tree>,
    A: Absence,
{
    fn inspect_field(&self, rule: &'static str, slot: &'static str, out: &mut Vec<Observation>) {
        if self.is_empty() {
            out.push(Observation {
                rule_kind: rule,
                slot,
                container: Container::Repeat,
                observed: Observed::Empty,
                actual_child_kind: String::new(),
            });
        }
        for element in self {
            observe(element.slot(), Container::Repeat, rule, slot, out);
        }
    }
}

/// An absorbed sequence-member token: not a child position, nothing to
/// record.
impl InspectField for LeafSpan<'_> {
    fn inspect_field(&self, _rule: &'static str, _slot: &'static str, _out: &mut Vec<Observation>) {
    }
}

/// Walk a tree-sitter tree, calling `callback` on every node.
pub fn walk_all<'tree, F>(node: tree_sitter::Node<'tree>, callback: &mut F)
where
    F: FnMut(tree_sitter::Node<'tree>),
{
    callback(node);
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all(cursor.node(), callback);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
