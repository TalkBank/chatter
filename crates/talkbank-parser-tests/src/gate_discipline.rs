//! A gate reads the tree it was handed, and no other.
//!
//! # The hole this closes
//!
//! [`crate::gate`] makes a clean verdict carry an [`crate::gate::Examined`]
//! witness that only a [`crate::gate::ReadTree`] can issue, so a gate cannot
//! say "0 problems" about a tree it never opened. That closes composition: the
//! witness and the summary cannot be paired by hand.
//!
//! It does not close the wider case, and no type can. `Tree::live` is public
//! and `pub(crate)` is no barrier to a gate living in this crate, so a `check`
//! that ignores its parameter, reads one file through a tree of its own and
//! calls `clean` composes a perfectly well formed verdict about the live
//! checkout. Every probe against such a gate goes green, because the planted
//! violation lived in the discarded parameter. A review found that reachable on
//! 2026-09-08, a day after the mechanism was written, and the module doc had
//! named a test for it that nobody had written.
//!
//! There is no signature that says "do not open a second tree". A check over
//! the gates' own sources says it instead, which is the same move
//! `tests/integration/gates.rs` already makes to keep the registry honest: an
//! affordance beats a rule, and where no affordance exists a scan of the
//! sources is the next best thing.
//!
//! # Scoped to `fn check` bodies, deliberately
//!
//! A file that declares a gate may also hold helpers that legitimately build
//! their own tree; `construct_coverage::cha_files_under` is one, and it is not
//! part of any gate's verdict. The rule is about what a GATE'S CHECK does, so
//! the scan reads exactly those bodies. That is also what keeps this gate from
//! flagging itself: the calls it forbids are named in a module-level `const`,
//! never inside a `check`.
//!
//! The text is blanked first ([`crate::test_hygiene::blank_literals`]), so a
//! forbidden call named inside a string or a LINE comment is not a call. Not a
//! BLOCK comment: `blank_literals` handles `//` only, so `/* Tree::live() */`
//! inside a `check` is a finding and a `/* } */` truncates the body this
//! extracts. That limit is declared below rather than left to be discovered,
//! and it is the same one `test_hygiene`'s vacuous gate declares.

use crate::gate::tree::RelPath;
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, UnprovenRule, listing};
use crate::test_hygiene::{blank_literals, block_end};

/// The declaration that puts a file in scope.
const DECLARES_A_GATE: &str = "impl Gate for";

/// Where a gate's `check` reads from, and why each is not allowed.
///
/// Every one of these reaches a checkout DIRECTLY, so nothing records the read
/// and a probe cannot plant into it. The tree parameter exists to make both
/// true; using one of these is how a gate stops being probeable while still
/// looking like it is.
const FORBIDDEN: &[(&str, &str)] = &[
    (
        "Tree::live(",
        "builds a second tree, so the probe's plant is in the parameter it discarded",
    ),
    (
        "Tree::rooted(",
        "builds a second tree, so the probe's plant is in the parameter it discarded",
    ),
    (
        "std::fs::",
        "reaches the filesystem directly, so no read is recorded and no plant is seen",
    ),
    (
        "File::open(",
        "reaches the filesystem directly, so no read is recorded and no plant is seen",
    ),
    (
        "include_str!(",
        "reads a copy compiled into the binary, which no probe can plant into",
    ),
];

/// Every gate's `check` reads through the tree it was given.
pub struct GateDisciplineGate;

impl Gate for GateDisciplineGate {
    fn name(&self) -> &'static str {
        "gates read only their own tree"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        let files = match tree.files_under("crates") {
            Ok(files) => files,
            // A walk failure is not a smaller file set: it is an unknown number
            // of gate files never offered, and a count taken after one is a
            // floor rather than a measurement.
            Err(err) => {
                return Outcome::failed(format!(
                    "FAIL: could not enumerate the crates tree, so no claim about \
                     what the gates read is available: {err}"
                ));
            }
        };

        let mut findings: Vec<String> = Vec::new();
        let mut declaring = 0usize;
        for path in files {
            if !path.extension_is("rs") {
                continue;
            }
            let text = match tree.read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    return Outcome::failed(format!(
                        "FAIL: a file could not be read, so this is a floor rather \
                         than a measurement: {err}"
                    ));
                }
            };
            // ONE function decides both "is this file in scope" and "what
            // does its `check` read", so the blanking cannot happen for one and
            // not the other. It did: the scope test ran on RAW text, matched
            // two files that only MENTION the declaration (`gate.rs` in its
            // module doc, `tests/integration/gates.rs` in a doc comment and a
            // string), and the clean summary said eight files declare a gate
            // where six do. A wrong number inside the clean summary of the gate
            // whose whole subject is clean summaries that lie.
            if let Some(found) = reads_around_its_tree(&path, &text) {
                declaring += found.declarations;
                findings.extend(found.reads_around);
            }
        }

        // A FLOOR, not merely a witness. If `DECLARES_A_GATE` ever stops
        // matching (a rename, a formatting change putting the type on the next
        // line), `declaring` falls to zero, `findings` is empty, the walk still
        // mints an `Examined`, and this would report clean having judged
        // nothing. Every registered gate lives in one of these files, so the
        // registry's own length is the floor the scan must clear. The sibling
        // gates declare this same case as an unprobed VACUITY rule; here it is
        // closed instead, because `ALL` supplies the number.
        // DECLARATIONS against GATES, which is like against like. The first
        // form of this compared FILES to `ALL.len()` and failed on the
        // unmodified tree, because seven gates live in six files: two integers
        // counting different things, in the floor written to stop a count that
        // means nothing.
        if declaring < crate::gate::ALL.len() {
            return Outcome::failed(format!(
                "FAIL: the scan found {declaring} declaration(s) of \
                 {DECLARES_A_GATE:?} and the registry holds {} gate(s), each of \
                 which needs one. The scan has stopped seeing declarations, so a \
                 clean verdict from it would be a report about nothing.",
                crate::gate::ALL.len()
            ));
        }
        if findings.is_empty() {
            return tree.clean(format!(
                "{declaring} gate declaration(s); every `check` reads through the \
                 tree it was handed"
            ));
        }
        Outcome::failed(listing(
            "FAIL: a gate's `check` reads around the tree it was handed. The plant a\n\
             probe applies lands in that parameter, so a gate reading elsewhere is\n\
             unprobeable while still reporting clean. Read through the `ReadTree`:",
            &findings,
        ))
    }

    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a gate whose `check` builds its own tree",
            "Tree::live(",
            |edit| {
                edit.write(PROBE_GATE, gate_source("        let mine = Tree::live();"));
                Ok(())
            },
        )
        .refusing(
            "a gate whose `check` reads the filesystem directly",
            "std::fs::",
            |edit| {
                edit.write(
                    PROBE_GATE,
                    gate_source("        let _ = std::fs::read_to_string(\"Cargo.toml\");"),
                );
                Ok(())
            },
        )
        .refusing(
            "a gate whose `check` reads a compiled-in copy of its input",
            "include_str!(",
            |edit| {
                edit.write(
                    PROBE_GATE,
                    gate_source("        let _ = include_str!(\"../golden_words.txt\");"),
                );
                Ok(())
            },
        )
        // The scoping rule, which no must-fail probe can reach: its whole job
        // is NOT to fire. Without it the gate could be tightened to the whole
        // file and every must-fail probe would still pass, while
        // `construct_coverage`'s own helper started failing the gate.
        .accepting(
            "a HELPER in a gate's file may build its own tree; only `check` may not",
            |edit| {
                let mut source = gate_source("        let _ = tree.root();");
                source.push_str("\nfn helper() -> Tree {\n    Tree::live()\n}\n");
                edit.write(PROBE_GATE, source);
                Ok(())
            },
        )
        .accepting(
            "a forbidden call NAMED in a string or a comment is not a call",
            |edit| {
                edit.write(
                    PROBE_GATE,
                    gate_source(
                        "        // never write Tree::live() here\n        \
                         let _ = \"Tree::live()\";",
                    ),
                );
                Ok(())
            },
        )
        .refusing(
            "the crates tree cannot be ENUMERATED",
            "could not enumerate the crates tree",
            |edit| {
                edit.fail_walk_under("crates");
                Ok(())
            },
        )
        .refusing(
            "the scan stops recognising declarations, so it judges nothing",
            "has stopped seeing declarations",
            |edit| {
                // Every gate file at once: with none of them in scope the count
                // falls below the registry's length and the floor refuses a
                // clean verdict. Hiding ONE would not do it, because the floor
                // is about the scan going blind rather than about any one file.
                edit.hide_files_under("crates/talkbank-parser-tests/src");
                Ok(())
            },
        )
        .accepting("a file that declares no gate is out of scope", |edit| {
            edit.write(
                "crates/talkbank-model/src/probe_not_a_gate.rs",
                "fn helper() -> Tree {\n    Tree::live()\n}\n".to_owned(),
            );
            Ok(())
        })
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}

/// The file every probe of this gate plants into, and the path two of them
/// require the failure to name.
const PROBE_GATE: &str = "crates/talkbank-model/src/probe_gate_discipline.rs";

/// A minimal file that declares a gate, with `body` inside its `check`.
///
/// Built here rather than written out per probe so the probes differ only in
/// the line under test. It never compiles and never needs to: every plant lands
/// in an in-memory overlay that no compiler sees.
fn gate_source(body: &str) -> String {
    format!(
        "impl Gate for Probe {{\n    \
         fn check(&self, tree: ReadTree) -> Outcome {{\n\
         {body}\n        \
         tree.clean(\"probe\")\n    }}\n}}\n"
    )
}

/// Every forbidden call in this file's `fn check` bodies, or `None` when the
/// file declares no gate.
///
/// Takes RAW source and blanks it ONCE, here. The scope test and the body scan
/// must see the same text, and when the caller owned the blanking they did
/// not: an `impl Gate for` written in a doc comment put a file in scope that
/// declares nothing. `None` rather than an empty `Vec`, because "no gate here"
/// and "a gate here that reads nothing it should not" are different facts and
/// only the second belongs in the count.
fn reads_around_its_tree(path: &RelPath, raw: &str) -> Option<GateFile> {
    let blanked = blank_literals(raw);
    let declarations = blanked.matches(DECLARES_A_GATE).count();
    if declarations == 0 {
        return None;
    }
    let mut reads_around = Vec::new();
    for body in check_bodies(&blanked) {
        for (call, why) in FORBIDDEN {
            if body.contains(call) {
                reads_around.push(format!("{path}: `{call}` {why}"));
            }
        }
    }
    Some(GateFile {
        declarations,
        reads_around,
    })
}

/// What one file in scope contributes.
///
/// Two fields rather than a `Vec` and a `+= 1` at the call site, because they
/// count different things and the floor needs the first: a file may declare
/// two gates, and seven of them live in six files.
#[derive(Debug, PartialEq, Eq)]
struct GateFile {
    /// `impl Gate for` declarations this file carries.
    declarations: usize,
    /// Forbidden calls inside its `check` bodies.
    reads_around: Vec<String>,
}

/// The body of every `fn check` in `blanked`, braces included.
///
/// Takes BLANKED source, so a brace inside a string cannot unbalance the match.
///
/// A DECLARATION has no body, and taking the next `{` after one swallows
/// whatever function follows it. `gate.rs` declares `fn check(&self, tree:
/// ReadTree) -> Outcome;` on the trait, and this took `listing`'s body as its
/// own: harmless only because `listing` happens to touch nothing forbidden.
/// The `;` before the `{` is what tells the two apart.
fn check_bodies(blanked: &str) -> Vec<&str> {
    let mut bodies = Vec::new();
    let mut search = 0usize;
    while let Some(offset) = blanked[search..].find("fn check(") {
        let at = search + offset;
        let rest = &blanked[at..];
        let open = rest.find('{');
        let semicolon = rest.find(';');
        match (open, semicolon) {
            // A declaration: the signature ends before any block begins.
            (Some(brace), Some(semi)) if semi < brace => {
                search = at + semi + 1;
                continue;
            }
            (None, _) => break,
            (Some(brace), _) => {
                let start = at + brace;
                let end = block_end(blanked, start);
                bodies.push(&blanked[start..end]);
                search = end.max(at + 1);
            }
        }
    }
    bodies
}

/// What no plant of this gate reaches.
///
/// VACUITY is deliberately absent, and that is the one entry a reader will look
/// for: the sibling gates all declare "nothing asserts the scan found any X" as
/// unprobed, and this gate closes it instead, because `gate::ALL` supplies the
/// number a floor needs. The probe that plants it hides every gate file at
/// once.
static UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "INDIRECTION: a `check` that calls a helper which opens a tree",
        "A textual scan reads one function body. `fn check` calling \
         `fn sweep()` that calls `Tree::live()` passes, and the finding would \
         be a real one. Closing it means following calls, which is a Rust \
         parser rather than a scan; the honest scope is stated here rather \
         than implied by silence.",
    ),
    UnprovenRule::new(
        "BLOCK COMMENTS, which `blank_literals` does not handle",
        "`blank_literals` blanks `//` line comments and string literals only, \
         so a forbidden call inside `/* ... */` reads as code and a `}` inside \
         one truncates the extracted body. A plant of that shape WOULD make \
         this gate fail, as a false positive of the scanner rather than of the \
         rule, so it is a bad probe and is excluded deliberately. The sibling \
         gate in `test_hygiene` declares the identical limit.",
    ),
    UnprovenRule::new(
        "ALIASED IMPORTS: `use std::fs;` then `fs::read_to_string`",
        "The scan names `std::fs::` as it is written at the call site. An \
         import that shortens it is invisible to this rule and to any \
         plant of it. Same cure as the entry above.",
    ),
];

#[cfg(test)]
mod tests {
    use super::{FORBIDDEN, check_bodies, reads_around_its_tree};
    use crate::gate::tree::RelPath;
    use crate::test_hygiene::blank_literals;

    /// SURVIVES: behaviour a signature cannot state. Whether a file declares a
    /// gate is a question about its text, and the two answers this got wrong
    /// were a doc comment and a string literal.
    #[test]
    fn a_mention_in_a_comment_or_a_string_does_not_declare_a_gate() {
        let path = RelPath::new("probe.rs");
        assert!(
            reads_around_its_tree(&path, "//! Every `impl Gate for` is registered.\n").is_none(),
            "a doc comment naming the declaration declares nothing"
        );
        assert!(
            reads_around_its_tree(&path, "let needle = \"impl Gate for\";\n").is_none(),
            "a string naming the declaration declares nothing"
        );
        assert_eq!(
            reads_around_its_tree(
                &path,
                "impl Gate for Real {\n    fn check(&self, tree: ReadTree) -> Outcome {\n        \
                 tree.clean(\"x\")\n    }\n}\n"
            ),
            Some(super::GateFile {
                declarations: 1,
                reads_around: Vec::new()
            }),
            "a real declaration is in scope and this one reads nothing forbidden"
        );
    }

    /// SURVIVES: behaviour. A trait DECLARATION has no body, and taking the
    /// next `{` after one swallows whatever function follows it. `gate.rs`
    /// declares `fn check(..) -> Outcome;` and this used to hand back
    /// `listing`'s body as a gate's own.
    #[test]
    fn a_bodyless_declaration_contributes_no_body() {
        let source = blank_literals(
            "trait Gate {\n    fn check(&self, tree: ReadTree) -> Outcome;\n}\n\
             pub fn listing() -> String {\n    let _ = std::fs::read_to_string(\"x\");\n    \
             String::new()\n}\n",
        );
        assert!(
            check_bodies(&source).is_empty(),
            "a declaration has no body, so nothing after it is a check body"
        );
    }

    /// SURVIVES: behaviour. The two must both be found, and the second must not
    /// swallow the first.
    #[test]
    fn two_implementations_in_one_file_yield_two_bodies() {
        let source = blank_literals(
            "impl Gate for A {\n    fn check(&self, tree: ReadTree) -> Outcome {\n        \
             tree.clean(\"a\")\n    }\n}\n\
             impl Gate for B {\n    fn check(&self, tree: ReadTree) -> Outcome {\n        \
             tree.clean(\"b\")\n    }\n}\n",
        );
        assert_eq!(check_bodies(&source).len(), 2);
    }

    /// SURVIVES: policy. WHICH calls reach a checkout directly is a judgement
    /// about this codebase's own API, not a fact a type carries. Pinned so the
    /// list cannot quietly lose an entry.
    #[test]
    fn the_forbidden_list_names_every_route_to_a_checkout() {
        let named: Vec<&str> = FORBIDDEN.iter().map(|(call, _)| *call).collect();
        assert_eq!(
            named,
            vec![
                "Tree::live(",
                "Tree::rooted(",
                "std::fs::",
                "File::open(",
                "include_str!(",
            ]
        );
    }
}
