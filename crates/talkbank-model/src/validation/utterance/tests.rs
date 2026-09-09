//! The one utterance-level claim no diagnostic expresses.
//!
//! # Why the other sixteen left
//!
//! They built `Utterance` values by hand and called `check_quotation_balance`,
//! `check_ca_delimiter_balance`, `check_overlap_index_values` and
//! `check_underline_balance` directly: a `MainTier` whose postcode stood in for
//! an opening quotation, a content list holding an `UnderlineBegin` with
//! nothing after it. That is the input stated twice, and the coverage it
//! produced was fabrication-backed: the check ran and no CHAT text was read.
//!
//! Four of the five codes are demonstrated by spec examples already. E242 is
//! the exception that matters: the spec's E242 examples exercise a curly-quote
//! scan in the PARSER, while `check_quotation_balance` reads POSTCODES (`"/`
//! and `"/.`), so the three deleted quotation tests were that rule's only
//! coverage. They are postcode rows now, with the messages they asserted. The
//! other variants moved in their original shapes, each with its exactly-once
//! count: an end then a balanced pair, two opens and one close, a nested end
//! inside a retrace group, two delimiter types interleaved. All are rows in
//! `talkbank-parser-tests/tests/integration/utterance_balance_from_source.rs`,
//! where the structure is whatever the parser builds.
//!
//! # What is left
//!
//! `analyze_ca_delimiter_roles` is `pub(crate)`, so no test outside this crate
//! can call it, and its return value is not a diagnostic: it is the roles
//! themselves, `Begin`/`End`/paired, which nothing published expresses. That
//! is the difference between the fabrication that left and the one that
//! remains.
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>

use crate::Span;
use crate::model::{
    CADelimiter, CADelimiterType, MainTier, Terminator, Utterance, UtteranceContent, Word,
    WordContent, WordText,
};
use crate::non_empty_literal;
use crate::validation::utterance::{CADelimiterRole, analyze_ca_delimiter_roles};

/// Delimiter role analysis assigns begin/end roles and pairing across tokens.
///
/// # Why this one did not move, when its sixteen siblings did
///
/// It asserts the RETURN VALUE of `analyze_ca_delimiter_roles`, which is
/// `pub(crate)`, so no test outside this crate can call it. The end-to-end
/// CONSEQUENCE of what it checks is covered honestly: this exact input is a
/// row in
/// `talkbank-parser-tests/tests/integration/utterance_balance_from_source.rs`,
/// asserting exactly one E230 that names the unpaired delimiter and not the
/// balanced pair.
/// What survives here is the claim no diagnostic expresses, that the roles
/// themselves come out `Begin`, `End`, `Begin` with the first two paired and
/// the third not.
///
/// So the hand-built utterance stays ON PURPOSE, and that is the line between
/// the fabrication that left and the fabrication that remains: this test's
/// subject is an internal function's output, not a verdict about a file.
#[test]
fn test_ca_delimiter_role_analysis_across_words() {
    let word1 = Word::new_unchecked("°soft", "soft").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::Text(WordText::from(non_empty_literal!("soft"))),
    ]);
    let word2 = Word::new_unchecked("more°", "more").with_content(vec![
        WordContent::Text(WordText::from(non_empty_literal!("more"))),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
    ]);
    let word3 = Word::new_unchecked("∆fast", "fast").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
        WordContent::Text(WordText::from(non_empty_literal!("fast"))),
    ]);

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
            UtteranceContent::Word(Box::new(word3)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let roles = analyze_ca_delimiter_roles(&utterance);

    assert_eq!(roles.len(), 3);
    assert_eq!(roles[0].delimiter_type, CADelimiterType::Softer);
    assert_eq!(roles[0].role, CADelimiterRole::Begin);
    assert!(roles[0].is_paired);

    assert_eq!(roles[1].delimiter_type, CADelimiterType::Softer);
    assert_eq!(roles[1].role, CADelimiterRole::End);
    assert!(roles[1].is_paired);

    assert_eq!(roles[2].delimiter_type, CADelimiterType::Faster);
    assert_eq!(roles[2].role, CADelimiterRole::Begin);
    assert!(!roles[2].is_paired);
}
