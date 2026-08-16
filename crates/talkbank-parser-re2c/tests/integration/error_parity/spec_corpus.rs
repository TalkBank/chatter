//! Reading `spec/errors` into cases that can be measured.
//!
//! Split out of `error_parity.rs` when that file passed the workspace's 800
//! line hard limit. Everything here is about getting from markdown on disk to
//! a testable [`SpecCase`], including deciding which specs are in scope at all.

use std::collections::BTreeSet;

use talkbank_model::ErrorCode;
use talkbank_parser_tests::error_specs::{self, SpecStatus};

use super::model::{Expected, SpecLabel};

// ---------------------------------------------------------------------------
// Reading the spec suite
// ---------------------------------------------------------------------------

/// One `chat` block from a spec file, before it is known to be testable.
struct SpecBlock {
    pub(super) input: String,
    /// `None` when neither a declaration nor the filename yields codes.
    pub(super) expected: Option<Expected>,
}

/// One example from a spec file: the input, and what it must produce.
pub(super) struct SpecCase {
    pub(super) label: SpecLabel,
    pub(super) input: String,
    pub(super) expected: Expected,
}

/// Everything `spec/errors` yielded, INCLUDING what could not be classified.
///
/// The unclassifiable blocks are returned rather than dropped. The previous
/// reader skipped them with a bare `continue`, so a spec that stopped declaring
/// its codes would quietly leave the denominator and improve the percentage.
pub(super) struct SpecCorpus {
    /// How many `.md` files the directory held, recorded at the point it is
    /// known rather than reconstructed downstream. The first cut of the summary
    /// line printed `cases + unclassifiable` under the word "file(s)", which is
    /// a count of one thing wearing the name of another.
    pub(super) files_scanned: usize,
    pub(super) cases: Vec<SpecCase>,
    /// How many files were skipped as `not_implemented`. A count, because a
    /// count is all anyone ever read: it had been a `Vec<SpecLabel>` whose only
    /// consumer was `.len()`, so 43 labels were built per run to produce a
    /// number, and `SpecLabel::whole_file` existed solely to build them.
    pub(super) not_implemented: usize,
    /// `chat` blocks with no expectation to test them against.
    pub(super) unclassifiable: Vec<SpecLabel>,
}

/// Read every example out of one spec file.
///
/// The expectation is the LAST `**Expected Error Codes**:` line before the
/// block, which is exact: these files are generated and always declare the
/// codes immediately above their example. The previous version searched a
/// 200-byte window running PAST the end of the block, so a spec whose NEXT
/// example declared different codes could contribute them to this one.
///
/// Falls back to the code in the filename (`E375_....md` -> `E375`), which the
/// spec generator guarantees. A name matching no declared variant is an error
/// rather than a silent coercion: `ErrorCode::new` maps anything unrecognised
/// to the unknown-code sentinel, which would turn a typo into an expectation
/// nothing can ever satisfy, on a case that would then look merely unmet.
fn parse_spec(content: &str, filename: &str) -> Result<Vec<SpecBlock>, String> {
    // One owner for the first-underscore rule and the reason behind it: a
    // hypothetical `E21` must not claim `E210_auto.md`.
    let filename_code = error_specs::code_of(filename);

    let mut blocks = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = content[search_from..].find("```chat\n") {
        let block_start = search_from + offset;
        let input_start = block_start + "```chat\n".len();
        let Some(block_end) = content[input_start..].find("\n```") else {
            break;
        };
        let input_end = input_start + block_end;

        // Declarations between the previous block and this one; the last wins.
        let mut declared: Option<Expected> = None;
        for line in content[search_from..block_start].lines() {
            let Some(after) = error_specs::expected_codes_declaration(line) else {
                continue;
            };
            let mut codes = BTreeSet::new();
            for token in after.split([',', ' ']).map(str::trim) {
                match token {
                    "" => continue,
                    token => {
                        let code = ErrorCode::parse_exact(token).ok_or_else(|| {
                            format!(
                                "{filename}: expected-codes line names an unknown code {token:?}"
                            )
                        })?;
                        codes.insert(code);
                    }
                }
            }
            declared = Expected::new(codes).or(declared);
        }

        blocks.push(SpecBlock {
            input: content[input_start..input_end].to_owned(),
            expected: declared.or_else(|| {
                filename_code.and_then(|code| Expected::new(std::iter::once(code).collect()))
            }),
        });

        search_from = input_end + "\n```".len();
    }
    Ok(blocks)
}

/// Whether a spec's examples are measured, over the SHARED
/// [`talkbank_parser_tests::error_specs::SpecStatus`] vocabulary.
///
/// This module used to declare its own five-state copy of that enum, parsed by
/// its own line scan, which was one of four readers of the same markdown
/// format. The vocabulary is shared now; the POLICY below is not, because the
/// three readers legitimately disagree about it.
///
/// Only `NotImplemented` is skipped here. `spec/runtime-tools` also skips
/// `Deprecated` and `UnreachableFromChat`, so three specs are measured here
/// that it skips (`E210_auto.md`, `E213_auto.md`,
/// `E768_media_filename_not_representable.md`). Checked before leaving the
/// difference in place: none of the three diverges, so none is in the baseline
/// and none adds ratchet noise.
fn measurable(status: SpecStatus) -> Measure {
    match status {
        SpecStatus::NotImplemented => Measure::Skip,
        SpecStatus::Implemented
        | SpecStatus::Deprecated
        | SpecStatus::UnreachableFromChat
        | SpecStatus::Undeclared => Measure::Run,
    }
}

/// The decision itself, rather than a `bool` that the caller has to remember
/// reads "true means measure it". No bool crosses a function boundary in this
/// module, and this is the one that tried to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Measure {
    Run,
    Skip,
}

/// Load every spec case, in filename order so a run is reproducible.
pub(super) fn load_spec_corpus() -> Result<SpecCorpus, String> {
    let spec_dir = talkbank_parser_tests::repo_paths::workspace_root().join("spec/errors");
    let specs = error_specs::load(&spec_dir)?;

    let mut corpus = SpecCorpus {
        files_scanned: specs.len(),
        cases: Vec::new(),
        not_implemented: 0,
        unclassifiable: Vec::new(),
    };

    for spec in &specs {
        let filename = spec.filename.clone();
        match measurable(spec.status()?) {
            Measure::Skip => {
                corpus.not_implemented += 1;
                continue;
            }
            Measure::Run => {}
        }

        let blocks = parse_spec(&spec.content, &filename)?;
        let in_file = blocks.len();
        for (index, block) in blocks.into_iter().enumerate() {
            let label = SpecLabel::new(&filename, index, in_file);
            match block.expected {
                Some(expected) => corpus.cases.push(SpecCase {
                    label,
                    input: block.input,
                    expected,
                }),
                None => corpus.unclassifiable.push(label),
            }
        }
    }

    match corpus.cases.first() {
        // An empty suite is a broken measurement, not a suite with nothing in
        // it, and the two read identically in every downstream count.
        None => Err(format!("no spec cases found under {}", spec_dir.display())),
        Some(_) => Ok(corpus),
    }
}
