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
//! # Why the descent is not written here any more
//!
//! It was, twice over: a `visit_structure` that tested for a retrace and then
//! re-derived the `enclosed()` loop that `ContentStructure` already owns the
//! definition of. Three other copies of that loop existed elsewhere. Descent
//! is `ContentStructure::walk` now, and this module supplies only the part
//! that is about retraces.
//!
//! An earlier version of that walk threaded `ControlFlow` for a caller that
//! did not exist, and was rightly deleted. The nested-quotation rule became
//! that caller in August 2026, which is why `walk` carries [`Descend`]: this
//! module wants every node and answers `Into` every time.

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::{ContentStructure, Descend, MainTier, Retrace};

/// Visit every retrace on the tier, outermost first, including retraces nested
/// inside another retrace's content.
pub(super) fn visit_every_retrace(main_tier: &MainTier, visit: &mut impl FnMut(&Retrace)) {
    for item in main_tier.content.content.iter() {
        item.structure().walk(&mut |structure| {
            if let ContentStructure::Retrace(retrace) = structure {
                visit(retrace.inner());
            }
            Descend::Into
        });
    }
}
