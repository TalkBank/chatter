//! Original whole-utterance slices retain the meaning of their reference document.

use super::{ComparisonBatch, LineEnding, both_parsers, compare_parses};
use talkbank_model::{
    ChatFile, ChatParser, DiagnosticKind, ErrorCollector, FragmentSemanticContext, MainTier,
    ParseOutcome, Severity, Utterance, kind_of,
};
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

/// The reference source and the model parsed from those exact bytes stay paired.
struct ReferenceDocument<'a> {
    source: &'a str,
    file: ChatFile,
    context: FragmentSemanticContext,
}

impl<'a> ReferenceDocument<'a> {
    fn parse(source: &'a str, parser: &talkbank_parser::TreeSitterParser) -> Result<Self, String> {
        let file =
            strict_parse(parser.parse_chat_file(source)).map_err(|error| error.to_string())?;
        let context = FragmentSemanticContext::new()
            .with_option_flags(file.option_flags().cloned().collect());
        Ok(Self {
            source,
            file,
            context,
        })
    }

    fn utterances(&self) -> impl Iterator<Item = Result<OriginalUtterance<'_>, String>> {
        self.file.utterances().map(|expected| {
            if !expected.preceding_headers.is_empty() {
                return Err("preceding headers require an original source extent".to_owned());
            }
            let main = expected.main.location.span().ok_or("unlocated main tier")?;
            let mut end = main.end();
            for entry in &expected.dependent_tiers {
                let span = entry
                    .tier
                    .location()
                    .span()
                    .ok_or("unlocated dependent tier")?;
                if span.start() < main.start() {
                    return Err("dependent tier precedes its main tier".to_owned());
                }
                end = end.max(span.end());
            }
            let input = self
                .source
                .get(main.start()..end)
                .ok_or("utterance outside source")?;
            Ok(OriginalUtterance {
                input,
                origin: main.start(),
                expected,
                context: &self.context,
                main_expectation: match expected.dependent_tiers.split_first() {
                    None => MainTierExpectation::Accepted(&expected.main),
                    Some(_) => MainTierExpectation::RefusedAttachedTiers,
                },
            })
        })
    }
}

/// A complete source slice with its document meaning and interpretation options.
struct OriginalUtterance<'a> {
    input: &'a str,
    origin: usize,
    expected: &'a Utterance,
    context: &'a FragmentSemanticContext,
    main_expectation: MainTierExpectation<'a>,
}

/// The admitted document determines shape before either fragment API runs.
enum MainTierExpectation<'a> {
    Accepted(&'a MainTier),
    RefusedAttachedTiers,
}

impl OriginalUtterance<'_> {
    fn check_main_boundary(&self, parser: &impl ChatParser, offset: usize) -> Result<(), String> {
        match self.main_expectation {
            MainTierExpectation::Accepted(main) => compare_parses(
                |_| ParseOutcome::Parsed(main.clone()),
                |errors| {
                    parser.parse_main_tier_with_context(self.input, offset, self.context, errors)
                },
            ),
            MainTierExpectation::RefusedAttachedTiers => {
                let errors = ErrorCollector::new();
                let outcome =
                    parser.parse_main_tier_with_context(self.input, offset, self.context, &errors);
                let diagnostics = errors.into_vec();
                if outcome.is_parsed() {
                    return Err(
                        "main-tier API returned a model for an utterance with attached tiers"
                            .to_owned(),
                    );
                }
                if diagnostics
                    .iter()
                    .any(|error| kind_of(error.code) == DiagnosticKind::InternalFailure)
                {
                    return Err(format!(
                        "internal failure is not fragment-shape refusal: {diagnostics:?}"
                    ));
                }
                if !diagnostics.iter().any(|error| {
                    error.severity == Severity::Error
                        && kind_of(error.code) == DiagnosticKind::Invalidity
                }) {
                    return Err(format!(
                        "shape refusal has no invalidity error: {diagnostics:?}"
                    ));
                }
                for error in &diagnostics {
                    let span = error
                        .location
                        .span()
                        .ok_or("shape diagnostic is unlocated")?;
                    let start = span
                        .start()
                        .checked_sub(offset)
                        .ok_or("diagnostic precedes input")?;
                    let end = span
                        .end()
                        .checked_sub(offset)
                        .ok_or("diagnostic precedes input")?;
                    self.input
                        .get(start..end)
                        .ok_or("diagnostic extends beyond caller bytes")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Local,
    Document,
}

#[derive(Clone, Copy, Debug)]
enum Backend {
    Primary,
    Native,
}

#[test]
fn reference_original_utterance_fragments_preserve_document_meaning() -> Result<(), String> {
    let (primary, native) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let mut batch = ComparisonBatch::default();
    let mut utterances = 0;
    let mut attached_tiers = 0;
    let mut continuations = 0;
    let mut contextual = 0;
    let mut main_controls = 0;
    let mut main_refusals = 0;
    for fixture in corpus.fixtures() {
        for ending in LineEnding::ALL {
            let source = ending.apply(fixture.source());
            let document = ReferenceDocument::parse(&source, &primary)
                .map_err(|error| format!("{} {ending:?}: {error}", fixture.path().display()))?;
            for original in document.utterances() {
                let original = original
                    .map_err(|error| format!("{} {ending:?}: {error}", fixture.path().display()))?;
                for origin in [Origin::Local, Origin::Document] {
                    let offset = match origin {
                        Origin::Local => 0,
                        Origin::Document => original.origin,
                    };
                    for backend in [Backend::Primary, Backend::Native] {
                        batch.record(
                            format!(
                                "{} {ending:?} main boundary {} {origin:?} {backend:?}",
                                fixture.path().display(),
                                original.origin
                            ),
                            match backend {
                                Backend::Primary => original.check_main_boundary(&primary, offset),
                                Backend::Native => original.check_main_boundary(&native, offset),
                            },
                        );
                        batch.record(
                            format!(
                                "{} {ending:?} utterance {} {origin:?} {backend:?}",
                                fixture.path().display(),
                                original.origin
                            ),
                            compare_parses(
                                |_| ParseOutcome::Parsed(original.expected.clone()),
                                |errors| match backend {
                                    Backend::Primary => ChatParser::parse_utterance_with_context(
                                        &primary,
                                        original.input,
                                        offset,
                                        original.context,
                                        errors,
                                    ),
                                    Backend::Native => ChatParser::parse_utterance_with_context(
                                        &native,
                                        original.input,
                                        offset,
                                        original.context,
                                        errors,
                                    ),
                                },
                            ),
                        );
                    }
                }
                match original.main_expectation {
                    MainTierExpectation::Accepted(_) => main_controls += 1,
                    MainTierExpectation::RefusedAttachedTiers => main_refusals += 1,
                }
                utterances += 1;
                attached_tiers += original.expected.dependent_tiers.len();
                continuations +=
                    usize::from(original.input.contains("\n\t") || original.input.contains("\r\t"));
                contextual += usize::from(!original.context.option_flags.is_empty());
            }
        }
    }
    eprintln!(
        "original utterances: {utterances}; attached tiers: {attached_tiers}; continuations: {continuations}; contextual: {contextual}"
    );
    assert!(
        attached_tiers > 0 && continuations > 0 && contextual > 0,
        "reference utterance boundary witnesses remain present"
    );
    eprintln!(
        "main-tier shape controls: {main_controls}; refused attached-tier inputs: {main_refusals}"
    );
    assert!(
        main_controls > 0 && main_refusals > 0,
        "both admitted source shapes are witnessed"
    );
    batch.finish()
}
