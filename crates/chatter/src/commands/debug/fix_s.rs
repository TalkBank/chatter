//! `chatter debug fix-s`, utterance-level language-switch rewrite.

use std::path::PathBuf;

use talkbank_model::WriteChat;

use super::*;

/// Rewrite whole-utterance `@s` runs into utterance precodes in place.
///
/// Implements `chatter debug fix-s`. Qualifying utterances are rewritten as
/// `[- LANG] ...`, matching per-word language markers are removed, and missing
/// explicit `@s:LANG` codes are appended to `@Languages`. Files with no
/// qualifying rewrites or language-header repairs are left untouched.
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
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", path.display())));
        let mut parsed = parser
            .parse_chat_file(&source)
            .unwrap_or_else(|e| die(&format!("parse failed for {}: {e:?}", path.display())));
        let stats =
            talkbank_transform::fix_s::rewrite_whole_utterance_language_switches(&mut parsed);
        if stats.is_empty() {
            continue;
        }

        let rewritten = parsed.to_chat_string();
        if rewritten == source {
            continue;
        }

        std::fs::write(&path, &rewritten)
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
        rewritten_files += 1;
        rewritten_utterances += stats.rewritten_utterances;
        appended_language_codes += stats.appended_language_codes;
    }

    if rewritten_files == 0 {
        println!("No fix-s rewrites or @Languages repairs needed.");
    } else {
        println!(
            "Rewrote {rewritten_files} file(s); updated {rewritten_utterances} utterance(s) and appended {appended_language_codes} @Languages code(s)."
        );
    }
}
