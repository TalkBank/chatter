//! `chatter experimental transform retag-language`, corpus-wide correction of a mislabelled code.

use std::path::PathBuf;

use talkbank_model::model::LanguageCode;
use talkbank_transform::retag_language::{RetagRefusal, retag_language};

use super::*;

/// Retag every occurrence of one language code as another, in place.
///
/// Implements `chatter experimental transform retag-language --from X --to Y`. A code is named by
/// `@Languages`, by `[- code]` utterance scopes and by `word@s:code` markers,
/// and all of them move together; a file naming it with a `<a b> [@s:code]`
/// span is REFUSED rather than partially retagged, because moving three
/// notations out of four produces a file that contradicts itself.
///
/// `--to` is deduplicated in `@Languages`: retagging a code the file already
/// declares removes the wrong entry rather than producing a duplicate.
///
/// A file that cannot be opened for in-place editing is reported and
/// skipped, so one bad file does not kill a multi-file run; see
/// [`InPlace::open`] for what that covers.
pub fn run_retag_language(
    paths: &[PathBuf],
    from: &str,
    to: &str,
    commit: Commit,
) -> crate::exit_codes::CommandResult {
    let from = parse_code(from)?;
    let to = parse_code(to)?;
    if from == to {
        return Err(die("--from and --to are the same code; nothing to retag"));
    }

    let (files, parser) = super::transcripts_and_parser(paths)?;
    let (mut files_changed, mut declarations, mut scopes, mut markers) = (0usize, 0, 0, 0);
    let mut refused: Vec<PathBuf> = Vec::new();
    let mut declaration_refusals = 0usize;

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path.clone()) else {
            refused.push(path);
            continue;
        };

        let stats = match retag_language(open.model_mut(), &from, &to) {
            Ok(stats) => stats,
            Err(RetagRefusal::Declaration(reason)) => {
                declaration_refusals += 1;
                eprintln!(
                    "Refusing language retag for {}: {reason}",
                    open.path().display()
                );
                continue;
            }
            Err(RetagRefusal::NamesCodeInSpan) => {
                refused.push(open.path().to_path_buf());
                continue;
            }
        };
        if stats.is_empty() {
            continue;
        }
        let path = open.path().to_owned();
        match open.commit(commit) {
            Ok(Committed::Wrote | Committed::WouldWrite) => {
                files_changed += 1;
                declarations += stats.declarations;
                scopes += stats.utterance_scopes;
                markers += stats.word_markers;
            }
            Ok(Committed::Unchanged) => {}
            Err(error) => {
                eprintln!("ERROR: cannot publish {}: {error}", path.display());
                refused.push(path);
            }
        }
    }

    let action = match commit {
        Commit::Write => "Retagged",
        Commit::DryRun => "Would retag",
    };
    if files_changed == 0 && declaration_refusals == 0 && refused.is_empty() {
        outln!("No {from} occurrences to retag.");
    } else {
        outln!(
            "{action} {from} -> {to} in {files_changed} file(s): \
             {declarations} declaration(s), {scopes} utterance scope(s), {markers} word marker(s)."
        );
    }

    // REPORTED, not silently skipped: a refusal is the tool declining to leave a
    // file half-retagged, and the operator has to know which files still name
    // the old code.
    if !refused.is_empty() {
        outln!(
            "\nREFUSED {} file(s) that cannot be rewritten. They are UNCHANGED and still name the old code:",
            refused.len()
        );
        for path in &refused {
            outln!("  {}", path.display());
        }
    }
    if !refused.is_empty() || declaration_refusals > 0 {
        return Err(crate::exit_codes::ReportedFailure::Input.into());
    }
    Ok(())
}

/// Parse a CLI-supplied language code, refusing anything the model rejects.
fn parse_code(code: &str) -> crate::exit_codes::CommandResult<LanguageCode> {
    LanguageCode::new(code).map_err(|e| die(&format!("not a valid language code `{code}`: {e:?}")))
}
