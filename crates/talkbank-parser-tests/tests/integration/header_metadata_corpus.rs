//! Mutable header lines remain the sole authority for file metadata policies.

use talkbank_model::model::{ChatOptionFlag, ChatOptionFlags};
use talkbank_model::{ChatFile, ChatParser, ErrorCollector, Header, Line};
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_transform::parse::{is_ca, is_no_align};
use talkbank_transform::validate::validate_output;

fn reference(parser: &impl ChatParser, path: &str) -> ChatFile {
    let source = std::fs::read_to_string(workspace_root().join(path)).unwrap();
    let errors = ErrorCollector::new();
    let file = parser
        .parse_chat_file(&source, 0, &errors)
        .into_option()
        .unwrap();
    assert!(errors.into_vec().is_empty());
    file
}

fn edited_option_policy(parser: &impl ChatParser) {
    let mut file = reference(parser, "corpus/reference/core/headers-time-and-types.cha");
    assert!(is_ca(&file));
    for line in &mut file.lines {
        if let Line::Utterance(utterance) = line {
            utterance.main.content.terminator = None;
            utterance.forget_parse_provenance();
        }
    }
    assert!(validate_output(&file, "edit").is_ok());
    let index = file
        .lines
        .iter()
        .position(|line| matches!(line.as_header(), Some(Header::Options { .. })))
        .unwrap();
    let header = file.lines.remove(index);
    assert!(!is_ca(&file), "removed CA header must stop waiving policy");
    assert!(
        validate_output(&file, "edit")
            .unwrap_err()
            .iter()
            .any(|error| { error.message.contains("lost its terminator") })
    );
    file.lines.insert(index, header);
    assert!(is_ca(&file), "restored header restores policy");
    assert!(validate_output(&file, "edit").is_ok());
    for line in &mut file.lines {
        if let Line::Header { header, .. } = line
            && let Header::Options { options } = header.as_mut()
        {
            *options = ChatOptionFlags::new(vec![ChatOptionFlag::NoAlign]);
        }
    }
    assert!(!is_ca(&file));
    assert!(
        is_no_align(&file),
        "in-place header edits drive pipeline selection"
    );
}

#[test]
fn header_metadata_option_policy_follows_edits_in_both_backends() {
    edited_option_policy(&talkbank_parser::TreeSitterParser::new().unwrap());
    edited_option_policy(&talkbank_parser_re2c::Re2cParser::new());
}

fn wire_metadata(parser: &impl ChatParser) {
    let file = reference(parser, "corpus/reference/core/headers-time-and-types.cha");
    let mut wire = serde_json::to_value(&file).unwrap();
    assert!(wire.get("options").is_none());
    assert!(wire.get("media").is_none());
    // An obsolete root field cannot cancel the actual CA header.
    wire["options"] = serde_json::json!([]);
    let imported: ChatFile = serde_json::from_value(wire).unwrap();
    assert!(is_ca(&imported));

    let mut file = reference(parser, "corpus/reference/audio/english-child-speech.cha");
    let expected = file.media_header().unwrap().clone();
    let index = file
        .lines
        .iter()
        .position(|line| matches!(line.as_header(), Some(Header::Media(_))))
        .unwrap();
    let header = file.lines.remove(index);
    assert!(file.media_header().is_none());
    let mut wire = serde_json::to_value(&file).unwrap();
    wire["media"] = serde_json::to_value(&expected).unwrap();
    let imported: ChatFile = serde_json::from_value(wire).unwrap();
    assert!(
        imported.media_header().is_none(),
        "root metadata cannot invent a header"
    );
    file.lines.insert(index, header);
    assert_eq!(file.media_header(), Some(&expected));
    let wire = serde_json::to_value(&file).unwrap();
    assert!(wire.get("media").is_none());
    let imported: ChatFile = serde_json::from_value(wire).unwrap();
    assert_eq!(imported.media_header(), Some(&expected));
}

#[test]
fn header_metadata_wire_uses_lines_without_independent_overrides() {
    wire_metadata(&talkbank_parser::TreeSitterParser::new().unwrap());
    wire_metadata(&talkbank_parser_re2c::Re2cParser::new());
}
