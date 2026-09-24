//! Assemble utterance lines from pre-formatted CHAT main-tier text.
//!
//! Each [`super::UtteranceDesc`] carries a CHAT utterance as text; this module parses
//! it through the tree-sitter parser (so the result is real, validated model
//! structure, never hand-built) and applies an optional per-utterance language
//! override. The batchalign word-level path (timed ASR tokens, retrace runs,
//! `%wor` generation) is not part of this general builder.

use talkbank_model::model::{LanguageCode, Line};

use super::UtteranceDesc;
use super::parser::BuildChatContext;

/// A declared timing is either absent or complete; a half-pair never renders.
enum TextTiming {
    Absent,
    Complete { start: u64, end: u64 },
}

/// Admission keeps supplied timing from vanishing with empty text.
enum TextInput<'a> {
    Empty,
    Present {
        speaker: &'a str,
        text: &'a str,
        timing: TextTiming,
    },
}

impl<'a> TextInput<'a> {
    fn admit(row: &'a UtteranceDesc) -> Result<Self, String> {
        let timing = match (row.start_ms, row.end_ms) {
            (None, None) => TextTiming::Absent,
            (Some(start), Some(end)) => TextTiming::Complete { start, end },
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "utterance timing for speaker {} requires both start_ms and end_ms",
                    row.speaker,
                ));
            }
        };
        let text = row.text.trim();
        if text.is_empty() {
            return match timing {
                TextTiming::Absent => Ok(Self::Empty),
                TextTiming::Complete { .. } => Err(format!(
                    "utterance timing for speaker {} requires main-tier content",
                    row.speaker,
                )),
            };
        }
        Ok(Self::Present {
            speaker: &row.speaker,
            text,
            timing,
        })
    }
}

pub(super) fn build_utterance_lines(context: &BuildChatContext<'_>) -> Result<Vec<Line>, String> {
    let utterances = &context.description().utterances;
    let mut lines = Vec::with_capacity(utterances.len());

    for utterance in utterances {
        let built = build_text_utterance(context, TextInput::admit(utterance)?)?;

        if let Some(mut line) = built {
            apply_utterance_language_override(
                &mut line,
                utterance.lang.as_deref(),
                context.primary_lang(),
            )?;
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
/// Parses a main-tier fragment through the description-bound semantic context.
/// File-level options must influence the model before it leaves the builder.
fn build_text_utterance(
    context: &BuildChatContext<'_>,
    input: TextInput<'_>,
) -> Result<Option<Line>, String> {
    let TextInput::Present {
        speaker,
        text,
        timing,
    } = input
    else {
        return Ok(None);
    };

    let bullet_str = match timing {
        TextTiming::Complete { start, end } => format!(" \x15{start}_{end}\x15"),
        TextTiming::Absent => String::new(),
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
    let utterance = context
        .parse_utterance(&line)
        .map_err(|error| format!("Failed to parse utterance for speaker {speaker}: {error}"))?;
    Ok(Some(Line::Utterance(Box::new(utterance))))
}
