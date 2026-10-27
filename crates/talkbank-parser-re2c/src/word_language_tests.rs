//! Both parsers must preserve independently specified language marker meaning.
#![allow(clippy::unwrap_used)]
use talkbank_model::model::{LanguageCode, WordLanguageMarker, WriteChat};
use talkbank_model::{ChatParser, ErrorCollector, Location, Span};

#[test]
fn word_language_markers_preserve_meaning_spelling_and_origin() {
    let code = |text| LanguageCode::new(text).unwrap();
    let cases = [
        ("@s", WordLanguageMarker::Shortcut),
        ("@s:und", WordLanguageMarker::Explicit(code("und"))),
        ("@s:eng", WordLanguageMarker::Explicit(code("eng"))),
        (
            "@s:eng+fra",
            WordLanguageMarker::Multiple(vec![code("eng"), code("fra")]),
        ),
        (
            "@s:eng&spa",
            WordLanguageMarker::Ambiguous(vec![code("eng"), code("spa")]),
        ),
    ];
    fn check(parser: &impl ChatParser, cases: &[(&str, WordLanguageMarker)]) {
        for (suffix, expected) in cases {
            for base in ["café", "hel:lo"] {
                let input = format!("{base}{suffix}");
                for offset in [0, 200] {
                    let errors = ErrorCollector::new();
                    let word = parser
                        .parse_word(&input, offset, &errors)
                        .into_option()
                        .unwrap();
                    assert_eq!(word.lang.as_ref(), Some(expected), "{input}");
                    assert_eq!(word.to_chat_string(), input);
                    assert_eq!(
                        word.location,
                        Location::Located(Span::new(offset, offset + input.len()))
                    );
                    assert!(errors.is_empty(), "{input}");
                }
            }
        }
    }
    check(&crate::Re2cParser::new(), &cases);
    check(&talkbank_parser::TreeSitterParser::new().unwrap(), &cases);
}

#[test]
fn word_categories_preserve_meaning_spelling_and_origin() {
    use talkbank_model::WordCategory;
    let cases = [
        ("0is", WordCategory::Omission),
        ("&-um", WordCategory::Filler),
        ("&~gaga", WordCategory::Nonword),
        ("&+fr", WordCategory::PhonologicalFragment),
    ];
    fn check(parser: &impl ChatParser, cases: &[(&str, WordCategory)]) {
        for (input, expected) in cases {
            for offset in [0, 200] {
                let errors = ErrorCollector::new();
                let word = parser
                    .parse_word(input, offset, &errors)
                    .into_option()
                    .unwrap();
                assert_eq!(word.category.as_ref(), Some(expected));
                assert_eq!(word.to_chat_string(), *input);
                assert_eq!(
                    word.location,
                    Location::Located(Span::new(offset, offset + input.len()))
                );
                assert!(errors.is_empty(), "{input}");
            }
        }
    }
    check(&crate::Re2cParser::new(), &cases);
    check(&talkbank_parser::TreeSitterParser::new().unwrap(), &cases);
}
