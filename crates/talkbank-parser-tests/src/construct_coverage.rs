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

use crate::gate::tree::RelPath;
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, Tier, Tree, UnprovenRule, listing};
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
fn pairs_in(tree: &ReadTree, files: &[RelPath]) -> Result<CoveredPairs, String> {
    let language: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| format!("cannot load the CHAT grammar: {e}"))?;

    let mut pairs = BTreeSet::new();
    for file in files {
        let source = tree.read_to_string(file).map_err(|e| e.to_string())?;
        let parsed = parser
            .parse(&source, None)
            .ok_or_else(|| format!("parse returned nothing for {file}"))?;
        walk(parsed.root_node(), &mut pairs);
    }
    if pairs.is_empty() {
        // An empty census is a broken measurement, not a corpus with no
        // structure in it, and the two read identically downstream.
        return Err(format!(
            "no construct pairs found in {} file(s)",
            files.len()
        ));
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
    let tree = Tree::rooted(root).into_read();
    let files = cha_files_in(&tree, "").map_err(TestError::Failure)?;
    Ok(files
        .into_iter()
        .map(|file| root.join(file.as_str()))
        .collect())
}

/// Every `.cha` under one directory OF A TREE, sorted.
///
/// The tree-reading half of [`cha_files_under`], which the gate uses so that a
/// probe can plant a corpus. Walk errors are propagated rather than dropped:
/// the predecessor's `filter_map(Result::ok)` meant an unreadable subdirectory
/// silently shrank the file set while the gate still reported clean, which is
/// a gate passing having measured less than it claims.
fn cha_files_in(tree: &ReadTree, dir: &str) -> Result<Vec<RelPath>, String> {
    let files: Vec<RelPath> = tree
        .files_under(dir)
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|path| path.extension_is("cha"))
        .collect();
    if files.is_empty() {
        return Err(format!("no .cha files under {:?}", tree.root().join(dir)));
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

/// The corpus this gate measures, in one spelling.
const REFERENCE_CORPUS: &str = "corpus/reference";

/// Rules of this gate no plant can reach.
///
/// See [`crate::gate::UnprovenRule`]. Two of these are compile-time inputs and
/// two are directions the gate deliberately does not enforce, which is worth
/// saying out loud beside a suite of green probes.
const UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "R4: a policy name that is not a grammar node kind",
        "`resolve_named_kind` compares `UNCOVERED_PAIRS` against \
         `tree_sitter_talkbank::LANGUAGE`. Both are compile-time inputs: the \
         const is Rust source and the node-kind vocabulary is linked in from \
         the committed `grammar/src/parser.c`. No edit to any file the gate \
         READS reaches it.",
    ),
    UnprovenRule::new(
        "the `parse returned nothing` branch",
        "`tree_sitter::Parser::parse` returns `None` only after a timeout or a \
         cancellation flag, and this gate sets neither. A dead branch, kept \
         because the alternative is fabricating a tree from nothing.",
    ),
    UnprovenRule::new(
        "R5's DISCOVERY direction, which the gate does not enforce",
        "Deleting a still-uncovered entry from `UNCOVERED_PAIRS` leaves the gate \
         green. Only the retire direction is ratcheted, so the list can rot by \
         getting SHORTER. Discovery needs the wild corpus and belongs to the \
         corpus-differential runner. This is the gate's real blind spot.",
    ),
    UnprovenRule::new(
        "the two numbers in the clean summary",
        "`covered.len()` and `UNCOVERED_PAIRS.len()` are printed and compared to \
         nothing, which is the shape `gate`'s own module doc says this trait \
         exists to close, surviving on the Ok side. A probe cannot prove a \
         check that does not exist; naming it is the only available action.",
    ),
    UnprovenRule::new(
        "R5 for 18 of its 23 listed pairs",
        "The content probes reach 5 instances. All 23 flow through one \
         `contains_named` loop and one failure text, so per-instance probes \
         would prove the same code path again; three of the rest would need \
         materially different constructs planted (a mid-content bullet, a \
         continuation inside a bullet-carrying tier, a `$pos` on a real word).",
    ),
];

/// The corpus file one probe writes invalid bytes into, and whose name that
/// same probe requires the failure to carry.
/// The one corpus file four probes reach for: three plant an edit into it and
/// one writes invalid bytes into it, and that fourth probe also requires the
/// failure to NAME it.
///
/// One spelling, because a rename that missed the assertion would leave a probe
/// asserting on a path nothing writes, which passes only when the gate fails
/// for some other reason.
const MOR_GRA_FIXTURE: &str = "corpus/reference/tiers/mor-gra.cha";

impl Gate for ConstructCoverageGate {
    fn name(&self) -> &'static str {
        "reference-corpus combination coverage"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        // ABSENT and EMPTY are different facts. A filtered clone that omits
        // `corpus/` has nothing to say about combination coverage and must not
        // be reported as a corpus defect; a corpus DIRECTORY holding no `.cha`
        // is a broken measurement and fails. The predecessor collapsed both
        // into one failure text.
        if !tree.dir_exists(REFERENCE_CORPUS) {
            return Outcome::unavailable(
                format!(
                    "{REFERENCE_CORPUS}/ is not in this checkout; a sparse or \
                     filtered clone omits it"
                ),
                Tier::PrePush,
            );
        }

        let files = match cha_files_in(&tree, REFERENCE_CORPUS) {
            Ok(files) => files,
            Err(why) => {
                return Outcome::failed(format!("cannot list the reference corpus: {why}"));
            }
        };
        let covered = match pairs_in(&tree, &files) {
            Ok(covered) => covered,
            Err(why) => return Outcome::failed(format!("cannot measure coverage: {why}")),
        };

        let mut retired = Vec::new();
        for &(parent, child) in UNCOVERED_PAIRS {
            let pair = NamedConstructPair { parent, child };
            match covered.contains_named(pair) {
                Ok(true) => retired.push(pair),
                Ok(false) => {}
                Err(why) => return Outcome::failed(why),
            }
        }

        if retired.is_empty() {
            return tree.clean(format!(
                "{} construct pair(s) covered; {} combination gap(s) remaining",
                covered.len(),
                UNCOVERED_PAIRS.len(),
            ));
        }
        Outcome::failed(listing(
            "FAIL: listed as uncovered but the reference corpus DOES produce them.\n\
             Delete them from UNCOVERED_PAIRS in the commit that covered them:",
            &retired,
        ))
    }

    /// Five of these plant a listed pair INTO the corpus, which is the ratchet's
    /// one rule; the other three are about refusing a measurement it cannot
    /// make, and the last is about declining to make one at all. The three
    /// chosen pairs each have a token with no equal-length competitor in its
    /// position, so none of them can quietly fail to appear.
    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a %mor tier terminated with +... (mor_contents -> trailing_off)",
            "mor_contents -> trailing_off",
            |edit| {
                edit.replace_once(
                    MOR_GRA_FIXTURE,
                    "noun|cookie-Plur .",
                    "noun|cookie-Plur +...",
                )
            },
        )
        .declining(
            "the reference corpus is not in this checkout at all",
            "corpus/reference/ is not in this checkout",
            Tier::PrePush,
            |edit| {
                edit.remove_dir(REFERENCE_CORPUS);
                Ok(())
            },
        )
        .refusing(
            "an @ID SES field holding a bare ethnicity (id_ses -> ethnicity_value)",
            "id_ses -> ethnicity_value",
            |edit| {
                edit.replace_once(
                    "corpus/reference/core/headers-time-and-types.cha",
                    "White,MC",
                    "White",
                )
            },
        )
        .refusing(
            "a tag marker inside a %wor body (wor_tier_body -> tag_marker)",
            "wor_tier_body -> tag_marker",
            |edit| {
                edit.replace_once(
                    "corpus/reference/tiers/wor.cha",
                    "\u{15}300_600\u{15} .",
                    "\u{15}300_600\u{15} \u{201E} .",
                )
            },
        )
        .refusing(
            "a trailing separator space on a main tier (main_tier -> sep_trailing_space)",
            "main_tier -> sep_trailing_space",
            |edit| {
                edit.replace_once(
                    MOR_GRA_FIXTURE,
                    "\tit's I want cookies .",
                    "\t it's I want cookies .",
                )
            },
        )
        .refusing(
            "a trailing separator space on a dependent tier (tier_sep -> sep_trailing_space)",
            "tier_sep -> sep_trailing_space",
            |edit| edit.replace_once(MOR_GRA_FIXTURE, "%gra:\t1|4|NSUBJ", "%gra:\t 1|4|NSUBJ"),
        )
        .refusing(
            "the corpus directory is there and holds no .cha",
            "no .cha files under",
            |edit| {
                edit.hide_files_under(REFERENCE_CORPUS);
                Ok(())
            },
        )
        .refusing(
            "a corpus file whose bytes are not UTF-8",
            // The PLANTED FILE and the READ's own words. Two things were wrong
            // with naming "cannot measure coverage": it is the wrapper
            // `pairs_in` puts around every failure it has, so the probe passed
            // on the grammar-load failure and the empty-census refusal alike;
            // and it named no file, so any non-UTF-8 corpus file satisfied it.
            // A probe should be satisfiable only by the tree it planted.
            "mor-gra.cha: stream did not contain valid UTF-8",
            |edit| {
                edit.write_bytes(MOR_GRA_FIXTURE, vec![0xFF]);
                Ok(())
            },
        )
        .refusing(
            "a corpus of one empty file, so the census is empty",
            "no construct pairs found",
            |edit| {
                edit.hide_files_under(REFERENCE_CORPUS);
                edit.write(&format!("{REFERENCE_CORPUS}/probe-empty.cha"), "");
                Ok(())
            },
        )
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}
