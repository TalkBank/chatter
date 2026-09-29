//! Counting classification must not remove authored MOR/GRA chunks.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};

#[test]
fn reference_punctuation_classification_preserves_dependency_chunks() {
    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/word-features/1082.cha"))
            .expect("authored punctuation reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let mut comma = 0;
    let mut punctuation = 0;
    let mut lexical = 0;
    for utterance in file.utterances() {
        let Some(mor) = utterance.mor_tier() else {
            continue;
        };
        let gra = utterance.gra_tier().expect("reference pairs MOR and GRA");
        let mut word_chunks = 0;
        for item in mor.items() {
            for word in std::iter::once(&item.main).chain(&item.post_clitics) {
                word_chunks += 1;
                let expected = match (word.pos.as_str(), word.lemma.as_str()) {
                    ("cm", "cm") => {
                        comma += 1;
                        true
                    }
                    ("punct", "‡") => {
                        punctuation += 1;
                        true
                    }
                    _ => {
                        lexical += 1;
                        false
                    }
                };
                assert_eq!(word.is_punctuation_marker(), expected);
            }
        }
        // This reference has a terminator in every MOR tier. All words,
        // including punctuation and post-clitics, retain a GRA position.
        assert_eq!(mor.count_chunks(), word_chunks + 1);
        assert_eq!(mor.count_chunks(), gra.relations().len());
    }
    assert!(comma > 0 && punctuation > 0 && lexical > 0);
}
