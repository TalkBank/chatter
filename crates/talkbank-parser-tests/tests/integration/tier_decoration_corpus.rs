//! Typed decoration editing using canonical parsed payloads.

use talkbank_model::{SemanticEq, Span, SpanShift};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{
    chat_corpus::ChatCorpus, repo_paths::workspace_root, test_error::strict_parse,
};

#[test]
fn reference_decoration_edits_shift_locations_without_changing_content() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(&workspace_root().join("corpus/reference")).expect("reference");
    let mut witnessed = [0; 2];
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            let original = &utterance.main.content;
            let mut edited = original.clone();
            for linker in &mut edited.linkers {
                let before = linker.span;
                assert_ne!(
                    before,
                    Span::DUMMY,
                    "reference linker must have source coordinates"
                );
                linker.shift_spans_after(0, 1);
                assert_eq!(linker.span, Span::new(before.start + 1, before.end + 1));
                witnessed[0] += 1;
            }
            for postcode in &mut edited.postcodes {
                let before = postcode.span;
                assert_ne!(
                    before,
                    Span::DUMMY,
                    "reference postcode must have source coordinates"
                );
                postcode.shift_spans_after(0, 1);
                assert_eq!(postcode.span, Span::new(before.start + 1, before.end + 1));
                witnessed[1] += 1;
            }
            assert!(edited.semantic_eq(original));
            assert_eq!(edited.to_content_string(), original.to_content_string());
            assert_eq!(
                serde_json::to_value(&edited).unwrap(),
                serde_json::to_value(original).unwrap()
            );
            for linker in &mut edited.linkers {
                linker.shift_spans_after(0, -1);
            }
            for postcode in &mut edited.postcodes {
                postcode.shift_spans_after(0, -1);
            }
            assert_eq!(
                &edited, original,
                "restoration includes exact spans and ordering"
            );
        }
    }
    assert!(witnessed.into_iter().all(|count| count > 0));
    eprintln!("reference decoration relocation witnesses: {witnessed:?}");
}
