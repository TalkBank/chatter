//! Canonical word combinations travel admitted source, typed content and policy.
use super::*;
use talkbank_model::alignment::helpers::{TierDomain, WordItem, counts_for_tier, walk_words};
use talkbank_model::model::{
    CADelimiterType, ChatFile, FormType, SemanticEq, TranscriptName, Word, WordCategory,
    WordContent, WordLanguageMarker, WordStressMarkerType,
};
use talkbank_model::{ErrorCode, ErrorCollector};

/// Only an error-free parsed and validated file supplies control spans.
struct AdmittedFile<'source> {
    source: &'source str,
    file: ChatFile,
}
impl<'source> AdmittedFile<'source> {
    fn parse(source: &'source str) -> Self {
        let parser = TreeSitterParser::new().expect("parser");
        let mut file = strict_parse(parser.parse_chat_file(source)).expect("clean control parse");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(
            errors.is_empty(),
            "control diagnostics: {:?}",
            errors.to_vec()
        );
        Self { source, file }
    }
    fn words(&self) -> Vec<(&Word, Wrapper)> {
        let mut words = Vec::new();
        for utterance in self.file.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => words.push((word, Wrapper::Plain)),
                    WordItem::ReplacedWord(word) => words.push((&word.word, Wrapper::Replaced)),
                    WordItem::Separator(_) => panic!("authored control has no separators"),
                },
            );
        }
        words
    }
    fn word(&self, spelling: &str) -> &Word {
        let words: Vec<_> = self
            .words()
            .into_iter()
            .filter(|(word, _)| {
                self.source.get(
                    word.location.span().expect("located").start()
                        ..word.location.span().expect("located").end(),
                ) == Some(spelling)
            })
            .collect();
        assert_eq!(words.len(), 1, "unique authored target: {spelling}");
        words[0].0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Wrapper {
    Plain,
    Replaced,
}
#[derive(Debug, PartialEq, Eq)]
enum Element<'text> {
    Text(&'text str),
    Phonetic(&'text str),
    Shortening(&'text str),
    Lengthening(usize),
    Clitic,
    Compound,
    Pause,
    Stress(WordStressMarkerType),
    Overlap,
    Delimiter(CADelimiterType),
}
#[derive(Clone, Copy)]
enum Language {
    None,
    Shortcut,
    Explicit(&'static str),
}
struct ExpectedWord {
    spelling: &'static str,
    content: Vec<Element<'static>>,
    category: Option<WordCategory>,
    form: Option<FormType>,
    language: Language,
    pos: Option<&'static str>,
    wrapper: Wrapper,
}
impl ExpectedWord {
    fn new(spelling: &'static str, content: Vec<Element<'static>>) -> Self {
        Self {
            spelling,
            content,
            category: None,
            form: None,
            language: Language::None,
            pos: None,
            wrapper: Wrapper::Plain,
        }
    }
    fn category(mut self, category: WordCategory) -> Self {
        self.category = Some(category);
        self
    }
    fn form(mut self, form: FormType) -> Self {
        self.form = Some(form);
        self
    }
    fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }
    fn pos(mut self, pos: &'static str) -> Self {
        self.pos = Some(pos);
        self
    }
    fn replaced(mut self) -> Self {
        self.wrapper = Wrapper::Replaced;
        self
    }
}

fn assert_reference(name: &str, expected: Vec<ExpectedWord>) {
    let source = std::fs::read_to_string(
        workspace_root().join(format!("corpus/reference/word-features/{name}.cha")),
    )
    .expect("authored reference");
    let admitted = AdmittedFile::parse(&source);
    let words = admitted.words();
    assert_eq!(words.len(), expected.len());
    for ((word, wrapper), expected) in words.into_iter().zip(expected) {
        let actual: Vec<_> = word
            .content()
            .iter()
            .map(|item| match item {
                WordContent::Text(text) => Element::Text(text.as_ref()),
                WordContent::Phonetic(text) => Element::Phonetic(text.as_ref()),
                WordContent::Shortening(text) => Element::Shortening(text.as_ref()),
                WordContent::Lengthening(value) => Element::Lengthening(value.count.get()),
                WordContent::CliticBoundary(_) => Element::Clitic,
                WordContent::CompoundMarker(_) => Element::Compound,
                WordContent::SyllablePause(_) => Element::Pause,
                WordContent::StressMarker(value) => Element::Stress(value.marker_type),
                WordContent::OverlapPoint(_) => Element::Overlap,
                WordContent::CADelimiter(value) => Element::Delimiter(value.delimiter_type),
                WordContent::CAElement(_)
                | WordContent::UnderlineBegin(_)
                | WordContent::UnderlineEnd(_) => panic!(
                    "unexpected typed content in {}: {item:?}",
                    expected.spelling
                ),
            })
            .collect();
        assert_eq!(actual, expected.content, "{}", expected.spelling);
        assert_eq!(word.category, expected.category, "{}", expected.spelling);
        assert_eq!(
            word.form_type().cloned(),
            expected.form,
            "{}",
            expected.spelling
        );
        assert_eq!(word.part_of_speech.as_deref(), expected.pos);
        assert_eq!(wrapper, expected.wrapper);
        match (word.language_marker(), expected.language) {
            (None, Language::None) | (Some(WordLanguageMarker::Shortcut), Language::Shortcut) => {}
            (Some(WordLanguageMarker::Explicit(code)), Language::Explicit(expected)) => {
                assert_eq!(code.as_str(), expected)
            }
            _ => panic!(
                "unexpected language marker in {}: {:?}",
                expected.spelling,
                word.language_marker().cloned()
            ),
        }
        assert_eq!(
            source.get(
                word.location.span().expect("located").start()
                    ..word.location.span().expect("located").end()
            ),
            Some(expected.spelling)
        );
        assert_eq!(
            word.to_chat_string(),
            expected.spelling,
            "markers retain their parenthesis side"
        );
        let policy = match expected.category {
            Some(WordCategory::Filler) => [false, true, true, true],
            Some(WordCategory::Nonword | WordCategory::PhonologicalFragment) => {
                [false, false, true, true]
            }
            Some(WordCategory::Omission | WordCategory::CAOmission) => [false; 4],
            None => [true; 4],
        };
        for (domain, counts) in [
            TierDomain::Mor,
            TierDomain::Wor,
            TierDomain::Pho,
            TierDomain::Sin,
        ]
        .into_iter()
        .zip(policy)
        {
            assert_eq!(
                counts_for_tier(word, domain),
                counts,
                "{}: {domain:?}",
                expected.spelling
            );
        }
    }
    assert_eq!(admitted.file.to_chat_string(), source);
}

#[test]
fn sound_categories_preserve_features_without_changing_tier_policy() {
    use CADelimiterType::Creaky;
    use Element::{Delimiter as D, Lengthening as L, Overlap as O, Pause as P, Text as T};
    use WordCategory::{Filler as F, Nonword as N};
    let w = ExpectedWord::new;
    assert_reference(
        "sound-category-combinations",
        vec![
            w("&-hmm:", vec![T("hmm"), L(1)]).category(F),
            w("&-h:um", vec![T("h"), L(1), T("um")]).category(F),
            w("&-erm@b", vec![T("erm")]).category(F).form(FormType::B),
            w("⌈anchor", vec![O, T("anchor")]),
            w("&-uh⌉", vec![T("uh"), O]).category(F),
            w("&~baba@b", vec![T("baba")]).category(N).form(FormType::B),
            w("&~ba:", vec![T("ba"), L(1)]).category(N),
            w(
                "&~ba:ba:bu:",
                vec![T("ba"), L(1), T("ba"), L(1), T("bu"), L(1)],
            )
            .category(N),
            w("&~ba:ba:bu", vec![T("ba"), L(1), T("ba"), L(1), T("bu")]).category(N),
            w("&~ba:ba:", vec![T("ba"), L(1), T("ba"), L(1)]).category(N),
            w("&~ba^ba", vec![T("ba"), P, T("ba")]).category(N),
            w("&~ba^ba^ba", vec![T("ba"), P, T("ba"), P, T("ba")]).category(N),
            w(
                "&~ba^ba^ba^ba^ba",
                vec![T("ba"), P, T("ba"), P, T("ba"), P, T("ba"), P, T("ba")],
            )
            .category(N),
            w(
                "&~ba⁎ba:⁎",
                vec![T("ba"), D(Creaky), T("ba"), L(1), D(Creaky)],
            )
            .category(N),
        ],
    );
}

#[test]
fn ca_omission_markers_stay_outside_the_lexical_parentheses() {
    use CADelimiterType::Faster;
    use Element::{Delimiter as D, Overlap as O, Text as T};
    let w = ExpectedWord::new;
    assert_reference(
        "ca-omission-combinations",
        vec![
            w("⌈(maybe)", vec![O, T("maybe")]).category(WordCategory::CAOmission),
            w("(soon)⌉", vec![T("soon"), O]).category(WordCategory::CAOmission),
            w("⌈(later)⌉", vec![O, T("later"), O]).category(WordCategory::CAOmission),
            w("∆(near)", vec![D(Faster), T("near")]).category(WordCategory::CAOmission),
            w("(far)∆", vec![T("far"), D(Faster)]).category(WordCategory::CAOmission),
        ],
    );
}

#[test]
fn repeated_segments_suffixes_and_phonetic_folding_preserve_every_leaf() {
    use Element::{
        Clitic as C, Compound as M, Lengthening as L, Overlap as O, Pause as P, Phonetic as U,
        Shortening as H, Stress as S, Text as T,
    };
    use WordStressMarkerType::{Primary, Secondary};
    let w = ExpectedWord::new;
    assert_reference(
        "segment-metadata-combinations",
        vec![
            w("ra~mi~na", vec![T("ra"), C, T("mi"), C, T("na")]),
            w(
                "ra~mi~na~lo",
                vec![T("ra"), C, T("mi"), C, T("na"), C, T("lo")],
            ),
            w(
                "ra~mi~na~lo~su",
                vec![T("ra"), C, T("mi"), C, T("na"), C, T("lo"), C, T("su")],
            ),
            w(
                "ra~mi~na~lo~su~te",
                vec![
                    T("ra"),
                    C,
                    T("mi"),
                    C,
                    T("na"),
                    C,
                    T("lo"),
                    C,
                    T("su"),
                    C,
                    T("te"),
                ],
            ),
            w(
                "ra~mi~na~lo~su~te~vi",
                vec![
                    T("ra"),
                    C,
                    T("mi"),
                    C,
                    T("na"),
                    C,
                    T("lo"),
                    C,
                    T("su"),
                    C,
                    T("te"),
                    C,
                    T("vi"),
                ],
            ),
            w(
                "sun+flower+seed",
                vec![T("sun"), M, T("flower"), M, T("seed")],
            ),
            w(
                "rain+water+storage+tank",
                vec![T("rain"), M, T("water"), M, T("storage"), M, T("tank")],
            ),
            w(
                "blue+green+garden+stone+path",
                vec![
                    T("blue"),
                    M,
                    T("green"),
                    M,
                    T("garden"),
                    M,
                    T("stone"),
                    M,
                    T("path"),
                ],
            ),
            w(
                "red+blue+green+flow(er)+seed+box",
                vec![
                    T("red"),
                    M,
                    T("blue"),
                    M,
                    T("green"),
                    M,
                    T("flow"),
                    H("er"),
                    M,
                    T("seed"),
                    M,
                    T("box"),
                ],
            ),
            w(
                "vo^ca^bu^la",
                vec![T("vo"), P, T("ca"), P, T("bu"), P, T("la")],
            ),
            w("vo^ca^bu", vec![T("vo"), P, T("ca"), P, T("bu")]).replaced(),
            w("vo^ca", vec![T("vo"), P, T("ca")]).replaced(),
            w("chir^rup@o", vec![T("chir"), P, T("rup")]).form(FormType::O),
            w("chir^rup^ing@o", vec![T("chir"), P, T("rup"), P, T("ing")]).form(FormType::O),
            w(
                "chir^rup^ing^ly@o",
                vec![T("chir"), P, T("rup"), P, T("ing"), P, T("ly")],
            )
            .form(FormType::O),
            w("ri::ver", vec![T("ri"), L(2), T("ver")]),
            w("ri:::ver", vec![T("ri"), L(3), T("ver")]),
            w(
                "ri:ver:bank:",
                vec![T("ri"), L(1), T("ver"), L(1), T("bank"), L(1)],
            ),
            w("ri:ver:", vec![T("ri"), L(1), T("ver"), L(1)]),
            w(
                "ri:ver:bank",
                vec![T("ri"), L(1), T("ver"), L(1), T("bank")],
            ),
            w("chir:rup@o", vec![T("chir"), L(1), T("rup")]).form(FormType::O),
            w("chirrup:@o", vec![T("chirrup"), L(1)]).form(FormType::O),
            w(
                "chir:rup:ing@o",
                vec![T("chir"), L(1), T("rup"), L(1), T("ing")],
            )
            .form(FormType::O),
            w("chir:rup:@o", vec![T("chir"), L(1), T("rup"), L(1)]).form(FormType::O),
            w("chi::rp@o", vec![T("chi"), L(2), T("rp")]).form(FormType::O),
            w("chirp::@o", vec![T("chirp"), L(2)]).form(FormType::O),
            w("ˈriver", vec![S(Primary), T("river")]),
            w("ˈriver:", vec![S(Primary), T("river"), L(1)]),
            w("reˈmark", vec![T("re"), S(Primary), T("mark")]).replaced(),
            w("ˈriˌver", vec![S(Primary), T("ri"), S(Secondary), T("ver")]),
            w("⌈ˈriver", vec![O, S(Primary), T("river")]),
            w("boundary⌉", vec![T("boundary"), O]),
            w("⌈(re)mark", vec![O, H("re"), T("mark")]),
            w("boundary⌉", vec![T("boundary"), O]),
            w("⌈(re)ma::rk", vec![O, H("re"), T("ma"), L(2), T("rk")]),
            w("boundary⌉", vec![T("boundary"), O]),
            w("chirrup@o$n", vec![T("chirrup")])
                .form(FormType::O)
                .pos("n"),
            w("café@s:spa$n", vec![T("café")])
                .language(Language::Explicit("spa"))
                .pos("n"),
            w("murmur@s$n", vec![T("murmur")])
                .language(Language::Shortcut)
                .pos("n"),
            w("sol+luna@s:spa$n", vec![T("sol"), M, T("luna")])
                .language(Language::Explicit("spa"))
                .pos("n"),
            w("con~ella@s:spa$pro", vec![T("con"), C, T("ella")])
                .language(Language::Explicit("spa"))
                .pos("pro"),
            w("cari(ño)@s:spa$n", vec![T("cari"), H("ño")])
                .language(Language::Explicit("spa"))
                .pos("n"),
            w("anchor", vec![T("anchor")]),
            w("bo^nito@s:spa", vec![T("bo"), P, T("nito")]).language(Language::Explicit("spa")),
            w("sol+luna@s:spa", vec![T("sol"), M, T("luna")]).language(Language::Explicit("spa")),
            w("anchor", vec![T("anchor")]),
            w("tʃɛ:@u", vec![U("tʃɛ:")]).form(FormType::U),
            w("ˌtʃ:ɛ~ɪ+mə^n(ə)↑@u$n", vec![U("ˌtʃ:ɛ~ɪ+mə^n(ə)↑")])
                .form(FormType::U)
                .pos("n"),
        ],
    );
}

#[derive(Clone, Copy)]
enum Mutation {
    Append(&'static str),
    Word(&'static str),
}
fn assert_policy_pairs(spec: &str, diagnostic: ErrorCode, targets: &[(&str, Mutation)]) {
    let parser = TreeSitterParser::new().expect("parser");
    let source_for = |index| {
        std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}_{index}.cha",
        )))
        .expect("canonical policy specimen")
    };
    for (pair, (spelling, mutation)) in targets.iter().enumerate() {
        let source = source_for(2 * pair + 2);
        let admitted = AdmittedFile::parse(&source);
        let word = admitted.word(spelling);
        assert_eq!(admitted.file.to_chat_string(), source);
        let start = match mutation {
            Mutation::Append(_) => word.location.span().expect("located").end(),
            Mutation::Word(_) => word.location.span().expect("located").start(),
        };
        let end = word.location.span().expect("located").end();
        let inserted = match mutation {
            Mutation::Append(text) | Mutation::Word(text) => text,
        };
        let mutant_source = source_for(2 * pair + 1);
        assert_eq!(
            format!(
                "{}{}{}",
                source.get(..start).expect("UTF-8 prefix"),
                inserted,
                source.get(end..).expect("UTF-8 suffix")
            ),
            mutant_source,
            "{spec}: only the target changes"
        );
        let parse_errors = ErrorCollector::new();
        let mut mutated = parser.parse_chat_file_streaming(&mutant_source, &parse_errors);
        if matches!(mutation, Mutation::Append(_)) {
            let mut replacements = Vec::new();
            for utterance in mutated.utterances() {
                walk_words(&utterance.main.content.content, None, &mut |item| {
                    if let WordItem::ReplacedWord(replaced) = item {
                        replacements.push(&replaced.word);
                    }
                });
            }
            assert_eq!(replacements.len(), 1);
            assert!(replacements[0].semantic_eq(word));
            assert_eq!(replacements[0].location, word.location);
        }
        let errors = ErrorCollector::new();
        mutated.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        assert!(
            parse_errors.is_empty(),
            "{spec}: {:?}",
            parse_errors.to_vec()
        );
        assert_eq!(findings.len(), 1, "{spec}: {findings:?}");
        assert_eq!(findings[0].code, diagnostic);
        assert_eq!(mutated.to_chat_string(), mutant_source);
    }
}

#[test]
fn category_feature_specs_keep_replacement_policy_on_the_produced_word() {
    use Mutation::Append as A;
    assert_policy_pairs(
        "E388_sound_features",
        ErrorCode::ReplacementOnNonword,
        &[
            ("&~baba@b", A(" [: babble]")),
            ("&~ba:", A(" [: babble]")),
            ("&~ba:ba:bu:", A(" [: babble]")),
            ("&~ba^ba", A(" [: babble]")),
            ("&~ba^ba^ba", A(" [: babble]")),
            ("&~ba^ba^ba^ba^ba", A(" [: babble]")),
            ("&~ba⁎ba:⁎", A(" [: babble]")),
        ],
    );
    assert_policy_pairs(
        "E389_sound_features",
        ErrorCode::ReplacementOnFiller,
        &[
            ("&-hmm:", A(" [: pause]")),
            ("&-h:um", A(" [: pause]")),
            ("&-erm@b", A(" [: pause]")),
            ("&-uh⌉", A(" [: pause]")),
        ],
    );
}

#[test]
fn marker_omission_specs_keep_the_single_shortening_normalization_boundary() {
    use Mutation::Word as W;
    assert_policy_pairs(
        "E209_marker_omissions",
        ErrorCode::EmptySpokenContent,
        &[
            ("⌈(maybe)", W("⌈(ma)(ybe)")),
            ("⌈(later)⌉", W("⌈(la)(ter)⌉")),
            ("∆(near)", W("∆(ne)(ar)")),
        ],
    );
}

#[test]
fn lengthening_and_suffix_specs_preserve_their_independent_boundaries() {
    use Mutation::Word as W;
    assert_policy_pairs(
        "E246_multiple_lengthening",
        ErrorCode::LengtheningNotAfterSpokenMaterial,
        &[
            ("va::se", W("(va)::se")),
            ("va:::se", W("(va):::se")),
            ("va::se@b", W("(va)::se@b")),
            ("&~ba::ba", W("&~(ba)::ba")),
        ],
    );
    assert_policy_pairs(
        "E203_feature_suffixes",
        ErrorCode::InvalidFormType,
        &[
            ("chirrup@o$n", W("chirrup@o@b$n")),
            ("chir^rup@o", W("chir^rup@o@b")),
            ("chirrup:@o", W("chirrup:@o@b")),
            ("tʃi@u$n", W("tʃi@u@b$n")),
        ],
    );
}

#[test]
fn secondary_stress_rejection_does_not_fabricate_a_word() {
    let path =
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E245_secondary_stress";
    let source = std::fs::read_to_string(workspace_root().join(format!("{path}_2.cha")))
        .expect("stress control");
    let admitted = AdmittedFile::parse(&source);
    let word = admitted.word("ˈriˌver");
    let mutant_source = std::fs::read_to_string(workspace_root().join(format!("{path}_1.cha")))
        .expect("stress mutation");
    assert_eq!(
        format!(
            "{}ˈˌ{}",
            source
                .get(..word.location.span().expect("located").start() as usize)
                .expect("UTF-8 prefix"),
            source
                .get(word.location.span().expect("located").end() as usize..)
                .expect("UTF-8 suffix")
        ),
        mutant_source
    );
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let recovered = parser.parse_chat_file_streaming(&mutant_source, &errors);
    assert!(
        errors
            .to_vec()
            .iter()
            .any(|error| error.code.as_str() == "E245")
    );
    let mut words = 0;
    for utterance in recovered.utterances() {
        walk_words(
            &utterance.main.content.content,
            None,
            &mut |item| match item {
                WordItem::Word(_) | WordItem::ReplacedWord(_) => words += 1,
                WordItem::Separator(_) => {}
            },
        );
    }
    assert_eq!(words, 0, "recovery cannot invent spoken content");
}
