//! Supertype matcher for scoped/base annotation node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

/// Whether `kind` is one of the `base_annotation` supertype's members.
///
/// The list is the grammar's `base_annotation` choice, and it MUST be updated
/// when that choice gains a member. On 2026-08-25 it had drifted both ways at
/// once: it named three kinds the choice does not contain
/// (`duration_annotation`, `retrace_uncertain`, `scoped_best_guess`, removed
/// here) and omitted `code_switch_annotation`, which the grammar had just
/// gained, so the parser accepted `<...> [@s]` and this predicate rejected it.
///
/// DERIVING IT FROM THE GENERATED TRAVERSAL WAS TRIED AND BACKED OUT, and the
/// reason is worth keeping. `extract_base_annotation` answers "is this node a
/// PRESENT member", which is a different question from "is this KIND a member".
/// The two diverge exactly on recovery nodes: a MISSING `retrace_complete`
/// still has that kind and still belongs here, but classifies as
/// `NodeSlot::Missing`. Deriving therefore turned the CHECK 51 fixture
/// (`<hello there>` with no annotation) from one diagnostic into two, the added
/// one reading "expected annotation, found 'retrace_complete'" about a kind
/// that IS an annotation.
///
/// THE LIST DOES EXIST AS DATA, and an earlier draft of this doc denied it:
/// generated `grammar/src/node-types.json` carries `base_annotation.subtypes`,
/// exactly these kinds, checked in and regenerated with the grammar. It
/// is not a node classifier, so the missing-node objection above does not reach
/// it. What is absent is a stable Rust EXPORT: the generated traversal keeps
/// its copy only as a positional private `static`. Having the generator emit
/// one would delete this list, this doc, and the drift class for every
/// supertype rather than just this one, and that is the real fix whenever the
/// generator is next touched.
///
/// What guards this list is behaviour rather than a second copy of it, but the
/// coverage is PARTIAL and worth stating: only
/// `retrace_complete` and `code_switch_annotation` have construct specs, so
/// only those two are parsed through a real file. That is exactly why the
/// omission of `code_switch_annotation` was caught here and why an equivalent
/// omission elsewhere would not be. A unit test asserting kind-by-kind
/// membership used to exist, did NOT catch this drift, and was deleted rather
/// than extended: it compared the list to a copy of itself.
#[must_use]
pub fn is_base_annotation(kind: &str) -> bool {
    matches!(
        kind,
        "base_annotation" |  // Keep for backwards compatibility (supertype wrapper)
        "alt_annotation" |
        "code_switch_annotation" |
        "error_marker_annotation" |
        "exclude_marker" |
        "explanation_annotation" |
        "indexed_overlap_follows" |
        "indexed_overlap_precedes" |
        "para_annotation" |
        "percent_annotation" |
        "retrace_complete" |
        "retrace_multiple" |
        "retrace_partial" |
        "retrace_reformulation" |
        "scoped_contrastive_stressing" |
        "scoped_stressing" |
        "scoped_uncertain"
    )
}
