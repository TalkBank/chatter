//! Which COMBINATIONS of CHAT constructs does the reference corpus exercise?
//!
//! # Node-kind coverage is somebody else's job, and it is already done
//!
//! `spec/tools/src/node_coverage.rs` owns "which grammar node kinds does the
//! corpus produce", with its own CI gate and its own exclusion lists. This
//! module deliberately does NOT measure that, and an earlier draft that did was
//! deleted: its list turned out to be exactly the union of that gate's
//! `INVALID_BY_CONSTRUCTION` and `NOT_YET_IN_CORPUS`, so one new fixture would
//! have had to be reflected in two lists in two workspaces.
//!
//! Worse than the duplication, the copy FLATTENED a distinction that gate
//! spends its module doc defending: three of those kinds are ones a valid
//! corpus must never contain. Merged into one "not covered yet" list, a
//! `blank_line` appearing in `corpus/reference/` would have been reported here
//! as good news ("delete the entry, you covered it") and there as corpus
//! corruption. Two gates, opposite verdicts, same input.
//!
//! # What IS new here: pairs
//!
//! Node-kind coverage of the reference corpus is **92.3%**, which sounds nearly
//! finished and is the wrong question, because every parser defect found on
//! 2026-08-08 was a COMBINATION gap rather than a missing construct:
//!
//! - `long_feature_begin` appears in the reference corpus, but only at the top
//!   level of a main tier. Nested inside a `group` it made `chatter validate`
//!   report E359 against valid CHAT, and made the re2c backend delete the
//!   marker outright.
//! - `langcode` appears, but never under `wor_tier_body`. Real Chinese and
//!   bilingual transcripts write `%wor:\t[- zho] ...` constantly, and re2c
//!   could not parse a single one: 510 E316 over a 2% corpus sample.
//! - `underline_begin` appears, but never inside `word_body`, which is a level
//!   the model's own content walker cannot reach at all.
//!
//! In every case the construct was "covered" and the bug lived in the pairing.
//! So this module measures parent-to-child pairs, the granularity that matches
//! the defects.
//!
//! # The ratchet, and what 100% means
//!
//! `UNCOVERED_PAIRS` lists combinations real transcripts contain that the
//! reference corpus does not. An entry that becomes covered FAILS, so the list
//! cannot rot into a permanent excuse after somebody adds the fixture and
//! forgets to prune it. The finish line is the list being empty, which is what
//! "the reference corpus exercises every path" actually means, and which a
//! percentage could never show.
//!
//! Only the retire direction is enforced here. Discovering a NEW uncovered pair
//! needs the wild corpus, which a unit test does not have; that direction
//! belongs to the corpus-differential runner, which does.
//!
//! # Fixtures are mined from real data, never invented
//!
//! An entry is retired by adding an ATTESTED fixture: find the construct in the
//! wild corpus, trim it, record where it came from in the file itself. The
//! reference corpus is synthesized, and that is precisely why it holds each
//! construct only in its simplest form, so filling gaps from imagination would
//! rebuild the same blind spot one level down.
//! `corpus/reference/tiers/wor.cha`'s Cantonese `%wor` pair is the worked
//! example.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::gate::{Gate, GateOutcome, listing};
use crate::test_error::TestError;

/// Tree-sitter node-kind identity within one grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TreeSitterKindId(u16);

/// One grammar-scoped construct written directly inside another.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ConstructPair {
    parent: TreeSitterKindId,
    child: TreeSitterKindId,
}

/// A policy pair named at the source boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NamedConstructPair {
    parent: &'static str,
    child: &'static str,
}

impl std::fmt::Display for NamedConstructPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {}", self.parent, self.child)
    }
}

/// Covered pairs bound to the exact grammar that assigned their IDs.
struct CoveredPairs {
    language: tree_sitter::Language,
    pairs: BTreeSet<ConstructPair>,
}

impl CoveredPairs {
    fn len(&self) -> usize {
        self.pairs.len()
    }

    fn contains_named(&self, pair: NamedConstructPair) -> Result<bool, String> {
        let parent = self.resolve_named_kind(pair.parent)?;
        let child = self.resolve_named_kind(pair.child)?;
        Ok(self.pairs.contains(&ConstructPair { parent, child }))
    }

    fn resolve_named_kind(&self, name: &str) -> Result<TreeSitterKindId, String> {
        let id = self.language.id_for_node_kind(name, true);
        if self.language.node_kind_for_id(id) == Some(name) {
            Ok(TreeSitterKindId(id))
        } else {
            Err(format!(
                "coverage policy names unknown grammar node kind `{name}`"
            ))
        }
    }
}

/// Collect every parent-to-child pair the given files produce.
///
/// Kind IDs are collected without per-node allocation and remain bundled with
/// the [`tree_sitter::Language`] that gives those IDs meaning.
fn pairs_in(files: &[PathBuf]) -> Result<CoveredPairs, TestError> {
    let language: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| TestError::ParserInit(format!("cannot load the CHAT grammar: {e}")))?;

    let mut pairs = BTreeSet::new();
    for file in files {
        let source = std::fs::read_to_string(file)?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| TestError::Failure(format!("parse returned nothing for {file:?}")))?;
        walk(tree.root_node(), &mut pairs);
    }
    if pairs.is_empty() {
        // An empty census is a broken measurement, not a corpus with no
        // structure in it, and the two read identically downstream.
        return Err(TestError::Failure(format!(
            "no construct pairs found in {} file(s)",
            files.len()
        )));
    }
    Ok(CoveredPairs { language, pairs })
}

/// Record each named child against its parent, then descend.
fn walk(node: tree_sitter::Node<'_>, pairs: &mut BTreeSet<ConstructPair>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        pairs.insert(ConstructPair {
            parent: TreeSitterKindId(node.kind_id()),
            child: TreeSitterKindId(child.kind_id()),
        });
        walk(child, pairs);
    }
}

/// Every `.cha` under a directory, sorted so a run is reproducible.
///
/// `Err` when the tree holds none, because every caller wants a corpus and an
/// empty vector is indistinguishable from a mistyped path. Shared rather than
/// copied: this crate already had several hand-rolled corpus walks, and the
/// roundtrip suite's copy was this function plus a `sort` and an `assert!` at
/// the call site.
///
/// Roughly a dozen other walks in this crate remain, and they do NOT all agree
/// with this one: several test `ext == "cha"` where this tests
/// `eq_ignore_ascii_case`. Measured 2026-08-09 before deciding it mattered:
/// **zero** files under `corpus/reference` and zero of the ~107,000 in the wild
/// corpus have a non-lowercase extension, so no walker sees a different file
/// set and the inconsistency is unobservable today. Retiring the rest onto this
/// function is mechanical dedup work, not a correctness fix; the case rule here
/// is the permissive one so that adopting it can never LOSE a file.
pub fn cha_files_under(root: &Path) -> Result<Vec<PathBuf>, TestError> {
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cha")))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(TestError::Failure(format!("no .cha files under {root:?}")));
    }
    Ok(files)
}

/// Combinations real transcripts produce that the reference corpus does not.
///
/// Measured against an unbiased 400-file sample of the wild corpus on
/// 2026-08-08, ordered by how often real data contains them, because that is
/// the order in which they are worth fixing. The leading entries are not
/// exotic: an inline bullet inside content occurs 4,225 times in that sample,
/// and terminators on `%mor` several thousand.
///
/// Derived from REAL DATA rather than from the grammar's full cross product.
/// Most of that cross product is unwritable CHAT, so a gate demanding it could
/// never close; a gate demanding everything real transcripts actually contain
/// is the bar the tool has to meet.
///
/// This is a hand-copied measurement, which is a value mirroring a fact it
/// cannot be derived from: when the corpus grows or a grammar node is renamed
/// it goes stale silently. Regenerating it from the corpus-differential runner,
/// with its sample size and date recorded beside it, is the recorded follow-up.
const UNCOVERED_PAIRS: &[(&str, &str)] = &[
    ("base_content_item", "bullet"),
    ("mor_contents", "trailing_off"),
    ("mor_contents", "interruption"),
    ("mor_contents", "self_interruption"),
    ("main_tier", "sep_trailing_space"),
    ("text_with_bullets", "continuation"),
    ("mor_contents", "trailing_off_question"),
    ("mor_contents", "interrupted_question"),
    ("standalone_word", "pos_tag"),
    ("id_ses", "ethnicity_value"),
    ("mor_contents", "self_interrupted_question"),
    ("wor_tier_body", "quoted_new_line"),
    ("wor_tier_body", "self_interruption"),
    ("wor_tier_body", "trailing_off"),
    ("wor_tier_body", "interruption"),
    ("wor_tier_body", "tag_marker"),
    ("wor_tier_body", "interrupted_question"),
    ("tier_sep", "sep_trailing_space"),
    ("mor_contents", "broken_question"),
    ("separator", "colon"),
    ("wor_tier_body", "trailing_off_question"),
    ("wor_tier_body", "self_interrupted_question"),
    ("mor_contents", "break_for_coding"),
];

/// Reference-corpus combination coverage, as a registered gate.
pub struct ConstructCoverageGate;

impl Gate for ConstructCoverageGate {
    fn name(&self) -> &'static str {
        "reference-corpus combination coverage"
    }

    fn check(&self) -> GateOutcome {
        let root = crate::repo_paths::workspace_root().join("corpus/reference");
        let files =
            cha_files_under(&root).map_err(|e| format!("cannot list the reference corpus: {e}"))?;
        let covered = pairs_in(&files).map_err(|e| format!("cannot measure coverage: {e}"))?;

        let mut retired = Vec::new();
        for &(parent, child) in UNCOVERED_PAIRS {
            let pair = NamedConstructPair { parent, child };
            if covered.contains_named(pair)? {
                retired.push(pair);
            }
        }

        if retired.is_empty() {
            return Ok(format!(
                "{} construct pair(s) covered; {} combination gap(s) remaining",
                covered.len(),
                UNCOVERED_PAIRS.len(),
            ));
        }
        Err(listing(
            "FAIL: listed as uncovered but the reference corpus DOES produce them.\n\
             Delete them from UNCOVERED_PAIRS in the commit that covered them:",
            &retired,
        ))
    }
}
