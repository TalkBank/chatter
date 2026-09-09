//! Find `match` blocks over the content enums that end in a `_ =>` catch-all.
//!
//! Design rule 3 in the repository's CLAUDE.md says: "Exhaustive matches on
//! `UtteranceContent`/`BracketedItem`: no `_ =>` catch-alls that discard
//! content; all group types recurse." That rule was prose, and prose does not
//! fire. This makes it countable.
//!
//! # Why it matters, with receipts
//!
//! Every instance is a place where adding a content variant compiles clean and
//! answers wrong. Four have already shipped as defects:
//!
//! - `validation/retrace/detection.rs` `_ => false` gated ALL retrace
//!   validation, so `<the> [/] [= gloss] .` validated clean.
//! - `alignment/.../units.rs` `_ => 0`, twice, a second implementation of
//!   `count.rs` that disagreed with it for 8,766 utterances.
//! - `re2c/convert/text_tiers.rs` `_ => {}`, twice, skipping normalisation the
//!   tree-sitter side performs.
//! - `validation/temporal.rs` `_ => false` treated retraced speech as
//!   untranscribed, dropping whole utterances out of the E704 timing check.
//!
//! None was found by a test. Each was found by a human or an agent reading two
//! walkers against each other.
//!
//! # This is the INVENTORY. The compiler is the enforcement.
//!
//! `clippy::wildcard_enum_match_arm` says exactly this rule, names each site,
//! suggests the full variant list, and runs in the clippy pass CI already
//! does. The workspace already denies six restriction lints the same way
//! (`unwrap_used`, `panic`, ...), so design rule 3 was the only rule of its
//! class not enforced like its siblings. It is now denied per file, added as
//! each file is cleaned, which is a real ratchet: a new catch-all in a
//! protected module is a COMPILE ERROR, not a number someone must notice.
//!
//! This module survives as the thing that lists the modules still to clean,
//! which the lint cannot do (an undenied file is silent).
//!
//! # Why this is a library module and not a binary
//!
//! It began as a `[[bin]]` whose check lived in `main`. CI runs
//! `cargo test --workspace --tests` and never `cargo run`, so the check
//! asserted nothing anywhere while reading like a gate in every doc citing it.
//!
//! The crate already had the answer, in `conformance_inventory`: the logic is
//! a library module, the runnable entry point is a thin renderer, and the GATE
//! is a `tests/integration/` module calling the library directly. Following it
//! keeps `test = false` a uniform rule for every bin in this crate, so no
//! future audit has to reason about cargo target selection to be run, and it
//! avoids launching one more test executable (the per-binary first-execution
//! cost on macOS that this crate's Cargo.toml comment exists to explain).
//!
//! The gate is `tests/integration/content_catch_alls.rs`; the renderer is
//! `src/bin/audit_content_catch_alls.rs`.

use std::collections::BTreeSet;

use crate::gate::tree::{RelPath, TreeError};
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, UnprovenRule, listing, report};

/// The files still carrying a content-enum catch-all.
///
/// # A set of paths, not a count
///
/// This was `const BASELINE: usize`, and the module doc above already argued
/// against it: a scalar over a heterogeneous set means fixing a harmless LSP
/// hover formatter FREES A SLOT for a new catch-all in a validator, and the
/// total never moves. Once that sentence is written the constant should stop
/// existing.
///
/// Paths are stable where a count is fungible, and the set is checked in BOTH
/// directions:
///
/// - a catch-all in a file that is not listed FAILS, and the file is named, so
///   a new one cannot hide behind somebody else's cleanup;
/// - a listed file with no catch-alls left FAILS, so the list cannot rot into
///   a permanent exemption. Delete the entry in the commit that cleans it.
///
/// The finish line is this list being empty, which a number could never show.
/// Within-file counts are deliberately not tracked: once a file is clean it
/// gains `#![deny(clippy::wildcard_enum_match_arm)]` and the compiler takes
/// over, so the only question this list answers is "which files are still
/// outside the lint".
pub const UNPROTECTED: &[&str] = &[
    "crates/chatter/src/commands/alignment/helpers.rs",
    "crates/talkbank-lsp/src/alignment/finders.rs",
    "crates/talkbank-lsp/src/alignment/formatters/content.rs",
    "crates/talkbank-lsp/src/backend/requests/alignment_sidecar.rs",
    "crates/talkbank-model/src/model/file/utterance/accessors.rs",
    "crates/talkbank-parser-re2c/src/parser/entry_points.rs",
    "crates/talkbank-parser-re2c/src/parser/file.rs",
    "crates/talkbank-parser-tests/src/bin/generate_golden_words.rs",
    "crates/talkbank-model/src/model/content/word/word_type.rs",
    "crates/talkbank-model/src/validation/utterance/repetition_segment.rs",
    "crates/talkbank-model/src/validation/word/structure.rs",
    "crates/talkbank-parser/src/parser/tree_parsing/main_tier/word/mod.rs",
    "crates/talkbank-parser/src/parser/tree_parsing/main_tier/structure/contents.rs",
];

/// The enums a catch-all must never be written over.
///
/// `ContentItem` and its siblings were MISSING until 2026-08-08, and the gap was
/// not theoretical: `validation/utterance/comma.rs` carried two `_ =>` arms over
/// `ContentItem` and never appeared on [`UNPROTECTED`], because the scan only
/// looked for the two model enums. The walk layer's items are the same closed
/// content vocabulary one level up, and they are what a traversal-merge extends,
/// so a blind spot there is a blind spot exactly where it costs most.
///
/// `ContentItem::` deliberately matches TWO distinct types: the walk layer's
/// item enum in `talkbank-model`, and `talkbank-parser-re2c`'s own
/// parser-internal one. The scan is textual and cannot tell them apart, and it
/// does not need to: both are closed content vocabularies where a `_ =>` drops
/// content, which is the whole rule. Widening this list made three real re2c
/// catch-alls visible for the first time; they are listed below as work
/// remaining, not fixed here.
///
/// `WordContent::` was added 2026-08-08 for the same reason, and surfaced seven
/// more files. The one that prompted it is
/// `validation/utterance/underline.rs`, whose word-content loop ends in
/// `_ => {}` while a neighbouring module's docs described that same traversal
/// as "exhaustive and correct". A word's contents are the same closed content
/// vocabulary one level further in, and underline markers live THERE, so a
/// blind spot at that level is a blind spot in the one scoped-marker family
/// this repo has not yet unified.
const CONTENT_ENUMS: &[&str] = &[
    "UtteranceContent::",
    "BracketedItem::",
    "ContentItem::",
    "ContentItemMut::",
    "WordItem::",
    "WordContent::",
];

/// A `match` block whose own arms name a content enum and which also has a
/// catch-all.
pub struct CatchAll {
    /// The file the catch-all sits in, workspace-relative.
    pub file: RelPath,
    /// 1-indexed line of the `match` keyword whose arms carry the catch-all.
    ///
    /// The MATCH, not the arm. It said "the `_ =>` arm" and computed the other
    /// thing, and an `UnprovenRule` was added recording the disagreement rather
    /// than correcting the sentence, which is the stale-claim-corrected-in-place
    /// rule broken inside the module that enforces it. The match is also the
    /// more useful anchor: the whole block is what an operator rewrites.
    pub line: usize,
}

/// What a sweep of the tree established.
///
/// A sum type rather than hits beside a list of problems, because the two
/// states support different questions: an incomplete sweep has no trustworthy
/// count, and the previous shape let a caller read `found.len()` without
/// consulting `unreadable` first. That is the same "clean result from a failed
/// measurement" shape `guard-silent-failure.sh` exists to refuse, so the type
/// refuses it instead.
enum Sweep {
    /// Every candidate file was read, so the hits are exact.
    Measured(Vec<CatchAll>),
    /// At least one file could not be read, so any count would be a FLOOR.
    ///
    /// [`TreeError`] rather than a local sum of the same cases. This module
    /// grew its own `Unreadable { Walk(String), File(TreeError) }`, which drew
    /// the identical distinction one level up, wrapped the real error, and
    /// stringified the walk case: the operator saw the prefix twice ("walk
    /// error: could not walk crates: ..."), and the typed directory the walk
    /// failed under was thrown away at the one place it was known.
    Incomplete(Vec<TreeError>),
}

impl Sweep {
    /// Walk `crates/` and record every content-enum catch-all.
    fn run(tree: &ReadTree) -> Self {
        let mut hits = Vec::new();
        let mut unreadable = Vec::new();

        let files = match tree.files_under("crates") {
            Ok(files) => files,
            // A walk failure is not a smaller file set: it is an unknown
            // number of files never offered. The predecessor pushed it onto
            // the same list and carried on, which meant a count taken after
            // one could still be reported as if it were a measurement.
            Err(err) => return Self::Incomplete(vec![err]),
        };
        for file in files {
            if !file.extension_is("rs") {
                continue;
            }
            // Test code is exempt: a test matching one variant it cares about
            // is not a traversal that can silently drop content.
            let as_str = file.as_str();
            if as_str.contains("/tests/") || as_str.contains("/generated/") {
                continue;
            }
            match tree.read_to_string(&file) {
                Ok(source) => hits.extend(scan(file, &source)),
                Err(err) => unreadable.push(err),
            }
        }

        if unreadable.is_empty() {
            hits.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            Self::Measured(hits)
        } else {
            Self::Incomplete(unreadable)
        }
    }
}

/// Whether [`UNPROTECTED`] and the tree agree.
///
/// TWO variants, not a struct of two vectors plus an emptiness test. The
/// predecessor could represent "drifted, with nothing in either direction",
/// which means clean, so one fact had two spellings and a caller could read
/// the wrong one. In the module whose entire job is refusing that shape, it
/// was worth one more enum.
pub enum Agreement {
    /// The list names precisely the files carrying a catch-all.
    Exact,
    /// They disagree; at least one vector is non-empty by construction.
    Drifted {
        /// Carrying a catch-all but not listed: a NEW one, the regression.
        appeared: Vec<RelPath>,
        /// Listed but carrying none: a stale exemption to delete.
        cleaned: Vec<RelPath>,
    },
}

impl Agreement {
    /// Compare the two sets, in both directions.
    ///
    /// Both sides are `&str`, so this is two `BTreeSet::difference` calls: a
    /// sorted merge. The predecessor held one side as `BTreeSet<&RelPath>`,
    /// which left the two directions spelled differently (`contains` one way,
    /// a linear `any` with a manual `as_str` the other) for no reason beyond
    /// the element types not matching.
    fn between(listed: &BTreeSet<&str>, carrying: &BTreeSet<&str>) -> Self {
        let own = |file: &&str| RelPath::new(file);
        let appeared: Vec<RelPath> = carrying.difference(listed).map(own).collect();
        let cleaned: Vec<RelPath> = listed.difference(carrying).map(own).collect();
        if appeared.is_empty() && cleaned.is_empty() {
            Self::Exact
        } else {
            Self::Drifted { appeared, cleaned }
        }
    }
}

/// Everything one run established: the sites, and whether the list agrees.
///
/// This is the value `main` used to compute and throw away, so the tool built
/// to enforce design rule 3 was itself an instance of "a total function
/// silently discards information". Naming it is what lets the gate assert the
/// same thing the command reports rather than a re-derivation that could
/// drift. Counts are deliberately NOT stored: `hits` already holds them, and
/// two `usize` fields side by side counting different things is the shape that
/// once reported "92 files checked" having checked 91.
pub enum Audit {
    /// The sweep failed, so no count from it means anything.
    Unmeasurable(Vec<TreeError>),
    /// The tree was measured; `agreement` says whether the list matches.
    Measured {
        /// Every catch-all the sweep found, in path order.
        hits: Vec<CatchAll>,
        /// Whether those hits match [`UNPROTECTED`] in both directions.
        agreement: Agreement,
    },
}

impl Audit {
    /// Sweep the tree and compare against [`UNPROTECTED`].
    pub fn of(tree: &ReadTree) -> Self {
        let hits = match Sweep::run(tree) {
            Sweep::Incomplete(unreadable) => return Self::Unmeasurable(unreadable),
            Sweep::Measured(hits) => hits,
        };
        let listed: BTreeSet<&str> = UNPROTECTED.iter().copied().collect();
        let carrying: BTreeSet<&str> = hits.iter().map(|hit| hit.file.as_str()).collect();
        let agreement = Agreement::between(&listed, &carrying);
        Self::Measured { hits, agreement }
    }

    /// Every site found, for the renderer to list. Empty when unmeasurable,
    /// which is correct: there is nothing trustworthy to list.
    pub fn hits(&self) -> &[CatchAll] {
        match self {
            Self::Unmeasurable(_) => &[],
            Self::Measured { hits, .. } => hits,
        }
    }

    /// The operator-facing result: `Ok` is the clean summary, `Err` is what to
    /// do about it.
    ///
    /// One call, not an `is_clean()` predicate beside a `report()` string. Two
    /// calls let a caller pair the wrong answers, and reducing a sum type to a
    /// bool at the seam throws away which failure it was. The renderer prints
    /// it and the gate IS it, so the two cannot disagree.
    /// The tree is a PARAMETER, and the witness is asked of it here rather
    /// than snapshotted into `Measured` when the sweep ran. A field would have
    /// been the hand-carried witness that [`crate::gate::Outcome`] refuses to
    /// offer: a summary about one tree paired with evidence earned by another,
    /// reconstructed twenty lines from the type built to forbid it.
    pub fn outcome(&self, tree: &ReadTree) -> Outcome {
        let (hits, agreement) = match self {
            Self::Unmeasurable(unreadable) => {
                let mut out = format!(
                    "FAIL: {} file(s) could not be read, so any count is a FLOOR,\n\
                     not a measurement. Refusing to report a number that a read\n\
                     failure could have lowered:",
                    unreadable.len()
                );
                for problem in unreadable {
                    out.push_str(&format!("\n  {problem}"));
                }
                return Outcome::failed(out);
            }
            Self::Measured { hits, agreement } => (hits, agreement),
        };

        let (appeared, cleaned) = match agreement {
            Agreement::Exact => {
                let files: BTreeSet<&str> = hits.iter().map(|hit| hit.file.as_str()).collect();
                return tree.clean(format!(
                    "content-enum catch-alls: {} across {} file(s); \
                     all listed in UNPROTECTED",
                    hits.len(),
                    files.len()
                ));
            }
            Agreement::Drifted { appeared, cleaned } => (appeared, cleaned),
        };

        Outcome::failed(report([
            listing(
                &format!(
                    "FAIL: {} file(s) gained a content-enum catch-all.\n\
                     A `_ =>` over UtteranceContent or BracketedItem means a future\n\
                     content variant compiles clean and answers wrong. List the arms\n\
                     instead; design rule 3.",
                    appeared.len()
                ),
                appeared,
            ),
            listing(
                &format!(
                    "FAIL: {} file(s) are listed as unprotected but carry no catch-all.\n\
                     Delete them from UNPROTECTED in the commit that cleaned them, and\n\
                     add `#![deny(clippy::wildcard_enum_match_arm)]` so the compiler\n\
                     keeps them clean. A list that outlives its entries becomes a\n\
                     permanent exemption.",
                    cleaned.len()
                ),
                cleaned,
            ),
        ]))
    }
}

/// Find every `match` block in `source` whose OWN top-level arms name a
/// content enum and which also carries a catch-all.
///
/// Brace-balanced and depth-aware on purpose. A first cut looked back a fixed
/// forty lines from each `_ =>` and reported 41 hits, sixteen of which were
/// matches over `Token`, `Separator` or `PauseDuration` that merely happened to
/// sit near a content-enum reference. Proximity is not scope.
fn scan(file: RelPath, source: &str) -> Vec<CatchAll> {
    // BYTES, not chars. `str::find` returns a BYTE offset, and an earlier
    // version indexed a `Vec<char>` with it. The two agree only in pure-ASCII
    // files, so every file containing a CHAT example (guillemets, bullets)
    // silently scanned from the wrong place and the audit under-reported
    // 25 sites as 13. Braces are ASCII, so byte scanning is exact.
    let bytes = source.as_bytes();
    let mut hits = Vec::new();
    let mut idx = 0usize;

    while let Some(rel) = source[idx..].find("match ") {
        let start = idx + rel;
        idx = start + "match ".len();

        // The scrutinee runs to the block's opening brace.
        let Some(brace_rel) = source[start..].find('{') else {
            break;
        };
        let open = start + brace_rel;

        // Balance braces to find the block's end.
        // Saturating throughout: the word "match" also occurs inside strings
        // and comments, so this scanner is fed text that is not always a real
        // match block and must not panic on it.
        let mut depth = 0usize;
        let mut cursor = open;
        let mut close = None;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'{' => depth = depth.saturating_add(1),
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(cursor);
                        break;
                    }
                }
                _ => {}
            }
            cursor = cursor.saturating_add(1);
        }
        let Some(close) = close else { continue };

        // Only the block's OWN arms count, so collect the text at depth 1.
        let mut own = String::new();
        let mut depth = 0usize;
        for &ch in &bytes[open..close] {
            match ch {
                b'{' => depth = depth.saturating_add(1),
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 1 {
                own.push(char::from(ch));
            }
        }

        // Only the PATTERN side of each arm counts. An arm may name a content
        // enum because it MATCHES on it, or because it CONSTRUCTS one in its
        // body, and those are opposite facts: `Token::Word(w) => ContentItem::Word(w)`
        // is a match over `Token`. Checking the whole arm text conflated them, and
        // widening the enum list to `ContentItem::` immediately produced three
        // false positives in the re2c parser, whose job is exactly to build
        // content items out of tokens. Splitting at `=>` keeps multi-line `|`
        // pattern lists intact, since those lines have no `=>` at all.
        let patterns: String = own
            .lines()
            .filter_map(|line| line.split("=>").next())
            .collect::<Vec<_>>()
            .join("\n");
        let names_content = CONTENT_ENUMS.iter().any(|name| patterns.contains(name));
        if names_content && has_catch_all(&own) {
            hits.push(CatchAll {
                file: file.clone(),
                line: source[..start].matches('\n').count() + 1,
            });
        }
    }
    hits
}

/// Whether the arm text contains a bare `_ =>` arm.
///
/// Anchored to a line start so `Some(_) =>` and `Self::Word(_) =>` do not
/// count: those name a variant and stay exhaustive.
fn has_catch_all(arms: &str) -> bool {
    arms.lines()
        .any(|line| line.trim_start().starts_with("_ =>") || line.trim_start().starts_with("_=>"))
}

/// Design rule 3, as a registered gate.
pub struct CatchAllGate;

/// Rust source for a probe fixture, built by FORMATTING rather than written as
/// a literal.
///
/// The reason is this file's own sweep: a literal containing a content-enum
/// name inside a `match` block, beside a `_ =>` line at depth 1, would be a
/// hit in THIS file, which is not listed in [`UNPROTECTED`], so the gate would
/// fail on the live tree. A probe that breaks its own gate is worse than no
/// probe. With the enum named by a parameter and substituted after formatting,
/// no such substring exists here.
///
/// None of these fixtures is ever compiled: the harness writes them into an
/// in-memory overlay, and no `mod` declaration names them.
fn probe_source(enum_name: &str, arms: &str) -> String {
    format!(
        "//! A probe fixture. Nothing declares it as a module.\n\
         fn probe(item: &Item) -> usize {{\n\
         {arms}\n\
         }}\n"
    )
    .replace("ENUM", enum_name)
}

/// One `match` over the named enum whose last arm is a catch-all.
fn catch_all_over(enum_name: &str, catch_all_arm: &str) -> String {
    probe_source(
        enum_name,
        &format!("    match item {{\n        ENUM::Word(_) => 1,\n        {catch_all_arm}\n    }}"),
    )
}

/// Rules of this gate no plant can reach.
///
/// See [`crate::gate::UnprovenRule`]. Two of these are genuine holes in the
/// rule rather than in the probes, and both are named as such: the sweep does
/// not leave `crates/`, and a bound wildcard is a catch-all the scan cannot
/// see.
const UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "the `/generated/` and non-`.rs` exclusions",
        "They can only cause a MISS, never a report, so no must-fail probe \
         reaches them. The sibling `/tests/` exclusion IS covered, from the \
         other side, by a must-pass probe.",
    ),
    UnprovenRule::new(
        "`RelPath`'s forward-slash normalisation",
        "Unreachable on macOS or Linux: it fires only on Windows, where a raw \
         `to_string_lossy` made the gate report every listed file as newly \
         gaining a catch-all. Its unit test in `gate::tree` is the coverage.",
    ),
    UnprovenRule::new(
        "`CatchAll.line`, which the verdict never reads",
        "The gate compares FILE SETS only, so no probe of `check` reaches the \
         line arithmetic; only the renderer bin prints it. The entry used to \
         add that the field disagreed with its own doc comment, which was a \
         defect to fix rather than a coverage gap to record, and the doc now \
         says what the code computes.",
    ),
    UnprovenRule::new(
        "SCOPE: everything outside `crates/`",
        "A LIVE HOLE, not a probe gap. `spec/`, `apps/`, `xtask/` and the root \
         `tests/` are never swept, and `spec/runtime-tools/src/bin/ca_census.rs` \
         carries real catch-alls today while the gate reports Exact. The \
         ratchet's finish line, `UNPROTECTED` empty, is reachable with those \
         still live. The fix is a code change, not a probe.",
    ),
    UnprovenRule::new(
        "VACUITY: nothing asserts the sweep read any FILE",
        "Half of this closed when a clean verdict began requiring an \
         `Examined`: a sweep that read nothing at all can no longer report \
         clean, and every clean summary now prints what was examined, so \
         `0 across 0 file(s)` no longer reads like a working sweep. What is \
         still missing is a FLOOR. Enumerating `crates/` counts as one \
         examination whether it offers 812 files or none, so a sweep that \
         reaches no file still passes, and the gate is saved today only by \
         `UNPROTECTED` being non-empty, which makes an empty sweep fail as \
         thirteen stale entries. The floor is a code change, not a probe.",
    ),
    UnprovenRule::new(
        "a BOUND wildcard (`_other =>`) is a catch-all the scan cannot see",
        "A real weakness in the detection rule, not merely an unprobed one. It \
         is what makes the stale-exemption probe expressible at all, and it is \
         why that probe can clear a file without deleting its arm.",
    ),
];

/// The file two probes plant a catch-all into, and whose PATH those same two
/// probes require the failure report to name.
///
/// A const rather than four string literals, because two of the four are the
/// plant and two are the assertion: a rename that missed one would leave a
/// probe asserting on a path nothing writes, which passes only if the gate
/// fails for some other reason. The sibling suite in `test_hygiene::probes`
/// already had this convention.
const PROBE_CATCH_ALL: &str = "crates/talkbank-model/src/probe_gate_catch_all.rs";

impl Gate for CatchAllGate {
    fn name(&self) -> &'static str {
        "content-enum catch-alls (design rule 3)"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        Audit::of(&tree).outcome(&tree)
    }

    /// Both directions of the ratchet, three of the six vocabulary entries, and
    /// three must-pass probes for sub-rules whose job is NOT to fire.
    ///
    /// The must-fail plants REPLACE arms rather than adding a `_ =>` beside an
    /// exhaustive list. That matters if these fixtures are ever materialised on
    /// disk instead of in the overlay: `unreachable_patterns` is denied
    /// workspace-wide, so an ADDED redundant arm is a compile error, and a
    /// harness would record a build failure as though it were a gate failure.
    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a WordItem catch-all in a file that is not listed",
            "design rule 3",
            |edit| {
                edit.replace_once(
                    "crates/talkbank-transform/src/fix_s.rs",
                    "        WordItem::Separator(_) => {}",
                    "        _ => {}",
                )
            },
        )
        .refusing(
            "a catch-all in a file the sweep must DISCOVER, not merely re-read",
            PROBE_CATCH_ALL,
            |edit| {
                edit.write(
                    PROBE_CATCH_ALL,
                    catch_all_over("UtteranceContent", "_ => 0,"),
                );
                Ok(())
            },
        )
        .refusing(
            "the second catch-all spelling, `_=>` with no space",
            PROBE_CATCH_ALL,
            |edit| {
                edit.write(PROBE_CATCH_ALL, catch_all_over("WordContent", "_=> 0,"));
                Ok(())
            },
        )
        .refusing(
            "a listed file that no longer carries a catch-all",
            "permanent exemption",
            |edit| {
                edit.replace_once(
                    "crates/talkbank-parser-tests/src/bin/generate_golden_words.rs",
                    "                    _ => {}",
                    "                    _other => {}",
                )
            },
        )
        .refusing(
            "the swept directory cannot be ENUMERATED",
            "could not be read, so any count is a FLOOR",
            |edit| {
                edit.fail_walk_under("crates");
                Ok(())
            },
        )
        .refusing(
            "a swept file that cannot be read",
            "could not be read, so any count is a FLOOR",
            |edit| {
                edit.write_bytes(
                    "crates/talkbank-model/src/probe_gate_unreadable.rs",
                    vec![0xFF, 0xFE],
                );
                Ok(())
            },
        )
        .accepting(
            "an inner match's catch-all does not implicate the outer content match",
            |edit| {
                edit.write(
                    "crates/talkbank-model/src/probe_gate_nested.rs",
                    probe_source(
                        "BracketedItem",
                        "    match item {\n        \
                             ENUM::Word(w) => match w.kind {\n            \
                             Kind::Plain => 1,\n            _ => 0,\n        },\n        \
                             ENUM::Pause(_) => 0,\n    }",
                    ),
                );
                Ok(())
            },
        )
        .accepting(
            "an arm that CONSTRUCTS a content item is a match over something else",
            |edit| {
                edit.write(
                    "crates/talkbank-model/src/probe_gate_constructing.rs",
                    probe_source(
                        "ContentItem",
                        "    match token {\n        \
                             Token::Word(w) => ENUM::Word(w),\n        _ => 0,\n    }",
                    ),
                );
                Ok(())
            },
        )
        .accepting(
            "a catch-all under a `tests/` directory is out of scope",
            |edit| {
                edit.write(
                    "crates/talkbank-model/tests/probe_gate_catch_all.rs",
                    catch_all_over("UtteranceContent", "_ => 0,"),
                );
                Ok(())
            },
        )
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}

#[cfg(test)]
mod tests {
    use super::{has_catch_all, scan};
    use crate::gate::tree::RelPath;

    /// SURVIVES: behaviour. `scan` is brace-balanced and depth-aware, which no
    /// signature describes; this pins that a catch-all in an INNER match over
    /// some other enum does not implicate the outer content match, which is
    /// the distinction the fixed-window predecessor got wrong on 16 of 41
    /// reported sites.
    #[test]
    fn an_inner_matchs_catch_all_does_not_implicate_the_outer_block() {
        let source = r#"
fn outer(item: &UtteranceContent) -> usize {
    match item {
        UtteranceContent::Word(w) => match w.kind {
            Kind::Plain => 1,
            _ => 0,
        },
        UtteranceContent::Pause(_) => 0,
    }
}
"#;
        let hits = scan(RelPath::new("probe.rs"), source);
        assert!(
            hits.is_empty(),
            "the outer match is exhaustive; the `_ =>` belongs to `Kind`, at depth 2"
        );
    }

    /// SURVIVES: behaviour. The arm-text predicate is line-anchored so a
    /// binding pattern such as `Some(_) =>` stays exhaustive; that anchoring
    /// is a property of the string scan, not of any type.
    #[test]
    fn a_binding_underscore_is_not_a_catch_all() {
        assert!(has_catch_all("    _ => todo!(),"));
        assert!(has_catch_all("_=> 0,"));
        assert!(!has_catch_all("    Some(_) => 1,"));
        assert!(!has_catch_all("    Self::Word(_) => 1,"));
    }
}
