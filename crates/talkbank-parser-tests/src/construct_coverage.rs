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
//! The historical node-kind measurement was **92.3%**. That percentage did not
//! answer the combination question: the parser defects examined on 2026-08-08
//! involved combinations rather than missing individual constructs:
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
//! # The finite combination ratchet
//!
//! `PENDING_ATTESTATION` retains the nineteen combinations observed in a
//! historical sample. All nineteen now have named authored controls. Evidence
//! states remain distinct: authored examples do not
//! discharge an attested-reference obligation. New structural coverage must be
//! reviewed for support and document policy (separator padding is legal in CA
//! but reports E758 otherwise).
//!
//! Discovery of new combinations belongs to a separately authorized corpus
//! investigation, not this finite test. Neither an empty pending list nor a
//! passing incidence check proves complete grammar or implementation coverage.
//!
//! # Attestation remains a separate obligation
//!
//! Retiring an entry requires a supported, legal, attested fixture with reviewed
//! provenance and redistribution permission. Authored controls are useful for
//! independently checking intended behavior but cannot supply that provenance.
//! The Cantonese pair in `corpus/reference/tiers/wor.cha` is an existing example;
//! it does not attest the independently authored ending-combination documents.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::gate::tree::RelPath;
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, Tier, Tree, UnprovenRule};
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

/// Historical combinations whose attested-reference obligations remain open.
///
/// The retained inventory records a 400-file sample dated 2026-08-08. This
/// gate measures current structural witnesses; it does not repeat that sample
/// or infer validity, provenance or redistribution permission from incidence.
///
/// Authored coverage and attestation are different evidence states.
/// Fixed-size storage retains all nineteen historical obligations.
const PENDING_ATTESTATION: [PendingAttestation; 19] = [
    PendingAttestation::mor("trailing_off"),
    PendingAttestation::mor("interruption"),
    PendingAttestation::mor("self_interruption"),
    PendingAttestation::padding("main_tier"),
    PendingAttestation::mor("trailing_off_question"),
    PendingAttestation::mor("interrupted_question"),
    PendingAttestation::mor("self_interrupted_question"),
    PendingAttestation::wor("quoted_new_line"),
    PendingAttestation::wor("self_interruption"),
    PendingAttestation::wor("trailing_off"),
    PendingAttestation::wor("interruption"),
    PendingAttestation::wor("tag_marker"),
    PendingAttestation::wor("interrupted_question"),
    PendingAttestation::padding("tier_sep"),
    PendingAttestation::mor("broken_question"),
    PendingAttestation::colon(),
    PendingAttestation::wor("trailing_off_question"),
    PendingAttestation::wor("self_interrupted_question"),
    PendingAttestation::mor("break_for_coding"),
];

#[derive(Clone, Copy)]
enum AuthoredReference {
    MorEndings,
    WorEndings,
    CaSeparators,
    ColonSeparator,
}

impl AuthoredReference {
    const fn path(self) -> &'static str {
        match self {
            Self::MorEndings => "corpus/reference/tiers/mor-ending-combinations.cha",
            Self::WorEndings => "corpus/reference/tiers/wor-ending-combinations.cha",
            Self::CaSeparators => "corpus/reference/ca/separator-padding.cha",
            Self::ColonSeparator => "corpus/reference/content/colon-separator.cha",
        }
    }
}

/// Every historical obligation owns an authored witness, but none is attested.
struct PendingAttestation {
    pair: NamedConstructPair,
    reference: AuthoredReference,
}

impl PendingAttestation {
    const fn colon() -> Self {
        Self {
            pair: NamedConstructPair {
                parent: "separator",
                child: "colon",
            },
            reference: AuthoredReference::ColonSeparator,
        }
    }
    const fn padding(parent: &'static str) -> Self {
        Self {
            pair: NamedConstructPair {
                parent,
                child: "sep_trailing_space",
            },
            reference: AuthoredReference::CaSeparators,
        }
    }
    const fn mor(child: &'static str) -> Self {
        Self {
            pair: NamedConstructPair {
                parent: "mor_contents",
                child,
            },
            reference: AuthoredReference::MorEndings,
        }
    }
    const fn wor(child: &'static str) -> Self {
        Self {
            pair: NamedConstructPair {
                parent: "wor_tier_body",
                child,
            },
            reference: AuthoredReference::WorEndings,
        }
    }
}

/// Measure each authored source once, without allowing another file to stand in.
struct AuthoredCoverage {
    mor: CoveredPairs,
    wor: CoveredPairs,
    ca_separators: CoveredPairs,
    colon_separator: CoveredPairs,
}

impl AuthoredCoverage {
    fn observe(tree: &ReadTree) -> Result<Self, String> {
        Ok(Self {
            mor: pairs_in(tree, &[RelPath::new(AuthoredReference::MorEndings.path())])?,
            wor: pairs_in(tree, &[RelPath::new(AuthoredReference::WorEndings.path())])?,
            ca_separators: pairs_in(
                tree,
                &[RelPath::new(AuthoredReference::CaSeparators.path())],
            )?,
            colon_separator: pairs_in(
                tree,
                &[RelPath::new(AuthoredReference::ColonSeparator.path())],
            )?,
        })
    }

    fn pairs(&self, reference: AuthoredReference) -> &CoveredPairs {
        match reference {
            AuthoredReference::MorEndings => &self.mor,
            AuthoredReference::WorEndings => &self.wor,
            AuthoredReference::CaSeparators => &self.ca_separators,
            AuthoredReference::ColonSeparator => &self.colon_separator,
        }
    }
}

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
        "`resolve_named_kind` compares `PENDING_ATTESTATION` against \
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
        "The historical list is compile-time policy. Input files cannot discover \
         an omitted pair or justify deleting an obligation. Attestation retirement \
         needs reviewed source evidence outside this structural gate.",
    ),
    UnprovenRule::new(
        "the summary counts",
        "The covered-pair and authored counts summarize observations, \
         not completeness. The nineteen-entry array preserves the historical \
         obligation inventory; no numeric threshold establishes attestation.",
    ),
];

/// The corpus file one probe writes invalid bytes into, and whose name that
/// same probe requires the failure to carry.
/// The invalid-byte probe requires the failure to name it.
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

        let authored = match AuthoredCoverage::observe(&tree) {
            Ok(authored) => authored,
            Err(why) => {
                return Outcome::failed(format!("cannot measure authored witnesses: {why}"));
            }
        };
        for PendingAttestation { pair, reference } in &PENDING_ATTESTATION {
            match authored.pairs(*reference).contains_named(*pair) {
                Ok(true) => {}
                Ok(false) => {
                    return Outcome::failed(format!(
                        "authored witness missing {pair} in {}",
                        reference.path()
                    ));
                }
                Err(why) => return Outcome::failed(why),
            }
        }
        tree.clean(format!(
            "{} construct pair(s) covered; {} named authored witness(es); {} attestation obligation(s) remain",
            covered.len(), PENDING_ATTESTATION.len(), PENDING_ATTESTATION.len(),
        ))
    }

    /// Remove the two authored separator-padding witnesses and
    /// retain the measurement-refusal, absent-corpus and accepted-edit controls.
    /// The padding tokens have no equal-length competitor in their positions.
    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "the authored CA reference losing main-tier separator padding",
            "authored witness missing main_tier -> sep_trailing_space",
            |edit| {
                edit.replace_once(
                    AuthoredReference::CaSeparators.path(),
                    "*CHI:\t  dog .",
                    "*CHI:\tdog .",
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
        .accepting(
            "an @ID SES ethnicity already covered by the reference corpus",
            |edit| {
                edit.replace_once(
                    "corpus/reference/core/headers-time-and-types.cha",
                    "White,MC",
                    "White",
                )
            },
        )
        .refusing(
            "the authored CA reference losing dependent-tier separator padding",
            "authored witness missing tier_sep -> sep_trailing_space",
            |edit| {
                edit.replace_once(
                    AuthoredReference::CaSeparators.path(),
                    "%mor:\t  n|dog .",
                    "%mor:\tn|dog .",
                )?;
                edit.replace_once(
                    AuthoredReference::CaSeparators.path(),
                    "%com:\t  tier text",
                    "%com:\ttier text",
                )
            },
        )
        .refusing(
            "the authored colon reference losing its standalone separator",
            "authored witness missing separator -> colon",
            |edit| {
                edit.replace_once(
                    AuthoredReference::ColonSeparator.path(),
                    "hello : world",
                    "hello: world",
                )
            },
        )
        .refusing(
            "an authored MOR reference losing its declared trailing-off pair",
            "authored witness missing mor_contents -> trailing_off",
            |edit| {
                edit.replace_once(
                    AuthoredReference::MorEndings.path(),
                    "%mor:\tco|hello +...\n",
                    "%mor:\tco|hello .\n",
                )
            },
        )
        .refusing(
            "an authored WOR reference losing its declared tag marker",
            "authored witness missing wor_tier_body -> tag_marker",
            |edit| {
                edit.replace_once(
                    AuthoredReference::WorEndings.path(),
                    "%wor:\thello „ .\n",
                    "%wor:\thello .\n",
                )
            },
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
