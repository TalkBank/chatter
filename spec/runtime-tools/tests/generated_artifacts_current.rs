//! THE GATE: every artifact generated from `spec/` matches what the specs say
//! it should be.
//!
//! # What this replaced
//!
//! Nothing, for most of the artifacts (the registry has since grown; `all()` is the count). `spec/tools/tests/generated_test_output.rs`
//! pins the SET of files one generator writes against the set the test tree
//! `include!`s, and never compares their content to anything, so a spec change
//! that was never regenerated left a stale artifact and every gate stayed green.
//!
//! Its first run found two real cases that had been invisible:
//!
//! - `grammar/test/corpus/generated/errors/e311.txt`, where the specs had
//!   produced `e311_1.txt` and `e311_2.txt` since 2026-07-30. Reasoning from
//!   commit dates had said that tree was fine, which is why a gate exists and
//!   an inference does not.
//! - `generated_diagnostic_kind.rs`, whose 224 arms were in a different order
//!   from today's `ErrorCode::iter()`. Semantically harmless in a `match`, and
//!   proof that the committed copy predated an enum reordering nobody noticed.
//!
//! # Proving it fires
//!
//! Verified by hand on 2026-08-15, both ways: appending a line to a committed
//! generated file fails this test naming that file, and deleting one fails it
//! as `missing`. The check writes nothing, so a failing run leaves the tree
//! exactly as it found it.

use generators::repo_paths::{self, RepoRoot};
use spec_runtime_tools::artifacts::all;

/// The repository root, resolved by the workspace's one resolver.
fn repo_root() -> RepoRoot {
    RepoRoot::resolve(None).expect(repo_paths::NOT_A_CHECKOUT)
}

/// Every committed generated artifact is what the current specs produce.
///
/// Iterates both halves of the registry, which is the same list `spec_gen`
/// writes from, so the gate cannot check a different set of artifacts from the
/// set that is produced.
#[test]
fn every_generated_artifact_is_current() {
    let root = repo_root();
    let mut stale = Vec::new();

    for artifact in all() {
        let differences = artifact
            .check(root.as_path())
            .unwrap_or_else(|error| panic!("checking {}: {error:#}", artifact.what));
        if !differences.is_empty() {
            stale.push(format!(
                "{} ({}):\n{}",
                artifact.what,
                artifact.root,
                differences
                    .iter()
                    .map(|d| format!("    {d}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }

    assert!(
        stale.is_empty(),
        "{} generated artifact(s) are stale. Run `just spec-gen` and commit the \
         result.\n\n{}",
        stale.len(),
        stale.join("\n\n")
    );
}

/// Every artifact's committed root exists.
///
/// Separated from the currency check because the failures mean different
/// things: a missing root is a misconfigured registry row or a deleted tree,
/// which no amount of regenerating fixes, whereas staleness is fixed by one
/// command. Reporting them through one assertion would send a contributor to
/// the wrong remedy.
#[test]
fn every_artifact_root_exists() {
    let root = repo_root();
    for artifact in all() {
        let path = artifact.path(root.as_path());
        assert!(
            path.exists(),
            "{}: {} does not exist. The registry names a destination that is \
             not in the tree.",
            artifact.what,
            artifact.root
        );
    }
}
