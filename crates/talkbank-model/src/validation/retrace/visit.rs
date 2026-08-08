//! One traversal that reaches every retrace node on a main tier.
//!
//! # Why this exists
//!
//! The rules under this module answer different questions ("does any retrace
//! wrap nothing but a marker?", "does any retrace enclose no words?", "is there
//! a retrace at all?") over the SAME set of nodes, and each used to carry its
//! own hand-written pair of exhaustive matches. The two drifted: one listed
//! `PhoGroup` and `SinGroup` as leaves while its sibling recursed into them, so
//! a marker-on-marker inside `‹...›` escaped its rule entirely.
//!
//! Nothing could have caught that. The walkers were separate code with no
//! shared definition of "container", so the disagreement was invisible to the
//! compiler and to every test; it was found by reading the two leaf-sets
//! against each other.
//!
//! The leaf sets themselves now live in `model::content::structure`, which owns
//! "which variants contain other content" for the rules in this module. This
//! file is what remains once that knowledge is not duplicated here: the
//! retrace-specific traversal order and nothing else.
//!
//! # What it does NOT replace
//!
//! `collection` and `rendering` also walk this tree and are deliberately left
//! alone: `collection` interleaves retraces with LEAF indices, and `rendering`
//! must emit text for every item including bracket punctuation. Neither is a
//! retrace visitor with extra steps, so folding them in here would mean a
//! traversal that yields everything, which is a different and much larger
//! design (see the note in `alignment::helpers::walk`, whose `ContentItem` has
//! no container variants for exactly this reason).
//!
//! # Why there is no early exit
//!
//! An earlier version threaded `ControlFlow` through three signatures so that a
//! caller wanting only "is there ANY retrace" could stop at the first one. No
//! such caller was ever written: `check_retraces` needs every retrace anyway,
//! for the two per-node rules, and answers the existence question with a flag
//! set inside the same walk. The apparatus was justified by a docstring rather
//! than by a caller, which is the thing this module's own history warns about,
//! so it is gone. Reinstate it when a caller genuinely needs it, not before.

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::{ContentStructure, MainTier, Retrace};

/// Visit every retrace on the tier, outermost first, including retraces nested
/// inside another retrace's content.
pub(super) fn visit_every_retrace(main_tier: &MainTier, visit: &mut impl FnMut(&Retrace)) {
    for item in main_tier.content.content.iter() {
        visit_structure(item.structure(), visit);
    }
}

/// Report this item if it is a retrace, then descend into whatever it encloses.
///
/// Both content enums classify into the same [`ContentStructure`], which is why
/// the main-tier and bracketed levels share this one function instead of the
/// near-identical pair they used to be. That pair is how `PhoGroup` came to be
/// a container in one copy and a leaf in the other.
fn visit_structure(structure: ContentStructure<'_>, visit: &mut impl FnMut(&Retrace)) {
    if let ContentStructure::Retrace(retrace) = structure {
        visit(retrace);
    }
    if let Some(content) = structure.enclosed() {
        for item in content.content.iter() {
            visit_structure(item.structure(), visit);
        }
    }
}
