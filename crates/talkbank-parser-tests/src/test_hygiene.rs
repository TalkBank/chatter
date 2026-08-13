//! Gates over this repository's own TESTS: none that cannot fail, none written
//! twice.
//!
//! # Why these are gates and not scripts
//!
//! Both checks began as Python under `scripts/`. That was the wrong altitude
//! twice over. This crate already scans the repo's own sources in Rust
//! ([`crate::content_catch_alls`]), and [`crate::gate`] exists precisely
//! because a check that computes findings and does not FAIL keeps reappearing
//! "in four distinct spellings". A script nobody invokes is the fifth and worst
//! spelling: it does not even print, because nothing runs it. As `Gate` impls
//! they run under `cargo test --workspace --tests`, which is CI, by
//! construction rather than by wiring.
//!
//! # The two checks
//!
//! **Vacuous.** A test with no assertion, no `?`, no `Err(`, no panic-family
//! call, and no call to a helper that has one, cannot report anything. The
//! helper resolution is load-bearing: without it 52 tests look vacuous, with it
//! 4 do, because a test delegating to `assert_roundtrip(..)` fails through it.
//!
//! **Duplicate.** Two tests whose signature and body are identical after
//! normalising whitespace assert the same thing under two names; neither can
//! fail without the other. The SIGNATURE is part of the comparison because for
//! a proptest the data source lives there, so two tests with identical bodies
//! drawing from `gra_content(2..3)` and `4..=6` are different tests.
//!
//! # Reading the literals correctly is the whole difficulty
//!
//! Both checks ask "does this text contain X", so a scanner that loses track of
//! where string literals end does not error: it returns a WRONG ANSWER THAT
//! LOOKS CLEAN. The Python versions got this wrong twice, and each time the
//! symptom was a test that visibly contains two `assert_eq!` calls being
//! reported as unable to fail:
//!
//! - a raw string (`r#"..."#`) whose contents contain a `"` inverted the
//!   scanner's parity for the REST OF THE FILE;
//! - a char literal holding a quote (`'"'`) opened a string that closed
//!   somewhere arbitrary, blanking real code in between.
//!
//! [`blank_literals`] handles both, plus ordinary strings and line comments,
//! and preserves length so byte offsets stay valid.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::gate::{Gate, GateOutcome, listing, report};
use crate::repo_paths::workspace_root;

/// A corpus of blanked sources, before helper names are known.
///
/// # Why this is two types and not one function with two loops
///
/// Judging a test requires the COMPLETE set of failing helper names: a test
/// calling `assert_roundtrip(..)` fails through it, and a scan that has not yet
/// seen that helper's definition will call the test vacuous. The two passes are
/// therefore ordered, and the order is a fact about correctness, not style.
///
/// Written as sequential loops in one function, nothing prevents a later edit
/// from judging first, and the symptom would be a gate reporting deletable
/// tests that are fine. [`Scanned::resolve`] CONSUMES the scan and returns the
/// only type that can answer, so the wrong order does not compile.
struct Scanned {
    files: Vec<SourceFile>,
    unreadable: Vec<String>,
}

/// The same corpus once every failing helper in it is known.
struct Resolved {
    files: Vec<SourceFile>,
    unreadable: Vec<String>,
    helpers: Vec<String>,
}

/// One source file: its repo-relative path, as written, and blanked.
struct SourceFile {
    path: String,
    original: String,
    blanked: String,
}

impl Scanned {
    /// Collect every function name whose own body can fail.
    ///
    /// Consumes `self`: there is no way back to an unresolved scan, and no way
    /// to hold both and use the wrong one.
    fn resolve(self) -> Resolved {
        let mut helpers = Vec::new();
        for file in &self.files {
            asserting_helpers(&file.blanked, &mut helpers);
        }
        Resolved {
            files: self.files,
            unreadable: self.unreadable,
            helpers,
        }
    }
}

impl Resolved {
    /// Tests that neither assert themselves nor call anything that does.
    fn vacuous(&self) -> Vec<TestFn> {
        let mut found = Vec::new();
        for file in &self.files {
            for test in tests_in(&file.path, &file.original, &file.blanked) {
                if can_fail(&test.shape_blanked) {
                    continue;
                }
                if self
                    .helpers
                    .iter()
                    .any(|helper| test.shape_blanked.0.contains(&format!("{helper}(")))
                {
                    continue;
                }
                found.push(test);
            }
        }
        found
    }
}

/// Tests that cannot fail, reviewed and deliberately kept.
///
/// A path set rather than a count, for the reason
/// [`crate::content_catch_alls::UNPROTECTED`] gives at length: a scalar lets a
/// fix in one place free a slot somewhere else and the total never moves.
/// Checked in BOTH directions, so an unlisted vacuous test fails AND a listed
/// entry that no longer exists fails, and the list can only shrink.
///
/// Each entry states which of the categories in the repo's testing doctrine it
/// survives under; the same sentence is in the test's own docstring.
pub const ACCEPTED_VACUOUS: &[&str] = &[
    // Behaviour a signature cannot describe: guards integer overflow on large
    // inputs, which no return type states and no lint catches.
    "crates/talkbank-transform/src/num_words/ordinal_year_eng.rs::ordinal_large_values_dont_crash",
    // Wire format, documentation sense: the assertion is that the book's
    // published example still COMPILES against the current public API.
    "crates/talkbank-transform/tests/integration/book_library_usage_examples.rs::book_custom_error_handling",
    // Manual investigation, the category `gate.rs` already grants to the
    // `#[ignore]`d divergence tools: a live Apple Events round-trip that blocks
    // on macOS automation consent and cannot assert, because whether CLAN is
    // installed is a property of the machine.
    "crates/send2clan/src/tests.rs::test_is_clan_available",
];

/// A test's signature and body AS WRITTEN, whitespace-normalised.
///
/// Comparing two of these answers "is this the same test twice?". It must be
/// the written text: blanking erases string literals, and for a table of tests
/// the literal IS the difference, so blanked comparison reported ten groups of
/// unrelated tests as duplicates.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct WrittenShape(String);

/// The same span with literals and comments blanked.
///
/// Searching this answers "can this test fail?". It must be blanked, or the
/// word `assert` inside a string clears a test that asserts nothing.
struct BlankedShape(String);

/// The two are separate types because the ONE bug this module kept making was
/// using the wrong one: they are both a test's text, both `String`, and each
/// gate wants the other. Now the compiler refuses the swap that produced a
/// silently-passing gate and a ten-group false report.
///
/// One `#[test]` function, located in a file.
struct TestFn {
    path: String,
    line: usize,
    name: String,
    /// Compared by [`DuplicateTestGate`].
    shape: WrittenShape,
    shape_blanked: BlankedShape,
}

/// Replace the CONTENT of string literals, char literals and line comments
/// with spaces, preserving length so offsets stay valid.
///
/// Delimiters are kept, so the result still parses as Rust to a brace matcher.
#[must_use]
pub fn blank_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out: Vec<u8> = bytes.to_vec();
    let mut i = 0usize;

    while i < bytes.len() {
        // Raw string: r, then zero or more #, then a quote. Terminated by a
        // quote followed by the SAME number of hashes.
        if bytes[i] == b'r' {
            let mut hashes = i + 1;
            while hashes < bytes.len() && bytes[hashes] == b'#' {
                hashes += 1;
            }
            if hashes < bytes.len() && bytes[hashes] == b'"' {
                let hash_count = hashes - i - 1;
                let mut j = hashes + 1;
                loop {
                    if j >= bytes.len() {
                        break;
                    }
                    if bytes[j] == b'"'
                        && bytes[j + 1..]
                            .iter()
                            .take(hash_count)
                            .filter(|b| **b == b'#')
                            .count()
                            == hash_count
                    {
                        break;
                    }
                    out[j] = b' ';
                    j += 1;
                }
                i = (j + 1 + hash_count).min(bytes.len());
                continue;
            }
        }
        match bytes[i] {
            b'"' => {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'"' {
                    if bytes[j] == b'\\' {
                        j += 1;
                    }
                    out.get_mut(j).map(|slot| *slot = b' ');
                    j += 1;
                }
                i = j + 1;
            }
            b'\'' => {
                // A char literal is at most `'\x'`; anything longer is a
                // lifetime (`'a`), which must be left alone.
                let close = if bytes.get(i + 1) == Some(&b'\\') {
                    i + 3
                } else {
                    i + 2
                };
                if bytes.get(close) == Some(&b'\'') {
                    for slot in out.iter_mut().take(close).skip(i + 1) {
                        *slot = b' ';
                    }
                    i = close + 1;
                } else {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let mut j = i;
                while j < bytes.len() && bytes[j] != b'\n' {
                    out[j] = b' ';
                    j += 1;
                }
                i = j;
            }
            _ => i += 1,
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| source.to_owned())
}

/// Collapse runs of whitespace so formatting is not a difference.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Index just past the `}` closing the block opening at `start`.
fn block_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(start) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return offset + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// Whether `body` contains anything that could make a test fail.
fn can_fail(body: &BlankedShape) -> bool {
    // `assert` is a SUBSTRING match, not a word match: `assert_roundtrip` is an
    // assertion helper, and a word-boundary match misses every one of them.
    let body = &body.0;
    body.contains("assert")
        || body.contains("panic")
        || body.contains("unreachable!")
        || body.contains("todo!")
        || body.contains("unimplemented!")
        || body.contains(".expect(")
        || body.contains(".unwrap(")
        || body.contains("Err(")
        || body.contains(")?")
        || body.contains("?;")
}

/// Every function name in `blanked` whose own body can fail.
fn asserting_helpers(blanked: &str, into: &mut Vec<String>) {
    let mut search = 0usize;
    while let Some(found) = blanked[search..].find("fn ") {
        let at = search + found + 3;
        let name: String = blanked[at..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        search = at + name.len().max(1);
        if name.is_empty() {
            continue;
        }
        let Some(open) = blanked[search..].find('{') else {
            continue;
        };
        let start = search + open;
        if can_fail(&BlankedShape(
            blanked[start..block_end(blanked, start)].to_owned(),
        )) {
            into.push(name);
        }
    }
}

/// Every `#[test]` in `blanked`, with its signature and body.
fn tests_in(path: &str, original: &str, blanked: &str) -> Vec<TestFn> {
    let mut found = Vec::new();
    let mut search = 0usize;
    while let Some(offset) = blanked[search..].find("#[test]") {
        let attr = search + offset;
        search = attr + "#[test]".len();
        let Some(fn_at) = blanked[search..].find("fn ") else {
            continue;
        };
        let sig_start = search + fn_at;
        let name: String = blanked[sig_start + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        // Shape starts at the PARAMETER LIST, not at `fn`: including the
        // name would make every test unique, so the gate could never fail.
        // Caught only by injecting a duplicate and watching it pass.
        let Some(paren) = blanked[sig_start..].find('(') else {
            continue;
        };
        let shape_start = sig_start + paren;
        let Some(open) = blanked[shape_start..].find('{') else {
            continue;
        };
        let body_start = shape_start + open;
        let end = block_end(blanked, body_start);
        found.push(TestFn {
            path: path.to_owned(),
            line: blanked[..attr].matches('\n').count() + 1,
            name,
            shape: WrittenShape(normalise(
                original.get(shape_start..end).unwrap_or_default(),
            )),
            shape_blanked: BlankedShape(normalise(&blanked[shape_start..end])),
        });
        search = end;
    }
    found
}

/// Read every tracked-looking `.rs` file under `crates/`, blanked.
fn sources(root: &Path) -> Scanned {
    let mut files = Vec::new();
    let mut unreadable = Vec::new();
    for entry in WalkDir::new(root.join("crates")) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                unreadable.push(err.to_string());
                continue;
            }
        };
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        // Forward-slashed on every host: the exclusions and the baseline keys
        // are written that way, and Windows would otherwise match neither.
        let as_str = path.to_string_lossy().replace('\\', "/");
        if as_str.contains("/generated/") || as_str.contains("/target/") {
            continue;
        }
        match fs::read_to_string(path) {
            Ok(text) => {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let blanked = blank_literals(&text);
                files.push(SourceFile {
                    path: rel,
                    original: text,
                    blanked,
                });
            }
            Err(err) => unreadable.push(format!("{}: {err}", path.display())),
        }
    }
    Scanned { files, unreadable }
}

/// No test in the tree is unable to fail, except those [`ACCEPTED_VACUOUS`]
/// names.
pub struct VacuousTestGate;

impl Gate for VacuousTestGate {
    fn name(&self) -> &'static str {
        "vacuous-tests"
    }

    fn check(&self) -> GateOutcome {
        let resolved = sources(&workspace_root()).resolve();
        let found = resolved.vacuous();
        let unreadable = &resolved.unreadable;

        let keys: Vec<String> = found
            .iter()
            .map(|t| format!("{}::{}", t.path, t.name))
            .collect();
        let unlisted: Vec<&String> = keys
            .iter()
            .filter(|k| !ACCEPTED_VACUOUS.contains(&k.as_str()))
            .collect();
        let stale: Vec<&&str> = ACCEPTED_VACUOUS
            .iter()
            .filter(|accepted| !keys.iter().any(|k| k == *accepted))
            .collect();

        let sections = report([
            listing(
                "tests that cannot fail and are not accepted (delete them, or \
                 add them to ACCEPTED_VACUOUS with the category they survive under):",
                &unlisted,
            ),
            listing(
                "ACCEPTED_VACUOUS entries that no longer exist (delete the entry \
                 in the commit that cleaned them):",
                &stale,
            ),
            listing("unreadable:", unreadable),
        ]);

        if sections.is_empty() {
            Ok(format!(
                "{} test(s) cannot fail, all accepted",
                ACCEPTED_VACUOUS.len()
            ))
        } else {
            Err(sections)
        }
    }
}

/// No two tests in the tree have the same signature and body.
pub struct DuplicateTestGate;

impl Gate for DuplicateTestGate {
    fn name(&self) -> &'static str {
        "duplicate-tests"
    }

    fn check(&self) -> GateOutcome {
        let scanned = sources(&workspace_root());
        let (files, unreadable) = (&scanned.files, &scanned.unreadable);

        let mut by_shape: BTreeMap<WrittenShape, Vec<String>> = BTreeMap::new();
        let mut total = 0usize;
        for file in files {
            for test in tests_in(&file.path, &file.original, &file.blanked) {
                total += 1;
                by_shape
                    .entry(test.shape.clone())
                    .or_default()
                    .push(format!("{}:{} {}", test.path, test.line, test.name));
            }
        }

        let duplicates: Vec<String> = by_shape
            .values()
            .filter(|members| members.len() > 1)
            .map(|members| members.join("  ==  "))
            .collect();

        let sections = report([
            listing(
                "tests with an identical signature and body; neither can fail \
                 without the other, so one is redundant:",
                &duplicates,
            ),
            listing("unreadable:", unreadable),
        ]);

        if sections.is_empty() {
            Ok(format!("{total} tests, no duplicates"))
        } else {
            Err(sections)
        }
    }
}
