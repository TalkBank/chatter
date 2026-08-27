//! `chatter debug fix-s`, utterance-level language-switch rewrite.

use std::path::PathBuf;

use super::*;

/// Rewrite whole-utterance `@s` runs into utterance precodes in place.
///
/// Implements `chatter debug fix-s`. Qualifying utterances are rewritten as
/// `[- LANG] ...`, matching per-word language markers are removed, and missing
/// explicit `@s:LANG` codes are appended to `@Languages`. Files with no
/// qualifying rewrites or language-header repairs are left untouched.
///
/// A file that cannot be opened for in-place editing is reported and
/// skipped, so one bad file does not kill a multi-file run; see
/// [`InPlace::open`] for what that covers.
pub fn run_fix_s(paths: &[PathBuf]) {
    let files = collect_cha_files(paths);
    if files.is_empty() {
        die("no .cha files found in the provided paths");
    }

    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));
    let mut rewritten_files = 0usize;
    let mut rewritten_utterances = 0usize;
    let mut appended_language_codes = 0usize;

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path) else {
            continue;
        };
        let stats =
            talkbank_transform::fix_s::rewrite_whole_utterance_language_switches(open.model_mut());
        if stats.is_empty() {
            continue;
        }
        // Counted from what COMMIT did, not from what the edit asked for: an
        // edit whose result equals the source writes nothing, and reporting it
        // as a rewritten file would be a count of intentions.
        if let Committed::Wrote = open.commit(Commit::Write) {
            rewritten_files += 1;
            rewritten_utterances += stats.rewritten_utterances;
            appended_language_codes += stats.appended_language_codes;
        }
    }

    if rewritten_files == 0 {
        println!("No fix-s rewrites or @Languages repairs needed.");
    } else {
        println!(
            "Rewrote {rewritten_files} file(s); updated {rewritten_utterances} utterance(s) and appended {appended_language_codes} @Languages code(s)."
        );
    }
}
