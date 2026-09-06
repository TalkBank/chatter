//! Integration coverage for the shared lexical checks.

#[cfg(test)]
mod tests {
    use crate::TreeSitterParser;
    use talkbank_model::{ErrorCollector, ParseError};

    fn control_characters(input: &str) -> std::vec::IntoIter<ParseError> {
        let errors = ErrorCollector::new();
        talkbank_model::validation::report_control_characters(input, &errors);
        errors.into_vec().into_iter()
    }

    fn document(lines: &str) -> String {
        format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n{lines}\n@End\n"
        )
    }

    fn codes_of(input: &str) -> Vec<(String, usize)> {
        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(input, &errors);
        errors
            .into_vec()
            .into_iter()
            .map(|e| (e.code.as_str().to_owned(), e.location.span.start as usize))
            .collect()
    }

    /// The delimiters CHAT itself uses are not control characters for this
    /// rule: a bullet, CRLF, and the CA underline pairs yield nothing.
    #[test]
    fn chat_delimiters_and_bullets_are_permitted() {
        let input =
            document("*CHI:\twe \u{2}\u{1}need\u{2}\u{2} to . \u{15}0_1000\u{15}\r\n%com:\tfine");
        assert_eq!(control_characters(&input).count(), 0);
        assert!(
            !codes_of(&input).iter().any(|(c, _)| c == "E315"),
            "{:?}",
            codes_of(&input)
        );
    }

    /// A control character is reported once, at its own offset, wherever it
    /// sits: a `%com` line and a header value parse as free text and used to
    /// pass silently.
    #[test]
    fn a_control_character_in_free_text_is_reported_at_its_offset() {
        let input = document("*CHI:\thello .\n%com:\tnote\u{1}here");
        let offset = input.find('\u{1}').expect("present");
        let found: Vec<_> = control_characters(&input).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location.span.start as usize, offset);
        assert_eq!(found[0].location.span.end as usize, offset + 1);
        let e315: Vec<usize> = codes_of(&input)
            .into_iter()
            .filter(|(c, _)| c == "E315")
            .map(|(_, at)| at)
            .collect();
        assert_eq!(e315, [offset]);

        let header = document("@Comment:\tnote\u{7f}here\n*CHI:\thello .");
        let at = header.find('\u{7f}').expect("present");
        assert_eq!(
            codes_of(&header)
                .into_iter()
                .filter(|(c, _)| c == "E315")
                .map(|(_, a)| a)
                .collect::<Vec<_>>(),
            [at]
        );
    }

    /// An attribute lead byte with any other code (CLAN italics) is not an
    /// underline marker: both characters are reported, each at its offset,
    /// and a lone lead byte is reported too.
    #[test]
    fn a_non_underline_attribute_pair_is_reported_character_by_character() {
        let input = document("*CHI:\t\u{2}\u{3}hey\u{2}\u{4} you .");
        let offsets: Vec<usize> = control_characters(&input)
            .map(|error| error.location.span.start as usize)
            .collect();
        let lead = input.find('\u{2}').expect("present");
        assert_eq!(offsets, [lead, lead + 1, lead + 5, lead + 6]);
        let lone = document("*CHI:\they\u{2} you .");
        assert_eq!(control_characters(&lone).count(), 1);
    }

    /// Inside a word the character also breaks the parse; E315 is still
    /// reported exactly once, by this rule, not by an ERROR-node classifier.
    #[test]
    fn a_control_character_inside_a_word_is_reported_once() {
        let input = document("*CHI:\tword\u{1}test .");
        let e315 = codes_of(&input).iter().filter(|(c, _)| c == "E315").count();
        assert_eq!(e315, 1);
    }
}
