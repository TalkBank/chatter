//! Clean command, show raw vs cleaned text for utterance words.
//!
//! Debugging aid for understanding what [`Word::cleaned_text()`] produces. CHAT words
//! carry markup (e.g. `word@s:eng`, `wo(r)d`, `word [*]`) that NLP pipelines strip
//! before sending text to Stanza or other processors. This command surfaces the
//! before/after for every word in a file so developers can diagnose tokenisation
//! mismatches.
//!
//! Use `--diff-only` to show only words where raw != cleaned. The output is available
//! as plain text (tabular) or JSON for programmatic consumption.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::fs;
use std::path::PathBuf;

use tracing::{Level, debug, info, span, warn};

use crate::cli::OutputFormat;
use talkbank_model::LineMap;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{MainTier, Word};

/// A single word entry with raw and cleaned forms.
#[derive(serde::Serialize)]
struct WordEntry {
    raw: String,
    cleaned: String,
}

/// An utterance's worth of word entries.
#[derive(serde::Serialize)]
struct UtteranceEntry {
    speaker: String,
    line: usize,
    words: Vec<WordEntry>,
}

/// Show cleaned text for each word in a CHAT file's utterances.
///
/// The command is intended for debugging the talkbank_utils pipeline: it highlights what `Word::cleaned_text()`
/// produces before data goes to NLP gateways such as Stanza or the talkbank-alignment stack. The headers referenced
/// in the CHAT manual’s Main Tier and Dependent Tier sections define which tokens are considered words and how they
/// should be iterated, so we reuse the validated `ChatFile` and line map to keep the output aligned with that
/// structure when emitting JSON or text diffs.
pub fn clean_file(input: &PathBuf, diff_only: bool, format: OutputFormat) {
    let _span = span!(Level::INFO, "clean_file", input = %input.display()).entered();
    info!("Showing cleaned text for utterance words");

    let content = match fs::read_to_string(input) {
        Ok(c) => {
            debug!("Read {} bytes from file", c.len());
            c
        }
        Err(e) => {
            warn!("Failed to read file: {}", e);
            eprintln!("Error reading file {:?}: {}", input, e);
            std::process::exit(1);
        }
    };

    let options = talkbank_model::ParseValidateOptions::default();
    let chat_file = match talkbank_transform::parse_and_validate(&content, options) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error parsing {:?}: {}", input, e);
            std::process::exit(1);
        }
    };

    let line_map = LineMap::new(&content);

    let mut entries: Vec<UtteranceEntry> = Vec::new();

    for utt in chat_file.utterances() {
        let main = &utt.main;
        // 0-indexed → 1-indexed for display
        let line_num = line_map.line_of(main.span.start) + 1;
        let speaker = main.speaker.to_string();

        let mut words = Vec::new();
        collect_words_from_main(main, &mut words);

        if diff_only {
            words.retain(|w| w.raw != w.cleaned);
        }

        if !diff_only || !words.is_empty() {
            entries.push(UtteranceEntry {
                speaker,
                line: line_num,
                words,
            });
        }
    }

    match format {
        OutputFormat::Json => {
            // serde_json::to_string_pretty is infallible for types
            // that derive `Serialize` cleanly (no custom serializers
            // can fail). `entries` is `Vec<CleanEntry>` with derived
            // Serialize.
            #[allow(clippy::expect_used)]
            let json = serde_json::to_string_pretty(&entries).expect("JSON serialization failed");
            println!("{json}");
        }
        OutputFormat::Text => {
            for (i, entry) in entries.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                println!("*{}: (line {})", entry.speaker, entry.line);
                for w in &entry.words {
                    println!("  {:<24}{}", w.raw, w.cleaned);
                }
            }
        }
    }
}

/// Collect word entries from a main tier's content.
///
/// Walks the validated `MainTier` content and delegates to `collect_words_from_utterance_content`,
/// ensuring that punctuation, pauses, and other non-word constituents are skipped per the CHAT manual’s
/// Main Tier blueprint.
fn collect_words_from_main(main: &MainTier, out: &mut Vec<WordEntry>) {
    // `walk_words` with no tier domain is exactly this walk: it recurses into
    // every group kind, quotations and both retrace forms, and skips pauses,
    // events, markers and overlap points. Two hand-rolled matches used to say
    // the same thing in 99 lines, and each new content variant had to be added
    // to both of them as well as to the walker.
    //
    // Separators are yielded by the walker and dropped here, because `clean`
    // surfaces only canonical words for alignment and NLP.
    walk_words(&main.content.content, None, &mut |item| match item {
        WordItem::Word(word) => push_word(word, out),
        WordItem::ReplacedWord(replaced) => push_word(&replaced.word, out),
        WordItem::Separator(_) => {}
    });
}

fn push_word(word: &Word, out: &mut Vec<WordEntry>) {
    out.push(WordEntry {
        raw: word.raw_text().to_string(),
        cleaned: word.cleaned_text().to_string(),
    });
}
