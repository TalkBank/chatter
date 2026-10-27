//! Typed tier and ID entry points consume original reference-source slices.
//! PHO words and participant entries have no individual shared-model locations;
//! this population makes no source-witness claim for those two APIs.

use std::collections::BTreeMap;

use super::{ComparisonBatch, both_parsers, compare_parses};
use crate::fixture_utils::LineEnding;
use talkbank_model::model::{DependentTier, Header, Line};
use talkbank_model::{ChatParser, FragmentSemanticContext, Span};
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Api {
    Id,
    Pho,
    Sin,
    Act,
    Cod,
    Com,
    Exp,
    Add,
    Gpx,
    Int,
    Spa,
    Sit,
    Wor,
}

impl Api {
    const ALL: [Self; 13] = [
        Self::Id,
        Self::Pho,
        Self::Sin,
        Self::Act,
        Self::Cod,
        Self::Com,
        Self::Exp,
        Self::Add,
        Self::Gpx,
        Self::Int,
        Self::Spa,
        Self::Sit,
        Self::Wor,
    ];

    fn tier(tier: &DependentTier) -> Result<Option<Self>, String> {
        Ok(match tier {
            DependentTier::Pho(_) | DependentTier::Mod(_) => Some(Self::Pho),
            DependentTier::Sin(_) => Some(Self::Sin),
            DependentTier::Act(_) => Some(Self::Act),
            DependentTier::Cod(_) => Some(Self::Cod),
            DependentTier::Com(_) => Some(Self::Com),
            DependentTier::Exp(_) => Some(Self::Exp),
            DependentTier::Add(_) => Some(Self::Add),
            DependentTier::Gpx(_) => Some(Self::Gpx),
            DependentTier::Int(_) => Some(Self::Int),
            DependentTier::Spa(_) => Some(Self::Spa),
            DependentTier::Sit(_) => Some(Self::Sit),
            DependentTier::Wor(_) => Some(Self::Wor),
            // Existing source-backed MOR/GRA comparison stays in its own test.
            // Other tiers have no dedicated ChatParser fragment method.
            DependentTier::Mor(_)
            | DependentTier::Gra(_)
            | DependentTier::Alt(_)
            | DependentTier::Coh(_)
            | DependentTier::Def(_)
            | DependentTier::Eng(_)
            | DependentTier::Err(_)
            | DependentTier::Fac(_)
            | DependentTier::Flo(_)
            | DependentTier::Modsyl(_)
            | DependentTier::Phosyl(_)
            | DependentTier::Phoaln(_)
            | DependentTier::Xphoint(_)
            | DependentTier::Gls(_)
            | DependentTier::Ort(_)
            | DependentTier::Par(_)
            | DependentTier::Tim(_)
            | DependentTier::UserDefined(_) => None,
            DependentTier::UnparsedMor(_)
            | DependentTier::UnparsedGra(_)
            | DependentTier::UnparsedPho(_)
            | DependentTier::UnparsedMod(_)
            | DependentTier::UnparsedSin(_)
            | DependentTier::UnparsedWor(_)
            | DependentTier::UnparsedPhoaln(_)
            | DependentTier::UnparsedText(_)
            | DependentTier::UnparsedXphoint(_)
            | DependentTier::Unsupported(_) => {
                return Err("recovery tier cannot supply a reference witness".into());
            }
        })
    }
}

/// The reference loader copies input from a checked model-owned source range,
/// retaining its origin and context instead of serializing the parsed value.
struct Witness {
    label: String,
    api: Api,
    input: String,
    origin: usize,
    context: FragmentSemanticContext,
}

impl Witness {
    fn from_source(
        label: &str,
        api: Api,
        source: &str,
        span: Span,
        context: &FragmentSemanticContext,
    ) -> Result<Self, String> {
        let input = source
            .get(span.to_range())
            .ok_or_else(|| format!("{label}: source span {span:?}"))?;
        if input.is_empty() {
            return Err(format!("{label}: empty reference witness for {api:?}"));
        }
        Ok(Self {
            label: label.to_owned(),
            api,
            input: input.to_owned(),
            origin: span.start(),
            context: context.clone(),
        })
    }

    fn compare(
        &self,
        primary: &impl ChatParser,
        native: &impl ChatParser,
        origin: Origin,
    ) -> Result<(), String> {
        let offset = match origin {
            Origin::Standalone => 0,
            Origin::Document => self.origin,
        };
        macro_rules! pair {
            ($method:ident) => {
                compare_parses(
                    |sink| primary.$method(&self.input, offset, &self.context, sink),
                    |sink| native.$method(&self.input, offset, &self.context, sink),
                )
            };
        }
        match self.api {
            Api::Id => pair!(parse_id_header_with_context),
            Api::Pho => pair!(parse_pho_tier_with_context),
            Api::Sin => pair!(parse_sin_tier_with_context),
            Api::Act => pair!(parse_act_tier_with_context),
            Api::Cod => pair!(parse_cod_tier_with_context),
            Api::Com => pair!(parse_com_tier_with_context),
            Api::Exp => pair!(parse_exp_tier_with_context),
            Api::Add => pair!(parse_add_tier_with_context),
            Api::Gpx => pair!(parse_gpx_tier_with_context),
            Api::Int => pair!(parse_int_tier_with_context),
            Api::Spa => pair!(parse_spa_tier_with_context),
            Api::Sit => pair!(parse_sit_tier_with_context),
            Api::Wor => pair!(parse_wor_tier_with_context),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Standalone,
    Document,
}

/// All thirteen routes must have witnesses before any comparison is accepted.
struct WitnessDeck {
    cases: Vec<Witness>,
    counts: BTreeMap<Api, usize>,
}

impl WitnessDeck {
    fn admit(cases: Vec<Witness>) -> Result<Self, String> {
        let mut counts = BTreeMap::new();
        for case in &cases {
            *counts.entry(case.api).or_insert(0) += 1;
        }
        for api in Api::ALL {
            if !counts.contains_key(&api) {
                return Err(format!("missing reference witnesses for {api:?}"));
            }
        }
        Ok(Self { cases, counts })
    }

    fn load(
        corpus: &ChatCorpus,
        parser: &talkbank_parser::TreeSitterParser,
        ending: LineEnding,
    ) -> Result<Self, String> {
        let mut cases = Vec::new();
        for fixture in corpus.fixtures() {
            let source = ending.apply(fixture.source());
            let file = strict_parse(parser.parse_chat_file(&source))
                .map_err(|error| format!("{} {ending:?}: {error}", fixture.path().display()))?;
            let flags = file
                .lines
                .iter()
                .filter_map(|line| match line {
                    Line::Header { header, .. } => match header.as_ref() {
                        Header::Options { options } => Some(options.iter().cloned()),
                        _ => None,
                    },
                    Line::Utterance(_) => None,
                })
                .flatten()
                .collect();
            let context = FragmentSemanticContext::new().with_option_flags(flags);
            let label = fixture.path().display().to_string();
            for line in &file.lines {
                match line {
                    Line::Header {
                        header, location, ..
                    } => {
                        if matches!(header.as_ref(), Header::ID(_)) {
                            let span = location
                                .span()
                                .ok_or_else(|| format!("{label}: unlocated ID header"))?;
                            cases.push(Witness::from_source(
                                &label,
                                Api::Id,
                                &source,
                                span,
                                &context,
                            )?);
                        }
                    }
                    Line::Utterance(utterance) => {
                        for entry in &utterance.dependent_tiers {
                            let Some(api) = Api::tier(&entry.tier)? else {
                                continue;
                            };
                            let span = entry
                                .content_span()
                                .ok_or_else(|| format!("{label}: unlocated {api:?} body"))?;
                            cases.push(Witness::from_source(&label, api, &source, span, &context)?);
                        }
                    }
                }
            }
        }
        Self::admit(cases)
    }
}

#[test]
fn typed_reference_bodies_and_id_headers_agree() -> Result<(), String> {
    let (primary, native) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let mut batch = ComparisonBatch::default();
    for ending in LineEnding::ALL {
        let deck = WitnessDeck::load(&corpus, &primary, ending)?;
        eprintln!("{ending:?} typed reference witnesses: {:?}", deck.counts);
        for case in deck.cases {
            for origin in [Origin::Standalone, Origin::Document] {
                batch.record(
                    format!(
                        "{} {:?} {} {ending:?} {origin:?}",
                        case.label, case.api, case.origin
                    ),
                    case.compare(&primary, &native, origin),
                );
            }
        }
    }
    batch.finish()
}

#[test]
fn typed_reference_deck_refuses_missing_routes_and_invalid_source() -> Result<(), String> {
    assert!(WitnessDeck::admit(Vec::new()).is_err());
    let context = FragmentSemanticContext::new();
    for absent in Api::ALL {
        let cases = Api::ALL
            .into_iter()
            .filter(|api| *api != absent)
            .map(|api| Witness::from_source("boundary", api, "x", Span::new(0, 1), &context))
            .collect::<Result<_, _>>()?;
        assert!(WitnessDeck::admit(cases).is_err(), "missing {absent:?}");
    }
    for span in [Span::new(0, 0), Span::new(0, 9), Span::new(1, 2)] {
        assert!(Witness::from_source("boundary", Api::Pho, "😀", span, &context).is_err());
    }
    Ok(())
}
