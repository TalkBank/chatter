//! `chatter experimental transform language-runs`, utterance-level language-switch rewrite.

use std::path::PathBuf;

use super::*;

/// Rewrite whole-utterance `@s` runs into utterance precodes in place.
///
/// Implements `chatter experimental transform language-runs`. Qualifying utterances are rewritten as
/// `[- LANG] ...`, matching per-word language markers are removed, and missing
/// explicit `@s:LANG` codes are appended to `@Languages`. Files with no
/// qualifying rewrites or language-header repairs are left untouched.
///
/// A file that cannot be opened for in-place editing is reported and
/// skipped, so one bad file does not kill a multi-file run; see
/// [`InPlace::open`] for what that covers.
pub fn run_fix_s(paths: &[PathBuf], commit: Commit) -> crate::exit_codes::CommandResult {
    let (files, parser) = super::transcripts_and_parser(paths)?;
    let mut refused_files = 0usize;
    let mut rewritten_files = 0usize;
    let mut rewritten_utterances = 0usize;
    let mut appended_language_codes = 0usize;

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path) else {
            refused_files += 1;
            continue;
        };
        let stats = match talkbank_transform::fix_s::rewrite_whole_utterance_language_switches(
            open.model_mut(),
        ) {
            Ok(stats) => stats,
            Err(reason) => {
                refused_files += 1;
                eprintln!(
                    "Refusing language rewrite for {}: {reason}",
                    open.path().display()
                );
                continue;
            }
        };
        if stats.is_empty() {
            continue;
        }
        // Counted from what COMMIT did, not from what the edit asked for: an
        // edit whose result equals the source writes nothing, and reporting it
        // as a rewritten file would be a count of intentions.
        let display = open.path().display().to_string();
        match open.commit(commit) {
            Ok(Committed::Wrote | Committed::WouldWrite) => {
                rewritten_files += 1;
                rewritten_utterances += stats.rewritten_utterances;
                appended_language_codes += stats.appended_language_codes;
            }
            Ok(Committed::Unchanged) => {}
            Err(error) => {
                refused_files += 1;
                eprintln!("ERROR: cannot publish {display}: {error}");
            }
        }
    }

    let action = match commit {
        Commit::Write => "Rewrote",
        Commit::DryRun => "Would rewrite",
    };
    if rewritten_files == 0 && refused_files == 0 {
        outln!("No fix-s rewrites or @Languages repairs needed.");
    } else {
        outln!(
            "{action} {rewritten_files} file(s); updated {rewritten_utterances} utterance(s) and appended {appended_language_codes} @Languages code(s)."
        );
    }
    if refused_files > 0 {
        outln!("Refused {refused_files} file(s); those files are unchanged.");
        return Err(crate::exit_codes::ReportedFailure::Input.into());
    }
    Ok(())
}
