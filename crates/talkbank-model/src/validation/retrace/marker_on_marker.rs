//! A retracing marker applied to nothing but another retracing marker.
//!
//! A marker retraces the material immediately to its left, and a marker is not
//! material. So `на [//] [/] на` has nothing for the second marker to refer to.
//! Brian MacWhinney ruled it an error on 2026-08-07, shown a corpus line:
//! "clearly a mistake", and to the direct question, "It's an error".
//!
//! # One rule, both spellings
//!
//! The parser folds a marker run into a left-associative chain, one wrapper per
//! marker, so the unbracketed `a [//] [/]` and the bracketed `<<a> [/]> [//]`
//! produce the SAME shape: a retrace whose content is a lone retrace. That is
//! why this is one check rather than two, and why it also covers the re2c
//! backend, which the parser-level refusal it replaces did not.
//!
//! # What this must NOT catch
//!
//! A retrace whose scope merely COVERS earlier disfluency is ordinary and
//! common:
//!
//! ```text
//! *PAR:	<the [/] the piece> [//] the people .
//! ```
//!
//! The speaker stuttered, then replaced the whole stuttered stretch. A typed
//! scan of all 106,480 corpus files found 2,650,099 retraces, of which 11,163
//! sit inside another retrace and only **4** are a marker over a lone marker.
//! A rule reading "no `Retrace` inside a `Retrace`" would invalidate the other
//! 11,159, concentrated in the aphasia and fluency corpora whose subject this
//! is. Hence the narrow test: the content must be a lone retrace, with no
//! material of its own.
//!
//! Whole-corpus population, measured by running this rule over all 107,376
//! files rather than over a located subset: **53 instances across 42 files**.
//! An earlier figure of 49 across 38 came from validating a byte-located file
//! list and undercounted, the same way two successive figures for the sibling
//! E378 rule did.
//! Evidence: `docs/investigations/2026-08-07-retrace-shapes-in-the-wild.md`.

// A CHAT speaker prefix is followed by a literal TAB, so the tabs in the
// examples below are the format being described, not indentation. Corrupting
// them to spaces would make the doc show invalid CHAT.
#![allow(clippy::tabs_in_doc_comments)]
// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
use crate::model::{ContentStructure, Retrace};

use crate::{ErrorCode, ErrorSink};

/// Report `retrace` if its content is another retrace and nothing else.
///
/// The predicate is inline rather than a separate function: it had one caller,
/// which early-returned on it, so the split bought a jump between two places to
/// learn one rule and a name (`report`) that lied about being conditional.
pub(super) fn report_if_marker_on_marker(retrace: &Retrace, errors: &impl ErrorSink) {
    // Exactly one item, and that item is itself a retrace in either form. Two
    // items means the marker has material of its own, which is the legitimate
    // shape and by far the common one.
    let [only] = retrace.content.content.as_slice() else {
        // Two or more items means the marker has material of its own, which is
        // the legitimate shape and by far the common one.
        return;
    };
    // Asks the shared classifier rather than listing the two retrace spellings,
    // so this rule cannot develop its own opinion about the content enum. This
    // file is the one that HAD a divergent opinion (`PhoGroup`/`SinGroup` as
    // leaves), which is what `model::content::structure` was written to end.
    if !matches!(only.structure(), ContentStructure::Retrace(_)) {
        return;
    }

    errors.report(super::retrace_error(
        ErrorCode::RetraceWithNoMaterial,
        retrace.span,
        format!(
            "the {} marker retraces another retracing marker, which is not \
             material; a marker retraces the words to its left",
            retrace.kind
        ),
        "Put the repeated or corrected words between the two markers, or remove one of them",
        "second retracing marker",
    ));
}
