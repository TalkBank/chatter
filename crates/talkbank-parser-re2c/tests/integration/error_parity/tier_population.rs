//! Original-source tier and word fragments from the complete authored error deck.
//! Main tiers and words include shared-model validation; dependent body and line
//! tiers retain their parse-only policy. None invokes alignment or whole-file
//! validation. Omitted recovery tiers are outside this population.

mod typed;
use typed::TypedApi;

use super::{
    model::{
        BackendReport, CaseReport, Detection, ExampleOrdinal, ParsingFindings, SpecLabel,
        ValidationFindings,
    },
    spec_corpus::load_spec_corpus,
};
use crate::fixture_utils::LineEnding;
use std::collections::BTreeMap;
use talkbank_model::model::{ContentStructure, Descend, LeafContent, WordRef};
use talkbank_model::validation::{Validate, ValidationContext};
use talkbank_model::{
    ChatParser, CompletedDiagnostics, ErrorCollector, FragmentSemanticContext, ParseOutcome,
    SpanShift,
};

/// Only this owner can select an extent from the document it parsed.
struct SpecDocument<'a> {
    source: &'a str,
    file: talkbank_model::ChatFile,
    context: FragmentSemanticContext,
}

impl<'a> SpecDocument<'a> {
    fn parse(source: &'a str, primary: &talkbank_parser::TreeSitterParser) -> Result<Self, String> {
        let errors = ErrorCollector::new();
        let file = primary.parse_chat_file_streaming(source, &errors);
        CompletedDiagnostics::admit(errors.into_vec()).map_err(|error| error.to_string())?;
        let context = FragmentSemanticContext::new()
            .with_option_flags(file.option_flags().cloned().collect());
        Ok(Self {
            source,
            file,
            context,
        })
    }

    fn fragments(&self, route: Route) -> Vec<Result<SourceObservation<'_>, String>> {
        let select = |location: talkbank_model::Location, kind, api| {
            let span = location.span().ok_or("unlocated retained fragment")?;
            let input = self
                .source
                .get(span.to_range())
                .filter(|text| !text.is_empty())
                .ok_or("invalid fragment source extent")?;
            Ok(SourceObservation::Fragment(SourceFragment {
                input,
                origin: span.start(),
                kind,
                context: &self.context,
                api,
            }))
        };
        match route {
            Route::Word => {
                let mut fragments = Vec::new();
                for utterance in self.file.utterances() {
                    for item in &utterance.main.content.content {
                        item.structure().walk(&mut |structure| {
                            let word = match structure {
                                ContentStructure::Word(word) => Some(match word {
                                    WordRef::Bare(word) => word,
                                    WordRef::Annotated(word) => &word.inner,
                                    WordRef::Replaced(word) => &word.word,
                                }),
                                ContentStructure::Leaf(leaf) => match leaf.content {
                                    LeafContent::Interposed(word) => Some(word),
                                    LeafContent::Pause
                                    | LeafContent::Spoken
                                    | LeafContent::Notation
                                    | LeafContent::OverlapPoint(_)
                                    | LeafContent::UnderlineBegin(_)
                                    | LeafContent::UnderlineEnd(_) => None,
                                },
                                ContentStructure::Group(_) | ContentStructure::Retrace(_) => None,
                            };
                            if let Some(word) = word {
                                fragments.push(select(word.location, "word", Api::Word));
                            }
                            Descend::Into
                        });
                    }
                }
                fragments
            }
            Route::Main => self
                .file
                .utterances()
                .map(|utterance| select(utterance.main.location, "main", Api::Main))
                .collect(),
            Route::Dependent => self
                .file
                .utterances()
                .flat_map(|utterance| &utterance.dependent_tiers)
                .map(|entry| select(entry.location(), entry.kind(), Api::Dependent))
                .collect(),
            Route::TypedBodies => {
                let mut observations = Vec::new();
                for entry in self
                    .file
                    .utterances()
                    .flat_map(|utterance| &utterance.dependent_tiers)
                {
                    // Select the public method from the retained model's label,
                    // including explicit raw recovery. Do not parse source prefixes.
                    let Some(api) = TypedApi::for_label(entry.kind()) else {
                        continue;
                    };
                    let Some(span) = entry.content_span() else {
                        observations.push(Ok(SourceObservation::UnavailableBody {
                            api,
                            tier: entry.location(),
                        }));
                        continue;
                    };
                    // Known empty bodies are valid source witnesses. Missing
                    // geometry above is a different observation, never an empty string.
                    observations.push(
                        self.source
                            .get(span.to_range())
                            .ok_or_else(|| "invalid typed body extent".to_owned())
                            .map(|input| {
                                SourceObservation::Fragment(SourceFragment {
                                    input,
                                    origin: span.start(),
                                    kind: entry.kind(),
                                    context: &self.context,
                                    api: Api::Typed(api),
                                })
                            }),
                    );
                }
                observations
            }
        }
    }
}

/// Source bytes, their origin, API and interpretation belong to one parsed document.
struct SourceFragment<'a> {
    input: &'a str,
    origin: usize,
    kind: &'a str,
    context: &'a FragmentSemanticContext,
    api: Api,
}

enum SourceObservation<'a> {
    Fragment(SourceFragment<'a>),
    UnavailableBody {
        api: TypedApi,
        tier: talkbank_model::Location,
    },
}

/// An unavailable body still has an authored case and source-tier observation.
/// Counts are derived from these witnesses, rather than erasing their identity.
struct UnavailableBodyWitness {
    case: String,
    ending: LineEnding,
    tier: talkbank_model::Location,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Api {
    Word,
    Main,
    Dependent,
    Typed(TypedApi),
}

/// Exact bytes, origin and API plus the complete invocation policy form a probe.
/// Policy equality includes unknown option text, order and repeated flags.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ProbeSource {
    input: String,
    origin: usize,
    api: Api,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Invocation {
    Implicit,
    DocumentContext(FragmentSemanticContext),
}

impl Invocation {
    fn validation_context(&self) -> ValidationContext {
        match self {
            Self::Implicit => ValidationContext::default(),
            Self::DocumentContext(context) => {
                ValidationContext::default().with_ca_mode(context.ca_mode())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Route {
    Word,
    Main,
    Dependent,
    TypedBodies,
}

fn measure(
    parser: &impl ChatParser,
    fragment: &SourceFragment<'_>,
    origin: usize,
    invocation: &Invocation,
) -> Result<BackendReport, String> {
    let errors = ErrorCollector::new();
    let validation = ErrorCollector::new();
    macro_rules! invoke {
        ($implicit:ident, $explicit:ident) => {
            check_payload(
                match invocation {
                    Invocation::Implicit => parser.$implicit(fragment.input, origin, &errors),
                    Invocation::DocumentContext(context) => {
                        parser.$explicit(fragment.input, origin, context, &errors)
                    }
                },
                fragment.input.len(),
                origin,
                &errors,
            )?
        };
    }
    match fragment.api {
        Api::Word => {
            if let Some(word) = invoke!(parse_word, parse_word_with_context) {
                word.validate(&invocation.validation_context(), &validation);
            }
        }
        Api::Main => {
            if let Some(main) = invoke!(parse_main_tier, parse_main_tier_with_context) {
                main.validate(&invocation.validation_context(), &validation);
            }
        }
        Api::Dependent => {
            let _ = invoke!(parse_dependent_tier, parse_dependent_tier_with_context);
        }
        Api::Typed(api) => api.measure(parser, fragment.input, origin, invocation, &errors)?,
    }
    Ok(BackendReport::new(
        ParsingFindings::from_sink(errors),
        ValidationFindings::from_sink(validation),
    ))
}

fn check_payload<T: SpanShift>(
    outcome: ParseOutcome<T>,
    length: usize,
    origin: usize,
    errors: &ErrorCollector,
) -> Result<Option<T>, String> {
    match outcome {
        ParseOutcome::Rejected if !errors.has_errors() => {
            Err("fragment rejection without an error".to_owned())
        }
        ParseOutcome::Rejected => Ok(None),
        ParseOutcome::Parsed(tier) => {
            let end = origin
                .checked_add(length)
                .ok_or("fragment origin overflow")?;
            let tier = tier.try_map_spans(&mut |span: talkbank_model::Span| {
                if span.start() >= origin && span.end() <= end {
                    Ok(span)
                } else {
                    Err(format!("fragment payload {span:?} outside {origin}..{end}"))
                }
            })?;
            Ok(Some(tier))
        }
    }
}

fn audit(route: Route) -> Result<(), String> {
    let corpus = load_spec_corpus()?;
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut fragments = 0;
    let mut comparisons = 0;
    let mut required = 0;
    let mut probes: BTreeMap<ProbeSource, Vec<Invocation>> = BTreeMap::new();
    let mut requested = 0;
    let mut selected = BTreeMap::<Api, usize>::new();
    let mut empty = BTreeMap::<Api, usize>::new();
    let mut unavailable = BTreeMap::<TypedApi, Vec<UnavailableBodyWitness>>::new();
    for case in &corpus.cases {
        for ending in LineEnding::ALL {
            let input = case.input.with_line_ending(ending);
            let document = SpecDocument::parse(input.text(), &primary)?;
            for fragment in document.fragments(route) {
                let fragment = match fragment? {
                    SourceObservation::Fragment(fragment) => fragment,
                    SourceObservation::UnavailableBody { api, tier } => {
                        unavailable
                            .entry(api)
                            .or_default()
                            .push(UnavailableBodyWitness {
                                case: case.label.to_string(),
                                ending,
                                tier,
                            });
                        continue;
                    }
                };
                *selected.entry(fragment.api).or_default() += 1;
                if fragment.input.is_empty() {
                    *empty.entry(fragment.api).or_default() += 1;
                }
                fragments += 1;
                for origin in [0, fragment.origin] {
                    for invocation in [
                        Invocation::Implicit,
                        Invocation::DocumentContext(fragment.context.clone()),
                    ] {
                        requested += 1;
                        let policies = probes
                            .entry(ProbeSource {
                                input: fragment.input.to_owned(),
                                origin,
                                api: fragment.api,
                            })
                            .or_default();
                        if policies.contains(&invocation) {
                            continue;
                        }
                        let label = format!(
                            "{} {} {ending:?} {invocation:?} origin {origin}",
                            case.label, fragment.kind
                        );
                        let report = CaseReport {
                            label: SpecLabel::new(&label, ExampleOrdinal::from_zero_based(0)),
                            tree_sitter: measure(&primary, &fragment, origin, &invocation)
                                .map_err(|error| format!("{label}: primary {error}"))?,
                            re2c: measure(&native, &fragment, origin, &invocation)
                                .map_err(|error| format!("{label}: native {error}"))?,
                        };
                        policies.push(invocation);
                        comparisons += 1;
                        match report.detection() {
                            Detection::Reported => required += 1,
                            Detection::NotRequired => {}
                            Detection::Missing(_) => failures.push(report.detail()),
                            Detection::ToolFailure { finding, .. } => {
                                failures.push(format!("{}: {}", report.detail(), finding.message))
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        fragments > 0 && required > 0,
        "fragment error population must not be empty"
    );
    println!(
        "{route:?}: {fragments} retained source fragments, {comparisons} distinct comparisons, {} identical observations reused, {required} required detections",
        requested - comparisons
    );
    let unavailable_counts: BTreeMap<_, _> = unavailable
        .iter()
        .map(|(api, witnesses)| (*api, witnesses.len()))
        .collect();
    println!(
        "Selected source observations: {selected:?}; unavailable body extents: {unavailable_counts:?}"
    );
    for (api, witnesses) in unavailable {
        for witness in witnesses {
            println!(
                "Unavailable {api:?} body: {} {:?}, tier {:?}",
                witness.case, witness.ending, witness.tier
            );
        }
    }
    if matches!(route, Route::TypedBodies) {
        let absent: Vec<_> = TypedApi::ALL
            .into_iter()
            .filter(|api| !selected.contains_key(&Api::Typed(*api)))
            .collect();
        println!(
            "Known empty bodies: {empty:?}; API families without source witnesses: {absent:?}"
        );
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn retained_spec_dependent_tiers_preserve_fragment_error_detection() -> Result<(), String> {
    audit(Route::Dependent)
}

#[test]
fn retained_spec_main_tiers_preserve_fragment_error_detection() -> Result<(), String> {
    audit(Route::Main)
}

#[test]
fn retained_spec_words_preserve_fragment_error_detection() -> Result<(), String> {
    audit(Route::Word)
}

#[test]
fn retained_spec_typed_bodies_preserve_fragment_error_detection() -> Result<(), String> {
    audit(Route::TypedBodies)
}
