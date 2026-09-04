//! Assemble utterance lines from pre-formatted CHAT main-tier text.
//!
//! Each [`UtteranceDesc`] carries a CHAT utterance as text; this module parses
//! it through the tree-sitter parser (so the result is real, validated model
//! structure, never hand-built) and applies an optional per-utterance language
//! override. The batchalign word-level path (timed ASR tokens, retrace runs,
//! `%wor` generation) is not part of this general builder.

use talkbank_model::model::{LanguageCode, Line};
use talkbank_parser::TreeSitterParser;

use super::UtteranceDesc;

pub(super) fn build_utterance_lines(
    utterances: &[UtteranceDesc],
    parser: &TreeSitterParser,
    primary_lang: &LanguageCode,
) -> Result<Vec<Line>, String> {
    let mut lines = Vec::with_capacity(utterances.len());

    for utterance in utterances {
        let built = build_text_utterance(
            parser,
            &utterance.speaker,
            &utterance.text,
            utterance.start_ms,
            utterance.end_ms,
        )?;

        if let Some(mut line) = built {
            apply_utterance_language_override(&mut line, utterance.lang.as_deref(), primary_lang)?;
            if let Line::Utterance(ref mut built) = line
                && let Some(comment) = &utterance.comment
            {
                built
                    .dependent_tiers
                    .push(talkbank_model::model::DependentTier::Com(comment.clone()).into());
            }
            lines.push(line);
        } else if utterance.comment.is_some() {
            return Err("an utterance comment requires main-tier content".to_owned());
        }
    }

    Ok(lines)
}

fn apply_utterance_language_override(
    line: &mut Line,
    utterance_lang: Option<&str>,
    primary_lang: &LanguageCode,
) -> Result<(), String> {
    if let Some(utterance_lang) = utterance_lang
        && utterance_lang != primary_lang.as_str()
        && let Line::Utterance(utterance) = line
    {
        let code = LanguageCode::new(utterance_lang)
            .map_err(|e| format!("invalid utterance language code {utterance_lang:?}: {e}"))?;
        utterance.main.content.language_code = Some(code);
    }
    Ok(())
}

/// Build a text-level utterance by parsing through tree-sitter.
///
/// Constructs a minimal valid CHAT document around the input text and parses
/// it with `parse_strict()`. The mini-document wrapper is necessary because
/// tree-sitter requires complete document context (headers, `@Begin`, `@End`)
/// to parse a single utterance correctly. This is the general public entry
/// path: a caller provides a pre-formatted CHAT utterance string and gets back
/// real, validated model structure.
fn build_text_utterance(
    parser: &TreeSitterParser,
    speaker: &str,
    text: &str,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
) -> Result<Option<Line>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }

    let bullet_str = match (start_ms, end_ms) {
        (Some(start), Some(end)) => format!(" \x15{start}_{end}\x15"),
        _ => String::new(),
    };

    // THE FRAGMENT PARSER, not a fabricated document.
    //
    // This used to `format!` a whole `@UTF8`/`@Begin`/`@Languages`/
    // `@Participants`/`@ID`/`@End` document around the utterance, parse it
    // strictly, then walk the result to dig the one `Line::Utterance` back out.
    // The scaffolding carried two invented values of its own along the way, an
    // `unwrap_or("eng")` language and a literal `corpus_name`, both discarded
    // after they had served to make the fake document parse.
    //
    // `parse_utterance` is the entry point for exactly this: a main tier line.
    let line = format!("*{speaker}:\t{text}{bullet_str}");
    let utterance = parser
        .parse_utterance(&line)
        .map_err(|error| format!("Failed to parse utterance for speaker {speaker}: {error}"))?;
    Ok(Some(Line::Utterance(Box::new(utterance))))
}
