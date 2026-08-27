//! Which grammar node types the reference corpus actually exercises.
//!
//! # Why this is a library module
//!
//! It lived entirely in `bin/corpus_node_coverage.rs`, ending in
//! `std::process::exit(1)`. CI runs `cargo test`, never `cargo run`, so the
//! exit code had never been observed by anything, while
//! `book/src/contributing/reference-corpus.md` cites the tool as the coverage
//! check. The gate is now `tests/node_coverage.rs`.
//!
//! # An exclusion has a KIND, and the kind decides what else to check
//!
//! The exclusion list was `&[&str]` with its reasons in comment groups. That
//! is fine for reading and useless for checking, and it hid a question nobody
//! could ask: three of the missing types (`blank_line`, `illegal_curly_quote`,
//! `sep_trailing_space`) are not gaps at all. They are nodes the grammar
//! carries so the parser can DETECT invalid CHAT, and each has an error code
//! (E747, E256, E758). A valid reference corpus cannot contain them.
//!
//! Once that reason is data rather than prose, it implies a check that did not
//! exist: if an invalid-by-construction node ever DOES appear in the reference
//! corpus, the corpus has acquired invalid content, and that is a finding
//! rather than an improvement in coverage. A flat string list can only ever
//! say "ignore this one", which is the same answer for both kinds and the
//! wrong one for this kind.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;
use tree_sitter::Parser as TSParser;
use tree_sitter_talkbank::LANGUAGE;
use walkdir::WalkDir;

use crate::repo_paths::RepoRoot;

/// Node types the grammar carries so the parser can DETECT invalid CHAT, with
/// the code the validator reports. A VALID reference corpus cannot contain
/// them, so an appearance is a defect in the corpus rather than new coverage.
///
/// Two slices rather than one list of `(name, ExclusionReason)`: the KIND is
/// the only part any code reads, and which slice an entry sits in says it. The
/// enum's `note` field was never read by anything, so eight identical
/// "strict+catch-all generic variant" literals were compiled in as write-only
/// data, which is exactly the prose-as-data this module's own doc argues
/// against.
const INVALID_BY_CONSTRUCTION: &[(&str, &str)] = &[
    ("blank_line", "E747"),
    ("illegal_curly_quote", "E256"),
    // A word carrying two `@` runs (`hello@@c`). The grammar was widened to
    // form the word so the validator can NAME the defect, rather than letting
    // the utterance fall to ERROR-node recovery and a generic E316.
    ("repeated_form_marker", "E203"),
    ("sep_trailing_space", "E758"),
];

/// Valid CHAT that the reference corpus does not happen to contain. An
/// appearance is good news: delete the entry so the coverage number counts it.
///
/// Checked in that direction too. Seven entries were deleted on the day this
/// check was written because the corpus had come to exercise them and nothing
/// had ever asked, which had been silently understating the coverage figure.
const NOT_YET_IN_CORPUS: &[&str] = &[
    // Strict+catch-all pattern: the generic variant means "unrecognised value",
    // which is a validation question rather than a grammar one, so these are
    // deliberately NOT invalid-by-construction.
    "generic_id_sex",
    "generic_media_status",
    "generic_media_type",
    "generic_number",
    "generic_option_name",
    "generic_recording_quality",
    "generic_transcription",
    "strict_date",
    "strict_time",
    // Parser-recovery concession: `;` is a `non_colon_separator` choice so
    // malformed input parses gracefully, but in well-formed CHAT semicolons
    // appear only inside `age_format` tokens (2;06.), never standalone.
    "semicolon",
    // Needs a reference file with POS tags.
    "pos_tag",
    // @ID SES subcategory nodes.
    "ethnicity_value",
    "generic_id_ses",
    // Uncommon header and tier types.
    "thumbnail_header",
    "thumbnail_prefix",
    "unsupported_dependent_tier",
    "unsupported_header",
    "unsupported_header_prefix",
    "unsupported_line",
    "unsupported_tier_prefix",
];

/// A node type that must not appear in a valid corpus, and did.
///
/// Carries its own evidence. As a bare `(String, &str)` tuple the consumer had
/// to go back to a separate map for the file list and fall back to "unknown"
/// when the lookup missed, so the finding could be reported without the one
/// fact an operator needs.
pub struct InvalidNodePresent {
    pub kind: &'static str,
    pub code: &'static str,
    pub files: Vec<String>,
}

/// One entry from `node-types.json`.
#[derive(Debug, Deserialize)]
struct NodeTypeEntry {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
    /// Only the COUNT matters (a non-empty list means a supertype), so the
    /// elements are skipped rather than modelled. A `SubtypeEntry` struct
    /// existed with both fields `#[allow(dead_code)]`, which is a shape that
    /// exists only to be ignored.
    #[serde(default)]
    subtypes: Vec<serde::de::IgnoredAny>,
}

/// One run's inputs.
pub struct Request {
    pub corpus_dir: PathBuf,
    pub node_types: PathBuf,
}

impl Request {
    /// The two paths this repository's own corpus run uses.
    ///
    /// # Why this is not a `Default`
    ///
    /// It was one, and the `Default` impl reached into the filesystem to
    /// resolve the repository root. `Default::default()` cannot fail, so a
    /// wrong or missing root had nowhere to go but a `panic!` two crates away,
    /// and that panic was the reason the root resolver could not return a
    /// `Result`. Taking an already-proved [`RepoRoot`] moves the failure to the
    /// one place that can report it, and states in the signature that this
    /// request is ABOUT a particular checkout rather than about nothing.
    #[must_use]
    pub fn for_repo(root: &RepoRoot) -> Self {
        Self {
            corpus_dir: root.join("corpus").join("reference"),
            node_types: root.join("grammar").join("src").join("node-types.json"),
        }
    }
}

/// What one run established.
///
/// Counts that are derivable are DERIVED. `exercised_count` was stored beside
/// `concrete_total` and `missing`, from which it is arithmetic, and `excluded`
/// was a `Vec<String>` copy of two consts in this same module: a value proxying
/// for a richer fact, twice.
pub struct Report {
    /// Concrete named node types the corpus is REQUIRED to exercise, i.e. after
    /// exclusions. Named for what it is: the predecessor called this
    /// `concrete_total` while assigning it the post-exclusion count, so
    /// `coverage_pct` computed over a denominator its own field name denied.
    pub required: usize,
    pub missing: Vec<String>,
    pub invalid_present: Vec<InvalidNodePresent>,
    pub stale_exclusions: Vec<&'static str>,
    pub files_parsed: usize,
    pub files_with_errors: usize,
    pub supertype_count: usize,
}

impl Report {
    pub fn exercised(&self) -> usize {
        self.required - self.missing.len()
    }

    pub fn coverage_pct(&self) -> f64 {
        if self.required == 0 {
            return 100.0;
        }
        (self.exercised() as f64 / self.required as f64) * 100.0
    }

    pub fn summary(&self) -> String {
        format!(
            "concrete node types: {}/{} exercised ({:.1}%); {} excluded; \
             {} file(s) parsed, {} with ERROR nodes",
            self.exercised(),
            self.required,
            self.coverage_pct(),
            INVALID_BY_CONSTRUCTION.len() + NOT_YET_IN_CORPUS.len(),
            self.files_parsed,
            self.files_with_errors
        )
    }

    /// The operator-facing result: `Ok` is the clean summary, `Err` is what to
    /// do about it.
    ///
    /// ONE call, consumed by both the renderer and the gate, so the two cannot
    /// print different text for the same state. With `is_clean()` beside
    /// `summary()` each caller assembled its own failure text and they had
    /// already diverged: the gate said "add it to EXCLUDED with the reason"
    /// where the binary said "STALE EXCLUSION, delete it", and only one of the
    /// two applied the exclusion rules at all.
    pub fn outcome(&self) -> Result<String, String> {
        if self.missing.is_empty()
            && self.invalid_present.is_empty()
            && self.stale_exclusions.is_empty()
        {
            return Ok(self.summary());
        }

        let mut out = self.summary();
        if !self.missing.is_empty() {
            out.push_str(&format!(
                "\n\n{} concrete node type(s) are exercised by no reference file.\n\
                 Add a file that uses the construct, or, if it cannot appear in\n\
                 valid CHAT, add it to INVALID_BY_CONSTRUCTION with its code:",
                self.missing.len()
            ));
            for kind in &self.missing {
                out.push_str(&format!("\n  {kind}"));
            }
        }
        // The check that only exists because the exclusion KIND is data. A flat
        // string list can say "ignore this one" and nothing more.
        if !self.invalid_present.is_empty() {
            out.push_str(&format!(
                "\n\n{} node type(s) marked INVALID BY CONSTRUCTION appear in the\n\
                 reference corpus. This is not new coverage: the corpus has\n\
                 acquired content the validator rejects. Fix the file(s):",
                self.invalid_present.len()
            ));
            for found in &self.invalid_present {
                out.push_str(&format!(
                    "\n  {} ({}) in: {}",
                    found.kind,
                    found.code,
                    found.files.join(", ")
                ));
            }
        }
        if !self.stale_exclusions.is_empty() {
            out.push_str(&format!(
                "\n\n{} exclusion(s) name a node type the corpus now exercises.\n\
                 Good news: delete the entry from NOT_YET_IN_CORPUS, so the\n\
                 coverage number starts counting it:",
                self.stale_exclusions.len()
            ));
            for kind in &self.stale_exclusions {
                out.push_str(&format!("\n  {kind}"));
            }
        }
        Err(out)
    }
}

/// Load the node types, parse every corpus file, and compare.
///
/// # Errors
///
/// When `node-types.json` cannot be read or parsed, when the corpus holds no
/// `.cha` files (a coverage run over nothing reports 100%), or when any corpus
/// file cannot be read or parsed. The predecessor warned and continued on the
/// last of these, which silently shrinks the exercised set.
pub fn run(request: &Request) -> Result<Report, String> {
    let json = std::fs::read_to_string(&request.node_types)
        .map_err(|err| format!("failed to read {}: {err}", request.node_types.display()))?;
    let entries: Vec<NodeTypeEntry> = serde_json::from_str(&json)
        .map_err(|err| format!("failed to parse node-types.json: {err}"))?;

    let mut supertype_count = 0usize;
    let mut concrete: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        if !entry.named {
            continue;
        }
        if entry.subtypes.is_empty() {
            concrete.insert(entry.type_name);
        } else {
            supertype_count += 1;
        }
    }
    if concrete.is_empty() {
        return Err(format!(
            "no concrete named node types in {}; a coverage run over an empty \
             set reports 100% and means nothing",
            request.node_types.display()
        ));
    }

    let invalid: BTreeMap<&str, &str> = INVALID_BY_CONSTRUCTION.iter().copied().collect();
    let excused: BTreeSet<&str> = invalid
        .keys()
        .copied()
        .chain(NOT_YET_IN_CORPUS.iter().copied())
        .collect();

    let mut parser = TSParser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|err| format!("failed to set tree-sitter language: {err}"))?;

    // `&'static str` throughout: `Node::kind()` already returns one, so the
    // predecessor's `to_owned()` plus `clone()` per named-node visit allocated
    // twice for every node in the corpus and dropped both.
    let mut exercised: BTreeSet<&'static str> = BTreeSet::new();
    // Files are recorded ONLY for the invalid-by-construction types, which are
    // the only ones any message names a file for. The predecessor indexed every
    // type against every file, with a linear scan per visit to dedupe, to serve
    // a branch that fires only when the corpus is broken.
    let mut invalid_files: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut files_parsed = 0usize;
    let mut files_with_errors = 0usize;

    for entry in WalkDir::new(&request.corpus_dir) {
        let entry =
            entry.map_err(|err| format!("walking {}: {err}", request.corpus_dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("cha") {
            continue;
        }
        let source = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| format!("tree-sitter returned no tree for {}", path.display()))?;

        files_parsed += 1;
        let root = tree.root_node();
        if has_error_node(root) {
            files_with_errors += 1;
        }
        let file_name = path
            .strip_prefix(&request.corpus_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let mut seen_here: BTreeSet<&'static str> = BTreeSet::new();
        collect_node_types(root, &mut exercised, &mut seen_here);
        for kind in seen_here {
            if invalid.contains_key(kind) {
                invalid_files
                    .entry(kind)
                    .or_default()
                    .push(file_name.clone());
            }
        }
    }

    if files_parsed == 0 {
        return Err(format!(
            "no .cha files under {}; every node type would read as unexercised",
            request.corpus_dir.display()
        ));
    }

    let missing: Vec<String> = concrete
        .iter()
        .filter(|kind| !excused.contains(kind.as_str()) && !exercised.contains(kind.as_str()))
        .cloned()
        .collect();

    let invalid_present: Vec<InvalidNodePresent> = INVALID_BY_CONSTRUCTION
        .iter()
        .filter(|(kind, _)| exercised.contains(kind))
        .map(|(kind, code)| InvalidNodePresent {
            kind,
            code,
            files: invalid_files.get(kind).cloned().unwrap_or_default(),
        })
        .collect();

    let stale_exclusions: Vec<&'static str> = NOT_YET_IN_CORPUS
        .iter()
        .copied()
        .filter(|kind| exercised.contains(kind))
        .collect();

    Ok(Report {
        required: concrete
            .iter()
            .filter(|kind| !excused.contains(kind.as_str()))
            .count(),
        missing,
        invalid_present,
        stale_exclusions,
        files_parsed,
        files_with_errors,
        supertype_count,
    })
}

fn has_error_node(node: tree_sitter::Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(has_error_node)
}

/// Record every named node kind in this tree, both globally and per file.
fn collect_node_types(
    node: tree_sitter::Node,
    exercised: &mut BTreeSet<&'static str>,
    seen_here: &mut BTreeSet<&'static str>,
) {
    if node.is_named() {
        exercised.insert(node.kind());
        seen_here.insert(node.kind());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, exercised, seen_here);
    }
}

#[cfg(test)]
mod tests {
    use super::{INVALID_BY_CONSTRUCTION, NOT_YET_IN_CORPUS};
    use crate::repo_paths::RepoRoot;
    use talkbank_spec_vocabulary::SpecErrorCode;

    /// SURVIVES: a roundtrip between two separate owners. The table names codes
    /// as strings and the registry owns which codes exist; no type of this
    /// crate's spans both, because `generators` is what GENERATES `ErrorCode`
    /// and so cannot depend on it.
    ///
    /// Nothing checked this before. The code is read only when an
    /// invalid-by-construction node turns up in the reference corpus, so a code
    /// naming nothing would have stayed silent until the one run that had a
    /// real corpus defect to report, and then named nothing in the report.
    #[test]
    fn every_invalid_by_construction_code_is_registered() -> Result<(), String> {
        let root = RepoRoot::resolve(None).map_err(|why| why.to_string())?;
        let registry = root.code_registry().map_err(|why| why.to_string())?;
        for (kind, code) in INVALID_BY_CONSTRUCTION {
            let parsed = SpecErrorCode::parse(code)
                .ok_or_else(|| format!("{kind}: `{code}` is not a well-formed code token"))?;
            registry
                .resolve(&parsed)
                .map_err(|why| format!("{kind}: {why}"))?;
        }
        Ok(())
    }

    /// SURVIVES: policy, in the same sense as the coverage gate itself. The two
    /// slices mean different things and get opposite reverse checks, so a node
    /// listed in both would be excused twice and checked inconsistently: once
    /// as "must not appear" and once as "should appear eventually".
    #[test]
    fn no_node_type_is_excused_twice() {
        for (kind, _) in INVALID_BY_CONSTRUCTION {
            assert!(
                !NOT_YET_IN_CORPUS.contains(kind),
                "{kind} is listed as both invalid-by-construction and not-yet-in-corpus"
            );
        }
    }
}
