// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

use talkbank_model::ChatOptionFlag;
use talkbank_model::ErrorCollector;
use talkbank_model::FragmentSemanticContext;
use talkbank_parser::TreeSitterParser;

fn parser() -> TreeSitterParser {
    TreeSitterParser::new().expect("grammar loads")
}

#[test]
fn direct_fragment_spans_do_not_subtract_a_nonexistent_wrapper() {
    let p = parser();
    for offset in [0, 200, 1000] {
        let errors = ErrorCollector::new();
        let word = p
            .parse_word_fragment("café", offset, &errors)
            .into_option()
            .unwrap();
        assert_eq!(
            word.span,
            talkbank_model::Span::from_usize(offset, offset + "café".len())
        );
        let main = p
            .parse_main_tier_fragment("*CHI:\thello .", offset, &errors)
            .into_option()
            .unwrap();
        assert_eq!(
            main.speaker_span,
            talkbank_model::Span::from_usize(offset + 1, offset + 4)
        );
        assert!(errors.is_empty());
    }
}

#[test]
fn direct_fragment_diagnostics_add_the_document_origin() {
    let p = parser();
    check_fragment_diagnostics("hello@@", |offset, errors| {
        assert!(
            p.parse_word_fragment("hello@@", offset, errors)
                .is_rejected()
        );
    });
    check_fragment_diagnostics("*CHI:\thello } .", |offset, errors| {
        assert!(
            p.parse_main_tier_fragment("*CHI:\thello } .", offset, errors)
                .is_rejected()
        );
    });
}

#[test]
fn header_fragment_must_account_for_the_whole_input() {
    let p = parser();
    for input in [
        "@Languages:\teng\n@Comment:\tsecond header",
        "@Comment:\tfirst header\n@Date:\t01-JAN-2020",
    ] {
        let errors = ErrorCollector::new();
        assert!(
            p.parse_header_fragment(input, 200, &errors).is_rejected(),
            "accepted only part of {input:?}"
        );
        assert!(!errors.is_empty());
    }
}

#[test]
fn complete_header_fragment_accepts_folded_content_and_line_endings() {
    let p = parser();
    for input in [
        "@Languages:\teng\n",
        "@Comment:\tcafé\r\n",
        "@Comment:\tfirst line\n\tcontinued content",
    ] {
        let errors = ErrorCollector::new();
        assert!(
            p.parse_header_fragment(input, 200, &errors).is_parsed(),
            "{input:?}"
        );
        assert!(errors.is_empty());
    }
}

#[test]
fn unlocated_header_reports_the_callers_source_and_origin() {
    let p = parser();
    // This malformed line prevents the wrapper's header lookup from finding
    // the requested CST node. That failure still belongs to this input.
    let input = format!("@Languages:\t{}a", "eng, ".repeat(60));
    for offset in [0, 200] {
        let errors = ErrorCollector::new();
        assert!(
            p.parse_header_fragment(&input, offset, &errors)
                .is_rejected()
        );
        let errors = errors.into_vec();
        assert!(!errors.is_empty());
        for error in errors {
            assert_eq!(
                error.location.span,
                talkbank_model::Span::from_usize(offset, offset + input.len())
            );
            let context = error.context.expect("diagnostic source");
            assert_eq!(context.source_text, input);
            assert_eq!(context.span, talkbank_model::Span::from(0..input.len()));
        }
    }
}

fn check_fragment_diagnostics(input: &str, parse: impl Fn(usize, &ErrorCollector)) {
    let local = ErrorCollector::new();
    parse(0, &local);
    let local = local.into_vec();
    assert!(!local.is_empty());
    let document = ErrorCollector::new();
    parse(200, &document);
    let document = document.into_vec();
    assert_eq!(local.len(), document.len());
    for (local, document) in local.iter().zip(&document) {
        assert!(local.location.span.end as usize <= input.len(), "{local:?}");
        assert_eq!(local.code, document.code);
        assert_eq!(
            document.location.span.start,
            local.location.span.start + 200
        );
        assert_eq!(document.location.span.end, local.location.span.end + 200);
        assert_eq!(document.context, local.context);
        if let Some(context) = &local.context {
            assert!(context.span.end as usize <= context.source_text.len());
        }
    }
}

#[test]
fn utterance_fragment_diagnostics_remove_only_the_real_wrapper() {
    let p = parser();
    let input = "*CHI:\thello } .";
    check_fragment_diagnostics(input, |offset, errors| {
        assert!(
            p.parse_utterance_fragment(input, offset, errors)
                .is_rejected()
        );
    });
}

#[test]
fn header_fragment_diagnostics_add_the_document_origin() {
    let p = parser();
    let input = "@Participants:\tCHI";
    check_fragment_diagnostics(input, |offset, errors| {
        assert!(p.parse_header_fragment(input, offset, errors).is_rejected());
    });
}

#[test]
fn participant_fragment_diagnostics_remove_only_the_real_wrapper() {
    let p = parser();
    let input = "CHI";
    check_fragment_diagnostics(input, |offset, errors| {
        assert!(
            p.parse_participant_entry_fragment(input, offset, errors)
                .is_rejected()
        );
    });
}

#[test]
fn dependent_fragment_diagnostics_remove_only_the_real_wrapper() {
    let p = parser();
    let input = "|we v|go .";
    check_fragment_diagnostics(input, |offset, errors| {
        assert!(
            p.parse_mor_tier_fragment(input, offset, errors)
                .is_rejected()
        );
    });
    let errors = ErrorCollector::new();
    p.parse_mor_tier_fragment(input, 200, &errors);
    let errors = errors.into_vec();
    let malformed = errors
        .iter()
        .find(|e| e.code == talkbank_model::ErrorCode::MorItemEmptyPos)
        .unwrap();
    assert_eq!(
        malformed.location.span,
        talkbank_model::Span::from_usize(200, 203)
    );
}

#[test]
fn public_main_tier_wrapper_rejects_ca_fragment_without_context() {
    let p = parser();
    let errors = ErrorCollector::new();
    let result = p.parse_main_tier_fragment("*CHI:\t(word) .", 0, &errors);
    assert!(result.into_option().is_none() || !errors.is_empty());
}

#[test]
fn public_main_tier_wrapper_accepts_ca_fragment_with_context() {
    let p = parser();
    let context = FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca);
    let errors = ErrorCollector::new();
    let result = p.parse_main_tier_fragment_with_context("*CHI:\t(word) .", 0, &context, &errors);
    assert!(result.into_option().is_some());
}

#[test]
fn public_utterance_wrapper_rejects_ca_fragment_without_context() {
    let p = parser();
    let errors = ErrorCollector::new();
    let result = p.parse_utterance_fragment("*CHI:\t(word) .\n%mor:\tv|(word) .\n", 0, &errors);
    assert!(result.into_option().is_none() || !errors.is_empty());
}

#[test]
fn public_utterance_wrapper_accepts_ca_fragment_with_context() {
    let p = parser();
    let context = FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca);
    let errors = ErrorCollector::new();
    let result = p.parse_utterance_fragment_with_context("*CHI:\t(word) .\n", 0, &context, &errors);
    assert!(result.into_option().is_some());
}

#[test]
fn utterance_coordinates_follow_the_actual_source_form() {
    let p = parser();
    for input in [
        "*CHI:\thello .",
        "*CHI:\thello .\n",
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n*CHI:\thello .\n@End\n",
    ] {
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let utterance = p
                .parse_utterance_fragment(input, offset, &errors)
                .into_option()
                .unwrap_or_else(|| panic!("{input:?}: {:?}", errors.into_vec()));
            let speaker_start = input.find("CHI:\t").unwrap();
            assert_eq!(
                utterance.main.speaker_span,
                talkbank_model::Span::from_usize(
                    offset + speaker_start,
                    offset + speaker_start + 3
                )
            );
        }
    }
}

#[test]
fn main_tier_fragment_spans_end_with_the_callers_source() {
    let p = parser();
    for input in ["*CHI:\thello .", "*CHI:\thello .\n", "*CHI:\thello .\r\n"] {
        for offset in [0, 200] {
            let errors = ErrorCollector::new();
            let main = p
                .parse_main_tier_fragment(input, offset, &errors)
                .into_option()
                .unwrap();
            assert_eq!(
                main.span,
                talkbank_model::Span::from_usize(offset, offset + input.len()),
                "{input:?}"
            );
            assert_eq!(
                main.content.content_span.unwrap().end as usize,
                offset + input.len()
            );
            assert!(errors.is_empty());
        }
    }
}

#[test]
fn main_tier_fragment_rejects_unconsumed_source() {
    let p = parser();
    for input in [
        "*CHI:\thello .\ntrailing garbage\n",
        "*CHI:\thello .\n*CHI:\tmore .\n",
    ] {
        let errors = ErrorCollector::new();
        assert!(
            p.parse_main_tier_fragment(input, 0, &errors).is_rejected(),
            "{input:?}"
        );
        assert!(!errors.is_empty());
    }
}
