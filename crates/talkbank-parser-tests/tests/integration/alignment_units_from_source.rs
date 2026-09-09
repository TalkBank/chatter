// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Per-domain alignment-unit counting, over main tiers the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `alignment/helpers/tests.rs` built each item by hand
//! (`Word::new_unchecked("&~gaga", "gaga").with_category(WordCategory::Nonword)`,
//! `Separator::Comma { span: Span::DUMMY }`) and asserted what
//! `count_tier_positions` made of it per domain. Each stated the CHAT it meant
//! twice, and nothing checked that `&~gaga` parses to a `Nonword`. Here each
//! row is the utterance as written, and the content is what the parser
//! builds.
//!
//! # The count is over the WHOLE tier
//!
//! The originals counted an isolated item. A parsed tier carries the item in
//! context (a retrace needs something after it; a separator sits between
//! words), so each row's `what` states the arithmetic, and each row asserts
//! only the domains its original asserted: a value nobody adjudicated is not
//! pinned here by accident.
//!
//! # Every fixture states its validity
//!
//! Six rows are invalid CHAT on purpose (`XXX`, `Xxx`, `Yyy` and `Www` are
//! E241, `100_200` is E220, `&+fr [: word]` is E387) and say so; the helper
//! refuses any other verdict.

use talkbank_model::alignment::helpers::{PositionalDomain, TierDomain, count_tier_positions};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

struct Row {
    what: &'static str,
    utterance: &'static str,
    /// The validation verdict the fixture is expected to carry.
    codes: &'static [&'static str],
    /// The domains the original test asserted, with the count over the whole
    /// parsed tier.
    counts: &'static [(TierDomain, usize)],
}

use TierDomain::{Mor, Pho, Sin, Wor};

const ROWS: &[Row] = &[
    Row {
        what: "retrace: %mor skips retraced content; %pho and %wor count it (1 retraced + 1 word)",
        utterance: "<hello> [//] hello .",
        codes: &[],
        counts: &[(Mor, 1), (Pho, 2), (Wor, 2)],
    },
    Row {
        what: "replacement: %mor follows the two replacement words; %pho and %wor the one spoken form",
        utterance: "goed [: went home] .",
        codes: &[],
        counts: &[(Mor, 2), (Pho, 1), (Wor, 1)],
    },
    Row {
        what: "untranscribed xxx: excluded from %mor and %wor, counted by %pho",
        utterance: "xxx .",
        codes: &[],
        counts: &[(Mor, 0), (Pho, 1), (Wor, 0)],
    },
    Row {
        what: "untranscribed yyy: excluded from %mor and %wor, counted by %pho",
        utterance: "yyy .",
        codes: &[],
        counts: &[(Mor, 0), (Pho, 1), (Wor, 0)],
    },
    Row {
        what: "untranscribed www: excluded from %mor and %wor, counted by %pho",
        utterance: "www .",
        codes: &[],
        counts: &[(Mor, 0), (Pho, 1), (Wor, 0)],
    },
    Row {
        what: "uppercase XXX is illegal (E241) and still untranscribed for %mor",
        utterance: "XXX .",
        codes: &["E241"],
        counts: &[(Mor, 0)],
    },
    Row {
        what: "mixed-case Xxx is illegal (E241) and still untranscribed for %mor",
        utterance: "Xxx .",
        codes: &["E241"],
        counts: &[(Mor, 0)],
    },
    Row {
        what: "mixed-case Yyy is illegal (E241) and still untranscribed for %mor",
        utterance: "Yyy .",
        codes: &["E241"],
        counts: &[(Mor, 0)],
    },
    Row {
        what: "mixed-case Www is illegal (E241) and still untranscribed for %mor",
        utterance: "Www .",
        codes: &["E241"],
        counts: &[(Mor, 0)],
    },
    Row {
        what: "a timestamp-shaped token (E220) is %wor metadata, not a %wor slot; %mor unchanged",
        utterance: "100_200 .",
        codes: &["E220"],
        counts: &[(Wor, 0), (Mor, 1)],
    },
    Row {
        what: "comma is a tag marker for %mor (2 words + 1), nothing for %pho and %wor",
        utterance: "hello , world .",
        codes: &[],
        counts: &[(Mor, 3), (Pho, 2), (Wor, 2)],
    },
    Row {
        what: "colon is not a tag marker for %mor (2 words + 0), nothing for %pho and %wor",
        utterance: "hello : world .",
        codes: &[],
        counts: &[(Mor, 2), (Pho, 2), (Wor, 2)],
    },
    Row {
        what: "tag marker counts for %mor (2 words + 1)",
        utterance: "hello \u{201E} world .",
        codes: &[],
        counts: &[(Mor, 3)],
    },
    Row {
        what: "vocative marker counts for %mor (2 words + 1)",
        utterance: "hello \u{2021} world .",
        codes: &[],
        counts: &[(Mor, 3)],
    },
    Row {
        what: "retraced group: %mor skips it (2 trailing words); %pho, %sin and %wor count it (2 + 2)",
        utterance: "<hi there> [//] hi there .",
        codes: &[],
        counts: &[(Mor, 2), (Pho, 4), (Sin, 4), (Wor, 4)],
    },
    Row {
        what: "a pause is a %pho unit (1 + 1 word) and nothing for %mor, %sin, %wor",
        utterance: "(.) hello .",
        codes: &[],
        counts: &[(Pho, 2), (Mor, 1), (Sin, 1), (Wor, 1)],
    },
    Row {
        what: "a %pho group is one %pho unit and expands to 2 for %mor and %wor",
        utterance: "\u{2039}hi there\u{203A} .",
        codes: &[],
        counts: &[(Pho, 1), (Wor, 2), (Mor, 2)],
    },
    Row {
        what: "a %sin group is one %sin unit and expands to 2 for %mor and %wor",
        utterance: "\u{3014}hi there\u{3015} .",
        codes: &[],
        counts: &[(Sin, 1), (Wor, 2), (Mor, 2)],
    },
    Row {
        what: "a fragment with a replacement (E387) is excluded from %pho and from %wor",
        utterance: "&+fr [: word] .",
        codes: &["E387"],
        counts: &[(Pho, 0), (Wor, 0)],
    },
    Row {
        what: "a nonword (&~) is excluded from %wor and counted by %pho",
        utterance: "&~gaga .",
        codes: &[],
        counts: &[(Wor, 0), (Pho, 1)],
    },
    Row {
        what: "a fragment (&+) is excluded from %wor and counted by %pho",
        utterance: "&+fr .",
        codes: &[],
        counts: &[(Wor, 0), (Pho, 1)],
    },
    Row {
        what: "a filler (&-) is a real spoken word and IS a %wor slot",
        utterance: "&-um .",
        codes: &[],
        counts: &[(Wor, 1)],
    },
    Row {
        what: "OCSC 4009: the retraced word counts for %wor, the retraced fragment does not (1 + 1); %mor skips the retrace (1)",
        utterance: "<one &+ss> [/] one .",
        codes: &[],
        counts: &[(Mor, 1), (Wor, 2)],
    },
    Row {
        what: "OCSC 4026: four retraced words count for %wor, the retraced xxx does not (4 + 5)",
        utterance: "<a pumpkin and a xxx> [/] a pumpkin and a house .",
        codes: &[],
        counts: &[(Wor, 9)],
    },
];

/// Every row's counts hold over the tier the parser builds from it.
#[test]
fn every_domain_count_holds_over_a_parsed_tier() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in ROWS {
        let lines = format!("*CHI:\t{}", row.utterance);
        let main = match SingleSpeaker::english(&lines).main_tier(row.codes) {
            Ok(main) => main,
            Err(err) => {
                wrong.push(format!("{}: {err}", row.what));
                continue;
            }
        };
        for &(domain, want) in row.counts {
            // `%wor` has no count of its own: its slots are the projection's.
            let got = match PositionalDomain::try_from(domain) {
                Ok(positional) => count_tier_positions(&main.content.content, positional),
                Err(_) => main.wor_projection().slot_count().get(),
            };
            if got != want {
                wrong.push(format!(
                    "{}: {domain:?} counted {got}, expected {want}",
                    row.what
                ));
            }
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(wrong.join("\n")))
    }
}

/// The public per-word predicate IS the projection's rule: over every row,
/// counting the walker's words through `WorSlotMembershipPolicy::admits`
/// gives exactly the projection's slot count. A downstream consumer that
/// needs a per-item `%wor` count can therefore ask the policy and delete its
/// own copy of the rule (both Batchalign trees carried one on 2026-09-09).
#[test]
fn admits_is_the_projection_rule() -> Result<(), TestError> {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    let mut wrong = Vec::new();
    for row in ROWS {
        let lines = format!("*CHI:\t{}", row.utterance);
        let main = match SingleSpeaker::english(&lines).main_tier(row.codes) {
            Ok(main) => main,
            Err(err) => {
                wrong.push(format!("{}: {err}", row.what));
                continue;
            }
        };
        let projection = main.wor_projection();
        let policy = projection.membership_policy();
        let mut admitted = 0usize;
        walk_words(&main.content.content, Some(TierDomain::Wor), &mut |item| {
            let word = match item {
                WordItem::Word(word) => word,
                WordItem::ReplacedWord(replaced) => &replaced.word,
                WordItem::Separator(_) => return,
            };
            if policy.admits(word) {
                admitted += 1;
            }
        });
        let slots = projection.slot_count().get();
        if admitted != slots {
            wrong.push(format!(
                "{}: admits counted {admitted}, the projection holds {slots} slot(s)",
                row.what
            ));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(wrong.join("\n")))
    }
}
