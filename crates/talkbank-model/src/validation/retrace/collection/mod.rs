//! Retrace leaf-kind collection orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::types::{LeafKind, RetraceCheck};
use crate::model::{BracketedContent, ContentStructure, LeafContent, MainTier};

/// Collect leaf classifications and retrace checkpoints from one main tier.
///
/// The returned `LeafKind` stream represents serialized content order; retrace
/// checks store the leaf index each retrace marker follows.
pub fn collect_retrace_checks(main_tier: &MainTier) -> (Vec<LeafKind>, Vec<RetraceCheck>) {
    let mut leaf_kinds = Vec::new();
    let mut retrace_checks = Vec::new();
    let mut retrace_index = 0usize;

    for item in main_tier.content.content.iter() {
        collect(
            item.structure(),
            &mut leaf_kinds,
            &mut retrace_checks,
            &mut retrace_index,
        );
    }

    if main_tier.content.terminator.is_some() {
        leaf_kinds.push(LeafKind::Terminator);
    }

    (leaf_kinds, retrace_checks)
}

/// Collect leaf kinds and retrace checkpoints from one classified item.
///
/// ONE function for both content enums. There were two, one per enum, and the
/// second's docstring said "Behavior mirrors utterance-level collection so
/// nested content and top-level content share identical retrace semantics",
/// which is a comment doing a type's job: sixteen leaf arms in two files that
/// had to agree, bound by nothing but that sentence.
///
/// `ContentStructure` is the parameter, so there is nothing left to mirror,
/// and the spoken/notation split it reads is the model's own.
fn collect(
    structure: ContentStructure<'_>,
    leaf_kinds: &mut Vec<LeafKind>,
    retrace_checks: &mut Vec<RetraceCheck>,
    retrace_index: &mut usize,
) {
    match structure {
        ContentStructure::Word(_) => leaf_kinds.push(LeafKind::RealContent),
        ContentStructure::Leaf(leaf) => leaf_kinds.push(match leaf.content {
            LeafContent::Spoken => LeafKind::RealContent,
            LeafContent::Notation => LeafKind::NonRealContent,
        }),
        ContentStructure::Group(group) => {
            collect_enclosed(group.content(), leaf_kinds, retrace_checks, retrace_index);
        }
        ContentStructure::Retrace(retrace) => {
            collect_enclosed(
                &retrace.inner().content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
            retrace_checks.push(RetraceCheck {
                retrace_index: *retrace_index,
                after_leaf_index: leaf_kinds.len(),
            });
            *retrace_index += 1;
        }
    }
}

/// Depth-first over enclosed content, preserving transcript order so leaf
/// indices stay compatible with retrace rendering and validation.
fn collect_enclosed(
    content: &BracketedContent,
    leaf_kinds: &mut Vec<LeafKind>,
    retrace_checks: &mut Vec<RetraceCheck>,
    retrace_index: &mut usize,
) {
    for item in content.content.iter() {
        collect(item.structure(), leaf_kinds, retrace_checks, retrace_index);
    }
}
