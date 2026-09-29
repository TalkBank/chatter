//! Finite reference field layouts exercise real JSON output truncation.
use std::collections::BTreeSet;
use std::io::Cursor;
use talkbank_model::model::{ContentStructure, Descend, Line, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

#[test]
fn reference_word_json_propagates_bounded_output_failure() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut layouts = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut refusals = 0;
    for fixture in corpus.fixtures() {
        let file = strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parse");
        for utterance in file.utterances() {
            for item in &utterance.main.content.content {
                item.structure().walk(&mut |structure| {
                    if let ContentStructure::Word(view) = structure {
                        for word in view.words() {
                            let value = serde_json::to_value(word).expect("word JSON");
                            let object = value.as_object().expect("word wire object");
                            let layout: Vec<_> = object.keys().cloned().collect();
                            fields.extend(layout.iter().cloned());
                            if !layouts.insert(layout) {
                                continue;
                            }
                            assert_eq!(word.to_string(), word.to_chat_string());
                            refusals += assert_json_refusals(word, fixture.path());
                        }
                    }
                    Descend::Into
                });
            }
        }
    }
    for field in [
        "raw_text",
        "cleaned_text",
        "content",
        "lang",
        "form_type",
        "category",
    ] {
        assert!(fields.contains(field), "missing reference field: {field}");
    }
    assert!(
        layouts.len() > 1 && refusals > 0,
        "nonvacuous field-layout deck"
    );
}

#[test]
fn reference_header_and_tier_json_propagate_bounded_output_failure() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut headers = BTreeSet::new();
    let mut tiers = BTreeSet::new();
    for fixture in corpus.fixtures() {
        let file = strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parse");
        for line in &file.lines {
            match line {
                Line::Header { header, .. } => {
                    if headers.insert(wire_layout(header)) {
                        assert!(assert_json_refusals(header, fixture.path()) > 0);
                    }
                }
                Line::Utterance(utterance) => {
                    for tier in &utterance.dependent_tiers {
                        if tiers.insert(wire_layout(tier)) {
                            assert!(assert_json_refusals(tier, fixture.path()) > 0);
                        }
                    }
                }
            }
        }
    }
    assert!(
        headers.len() > 1 && tiers.len() > 1,
        "both wire families need witnesses"
    );
}

fn wire_layout(value: &impl serde::Serialize) -> (String, Vec<String>) {
    let json = serde_json::to_value(value).expect("tagged wire value");
    let object = json.as_object().expect("tagged object");
    let kind = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .expect("wire discriminant")
        .to_owned();
    (kind, object.keys().cloned().collect())
}

fn assert_json_refusals(value: &impl serde::Serialize, path: &std::path::Path) -> usize {
    let expected = serde_json::to_vec(value).expect("accepting JSON sink");
    let mut buffer = vec![0; expected.len()];
    // Cursor over a bounded slice is a standard I/O sink: write_all reports
    // WriteZero when it fills, including within a UTF-8 code point.
    for capacity in 0..expected.len() {
        let mut sink = Cursor::new(&mut buffer[..capacity]);
        let error = serde_json::to_writer(&mut sink, value)
            .expect_err("truncated JSON must never report success");
        assert!(error.is_io(), "{}: {error}", path.display());
        assert_eq!(sink.position() as usize, capacity);
        assert_eq!(&buffer[..capacity], &expected[..capacity]);
    }
    serde_json::to_writer(Cursor::new(buffer.as_mut_slice()), value)
        .expect("exact capacity accepts the complete JSON");
    assert_eq!(buffer, expected);
    expected.len()
}
