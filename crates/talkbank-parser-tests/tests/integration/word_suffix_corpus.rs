//! Word suffix admission and recovery over authored CHAT sources.

use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::{
    ChatParser, ErrorCode, ErrorCollector, FormType, TranscriptName, Word, WriteChat,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WirePresence {
    Absent,
    Null,
    Present,
}

impl WirePresence {
    fn put(self, object: &mut serde_json::Value, field: &str, payload: &serde_json::Value) {
        match self {
            Self::Absent => {
                object.as_object_mut().expect("word object").remove(field);
            }
            Self::Null => object[field] = serde_json::Value::Null,
            Self::Present => object[field] = payload.clone(),
        }
    }
}

#[test]
fn reference_word_suffixes_keep_wire_spelling_and_refuse_conflicting_fields() {
    fn check(parser: &impl ChatParser) {
        let source = include_str!("../../../../corpus/reference/content/words-special-forms.cha");
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .expect("reference");
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        let schema = serde_json::to_value(schemars::schema_for!(Word)).expect("word schema");
        let validator = jsonschema::validator_for(&schema).expect("word schema validator");
        let mut originals = Vec::new();
        let mut form = None;
        let mut language = None;
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                let wire = serde_json::to_value(word).expect("word JSON");
                assert!(validator.is_valid(&wire), "{wire}");
                let readback: Word = serde_json::from_value(wire.clone()).expect("readback");
                assert_eq!(readback.to_chat(), word.to_chat());
                assert_eq!(serde_json::to_value(readback).expect("readback wire"), wire);
                if word.form_type().is_some() {
                    form = Some(wire["form_type"].clone());
                }
                if word.language_marker().is_some() {
                    language = Some(wire["lang"].clone());
                }
                originals.push(wire);
            });
        }
        let form = form.expect("authored form marker");
        let language = language.expect("authored language marker");
        let mut full_wire = serde_json::to_value(&file).expect("file JSON");
        let word = full_wire["lines"]
            .as_array_mut()
            .expect("lines")
            .iter_mut()
            .filter_map(|line| line.pointer_mut("/main/content/content/0"))
            .find(|word| word.get("form_type").is_some())
            .expect("authored form word");
        word["lang"] = language.clone();
        assert!(
            talkbank_transform::validate_json_string(
                &serde_json::to_string(&full_wire).expect("file wire")
            )
            .is_err()
        );
        assert!(serde_json::from_value::<talkbank_model::ChatFile>(full_wire).is_err());
        assert!(!originals.is_empty());
        for original in originals {
            for form_presence in [
                WirePresence::Absent,
                WirePresence::Null,
                WirePresence::Present,
            ] {
                for language_presence in [
                    WirePresence::Absent,
                    WirePresence::Null,
                    WirePresence::Present,
                ] {
                    let mut wire = original.clone();
                    form_presence.put(&mut wire, "form_type", &form);
                    language_presence.put(&mut wire, "lang", &language);
                    let admitted = !(form_presence == WirePresence::Present
                        && language_presence == WirePresence::Present);
                    assert_eq!(validator.is_valid(&wire), admitted, "{wire}");
                    let readback = serde_json::from_value::<Word>(wire.clone());
                    assert_eq!(readback.is_ok(), admitted, "{wire}");
                    if let Ok(word) = readback {
                        assert_eq!(
                            word.form_type().is_some(),
                            form_presence == WirePresence::Present
                        );
                        assert_eq!(
                            word.language_marker().is_some(),
                            language_presence == WirePresence::Present
                        );
                    }
                }
            }
        }
    }
    check(&talkbank_parser::TreeSitterParser::new().expect("parser"));
    check(&talkbank_parser_re2c::Re2cParser::new());
}

#[test]
fn authored_repeated_suffix_recovery_stays_invalid_after_json() {
    let parser = talkbank_parser::TreeSitterParser::new().expect("parser");
    for source in [
        include_str!("../error_corpus/validation_errors/E203_1.cha"),
        include_str!("../error_corpus/validation_errors/E203_2.cha"),
    ] {
        let parse_errors = ErrorCollector::new();
        let file = ChatParser::parse_chat_file(&parser, source, 0, &parse_errors)
            .into_option()
            .expect("recovery model");
        assert!(parse_errors.is_empty(), "{:?}", parse_errors.to_vec());
        assert_eq!(file.to_chat_string(), source);
        let wire = serde_json::to_value(&file).expect("recovery JSON");
        let readback: talkbank_model::ChatFile =
            serde_json::from_value(wire).expect("recovery readback");
        assert_eq!(readback.to_chat_string(), source);
        let mut unlocated_recoveries = 0;
        for utterance in readback.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                if let WordItem::Word(word) = item
                    && matches!(word.form_type(), Some(FormType::Undeclared(_)))
                {
                    assert!(word.location.span().is_none());
                    unlocated_recoveries += 1;
                }
            });
        }
        assert_eq!(unlocated_recoveries, 1);
        for document in [file, readback] {
            let errors = ErrorCollector::new();
            assert!(
                document
                    .validate_into(&errors, TranscriptName::Anonymous)
                    .is_err()
            );
            assert_eq!(
                errors
                    .to_vec()
                    .iter()
                    .filter(|error| error.code == ErrorCode::InvalidFormType)
                    .count(),
                1
            );
        }
    }
}
