//! `chatter debug retag-language`, corpus-wide correction of a mislabelled code.

use std::path::PathBuf;

use talkbank_model::model::LanguageCode;
use talkbank_transform::retag_language::{RetagRefusal, retag_language};

use super::*;

/// Retag every occurrence of one language code as another, in place.
///
/// Implements `chatter debug retag-language --from X --to Y`. A code is named by
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
pub fn run_retag_language(paths: &[PathBuf], from: &str, to: &str) {
    let from = parse_code(from);
    let to = parse_code(to);
    if from == to {
        die("--from and --to are the same code; nothing to retag");
    }

    let files = collect_cha_files(paths);
    if files.is_empty() {
        die("no .cha files found in the provided paths");
    }

    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));
    let (mut files_changed, mut declarations, mut scopes, mut markers) = (0usize, 0, 0, 0);
    let mut refused: Vec<PathBuf> = Vec::new();

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path) else {
            continue;
        };

        let stats = match retag_language(open.model_mut(), &from, &to) {
            Ok(stats) => stats,
            Err(RetagRefusal::NamesCodeInSpan) => {
                refused.push(open.path().to_path_buf());
                continue;
            }
        };
        if stats.is_empty() {
            continue;
        }
        if let Committed::Wrote = open.commit(Commit::Write) {
            files_changed += 1;
            declarations += stats.declarations;
            scopes += stats.utterance_scopes;
            markers += stats.word_markers;
        }
    }

    if files_changed == 0 {
        println!("No {from} occurrences to retag.");
    } else {
        println!(
            "Retagged {from} -> {to} in {files_changed} file(s): \
             {declarations} declaration(s), {scopes} utterance scope(s), {markers} word marker(s)."
        );
    }

    // REPORTED, not silently skipped: a refusal is the tool declining to leave a
    // file half-retagged, and the operator has to know which files still name
    // the old code.
    if !refused.is_empty() {
        println!(
            "\nREFUSED {} file(s) naming {from} in a `<...> [@s:{from}]` span, which this \
             cannot rewrite. They are UNCHANGED and still name the old code:",
            refused.len()
        );
        for path in &refused {
            println!("  {}", path.display());
        }
    }
}

/// Parse a CLI-supplied language code, refusing anything the model rejects.
fn parse_code(code: &str) -> LanguageCode {
    LanguageCode::new(code)
        .unwrap_or_else(|e| die(&format!("not a valid language code `{code}`: {e:?}")))
}
