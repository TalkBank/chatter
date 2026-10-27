//! Language precodes retain their admitted value and exact source location.

use talkbank_model::{ChatParser, ErrorCollector, Span};

struct ExpectedPrecodes {
    main: usize,
    wor: usize,
}

fn check(parser: &impl ChatParser, source: &str, origin: usize, expected: &ExpectedPrecodes) {
    let errors = ErrorCollector::new();
    let file = parser
        .parse_chat_file(source, origin, &errors)
        .into_option()
        .unwrap();
    assert!(errors.is_empty(), "{:?}", errors.to_vec());
    let mut observed = 0;
    let mut wor_precodes = 0;
    for utterance in file.utterances() {
        let Some(precode) = &utterance.main.content.language_code else {
            continue;
        };
        observed += 1;
        assert_eq!(precode.code.as_str(), "zho");
        let span = precode.location.span().unwrap();
        assert_eq!(
            &source[span.start() - origin..span.end() - origin],
            "[- zho]"
        );

        let line_span = utterance.main.location.span().unwrap();
        let input = &source[line_span.start() - origin..line_span.end() - origin];
        let fragment = parser
            .parse_main_tier(input, line_span.start(), &errors)
            .into_option()
            .unwrap();
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        assert_eq!(fragment.content.language_code.as_ref(), Some(precode));
        let precode_start = line_span.start() + input.find("[- zho]").unwrap();
        assert_eq!(
            span,
            Span::new(precode_start, precode_start + "[- zho]".len())
        );
        if let Some(wor) = utterance.wor_tier() {
            wor_precodes += 1;
            assert_eq!(wor.language_code.as_ref(), Some(&precode.code));
        }
    }
    assert_eq!(observed, expected.main);
    assert_eq!(wor_precodes, expected.wor);
}

#[test]
fn reference_precodes_keep_file_and_fragment_locations() {
    let (tree, native) = super::both_parsers();
    for (reference, expected) in [
        (
            include_str!("../../../../../corpus/reference/content/language-switching.cha"),
            ExpectedPrecodes { main: 2, wor: 0 },
        ),
        (
            include_str!("../../../../../corpus/reference/tiers/wor.cha"),
            ExpectedPrecodes { main: 1, wor: 1 },
        ),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = reference.replace('\n', newline);
            for origin in [0, 73] {
                check(&tree, &source, origin, &expected);
                check(&native, &source, origin, &expected);
            }
        }
    }
}
