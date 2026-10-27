//! Code lens, utterance counts per speaker above `@Participants`.
//!
//! Displays a lens like "CHI: 42 utterances" above the `@Participants` header,
//! giving annotators an at-a-glance summary of transcript composition.

use crate::backend::diagnostics::DocumentAnalysis;
use crate::backend::utils::LineIndex;
use talkbank_model::model::Header;
use tower_lsp::lsp_types::*;

/// Generates code lenses showing utterance count per speaker above @Participants.
pub(crate) fn code_lens(analysis: &DocumentAnalysis) -> Option<Vec<CodeLens>> {
    let chat_file = analysis.file();
    // Count utterances per speaker.
    //
    // Iterate the DECLARED roster, not `chat_file.participants`: that map is
    // the `@Participants`-to-`@ID` join, so a speaker declared without an
    // `@ID` is absent from it and would get no lens at all, even while
    // speaking. In an editor that is exactly the file being worked on.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for declared in chat_file.declared_speakers() {
        let speaker_str = declared.code().as_str();
        let count = chat_file
            .utterances()
            .filter(|u| u.main.speaker.as_str() == speaker_str)
            .count();
        counts.push((speaker_str.to_string(), count));
    }

    if counts.is_empty() {
        return None;
    }

    // Find the @Participants header line.
    let location = chat_file
        .headers_with_locations()
        .find_map(|(header, location)| {
            matches!(header, Header::Participants { .. }).then_some(location)
        })?;
    let span = location.span()?;
    let position = LineIndex::new(analysis.source()).offset_to_position(span.start());
    let range = Range {
        start: position,
        end: position,
    };

    // Build one lens per speaker with the count.
    let mut lenses = Vec::new();
    for (speaker, count) in &counts {
        let plural = if *count == 1 {
            "utterance"
        } else {
            "utterances"
        };
        lenses.push(CodeLens {
            range,
            command: Some(Command {
                title: format!("{speaker}: {count} {plural}"),
                command: String::new(),
                arguments: None,
            }),
            data: None,
        });
    }

    Some(lenses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::document_analysis;
    const BASIC: &str = include_str!("../../../../../corpus/reference/core/basic-conversation.cha");

    #[test]
    fn test_code_lens_shows_utterance_counts() {
        let input = BASIC;
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), input);
        let lenses = code_lens(&analysis);
        assert!(lenses.is_some());

        let lenses = lenses.unwrap();
        assert_eq!(lenses.len(), 2);

        // CHI has 2 utterances
        assert!(lenses.iter().any(|l| {
            l.command
                .as_ref()
                .unwrap()
                .title
                .contains("CHI: 2 utterances")
        }));
        // MOT has 2 utterances
        assert!(lenses.iter().any(|l| {
            l.command
                .as_ref()
                .unwrap()
                .title
                .contains("MOT: 2 utterances")
        }));
    }

    #[test]
    fn test_code_lens_none_without_participants() {
        let input = BASIC.replace("@Participants:\tCHI Child, MOT Mother\n", "");
        let analysis = document_analysis(&Url::parse("file:///reference.cha").unwrap(), &input);
        let lenses = code_lens(&analysis);
        assert!(lenses.is_none());
    }
}
