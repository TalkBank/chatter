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
//! This binary survives as the thing that lists the modules still to clean,
//! which the lint cannot do (an undenied file is silent). It exits 1 above
//! BASELINE as a backstop for unprotected modules, but that count is a scalar
//! over a heterogeneous set: fixing a harmless LSP hover formatter frees a slot
//! for a new catch-all in a validator. Do not treat it as the gate.
//!
//! Usage:
//!   cargo run -p talkbank-parser-tests --bin audit_content_catch_alls

// Dev tool. The one panic is `workspace_root`'s, on a tree with no
// `[workspace]` manifest above it, which cannot happen in a checkout and has
// no sensible recovery. `expect_used` was allowed here by copy-paste from a
// sibling; this file contains no `expect()`.
#![allow(clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

/// Backstop for modules that do not yet carry the clippy deny.
///
/// Lower it when sites are fixed; never raise it. The real gate is the lint.
const BASELINE: usize = 24;

/// A `match` block whose own arms name a content enum and which also has a
/// catch-all.
struct CatchAll {
    file: PathBuf,
    line: usize,
}

fn main() -> ExitCode {
    let root = workspace_root();
    let mut found = Vec::new();

    // Walk errors and unreadable files are COUNTED, not skipped. Dropping them
    // silently lowers the count, and this tool's whole output is a count: a
    // ratchet that passes because it could not read the files is the exact
    // defect `scripts/guard-silent-failure.sh` exists to forbid, in the tool
    // built to make a rule checkable. The sibling `audit_error_codes` returns
    // walk errors as `TestError` for the same reason.
    let mut unreadable = Vec::new();

    for entry in WalkDir::new(root.join("crates")) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                unreadable.push(format!("walk error: {err}"));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let as_str = path.to_string_lossy();
        // Test code is exempt: a test matching one variant it cares about is
        // not a traversal that can silently drop content.
        if as_str.contains("/tests/") || as_str.contains("/generated/") {
            continue;
        }
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                unreadable.push(format!("{}: {err}", path.display()));
                continue;
            }
        };
        found.extend(scan(path, &source));
    }

    found.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    for hit in &found {
        println!(
            "{}:{}",
            hit.file.strip_prefix(&root).unwrap_or(&hit.file).display(),
            hit.line
        );
    }
    println!(
        "\ncontent-enum catch-alls: {} (baseline {BASELINE})",
        found.len()
    );

    if !unreadable.is_empty() {
        eprintln!(
            "\nFAIL: {} file(s) could not be read, so the count below is a FLOOR,\n\
             not a measurement. Refusing to report a number that a read failure\n\
             could have lowered:",
            unreadable.len()
        );
        for problem in &unreadable {
            eprintln!("  {problem}");
        }
        return ExitCode::FAILURE;
    }

    if found.len() > BASELINE {
        eprintln!(
            "\nFAIL: {} catch-alls, above the baseline of {BASELINE}.\n\
             A `_ =>` over UtteranceContent or BracketedItem means a future\n\
             content variant compiles clean and answers wrong. List the arms\n\
             instead; design rule 3.",
            found.len()
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Find every `match` block in `source` whose OWN top-level arms name a
/// content enum and which also carries a catch-all.
///
/// Brace-balanced and depth-aware on purpose. A first cut looked back a fixed
/// forty lines from each `_ =>` and reported 41 hits, sixteen of which were
/// matches over `Token`, `Separator` or `PauseDuration` that merely happened to
/// sit near a content-enum reference. Proximity is not scope.
fn scan(path: &Path, source: &str) -> Vec<CatchAll> {
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

        let names_content = own.contains("UtteranceContent::") || own.contains("BracketedItem::");
        if names_content && has_catch_all(&own) {
            hits.push(CatchAll {
                file: path.to_path_buf(),
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

/// Nearest ancestor directory holding a `[workspace]` Cargo.toml.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = dir.join("Cargo.toml");
        if fs::read_to_string(&manifest)
            .map(|text| text.contains("[workspace]"))
            .unwrap_or(false)
        {
            return dir;
        }
        if !dir.pop() {
            panic!("audit_content_catch_alls: no [workspace] Cargo.toml above CARGO_MANIFEST_DIR");
        }
    }
}
