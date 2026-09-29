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
/// parsed `%mor` or `%gra` tier. Unknown locations and similarly named tiers
/// remain visible; the recovered model's alignment taint is never cleared.
pub fn parse_lenient(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> (ChatFile, Vec<talkbank_model::ParseError>) {
    let errors = talkbank_model::ErrorCollector::new();
    let chat_file = parser.parse_chat_file_streaming(chat_text, &errors);
    let generated: Vec<_> = chat_file
        .utterances()
        .flat_map(|utterance| &utterance.dependent_tiers)
        .filter_map(|entry| GeneratedTier::from_tier(&entry.tier))
        .collect();
    let error_vec = errors.into_vec();
    let error_vec = error_vec
        .into_iter()
        .filter(|error| !generated.iter().any(|tier| tier.owns(error.location.span)))
        .collect();
    (chat_file, error_vec)
}

/// Only actual generated tiers from this parse can own ignored diagnostics.
/// Holding typed payloads prevents similarly named free-text tiers from being
/// admitted by a string-prefix comparison; their full spans include continuations.
enum GeneratedTier<'file> {
    Mor(&'file MorTier),
    Gra(&'file GraTier),
}

impl<'file> GeneratedTier<'file> {
    fn from_tier(tier: &'file DependentTier) -> Option<Self> {
        match tier {
            DependentTier::Mor(tier) => Some(Self::Mor(tier)),
            DependentTier::Gra(tier) => Some(Self::Gra(tier)),
            _ => None,
        }
    }

    fn owns(&self, diagnostic: talkbank_model::Span) -> bool {
        let owner = match self {
            Self::Mor(tier) => tier.span,
            Self::Gra(tier) => tier.span,
        };
        !owner.is_dummy()
            && !diagnostic.is_dummy()
            && owner.start <= diagnostic.start
            && diagnostic.start < owner.end
            && diagnostic.start <= diagnostic.end
            && diagnostic.end <= owner.end
    }
}

/// Parse CHAT text strictly (tree-sitter, no error recovery).
///
/// This reproduces the pre-`ParseProduct` strict contract explicitly and
/// deliberately: a caller of `parse_strict` wants "did this text parse
/// completely cleanly," not "give me whatever model was built." A
/// [`talkbank_parser::ParseProduct::Built`] that carries an error-severity
/// diagnostic is therefore an `Err` here, the same as
/// [`talkbank_parser::ParseProduct::Unbuildable`], even though the
/// underlying parser call did build a model in the `Built` case.
pub fn parse_strict(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> Result<ChatFile, talkbank_model::ParseErrors> {
    match parser.parse_chat_file(chat_text) {
        talkbank_parser::ParseProduct::Built { file, diagnostics } => {
            if diagnostics
                .iter()
                .any(|d| matches!(d.severity, talkbank_model::Severity::Error))
            {
                Err(talkbank_model::ParseErrors::from(diagnostics))
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
        .options
        .iter()
        .any(|f| matches!(f, ChatOptionFlag::Unsupported(s) if s == "dummy"))
}

/// Check whether a parsed CHAT file has `@Options: NoAlign`.
///
/// Files with `NoAlign` should skip forced alignment and be output unchanged by
/// alignment-oriented commands.
pub fn is_no_align(chat_file: &ChatFile) -> bool {
    chat_file.options.iter().any(|f| f.skips_alignment())
}

/// Check whether a parsed CHAT file carries the `@Options: CA` flag.
///
/// A CONSUMER POLICY, not a format fact: this crate skips morphotagging on such
/// files by default. It asks about the flag's presence, so it does not name a
/// `CaOptionEffect`; the flag says nothing about whether the file uses
/// CA-originated notation.
pub fn is_ca(chat_file: &ChatFile) -> bool {
    chat_file
        .options
        .iter()
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
    fn test_lenient_parse_ignores_malformed_gra_tier() {
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
            matches!(tiers.first(), Some(talkbank_model::model::DependentTier::Gra(g)) if g.relations().is_empty()),
            "malformed %gra should remain as an empty placeholder in its original slot"
        );
        assert!(
            matches!(
                tiers.get(1),
                Some(talkbank_model::model::DependentTier::Wor(_))
            ),
            "placeholder %gra must remain before existing %wor to avoid tier reordering"
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
            matches!(utterance.mor_tier(), Some(m) if m.items().is_empty()),
            "malformed %mor should remain as an empty placeholder tier"
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
