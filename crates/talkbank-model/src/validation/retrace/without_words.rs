//! A retracing marker applied to material that contains no words.
//!
//! A marker retraces the WORDS immediately to its left. A laugh is not a word,
//! so `&=laughs [//] water` has nothing for the marker to refer to.
//!
//! Brian MacWhinney ruled this on 2026-08-07, shown the corpus line
//! `*PAR: <the floor on the> [//] &=laughs [//] water [//] the floor on the xxx .`
//! and asked whether `&=laughs [//]` means anything: **"No, not legal. You
//! can't retrace a laugh."**
//!
//! # Why "contains no word" and not "is an event"
//!
//! Half an hour earlier in the same thread he gave the form that IS legal: put
//! the laugh inside material that has words, and retrace that.
//!
//! ```text
//! *PAR:	<the floor on the &=laughs water> [//] the floor on the xxx .
//! ```
//!
//! So the event is not the problem; the absence of words is. A rule phrased
//! against `Event` would reject the shape the maintainer himself proposed.
//!
//! # Why "at any depth"
//!
//! 205 corpus retraces enclose an annotated group or a quotation and so have no
//! DIRECT word child, while holding words one level down:
//!
//! ```text
//! *CHI:	<<the dog> [?]> [/] the dog .
//! ```
//!
//! A rule testing the immediate children would reject all 205. The predicate
//! therefore recurses, through the shared `ContentStructure` classification so
//! that it cannot disagree with the traversal that reaches it.
//!
//! # Relationship to E377
//!
//! Disjoint, despite the similar names. E377 catches a marker whose content is
//! a lone marker (`a [//] [/] a`), and in that shape the inner retrace still
//! holds words, so this rule stays silent. This one catches material with no
//! words anywhere, and in that shape there is no second marker. Two different
//! mistakes with two different repairs: E377 says drop one marker, E378 says
//! retrace the words instead of the vocalization.
//!
//! # Scope in the corpora
//!
//! Small and concentrated: **15 instances across 12 files**, measured by
//! running this rule over all 107,376 corpus files. Attested shapes are a
//! repeated vocalization (`<&=sigh> [/] &=sigh`, `<&=eh> [/] <&=eh> [/] &=eh`),
//! a bare event (`&=laughs [//] water`), and a zero-word carrying only a
//! paralinguistic annotation (`0 [=! skratt] [/]`).
//!
//! Two EARLIER figures for this rule, 7 and then 14, were both produced by
//! locating candidate files with `rg` and validating only those. A locate is a
//! hypothesis about what a rule catches, and both hypotheses were wrong: the
//! first predated events being lowered as retraces at all, and the second
//! searched for events, so it could not see the zero-word case. Only the rule
//! knows what the rule catches. Do not re-derive this number from a search.
//! Evidence: `docs/investigations/2026-08-07-retrace-shapes-in-the-wild.md`.

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::{BracketedContent, Retrace, Word};

use crate::{ErrorCode, ErrorSink};

/// Report `retrace` if the material it retraces contains no word at any depth.
pub(super) fn report_if_no_words_retraced(retrace: &Retrace, errors: &impl ErrorSink) {
    if contains_word(&retrace.content) {
        return;
    }

    errors.report(super::retrace_error(
        ErrorCode::RetraceWithoutWords,
        retrace.span,
        format!(
            "the {} marker retraces material with no words in it; a marker \
             retraces the words to its left",
            retrace.kind
        ),
        "Retrace the words instead, with the event inside them \
         (<the floor &=laughs water> [//] ...), or remove the marker",
        "nothing here is a word",
    ));
}

/// Whether `content` holds a word anywhere beneath it.
///
/// The recursion itself lives on [`ContentStructure::any_word`], because three
/// validators grew the same walk with three different word predicates within a
/// week. That divergence is what silently disabled E377 inside `‹...›`.
fn contains_word(content: &BracketedContent) -> bool {
    let any = |_: &Word| true;
    content
        .content
        .iter()
        .any(|item| item.structure().any_word(&any))
}
