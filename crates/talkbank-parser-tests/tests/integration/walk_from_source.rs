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
//! The content-tree walkers and the positional counter, over main tiers the
//! parser builds.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `alignment/helpers/walk/tests.rs` built every content
//! list by hand: `UtteranceContent::Word(Box::new(Word::simple("hello")))`,
//! a `Separator::Comma { span: Span::DUMMY }`, a `Pause` at `Span::DUMMY`,
//! an `Annotated` wrapper at `Span::DUMMY`, and a bare `Group`, which no
//! valid CHAT spells. Here every content list is what the parser builds from
//! a line, so a walker is exercised over the shapes the parser actually
//! produces, recovered ones included, and each row says which codes its
//! line reports (`&[]`: valid CHAT).
//!
//! # What parsed text changed
//!
//! - A bare `Group` (`<w>` with no marker after it) is a recovery shape:
//!   tree-sitter inserts a MISSING retrace marker, E342, and the group stays
//!   bare. Until the commit that wrote this row the annotation decoder read
//!   the placeholder's kind and built a `Full` retrace nobody wrote, so this
//!   row was the failing test of that fix. The rows about descent INTO a
//!   group use `<...> [?]`, an annotated group under the one annotation no
//!   descent rule reads.
//! - `⌈ hello ⌉`, spaced as the spec's standalone-overlap example writes
//!   it, builds a begin AND an end overlap point as content items, so the
//!   overlap row sees two where the hand-built list had a lone begin.
//!   Unspaced, `⌈hello⌉` is one word carrying its points as word content,
//!   and the content walk sees no overlap item at all.
//! - A retrace with nothing after it is E370 (structural order). The descent
//!   rows about retraces carry that code rather than pad the line with a
//!   word the count would then have to subtract.
//! - The descent table's nested level is an annotated group; a group under
//!   `[?]` is entered under every domain, so the inner verdict shows through
//!   exactly as it did through the hand-built bare one.
//!
//! # What survives, and as what
//!
//! The pho/sin opacity rows are POLICY (a choice with a real alternative,
//! descending into their words). The walk_content-versus-walk_words row is a
//! ROUNDTRIP between two functions. The descent table is a MEASUREMENT of
//! two hand-written arm groupings against one shared rule, and now also pins
//! that the parser builds the variant each row names.

use talkbank_model::alignment::helpers::{
    ContentItem, PositionalDomain, TierDomain, WordItem, WordItemMut, count_tier_positions,
    walk_content, walk_words, walk_words_mut,
};
use talkbank_model::model::{BracketedItem, Bullet, MainTier, UtteranceContent};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// The main tier the parser builds for `*CHI:<tab>{line}`, the whole fixture
/// having reported exactly `codes`.
fn tier(line: &str, codes: &[&str]) -> Result<MainTier, TestError> {
    let lines = format!("*CHI:\t{line}");
    SingleSpeaker::english(&lines).main_tier(codes)
}

/// The cleaned text of every word the walk emits, in order.
fn word_texts(content: &[UtteranceContent], domain: Option<TierDomain>) -> Vec<String> {
    let mut texts = Vec::new();
    walk_words(content, domain, &mut |leaf| {
        if let WordItem::Word(word) = leaf {
            texts.push(word.cleaned_text().to_string());
        }
    });
    texts
}

/// Which words `walk_words` emits for a line under a domain: flat words,
/// words inside a bare (recovered) and an annotated group, a retrace under
/// the domain that skips it and the two that do not, a phonological and a
/// sign group under the domains that treat them as opaque and the three that
/// descend, and a quotation.
///
/// The pho/sin cells walk BOTH kinds under BOTH measuring domains: the rule
/// pairs Pho with Sin on each side, so the cross cells (a pho group under
/// Sin, a sin group under Pho) are governed too. `Wor` is the cell most
/// worth pinning: `count.rs` also descends there, so a drift between the
/// walk and the count would show up in it first. **The `%sin` half is
/// unreachable from real data** (no file in a ~106,000-file corpus carried a
/// `%sin` tier when this was measured on 2026-08-26), so no comparison over
/// corpora is evidence about the Sin domain; these rows are.
#[test]
fn leaf_words_by_domain() -> Result<(), TestError> {
    use TierDomain::{Mor, Pho, Sin, Wor};
    struct Row {
        line: &'static str,
        codes: &'static [&'static str],
        domain: Option<TierDomain>,
        words: &'static [&'static str],
    }
    let rows = [
        Row {
            line: "hello world .",
            codes: &[],
            domain: None,
            words: &["hello", "world"],
        },
        // Recovered: the missing marker is E342 and the group stays bare.
        Row {
            line: "<in group> .",
            codes: &["E342"],
            domain: None,
            words: &["in", "group"],
        },
        Row {
            line: "<in group> [?] .",
            codes: &[],
            domain: None,
            words: &["in", "group"],
        },
        Row {
            line: "<retraced> [/] kept .",
            codes: &[],
            domain: Some(Mor),
            words: &["kept"],
        },
        Row {
            line: "<retraced> [/] kept .",
            codes: &[],
            domain: None,
            words: &["retraced", "kept"],
        },
        Row {
            line: "<retraced> [/] kept .",
            codes: &[],
            domain: Some(Wor),
            words: &["retraced", "kept"],
        },
        Row {
            line: "\u{2039}inside\u{203a} after .",
            codes: &[],
            domain: Some(Pho),
            words: &["after"],
        },
        Row {
            line: "\u{2039}inside\u{203a} after .",
            codes: &[],
            domain: Some(Sin),
            words: &["after"],
        },
        Row {
            line: "\u{2039}inside\u{203a} after .",
            codes: &[],
            domain: Some(Mor),
            words: &["inside", "after"],
        },
        Row {
            line: "\u{2039}inside\u{203a} after .",
            codes: &[],
            domain: Some(Wor),
            words: &["inside", "after"],
        },
        Row {
            line: "\u{2039}inside\u{203a} after .",
            codes: &[],
            domain: None,
            words: &["inside", "after"],
        },
        Row {
            line: "\u{3014}inside\u{3015} after .",
            codes: &[],
            domain: Some(Pho),
            words: &["after"],
        },
        Row {
            line: "\u{3014}inside\u{3015} after .",
            codes: &[],
            domain: Some(Sin),
            words: &["after"],
        },
        Row {
            line: "\u{3014}inside\u{3015} after .",
            codes: &[],
            domain: Some(Mor),
            words: &["inside", "after"],
        },
        Row {
            line: "\u{3014}inside\u{3015} after .",
            codes: &[],
            domain: Some(Wor),
            words: &["inside", "after"],
        },
        Row {
            line: "\u{3014}inside\u{3015} after .",
            codes: &[],
            domain: None,
            words: &["inside", "after"],
        },
        Row {
            line: "\u{201c}quoted\u{201d} .",
            codes: &[],
            domain: None,
            words: &["quoted"],
        },
    ];
    let mut wrong = Vec::new();
    for row in &rows {
        let tier = tier(row.line, row.codes)?;
        let got = word_texts(&tier.content.content, row.domain);
        if got != row.words {
            wrong.push(format!(
                "{:?} under {:?}: expected {:?}, got {got:?}",
                row.line, row.domain, row.words
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}

/// A comma is a separator leaf, emitted once.
#[test]
fn a_comma_is_yielded_as_a_separator() -> Result<(), TestError> {
    let tier = tier("hello , world .", &[])?;
    let mut separators = 0;
    walk_words(&tier.content.content, None, &mut |leaf| {
        if let WordItem::Separator(_) = leaf {
            separators += 1;
        }
    });
    assert_eq!(separators, 1);
    Ok(())
}

/// The mutable walk reaches every word the immutable walk does: a bullet set
/// through one is seen by the other.
#[test]
fn the_mutable_walk_reaches_every_word() -> Result<(), TestError> {
    let mut tier = tier("hello world .", &[])?;
    walk_words_mut(tier.content.content.as_mut_slice(), None, &mut |leaf| {
        if let WordItemMut::Word(word) = leaf {
            word.inline_bullet = Some(Bullet::new(0, 100));
        }
    });
    let mut with_bullet = 0;
    walk_words(&tier.content.content, None, &mut |leaf| {
        if let WordItem::Word(word) = leaf {
            assert!(
                word.inline_bullet.is_some(),
                "{:?} kept no bullet",
                word.raw_text()
            );
            with_bullet += 1;
        }
    });
    assert_eq!(with_bullet, 2);
    Ok(())
}

/// How many items of each kind `walk_content` emits. Exhaustive over
/// `ContentItem`, so a new kind of item must be counted here to compile.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    words: usize,
    replaced_words: usize,
    separators: usize,
    events: usize,
    pauses: usize,
    actions: usize,
    overlap_points: usize,
    other_spoken_events: usize,
    freecodes: usize,
    internal_bullets: usize,
    long_feature_begins: usize,
    long_feature_ends: usize,
    underline_begins: usize,
    underline_ends: usize,
    nonvocal_begins: usize,
    nonvocal_ends: usize,
    nonvocal_simples: usize,
}

fn count_items(content: &[UtteranceContent], domain: Option<TierDomain>) -> Counts {
    let mut counts = Counts::default();
    walk_content(content, domain, &mut |item| match item {
        ContentItem::Word(_) => counts.words += 1,
        ContentItem::ReplacedWord(_) => counts.replaced_words += 1,
        ContentItem::Separator(_) => counts.separators += 1,
        ContentItem::Event(_) => counts.events += 1,
        ContentItem::Pause(_) => counts.pauses += 1,
        ContentItem::Action(_) => counts.actions += 1,
        ContentItem::OverlapPoint(_) => counts.overlap_points += 1,
        ContentItem::OtherSpokenEvent(_) => counts.other_spoken_events += 1,
        ContentItem::Freecode(_) => counts.freecodes += 1,
        ContentItem::InternalBullet(_) => counts.internal_bullets += 1,
        ContentItem::LongFeatureBegin(_) => counts.long_feature_begins += 1,
        ContentItem::LongFeatureEnd(_) => counts.long_feature_ends += 1,
        ContentItem::UnderlineBegin(_) => counts.underline_begins += 1,
        ContentItem::UnderlineEnd(_) => counts.underline_ends += 1,
        ContentItem::NonvocalBegin(_) => counts.nonvocal_begins += 1,
        ContentItem::NonvocalEnd(_) => counts.nonvocal_ends += 1,
        ContentItem::NonvocalSimple(_) => counts.nonvocal_simples += 1,
    });
    counts
}

/// What `walk_content` emits beyond words: an event and a pause between
/// words, the event inside an annotated event, both overlap points around a
/// word, the word and the event inside a group, and a phonological group's
/// word under the domain that treats the group as opaque and under none.
/// Each row states the WHOLE count, so an item kind the row did not expect
/// is a failure too.
#[test]
fn content_items_by_kind() -> Result<(), TestError> {
    struct Row {
        line: &'static str,
        domain: Option<TierDomain>,
        counts: Counts,
    }
    let rows = [
        Row {
            line: "hello &=laughs (.) world .",
            domain: None,
            counts: Counts {
                words: 2,
                events: 1,
                pauses: 1,
                ..Counts::default()
            },
        },
        Row {
            line: "&=coughs [?] .",
            domain: None,
            counts: Counts {
                events: 1,
                ..Counts::default()
            },
        },
        Row {
            line: "hi \u{2308} hello \u{2309} .",
            domain: None,
            counts: Counts {
                words: 2,
                overlap_points: 2,
                ..Counts::default()
            },
        },
        Row {
            line: "<inside &=claps> [?] .",
            domain: None,
            counts: Counts {
                words: 1,
                events: 1,
                ..Counts::default()
            },
        },
        Row {
            line: "\u{2039}phonological\u{203a} after .",
            domain: Some(TierDomain::Pho),
            counts: Counts {
                words: 1,
                ..Counts::default()
            },
        },
        Row {
            line: "\u{2039}phonological\u{203a} after .",
            domain: None,
            counts: Counts {
                words: 2,
                ..Counts::default()
            },
        },
    ];
    let mut wrong = Vec::new();
    for row in &rows {
        let tier = tier(row.line, &[])?;
        let got = count_items(&tier.content.content, row.domain);
        if got != row.counts {
            wrong.push(format!(
                "{:?} under {:?}: expected {:?}, got {got:?}",
                row.line, row.domain, row.counts
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}

/// The words `walk_content` emits are the words `walk_words` emits: a
/// roundtrip between two traversals of one list.
#[test]
fn walk_content_and_walk_words_agree_on_words() -> Result<(), TestError> {
    let tier = tier("hello , world .", &[])?;
    let mut through_content = Vec::new();
    walk_content(&tier.content.content, None, &mut |item| {
        if let ContentItem::Word(word) = item {
            through_content.push(word.cleaned_text().to_string());
        }
    });
    assert_eq!(through_content, word_texts(&tier.content.content, None));
    Ok(())
}

/// The container descent table, measured through both traversal families,
/// over containers the parser built.
///
/// `helpers::descent` owns the RULE for every traversal. What is still
/// hand-written is the ARM GROUPING: `count.rs`'s traversals and the walkers
/// each list the container variants themselves before handing off, so a new
/// container variant can land in the wrong group and compile. That is what
/// this table catches, and why each row is read through both families
/// rather than through `descend` directly.
///
/// # The probe distinguishes three states, which is the point
///
/// Each container holds exactly ONE word, so the PAIR of answers says which
/// of three things the rule decided, where a walker's boolean view cannot:
///
/// | count | words | meaning |
/// |---|---|---|
/// | 1 | 1 | `Into`: the traversal descends and finds the word |
/// | 1 | 0 | `Atomic`: ONE alignable position, contributing no words |
/// | 0 | 0 | `Excluded`: contributes nothing at all |
/// | 0 | 1 | IMPOSSIBLE: a word reached the walk that the count does not know about |
///
/// Only four of the thirty-six cells are `Atomic`, all of them a
/// phonological or sign group under a domain that measures it, and half of
/// those are unreachable from any real corpus (no `%sin` tier exists in the
/// data). Each row also pins that the parser built the variant it names, at
/// both levels, since a row whose line built some other container would
/// measure the wrong arms and could still pass.
///
/// SURVIVES a type, and says which kind: a MEASUREMENT, of two arm groupings
/// against one shared rule.
#[test]
fn container_descent_table_is_one_rule_for_both_consumers() -> Result<(), TestError> {
    /// What the rule decided, read off the pair of consumer answers.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum Decided {
        Into,
        Atomic,
        Excluded,
    }
    use Decided::{Atomic, Excluded, Into};

    /// The same container one level down, inside an annotated group: the
    /// line, its codes, and the `BracketedItem` variant it must have built.
    struct Nested {
        line: &'static str,
        codes: &'static [&'static str],
        is: fn(&BracketedItem) -> bool,
    }

    /// One container at both levels, with the variant each level must have
    /// built and the row of verdicts it must produce under `DOMAINS`.
    ///
    /// `nested` is `None` only for the bare `Group`: nested inside another
    /// group (`<<w>> [?]`) it is E316 and no inner group is built, so the
    /// `BracketedItem::Group` arm has no parsed spelling, as the hand-built
    /// table had none for it either.
    struct Row {
        name: &'static str,
        line: &'static str,
        codes: &'static [&'static str],
        is: fn(&UtteranceContent) -> bool,
        nested: Option<Nested>,
        expected: [Decided; 4],
    }

    const DOMAINS: [TierDomain; 4] = [
        TierDomain::Mor,
        TierDomain::Pho,
        TierDomain::Sin,
        TierDomain::Wor,
    ];

    let rows = [
        Row {
            name: "Group",
            line: "<w> .",
            codes: &["E342"],
            is: |c| matches!(c, UtteranceContent::Group(_)),
            nested: None,
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedGroup{}",
            line: "<w> [?] .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::AnnotatedGroup(_)),
            nested: Some(Nested {
                line: "<<w> [?]> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::AnnotatedGroup(_)),
            }),
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedGroup[e]",
            line: "<w> [e] .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::AnnotatedGroup(_)),
            nested: Some(Nested {
                line: "<<w> [e]> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::AnnotatedGroup(_)),
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "Quotation",
            line: "\u{201c}w\u{201d} .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::Quotation(_)),
            nested: Some(Nested {
                line: "<\u{201c}w\u{201d}> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::Quotation(_)),
            }),
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedQuotation[e]",
            line: "\u{201c}w\u{201d} [e] .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::AnnotatedQuotation(_)),
            nested: Some(Nested {
                line: "<\u{201c}w\u{201d} [e]> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::AnnotatedQuotation(_)),
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "PhoGroup",
            line: "\u{2039}w\u{203a} .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::PhoGroup(_)),
            nested: Some(Nested {
                line: "<\u{2039}w\u{203a}> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::PhoGroup(_)),
            }),
            expected: [Into, Atomic, Excluded, Into],
        },
        Row {
            name: "SinGroup",
            line: "\u{3014}w\u{3015} .",
            codes: &[],
            is: |c| matches!(c, UtteranceContent::SinGroup(_)),
            nested: Some(Nested {
                line: "<\u{3014}w\u{3015}> [?] .",
                codes: &[],
                is: |b| matches!(b, BracketedItem::SinGroup(_)),
            }),
            expected: [Into, Excluded, Atomic, Into],
        },
        Row {
            name: "Retrace",
            line: "<w> [/] .",
            codes: &["E370"],
            is: |c| matches!(c, UtteranceContent::Retrace(_)),
            nested: Some(Nested {
                line: "<<w> [/]> [?] .",
                codes: &["E370"],
                is: |b| matches!(b, BracketedItem::Retrace(_)),
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "AnnotatedRetrace",
            line: "<w> [/] [?] .",
            codes: &["E370"],
            is: |c| matches!(c, UtteranceContent::AnnotatedRetrace(_)),
            nested: Some(Nested {
                line: "<<w> [/] [?]> [?] .",
                codes: &["E370"],
                is: |b| matches!(b, BracketedItem::AnnotatedRetrace(_)),
            }),
            expected: [Excluded, Into, Into, Into],
        },
    ];

    /// Read the decision off the counter against the walk. `%wor` has no
    /// count of its own here (its count is the projection's, which is this
    /// walk), so its column reads the walk alone: a container is never
    /// atomic under `%wor`, so the walk sees every verdict.
    fn decide(content: &[UtteranceContent], domain: TierDomain) -> Result<Decided, String> {
        let words = word_texts(content, Some(domain)).len();
        let counted = match PositionalDomain::try_from(domain) {
            Ok(positional) => count_tier_positions(content, positional),
            Err(_) => words,
        };
        match (counted, words) {
            (1, 1) => Ok(Into),
            (1, 0) => Ok(Atomic),
            (0, 0) => Ok(Excluded),
            other => Err(format!("impossible pair {other:?}")),
        }
    }

    let mut wrong = Vec::new();
    for row in &rows {
        let top = tier(row.line, row.codes)?;
        let content: &[UtteranceContent] = &top.content.content;
        match content {
            [one] if (row.is)(one) => {}
            other => {
                wrong.push(format!(
                    "{}: {:?} did not build one {} but {other:?}",
                    row.name, row.line, row.name
                ));
                continue;
            }
        }
        for (column, domain) in DOMAINS.iter().enumerate() {
            match decide(content, *domain) {
                Ok(got) if got == row.expected[column] => {}
                Ok(got) => wrong.push(format!(
                    "{} / {domain:?}: expected {:?}, got {got:?}",
                    row.name, row.expected[column]
                )),
                Err(err) => wrong.push(format!("{} / {domain:?}: {err}", row.name)),
            }
        }
        // Every container is entered when the walk is not tier-scoped at all.
        let unscoped = word_texts(content, None).len();
        if unscoped != 1 {
            wrong.push(format!(
                "{}: a walk with no domain excludes nothing, yet saw {unscoped} words",
                row.name
            ));
        }

        // One level down, through `BracketedItem`'s traversals, which are a
        // SECOND pair of hand-written arm sets in each module. The outer
        // annotated group is entered under every domain, so the inner verdict
        // shows through.
        let Some(Nested {
            line: nested_line,
            codes: nested_codes,
            is: is_nested,
        }) = row.nested
        else {
            continue;
        };
        let nested = tier(nested_line, nested_codes)?;
        let nested_content: &[UtteranceContent] = &nested.content.content;
        let inner_ok = match nested_content {
            [UtteranceContent::AnnotatedGroup(outer)] => match &outer.inner.content.content[..] {
                [inner] => is_nested(inner),
                _ => false,
            },
            _ => false,
        };
        if !inner_ok {
            wrong.push(format!(
                "{}: {:?} did not build an annotated group around one {} but {nested_content:?}",
                row.name, nested_line, row.name
            ));
            continue;
        }
        for (column, domain) in DOMAINS.iter().enumerate() {
            match decide(nested_content, *domain) {
                Ok(got) if got == row.expected[column] => {}
                Ok(got) => wrong.push(format!(
                    "{} / {domain:?} nested: the BracketedItem arms disagree with the \
                     UtteranceContent arms: expected {:?}, got {got:?}",
                    row.name, row.expected[column]
                )),
                Err(err) => wrong.push(format!("{} / {domain:?} nested: {err}", row.name)),
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}
