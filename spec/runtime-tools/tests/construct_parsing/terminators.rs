//! Independent model expectations for authored main/dependent-tier ending combinations.

use super::policy::Invocation;
use super::{Api, ConstructDeck, Failure, Placement, Transport, observation::check_parse};
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::{ChatParser, DependentTier, Separator, Utterance, UtteranceContent};

#[derive(Clone, Copy)]
enum ExpectedTier {
    MorEnding(&'static str),
    WorEnding(&'static str),
    WorTagMarker(&'static str),
}

impl ExpectedTier {
    fn ending(self) -> &'static str {
        match self {
            Self::MorEnding(ending) | Self::WorEnding(ending) => ending,
            Self::WorTagMarker(_) => ".",
        }
    }

    fn wrong(self) -> Self {
        match self {
            Self::MorEnding(_) => Self::MorEnding("."),
            Self::WorEnding(_) => Self::WorEnding("."),
            Self::WorTagMarker(_) => Self::WorTagMarker(","),
        }
    }
}

/// A declared utterance example, admitted before any parser call.
struct UtteranceWitness<'a> {
    input: &'a str,
    expected: ExpectedTier,
}

impl<'a> UtteranceWitness<'a> {
    fn admit(deck: &'a ConstructDeck, name: &str, expected: ExpectedTier) -> Result<Self, Failure> {
        let label = format!("construct/utterance/utterance/{name}");
        let case = deck
            .0
            .iter()
            .find(|case| case.label == label)
            .ok_or_else(|| format!("missing terminator witness: {label}"))?;
        if !matches!(case.api, Api::Utterance) {
            return Err(format!("terminator witness is not an utterance: {label}").into());
        }
        Ok(Self {
            input: &case.input,
            expected,
        })
    }

    fn check(&self, utterance: &Utterance) -> Result<(), String> {
        let main = utterance
            .main
            .content
            .terminator
            .as_ref()
            .ok_or("main terminator was dropped")?;
        let [entry] = utterance.dependent_tiers.as_slice() else {
            return Err("expected exactly one retained dependent tier".into());
        };
        match (self.expected, utterance.main.content.content.as_slice()) {
            (
                ExpectedTier::WorTagMarker(_),
                [
                    UtteranceContent::Word(_),
                    UtteranceContent::Separator(Separator::Tag { .. }),
                ],
            ) => {}
            (
                ExpectedTier::MorEnding(_) | ExpectedTier::WorEnding(_),
                [UtteranceContent::Word(_)],
            ) => {}
            _ => return Err("main word/separator sequence differs from the spec".into()),
        }
        let ending = match self.expected {
            ExpectedTier::MorEnding(_) => {
                let DependentTier::Mor(mor) = &entry.tier else {
                    return Err("MOR tier was rejected or reclassified".into());
                };
                mor.terminator.to_string()
            }
            ExpectedTier::WorEnding(_) | ExpectedTier::WorTagMarker(_) => {
                let DependentTier::Wor(wor) = &entry.tier else {
                    return Err("WOR tier was rejected or reclassified".into());
                };
                match (self.expected, wor.items.as_slice()) {
                    (ExpectedTier::WorEnding(_), [WorItem::Word(_)]) => {}
                    (
                        ExpectedTier::WorTagMarker(expected),
                        [WorItem::Word(_), WorItem::Separator { text, .. }],
                    ) if text == expected => {}
                    _ => return Err("WOR word/separator sequence differs from the spec".into()),
                }
                wor.terminator
                    .as_ref()
                    .ok_or("WOR terminator was dropped")?
                    .to_string()
            }
        };
        let expected = self.expected.ending();
        if main.to_string() != expected || ending != expected {
            return Err(format!(
                "expected {expected}, main={main}, dependent={ending}"
            ));
        }
        Ok(())
    }
}

// Specific fixture expectations, not a replacement terminator vocabulary.
const MOR_WITNESSES: &[(&str, ExpectedTier); 8] = &[
    ("mor_trailing_off", ExpectedTier::MorEnding("+...")),
    ("mor_interruption", ExpectedTier::MorEnding("+/.")),
    ("mor_self_interruption", ExpectedTier::MorEnding("+//.")),
    ("mor_trailing_off_question", ExpectedTier::MorEnding("+..?")),
    ("mor_interrupted_question", ExpectedTier::MorEnding("+/?")),
    (
        "mor_self_interrupted_question",
        ExpectedTier::MorEnding("+//?"),
    ),
    ("mor_broken_question", ExpectedTier::MorEnding("+!?")),
    ("mor_break_for_coding", ExpectedTier::MorEnding("+.")),
];
const WOR_WITNESSES: &[(&str, ExpectedTier); 8] = &[
    ("wor_quoted_new_line", ExpectedTier::WorEnding("+\"/.")),
    ("wor_self_interruption", ExpectedTier::WorEnding("+//.")),
    ("wor_trailing_off", ExpectedTier::WorEnding("+...")),
    ("wor_interruption", ExpectedTier::WorEnding("+/.")),
    ("wor_tag_marker", ExpectedTier::WorTagMarker("„")),
    ("wor_interrupted_question", ExpectedTier::WorEnding("+/?")),
    ("wor_trailing_off_question", ExpectedTier::WorEnding("+..?")),
    (
        "wor_self_interrupted_question",
        ExpectedTier::WorEnding("+//?"),
    ),
];

#[test]
fn authored_dependent_endings_survive_both_public_utterance_parsers() -> Result<(), Failure> {
    let deck = ConstructDeck::load(&generators::repo_paths::RepoRoot::resolve(None)?)?;
    let primary = talkbank_parser::TreeSitterParser::new()?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let witnesses = MOR_WITNESSES
        .iter()
        .chain(WOR_WITNESSES)
        .copied()
        .map(|(name, expected)| UtteranceWitness::admit(&deck, name, expected))
        .collect::<Result<Vec<_>, _>>()?;

    fn check(
        parser: &impl ChatParser,
        witness: &UtteranceWitness<'_>,
        input: &str,
        invocation: Invocation,
        placement: Placement,
    ) -> Result<(), String> {
        let context = invocation.context();
        check_parse(
            |sink| match &context {
                None => parser.parse_utterance(input, placement.offset(), sink),
                Some(context) => {
                    parser.parse_utterance_with_context(input, placement.offset(), context, sink)
                }
            },
            |utterance| witness.check(utterance),
        )
    }

    let mut observations = 0;
    for witness in &witnesses {
        for transport in [Transport::Authored, Transport::CrLf, Transport::Cr] {
            let input = transport.apply(witness.input);
            for invocation in Invocation::ALL {
                for placement in [Placement::Start, Placement::Embedded] {
                    check(&primary, witness, &input, invocation, placement)?;
                    check(&native, witness, &input, invocation, placement)?;
                    observations += 2;
                }
            }
        }
        // Both backends must refuse an incorrect expectation even if they agree.
        let wrong = UtteranceWitness {
            input: witness.input,
            expected: witness.expected.wrong(),
        };
        assert!(
            check(
                &primary,
                &wrong,
                wrong.input,
                Invocation::Implicit,
                Placement::Start
            )
            .is_err()
        );
        assert!(
            check(
                &native,
                &wrong,
                wrong.input,
                Invocation::Implicit,
                Placement::Start
            )
            .is_err()
        );
    }
    eprintln!(
        "{} dependent-tier ending witnesses; {observations} independent model observations",
        witnesses.len()
    );
    Ok(())
}

/// A complete reference and its declared spec expectations travel together.
struct ReferenceEndingWitness<'a> {
    source: String,
    utterances: Vec<UtteranceWitness<'a>>,
}

impl<'a> ReferenceEndingWitness<'a> {
    fn admit(
        root: &generators::repo_paths::RepoRoot,
        deck: &'a ConstructDeck,
        path: &str,
        declarations: &[(&str, ExpectedTier); 8],
    ) -> Result<Self, Failure> {
        Ok(Self {
            source: std::fs::read_to_string(root.join(path))?,
            utterances: declarations
                .iter()
                .map(|&(name, expected)| UtteranceWitness::admit(deck, name, expected))
                .collect::<Result<_, _>>()?,
        })
    }

    fn check(&self, parser: &impl ChatParser, transport: Transport) -> Result<(), String> {
        let source = transport.apply(&self.source);
        check_parse(
            |sink| parser.parse_chat_file(&source, 0, sink),
            |chat| {
                let utterances: Vec<_> = chat.utterances().collect();
                if utterances.len() != self.utterances.len() {
                    return Err(format!(
                        "expected {} utterances, observed {}",
                        self.utterances.len(),
                        utterances.len()
                    ));
                }
                for (utterance, witness) in utterances.into_iter().zip(&self.utterances) {
                    witness.check(utterance)?;
                }
                Ok(())
            },
        )
    }
}

#[test]
fn authored_ending_references_retain_every_declared_tier() -> Result<(), Failure> {
    let root = generators::repo_paths::RepoRoot::resolve(None)?;
    let deck = ConstructDeck::load(&root)?;
    let primary = talkbank_parser::TreeSitterParser::new()?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    for (path, declarations) in [
        (
            "corpus/reference/tiers/mor-ending-combinations.cha",
            MOR_WITNESSES,
        ),
        (
            "corpus/reference/tiers/wor-ending-combinations.cha",
            WOR_WITNESSES,
        ),
    ] {
        let witness = ReferenceEndingWitness::admit(&root, &deck, path, declarations)?;
        for transport in [Transport::Authored, Transport::CrLf, Transport::Cr] {
            witness.check(&primary, transport)?;
            witness.check(&native, transport)?;
        }
    }
    Ok(())
}
