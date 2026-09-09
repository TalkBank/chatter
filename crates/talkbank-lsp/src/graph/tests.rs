//! Regression tests for `%mor`/`%gra` DOT graph generation, over utterances
//! the parser builds from CHAT text and the model aligns, which is the route
//! the backend's analysis takes to the graph. Until 2026-09-09 every
//! utterance here was hand-built: `Span::DUMMY` terminators, an
//! `AlignmentSet` assembled pair by pair, and nothing holding the `%mor`
//! items, the `%gra` relations and the alignment to one transcript.
//!
//! # A state no parsed utterance reaches
//!
//! A test used to assert `TierAlignmentMissing { tier: Gra }` for an
//! utterance carrying both tiers and an `AlignmentSet` whose `gra` was
//! `None`. `Utterance::compute_alignments` fills `gra` whenever both tiers
//! are present (with an error-carrying alignment when parse health blocks
//! the pairing), so that state is reachable only by building the set by
//! hand, and the test went with the hand-building. The variant is still
//! constructible because `AlignmentSet::gra` is an `Option` a caller may
//! leave empty; that is the model's shape to close, not this crate's.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::{DependencyGraphResponse, build_dependency_graph_response, generate_dot_graph};
use crate::backend::{LspBackendError, ParseState, TierName};
use crate::test_fixtures::{first_utterance, valid_chat, valid_chat_with_alignments};
use talkbank_model::model::Utterance;

/// The header every fixture shares: one English child speaker.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                      @ID:\teng|corpus|CHI|||||Target_Child|||\n";

/// One word with a `%mor` and a `%gra` covering the word and the terminator.
const CAT: &str = "*CHI:\tcat .\n%mor:\tn|cat .\n%gra:\t1|0|ROOT 2|1|PUNCT\n";

/// The same word with a `%mor` and no `%gra`.
const CAT_MOR_ONLY: &str = "*CHI:\tcat .\n%mor:\tn|cat .\n";

/// The same word with no dependent tier at all.
const CAT_BARE: &str = "*CHI:\tcat .\n";

/// `it's cookie .`, whose post-clitic `be` is its own `%gra` chunk: the
/// chunk sequence is `it` (1), `be` (2), `cookie` (3), `.` (4).
const CLITIC: &str = "*CHI:\tit's cookie .\n%mor:\tpron|it~aux|be n|cookie .\n\
                      %gra:\t1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT\n";

/// The whole transcript around `lines`.
fn transcript(lines: &str) -> String {
    format!("{HEADER}{lines}@End\n")
}

/// The utterance `lines` describe, parsed and aligned as analysis leaves it.
fn aligned(lines: &str) -> Result<Utterance, String> {
    first_utterance(valid_chat_with_alignments(&transcript(lines))?)
}

/// The utterance `lines` describe, parsed with no alignment computed.
fn unaligned(lines: &str) -> Result<Utterance, String> {
    first_utterance(valid_chat(&transcript(lines))?)
}

/// An utterance whose alignment has not been computed has no graph, however
/// complete its tiers: the graph reads the `%mor`/`%gra` pairing from the
/// alignment, never from the tiers directly.
#[test]
fn graph_needs_computed_alignments() -> Result<(), String> {
    let utterance = unaligned(CAT)?;
    let result = generate_dot_graph(&utterance, ParseState::Clean);
    assert!(
        matches!(result, Err(LspBackendError::AlignmentMetadataMissing)),
        "expected AlignmentMetadataMissing, got {result:?}",
    );
    Ok(())
}

/// A `%mor` with no `%gra` has nothing to draw edges from, and the error
/// names the tier that is missing rather than the alignment that was never
/// attempted.
#[test]
fn graph_needs_a_gra_tier() -> Result<(), String> {
    let utterance = aligned(CAT_MOR_ONLY)?;
    let result = generate_dot_graph(&utterance, ParseState::Clean);
    assert!(
        matches!(
            result,
            Err(LspBackendError::MissingTier {
                tier: TierName::Gra,
            })
        ),
        "expected MissingTier {{ tier: Gra }}, got {result:?}",
    );
    Ok(())
}

/// On `ParseState::StaleBaseline`, the DOT header gains a muted
/// top-left `stale baseline` label so the viewer can tell the
/// dependency graph was computed against the last successful parse
/// (KIB-013). On `ParseState::Clean` the same label must be absent.
#[test]
fn stale_baseline_emits_muted_label_attrs() -> Result<(), String> {
    let utterance = aligned(CAT)?;

    let clean =
        generate_dot_graph(&utterance, ParseState::Clean).map_err(|err| format!("clean: {err}"))?;
    assert!(
        !clean.contains("stale baseline"),
        "Clean parse state must not emit the stale-baseline marker"
    );

    let stale = generate_dot_graph(&utterance, ParseState::StaleBaseline)
        .map_err(|err| format!("stale: {err}"))?;
    assert!(
        stale.contains("label=\"stale baseline\""),
        "StaleBaseline must emit the marker label; got:\n{stale}"
    );
    assert!(
        stale.contains("fontcolor=\"#888888\""),
        "marker must use the muted fontcolor"
    );
    assert!(
        stale.contains("fontname=\"Courier\""),
        "marker must use Courier to signal meta-information"
    );
    Ok(())
}

/// The DOT shape for one word: a left-to-right digraph, the word as a node,
/// its relation to ROOT and the terminator's relation to it as labelled
/// edges.
#[test]
fn a_one_word_utterance_renders_its_node_and_edges() -> Result<(), String> {
    let utterance = aligned(CAT)?;
    let dot = generate_dot_graph(&utterance, ParseState::Clean)
        .map_err(|err| format!("Failed to generate graph: {err}"))?;

    assert!(
        dot.contains("digraph utterance"),
        "no digraph header in:\n{dot}"
    );
    assert!(
        dot.contains("rankdir=LR"),
        "no left-to-right layout in:\n{dot}"
    );
    assert!(dot.contains("cat"), "the word is not a node in:\n{dot}");
    for edge in ["1 -> 0 [label=\"ROOT\"", "2 -> 1 [label=\"PUNCT\""] {
        assert!(dot.contains(edge), "expected edge `{edge}` in:\n{dot}");
    }
    Ok(())
}

/// A request for an utterance with no `%mor` tier returns a typed `Unavailable`
/// variant, so the extension can distinguish "no graph to render" from actual
/// DOT syntax. The previous string-valued API collapsed both into one field and
/// caused the Graphviz renderer to choke on the reason text.
#[test]
fn response_is_unavailable_when_no_mor_tier() -> Result<(), String> {
    let utterance = aligned(CAT_BARE)?;

    match build_dependency_graph_response(&utterance, ParseState::Clean) {
        DependencyGraphResponse::Unavailable { reason } => {
            assert!(
                reason.contains("%mor"),
                "reason should mention the missing %mor tier; got: {reason}",
            );
        }
        DependencyGraphResponse::Dot { .. } => {
            panic!("expected Unavailable variant when %mor tier is missing");
        }
    }
    Ok(())
}

/// A fully aligned utterance produces a `Dot` variant carrying the Graphviz
/// source, never a bare string conflated with error text.
#[test]
fn response_is_dot_when_aligned() -> Result<(), String> {
    let utterance = aligned(CAT)?;

    match build_dependency_graph_response(&utterance, ParseState::Clean) {
        DependencyGraphResponse::Dot { source } => {
            assert!(
                source.contains("digraph"),
                "DOT source missing digraph header"
            );
        }
        DependencyGraphResponse::Unavailable { reason } => {
            panic!("expected Dot variant; got Unavailable({reason})");
        }
    }
    Ok(())
}

/// The response serialises to a JSON discriminant object so the TS client can
/// branch on `kind` without parsing free-form text. A wire format, which no
/// type pins.
#[test]
fn response_serializes_with_kind_discriminant() {
    let dot = DependencyGraphResponse::Dot {
        source: "digraph {}".to_string(),
    };
    let value = serde_json::to_value(&dot).expect("dot response should serialize");
    assert_eq!(value["kind"], "dot");
    assert_eq!(value["source"], "digraph {}");

    let unavailable = DependencyGraphResponse::Unavailable {
        reason: "No %mor tier found".to_string(),
    };
    let value = serde_json::to_value(&unavailable).expect("unavailable response should serialize");
    assert_eq!(value["kind"], "unavailable");
    assert_eq!(value["reason"], "No %mor tier found");
}

/// Dependency edges must address the `%mor` **chunk** sequence, so a
/// post-clitic gets its own graph node and relations referring to it
/// connect through that node, not through the node of the next `%mor`
/// item. For `pron|it~aux|be n|cookie .` under
/// `1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT`, node IDs follow chunk order:
///
/// | node_id | chunk_idx | label   |
/// |---------|-----------|---------|
/// | 1       | 0         | `it`    |
/// | 2       | 1         | `be`    |  the post-clitic of the `it's` item
/// | 3       | 2         | `cookie`|
/// | 4       | 3         | `.`     |
///
/// The test pins the four edges so any regression (e.g. reverting the
/// `labels.rs` walk to an items-only loop, or making `edges.rs` treat the
/// chunk index as an item index) shows up as a failed containment check.
#[test]
fn dependency_edges_with_post_clitic_connect_correct_chunks() -> Result<(), String> {
    let utterance = aligned(CLITIC)?;
    let dot = generate_dot_graph(&utterance, ParseState::Clean)
        .map_err(|err| format!("generate: {err}"))?;

    // Every chunk, including the post-clitic `be`, gets its own node label.
    for lemma in ["it", "be", "cookie"] {
        assert!(
            dot.contains(lemma),
            "expected DOT to label every chunk ({lemma}); got:\n{dot}"
        );
    }

    // The critical edges: `1 -> 2 NSUBJ`, `3 -> 2 OBJ` and `4 -> 2 PUNCT`
    // all terminate at node 2 (the `be` post-clitic). If edge generation
    // ever confused chunk with item indices, `2` would collapse into the
    // next item's node and these would read `-> 3` or `-> 1` instead.
    for (edge, relation) in [
        ("1 -> 2", "NSUBJ"),
        ("2 -> 0", "ROOT"),
        ("3 -> 2", "OBJ"),
        ("4 -> 2", "PUNCT"),
    ] {
        let expected = format!("{edge} [label=\"{relation}\"");
        assert!(
            dot.contains(&expected),
            "expected edge `{expected}`; DOT was:\n{dot}"
        );
    }

    Ok(())
}
