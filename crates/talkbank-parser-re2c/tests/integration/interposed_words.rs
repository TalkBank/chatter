//! Interposed words retain ordinary word prefixes and suffixes.

use talkbank_model::{ChatParser, ErrorCollector, Terminator, UtteranceContent, WriteChat};

#[derive(Debug, Clone, Copy)]
enum Route {
    MainTier,
    Utterance,
}

fn check(parser: &impl ChatParser) {
    let cases: [(&str, &[&str]); 2] = [
        (
            include_str!("../../../../spec/constructs/word/interposed_word_with_prefixes.md"),
            &["&-uh", "&~hmm", "&+sh", "0is", "&-uh@i", "&~hmm@s:eng$n"],
        ),
        (
            include_str!("../../../../spec/constructs/word/interposed_word_with_form_marker.md"),
            &["a:nda@i"],
        ),
    ];
    for (spec, expected) in cases {
        let (_, fence) = spec.split_once("```main_tier\n").unwrap();
        let (source, _) = fence.split_once("```").unwrap();
        for origin in [0, 59] {
            for route in [Route::MainTier, Route::Utterance] {
                let errors = ErrorCollector::new();
                let main = match route {
                    Route::MainTier => parser
                        .parse_main_tier(source, origin, &errors)
                        .into_option(),
                    Route::Utterance => parser
                        .parse_utterance(source, origin, &errors)
                        .into_option()
                        .map(|utterance| utterance.main),
                }
                .unwrap_or_else(|| panic!("{route:?} origin {origin}: {:?}", errors.to_vec()));
                assert!(
                    errors.to_vec().is_empty(),
                    "{route:?}: {:?}",
                    errors.to_vec()
                );
                let events: Vec<_> = main
                    .content
                    .content
                    .iter()
                    .filter_map(|item| match item {
                        UtteranceContent::OtherSpokenEvent(event) => Some(event),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    events
                        .iter()
                        .map(|event| event.word().raw_text())
                        .collect::<Vec<_>>(),
                    expected,
                    "{route:?} origin {origin}"
                );
                for event in events {
                    let span = event.location.span().unwrap();
                    assert_eq!(
                        &source[span.start() - origin..span.end() - origin],
                        event.to_chat_string(),
                        "{route:?} event source"
                    );
                }
                let Some(Terminator::Period { location }) = main.content.terminator else {
                    panic!("{route:?}: interposed word consumed the utterance terminator");
                };
                let span = location.span().unwrap();
                assert_eq!(&source[span.start() - origin..span.end() - origin], ".");
            }
        }
    }
}

#[test]
fn authored_interposed_word_prefixes_and_suffixes_retain_their_source() {
    check(&talkbank_parser_re2c::Re2cParser::new());
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
}
