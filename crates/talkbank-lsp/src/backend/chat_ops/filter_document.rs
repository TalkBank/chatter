//! Speaker-based document filtering for `talkbank/filterDocument`.

use std::collections::HashSet;

use talkbank_model::model::{ChatFile, Line};

use crate::backend::execute_commands::FilterDocumentRequest;
use crate::backend::state::Backend;

use super::shared::get_document_and_chat_file;
use crate::backend::utils::LineIndex;

/// Handle `talkbank/filterDocument`.
pub(crate) fn handle_filter_document(
    backend: &Backend,
    request: &FilterDocumentRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (text, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let selected: HashSet<&str> = request
        .speakers
        .iter()
        .map(|speaker| speaker.as_str())
        .collect();
    Ok(serde_json::Value::String(filter_by_speakers(
        &chat_file, &text, &selected,
    )))
}

/// Render a filtered copy of the document containing only selected speakers.
fn filter_by_speakers(chat_file: &ChatFile, text: &str, selected: &HashSet<&str>) -> String {
    let line_index = LineIndex::new(text);
    let lines: Vec<&str> = crate::backend::utils::protocol_lines(text)
        .map(|line| line.text())
        .collect();
    let mut output: Vec<String> = Vec::new();

    for model_line in &chat_file.lines {
        match model_line {
            Line::Header { location, .. } => {
                // A header with no source location has no lines to copy.
                let Some(span) = location.span() else {
                    continue;
                };
                let start_line = line_index.byte_to_line(span.start());
                let end_line = line_index.byte_to_line(span.end().saturating_sub(1));
                for line in &lines[start_line..=end_line.min(lines.len().saturating_sub(1))] {
                    output.push(line.to_string());
                }
            }
            Line::Utterance(utterance) => {
                let speaker = utterance.main.speaker.as_str();
                if !selected.contains(speaker) {
                    continue;
                }

                for hdr in &utterance.preceding_headers {
                    output.push(hdr.to_string());
                }

                // An utterance with no source location has no lines to copy.
                let Some(main_span) = utterance.main.location.span() else {
                    continue;
                };
                let main_start = line_index.byte_to_line(main_span.start());
                let block_end = utterance
                    .dependent_tiers
                    .last()
                    .and_then(|dt| dt.tier.location().span())
                    .map_or_else(
                        || line_index.byte_to_line(main_span.end().saturating_sub(1)),
                        |span| line_index.byte_to_line(span.end().saturating_sub(1)),
                    );

                for line in &lines[main_start..=block_end.min(lines.len().saturating_sub(1))] {
                    output.push(line.to_string());
                }
            }
        }
    }

    output.join("\n")
}
