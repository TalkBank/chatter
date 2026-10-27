//! Native word-timing entry point and structured-tier boundary tests.

use talkbank_model::ErrorSink;
use talkbank_model::model::*;

/// Parse `%wor` tier content and convert to a model `WorTier`.
///
/// `None` when the tier does not parse, so the caller can reject it. It used to
/// return a bare `WorTier`, substituting an EMPTY one on failure, which
/// reported an unparsable tier as a successfully parsed tier with no words.
/// The seam to say otherwise already existed at both call sites: one holds an
/// `ErrorSink` it was ignoring, the other an outcome type with a `rejected`
/// variant. Matching the `Option`-then-`rejected` shape the three sibling
/// entry points in `chat_parser_impl` already use.
///
/// Delegates to `wor_tier_parser`, the same parser the file-level path uses.
/// It used to be a SECOND implementation, parsing with the MAIN-TIER
/// `contents_parser` and keeping only bare words and separators from a flat
/// loop, so it disagreed with the real one about timing bullets, language
/// precodes and terminators. Two parsers for one tier is a divergence with
/// nothing holding it shut, and this one was the fallback the other fell back
/// TO, so both had to fail before anyone saw a difference.
pub fn wor_tier_from_input(input: &str, errors: &impl ErrorSink) -> Option<WorTier> {
    use chumsky::Parser as _;

    let lexed = crate::parser::LexedSource::new(input, crate::lexer::COND_MAIN_CONTENT);
    crate::parser::file::report_unrepresentable_bullet_times(&lexed, errors);
    let source = lexed.source_text();
    crate::parser::dependent_tiers::wor_tier_parser(source, source.whole())
        .parse(lexed.tokens())
        .into_result()
        .ok()
}

#[cfg(test)]
mod gra_lowering_tests {
    use talkbank_model::{ErrorCode, ErrorCollector, ParseOutcome};

    /// Numeric failures retain their field-specific diagnoses and no partial graph.
    #[test]
    fn numeric_rejection_never_returns_a_partial_graph() {
        for (body, code) in [
            (
                "1|0|ROOT 2|99999999999999999999|PUNCT\n",
                ErrorCode::UnexpectedGrammarNode,
            ),
            ("0|0|ROOT\n", ErrorCode::InvalidGrammarIndex),
            (
                "99999999999999999999|0|ROOT\n",
                ErrorCode::MalformedGrammarRelation,
            ),
        ] {
            let errors = ErrorCollector::new();
            assert!(matches!(
                crate::parser::parse_gra_tier_with_errors(body, &errors),
                ParseOutcome::Rejected
            ));
            assert_eq!(
                errors
                    .into_vec()
                    .iter()
                    .map(|error| error.code)
                    .collect::<Vec<_>>(),
                vec![code]
            );
        }
    }

    #[test]
    fn a_well_formed_tier_preserves_all_relations() {
        let errors = ErrorCollector::new();
        let tier =
            crate::parser::parse_gra_tier_with_errors("1|2|SUBJ 2|0|ROOT 3|2|OBJ\n", &errors)
                .into_option()
                .expect("complete GRA");
        assert!(errors.is_empty());
        assert_eq!(tier.relations().len(), 3);
    }
}
