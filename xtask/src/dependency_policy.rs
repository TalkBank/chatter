//! Architecture checks that direct-parent allowlists cannot express.
use crate::Result;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Only a successful Cargo invocation can supply graph evidence.
struct CargoEvidence(String);

impl CargoEvidence {
    fn acquire(root: &Path, args: &[&str]) -> Result<Self> {
        let output = Command::new("cargo")
            .current_dir(root)
            .args(args)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "cargo {} failed ({}): {}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        Ok(Self(String::from_utf8(output.stdout)?))
    }
}

#[derive(Deserialize)]
struct ManifestGraph {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    // Cargo encodes normal dependencies as null; all named kinds are closed.
    kind: Option<NamedKind>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum NamedKind {
    Dev,
    Build,
}

/// Consumed evidence has satisfied every dev-only exception.
struct CheckedTestEdges;

impl ManifestGraph {
    fn check(self) -> Result<CheckedTestEdges> {
        for (parent, dependency) in [
            ("talkbank-parser-tests", "talkbank-parser-re2c"),
            ("talkbank-parser-tests", "talkbank-cache"),
            ("talkbank-parser-re2c", "talkbank-parser"),
            ("talkbank-cache", "talkbank-cache"),
        ] {
            let package = self
                .packages
                .iter()
                .find(|p| p.name == parent)
                .ok_or_else(|| format!("missing policy package {parent}"))?;
            for edge in package.dependencies.iter().filter(|d| d.name == dependency) {
                match edge.kind {
                    Some(NamedKind::Dev) => {}
                    None | Some(NamedKind::Build) => {
                        return Err(format!("{parent} -> {dependency} must remain dev-only").into());
                    }
                }
            }
        }
        Ok(CheckedTestEdges)
    }
}

/// A successfully decoded, selected normal-dependency tree, not a workspace
/// feature union. Cargo's explicit format is package display, '|', feature CSV.
struct LspGraph(Vec<TreePackage>);

struct TreePackage {
    name: String,
    features: Vec<String>,
}

struct CheckedLspGraph;

impl LspGraph {
    fn parse(evidence: CargoEvidence) -> Result<Self> {
        let mut packages = Vec::new();
        for line in evidence.0.lines() {
            let (package, features) = line
                .split_once('|')
                .ok_or("Cargo tree record lacks feature delimiter")?;
            let mut words = package.split_whitespace();
            let name = words.next().ok_or("Cargo tree record lacks package name")?;
            let version = words.next().ok_or("Cargo tree record lacks version")?;
            if !version.starts_with('v') || features.contains('|') {
                return Err("malformed Cargo tree record".into());
            }
            packages.push(TreePackage {
                name: name.to_owned(),
                features: if features.is_empty() {
                    Vec::new()
                } else {
                    features.split(',').map(str::to_owned).collect()
                },
            });
        }
        if !matches!(packages.first(), Some(p) if p.name == "talkbank-lsp") {
            return Err("Cargo tree must start with talkbank-lsp".into());
        }
        Ok(Self(packages))
    }

    fn check(self) -> Result<CheckedLspGraph> {
        if !self.0.iter().any(|p| p.name == "talkbank-transform") {
            return Err("LSP graph lacks the expected transform dependency".into());
        }
        for package in self.0 {
            match package.name.as_str() {
                "talkbank-cache" | "sqlx" | "crossbeam-channel" => {
                    return Err(format!("LSP must not reach {}", package.name).into());
                }
                "talkbank-transform"
                    if package.features.iter().any(|f| f == "validation-runner") =>
                {
                    return Err("LSP must not enable validation-runner".into());
                }
                _ => {}
            }
        }
        Ok(CheckedLspGraph)
    }
}

pub(super) fn run(root: &Path) -> Result<()> {
    let manifests = CargoEvidence::acquire(
        root,
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?;
    let graph: ManifestGraph = serde_json::from_str(&manifests.0)?;
    let CheckedTestEdges = graph.check()?;
    let selected = CargoEvidence::acquire(
        root,
        &[
            "tree",
            "--locked",
            "-p",
            "talkbank-lsp",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--no-dedupe",
            "--format",
            "{p}|{f}",
        ],
    )?;
    let CheckedLspGraph = LspGraph::parse(selected)?.check()?;
    println!("dependency boundaries passed: dev-only exceptions and isolated LSP graph");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn selected_tree_rejects_missing_or_forbidden_evidence() {
        for invalid in ["", "cargo unavailable", "talkbank-lsp v1|\n"] {
            assert!(
                LspGraph::parse(CargoEvidence(invalid.into()))
                    .and_then(LspGraph::check)
                    .is_err()
            );
        }
        let good = "talkbank-lsp v1|\ntalkbank-transform v1|\n";
        assert!(
            LspGraph::parse(CargoEvidence(good.into()))
                .and_then(LspGraph::check)
                .is_ok()
        );
        for forbidden in [
            "talkbank-cache v1|",
            "sqlx v1|",
            "crossbeam-channel v1|",
            "talkbank-transform v1|validation-runner",
        ] {
            assert!(
                LspGraph::parse(CargoEvidence(format!("{good}{forbidden}\n")))
                    .and_then(LspGraph::check)
                    .is_err()
            );
        }
    }

    #[test]
    fn manifest_wire_kinds_cannot_turn_normal_or_build_edges_into_exceptions() {
        for (kind, permitted) in [("\"dev\"", true), ("null", false), ("\"build\"", false)] {
            let json = format!(
                r#"{{"packages":[
                {{"name":"talkbank-parser-tests","dependencies":[{{"name":"talkbank-cache","kind":{kind}}}]}},
                {{"name":"talkbank-parser-re2c","dependencies":[]}},
                {{"name":"talkbank-cache","dependencies":[]}}
            ]}}"#
            );
            let graph: ManifestGraph = serde_json::from_str(&json).expect("wire fixture");
            assert_eq!(graph.check().is_ok(), permitted);
        }
        assert!(serde_json::from_str::<Dependency>(r#"{"name":"x","kind":"unknown"}"#).is_err());
    }
}
