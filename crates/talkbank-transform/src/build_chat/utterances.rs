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
    langs: &[LanguageCode],
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
            langs,
        )?;

        if let Some(mut line) = built {
            apply_utterance_language_override(&mut line, utterance.lang.as_deref(), primary_lang)?;
            lines.push(line);
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
    langs: &[LanguageCode],
) -> Result<Option<Line>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }

    let bullet_str = match (start_ms, end_ms) {
        (Some(start), Some(end)) => format!(" \x15{start}_{end}\x15"),
        _ => String::new(),
    };

    let lang_code = langs.first().map(LanguageCode::as_str).unwrap_or("eng");
    let mini_chat = format!(
        "@UTF8\n@Begin\n@Languages:\t{lang}\n@Participants:\t{speaker} Participant Participant\n\
         @ID:\t{lang}|corpus_name|{speaker}|||||Participant|||\n*{speaker}:\t{text}{bullet}\n@End\n",
        lang = lang_code,
        speaker = speaker,
        text = text,
        bullet = bullet_str,
    );

    let parsed = crate::parse::parse_strict(parser, &mini_chat).map_err(|error| {
        format!("Failed to parse text utterance for speaker {speaker}: {error}")
    })?;

    for parsed_line in parsed.lines.into_iter() {
        if let Line::Utterance(utterance) = parsed_line {
            return Ok(Some(Line::Utterance(utterance)));
        }
    }

    Ok(None)
}
