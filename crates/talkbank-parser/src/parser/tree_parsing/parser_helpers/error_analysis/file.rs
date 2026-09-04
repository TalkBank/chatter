//! File-level `ERROR` analysis and fallback diagnostic routing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use tree_sitter::Node;

/// Classifies a top-level `ERROR` node into a specific parse error.
pub(crate) fn analyze_error_node(node: Node, source: &str, errors: &impl ErrorSink) {
    let error_text = extract_utf8_text(node, source, errors, "file_error", "");
    let start = node.start_byte();
    let end = node.end_byte();

    if let super::dedicated::QuotationDelimiterScan::Unbalanced(finding) =
        super::dedicated::scan_quotation_delimiters(node)
    {
        errors.report(finding.into_diagnostic(source));
        return;
    }

    // Check if this is a dependent tier error (starts with %)
    if matches!(error_text.chars().next(), Some('%')) {
        // E710: Invalid %gra - non-numeric index
        if error_text.contains("%gra:") {
            errors.report(
                ParseError::new(
                    ErrorCode::UnexpectedGrammarNode,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Invalid GRA relation - non-numeric index",
                )
                .with_suggestion(
                    "GRA relation indices must be numbers (e.g., 1|2|SUBJ, not one|2|SUBJ)",
                ),
            );
            return;
        }

        // Recoverable dependent-tier parse failures:
        // keep file parsing alive and let downstream validation report semantic issues.
        let (code, message) = if error_text.contains(":\t") {
            (
                ErrorCode::InvalidDependentTier,
                format!(
                    "Could not fully parse dependent tier: {}",
                    match error_text.lines().next() {
                        Some(line) => line,
                        None => error_text,
                    }
                ),
            )
        } else {
            (
                ErrorCode::MalformedTierHeader,
                format!(
                    "Malformed dependent tier header: {}",
                    match error_text.lines().next() {
                        Some(line) => line,
                        None => error_text,
                    }
                ),
            )
        };

        errors.report(
            ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                message,
            )
            .with_suggestion(
                "Check dependent tier syntax (%tier:\\tcontent) and tier-specific format",
            ),
        );
        return;
    }

    // Check if this is a main tier error (starts with *)
    if matches!(error_text.chars().next(), Some('*')) {
        // E301: Check for empty speaker (*: with no code between * and :)
        if error_text.contains("*:") || error_text.contains("*\t") {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingMainTier,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Empty speaker code in main tier",
                )
                .with_suggestion("Add a speaker code between * and : (e.g., *CHI:)"),
            );
            return;
        }

        // E306: `*SPEAKER:` with nothing after, empty utterance.
        // Detected when the error text ends at (or just after) the colon.
        if let Some(last_colon) = error_text.rfind(':') {
            let trailing_ws = error_text
                .bytes()
                .rev()
                .take_while(|&b| b == b'\n' || b == b'\r' || b == b'\t' || b == b' ')
                .count();
            if trailing_ws + 1 >= error_text.len() - last_colon {
                errors.report(
                    ParseError::new(
                        ErrorCode::EmptyUtterance,
                        Severity::Error,
                        SourceLocation::from_offsets(start, end),
                        ErrorContext::new(source, start..end, error_text),
                        "Main tier missing content after speaker",
                    )
                    .with_suggestion(
                        "Add utterance content after the colon-tab (e.g., *CHI:\thello world .)",
                    ),
                );
                return;
            }
        }
    }

    // Check if this is a header error by looking at the content
    if matches!(error_text.chars().next(), Some('@')) {
        // Empty-header checks via `strip_prefix` so we never slice at
        // a byte index that might fall inside a multi-byte UTF-8 char.
        // (A previous `error_text[..N] == "@Name:"` pattern panicked on
        // fuzz input like `@%…˻…` where byte index 7 sat inside `˻`.)
        let empty_header_check: Option<(&str, ErrorCode, &'static str)> = None
            .or_else(|| {
                error_text
                    .strip_prefix("@Languages:")
                    .map(|rest| (rest, ErrorCode::EmptyLanguagesHeader, "@Languages"))
            })
            .or_else(|| {
                error_text
                    .strip_prefix("@Date:")
                    .map(|rest| (rest, ErrorCode::EmptyDateHeader, "@Date"))
            })
            .or_else(|| {
                error_text
                    .strip_prefix("@Media:")
                    .map(|rest| (rest, ErrorCode::EmptyMediaHeader, "@Media"))
            });
        if let Some((after_colon, code, name)) = empty_header_check
            && after_colon
                .bytes()
                .all(|b| b == b'\t' || b == b' ' || b == b'\n' || b == b'\r')
        {
            errors.report(ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                format!("{name} header cannot be empty"),
            ));
            return;
        }

        // @Page header (not a standard CHAT header but used in some files)
        if error_text.starts_with("@Page") {
            errors.report(
                ParseError::new(
                    ErrorCode::UnknownHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "@Page header is not a standard CHAT header",
                )
                .with_suggestion("@Page is a legacy header. Consider removing it."),
            );
            return;
        }

        // A header whose colon is not followed by a TAB. One rule for every
        // header: until 2026-09-03 only `@Comment:` reached E303 and every
        // other header fell to generic E316, and the message said "space"
        // whatever actually followed the colon.
        if let Some(HeaderColon {
            after:
                after @ (AfterHeaderColon::Space
                | AfterHeaderColon::Nothing
                | AfterHeaderColon::Other(_)),
            ..
        }) = HeaderColon::of(error_text)
        {
            errors.report(
                ParseError::new(
                    ErrorCode::SyntaxError,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    format!("{after} after the header colon; CHAT requires a single TAB"),
                )
                .with_suggestion("Put exactly one TAB between the header's ':' and its value"),
            );
            return;
        }

        // Check for @ID errors
        // ERROR node with @ID means tree-sitter failed to parse the structure
        // Don't try to manually parse it - just report it's malformed
        if error_text.contains("@ID:") {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidIDFormat,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Invalid @ID header format: structure could not be parsed",
                )
                .with_suggestion(
                    "@ID requires exactly 10 pipe-separated fields: @ID:\tlang|corpus|speaker|age|sex|group|SES|role|education|custom|",
                ),
            );
            return;
        }
    }

    // Duplicate @Begin
    if error_text.starts_with("@Begin") {
        errors.report(
            ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Duplicate @Begin header: only one @Begin is allowed per file",
            )
            .with_suggestion("Remove the extra @Begin header"),
        );
        return;
    }

    // Content after @End
    if error_text.starts_with("@End") {
        errors.report(
            ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Duplicate @End header or content after @End: nothing may follow the @End line",
            )
            .with_suggestion("Remove all content after @End, only one @End is allowed per file"),
        );
        return;
    }

    // Main tier containing inline annotations ([%add:]), repetition ([x N]),
    // or other content that causes the entire line to be an ERROR node.
    //
    // NOTE (2026-06-25): a leading syllable pause (`^word`) is NO LONGER handled
    // here by scanning the ERROR text. The grammar's `word_body` now accepts a
    // leading `syllable_pause`, so `*CHI:\t^banana .` parses into a structured
    // word (no file-level ERROR), and E252 (SyllablePauseNotBetweenSpokenMaterial)
    // is emitted by the typed-model validator `check_prosodic_markers` reading the
    // parsed `WordContent::SyllablePause` position. Classifying the raw text of an
    // ERROR node to guess the diagnostic is the banned anti-pattern (root CLAUDE.md
    // "CST Traversal Rules"); this diagnostic was re-homed onto structure + model.
    if error_text.starts_with('*') && error_text.contains(":\t") {
        let content_start = error_text.find(":\t").unwrap_or(0) + 2;
        let content = error_text[content_start..].trim();

        // E759: utterance content begins with a postfix annotation
        // (retrace / overlap / replacement / quotation). These codes scope
        // over the material that PRECEDES them, so a leading one has no
        // host and the parse is genuinely broken; name it instead of
        // falling through to the generic E316. Trigger set mirrors CLAN
        // CHECK error 52: a leading bracket code whose first inner
        // character is one of `/`, `<`, `>`, `:`, `"`. Legal leading
        // codes (`[- lang]` precodes, `[^ ...]`) parse normally and never
        // reach this analysis.
        if let Some(code_token) = super::dedicated::leading_postfix_annotation(content) {
            errors.report(
                ParseError::new(
                    ErrorCode::AnnotationAtUtteranceStart,
                    Severity::Error,
                    SourceLocation::from_offsets(start + content_start, end),
                    ErrorContext::new(source, start..end, error_text),
                    format!(
                        "Annotation '{code_token}' at utterance start has no content to attach to"
                    ),
                )
                .with_suggestion(
                    "Retraces, overlap markers, replacements, and quotation codes scope over the \
                     material BEFORE them; put the annotated content first, or remove the code",
                ),
            );
            return;
        }

        // [%add: ...] or similar inline dependent tier annotation
        if content.starts_with("[%") {
            errors.report(
                ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(start + content_start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Inline dependent tier annotation cannot appear at utterance start".to_string(),
                )
                .with_suggestion(
                    "Place [%add: ...] after the word it modifies, not at utterance start",
                ),
            );
            return;
        }

        // <group> [x N], repetition that fails to parse at file level
        if content.contains("[x ") || content.contains("[x\t") {
            errors.report(
                ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Could not parse utterance containing repetition count [x N]".to_string(),
                )
                .with_suggestion(
                    "Check repetition format: word [x N] or <group> [x N]. \
                     The number must follow [x with a space.",
                ),
            );
            return;
        }
    }

    // Main tier with non-ASCII speaker name (e.g., *CHIé:)
    if error_text.starts_with('*')
        && let Some(colon_pos) = error_text.find(':')
    {
        let speaker = &error_text[1..colon_pos];
        if !speaker.is_ascii() {
            errors.report(
                ParseError::new(
                    ErrorCode::SpeakerNotDefined,
                    Severity::Error,
                    SourceLocation::from_offsets(start, start + 1 + colon_pos),
                    ErrorContext::new(source, start..start + 1 + colon_pos, ""),
                    format!("Speaker '{}' contains non-ASCII characters and cannot be resolved", speaker),
                )
                .with_suggestion(
                    "Speaker codes must use only uppercase ASCII letters and digits (e.g., CHI, MOT, SP1)",
                ),
            );
            return;
        }
    }

    // Generic file-level error
    errors.report(
        ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            format!(
                "Unparsable content at file level: '{}'",
                match error_text.lines().next() {
                    Some(line) => line,
                    None => error_text,
                }
            ),
        )
        .with_suggestion("Check CHAT format specification for valid syntax at this position"),
    );
}

/// What a header line's colon is followed by.
///
/// CHAT separates a header name from its value with exactly one TAB; a space,
/// nothing at all, or any other character is the same defect, and the message
/// should say which one was found rather than guess "space".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterHeaderColon {
    Tab,
    Space,
    Nothing,
    Other(char),
}

impl std::fmt::Display for AfterHeaderColon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AfterHeaderColon::Tab => write!(f, "A TAB"),
            AfterHeaderColon::Space => write!(f, "A space"),
            AfterHeaderColon::Nothing => write!(f, "Nothing"),
            AfterHeaderColon::Other(ch) => write!(f, "{ch:?}"),
        }
    }
}

/// A header-shaped ERROR line, split at its colon.
///
/// Built only by [`HeaderColon::of`], from an ERROR node's text that starts
/// with `@`, a header name (letters and spaces, as in `@Time Duration`), and a
/// colon; `@Begin`, `@End` and `@UTF8` have no colon and are not this shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeaderColon<'a> {
    name: &'a str,
    after: AfterHeaderColon,
}

impl<'a> HeaderColon<'a> {
    fn of(error_text: &'a str) -> Option<HeaderColon<'a>> {
        let body = error_text.strip_prefix('@')?;
        let colon = body.find(':')?;
        let name = &body[..colon];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
            return None;
        }
        let after = match body[colon + 1..].chars().next() {
            None | Some('\n') | Some('\r') => AfterHeaderColon::Nothing,
            Some('\t') => AfterHeaderColon::Tab,
            Some(' ') => AfterHeaderColon::Space,
            Some(other) => AfterHeaderColon::Other(other),
        };
        Some(HeaderColon { name, after })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use talkbank_model::ErrorCollector;

    fn document(header_line: &str) -> String {
        format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n{header_line}\n*CHI:\thello .\n@End\n"
        )
    }

    fn diagnostics(input: &str) -> Vec<(String, String)> {
        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(input, &errors);
        errors
            .into_vec()
            .into_iter()
            .map(|e| (e.code.as_str().to_owned(), e.message.clone()))
            .collect()
    }

    /// The split names what followed the colon, and refuses non-header text.
    #[test]
    fn header_colon_split_names_what_follows() {
        assert_eq!(
            HeaderColon::of("@Situation: at home"),
            Some(HeaderColon {
                name: "Situation",
                after: AfterHeaderColon::Space
            })
        );
        assert_eq!(
            HeaderColon::of("@Time Duration:\t00:00-01:00").map(|h| h.after),
            Some(AfterHeaderColon::Tab)
        );
        assert_eq!(
            HeaderColon::of("@Comment:note").map(|h| h.after),
            Some(AfterHeaderColon::Other('n'))
        );
        assert_eq!(
            HeaderColon::of("@Comment:\n").map(|h| h.after),
            Some(AfterHeaderColon::Nothing)
        );
        assert_eq!(HeaderColon::of("@Begin"), None);
        assert_eq!(HeaderColon::of("*CHI: hello ."), None);
        assert_eq!(HeaderColon::of("@ID|x:"), None);
    }

    /// Every header with a space after its colon reaches E303, not only
    /// `@Comment`, and the message says what was found.
    #[test]
    fn any_header_without_a_tab_after_its_colon_is_e303() {
        for (line, expected) in [
            ("@Situation: at home", "A space after the header colon"),
            ("@Comment: a note", "A space after the header colon"),
            ("@Date: 01-JAN-2020", "A space after the header colon"),
            ("@Comment:note", "'n' after the header colon"),
        ] {
            let diags = diagnostics(&document(line));
            let e303: Vec<&String> = diags
                .iter()
                .filter(|(c, _)| c == "E303")
                .map(|(_, m)| m)
                .collect();
            assert_eq!(e303.len(), 1, "{line}: {diags:?}");
            assert!(e303[0].starts_with(expected), "{line}: {}", e303[0]);
            assert!(
                !diags.iter().any(|(c, _)| c == "E316"),
                "{line} must not also fall to E316: {diags:?}"
            );
        }
    }

    /// A tabbed header is not this rule's business.
    #[test]
    fn a_tabbed_header_is_not_e303() {
        let diags = diagnostics(&document("@Situation:\tat home"));
        assert!(!diags.iter().any(|(c, _)| c == "E303"), "{diags:?}");
    }
}
