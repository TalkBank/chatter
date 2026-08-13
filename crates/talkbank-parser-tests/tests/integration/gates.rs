//! Run every registered repository gate, and check the registry itself.
//!
//! ONE test over `gate::ALL` rather than one test file per gate: a gate is
//! enforced by being listed, not by somebody also remembering to write a test
//! module and declare it in `main.rs`, which is the step three of this
//! workspace's gates were lost at.
//!
//! The failure text is the gate's own operator-facing report, so CI output
//! reads exactly like running the corresponding audit binary by hand.

use std::collections::BTreeSet;

use talkbank_parser_tests::gate::{ALL, listing, report};
use talkbank_parser_tests::repo_paths::workspace_root;

/// SURVIVES: policy. WHICH invariants this repository enforces is a set of
/// choices with real alternatives, so no type can hold the list. What the type
/// does hold is that a registered gate cannot report findings without a
/// verdict: `Gate::check` returns `GateOutcome` and there is no accessor
/// yielding the findings alone.
#[test]
fn every_registered_gate_passes() -> Result<(), String> {
    let failures: Vec<String> = ALL
        .iter()
        .filter_map(|gate| match gate.check() {
            Ok(summary) => {
                println!("ok  {}: {summary}", gate.name());
                None
            }
            Err(failure) => Some(format!("{}:\n{failure}", gate.name())),
        })
        .collect();

    if failures.is_empty() {
        return Ok(());
    }
    Err(report(failures))
}

/// SURVIVES: policy. That the registry lists every implementor is a convention
/// about this crate's own source, which no type can carry.
///
/// The module doc for `gate` used to claim an unregistered gate "shows up as an
/// unused-import or dead-code warning". That was FALSE: `dead_code` does not
/// fire on public items of a library crate, and the `use crate::gate::...`
/// import is consumed by the `impl` block whether or not the type is ever
/// registered. An unregistered gate produced exactly zero diagnostics, and
/// asserting `!ALL.is_empty()` would let a one-of-four registry pass.
///
/// So the registry is checked the way this workspace checks its other
/// source-derived facts, in BOTH directions: an implementor missing from `ALL`
/// fails, and an entry naming a type that no longer implements `Gate` fails
/// (the latter would usually be a compile error, but not if the type still
/// exists under a different trait).
#[test]
fn the_registry_lists_every_gate() -> Result<(), String> {
    let src = workspace_root().join("crates/talkbank-parser-tests/src");

    let mut implementors: BTreeSet<String> = BTreeSet::new();
    let mut registered: BTreeSet<String> = BTreeSet::new();

    for entry in walkdir::WalkDir::new(&src) {
        let entry = entry.map_err(|err| format!("walking {}: {err}", src.display()))?;
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("impl Gate for ") {
                implementors.insert(rest.trim_end_matches(" {").trim().to_owned());
            }
            // Entries in `ALL` are written `&crate::<module>::<Type>,`.
            if let Some(rest) = trimmed.strip_prefix("&crate::") {
                if let Some(name) = rest.trim_end_matches(',').rsplit("::").next() {
                    registered.insert(name.to_owned());
                }
            }
        }
    }

    if implementors.is_empty() {
        return Err(format!(
            "found no `impl Gate for` in {}; this check cannot report clean \
             having matched nothing",
            src.display()
        ));
    }

    let unregistered: Vec<&String> = implementors.difference(&registered).collect();
    let phantom: Vec<&String> = registered.difference(&implementors).collect();

    let text = report([
        listing(
            "FAIL: gate(s) implement `Gate` but are missing from `gate::ALL`,\n\
             so they do not run. Add them:",
            &unregistered,
        ),
        listing(
            "FAIL: `gate::ALL` names type(s) with no `impl Gate for`. Remove them:",
            &phantom,
        ),
    ]);
    if text.is_empty() { Ok(()) } else { Err(text) }
}
