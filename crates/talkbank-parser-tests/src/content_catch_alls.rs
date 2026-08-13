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
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::gate::{Gate, GateOutcome, listing, report};
use crate::repo_paths::workspace_root;

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
    "crates/talkbank-lsp/src/alignment/tier_hover/main_tier.rs",
    "crates/talkbank-lsp/src/backend/diagnostics/cache_builder.rs",
    "crates/talkbank-lsp/src/backend/features/highlights/range_finders.rs",
    "crates/talkbank-lsp/src/backend/features/highlights/tier_handlers.rs",
    "crates/talkbank-lsp/src/backend/requests/alignment_sidecar.rs",
    "crates/talkbank-lsp/src/backend/requests/goto_definition.rs",
    "crates/talkbank-model/src/model/file/utterance/accessors.rs",
    "crates/talkbank-parser-re2c/src/parser/entry_points.rs",
    "crates/talkbank-parser-re2c/src/parser/file.rs",
    "crates/talkbank-parser-tests/src/bin/generate_golden_words.rs",
    "crates/talkbank-model/src/model/content/word/word_type.rs",
    "crates/talkbank-model/src/validation/utterance/repetition_segment.rs",
    "crates/talkbank-model/src/validation/word/structure.rs",
    "crates/talkbank-parser/src/parser/tree_parsing/main_tier/word/mod.rs",
    "crates/talkbank-transform/src/capitalize.rs",
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

/// A workspace-relative path: the one spelling in which a file is listed in
/// [`UNPROTECTED`], compared against it, and printed.
///
/// Both absolute and relative paths were in play, converted at three separate
/// call sites by a `relative(&root, ..)` helper. A comparison that forgot the
/// call would compile and silently never match, which in a ratchet reads as
/// "clean". Converting once, where the hit is recorded, removes the chance.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepoPath(String);

impl RepoPath {
    /// Strip the workspace root, the only way a `RepoPath` is made from a
    /// filesystem path.
    fn of(root: &Path, file: &Path) -> Self {
        Self(
            file.strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .into_owned(),
        )
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepoPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A `match` block whose own arms name a content enum and which also has a
/// catch-all.
pub struct CatchAll {
    /// The file the catch-all sits in, workspace-relative.
    pub file: RepoPath,
    /// 1-indexed line of the `_ =>` arm.
    pub line: usize,
}

/// Why a candidate file did not contribute to the sweep.
///
/// Named cases rather than a preformatted `String`, because the two call for
/// different operator actions and a walk failure is the worse one: it means an
/// unknown number of files were never offered at all, where a read failure
/// names exactly what was lost.
pub enum Unreadable {
    /// The directory walk itself failed. How many files it skipped is not
    /// knowable from here.
    Walk(String),
    /// One named file could not be read.
    File {
        /// The file that could not be read, workspace-relative.
        path: RepoPath,
        /// The underlying IO failure, as reported.
        error: String,
    },
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Walk(error) => write!(f, "walk error: {error}"),
            Self::File { path, error } => write!(f, "{path}: {error}"),
        }
    }
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
    Incomplete(Vec<Unreadable>),
}

impl Sweep {
    /// Walk `crates/` and record every content-enum catch-all.
    fn run(root: &Path) -> Self {
        let mut hits = Vec::new();
        let mut unreadable = Vec::new();

        for entry in WalkDir::new(root.join("crates")) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    unreadable.push(Unreadable::Walk(err.to_string()));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let as_str = path.to_string_lossy();
            // Test code is exempt: a test matching one variant it cares about
            // is not a traversal that can silently drop content.
            if as_str.contains("/tests/") || as_str.contains("/generated/") {
                continue;
            }
            let file = RepoPath::of(root, path);
            match fs::read_to_string(path) {
                Ok(source) => hits.extend(scan(file, &source)),
                Err(err) => unreadable.push(Unreadable::File {
                    path: file,
                    error: err.to_string(),
                }),
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
        appeared: Vec<RepoPath>,
        /// Listed but carrying none: a stale exemption to delete.
        cleaned: Vec<RepoPath>,
    },
}

impl Agreement {
    /// Compare the two sets, in both directions.
    ///
    /// Both sides are `&str`, so this is two `BTreeSet::difference` calls: a
    /// sorted merge. The predecessor held one side as `BTreeSet<&RepoPath>`,
    /// which left the two directions spelled differently (`contains` one way,
    /// a linear `any` with a manual `as_str` the other) for no reason beyond
    /// the element types not matching.
    fn between(listed: &BTreeSet<&str>, carrying: &BTreeSet<&str>) -> Self {
        let own = |file: &&str| RepoPath((*file).to_owned());
        let appeared: Vec<RepoPath> = carrying.difference(listed).map(own).collect();
        let cleaned: Vec<RepoPath> = listed.difference(carrying).map(own).collect();
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
    Unmeasurable(Vec<Unreadable>),
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
    pub fn of(root: &Path) -> Self {
        let hits = match Sweep::run(root) {
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
    pub fn outcome(&self) -> GateOutcome {
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
                return Err(out);
            }
            Self::Measured { hits, agreement } => (hits, agreement),
        };

        let (appeared, cleaned) = match agreement {
            Agreement::Exact => {
                let files: BTreeSet<&str> = hits.iter().map(|hit| hit.file.as_str()).collect();
                return Ok(format!(
                    "content-enum catch-alls: {} across {} file(s); \
                     all listed in UNPROTECTED",
                    hits.len(),
                    files.len()
                ));
            }
            Agreement::Drifted { appeared, cleaned } => (appeared, cleaned),
        };

        Err(report([
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
fn scan(file: RepoPath, source: &str) -> Vec<CatchAll> {
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

impl Gate for CatchAllGate {
    fn name(&self) -> &'static str {
        "content-enum catch-alls (design rule 3)"
    }

    fn check(&self) -> GateOutcome {
        Audit::of(workspace_root()).outcome()
    }
}

#[cfg(test)]
mod tests {
    use super::{RepoPath, has_catch_all, scan};

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
        let hits = scan(RepoPath("probe.rs".to_owned()), source);
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
