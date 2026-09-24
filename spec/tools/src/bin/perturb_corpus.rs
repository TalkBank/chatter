//! Generate unreviewed CHAT mutation candidates.
//!
//! Input validity and diagnostic expectations are NOT established here.
//! Textual mutations may introduce multiple defects or no defect at all.
//! Review seeds and mutations against canonical specs before promoting a
//! candidate into an authored spec claim; parser output is not an oracle.
//!
//! Usage:
//!   cargo run --bin perturb_corpus -- \
//!     --input ../../corpus/reference/eng/15f_cspu.cha \
//!     --output-dir /tmp/perturbed \
//!     --perturbation delete-participants
//!
//!   # Or apply all perturbations:
//!   cargo run --bin perturb_corpus -- \
//!     --input ../../corpus/reference/eng/15f_cspu.cha \
//!     --output-dir /tmp/perturbed \
//!     --all
//!
//!   # Or mine real errors from a data directory:
//!   cargo run --bin perturb_corpus -- \
//!     --mine ../data/childes-data/Eng-NA \
//!     --output-dir /tmp/mined-errors \
//!     --max-errors 100

use clap::Parser as ClapParser;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(ClapParser)]
#[command(name = "perturb_corpus")]
#[command(about = "Generate unreviewed CHAT mutation candidates or inspect syntax errors")]
struct Args {
    /// Input .cha file or directory of .cha files to perturb
    #[arg(long)]
    input: Option<PathBuf>,

    /// Data directory to mine for real-world errors
    #[arg(long)]
    mine: Option<PathBuf>,

    /// Output directory for perturbed/mined files
    #[arg(long)]
    output_dir: PathBuf,

    /// Specific perturbation to apply (use --list to see available perturbations)
    #[arg(long)]
    perturbation: Option<String>,

    /// Apply all perturbations
    #[arg(long)]
    all: bool,

    /// List available perturbations and exit
    #[arg(long)]
    list: bool,

    /// Maximum errors to collect when mining (default: 50)
    #[arg(long, default_value = "50")]
    max_errors: usize,

    /// Output results as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Perturbation {
    name: &'static str,
    description: &'static str,
}

const PERTURBATIONS: &[Perturbation] = &[
    Perturbation {
        name: "delete-participants",
        description: "Delete @Participants header",
    },
    Perturbation {
        name: "delete-languages",
        description: "Delete @Languages header",
    },
    Perturbation {
        name: "delete-id",
        description: "Delete all @ID headers",
    },
    Perturbation {
        name: "undeclared-speaker",
        description: "Change speaker code to undeclared XXX",
    },
    Perturbation {
        name: "delete-terminator",
        description: "Remove terminator from first utterance",
    },
    Perturbation {
        name: "extra-mor-word",
        description: "Add extra word to first %mor tier",
    },
    Perturbation {
        name: "fewer-mor-words",
        description: "Remove a word from first %mor tier",
    },
    Perturbation {
        name: "delete-begin",
        description: "Delete @Begin header",
    },
    Perturbation {
        name: "delete-end",
        description: "Delete @End header",
    },
    Perturbation {
        name: "duplicate-participants",
        description: "Duplicate the @Participants header",
    },
    Perturbation {
        name: "mor-terminator-mismatch",
        description: "Change %mor terminator to differ from main tier",
    },
];

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.list {
        println!("Available perturbations:\n");
        for p in PERTURBATIONS {
            println!("  {:<30} {} (unreviewed)", p.name, p.description);
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output_dir)?;

    if let Some(mine_dir) = &args.mine {
        mine_real_errors(mine_dir, &args.output_dir, args.max_errors, args.json)?;
    } else if let Some(input) = &args.input {
        let perturbations = if args.all {
            PERTURBATIONS.to_vec()
        } else if let Some(name) = &args.perturbation {
            match PERTURBATIONS.iter().find(|p| p.name == name.as_str()) {
                Some(p) => vec![p.clone()],
                None => {
                    anyhow::bail!(
                        "Unknown perturbation: '{}'. Use --list to see available perturbations.",
                        name
                    );
                }
            }
        } else {
            anyhow::bail!("Specify --perturbation NAME, --all, or --mine DIR");
        };

        let files = collect_cha_files(input)?;
        if files.is_empty() {
            anyhow::bail!("No .cha files found in {:?}", input);
        }

        let mut candidates = Vec::new();
        for file in &files {
            let source = std::fs::read_to_string(file)?;
            let stem = file
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            for p in &perturbations {
                if let Some(mutated) = apply_perturbation(&source, p) {
                    let out_name = format!("{}_{}.cha", stem, p.name.replace('-', "_"));
                    let out_path = args.output_dir.join(&out_name);
                    candidates.push(PreparedCandidate {
                        source: file.clone(),
                        output: out_path,
                        perturbation: p.name,
                        text: mutated,
                    });
                }
            }
        }

        // Admit the entire destination set before the first candidate write.
        // Execution still uses create_new so a concurrent creator cannot be
        // overwritten after this preflight.
        let results = CandidateWritePlan::admit(candidates, |path| path.try_exists())?.write()?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&results)?);
        } else {
            println!(
                "Generated {} unreviewed candidates (not golden tests):\n",
                results.len()
            );
            for r in &results {
                println!("  [unreviewed] {} → {}", r.perturbation, r.output);
            }
        }
    } else {
        anyhow::bail!("Specify --input FILE or --mine DIR");
    }

    Ok(())
}

struct PreparedCandidate {
    source: PathBuf,
    output: PathBuf,
    perturbation: &'static str,
    text: String,
}

/// Only an admitted, collision-free destination set can execute writes.
/// This is output admission, not CHAT validity or golden-test approval.
struct CandidateWritePlan(Vec<PreparedCandidate>);

impl CandidateWritePlan {
    fn admit(
        candidates: Vec<PreparedCandidate>,
        mut exists: impl FnMut(&Path) -> std::io::Result<bool>,
    ) -> anyhow::Result<Self> {
        let mut destinations = std::collections::BTreeSet::new();
        for candidate in &candidates {
            if !destinations.insert(&candidate.output) {
                anyhow::bail!("Candidate output collision: {}", candidate.output.display());
            }
            if exists(&candidate.output)? {
                anyhow::bail!(
                    "Refusing to overwrite candidate output: {}",
                    candidate.output.display()
                );
            }
        }
        Ok(Self(candidates))
    }

    fn write(self) -> anyhow::Result<Vec<UnreviewedCandidate>> {
        use std::io::Write;
        let mut results = Vec::with_capacity(self.0.len());
        for candidate in self.0 {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate.output)?;
            file.write_all(candidate.text.as_bytes())?;
            results.push(UnreviewedCandidate {
                source: candidate.source.to_string_lossy().to_string(),
                output: candidate.output.to_string_lossy().to_string(),
                perturbation: candidate.perturbation.to_owned(),
            });
        }
        Ok(results)
    }
}

/// This phase deliberately has no diagnostic claim field. Promotion belongs
/// to the reviewed spec vocabulary, not to a string table in the mutator.
#[derive(Debug, Serialize)]
#[serde(tag = "assessment", rename = "unreviewed")]
struct UnreviewedCandidate {
    source: String,
    output: String,
    perturbation: String,
}

fn collect_cha_files(path: &PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.clone());
    } else {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Skip Password directories
                !e.path()
                    .components()
                    .any(|c| c.as_os_str().eq_ignore_ascii_case("password"))
            })
        {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("cha") {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    Ok(files)
}

fn apply_perturbation(source: &str, perturbation: &Perturbation) -> Option<String> {
    match perturbation.name {
        "delete-participants" => {
            let lines: Vec<&str> = source.lines().collect();
            if !lines.iter().any(|l| l.starts_with("@Participants:")) {
                return None;
            }
            let result: Vec<&str> = lines
                .iter()
                .filter(|l| !l.starts_with("@Participants:"))
                .copied()
                .collect();
            Some(result.join("\n") + "\n")
        }
        "delete-languages" => {
            let lines: Vec<&str> = source.lines().collect();
            if !lines.iter().any(|l| l.starts_with("@Languages:")) {
                return None;
            }
            let result: Vec<&str> = lines
                .iter()
                .filter(|l| !l.starts_with("@Languages:"))
                .copied()
                .collect();
            Some(result.join("\n") + "\n")
        }
        "delete-id" => {
            let lines: Vec<&str> = source.lines().collect();
            if !lines.iter().any(|l| l.starts_with("@ID:")) {
                return None;
            }
            let result: Vec<&str> = lines
                .iter()
                .filter(|l| !l.starts_with("@ID:"))
                .copied()
                .collect();
            Some(result.join("\n") + "\n")
        }
        "undeclared-speaker" => {
            // Replace first *SPEAKER: with *XXX:
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.starts_with('*')
                    && !found
                    && let Some(colon_pos) = line.find(':')
                {
                    let rest = &line[colon_pos..];
                    *line = format!("*XXX{}", rest);
                    found = true;
                }
            }
            if !found {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        "delete-terminator" => {
            // Remove the terminator from the first utterance
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.starts_with('*') && !found {
                    // Remove trailing terminator (. ? ! +/. +... etc.)
                    let trimmed = line.trim_end();
                    for term in &[".", "?", "!"] {
                        if trimmed.ends_with(term)
                            && !trimmed.ends_with("+...")
                            && !trimmed.ends_with("+/.")
                        {
                            *line = trimmed[..trimmed.len() - term.len()].trim_end().to_string();
                            found = true;
                            break;
                        }
                    }
                }
            }
            if !found {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        "extra-mor-word" => {
            // Add an extra word to the first %mor tier
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.starts_with("%mor:") && !found {
                    // Insert an extra word after the tab
                    if let Some(tab_pos) = line.find('\t') {
                        let prefix = &line[..tab_pos + 1];
                        let content = &line[tab_pos + 1..];
                        *line = format!("{}n|extra {}", prefix, content);
                        found = true;
                    }
                }
            }
            if !found {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        "fewer-mor-words" => {
            // Remove the first word from the first %mor tier
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.starts_with("%mor:")
                    && !found
                    && let Some(tab_pos) = line.find('\t')
                {
                    let content = &line[tab_pos + 1..];
                    // Remove first word (up to first space)
                    if let Some(space_pos) = content.find(' ') {
                        *line = format!("{}{}", &line[..tab_pos + 1], &content[space_pos + 1..]);
                        found = true;
                    }
                }
            }
            if !found {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        "delete-begin" => {
            let lines: Vec<&str> = source.lines().collect();
            if !lines.iter().any(|l| l.starts_with("@Begin")) {
                return None;
            }
            let result: Vec<&str> = lines
                .iter()
                .filter(|l| !l.starts_with("@Begin"))
                .copied()
                .collect();
            Some(result.join("\n") + "\n")
        }
        "delete-end" => {
            let lines: Vec<&str> = source.lines().collect();
            if !lines.iter().any(|l| l.starts_with("@End")) {
                return None;
            }
            let result: Vec<&str> = lines
                .iter()
                .filter(|l| !l.starts_with("@End"))
                .copied()
                .collect();
            Some(result.join("\n") + "\n")
        }
        "duplicate-participants" => {
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut inserted = false;
            let mut i = 0;
            while i < lines.len() && !inserted {
                if lines[i].starts_with("@Participants:") {
                    lines.insert(i + 1, lines[i].clone());
                    inserted = true;
                }
                i += 1;
            }
            if !inserted {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        "mor-terminator-mismatch" => {
            // Change first %mor terminator to "?" if main tier has "."
            let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                if line.starts_with("%mor:") && !found {
                    let trimmed = line.trim_end();
                    if let Some(prefix) = trimmed.strip_suffix(" .") {
                        *line = format!("{prefix} ?");
                        found = true;
                    }
                }
            }
            if !found {
                return None;
            }
            Some(lines.join("\n") + "\n")
        }
        _ => None,
    }
}

fn mine_real_errors(
    data_dir: &Path,
    output_dir: &Path,
    max_errors: usize,
    json_output: bool,
) -> anyhow::Result<()> {
    use tree_sitter::Parser as TSParser;
    use tree_sitter_talkbank::LANGUAGE;

    let mut parser = TSParser::new();
    parser.set_language(&LANGUAGE.into())?;

    let mut error_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut error_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total_files = 0;
    let mut error_file_count = 0;

    eprintln!("Mining errors from {:?}...", data_dir);

    for entry in WalkDir::new(data_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Skip Password and xml directories
            !e.path().components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.eq_ignore_ascii_case("password") || s.ends_with("-xml")
            })
        })
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cha") {
            continue;
        }

        total_files += 1;

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Parse with tree-sitter
        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => continue,
        };

        // Check for ERROR nodes
        if has_error_node(tree.root_node()) {
            error_file_count += 1;
            // Classify errors by looking at ERROR node context
            let errors = classify_errors(tree.root_node(), &source);
            for error_type in &errors {
                *error_counts.entry(error_type.clone()).or_insert(0) += 1;
                error_files
                    .entry(error_type.clone())
                    .or_default()
                    .push(path.to_string_lossy().to_string());
            }
        }

        if error_file_count >= max_errors {
            break;
        }
    }

    // Write a summary
    let summary_path = output_dir.join("mining_summary.json");
    let summary = serde_json::json!({
        "total_files_scanned": total_files,
        "files_with_errors": error_file_count,
        "error_type_counts": error_counts,
        "sample_files": error_files.iter().map(|(k, v)| {
            (k.clone(), v.iter().take(3).cloned().collect::<Vec<_>>())
        }).collect::<BTreeMap<_, _>>(),
    });
    std::fs::write(&summary_path, serde_json::to_string_pretty(&summary)?)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        eprintln!(
            "\nScanned {} files, {} with parse errors",
            total_files, error_file_count
        );
        println!("\nError type distribution:");
        for (error_type, count) in &error_counts {
            println!("  {:<40} {}", error_type, count);
        }
        println!("\nSummary written to {:?}", summary_path);
    }

    Ok(())
}

fn has_error_node(node: tree_sitter::Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_error_node(child) {
            return true;
        }
    }
    false
}

fn classify_errors(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut errors = Vec::new();
    collect_error_types(node, source, &mut errors);
    errors.sort();
    errors.dedup();
    errors
}

fn collect_error_types(node: tree_sitter::Node, source: &str, errors: &mut Vec<String>) {
    if node.is_error() {
        // Try to classify based on parent context
        let context = if let Some(parent) = node.parent() {
            match parent.kind() {
                "utterance" | "main_tier" => "utterance_error",
                "header"
                | "participants_header"
                | "languages_header"
                | "id_header"
                | "date_header"
                | "media_header" => "header_error",
                "mor_dependent_tier" | "gra_dependent_tier" | "pho_dependent_tier"
                | "wor_dependent_tier" | "sin_dependent_tier" => "tier_error",
                "line" => "line_error",
                _ => "other_error",
            }
        } else {
            "root_error"
        };

        // Also note what text is in the error
        let text = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .chars()
            .take(50)
            .collect::<String>();
        errors.push(format!("{}: {}", context, text.replace('\n', "\\n")));
    }

    if node.is_missing() {
        errors.push(format!("missing_{}", node.kind()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_types(child, source, errors);
    }
}

#[cfg(test)]
mod candidate_contract {
    use super::{CandidateWritePlan, PreparedCandidate, UnreviewedCandidate};

    fn candidate(source: &str, output: &str) -> PreparedCandidate {
        PreparedCandidate {
            source: source.into(),
            output: output.into(),
            perturbation: "delete-end",
            text: "unreviewed candidate".into(),
        }
    }

    #[test]
    fn output_admission_refuses_collisions_existing_paths_and_failed_probes() {
        let collision = CandidateWritePlan::admit(
            vec![
                candidate("first/seed.cha", "seed_delete_end.cha"),
                candidate("second/seed.cha", "seed_delete_end.cha"),
            ],
            |_| Ok(false),
        );
        assert!(collision.is_err());
        assert!(
            CandidateWritePlan::admit(vec![candidate("seed.cha", "existing.cha")], |_| Ok(true))
                .is_err()
        );
        assert!(
            CandidateWritePlan::admit(vec![candidate("seed.cha", "unknown.cha")], |_| Err(
                std::io::Error::other("probe refused")
            ))
            .is_err()
        );
    }

    #[test]
    fn output_admission_preserves_distinct_candidates_without_writing() {
        let plan = CandidateWritePlan::admit(
            vec![
                candidate("first.cha", "first_delete_end.cha"),
                candidate("second.cha", "second_delete_end.cha"),
            ],
            |_| Ok(false),
        )
        .expect("distinct absent destinations");
        assert_eq!(plan.0.len(), 2);
        assert_eq!(plan.0[0].source.to_str(), Some("first.cha"));
        assert_eq!(plan.0[1].source.to_str(), Some("second.cha"));
    }

    #[test]
    fn execution_refuses_a_file_created_after_admission() -> anyhow::Result<()> {
        use std::io::Write;
        // One owned file in the existing system temporary directory; no new
        // directory and no canonical source can be touched by this test.
        let mut existing = tempfile::NamedTempFile::new()?;
        existing.write_all(b"preserve existing output")?;
        let planned = PreparedCandidate {
            source: "seed.cha".into(),
            output: existing.path().to_owned(),
            perturbation: "delete-end",
            text: "must not overwrite".into(),
        };
        // Model the destination appearing between admission and execution.
        let plan = CandidateWritePlan::admit(vec![planned], |_| Ok(false))?;
        assert!(plan.write().is_err());
        assert_eq!(std::fs::read(existing.path())?, b"preserve existing output");
        Ok(())
    }

    #[test]
    fn serialized_candidate_is_not_a_diagnostic_claim() -> Result<(), serde_json::Error> {
        let candidate = UnreviewedCandidate {
            source: "seed.cha".into(),
            output: "seed_delete_end.cha".into(),
            perturbation: "delete-end".into(),
        };
        assert_eq!(
            serde_json::to_value(candidate)?,
            serde_json::json!({
                "assessment": "unreviewed",
                "source": "seed.cha",
                "output": "seed_delete_end.cha",
                "perturbation": "delete-end",
            })
        );
        Ok(())
    }
}
