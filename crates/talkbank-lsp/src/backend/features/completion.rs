//! Completion candidates for speakers, headers, dependent tiers, annotations, and postcodes.
//!
//! Triggered by `*` (speaker codes from `@Participants`), `%` (standard
//! dependent-tier prefixes like `%mor:`, `%gra:`, `%pho:`, etc.), `+`
//! (postcodes from the CST), `@` (header names), and `[` (bracket
//! annotations like `[//]`, `[/]`, `[: ...]`, etc.).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_parser::node_types::{
    ACT_TIER_PREFIX, ADD_TIER_PREFIX, ALT_TIER_PREFIX, COD_TIER_PREFIX, COH_TIER_PREFIX,
    COM_TIER_PREFIX, DEF_TIER_PREFIX, ENG_TIER_PREFIX, ERR_TIER_PREFIX, EXP_TIER_PREFIX,
    FAC_TIER_PREFIX, FLO_TIER_PREFIX, GLS_TIER_PREFIX, GPX_TIER_PREFIX, GRA_TIER_PREFIX,
    INT_TIER_PREFIX, MAIN_TIER, MOD_TIER_PREFIX, MOR_TIER_PREFIX, ORT_TIER_PREFIX, PAR_TIER_PREFIX,
    PHO_TIER_PREFIX, POSTCODE, SIN_TIER_PREFIX, SIT_TIER_PREFIX, SPA_TIER_PREFIX, SPEAKER,
    TIM_TIER_PREFIX, WOR_TIER_PREFIX, X_TIER_PREFIX,
};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use crate::backend::utils;

/// Compute completion candidates from CST context at cursor position.
pub fn completion(
    chat_file: &talkbank_model::model::ChatFile,
    tree: &Tree,
    document: &str,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = utils::position_to_offset(document, position);

    // Text is used only to match unfinished editor prefixes against the offered
    // literals. The native tree owns recognized token and main-tier context.
    let line_start = document[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line_prefix = &document[line_start..offset];

    if line_prefix.starts_with('@')
        && let Some(site) = header_completion_site(tree, document, line_start, offset)
    {
        return Some(CompletionResponse::Array(complete_header(site)));
    }

    let root = tree.root_node();
    let node = root.descendant_for_byte_range(offset, offset)?;

    // A recognized postcode is more specific than the unfinished-bracket
    // trigger. Replace its complete CST extent, not an editor-guessed word.
    if let Some(postcode) = find_ancestor_kind(node, POSTCODE) {
        return Some(CompletionResponse::Array(complete_postcode(node_range(
            postcode, document,
        ))));
    }

    if let Some(relative_start) = line_prefix.rfind('[') {
        let start = line_start + relative_start;
        let draft = &document[start..offset];
        if ANNOTATIONS.iter().any(|(code, _)| code.starts_with(draft))
            && let Some(opening) = root.descendant_for_byte_range(start, start + 1)
            && find_ancestor_kind(opening, MAIN_TIER).is_some()
            && !has_complete_extent(opening, start, offset)
        {
            let index = utils::LineIndex::new(document);
            let range = Range::new(
                index.offset_to_position(start),
                index.offset_to_position(offset),
            );
            return Some(CompletionResponse::Array(complete_bracket_annotation(
                range,
            )));
        }
    }

    if let Some(speaker) = find_ancestor_kind(node, SPEAKER)
        && let Some(completions) = complete_speaker_code(chat_file, node_range(speaker, document))
    {
        return Some(CompletionResponse::Array(completions));
    }

    if let Some(prefix) = tier_prefix_node(node) {
        return Some(CompletionResponse::Array(complete_tier_type(node_range(
            prefix, document,
        ))));
    }

    None
}

const HEADERS: &[(&str, &str)] = &[
    ("@Begin", "Start of transcript"),
    ("@End", "End of transcript"),
    ("@UTF8", "UTF-8 encoding declaration"),
    ("@Languages:\t", "Language(s) of the transcript"),
    ("@Participants:\t", "Participant declarations"),
    (
        "@ID:\t",
        "Participant identification (language|corpus|code|age|sex|||role|||)",
    ),
    ("@Media:\t", "Associated media file"),
    ("@Date:\t", "Recording date (DD-MMM-YYYY)"),
    ("@Location:\t", "Recording location"),
    ("@Situation:\t", "Recording situation description"),
    ("@Comment:\t", "General comment"),
    ("@Transcriber:\t", "Transcriber name"),
    ("@Coder:\t", "Coder name"),
    ("@Activities:\t", "Activities in the session"),
    ("@Options:\t", "Transcript options (CA, multi)"),
    ("@Bg:\t", "Begin gem (activity section)"),
    ("@Eg:\t", "End gem (activity section)"),
    ("@Birth of ", "Birth date for participant"),
    ("@Birthplace of ", "Birthplace for participant"),
    ("@L1 of ", "First language of participant"),
    ("@Number:\t", "Number of participants in transcript"),
    ("@Recording Quality:\t", "Quality of the recording"),
    ("@Room Layout:\t", "Layout of the recording room"),
    ("@Tape Location:\t", "Location on tape"),
    ("@Time Duration:\t", "Duration of the session"),
    ("@Time Start:\t", "Start time of the session"),
    ("@Transcription:\t", "Transcription status (full, partial)"),
    ("@Warning:\t", "Warning note"),
];

/// Headers with fields preserve them; fieldless names and drafts insert the
/// full offered template. Neither capability grants permission to edit a value.
#[derive(Clone, Copy)]
enum HeaderCompletionSite {
    Name(Range),
    Draft(Range),
}

fn header_name(template: &str) -> &str {
    template.trim_end_matches([':', '\t', ' '])
}

fn header_completion_site(
    tree: &Tree,
    document: &str,
    line_start: usize,
    offset: usize,
) -> Option<HeaderCompletionSite> {
    let token = tree
        .root_node()
        .descendant_for_byte_range(line_start, line_start + 1)?;
    if token.start_byte() == line_start
        && token.child_count() == 0
        && !token.has_error()
        && !token.is_missing()
        && offset <= token.end_byte()
        && let Ok(text) = token.utf8_text(document.as_bytes())
        && HEADERS
            .iter()
            .any(|(template, _)| header_name(template) == text)
    {
        let has_fields = token.parent().is_some_and(|header| {
            let mut cursor = header.walk();
            header
                .named_children(&mut cursor)
                .any(|child| child.kind() == talkbank_parser::node_types::HEADER_SEP)
        });
        let range = node_range(token, document);
        return Some(if has_fields {
            HeaderCompletionSite::Name(range)
        } else {
            HeaderCompletionSite::Draft(range)
        });
    }

    // Only a complete unfinished line that matches an offered template is a
    // draft. This is completion-prefix matching, not CHAT header parsing.
    let line = document[line_start..].split(['\r', '\n']).next()?;
    if offset != line_start + line.len()
        || !HEADERS
            .iter()
            .any(|(template, _)| template.starts_with(line))
    {
        return None;
    }
    let index = utils::LineIndex::new(document);
    Some(HeaderCompletionSite::Draft(Range::new(
        index.offset_to_position(line_start),
        index.offset_to_position(offset),
    )))
}

/// Do not treat a recognized complete construct as an unfinished prefix.
fn has_complete_extent(mut node: Node<'_>, start: usize, offset: usize) -> bool {
    while node.start_byte() == start {
        if !node.has_error() && node.end_byte() > offset {
            return true;
        }
        let Some(parent) = node.parent() else { break };
        node = parent;
    }
    false
}

/// Complete header names after `@`.
fn complete_header(site: HeaderCompletionSite) -> Vec<CompletionItem> {
    HEADERS
        .iter()
        .map(|(name, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            text_edit: Some(CompletionTextEdit::Edit(match site {
                HeaderCompletionSite::Name(range) => TextEdit {
                    range,
                    new_text: header_name(name).to_owned(),
                },
                HeaderCompletionSite::Draft(range) => TextEdit {
                    range,
                    new_text: name.to_string(),
                },
            })),
            ..Default::default()
        })
        .collect()
}

const ANNOTATIONS: &[(&str, &str)] = &[
    ("[//]", "Retracing/correction with different words"),
    ("[/]", "Partial repetition"),
    ("[///]", "Multiple retracing/false starts"),
    ("[: replacement]", "Replacement, actual intended word"),
    ("[=! text]", "Paralinguistic, action/sound description"),
    ("[= text]", "Explanation, clarification of context"),
    ("[*]", "Error marker, marks preceding word as error"),
    ("[+ gram]", "Postcode, grammatical utterance"),
    ("[+ bch]", "Postcodes, backchannel"),
    ("[+ trn]", "Postcode, translation utterance"),
    ("[?]", "Best guess, uncertain transcription"),
    ("[>]", "Overlap follows, speaker is overlapped"),
    ("[<]", "Overlap precedes, speaker overlaps"),
    ("[% comment]", "Comment on preceding word or scope"),
    ("[!]", "Stressing, emphasis"),
    ("[!!]", "Contrastive stressing, strong emphasis"),
];

/// Complete an unfinished annotation prefix inside a native main-tier context.
fn complete_bracket_annotation(range: Range) -> Vec<CompletionItem> {
    ANNOTATIONS
        .iter()
        .map(|(code, desc)| CompletionItem {
            label: code.to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some(desc.to_string()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: code.to_string(),
            })),
            ..Default::default()
        })
        .collect()
}

/// Complete speaker codes after `*`
fn complete_speaker_code(
    chat_file: &talkbank_model::model::ChatFile,
    range: Range,
) -> Option<Vec<CompletionItem>> {
    // Get all declared participants
    let mut speaker_codes = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in &chat_file.lines {
        if let talkbank_model::model::Line::Header { header, .. } = line {
            use talkbank_model::model::Header;
            match header.as_ref() {
                Header::Participants { entries } => {
                    for entry in entries {
                        if seen.insert(&entry.speaker_code) {
                            speaker_codes.push((entry.speaker_code.as_str(), entry.role.as_str()));
                        }
                    }
                }
                Header::ID(id_header) if seen.insert(&id_header.speaker) => {
                    speaker_codes.push((id_header.speaker.as_str(), id_header.role.as_str()));
                }
                _ => {}
            }
        }
    }

    let items: Vec<CompletionItem> = speaker_codes
        .into_iter()
        .map(|(code, role)| CompletionItem {
            label: code.to_owned(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(role.to_owned()),
            documentation: Some(Documentation::String(format!(
                "Speaker: {} ({})",
                code, role
            ))),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: code.to_owned(),
            })),
            ..Default::default()
        })
        .collect();

    if items.is_empty() { None } else { Some(items) }
}

/// Complete dependent tier types after `%`
fn complete_tier_type(range: Range) -> Vec<CompletionItem> {
    // Common tier types with descriptions
    let tier_types = vec![
        ("mor", "Morphological analysis"),
        ("gra", "Grammatical relations"),
        ("pho", "Phonological transcription"),
        ("mod", "Model phonology"),
        ("sin", "Gesture/sign annotations"),
        ("act", "Action coding"),
        ("cod", "General coding"),
        ("com", "Comments"),
        ("exp", "Explanations"),
        ("add", "Additional information"),
        ("sit", "Situational information"),
        ("spa", "Spatial information"),
        ("int", "Intonation"),
        ("gpx", "Gestural transcription"),
    ];

    tier_types
        .into_iter()
        .map(|(name, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: format!("%{name}"),
            })),
            ..Default::default()
        })
        .collect()
}

/// Suggest model-serialized tags for an already recognized postcode extent.
fn complete_postcode(range: Range) -> Vec<CompletionItem> {
    // Suggestions, not a closed vocabulary or automatic analysis policy.
    let postcodes = [
        ("bch", "Backchannel"),
        ("exc", "Exclude utterance from analysis"),
        ("gram", "Grammatical utterance"),
        ("trn", "Translation utterance"),
    ];

    postcodes
        .into_iter()
        .map(|(code, desc)| {
            // A completion item is synthesized text, not a source position.
            let postcode =
                talkbank_model::model::Postcode::new(talkbank_model::Location::Unlocated, code)
                    .to_string();
            CompletionItem {
                label: postcode.clone(),
                kind: Some(CompletionItemKind::VALUE),
                detail: Some(desc.to_string()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: postcode,
                })),
                ..Default::default()
            }
        })
        .collect()
}

/// Retain the recognized prefix extent for replacement; its colon is separate.
fn tier_prefix_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);
    while let Some(node) = current {
        match node.kind() {
            ACT_TIER_PREFIX | ADD_TIER_PREFIX | ALT_TIER_PREFIX | COD_TIER_PREFIX
            | COH_TIER_PREFIX | COM_TIER_PREFIX | DEF_TIER_PREFIX | ENG_TIER_PREFIX
            | ERR_TIER_PREFIX | EXP_TIER_PREFIX | FAC_TIER_PREFIX | FLO_TIER_PREFIX
            | GLS_TIER_PREFIX | GPX_TIER_PREFIX | GRA_TIER_PREFIX | INT_TIER_PREFIX
            | MOD_TIER_PREFIX | MOR_TIER_PREFIX | ORT_TIER_PREFIX | PAR_TIER_PREFIX
            | PHO_TIER_PREFIX | SIN_TIER_PREFIX | SIT_TIER_PREFIX | SPA_TIER_PREFIX
            | TIM_TIER_PREFIX | WOR_TIER_PREFIX | X_TIER_PREFIX => return Some(node),
            _ => {}
        }
        current = node.parent();
    }
    None
}

/// Convert a recognized token's complete extent using the document's UTF-16 index.
fn node_range(node: Node<'_>, document: &str) -> Range {
    let index = utils::LineIndex::new(document);
    Range::new(
        index.offset_to_position(node.start_byte()),
        index.offset_to_position(node.end_byte()),
    )
}

/// Walk parent chain until a node with the requested `kind` is found.
fn find_ancestor_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(node) = current {
        if node.kind() == kind {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{parse_chat, parse_tree};

    #[test]
    fn header_completion_at_line_start() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position cursor on @End line at character 1 (inside the '@')
        let pos = Position {
            line: 6,
            character: 1,
        };
        let result = completion(&chat_file, &tree, content, pos);
        assert!(result.is_some(), "Expected header completions after @");
        if let Some(CompletionResponse::Array(items)) = result {
            assert!(
                items.len() > 10,
                "Expected many header completions, got {}",
                items.len()
            );
            assert!(
                items.iter().any(|i| i.label == "@Languages:\t"),
                "Expected @Languages in completions"
            );
            let language = items
                .iter()
                .find(|item| item.label == "@Languages:\t")
                .expect("language template");
            let Some(CompletionTextEdit::Edit(edit)) = &language.text_edit else {
                panic!("explicit fieldless-header edit");
            };
            assert_eq!(edit.new_text, "@Languages:\t");
            assert_eq!(
                edit.range,
                Range::new(Position::new(6, 0), Position::new(6, 4))
            );
            assert!(
                items.iter().any(|i| i.label == "@Begin"),
                "Expected @Begin in completions"
            );
            assert!(
                items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::KEYWORD)),
                "All header completions should be KEYWORD kind"
            );
        }
    }

    #[test]
    fn header_completion_edits_names_without_touching_values() {
        let reference = include_str!("../../../../../corpus/reference/content/split-metadata.cha");
        for source in [reference.to_owned(), reference.replace('\n', "\r\n")] {
            let file = crate::test_fixtures::valid_chat(&source).expect("valid reference");
            let tree = parse_tree(&source);
            let start = source.find("@Comment:").expect("comment header");
            let pos = utils::offset_to_position(&source, start + 3);
            let Some(CompletionResponse::Array(items)) = completion(&file, &tree, &source, pos)
            else {
                panic!("header completions");
            };
            let item = items
                .iter()
                .find(|item| item.label == "@Comment:\t")
                .expect("comment suggestion");
            let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                panic!("explicit header edit");
            };
            assert_eq!(utils::position_to_offset(&source, edit.range.start), start);
            assert_eq!(
                utils::position_to_offset(&source, edit.range.end),
                start + "@Comment".len()
            );
            assert_eq!(edit.new_text, "@Comment");
            let mut changed = source.clone();
            changed.replace_range(start..start + "@Comment".len(), &edit.new_text);
            assert_eq!(changed, source);
            let value_pos = utils::offset_to_position(&source, start + "@Comment:\tAuthored".len());
            assert!(
                completion(&file, &tree, &source, value_pos).is_none(),
                "no header-name edits in a value"
            );

            let partial = source.replace("@End", "@E");
            let draft_start = partial.rfind("@E").expect("partial header");
            let partial_file = parse_chat(&partial);
            let partial_tree = parse_tree(&partial);
            let pos = utils::offset_to_position(&partial, draft_start + 2);
            let Some(CompletionResponse::Array(items)) =
                completion(&partial_file, &partial_tree, &partial, pos)
            else {
                panic!("partial header completion");
            };
            let item = items
                .iter()
                .find(|item| item.label == "@End")
                .expect("end suggestion");
            let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                panic!("explicit draft edit");
            };
            let mut restored = partial.clone();
            restored.replace_range(
                utils::position_to_offset(&partial, edit.range.start)
                    ..utils::position_to_offset(&partial, edit.range.end),
                &edit.new_text,
            );
            assert_eq!(restored, source);
        }
    }

    #[test]
    fn bracket_completion_replaces_the_current_draft_not_earlier_annotations() {
        let reference =
            include_str!("../../../../../corpus/reference/content/postcodes-and-freecodes.cha");
        for source in [
            reference.replace("hello . [+ bch] [+ foo]", "hello [= 😀] [ ."),
            reference
                .replace("hello . [+ bch] [+ foo]", "hello [= 😀] [ .")
                .replace('\n', "\r\n"),
        ] {
            let start = source.find("[ .").expect("draft annotation");
            let file = parse_chat(&source);
            let tree = parse_tree(&source);
            let position = utils::offset_to_position(&source, start + 1);
            let Some(CompletionResponse::Array(items)) =
                completion(&file, &tree, &source, position)
            else {
                panic!("completion after a prior closed annotation");
            };
            let item = items
                .iter()
                .find(|item| item.label == "[% comment]")
                .expect("comment annotation");
            let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                panic!("explicit annotation edit");
            };
            assert_eq!(utils::position_to_offset(&source, edit.range.start), start);
            assert_eq!(
                utils::position_to_offset(&source, edit.range.end),
                start + 1
            );
            let mut changed = source.clone();
            changed.replace_range(start..start + 1, &edit.new_text);
            assert_eq!(&changed[..start], &source[..start]);
            assert_eq!(
                &changed[start + edit.new_text.len()..],
                &source[start + 1..]
            );
            crate::test_fixtures::valid_chat(&changed).expect("completed annotation is valid");
        }
    }

    #[test]
    fn bracket_drafts_do_not_reinterpret_header_tier_text_or_complete_annotations() {
        let reference = include_str!("../../../../../corpus/reference/content/split-metadata.cha");
        for (source, needle) in [
            (
                reference.replace("Authored split control:", "["),
                "@Comment:\t[",
            ),
            (reference.replace("first-child comment", "["), "%com:\t["),
            (reference.replace("ready .", "ready [?] ."), "ready ["),
        ] {
            let offset = source.find(needle).expect("control location") + needle.len();
            let file = crate::test_fixtures::valid_chat(&source).expect("valid control");
            let tree = parse_tree(&source);
            let position = utils::offset_to_position(&source, offset);
            assert!(
                completion(&file, &tree, &source, position).is_none(),
                "not an annotation draft: {needle}"
            );
        }
    }

    #[test]
    fn no_completion_on_main_tier_word() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position on "hello" (the word, not a trigger character)
        let pos = Position {
            line: 5,
            character: 8,
        };
        let result = completion(&chat_file, &tree, content, pos);
        assert!(result.is_none(), "Expected no completions on a plain word");
    }

    #[test]
    fn speaker_code_completion_returns_declared_participants() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position on the speaker code '*CHI:', character 1 is inside the speaker
        let pos = Position {
            line: 6,
            character: 1,
        };
        let result = completion(&chat_file, &tree, content, pos);
        // The handler checks if cursor is in a SPEAKER node. If the tree-sitter
        // tree has a SPEAKER node at that position, we get speaker completions.
        if let Some(CompletionResponse::Array(items)) = &result {
            // If we got completions, they should be speaker codes.
            assert!(
                items.iter().any(|i| i.label == "CHI"),
                "Expected CHI in speaker completions"
            );
            assert!(
                items.iter().any(|i| i.label == "MOT"),
                "Expected MOT in speaker completions"
            );
            for item in items {
                assert_eq!(
                    item.kind,
                    Some(CompletionItemKind::CLASS),
                    "Speaker completions should be CLASS kind"
                );
            }
        } else {
            panic!("expected speaker completion response");
        }
    }

    #[test]
    fn cst_completion_edits_preserve_speaker_and_tier_delimiters() {
        let reference = include_str!("../../../../../corpus/reference/content/split-metadata.cha");
        for source in [reference.to_owned(), reference.replace('\n', "\r\n")] {
            let file = crate::test_fixtures::valid_chat(&source).expect("valid reference");
            let tree = parse_tree(&source);
            for (needle, skip, label, replacement) in
                [("*MOT", 1, "CHI", "CHI"), ("%com", 0, "exp", "%exp")]
            {
                let start = source.find(needle).expect("reference prefix") + skip;
                let end = start + needle.len() - skip;
                let pos = utils::offset_to_position(&source, start + 1);
                let Some(CompletionResponse::Array(items)) = completion(&file, &tree, &source, pos)
                else {
                    panic!("expected prefix completion");
                };
                let matches: Vec<_> = items.iter().filter(|item| item.label == label).collect();
                assert_eq!(matches.len(), 1, "one suggestion per identifier");
                let Some(CompletionTextEdit::Edit(edit)) = &matches[0].text_edit else {
                    panic!("prefix completion needs an explicit edit");
                };
                assert_eq!(utils::position_to_offset(&source, edit.range.start), start);
                assert_eq!(utils::position_to_offset(&source, edit.range.end), end);
                assert_eq!(edit.new_text, replacement);
                let mut changed = source.clone();
                changed.replace_range(start..end, &edit.new_text);
                assert_eq!(&changed[..start], &source[..start]);
                assert_eq!(&changed[start + edit.new_text.len()..], &source[end..]);
                assert!(!parse_tree(&changed).root_node().has_error());
                crate::test_fixtures::valid_chat(&changed).expect("valid compatible edit");
            }
        }
    }

    #[test]
    fn bracket_annotation_completion_inside_bracket() {
        let reference =
            include_str!("../../../../../corpus/reference/content/postcodes-and-freecodes.cha");
        // Incomplete editor input intentionally enters recovery. Exercise the
        // request handler, not just the candidate-list helper.
        let content = reference.replace("hello . [+ bch] [+ foo]", "hello [");
        let chat_file = parse_chat(&content);
        let tree = parse_tree(&content);
        let offset = content.find("hello [").expect("edited utterance") + "hello [".len();
        let position = utils::offset_to_position(&content, offset);
        let Some(CompletionResponse::Array(annotations)) =
            completion(&chat_file, &tree, &content, position)
        else {
            panic!("expected bracket completions for incomplete editor input");
        };
        assert!(
            annotations.iter().any(|i| i.label == "[//]"),
            "Expected [//] retracing in completions"
        );
        assert!(
            annotations.iter().any(|i| i.label == "[*]"),
            "Expected [*] error marker in completions"
        );
        assert!(
            annotations
                .iter()
                .all(|i| i.kind == Some(CompletionItemKind::SNIPPET)),
            "All bracket annotations should be SNIPPET kind"
        );
    }

    #[test]
    fn complete_header_returns_expected_count() {
        let headers = complete_header(HeaderCompletionSite::Draft(Range::default()));
        // The implementation lists header types; verify we get a reasonable count.
        assert!(
            headers.len() >= 20,
            "Expected at least 20 header completions, got {}",
            headers.len()
        );
        // Verify each has a label and detail.
        for item in &headers {
            assert!(!item.label.is_empty(), "Header label should not be empty");
            assert!(
                item.detail.is_some(),
                "Header {:?} should have a detail",
                item.label
            );
        }
    }

    #[test]
    fn bracket_annotation_catalog_excludes_unsupported_quotation_syntax() {
        let annotations = complete_bracket_annotation(Range::default());
        assert!(!annotations.iter().any(|item| item.label.contains("\"\"\"")));
    }

    #[test]
    fn postcode_completion_replaces_only_the_selected_cst_postcode() {
        let reference =
            include_str!("../../../../../corpus/reference/content/postcodes-and-freecodes.cha");
        for source in [
            reference.to_owned(),
            reference.replace('\n', "\r\n"),
            reference.replace("hello .", "hello [= 😀] ."),
        ] {
            let chat_file = crate::test_fixtures::valid_chat(&source).expect("valid reference");
            let tree = parse_tree(&source);
            assert!(!tree.root_node().has_error());
            for original in ["[+ bch]", "[+ foo]"] {
                let start = source.find(original).expect("reference postcode");
                let end = start + original.len();
                let position = utils::offset_to_position(&source, start + 4);
                let Some(CompletionResponse::Array(items)) =
                    completion(&chat_file, &tree, &source, position)
                else {
                    panic!("expected postcode completion response");
                };
                assert_eq!(items.len(), 4);
                for (item, expected) in items.iter().zip(["bch", "exc", "gram", "trn"]) {
                    let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                        panic!("postcode completion must carry an explicit replacement range");
                    };
                    assert_eq!(utils::position_to_offset(&source, edit.range.start), start);
                    assert_eq!(utils::position_to_offset(&source, edit.range.end), end);
                    assert_eq!(edit.new_text, format!("[+ {expected}]"));
                    let mut changed = source.clone();
                    changed.replace_range(start..end, &edit.new_text);
                    assert_eq!(&changed[..start], &source[..start]);
                    assert_eq!(&changed[start + edit.new_text.len()..], &source[end..]);
                    assert!(!parse_tree(&changed).root_node().has_error());
                    let parsed = crate::test_fixtures::valid_chat(&changed).expect("valid edit");
                    let codes: Vec<_> = parsed
                        .utterances()
                        .next()
                        .expect("first utterance")
                        .main
                        .content
                        .postcodes
                        .iter()
                        .map(|code| code.text.as_str())
                        .collect();
                    let expected_codes = if original == "[+ bch]" {
                        vec![expected, "foo"]
                    } else {
                        vec!["bch", expected]
                    };
                    assert_eq!(codes, expected_codes);
                }
            }
        }
    }

    #[test]
    fn complete_tier_type_returns_tier_names() {
        let items = complete_tier_type(Range::default());
        assert!(
            items.iter().any(|i| i.label == "mor"),
            "Expected mor in tier completions"
        );
        assert!(
            items.iter().any(|i| i.label == "gra"),
            "Expected gra in tier completions"
        );
        // Only the prefix is replaced; the existing colon/tab is not duplicated.
        for item in &items {
            let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                panic!("tier completion needs an edit");
            };
            assert_eq!(edit.new_text, format!("%{}", item.label));
        }
    }

    #[test]
    fn complete_speaker_code_empty_when_no_participants() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@End\n";
        let chat_file = parse_chat(content);
        let result = complete_speaker_code(&chat_file, Range::default());
        assert!(
            result.is_none(),
            "Expected None when no participants declared"
        );
    }

    #[test]
    fn complete_speaker_code_extracts_from_id_headers() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother, FAT Father\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n@ID:\teng|corpus|FAT|||||Father|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let result = complete_speaker_code(&chat_file, Range::default());
        assert!(result.is_some(), "Expected speaker completions");
        let items = result.unwrap();
        // Both header forms contribute, but each identity is suggested once.
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.contains(&"CHI"), "Expected CHI speaker code");
        assert!(labels.contains(&"MOT"), "Expected MOT speaker code");
        assert!(labels.contains(&"FAT"), "Expected FAT speaker code");
    }
}
