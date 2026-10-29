//! CHAT parsing helpers for transform-oriented callers.
//!
//! These are thin convenience wrappers over `talkbank-parser` that keep
//! common parse-and-validate call patterns available in one place.

use talkbank_model::model::{ChatFile, DependentTier, GraTier, MorTier};
pub use talkbank_parser::TreeSitterParser;

/// Parse CHAT text leniently (tree-sitter with error recovery).
///
/// Always returns a `ChatFile` (best-effort), plus any parse warnings/errors.
///
/// Lenient parsing intentionally ignores malformed existing `%mor` / `%gra`
/// tiers: their slots stay present in the recovered AST, but their parse
/// diagnostics are not surfaced through this helper. Main-tier and header
/// parse failures still come back in `error_vec`.
/// Suppression requires the diagnostic's complete span to belong to an actual
/// typed `%mor` or `%gra` slot, including explicit unparsed variants. Unknown
/// locations and similarly named tiers remain visible; recovery invalidity and
/// the model's alignment taint are never cleared.
/// If the attempt reports an internal failure, all diagnostics are retained
/// without suppression, regardless of severity. The returned recovery model
/// does not certify completion or validity; callers must inspect the findings.
pub fn parse_lenient(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> (ChatFile, Vec<talkbank_model::ParseError>) {
    let errors = talkbank_model::ErrorCollector::new();
    let chat_file = parser.parse_chat_file_streaming(chat_text, &errors);
    lenient_product(chat_file, errors.into_vec())
}

fn lenient_product(
    chat_file: ChatFile,
    error_vec: Vec<talkbank_model::ParseError>,
) -> (ChatFile, Vec<talkbank_model::ParseError>) {
    let retained = match talkbank_model::CompletedDiagnostics::admit(error_vec) {
        Ok(completed) => suppress_generated_diagnostics(&chat_file, completed),
        Err(failure) => failure.into_diagnostics(),
    };
    (chat_file, retained)
}

/// Only a completed attempt may apply the generated-tier suppression policy.
fn suppress_generated_diagnostics(
    chat_file: &ChatFile,
    completed: talkbank_model::CompletedDiagnostics,
) -> Vec<talkbank_model::ParseError> {
    let generated: Vec<_> = chat_file
        .utterances()
        .flat_map(|utterance| &utterance.dependent_tiers)
        .filter_map(|entry| GeneratedTier::from_tier(&entry.tier))
        .collect();
    completed
        .into_diagnostics()
        .into_iter()
        // An unlocated diagnostic lies in no generated tier, so it is kept.
        .filter(|error| {
            !error
                .location
                .span()
                .is_some_and(|span| generated.iter().any(|tier| tier.owns(span)))
        })
        .collect()
}

/// Only actual generated tiers from this parse can own ignored diagnostics.
/// Holding typed payloads prevents similarly named free-text tiers from being
/// admitted by a string-prefix comparison; their full spans include continuations.
enum GeneratedTier<'file> {
    Mor(&'file MorTier),
    UnparsedMor(&'file talkbank_model::UnparsedTier),
    Gra(&'file GraTier),
    UnparsedGra(&'file talkbank_model::UnparsedTier),
}

impl<'file> GeneratedTier<'file> {
    fn from_tier(tier: &'file DependentTier) -> Option<Self> {
        match tier {
            DependentTier::Mor(tier) => Some(Self::Mor(tier)),
            DependentTier::UnparsedMor(tier) => Some(Self::UnparsedMor(tier)),
            DependentTier::Gra(tier) => Some(Self::Gra(tier)),
            DependentTier::UnparsedGra(tier) => Some(Self::UnparsedGra(tier)),
            _ => None,
        }
    }

    fn owns(&self, diagnostic: talkbank_model::Span) -> bool {
        let owner = match self {
            Self::Mor(tier) => tier.location,
            Self::UnparsedMor(tier) => tier.location(),
            Self::Gra(tier) => tier.location,
            Self::UnparsedGra(tier) => tier.location(),
        };
        // Only a located generated tier owns anything.
        owner.span().is_some_and(|owner| {
            owner.contains_span(diagnostic) && diagnostic.start() < owner.end()
        })
    }
}

/// Parse CHAT text, refusing incomplete attempts and error-severity findings.
///
/// This reproduces the pre-`ParseProduct` strict contract explicitly and
/// deliberately: a caller of `parse_strict` requires a completed, error-free
/// parse, not merely a recovered model. Ordinary warnings are allowed. A
/// [`talkbank_parser::ParseProduct::Built`] that carries an error-severity
/// diagnostic is therefore an `Err` here, the same as
/// [`talkbank_parser::ParseProduct::Unbuildable`], even though the
/// underlying parser call did build a model in the `Built` case.
/// Internal failures are refused regardless of severity. Success does not
/// certify model validity or lossless serialization.
pub fn parse_strict(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> Result<ChatFile, talkbank_model::ParseErrors> {
    strict_product(parser.parse_chat_file(chat_text))
}

fn strict_product(
    product: talkbank_parser::ParseProduct,
) -> Result<ChatFile, talkbank_model::ParseErrors> {
    match product {
        talkbank_parser::ParseProduct::Built { file, diagnostics } => {
            let completed = talkbank_model::CompletedDiagnostics::admit(diagnostics)
                .map_err(|failure| talkbank_model::ParseErrors::from(failure.into_diagnostics()))?;
            if completed
                .diagnostics()
                .iter()
                .any(|d| matches!(d.severity, talkbank_model::Severity::Error))
            {
                Err(talkbank_model::ParseErrors::from(
                    completed.into_diagnostics(),
                ))
            } else {
                Ok(file)
            }
        }
        talkbank_parser::ParseProduct::Unbuildable { diagnostics } => {
            Err(talkbank_model::ParseErrors::from(diagnostics))
        }
    }
}

/// Check whether a parsed CHAT file has `@Options: dummy`.
///
/// Dummy files are pass-through placeholders that should not be processed by
/// any NLP pipeline.
pub fn is_dummy(chat_file: &ChatFile) -> bool {
    use talkbank_model::model::ChatOptionFlag;

    chat_file
        .option_flags()
        .any(|f| matches!(f, ChatOptionFlag::Unsupported(s) if s == "dummy"))
}

/// Check whether a parsed CHAT file has `@Options: NoAlign`.
///
/// Files with `NoAlign` should skip forced alignment and be output unchanged by
/// alignment-oriented commands.
pub fn is_no_align(chat_file: &ChatFile) -> bool {
    chat_file.option_flags().any(|f| f.skips_alignment())
}

/// Check whether a parsed CHAT file carries the `@Options: CA` flag.
///
/// A CONSUMER POLICY, not a format fact: this crate skips morphotagging on such
/// files by default. It asks about the flag's presence, so it does not name a
/// `CaOptionEffect`; the flag says nothing about whether the file uses
/// CA-originated notation.
pub fn is_ca(chat_file: &ChatFile) -> bool {
    chat_file
        .option_flags()
        .any(|f| matches!(f, talkbank_model::model::ChatOptionFlag::Ca))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello world .\n@End\n";

    const DUMMY_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tdummy\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    const NOALIGN_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tNoAlign\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    const BULLETS_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tbullets\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    #[test]
    fn lenient_product_preserves_failed_attempt_diagnostics() {
        use talkbank_model::{ErrorCode, ParseError, Severity};

        let parser = TreeSitterParser::new().expect("parser");
        let file = parse_strict(
            &parser,
            include_str!("../../../corpus/reference/tiers/mor-gra.cha"),
        )
        .expect("reference parses");
        let spans: Vec<_> = file
            .utterances()
            .flat_map(|utterance| &utterance.dependent_tiers)
            .filter_map(|entry| match &entry.tier {
                DependentTier::Mor(tier) => tier.location.span(),
                DependentTier::Gra(tier) => tier.location.span(),
                _ => None,
            })
            .collect();
        assert!(spans.len() >= 2, "reference includes generated tiers");
        for span in spans {
            for severity in [Severity::Error, Severity::Warning] {
                let findings = vec![
                    ParseError::at_span(
                        ErrorCode::SyntaxError,
                        Severity::Error,
                        span,
                        "input finding",
                    ),
                    ParseError::at_span(ErrorCode::InternalError, severity, span, "producer fault"),
                ];
                let (_, retained) = lenient_product(file.clone(), findings);
                assert_eq!(retained.len(), 2, "a failed attempt cannot be suppressed");
                assert_eq!(retained[0].code, ErrorCode::SyntaxError);
                assert_eq!(retained[1].code, ErrorCode::InternalError);
                assert_eq!(retained[1].severity, severity);
            }
        }
    }

    #[test]
    fn strict_product_refuses_internal_warnings() {
        use talkbank_model::{ErrorCode, ParseError, Severity, Span};
        use talkbank_parser::ParseProduct;

        let parser = TreeSitterParser::new().expect("parser");
        let file = parse_strict(
            &parser,
            include_str!("../../../corpus/reference/core/basic-conversation.cha"),
        )
        .expect("reference parses");
        let fault = ParseError::at_span(
            ErrorCode::InternalError,
            Severity::Warning,
            Span::new(0, 1),
            "producer fault",
        );
        for product in [
            ParseProduct::Built {
                file: file.clone(),
                diagnostics: vec![fault.clone()],
            },
            ParseProduct::Unbuildable {
                diagnostics: vec![fault],
            },
        ] {
            let errors = strict_product(product)
                .expect_err("internal failure")
                .into_error_vec();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, ErrorCode::InternalError);
            assert_eq!(errors[0].severity, Severity::Warning);
        }
        for severity in [Severity::Warning, Severity::Error] {
            let diagnostic = ParseError::at_span(
                ErrorCode::UnknownAnnotation,
                severity,
                Span::new(0, 1),
                "input finding",
            );
            let result = strict_product(ParseProduct::Built {
                file: file.clone(),
                diagnostics: vec![diagnostic],
            });
            match severity {
                Severity::Warning => assert!(result.is_ok()),
                Severity::Error => {
                    let errors = result.expect_err("input error").into_error_vec();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, ErrorCode::UnknownAnnotation);
                }
            }
        }
        assert!(
            strict_product(ParseProduct::Unbuildable {
                diagnostics: vec![]
            })
            .is_err()
        );
    }

    #[test]
    fn test_parse_strict_round_trip_basics() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let chat_file = parse_strict(&parser, MINIMAL_CHAT).expect("strict parse");
        assert_eq!(chat_file.utterances().count(), 1);
    }

    #[test]
    fn test_is_dummy_with_dummy_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, DUMMY_CHAT);
        assert!(is_dummy(&chat_file));
    }

    #[test]
    fn test_is_dummy_without_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, MINIMAL_CHAT);
        assert!(!is_dummy(&chat_file));
    }

    #[test]
    fn test_is_dummy_with_other_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, BULLETS_CHAT);
        assert!(!is_dummy(&chat_file));
    }

    #[test]
    fn test_is_no_align_with_noalign_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, NOALIGN_CHAT);
        assert!(is_no_align(&chat_file));
    }

    #[test]
    fn test_is_no_align_without_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, MINIMAL_CHAT);
        assert!(!is_no_align(&chat_file));
    }

    #[test]
    fn test_is_no_align_with_dummy_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, DUMMY_CHAT);
        assert!(!is_no_align(&chat_file));
    }

    #[test]
    fn test_lenient_parse_retains_rejected_gra_source() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\twake up .\n\
%gra:\t1|0|ROOT 2|1|compound:prt 3|1|PUNCT\n%wor:\twake up .\n@End\n";
        let (chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            errors.is_empty(),
            "malformed %gra should be ignored in lenient parse"
        );
        let utterance = chat_file.utterances().next().expect("utterance");
        let tiers: Vec<_> = utterance
            .dependent_tiers
            .iter()
            .map(|entry| &entry.tier)
            .collect();
        assert!(
            matches!(tiers.first(), Some(talkbank_model::model::DependentTier::UnparsedGra(raw)) if input.contains(raw.source())),
            "malformed %gra retains its rejected source in its original slot"
        );
        assert!(
            matches!(
                tiers.get(1),
                Some(talkbank_model::model::DependentTier::Wor(_))
            ),
            "rejected %gra must remain before existing %wor to avoid tier reordering"
        );
    }

    #[test]
    fn test_lenient_parse_ignores_malformed_mor_tier() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello .\n\
%mor:\t|hello .\n@End\n";
        let (chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            errors.is_empty(),
            "malformed %mor should be ignored in lenient parse"
        );
        let utterance = chat_file.utterances().next().expect("utterance");
        assert!(
            matches!(&utterance.dependent_tiers[0].tier,
                DependentTier::UnparsedMor(tier) if tier.source() == "%mor:\t|hello .\n"),
            "malformed MOR retains its original source, not invented morphology"
        );
    }

    #[test]
    fn test_lenient_parse_still_reports_main_tier_errors() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello @ .\n@End\n";
        let (_chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            !errors.is_empty(),
            "lenient parse must still surface main-tier parse errors"
        );
    }
}
