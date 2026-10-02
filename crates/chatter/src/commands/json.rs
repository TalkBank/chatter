//! JSON conversion commands (to-json, from-json).
//!
//! `chat_to_json` optionally runs validation/alignment and schema checking before
//! serializing to JSON. `json_to_chat` parses the JSON back into a `ChatFile`
//! and writes canonical CHAT text. Keeping both conversions in one module keeps
//! command-level concerns centralized.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::fs;
use std::path::{Path, PathBuf};
use talkbank_model::model::{ChatFile, WriteChat};
use talkbank_transform::paths::{FoundTranscript, Links, walk_files};
use tracing::{Level, debug, info, span, warn};

use talkbank_model::{CheckLevel, ParseValidateOptions};
use talkbank_transform::{JsonLayout, JsonSchemaPolicy};

/// Whether a directory conversion rewrites JSON that is already newer than
/// its transcript (`--force`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRefresh {
    /// Skip a transcript whose JSON is newer (the default).
    Incremental,
    /// Convert every transcript.
    Rebuild,
}

/// What a directory conversion does with JSON whose transcript is gone
/// (`--prune`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanJson {
    /// Leave it (the default).
    Keep,
    /// Delete it, and any directory that leaves empty.
    Prune,
}

/// What `to-json` converts: one file, or a directory tree, with only the
/// options that mode reads. Built by [`ToJsonTarget::resolve`], which refuses
/// a directory-mode option given for a file (and a file-mode one for a
/// directory) instead of ignoring it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToJsonTarget {
    /// One transcript; JSON to `output`, or stdout.
    File {
        /// `-o/--output`.
        output: Option<PathBuf>,
    },
    /// Every transcript under a directory, mirrored under `output_dir`.
    Directory {
        /// `--output-dir`.
        output_dir: PathBuf,
        /// `--force`.
        refresh: JsonRefresh,
        /// `--prune`.
        orphans: OrphanJson,
        /// `--jobs`.
        jobs: Option<std::num::NonZeroUsize>,
    },
}

/// A `to-json` option given for the other mode than the input's.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToJsonUsage {
    /// A directory input needs somewhere to write.
    #[error("directory input requires --output-dir")]
    DirectoryNeedsOutputDir,
    /// A directory input writes one JSON file per transcript, not one file.
    #[error("--output names one file; a directory input writes under --output-dir")]
    OutputForDirectory,
    /// These options only apply to a directory input.
    #[error("{flags} only apply to a directory input")]
    DirectoryOptionsForFile {
        /// The options given, as typed.
        flags: String,
    },
}

/// What a `to-json` input is on disk, which decides its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// Anything but a directory: one transcript.
    File,
    /// A directory tree of transcripts.
    Directory,
}

impl InputKind {
    /// What `path` is now.
    pub fn of(path: &std::path::Path) -> Self {
        match path.is_dir() {
            true => Self::Directory,
            false => Self::File,
        }
    }
}

impl ToJsonTarget {
    /// The target for an input of `kind`, from the options as parsed.
    pub fn resolve(
        kind: InputKind,
        output: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        refresh: JsonRefresh,
        orphans: OrphanJson,
        jobs: Option<std::num::NonZeroUsize>,
    ) -> Result<Self, ToJsonUsage> {
        match kind {
            InputKind::Directory => match (output, output_dir) {
                (Some(_), _) => Err(ToJsonUsage::OutputForDirectory),
                (None, None) => Err(ToJsonUsage::DirectoryNeedsOutputDir),
                (None, Some(output_dir)) => Ok(Self::Directory {
                    output_dir,
                    refresh,
                    orphans,
                    jobs,
                }),
            },
            InputKind::File => {
                let given: Vec<&str> = [
                    output_dir.is_some().then_some("--output-dir"),
                    matches!(refresh, JsonRefresh::Rebuild).then_some("--force"),
                    matches!(orphans, OrphanJson::Prune).then_some("--prune"),
                    jobs.is_some().then_some("--jobs"),
                ]
                .into_iter()
                .flatten()
                .collect();
                match given.is_empty() {
                    true => Ok(Self::File { output }),
                    false => Err(ToJsonUsage::DirectoryOptionsForFile {
                        flags: given.join(", "),
                    }),
                }
            }
        }
    }
}

/// Everything `to-json` checks before it writes a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToJsonChecks {
    /// The CHAT checks.
    pub chat: CheckLevel,
    /// Whether the serialized JSON is checked against the schema.
    pub schema: JsonSchemaPolicy,
}

impl ToJsonChecks {
    /// The pipeline options for the CHAT checks.
    fn options(self) -> ParseValidateOptions {
        ParseValidateOptions::default().with_level(self.chat)
    }
}

/// Convert the CHAT file into a JSON representation, optionally running validation/alignment.
///
/// This function reads the file, configures the pipeline options (validation + `%wor` alignment),
/// and routes through `talkbank_transform::chat_to_json` so that the resulting JSON matches the
/// structure described in the CHAT manual's File Format and Main Tier sections. Schema checks
/// mirror the CHAT manual’s requirements when not explicitly skipped, and any validation failures
/// emit the same diagnostic codes the manual discusses before exiting with a failure status.
pub fn chat_to_json(
    input: &PathBuf,
    output: Option<&PathBuf>,
    layout: JsonLayout,
    checks: ToJsonChecks,
) {
    let _span = span!(Level::INFO, "chat_to_json", input = %input.display()).entered();
    info!("Converting CHAT to JSON");
    let stored = match talkbank_transform::paths::StoredTranscript::resolve(input) {
        Ok(stored) => stored,
        Err(error) => {
            eprintln!(
                "Cannot resolve transcript name {}: {error}",
                input.display()
            );
            std::process::exit(1);
        }
    };

    // Read CHAT file
    let content = {
        let _span = span!(Level::DEBUG, "read_file").entered();
        match fs::read_to_string(input) {
            Ok(c) => {
                debug!("Read {} bytes from file", c.len());
                c
            }
            Err(e) => {
                warn!("Failed to read file: {}", e);
                eprintln!("Error reading file {:?}: {}", input, e);
                std::process::exit(1);
            }
        }
    };

    // Parse, run the requested checks, serialize, and check the schema
    // unless skipped.
    let json = {
        let _span = span!(Level::DEBUG, "pipeline").entered();
        let result = talkbank_transform::chat_to_json_with_schema_policy(
            &content,
            checks.options(),
            layout,
            stored.name(),
            checks.schema,
        );
        match result {
            Ok(json_str) => {
                debug!("Pipeline successful, {} bytes", json_str.len());
                match checks.chat {
                    CheckLevel::ParseOnly => {}
                    CheckLevel::Validate(_) => {
                        info!("✓ Validation passed");
                        eprintln!("✓ Validation passed");
                    }
                }
                match checks.schema {
                    JsonSchemaPolicy::Validate => info!("✓ JSON schema validation passed"),
                    JsonSchemaPolicy::Skip => {}
                }
                json_str
            }
            Err(e) => {
                crate::output::report_pipeline_failure(input, &content, &e);
                std::process::exit(1);
            }
        }
    };

    // Write or print JSON
    if let Some(output_path) = output {
        let _span = span!(Level::DEBUG, "write_output").entered();
        if let Err(e) = fs::write(output_path, &json) {
            warn!("Failed to write output: {}", e);
            eprintln!("Error writing JSON to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        info!("Converted {} to {}", input.display(), output_path.display());
        eprintln!(
            "✓ Converted {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        outln!("{}", json);
    }
}

/// Convert a JSON representation back into canonical CHAT text.
///
/// The deserialization/serialization cycle mirrors the chat format described in the manual's
/// File Format and Dependent Tier sections, and errors bubble up so callers receive clear
/// CHAT-aligned diagnostics when the JSON is malformed or cannot be emitted.
pub fn json_to_chat(input: &PathBuf, output: Option<&PathBuf>) {
    let _span = span!(Level::INFO, "json_to_chat", input = %input.display()).entered();
    info!("Converting JSON to CHAT");

    // Read JSON file
    let content = {
        let _span = span!(Level::DEBUG, "read_file").entered();
        match fs::read_to_string(input) {
            Ok(c) => {
                debug!("Read {} bytes from file", c.len());
                c
            }
            Err(e) => {
                warn!("Failed to read file: {}", e);
                eprintln!("Error reading file {:?}: {}", input, e);
                std::process::exit(1);
            }
        }
    };

    // Deserialize JSON to ChatFile
    let chat_file: ChatFile = {
        let _span = span!(Level::DEBUG, "deserialize_json").entered();
        match serde_json::from_str(&content) {
            Ok(cf) => {
                info!("Deserialized ChatFile successfully");
                cf
            }
            Err(e) => {
                warn!("JSON parse error: {}", e);
                eprintln!("Error parsing JSON: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Serialize to CHAT format
    let chat_text = {
        let _span = span!(Level::DEBUG, "serialize_to_chat").entered();
        let result = chat_file.to_chat_string();
        debug!("Serialized to {} bytes", result.len());
        result
    };

    // Write or print CHAT
    if let Some(output_path) = output {
        let _span = span!(Level::DEBUG, "write_output").entered();
        if let Err(e) = fs::write(output_path, &chat_text) {
            warn!("Failed to write output: {}", e);
            eprintln!("Error writing CHAT to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        info!("Converted {} to {}", input.display(), output_path.display());
        eprintln!(
            "✓ Converted {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        out!("{}", chat_text);
    }
}

/// Convert all CHAT files in a directory to JSON, preserving directory structure.
///
/// Walks `input_dir` recursively, converting each `.cha` file to a `.json`
/// file under `output_dir` with the same relative path. With
/// [`JsonRefresh::Incremental`] a transcript whose JSON is already newer is
/// skipped, and [`JsonRefresh::Rebuild`] converts every one;
/// [`OrphanJson::Prune`] removes JSON whose transcript is gone.
pub fn chat_to_json_directory(
    input_dir: &Path,
    output_dir: &Path,
    layout: JsonLayout,
    checks: ToJsonChecks,
    refresh: JsonRefresh,
    orphans: OrphanJson,
    jobs: Option<std::num::NonZeroUsize>,
) {
    let _span =
        span!(Level::INFO, "chat_to_json_directory", input = %input_dir.display()).entered();

    // Every transcript under the input directory (AppleDouble `._*.cha`
    // sidecars excluded), each with its path relative to the input: at least
    // one. An entry that cannot be read, or an input with no transcript (an
    // empty tree, an unmounted mount point), refuses the run before anything
    // is converted or pruned.
    let transcripts =
        super::inputs::readable_transcripts_under(input_dir).unwrap_or_else(|refusal| {
            eprintln!("ERROR: {refusal}");
            std::process::exit(1);
        });

    let total = transcripts.count().get();
    eprintln!("Found {total} .cha files in {}", input_dir.display());

    // What a prune keeps, from the population just found; `None` when there
    // is no prune.
    let owned_json = match orphans {
        OrphanJson::Keep => None,
        OrphanJson::Prune => Some(OwnedJson::of(&transcripts)),
    };

    let settings = ConvertSettings {
        output_dir,
        layout,
        checks,
        refresh,
    };
    // The shared pool (serial is a pool of width one). Each worker returns
    // its own counts, summed after the join; the feeder prints progress.
    let run = talkbank_transform::worker_pool::fan_out(
        transcripts
            .into_iter()
            .enumerate()
            .map(|(index, transcript)| {
                report_progress(index, total);
                transcript
            }),
        jobs,
        |work| {
            let mut counts = ConversionCounts::default();
            for transcript in work {
                counts.record(convert_one_file(&transcript, &settings));
            }
            counts
        },
    );
    let counts = run
        .results
        .into_iter()
        .fold(ConversionCounts::default(), ConversionCounts::add);
    let ConversionCounts {
        converted: conv,
        up_to_date: skip,
        failed: fail,
    } = counts;
    let mut pool_failed = false;
    for fault in run.outcome.faults() {
        pool_failed = true;
        eprintln!("ERROR: conversion workers failed: {fault}");
    }
    // Files fed to a worker that unwound, or never fed because the pool could
    // not start, produced no outcome. Measured, not assumed, and a shortfall
    // fails the run whether or not a fault explains it: one no fault explains
    // is a defect in this command, never a clean run.
    let covered = match counts.total().cmp(&total) {
        std::cmp::Ordering::Equal => true,
        std::cmp::Ordering::Less => {
            let unaccounted = total - counts.total();
            eprintln!("ERROR: {unaccounted} file(s) were not converted");
            false
        }
        std::cmp::Ordering::Greater => {
            eprintln!(
                "ERROR: {} outcomes for {total} file(s); this is a defect in the converter",
                counts.total()
            );
            false
        }
    };

    let prune_report = match &owned_json {
        None => PruneReport::default(),
        Some(owned_json) => prune_orphaned_json(input_dir, output_dir, owned_json),
    };
    let pruned = prune_report.pruned;

    eprintln!();
    eprintln!(
        "Done: {conv} converted, {skip} up-to-date, {fail} failed, {pruned} pruned (of {total} total)"
    );
    if prune_report.failures > 0 {
        eprintln!("ERROR: {} pruning step(s) failed", prune_report.failures);
    }
    if fail > 0 || pool_failed || !covered || prune_report.failures > 0 {
        std::process::exit(1);
    }
}

/// Print a progress line every thousand files, before file `index` is fed.
fn report_progress(index: usize, total: usize) {
    if index > 0 && index.is_multiple_of(1000) {
        eprintln!("  ...{index}/{total} files queued");
    }
}

/// The settings every file of one directory conversion shares.
struct ConvertSettings<'a> {
    output_dir: &'a Path,
    layout: JsonLayout,
    checks: ToJsonChecks,
    refresh: JsonRefresh,
}

/// What happened to one transcript in a directory conversion. Each file
/// produces exactly one, so each is counted exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileConversion {
    /// The JSON was written.
    Converted,
    /// The JSON was already newer than the transcript, and `--force` was off.
    UpToDate,
    /// The transcript could not be converted or its JSON not written; the
    /// reason has been printed.
    Failed,
}

/// One worker's totals of [`FileConversion`]s, combined after the join.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ConversionCounts {
    converted: usize,
    up_to_date: usize,
    failed: usize,
}

impl ConversionCounts {
    /// Count one file's outcome in its bucket.
    fn record(&mut self, outcome: FileConversion) {
        let counter = match outcome {
            FileConversion::Converted => &mut self.converted,
            FileConversion::UpToDate => &mut self.up_to_date,
            FileConversion::Failed => &mut self.failed,
        };
        *counter += 1;
    }

    /// Two workers' totals together.
    fn add(self, other: Self) -> Self {
        Self {
            converted: self.converted + other.converted,
            up_to_date: self.up_to_date + other.up_to_date,
            failed: self.failed + other.failed,
        }
    }

    /// How many files these counts account for.
    fn total(self) -> usize {
        self.converted + self.up_to_date + self.failed
    }
}

/// Convert a single .cha file to .json under the output directory.
fn convert_one_file(
    transcript: &FoundTranscript,
    settings: &ConvertSettings<'_>,
) -> FileConversion {
    let ConvertSettings {
        output_dir,
        layout,
        checks,
        refresh,
    } = *settings;
    let cha_path = transcript.path();
    // The walk proved the transcript is under the input directory at this
    // relative path, so the JSON goes to the same place under the output.
    let json_path = output_dir
        .join(transcript.relative())
        .with_extension("json");

    match refresh {
        JsonRefresh::Rebuild => {}
        JsonRefresh::Incremental => {
            if json_is_newer(cha_path, &json_path) {
                return FileConversion::UpToDate;
            }
        }
    }

    // The walk named the transcript from its own listing.
    let stored = transcript.stored();
    // Read source
    let content = match fs::read_to_string(cha_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: cannot read {}: {e}", cha_path.display());
            return FileConversion::Failed;
        }
    };

    // Convert
    let json = talkbank_transform::chat_to_json_with_schema_policy(
        &content,
        checks.options(),
        layout,
        stored.name(),
        checks.schema,
    );

    match json {
        Ok(json_str) => {
            // Ensure parent directory exists
            if let Some(parent) = json_path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                eprintln!("ERROR: cannot create directory {}: {e}", parent.display());
                return FileConversion::Failed;
            }
            if let Err(e) = fs::write(&json_path, &json_str) {
                eprintln!("ERROR: cannot write {}: {e}", json_path.display());
                return FileConversion::Failed;
            }
            FileConversion::Converted
        }
        Err(e) => {
            crate::output::report_pipeline_failure(cha_path, &content, &e);
            // Remove the stale JSON of an earlier successful run, so the
            // output never holds a conversion of different content. None to
            // remove is the ordinary case; failing to remove one is said.
            match fs::remove_file(&json_path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("WARN: cannot remove stale {}: {e}", json_path.display()),
            }
            FileConversion::Failed
        }
    }
}

/// Whether the JSON at `json_path` exists and changed no earlier than its
/// transcript. Any time that cannot be read means "not known to be newer",
/// so the transcript is converted again: the safe direction, never a skip
/// on a guess.
fn json_is_newer(cha_path: &Path, json_path: &Path) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|metadata| metadata.modified());
    match (modified(cha_path), modified(json_path)) {
        (Ok(cha_time), Ok(json_time)) => json_time >= cha_time,
        (Err(_), _) | (_, Err(_)) => false,
    }
}

/// What pruning did: how many orphaned JSON files it removed, and how many
/// of its steps failed (each reported as it happened).
#[derive(Debug, Default)]
struct PruneReport {
    pruned: usize,
    failures: usize,
}

impl PruneReport {
    /// Report one failed step now and count it.
    fn fail(&mut self, message: String) {
        eprintln!("ERROR: {message}");
        self.failures += 1;
    }
}

/// The relative JSON paths a non-empty transcript population owns: what a
/// prune keeps without a second look at the input tree.
///
/// Built only from a [`NonEmpty`](super::inputs::NonEmpty) population, so a
/// prune never runs over an empty input, where every JSON file under the
/// output directory would read as an orphan.
struct OwnedJson(std::collections::HashSet<PathBuf>);

impl OwnedJson {
    /// The JSON paths `transcripts` own.
    fn of(transcripts: &super::inputs::NonEmpty<FoundTranscript>) -> Self {
        Self(
            transcripts
                .iter()
                .map(|transcript| transcript.relative().with_extension("json"))
                .collect(),
        )
    }

    /// Whether a transcript of this run owns `relative`.
    fn contains(&self, relative: &Path) -> bool {
        self.0.contains(relative)
    }
}

/// Remove `.json` files in `output_dir` that have no matching `.cha` in
/// `input_dir`. `owned_json` holds the relative JSON paths of the transcripts
/// this run walked; only a JSON file outside it is checked against the input
/// tree.
fn prune_orphaned_json(input_dir: &Path, output_dir: &Path, owned_json: &OwnedJson) -> PruneReport {
    let mut report = PruneReport::default();
    // Links are skipped, never followed: this walk's results are deleted,
    // and a link reaches outside the tree this command owns (a linked
    // directory's JSON, and then the directories the empty-directory climb
    // would remove).
    let (found, failures) = walk_files(output_dir, Links::Skip, |name| {
        name.extension().is_some_and(|ext| ext == "json")
    })
    .into_parts();
    for failure in failures {
        report.fail(failure.to_string());
    }
    for json in found {
        if owned_json.contains(json.relative()) {
            continue;
        }
        let cha_path = input_dir.join(json.relative()).with_extension("cha");
        // `exists` would answer "no" for a transcript it cannot see and so
        // delete JSON whose transcript is merely unreadable.
        match cha_path.try_exists() {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                report.fail(format!(
                    "cannot tell whether {} exists, keeping its JSON: {e}",
                    cha_path.display()
                ));
                continue;
            }
        }
        let json_path = json.path();
        if let Err(e) = fs::remove_file(json_path) {
            report.fail(format!("cannot prune {}: {e}", json_path.display()));
            continue;
        }
        report.pruned += 1;
        // Remove the directories this leaves empty, up to the output root.
        let mut dir = json_path.parent();
        while let Some(d) = dir.filter(|d| *d != output_dir) {
            match fs::read_dir(d).map(|mut entries| entries.next().is_none()) {
                Ok(true) => {}
                Ok(false) => break,
                Err(e) => {
                    report.fail(format!("cannot list {} to prune it: {e}", d.display()));
                    break;
                }
            }
            if let Err(e) = fs::remove_dir(d) {
                report.fail(format!("cannot remove empty {}: {e}", d.display()));
                break;
            }
            dir = d.parent();
        }
    }
    report
}

#[cfg(test)]
mod target_tests {
    use super::*;

    /// Each mode keeps only its own options, and an option for the other
    /// mode is refused rather than ignored.
    #[test]
    fn a_target_takes_only_its_own_mode_options() {
        let jobs = std::num::NonZeroUsize::new(4);
        assert_eq!(
            ToJsonTarget::resolve(
                InputKind::File,
                None,
                None,
                JsonRefresh::Incremental,
                OrphanJson::Keep,
                None
            ),
            Ok(ToJsonTarget::File { output: None })
        );
        assert_eq!(
            ToJsonTarget::resolve(
                InputKind::File,
                None,
                Some(PathBuf::from("out")),
                JsonRefresh::Rebuild,
                OrphanJson::Prune,
                jobs,
            ),
            Err(ToJsonUsage::DirectoryOptionsForFile {
                flags: "--output-dir, --force, --prune, --jobs".to_owned()
            })
        );
        assert_eq!(
            ToJsonTarget::resolve(
                InputKind::Directory,
                None,
                Some(PathBuf::from("out")),
                JsonRefresh::Rebuild,
                OrphanJson::Keep,
                jobs,
            ),
            Ok(ToJsonTarget::Directory {
                output_dir: PathBuf::from("out"),
                refresh: JsonRefresh::Rebuild,
                orphans: OrphanJson::Keep,
                jobs,
            })
        );
        assert_eq!(
            ToJsonTarget::resolve(
                InputKind::Directory,
                None,
                None,
                JsonRefresh::Incremental,
                OrphanJson::Keep,
                None
            ),
            Err(ToJsonUsage::DirectoryNeedsOutputDir)
        );
        assert_eq!(
            ToJsonTarget::resolve(
                InputKind::Directory,
                Some(PathBuf::from("one.json")),
                Some(PathBuf::from("out")),
                JsonRefresh::Incremental,
                OrphanJson::Keep,
                None,
            ),
            Err(ToJsonUsage::OutputForDirectory)
        );
    }
}
