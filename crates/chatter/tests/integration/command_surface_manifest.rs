// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Release-facing manifest for the published `chatter` command surface.
//!
//! This keeps the current top-level subcommand vocabulary explicit so deeper
//! refactors can tighten internals without accidentally drifting the CLI.

use std::collections::BTreeSet;

use crate::common::{
    CliHarness,
    command_surface::{SURFACE_GROUPS, SurfaceScope, UNPUBLISHED_TOP_LEVEL},
};

fn listed_commands(help: &str) -> BTreeSet<String> {
    let mut commands = BTreeSet::new();
    let mut in_commands = false;

    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed == "Commands:"
            || trimmed.ends_with("Commands:")
            || trimmed.ends_with("Converters:")
            || trimmed.ends_with("Aliases:")
            || trimmed.starts_with("Not Available")
        {
            in_commands = true;
            continue;
        }

        if !in_commands {
            continue;
        }

        if trimmed == "Options:" {
            break;
        }

        // A command entry begins at EXACTLY two spaces of indent. Wrapped
        // continuation lines of a long description are indented far further, and
        // taking the first word of any indented line swept them in: a
        // description mentioning ASR yielded a phantom command `ASR`. That went
        // unnoticed because the only assertion was manifest ⊆ live, where extra
        // junk in `live` is harmless. It becomes fatal the moment the other
        // direction is checked, which is the direction that catches real drift.
        if line.starts_with("  ")
            && !line.starts_with("   ")
            && !trimmed.is_empty()
            && let Some(command) = trimmed.split_whitespace().next()
        {
            commands.insert(command.to_string());
        }
    }

    commands
}

fn help_output(args: &[&str]) -> String {
    let harness =
        CliHarness::new().expect("command-surface help should get an isolated CLI harness");
    let output = harness
        .chatter_cmd()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8_lossy(&output).into_owned()
}

fn manifest_commands(scope: SurfaceScope) -> BTreeSet<&'static str> {
    SURFACE_GROUPS
        .iter()
        .filter(|group| group.scope == scope)
        .flat_map(|group| group.commands.iter().copied())
        .collect()
}

#[test]
fn command_surface_manifest_has_unique_command_names_per_scope() {
    for scope in [SurfaceScope::TopLevel] {
        let mut seen = BTreeSet::new();
        for command in manifest_commands(scope) {
            assert!(
                seen.insert(command),
                "duplicate command `{command}` in {:?} surface manifest",
                scope
            );
        }
    }
}

#[test]
fn top_level_help_lists_all_manifested_commands() {
    let commands = listed_commands(&help_output(&["--help"]));
    for command in manifest_commands(SurfaceScope::TopLevel) {
        assert!(
            commands.contains(command),
            "top-level help is missing manifested command `{command}`"
        );
    }
    assert!(
        !commands.contains("analyze"),
        "stale removed command `analyze` reappeared in top-level help"
    );
}

/// Every top-level command is accounted for: published, or excluded with a
/// reason. This is the direction the manifest could not check.
///
/// `top_level_help_lists_all_manifested_commands` asserts manifest ⊆ help,
/// which catches a command being REMOVED. Nothing asserted help ⊆ manifest, so
/// a command could be ADDED to the CLI and belong to no group, carry no
/// coverage expectation, and appear in no documentation, with every gate green.
#[test]
fn every_top_level_command_is_published_or_declared_unpublished() {
    let live = listed_commands(&help_output(&["--help"]));
    let published = manifest_commands(SurfaceScope::TopLevel);
    let unpublished: BTreeSet<&str> = UNPUBLISHED_TOP_LEVEL
        .iter()
        .map(|(name, _)| *name)
        .collect();

    for command in &live {
        // `help` is clap's own built-in, not part of chatter's surface.
        if command == "help" {
            continue;
        }
        assert!(
            published.contains(command.as_str()) || unpublished.contains(command.as_str()),
            "top-level command `{command}` is in `chatter --help` but is neither in the \
             release-facing manifest nor declared unpublished. Add it to SURFACE_GROUPS with \
             coverage expectations, or to UNPUBLISHED_TOP_LEVEL with the reason it is excluded."
        );
    }

    // The exclusion list must not rot either: a name here that no longer
    // exists is a stale exclusion silently covering nothing.
    for (command, reason) in UNPUBLISHED_TOP_LEVEL {
        assert!(
            live.contains(*command),
            "`{command}` is declared unpublished ({reason}) but no longer appears in \
             `chatter --help`; remove the stale exclusion"
        );
        assert!(
            !published.contains(command),
            "`{command}` is both manifested as published and declared unpublished"
        );
    }
}

/// Every PUBLISHED command appears in the CLI reference page.
///
/// A command a user cannot find is, for that user, a command that does not
/// exist. `update` shipped in the binary and in the manifest while the CLI
/// reference never named it once, which no gate could see, because the only
/// documentation check in this repo is a blocklist of strings known to be dead:
/// it detects a doc mentioning something REMOVED and is structurally blind to a
/// doc omitting something ADDED.
///
/// Deliberately scoped to published commands. The experimental ones are listed
/// on the page as a group, and holding them to per-command documentation would
/// be asserting a policy nobody has adopted.
#[test]
fn published_commands_appear_in_the_cli_reference() {
    let page_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../book/src/chatter/user-guide/cli-reference.md");
    let page = std::fs::read_to_string(&page_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", page_path.display()));

    let missing: Vec<&str> = manifest_commands(SurfaceScope::TopLevel)
        .into_iter()
        .filter(|command| !page.contains(&format!("chatter {command}")))
        .collect();

    assert!(
        missing.is_empty(),
        "published commands missing from book/src/chatter/user-guide/cli-reference.md: {missing:?}"
    );
}

#[test]
fn every_surface_group_declares_coverage_and_rationale() {
    for group in SURFACE_GROUPS {
        assert!(
            !group.commands.is_empty(),
            "{:?} group {:?} has no commands",
            group.scope,
            group.family
        );
        assert!(
            !group.coverage.is_empty(),
            "{:?} group {:?} has no coverage expectations",
            group.scope,
            group.family
        );
        assert!(
            !group.note.is_empty(),
            "{:?} group {:?} has no rationale",
            group.scope,
            group.family
        );
    }
}

/// An independent public-surface oracle catches missing leaves and mismatched
/// semantics while argument details are derived exclusively from clap.
#[test]
fn machine_discovery_covers_executable_leaves_and_their_help() {
    let harness = CliHarness::new().expect("isolated harness");
    let output = harness
        .run_output(&["commands", "list", "--format", "json"])
        .expect("discovery");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let catalog: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one complete JSON document");
    assert_eq!(catalog["schema"], "chatter.commands.v1");
    let entries = catalog["commands"].as_array().expect("command array");
    let expected: BTreeSet<&str> = [
        "validate",
        "fix",
        "format",
        "new",
        "watch",
        "schema",
        "cache stats",
        "cache clear",
        "convert to-json",
        "convert from-json",
        "inspect alignment",
        "inspect words",
        "inspect overlaps",
        "inspect linkers",
        "checks list",
        "checks explain",
        "commands list",
        "commands show",
        "experimental update",
        "experimental speakers map",
        "experimental speakers match",
        "experimental speakers replay",
        "experimental speakers suggest",
        "experimental speakers rediarize",
        "experimental speakers scan",
        "experimental review scripted",
        "experimental review interactive",
        "experimental transform language-runs",
        "experimental transform retag-language",
        "experimental transform join-retrace",
        "experimental transform redact",
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<String> = entries
        .iter()
        .map(|entry| {
            entry["path"]
                .as_array()
                .expect("path")
                .iter()
                .map(|word| word.as_str().expect("path word"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(actual, expected.into_iter().map(str::to_owned).collect());
    assert_eq!(actual.len(), entries.len(), "no duplicate paths");
    for entry in entries {
        let path: Vec<&str> = entry["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        if path.starts_with(&["experimental", "review"]) {
            assert_eq!(entry["semantics"]["stdout"], "no_result");
        }
        let mut command = harness.chatter_cmd();
        let output = command
            .args(&path)
            .arg("--help")
            .output()
            .expect("leaf help");
        assert!(output.status.success(), "{path:?}");
        assert_eq!(
            entry["semantics"]["maturity"],
            if path[0] == "experimental" {
                "experimental"
            } else {
                "general"
            }
        );
        assert!(
            !entry["arguments"].as_array().unwrap().is_empty(),
            "help is always an argument"
        );
        for argument in entry["arguments"].as_array().unwrap() {
            if let Some(long) = argument["long"].as_str() {
                assert!(
                    String::from_utf8_lossy(&output.stdout).contains(&format!("--{long}")),
                    "{path:?}: {long}"
                );
            }
        }
    }
    let validate = entries
        .iter()
        .find(|entry| entry["path"] == serde_json::json!(["validate"]))
        .unwrap();
    assert_eq!(validate["semantics"]["effects"], "validation_cache");
    let clear = entries
        .iter()
        .find(|entry| entry["path"] == serde_json::json!(["cache", "clear"]))
        .unwrap();
    assert_eq!(clear["semantics"]["effects"], "preview_or_apply");
}

#[test]
fn namespaces_are_successful_help_and_invalid_modes_are_usage_errors() {
    let harness = CliHarness::new().expect("isolated harness");
    for path in [
        vec![],
        vec!["convert"],
        vec!["inspect"],
        vec!["checks"],
        vec!["cache"],
        vec!["commands"],
        vec!["experimental"],
        vec!["experimental", "speakers"],
        vec!["experimental", "review"],
        vec!["experimental", "transform"],
    ] {
        let output = harness.run_output(&path).expect("namespace help");
        assert!(
            output.status.success(),
            "{path:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    }
    for args in [
        vec!["validate"],
        vec!["validate", "--list-checks"],
        vec!["schema", "--tui-mode", "disable"],
        vec![
            "experimental",
            "speakers",
            "map",
            "missing.cha",
            "--mapping",
            "PAR=CHI:Target_Child",
            "--reference",
            "missing.cha",
        ],
        vec![
            "experimental",
            "speakers",
            "replay",
            "missing.cha",
            "--override-file",
            "missing.toml",
        ],
        vec![
            "experimental",
            "review",
            "scripted",
            "missing.toml",
            "--override-file",
            "out.toml",
            "--operator",
            "fixture",
        ],
    ] {
        let output = harness.run_output(&args).expect("usage refusal");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
    }
    for old in [
        "normalize",
        "to-json",
        "from-json",
        "show-alignment",
        "clean",
        "new-file",
        "debug",
        "speaker-id",
        "rediarize",
        "adjudicate",
        "sanity-scan",
        "update",
    ] {
        let output = harness
            .run_output(&[old, "--help"])
            .expect("removed spelling");
        assert_eq!(output.status.code(), Some(2), "{old}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Migration: Use chatter"),
            "{old}"
        );
    }
}
