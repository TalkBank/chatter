//! Invocation policies are explicit; NoAlign neutrality does not imply CA neutrality.

use talkbank_model::{ChatOptionFlag, FragmentSemanticContext};

#[derive(Clone, Copy, Debug)]
pub(super) enum Invocation {
    Implicit,
    Explicit(Options),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Options {
    Empty,
    NoAlign,
    Ca,
    CaNoAlign,
}

impl Invocation {
    pub(super) const ALL: [Self; 5] = [
        Self::Implicit,
        Self::Explicit(Options::Empty),
        Self::Explicit(Options::NoAlign),
        Self::Explicit(Options::Ca),
        Self::Explicit(Options::CaNoAlign),
    ];

    pub(super) fn context(self) -> Option<FragmentSemanticContext> {
        let flags = match self {
            Self::Implicit => return None,
            Self::Explicit(Options::Empty) => vec![],
            Self::Explicit(Options::NoAlign) => vec![ChatOptionFlag::NoAlign],
            Self::Explicit(Options::Ca) => vec![ChatOptionFlag::Ca],
            Self::Explicit(Options::CaNoAlign) => vec![ChatOptionFlag::Ca, ChatOptionFlag::NoAlign],
        };
        Some(FragmentSemanticContext::new().with_option_flags(flags))
    }
}

/// Only reviewed interpretation comparisons can be requested. There is no
/// ordinary-versus-CA invariance variant: CA may change a parenthetical's meaning.
#[derive(Clone, Copy, Debug)]
pub(super) enum PolicyPair {
    Same(Invocation),
    OrdinaryNoAlign,
    CaNoAlign,
}

impl PolicyPair {
    pub(super) fn invocations(self) -> (Invocation, Invocation) {
        match self {
            Self::Same(invocation) => (invocation, invocation),
            Self::OrdinaryNoAlign => (
                Invocation::Explicit(Options::Empty),
                Invocation::Explicit(Options::NoAlign),
            ),
            Self::CaNoAlign => (
                Invocation::Explicit(Options::Ca),
                Invocation::Explicit(Options::CaNoAlign),
            ),
        }
    }
}

/// A source-owned witness makes dropping CA context observable even if both
/// backends make the same mistake. This is separate from peer equality.
#[derive(Clone, Copy, Debug)]
enum WordMeaning {
    Shortening,
    CaOmission,
}

impl WordMeaning {
    fn for_invocation(invocation: Invocation) -> Self {
        match invocation {
            Invocation::Implicit | Invocation::Explicit(Options::Empty | Options::NoAlign) => {
                Self::Shortening
            }
            Invocation::Explicit(Options::Ca | Options::CaNoAlign) => Self::CaOmission,
        }
    }

    fn check(self, word: &talkbank_model::Word) -> Result<(), String> {
        use talkbank_model::{WordCategory, WordContent};
        let shortenings = word
            .content()
            .iter()
            .filter(|item| matches!(item, WordContent::Shortening(_)))
            .count();
        let accepted =
            match self {
                Self::Shortening => word.category.is_none() && shortenings == 1,
                Self::CaOmission => word.category == Some(WordCategory::CAOmission)
                    && shortenings == 0
                    && word.content().iter().any(
                        |item| matches!(item, WordContent::Text(text) if text.as_ref() == "maybe"),
                    ),
            };
        if accepted {
            Ok(())
        } else {
            Err(format!("expected {self:?}, observed {word:?}"))
        }
    }
}

#[test]
fn context_invocations_preserve_the_authored_parenthetical_policy() -> Result<(), super::Failure> {
    use super::Placement;
    use super::observation::check_parse;
    use talkbank_model::ChatParser;

    const SOURCE: &str =
        include_str!("../../../../corpus/reference/word-features/ca-omission-combinations.cha");
    // Quote the exact reference word; do not normalize or regenerate its bytes.
    let start = SOURCE
        .find("⌈(maybe)")
        .ok_or("missing parenthetical source witness")?;
    let input = &SOURCE[start..start + "⌈(maybe)".len()];
    let primary = talkbank_parser::TreeSitterParser::new()?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    fn check(
        parser: &impl ChatParser,
        input: &str,
        invocation: Invocation,
        placement: Placement,
    ) -> Result<(), String> {
        let context = invocation.context();
        check_parse(
            |sink| match &context {
                None => parser.parse_word(input, placement.offset(), sink),
                Some(context) => {
                    parser.parse_word_with_context(input, placement.offset(), context, sink)
                }
            },
            |word| WordMeaning::for_invocation(invocation).check(word),
        )
    }
    for invocation in Invocation::ALL {
        for placement in [Placement::Start, Placement::Embedded] {
            check(&primary, input, invocation, placement)?;
            check(&native, input, invocation, placement)?;
        }
    }
    // Planted policy reversal must fail in both directions. A successful parse
    // or two agreeing parsers cannot certify the intended interpretation.
    for expected in [WordMeaning::Shortening, WordMeaning::CaOmission] {
        let opposite = match expected {
            WordMeaning::Shortening => Invocation::Explicit(Options::Ca),
            WordMeaning::CaOmission => Invocation::Explicit(Options::Empty),
        }
        .context()
        .ok_or("explicit policy witness")?;
        assert!(
            check_parse(
                |sink| primary.parse_word_with_context(input, 0, &opposite, sink),
                |word| expected.check(word)
            )
            .is_err()
        );
        assert!(
            check_parse(
                |sink| native.parse_word_with_context(input, 0, &opposite, sink),
                |word| expected.check(word)
            )
            .is_err()
        );
    }
    Ok(())
}
