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

//! Source-based GRA parsing contracts: exact valid relation values and diagnosed
//! rejection of malformed input. File recovery retains raw source; neither
//! structural recovery nor a rejected numeric field constructs an empty graph.
//!
//! Source-bound lowering uses the generated tier, contents and relation visitors.
//! Structural recovery at document dispatch reports the actual tier range;
//! numeric rejection in a healthy carrier retains its field-specific diagnostic.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line};
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, usize, usize, String);

/// A valid `%gra` line with five `index|head|relation` triples on a single
/// utterance. Exercises the reachable path: a `Present` `gra_contents` body slot
/// and five `Present` relation slots, each decoded by `parse_gra_relation`.
const VALID_MULTI_RELATION: &str = "@UTF8\n@Begin\n*CHI:\tI go home now .\n%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|ADVMOD 5|2|PUNCT\n@End\n";

/// A structurally valid `%gra` line whose single relation is SEMANTICALLY
/// invalid (index 0; indices are 1-indexed). This reaches `parse_gra_tier`
/// (the tier node has no tree-sitter error) and `parse_gra_relation`, which
/// rejects the relation with `InvalidGrammarIndex` (E709). The gra tier stays
/// retained as UnparsedGra (no fabricated graph). Byte offsets: the
/// `0|0|ROOT` relation node spans 30..38.
const ZERO_INDEX_RELATION: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%gra:\t0|0|ROOT\n@End\n";

/// A `%gra:` line with an EMPTY body (tab then newline, no relations). This makes
/// the tier node `has_error()`, so document dispatch reports E316 and retains
/// raw source without constructing a GraTier.
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
    for line in chat.lines.as_slice() {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Gra(t) = &dt.tier {
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
                d.location.span().expect("located diagnostic").start(),
                d.location.span().expect("located diagnostic").end(),
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
            (4, 2, "ADVMOD".to_string()),
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
/// (E709) at the relation span (30..38); file recovery keeps raw source without
/// a parsed graph. This exercises `parse_gra_tier`
/// relation iteration + `parse_gra_relation` rejection.
#[test]
fn zero_index_relation_does_not_construct_a_gra_tier() {
    let (relations, saw_gra_tier, diags) = parse_gra(ZERO_INDEX_RELATION);

    assert!(
        !saw_gra_tier,
        "rejected lowering must not expose a parsed graph"
    );
    assert!(
        relations.is_empty(),
        "the rejected relation must not be recovered as a fabricated default: {relations:?}"
    );

    let e709: Vec<&(String, usize, usize, String)> =
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

/// Empty-body structural recovery reports its source range without a graph or
/// a fabricated relation-level diagnostic.
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
            .any(|(c, start, end, _)| c == "E316" && *start == 24 && *end == 30),
        "expected structural recovery at the unparsed tier, got: {diags:?}"
    );
    assert!(
        !diags.iter().any(|(c, _, _, _)| c == "E708"),
        "parse_gra_tier's MalformedGrammarRelation (E708) must be unreachable here, got: {diags:?}"
    );
}
