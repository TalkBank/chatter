//! Leaf fragment witnesses come from typed nodes bound to the original source.
//! CST classification selects inputs; neither backend is a semantic oracle.

use std::collections::BTreeMap;

use super::{ComparisonBatch, both_parsers, compare_parses};
use crate::fixture_utils::LineEnding;
use talkbank_model::model::{Header, Line};
use talkbank_model::{ChatParser, FragmentSemanticContext};
use talkbank_parser::generated_traversal::{
    AsRawNode, GraRelationNode, MorWordNode, ParticipantNode, PhoWordsNode, SourceSlice,
};
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Api {
    Participant,
    MorWord,
    GraRelation,
    PhoWord,
}

impl Api {
    const ALL: [Self; 4] = [
        Self::Participant,
        Self::MorWord,
        Self::GraRelation,
        Self::PhoWord,
    ];
}

/// Only a producer-bound typed node can supply the copied text and origin.
/// No independent range/source pair or serialized model enters this constructor.
#[derive(Clone)]
struct Witness {
    api: Api,
    input: String,
    origin: usize,
    label: String,
    context: FragmentSemanticContext,
}

impl Witness {
    fn from_node(
        node: SourceSlice<'_, '_>,
        label: &str,
        context: &FragmentSemanticContext,
    ) -> Result<Option<Self>, String> {
        let api = if node.typed::<ParticipantNode>().is_some() {
            Api::Participant
        } else if node.typed::<MorWordNode>().is_some() {
            Api::MorWord
        } else if node.typed::<GraRelationNode>().is_some() {
            Api::GraRelation
        } else if node.typed::<PhoWordsNode>().is_some() {
            // This node covers the complete compound, not its individual segments.
            Api::PhoWord
        } else {
            return Ok(None);
        };
        if node.text().is_empty() {
            return Err(format!("{label}: empty {api:?} source witness"));
        }
        Ok(Some(Self {
            api,
            input: node.text().to_owned(),
            origin: node.raw_node().start_byte(),
            label: label.to_owned(),
            context: context.clone(),
        }))
    }

    fn compare(
        &self,
        primary: &impl ChatParser,
        native: &impl ChatParser,
        origin: Origin,
        invocation: Invocation,
    ) -> Result<(), String> {
        let offset = match origin {
            Origin::Standalone => 0,
            Origin::Document => self.origin,
        };
        macro_rules! pair {
            ($plain:ident, $contextual:ident) => {
                match invocation {
                    Invocation::Implicit => compare_parses(
                        |sink| primary.$plain(&self.input, offset, sink),
                        |sink| native.$plain(&self.input, offset, sink),
                    ),
                    Invocation::DocumentContext => compare_parses(
                        |sink| primary.$contextual(&self.input, offset, &self.context, sink),
                        |sink| native.$contextual(&self.input, offset, &self.context, sink),
                    ),
                }
            };
        }
        match self.api {
            Api::Participant => pair!(
                parse_participant_entry,
                parse_participant_entry_with_context
            ),
            Api::MorWord => pair!(parse_mor_word, parse_mor_word_with_context),
            Api::GraRelation => pair!(parse_gra_relation, parse_gra_relation_with_context),
            Api::PhoWord => pair!(parse_pho_word, parse_pho_word_with_context),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Standalone,
    Document,
}

#[derive(Clone, Copy, Debug)]
enum Invocation {
    Implicit,
    DocumentContext,
}

/// A partial route population cannot enter the comparison phase.
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
                return Err(format!("missing source witnesses for {api:?}"));
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
            let label = fixture.path().display().to_string();
            let model = strict_parse(parser.parse_chat_file(&source))
                .map_err(|error| format!("{label} {ending:?}: {error}"))?;
            let flags = model
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
            let parsed = parser
                .parse_source_incremental(&source, None)
                .map_err(|error| format!("{label}: {error}"))?;
            let root = parsed.root().map_err(|error| format!("{label}: {error}"))?;
            for node in root.descendants() {
                let node = node.map_err(|error| format!("{label}: {error}"))?;
                if let Some(witness) = Witness::from_node(node, &label, &context)? {
                    cases.push(witness);
                }
            }
        }
        Self::admit(cases)
    }
}

#[test]
fn original_reference_leaf_fragments_agree() -> Result<(), String> {
    let (primary, native) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let mut batch = ComparisonBatch::default();
    for ending in LineEnding::ALL {
        let deck = WitnessDeck::load(&corpus, &primary, ending)?;
        eprintln!("{ending:?} original leaf witnesses: {:?}", deck.counts);
        for case in deck.cases {
            for origin in [Origin::Standalone, Origin::Document] {
                for invocation in [Invocation::Implicit, Invocation::DocumentContext] {
                    batch.record(
                        format!(
                            "{} {:?} {} {ending:?} {origin:?} {invocation:?}",
                            case.label, case.api, case.origin
                        ),
                        case.compare(&primary, &native, origin, invocation),
                    );
                }
            }
        }
    }
    batch.finish()
}

#[test]
fn original_leaf_population_requires_every_route() -> Result<(), String> {
    let (primary, _) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let deck = WitnessDeck::load(&corpus, &primary, LineEnding::Authored)?;
    assert!(WitnessDeck::admit(Vec::new()).is_err());
    // Remove one actual route at a time; no fabricated source witness can
    // accidentally make the missing-route control pass.
    for missing in Api::ALL {
        let retained = deck
            .cases
            .iter()
            .filter(|case| case.api != missing)
            .cloned()
            .collect();
        assert!(WitnessDeck::admit(retained).is_err(), "missing {missing:?}");
    }
    Ok(())
}
