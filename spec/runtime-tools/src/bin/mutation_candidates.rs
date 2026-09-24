//! Emit unreviewed, source-bound terminator-deletion candidates to stdout.
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use serde::Serialize;
use spec_runtime_tools::mutation::AdmittedSeed;
use talkbank_model::RuleSelection;
use talkbank_model::model::{FileStem, TranscriptName};
use talkbank_parser::TreeSitterParser;

#[derive(Parser)]
#[command(about = "Generate unreviewed terminator deletions from a diagnostic-free CHAT seed")]
struct Args {
    /// One canonical CHAT seed; its filename participates in validation.
    input: PathBuf,
    /// Also require strict cross-utterance linker validation at seed admission.
    #[arg(long)]
    strict_linkers: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Assessment {
    Unreviewed,
}

#[derive(Serialize)]
struct Batch<'a> {
    source: &'a std::path::Path,
    transcript_stem: &'a str,
    strict_linkers: bool,
    seed: &'a str,
    candidates: Vec<Candidate>,
}

#[derive(Serialize)]
struct Candidate {
    assessment: Assessment,
    mutation: &'static str,
    deleted_range: std::ops::Range<usize>,
    removed: String,
    chat: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let source = std::fs::read_to_string(&args.input)
        .with_context(|| format!("read seed {}", args.input.display()))?;
    let stem = FileStem::from_path(&args.input).context("seed needs a UTF-8 transcript stem")?;
    let rules = if args.strict_linkers {
        RuleSelection::new().with_strict_linkers()
    } else {
        RuleSelection::new()
    };
    let parser = TreeSitterParser::new()?;
    let seed = AdmittedSeed::admit(&parser, &source, TranscriptName::Named(stem), rules)?;
    // Prepare every candidate before stdout, so a producer-span error cannot
    // masquerade as a successfully completed partial batch.
    let candidates = seed
        .terminator_deletions()
        .map(|candidate| {
            let candidate = candidate?;
            Ok(Candidate {
                assessment: Assessment::Unreviewed,
                mutation: "delete_main_terminator",
                deleted_range: candidate.range(),
                removed: candidate.removed().to_owned(),
                chat: candidate.render(),
            })
        })
        .collect::<Result<Vec<_>, spec_runtime_tools::mutation::SpanMismatch>>()?;
    let batch = Batch {
        source: &args.input,
        transcript_stem: stem.as_str(),
        strict_linkers: seed.rules().strict_linkers_enabled(),
        seed: seed.source(),
        candidates,
    };
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &batch)?;
    writeln!(stdout)?;
    Ok(())
}
