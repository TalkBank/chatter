//! Does any main-tier word in the repository's CHAT files have an empty
//! cleaned text?
//!
//! `Word`'s initial content is one non-empty `Text`, but a word's cleaned text
//! is COMPUTED from its content, and a recovery word holding only structural
//! elements has none. The sanitizer's `%wor` display word is built from that
//! computed text through an unchecked constructor whose debug assertion is
//! the only thing that would notice. This probe measures whether any input
//! here reaches that state.
//!
//! ```text
//! cargo run -p talkbank-parser-tests --example empty_cleaned_text_probe
//! ```

use std::error::Error;
use std::path::{Path, PathBuf};

use talkbank_model::ErrorCollector;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_parser::TreeSitterParser;

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != "target" && name != "node_modules" && name != ".git"
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
        .map(walkdir::DirEntry::into_path)
        .collect();
    files.sort();
    let parser = TreeSitterParser::new()?;
    let mut words = 0usize;
    let mut empties: Vec<String> = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path)?;
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let shown = path
            .strip_prefix(&root)
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                words += 1;
                if word.cleaned_text().is_empty() {
                    empties.push(format!(
                        "{shown}: raw={:?} span={:?}",
                        word.raw_text(),
                        word.span
                    ));
                }
            });
        }
    }
    println!(
        "{} files, {words} main-tier words, {} with an empty cleaned text",
        files.len(),
        empties.len()
    );
    for e in &empties {
        println!("  {e}");
    }
    Ok(())
}
