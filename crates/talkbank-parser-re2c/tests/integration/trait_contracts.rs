//! Optional public-trait controls require diagnostic-free admission before
//! inspecting a returned model. Recovery is a different outcome.

use talkbank_model::{ErrorCollector, ParseError, ParseOutcome};

/// A model paired with an observed empty diagnostic batch.
/// Only this consuming admission can construct the clean test state.
struct CleanParse<T>(T);

#[derive(Debug, thiserror::Error)]
enum UncleanParse {
    #[error("parse emitted diagnostics: {0:?}")]
    Diagnostics(Vec<ParseError>),
    #[error("parse rejected the input without diagnostics")]
    Rejected,
}

impl<T> CleanParse<T> {
    fn run(parse: impl FnOnce(&ErrorCollector) -> ParseOutcome<T>) -> Result<Self, UncleanParse> {
        let errors = ErrorCollector::new();
        let result = parse(&errors);
        let diagnostics = errors.into_vec();
        if !diagnostics.is_empty() {
            return Err(UncleanParse::Diagnostics(diagnostics));
        }
        match result {
            ParseOutcome::Parsed(model) => Ok(Self(model)),
            ParseOutcome::Rejected => Err(UncleanParse::Rejected),
        }
    }

    fn into_model(self) -> T {
        self.0
    }
}

#[test]
fn trait_parse_mor_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result =
        CleanParse::run(|errors| p.parse_mor_tier("pro|I v|want n|cookie-PL .\n", 0, errors))
            .expect("diagnostic-free trait parse");
    let tier = result.into_model();
    assert_eq!(tier.items().len(), 3);
}

#[test]
fn trait_parse_gra_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result =
        CleanParse::run(|errors| p.parse_gra_tier("1|2|SUBJ 2|0|ROOT 3|2|OBJ\n", 0, errors))
            .expect("diagnostic-free trait parse");
    let tier = result.into_model();
    assert_eq!(tier.relations().len(), 3);
}

#[test]
fn trait_parse_id_header() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| {
        p.parse_id_header(
            "@ID:\teng|corpus|CHI|3;00.|female|typical||Child|||\n",
            0,
            errors,
        )
    })
    .expect("diagnostic-free trait parse");
    let id = result.into_model();
    assert_eq!(id.speaker.to_string(), "CHI");
}

#[test]
fn trait_parse_mor_word() {
    use talkbank_model::errors::ErrorCollector;
    use talkbank_model::{ChatParser, SemanticEq, Severity};
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    // A word body is contiguous; a newline is a tier boundary, not part of it.
    let input = "verb|want-Fin-Ind-Pres";
    let result = p.parse_mor_word(input, 0, &errors);
    assert!(result.is_parsed());
    let word = result.into_option().unwrap();
    assert_eq!(word.pos.as_str(), "verb");
    assert_eq!(word.lemma.as_str(), "want");
    assert!(errors.into_vec().is_empty());
    let canonical = talkbank_parser::TreeSitterParser::new().unwrap();
    let canonical_errors = ErrorCollector::new();
    let canonical_word = canonical
        .parse_mor_word(input, 0, &canonical_errors)
        .into_option()
        .unwrap();
    assert!(canonical_errors.into_vec().is_empty());
    assert!(word.semantic_eq(&canonical_word));
    {
        let input = format!("{input}\n");
        let native_errors = ErrorCollector::new();
        let canonical_errors = ErrorCollector::new();
        p.parse_mor_word(&input, 0, &native_errors);
        canonical.parse_mor_word(&input, 0, &canonical_errors);
        assert!(
            native_errors
                .into_vec()
                .iter()
                .any(|error| error.severity == Severity::Error)
        );
        assert!(
            canonical_errors
                .into_vec()
                .iter()
                .any(|error| error.severity == Severity::Error)
        );
    }
}

#[test]
fn trait_parse_gra_relation() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| p.parse_gra_relation("1|2|SUBJ\n", 0, errors))
        .expect("diagnostic-free trait parse");
    let rel = result.into_model();
    assert_eq!(rel.index, 1);
    assert_eq!(rel.head, 2);
}

#[test]
fn trait_parse_main_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| p.parse_main_tier("*CHI:\thello world .\n", 0, errors))
        .expect("diagnostic-free trait parse");
    let mt = result.into_model();
    assert_eq!(mt.speaker.to_string(), "CHI");
}

#[test]
fn trait_parse_word() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let _result = CleanParse::run(|errors| p.parse_word("hello", 0, errors))
        .expect("diagnostic-free trait parse");
}

#[test]
fn trait_parse_word_compound() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| p.parse_word("ice+cream", 0, errors))
        .expect("diagnostic-free trait parse");
    let word = result.into_model();
    assert!(word.raw_text().contains("+"));
}

#[test]
fn trait_parse_act_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let _result =
        CleanParse::run(|errors| p.parse_act_tier("CHI is playing with blocks\n", 0, errors))
            .expect("diagnostic-free trait parse");
}

#[test]
fn trait_parse_com_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let _result =
        CleanParse::run(|errors| p.parse_com_tier("child is waving to camera\n", 0, errors))
            .expect("diagnostic-free trait parse");
}

#[test]
fn trait_parse_text_tier_with_bullet() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| {
        p.parse_act_tier("hello \u{0015}100_200\u{0015} world\n", 0, errors)
    })
    .expect("diagnostic-free trait parse");
    let tier = result.into_model();
    assert!(!tier.content.segments.is_empty());
}

#[test]
fn trait_parse_all_text_tiers() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let input = "some text content\n";
    CleanParse::run(|errors| p.parse_act_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_cod_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_com_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_exp_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_add_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_gpx_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_int_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_spa_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
    CleanParse::run(|errors| p.parse_sit_tier(input, 0, errors))
        .expect("diagnostic-free text tier");
}

#[test]
fn trait_parse_pho_tier() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| p.parse_pho_tier("hɛloʊ wɝld\n", 0, errors))
        .expect("diagnostic-free trait parse");
    let tier = result.into_model();
    assert_eq!(tier.items.len(), 2);
}

#[test]
fn trait_parse_pho_word() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let _result = CleanParse::run(|errors| p.parse_pho_word("hɛloʊ\n", 0, errors))
        .expect("diagnostic-free trait parse");
}

#[test]
fn trait_parse_chat_file() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let input = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    let result = CleanParse::run(|errors| p.parse_chat_file(input, 0, errors))
        .expect("diagnostic-free trait parse");
    let file = result.into_model();
    assert!(!file.lines.is_empty());
}

#[test]
fn recovered_file_cannot_enter_clean_admission() {
    use talkbank_model::{ChatParser, ErrorCode, Severity};
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E522.md")
        .unwrap();
    // The authored second example declares a participant without an @ID.
    let source = spec.examples()[1].chat.as_str();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| {
        let recovered = parser.parse_chat_file(source, 0, errors);
        assert!(
            recovered.is_parsed(),
            "exercise recovery, not model refusal"
        );
        recovered
    });
    let UncleanParse::Diagnostics(diagnostics) = result.err().expect("recovery is not clean")
    else {
        panic!("the recovered model must be refused for its diagnostics");
    };
    assert!(diagnostics.iter().any(|error| {
        error.code == ErrorCode::SpeakerNotDefined && error.severity == Severity::Error
    }));
    assert!(
        diagnostics
            .iter()
            .all(|error| error.code != ErrorCode::InternalError)
    );
}

#[test]
fn trait_parse_header() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let _result = CleanParse::run(|errors| p.parse_header("@Comment:\tsome text\n", 0, errors))
        .expect("diagnostic-free trait parse");
}

#[test]
fn trait_parse_utterance() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let result = CleanParse::run(|errors| {
        p.parse_utterance("*CHI:\thello .\n%mor:\tn|hello .\n", 0, errors)
    })
    .expect("diagnostic-free trait parse");
    let utt = result.into_model();
    assert_eq!(utt.main.speaker.to_string(), "CHI");
    assert!(!utt.dependent_tiers.is_empty());
}

#[test]
fn trait_parser_name() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    assert_eq!(p.parser_name(), "Re2cParser");
}
