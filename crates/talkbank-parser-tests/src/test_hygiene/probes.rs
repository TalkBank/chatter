//! Trees the two hygiene gates must reject, and trees they must still accept.
//!
//! # Why the fixtures are built rather than written
//!
//! Both gates scan every `.rs` file under `crates/`, INCLUDING this one. A
//! fixture written as a plain literal is still text in a real file, and
//! `blank_literals` is what stops it being read as a test declared here. That
//! is load-bearing rather than incidental: if the blanking ever regressed,
//! this file would start reporting its own fixtures as tests, which is exactly
//! the failure the blanking exists to prevent and a reason to keep every
//! fixture short and obviously literal.
//!
//! Nothing here is ever compiled. Plants land in an in-memory overlay and no
//! `mod` declaration names the files they create, so a fixture is free to be
//! Rust nobody could build, and the gates, which are byte scans, judge it the
//! same way either way.

use crate::gate::{ProbeSuite, UnprovenRule};

/// Where a planted fixture goes: under `crates/`, a `.rs` file, and outside
/// every exclusion, so the sweep is obliged to offer it.
const FIXTURE: &str = "crates/talkbank-model/src/probe_hygiene.rs";
/// A second file, for the probe that proves grouping is repo-wide.
const FIXTURE_B: &str = "crates/talkbank-transform/src/probe_hygiene_b.rs";

/// A file of `#[test]` functions, each a name and a body.
fn fixture(tests: &[(&str, &str)]) -> String {
    let mut out = String::from("//! A probe fixture. Nothing declares it as a module.\n");
    for (name, body) in tests {
        out.push_str(&format!("\n#[test]\nfn {name}() {{\n{body}}}\n"));
    }
    out
}

/// Trees [`super::VacuousTestGate`] must judge.
pub(super) fn vacuous() -> ProbeSuite {
    ProbeSuite::must_fail(
        "a test with no assertion, no `?`, and no call at all",
        "probe_hygiene.rs::probe_vacuous_unlisted",
        |edit| {
            // No call syntax anywhere in the body, deliberately. Any
            // `name(` where some `fn name` under `crates/` can fail counts
            // as delegation and CLEARS the test, and there are thousands of
            // such names, `new` among them: `Vec::new()` alone would
            // silence this probe.
            edit.write(
                FIXTURE,
                fixture(&[("probe_vacuous_unlisted", "    let x = 1;\n    let _ = x;\n")]),
            );
            Ok(())
        },
    )
    .refusing(
        "a vacuous test whose NAME contains `assert`",
        "probe_hygiene.rs::probe_assert_named_but_empty",
        |edit| {
            // The only probe that reaches the rule that a test's shape
            // starts at the PARAMETER LIST. If it started at `fn`, this
            // name's own `assert` would clear the test and the gate could
            // never fail; the module's own comment records that mistake
            // being caught only by planting one and watching it pass.
            edit.write(FIXTURE, fixture(&[("probe_assert_named_but_empty", "")]));
            Ok(())
        },
    )
    .refusing(
        "a vacuous test whose only `assert` is inside a string literal",
        "probe_hygiene.rs::probe_only_a_string_literal",
        |edit| {
            edit.write(
                FIXTURE,
                fixture(&[(
                    "probe_only_a_string_literal",
                    "    let s = \"assert_eq!(1, 2)\";\n    let _ = s;\n",
                )]),
            );
            Ok(())
        },
    )
    .refusing(
        "a vacuous test whose only `assert` is inside a raw string holding a quote",
        "probe_hygiene.rs::probe_only_a_raw_string",
        |edit| {
            // The documented historical bug: a raw string whose contents
            // contain a quote inverted the scanner's parity for the rest of
            // the file. Without the raw-string branch the `assert_eq!` here
            // stays visible, the test is cleared, and the gate reports
            // clean.
            edit.write(
                FIXTURE,
                fixture(&[(
                    "probe_only_a_raw_string",
                    "    let s = r#\"he said \"assert_eq!(1, 2)\" here\"#;\n    let _ = s;\n",
                )]),
            );
            Ok(())
        },
    )
    .refusing(
        "an accepted test that is no longer vacuous, so its entry is stale",
        "no longer exist",
        |edit| {
            edit.replace_once(
                "crates/send2clan/src/tests.rs",
                "    // We don't assert anything since CLAN may or may not be installed",
                "    assert!(available || !available);",
            )
        },
    )
    .refusing(
        "the swept directory cannot be ENUMERATED",
        "unreadable:",
        |edit| {
            edit.fail_walk_under("crates");
            Ok(())
        },
    )
    .refusing("a swept file that cannot be read", "unreadable:", |edit| {
        edit.write_bytes(FIXTURE, vec![0xFF, 0xFE]);
        Ok(())
    })
    .accepting(
        "a test whose only failing token is `.unwrap(` can fail",
        |edit| {
            // One of the ten tokens `can_fail` accepts. Each is a rule
            // whose job is NOT to fire, so a must-fail probe cannot reach
            // any of them; this reaches one.
            edit.write(
                FIXTURE,
                fixture(&[(
                    "probe_unwrap_can_fail",
                    "    let value: Option<u8> = None;\n    let _ = value.unwrap();\n",
                )]),
            );
            Ok(())
        },
    )
    .accepting(
        "a vacuous test under a `generated/` directory is out of scope",
        |edit| {
            edit.write(
                "crates/talkbank-model/src/generated/probe_hygiene.rs",
                fixture(&[(
                    "probe_generated_vacuous",
                    "    let x = 1;\n    let _ = x;\n",
                )]),
            );
            Ok(())
        },
    )
}

/// Trees [`super::DuplicateTestGate`] must judge.
pub(super) fn duplicate() -> ProbeSuite {
    ProbeSuite::must_fail(
        "two tests in one file whose only difference is the name",
        "identical signature and body",
        |edit| {
            edit.write(
                FIXTURE,
                fixture(&[
                    ("probe_duplicate_a", "    assert_eq!(29 + 13, 42);\n"),
                    ("probe_duplicate_b", "    assert_eq!(29 + 13, 42);\n"),
                ]),
            );
            Ok(())
        },
    )
    .refusing(
        "the same test in two different crates, so grouping is repo-wide",
        "probe_hygiene_b.rs",
        |edit| {
            edit.write(
                FIXTURE,
                fixture(&[("probe_cross_crate_a", "    assert_eq!(31 * 3, 93);\n")]),
            );
            edit.write(
                FIXTURE_B,
                fixture(&[("probe_cross_crate_b", "    assert_eq!(31 * 3, 93);\n")]),
            );
            Ok(())
        },
    )
    .refusing(
        "two tests differing only in indentation",
        "identical signature and body",
        |edit| {
            // `normalise` collapses whitespace runs before comparison.
            // Delete that call and the other duplicate probes still fail
            // while this one stops, so it isolates the step.
            edit.write(
                FIXTURE,
                fixture(&[
                    ("probe_ws_indented", "    assert_eq!(57 - 15, 42);\n"),
                    ("probe_ws_flush", "assert_eq!(57 - 15, 42);\n"),
                ]),
            );
            Ok(())
        },
    )
    .refusing(
        "the swept directory cannot be ENUMERATED",
        "unreadable:",
        |edit| {
            edit.fail_walk_under("crates");
            Ok(())
        },
    )
    .refusing("a swept file that cannot be read", "unreadable:", |edit| {
        edit.write_bytes(FIXTURE, vec![0xFF, 0xFE]);
        Ok(())
    })
    .accepting(
        "identical bodies drawing from different signatures are different tests",
        |edit| {
            // The proptest case the module doc cites: the data source lives
            // in the signature, so two cases with identical bodies over
            // different ranges are two tests. This is why the SIGNATURE is
            // part of the compared shape.
            edit.write(
                FIXTURE,
                "//! A probe fixture. Nothing declares it as a module.\n\
                     \n#[test]\nfn probe_range_a(n in 0..3u8) {\n    assert!(n < 9);\n}\n\
                     \n#[test]\nfn probe_range_b(n in 4..=6u8) {\n    assert!(n < 9);\n}\n",
            );
            Ok(())
        },
    )
    .accepting(
        "two tests differing only inside a string literal are different tests",
        |edit| {
            // The shape is the ORIGINAL text, never the blanked text.
            // Compared blanked, a table of tests differing only in their
            // literals reported ten groups of false duplicates.
            edit.write(
                FIXTURE,
                fixture(&[
                    ("probe_literal_a", "    assert_eq!(render(), \"alpha\");\n"),
                    ("probe_literal_b", "    assert_eq!(render(), \"beta\");\n"),
                ]),
            );
            Ok(())
        },
    )
}

/// Rules of [`super::VacuousTestGate`] no plant can reach.
pub(super) const VACUOUS_UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "SCOPE: only `crates/` is walked",
        "A vacuous test under `apps/`, `spec/`, `xtask/` or the root `tests/` \
         produces no finding at all. The gate's own doc says `no test in the \
         tree`; the truth is `no test under crates/`.",
    ),
    UnprovenRule::new(
        "ALTERNATE TEST ATTRIBUTES",
        "`tests_in` matches the literal `#[test]` only, so `#[tokio::test]`, \
         `#[rstest]` and `#[test_case]` functions are outside the gate \
         entirely. No plant using one can make it fail.",
    ),
    UnprovenRule::new(
        "HELPER SHADOWING, the largest hole",
        "A genuinely vacuous test is silently cleared if its body contains any \
         `name(` where some `fn name` anywhere under `crates/` can fail, and \
         thousands of such names are collected. No must-fail probe can express \
         it, because the input produces a PASS; only a mutation of the gate \
         would reach it.",
    ),
    UnprovenRule::new(
        "DUPLICATE `path::name` KEYS",
        "The baseline key is file plus function name and is not unique in \
         principle: a second vacuous test of the same name in another `mod` of \
         the same file is absorbed by the existing accepted entry and the gate \
         stays green. A real defect in the rule; the key should carry the \
         module path or the line.",
    ),
    UnprovenRule::new(
        "nine of the ten `can_fail` tokens",
        "Each token is what makes the gate NOT fire, so each is reachable only \
         as a must-pass probe. One (`.unwrap(`) is probed; the rest are not, \
         and removing one from the list would make MORE tests look vacuous, \
         which a probe asserting message CONTENT cannot notice.",
    ),
    UnprovenRule::new(
        "BLOCK COMMENTS, which `blank_literals` does not handle",
        "A commented-out test inside a `/* */` block is scanned as live code. A \
         plant of that shape WOULD make the gate fail, but as a false positive \
         of the scanner rather than the rule under test, so it is a bad probe \
         and is excluded deliberately rather than quietly.",
    ),
    UnprovenRule::new(
        "the silent UTF-8 fallback inside `blank_literals`",
        "If blanking ever produced invalid UTF-8 the file would be scanned \
         UNBLANKED and the gate would still report clean. No input reaching it \
         could be constructed with confidence, so no probe is proposed; it is a \
         fabricated-value shape in a module about answers that look clean.",
    ),
    UnprovenRule::new(
        "the scan-then-resolve ORDERING",
        "`Scanned::resolve` consumes the scan so judging before every helper is \
         known does not compile. Enforced by the type graph rather than by the \
         gate's output, which is the right answer: it is deliberately unprobed \
         rather than backed by a runtime check.",
    ),
];

/// Rules of [`super::DuplicateTestGate`] no plant can reach.
pub(super) const DUPLICATE_UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "VACUITY: nothing asserts the scan found any TEST",
        "`total` is computed, printed in the clean summary, and compared to \
         nothing, so a scan that reaches no `#[test]` still reports \
         `0 tests, no duplicates` and passes. The `Examined` witness \
         a clean verdict now requires closes only the outer case, a gate that \
         read nothing at all: enumerating `crates/` is an examination even when \
         it offers no file. Not a probe gap either way; the rule does not \
         exist, and the fix is a floor, the way `the_registry_lists_every_gate` \
         already refuses to report clean having matched nothing.",
    ),
    UnprovenRule::new(
        "the `/generated/` exclusion",
        "Generated tests are exactly where mass duplication would arise, and \
         the generated construct-test file alone carries over a hundred \
         `#[test]` attributes the gate never reads. Out of the rule, not merely \
         unprobed.",
    ),
    UnprovenRule::new(
        "SCOPE: only `crates/` is walked",
        "The same hole as its sibling gate's, through the same `sources` walk.",
    ),
    UnprovenRule::new(
        "MEMBER ORDER within a finding",
        "Members are pushed in directory-walk order and never sorted, so the \
         order of a cross-file group is not a property any probe may pin.",
    ),
    UnprovenRule::new(
        "the FALSE-POSITIVE vector, which must not be made a probe",
        "Attributes between `#[test]` and `fn` are outside the compared shape, \
         so a `#[should_panic]` twin of an ordinary test is reported as a \
         duplicate although the two assert opposite things. It would work as a \
         probe, and it is a gate bug rather than a violation, which is why it \
         is recorded here instead.",
    ),
];
