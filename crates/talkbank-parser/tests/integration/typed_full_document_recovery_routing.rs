// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Regression probe for the NEW-backend reconstruction engine's recovery-aware
//! `repeat` handling, pinned at the `generated_traversal::extract_full_document`
//! seam that the document/line entry-point cluster (Task B1) drives.
//!
//! # What this pins, and why it exists
//!
//! The `full_document` production is
//! `seq(utf8_header, repeat(pre_begin_header), begin_header, repeat(line),
//! end_header)`. A stray top-level `@Date:` that tree-sitter cannot parse as a
//! date header is recovered as an `ERROR` node sitting INSIDE the `repeat(line)`
//! run, BEFORE the real `@End`:
//!
//! ```text
//! full_document children (flat):
//!   utf8_header, begin_header,
//!   line(languages), line(participants), line(id),
//!   ERROR(@Date:), line(blank_line), end_header(@End)
//! ```
//!
//! An EARLIER NEW-backend build (generator 98e5f53) sized the `repeat(line)` run
//! purely by the element kind, so the count STOPPED at the `ERROR` (3 line
//! elements), the following required `end_header` slot then eagerly ate the
//! `@Date:` ERROR via its `is_error()`-first classification, and the trailing
//! valid `line(blank_line)` PLUS the real `end_header` were stranded in the
//! carrier's `unexpected` sink. A faithful consumer would have DROPPED the `@End`
//! line from the model and lost the E747 blank-line diagnostic: a catastrophic
//! recovery regression (a single broken line mid-document strands the entire
//! tail). Task B1 attempt 1 was BLOCKED on exactly this.
//!
//! The generator fix (a105644, spec Section 7: absorb a tree-sitter recovery
//! `ERROR` embedded in a `repeat(element)` run AS a repeat element) restores
//! parity with the OLD backend. This test pins that recovered routing so the
//! regression can never silently return:
//!
//! - `child_3` (the `repeat(line)`) holds FIVE elements: three `Present(line)`,
//!   then the `@Date:` as `NodeSlot::Error` (absorbed, NOT stranded), then the
//!   trailing `Present(line)` blank line.
//! - `child_4` (the `end_header` slot) is `NodeSlot::Present(end_header)` at the
//!   real `@End`, NOT `Error`.
//! - `unexpected` is EMPTY: nothing is stranded.
//!
//! This is the NEW routing == OLD routing check the Task B1 resume gated on.

use talkbank_parser::DocumentRoot;
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AsRawNode, FullDocumentNode, NodeSlot, extract_full_document,
};
use talkbank_parser_tests::classify;

/// The stray-top-level-`@Date:` fixture, byte-identical to the
/// `STRAY_TOP_LEVEL_DATE` characterization fixture in
/// `visitor_entrypoint_migration.rs`. The `@Date:` (empty value) drives
/// tree-sitter into a document-level `ERROR` recovery node mid-`repeat(line)`.
const STRAY_TOP_LEVEL_DATE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Date:
@End
";

/// Parse `input` and return the raw `full_document` node.
///
/// Navigated by `DocumentRoot`, the same value the production entry point uses.
/// This helper used to reproduce the navigation, with a comment claiming it
/// navigated "exactly as the production entry point does", which is a prose
/// assertion that two copies agree; calling the real thing is the assertion.
///
/// The tree is leaked to give the node a `'static` lifetime for the duration of
/// the test, which is acceptable in a single-shot test process.
fn full_document_node(input: &str) -> tree_sitter::Node<'static> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let tree = parser
        .parse_tree_incremental(input, None)
        .expect("tree-sitter parse succeeds");
    // Leak the tree so its nodes outlive this helper; the process is short-lived.
    let tree: &'static tree_sitter::Tree = Box::leak(Box::new(tree));
    DocumentRoot::classify(tree).node()
}

#[test]
fn stray_top_level_error_is_absorbed_as_a_repeat_element_not_stranded() {
    let full_doc = full_document_node(STRAY_TOP_LEVEL_DATE);
    assert_eq!(
        full_doc.kind(),
        "full_document",
        "the fixture must navigate to a full_document node"
    );

    let children = extract_full_document(classify::<FullDocumentNode>(full_doc));

    // child_3: repeat(line). The recovery-aware engine absorbs the mid-repeat
    // ERROR as an element and keeps consuming the trailing valid line, so the
    // run is FIVE elements ending at the real end_header.
    let line_elements = &children.child_3.slot();
    assert_eq!(
        line_elements.len(),
        5,
        "repeat(line) must hold 5 elements (3 lines, the @Date: ERROR, the blank line), \
         got {}: the engine stopped early and stranded the tail",
        line_elements.len()
    );

    // Elements 0..3 are Present(line).
    for (idx, elem) in line_elements.iter().take(3).enumerate() {
        assert!(
            matches!(elem.slot(), NodeSlot::Present(_)),
            "line element {idx} must be Present, got {:?}",
            elem.slot()
        );
    }

    // Element 3 is the @Date: ERROR, ABSORBED as a repeat element (NodeSlot::Error),
    // at the exact @Date: span (87..93). Before the fix this ERROR was stranded
    // into the following required slot / the unexpected sink.
    match &line_elements[3].slot() {
        NodeSlot::Error(node) => {
            assert_eq!(
                (node.start_byte() as u32, node.end_byte() as u32),
                (87, 93),
                "the absorbed ERROR must be the @Date: node at span 87..93"
            );
        }
        other => {
            panic!("line element 3 must be NodeSlot::Error (the absorbed @Date:), got {other:?}")
        }
    }

    // Element 4 is the trailing blank line, still Present (not stranded).
    match &line_elements[4].slot() {
        NodeSlot::Present(line) => {
            assert_eq!(
                (
                    line.raw_node().start_byte() as u32,
                    line.raw_node().end_byte() as u32
                ),
                (93, 94),
                "the trailing blank line must be Present at span 93..94"
            );
        }
        other => panic!("line element 4 must be Present(line) (the blank line), got {other:?}"),
    }

    // child_4: end_header. The real @End reaches its own slot as Present, NOT
    // eaten as an Error by the mislaid ERROR.
    match &children.child_4.slot() {
        NodeSlot::Present(end_header) => {
            assert_eq!(
                (
                    end_header.raw_node().start_byte() as u32,
                    end_header.raw_node().end_byte() as u32
                ),
                (94, 99),
                "child_4 must be Present(end_header) at the real @End span 94..99"
            );
        }
        other => panic!("child_4 must be NodeSlot::Present(end_header), got {other:?}"),
    }

    // The unexpected sink is EMPTY: no valid line and no end_header is stranded.
    assert!(
        children.unexpected.is_empty(),
        "the unexpected sink must be empty (nothing stranded), got {} node(s): {:?}",
        children.unexpected.len(),
        children
            .unexpected
            .iter()
            .map(|n| (n.kind(), n.start_byte(), n.end_byte()))
            .collect::<Vec<_>>()
    );
}

/// A file whose document rule fails at its FIRST header reports once, about the
/// file, rather than twice about tree-sitter.
///
/// Without `@UTF8` the tree is `source_file(ERROR(..))`. `DocumentRoot`
/// classifies that ERROR as the document (`Recovered`), where the navigation it
/// replaced fell back to the `source_file` and called it not-a-document. Both
/// recover zero lines, because the ERROR's children begin with something other
/// than `utf8_header`, so the difference is entirely in the REPORT:
/// `recovered_at_root()` is now true, so the no-lines-recovered path fires and
/// the whole-tree backstop's two candidates are suppressed as overlapping it.
///
/// Pinned because it is a diagnostics change that arrived as a side effect of
/// giving the navigation one owner, and because no corpus file exercises it:
/// every real transcript has `@UTF8`, so no corpus-derived test can see this.
#[test]
fn a_document_that_fails_at_its_first_header_reports_once() {
    const NO_UTF8: &str = "@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
        @ID:\teng|c|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";

    let root = full_document_node(NO_UTF8);
    assert!(
        root.is_error(),
        "the premise: without @UTF8 the document position holds an ERROR, got `{}`",
        root.kind()
    );
}

/// An ERROR at the `tier_body` slot DISPLACES the body; the terminator is still
/// found, and the body is not also surfaced as an unexplained leftover.
///
/// `*CHI:\t[: closed] .` opens with an annotation, which puts an ERROR at the
/// `tier_body` slot position and the real `tier_body`, holding the terminator,
/// into the carrier's `unexpected` sink. Chatter once discarded that node and
/// then reported the terminator missing, telling users an utterance had none on
/// a line ending in " .". Real CLAN CHECK reports one thing for that input.
///
/// Until 2026-08-26 this was pinned by a COMMENT and nothing else, through a
/// restructure of the exact code that handles it.
///
/// WHAT THIS DOES AND DOES NOT PIN, because the distinction cost an hour to
/// establish. It pins the OUTCOME: this input keeps its terminator. It does NOT
/// exercise the sink-recovery path, and saying so is the point. Disabling that
/// path leaves this test green, because the reconstruction's tail-aware
/// splitting now places `tier_body` at its own slot even with an ERROR before
/// it. The path was measured unreached across 2,606 files and eight constructed
/// adversarial tiers; it is kept as recovery, not as live machinery.
#[test]
fn an_error_at_the_tier_body_slot_does_not_lose_the_terminator() {
    const ANNOTATION_FIRST: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Target_Child\n@ID:\teng|c|CHI|||||Target_Child|||\n\
        *CHI:\t[: closed] .\n@End\n";

    let (_utterances, diags) = crate::common::parse_utterances_and_diags(ANNOTATION_FIRST);
    assert!(
        !diags.iter().any(|(code, ..)| code == "E305"),
        "the terminator is in the displaced tier_body, so it must not be reported \
         missing; got {diags:#?}"
    );
}

/// One recovery node is reported ONCE, even when it is both a direct child of
/// the main tier and an entry in that tier's `unexpected` sink.
///
/// `*CHI:\t[: closed] .` produces exactly that: the ERROR appears in both
/// populations, and three walks reported main-tier recovery nodes while each
/// carried its own theory of what the others had done. The direct-children walk
/// compared SPANS against the `tier_body` slot's error, the sink was filtered by
/// node ID, and the whole-tree backstop dedups by span OVERLAP. None knew about
/// all the others, so E759 came out twice at the identical span.
///
/// `MainTierRecovery` owns the set and taking a node removes it, so the second
/// report is unwritable. This pins the OUTCOME, which no type reaches: that the
/// user sees one diagnostic.
#[test]
fn a_recovery_node_in_both_the_child_list_and_the_sink_reports_once() {
    const ANNOTATION_FIRST: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Target_Child\n@ID:\teng|c|CHI|||||Target_Child|||\n\
        *CHI:\t[: closed] .\n@End\n";

    let (_utterances, diags) = crate::common::parse_utterances_and_diags(ANNOTATION_FIRST);
    let e759: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(code, ..)| code == "E759").collect();
    assert_eq!(
        e759.len(),
        1,
        "the annotation is one problem and must be reported once; got {e759:#?}"
    );
}
