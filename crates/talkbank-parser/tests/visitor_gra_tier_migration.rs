// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Characterization tests for the `%gra` dependent tier migrated onto the
//! generated `extract_gra_dependent_tier` / `extract_gra_contents` /
//! `extract_gra_relation` typed visitor (Task 4c).
//!
//! These tests pin the OBSERVABLE behaviour of the `%gra` tier at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces two hand-walks:
//!
//! - `tier_parsers/gra/tier.rs`: the `child.kind() == GRA_CONTENTS` locate loop
//!   plus the `while gra_contents.child(idx)` + `match child.kind()` relation
//!   iteration, with exhaustive typed `NodeSlot` matching over the
//!   `gra_dependent_tier` body slot (`child_2`) and each `gra_contents` relation
//!   slot.
//! - `tier_parsers/gra/relation.rs`: the positional `node.child(0/2/4)` walk for
//!   index / head / relation, with the named `index` / `head` / `relation` slots.
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed relation model and the
//! recovery diagnostics must not change. Every value here was captured by
//! RUNNING the pre-migration parser (base HEAD `99ed501`), not guessed. The
//! tests PASS on the current code and MUST STAY GREEN, byte-identical, after the
//! migration.
//!
//! # Reachability note (why the malformed cases look the way they do)
//!
//! `parse_gra_tier` is only invoked from the dispatch when the tier node has NO
//! tree-sitter error (`GRA_DEPENDENT_TIER => if tier_node.has_error() { ... } else
//! { parse_gra_tier(...) }`). A structurally broken `%gra` line (empty body,
//! missing tab) makes the tier node `has_error()` and is routed to the upstream
//! placeholder / error path (E600 / E602), so `parse_gra_tier`'s own
//! "missing gra_contents -> MalformedGrammarRelation + empty tier" silent-partial
//! is UNREACHABLE from the boundary (documented by
//! `empty_body_gra_tier_is_handled_upstream_without_a_gra_tier`). The reachable
//! malformed path is a SEMANTICALLY invalid relation (index 0) inside a
//! structurally valid `%gra` line, which does reach `parse_gra_tier` +
//! `parse_gra_relation` and is pinned by
//! `zero_index_relation_is_rejected_leaving_an_empty_gra_tier`.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line};
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// A valid `%gra` line with five `index|head|relation` triples on a single
/// utterance. Exercises the reachable path: a `Present` `gra_contents` body slot
/// and five `Present` relation slots, each decoded by `parse_gra_relation`.
const VALID_MULTI_RELATION: &str = "@UTF8\n@Begin\n*CHI:\tI go home now .\n%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|ADV 5|2|PUNCT\n@End\n";

/// A structurally valid `%gra` line whose single relation is SEMANTICALLY
/// invalid (index 0; indices are 1-indexed). This reaches `parse_gra_tier`
/// (the tier node has no tree-sitter error) and `parse_gra_relation`, which
/// rejects the relation with `InvalidGrammarIndex` (E709). The gra tier stays
/// attached with ZERO relations (no fabricated default). Byte offsets: the
/// `0|0|ROOT` relation node spans 30..38.
const ZERO_INDEX_RELATION: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%gra:\t0|0|ROOT\n@End\n";

/// A `%gra:` line with an EMPTY body (tab then newline, no relations). This makes
/// the tier node `has_error()`, so it is handled by the upstream error path
/// (E600) and NO gra tier is attached; `parse_gra_tier` is never called.
const EMPTY_BODY: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%gra:\t\n@End\n";

/// One grammatical relation rendered as a stable structural tuple for assertions.
fn rel_tuple(rel: &talkbank_model::model::GrammaticalRelation) -> (usize, usize, String) {
    (rel.index, rel.head, rel.relation.as_str().to_string())
}

/// Parse `input` at the streaming boundary and return, in document order, the
/// relations of every `%gra` tier as `(index, head, relation)` tuples, whether
/// at least one `%gra` tier was attached at all, and every collected diagnostic
/// as `(code, start, end, message)`.
fn parse_gra(input: &str) -> (Vec<(usize, usize, String)>, bool, Vec<Diag>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut relations = Vec::new();
    let mut saw_gra_tier = false;
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Gra(t) = dt {
                    saw_gra_tier = true;
                    relations.extend(t.relations().iter().map(rel_tuple));
                }
            }
        }
    }

    let diags = errors
        .to_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();

    (relations, saw_gra_tier, diags)
}

/// VALID: a five-relation `%gra` line must parse to the exact ordered relation
/// model and produce ZERO diagnostics. Pins the
/// `NodeSlot::Present(gra_contents)` -> `extract_gra_contents` ->
/// `NodeSlot::Present(gra_relation)` -> `parse_gra_relation` path that valid CHAT
/// always takes.
#[test]
fn valid_multi_relation_gra_parses_byte_identical_with_zero_diagnostics() {
    let (relations, saw_gra_tier, diags) = parse_gra(VALID_MULTI_RELATION);

    assert!(saw_gra_tier, "a valid %gra line must attach a gra tier");
    assert_eq!(
        relations,
        vec![
            (1, 2, "NSUBJ".to_string()),
            (2, 0, "ROOT".to_string()),
            (3, 2, "OBJ".to_string()),
            (4, 2, "ADV".to_string()),
            (5, 2, "PUNCT".to_string()),
        ],
        "every relation must decode to its exact index|head|relation, in order"
    );
    assert!(
        diags.is_empty(),
        "a valid %gra line must produce zero diagnostics, got: {diags:?}"
    );
}

/// MALFORMED (reachable): a structurally valid `%gra` line whose single relation
/// has index 0. `parse_gra_relation` rejects it with `InvalidGrammarIndex`
/// (E709) at the relation span (30..38); the gra tier stays attached with ZERO
/// relations. This is the reachable exercise of the migrated `parse_gra_tier`
/// relation iteration + `parse_gra_relation` rejection.
#[test]
fn zero_index_relation_is_rejected_leaving_an_empty_gra_tier() {
    let (relations, saw_gra_tier, diags) = parse_gra(ZERO_INDEX_RELATION);

    assert!(
        saw_gra_tier,
        "the gra tier must remain attached for downstream diagnostics"
    );
    assert!(
        relations.is_empty(),
        "the rejected relation must not be recovered as a fabricated default: {relations:?}"
    );

    let e709: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(c, _, _, _)| c == "E709").collect();
    assert_eq!(
        e709,
        vec![&(
            "E709".to_string(),
            30,
            38,
            "Index cannot be 0 (indices are 1-indexed)".to_string(),
        )],
        "exactly one InvalidGrammarIndex diagnostic at the relation span is expected, got: {diags:?}"
    );
    assert!(
        !diags.iter().any(|(c, _, _, _)| c == "E708"),
        "no MalformedGrammarRelation diagnostic is expected for a present-but-invalid relation, got: {diags:?}"
    );
}

/// MALFORMED (upstream): an empty-body `%gra:` line makes the tier node
/// `has_error()`, so the dispatch routes it to the upstream error path (E600)
/// and attaches NO gra tier; `parse_gra_tier` is never called and its internal
/// `MalformedGrammarRelation` + empty-tier silent-partial is unreachable from
/// the boundary. Pins that reality so the migration cannot accidentally start
/// attaching a gra tier or emitting `MalformedGrammarRelation` here.
#[test]
fn empty_body_gra_tier_is_handled_upstream_without_a_gra_tier() {
    let (relations, saw_gra_tier, diags) = parse_gra(EMPTY_BODY);

    assert!(
        !saw_gra_tier,
        "an empty-body %gra line must NOT attach a gra tier (handled upstream)"
    );
    assert!(
        relations.is_empty(),
        "no relations can exist without a gra tier: {relations:?}"
    );
    assert!(
        diags
            .iter()
            .any(|(c, start, end, _)| c == "E600" && *start == 24 && *end == 30),
        "expected the upstream E600 'could not fully parse dependent tier' diagnostic, got: {diags:?}"
    );
    assert!(
        !diags.iter().any(|(c, _, _, _)| c == "E708"),
        "parse_gra_tier's MalformedGrammarRelation (E708) must be unreachable here, got: {diags:?}"
    );
}
