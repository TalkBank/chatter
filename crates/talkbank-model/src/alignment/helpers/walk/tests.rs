//! Unit tests for the content tree walkers.

use super::*;
use crate::Span;
use crate::annotation::AnnotatedContentAnnotations;
use crate::model::{
    Annotated, BracketedContent, BracketedItem, Event, Group, OverlapPoint, OverlapPointKind,
    Pause, PauseDuration, PhoGroup, Retrace, RetraceKind, Separator, SinGroup, UtteranceContent,
    Word,
};

fn word(text: &str) -> Word {
    Word::simple(text)
}

fn boxed_word(text: &str) -> Box<Word> {
    Box::new(word(text))
}

// ---------------------------------------------------------------------------
// walk_words tests (backward-compat with old walk_words tests)
// ---------------------------------------------------------------------------

/// Collects leaf word texts from content using the walker.
fn collect_word_texts(content: &[UtteranceContent], domain: Option<TierDomain>) -> Vec<String> {
    let mut texts = Vec::new();
    walk_words(content, domain, &mut |leaf| {
        if let WordItem::Word(w) = leaf {
            texts.push(w.cleaned_text().to_string());
        }
    });
    texts
}

#[test]
fn flat_words() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Word(boxed_word("world")),
    ];
    assert_eq!(collect_word_texts(&content, None), ["hello", "world"]);
}

#[test]
fn words_inside_group() {
    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(boxed_word("in")),
        BracketedItem::Word(boxed_word("group")),
    ]));
    let content = vec![UtteranceContent::Group(group)];
    assert_eq!(collect_word_texts(&content, None), ["in", "group"]);
}

#[test]
fn retrace_group_skipped_for_mor() {
    let bracketed = BracketedContent::new(vec![BracketedItem::Word(boxed_word("retraced"))]);
    let retrace = Retrace::new(bracketed, RetraceKind::Full).as_group();
    let content = vec![
        UtteranceContent::Retrace(Box::new(retrace)),
        UtteranceContent::Word(boxed_word("kept")),
    ];

    // Mor domain: retrace is skipped
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Mor)),
        ["kept"]
    );
    // No domain: retrace is included
    assert_eq!(collect_word_texts(&content, None), ["retraced", "kept"]);
    // Wor domain: retrace is included
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Wor)),
        ["retraced", "kept"]
    );
}

/// Every cell of the phonological/sign descent rule, because the corpus
/// cannot reach half of them and a differential over it never will.
///
/// `descent::descends_into_group` decides Pho and Sin groups together, for the
/// Pho and Sin domains together, so the rule spans two kinds times five domain
/// values. This walks all ten. It absorbed `pho_group_skipped_for_pho_domain`,
/// which asserted three of them.
///
/// **The `%sin` half is unreachable from real data.** Measured 2026-08-26 over
/// a whole ~106,000-file CHAT corpus, not a sample:
///
/// ```text
/// $ rg -l --no-messages '^%sin:' --glob '*.cha' <corpus-root> | wc -l   ->      0
/// $ rg -l --no-messages '^%pho:' --glob '*.cha' <corpus-root> | wc -l   ->   2847
/// ```
///
/// So no comparison over real corpora, however green and however large, is
/// evidence about the Sin domain. This test is the only thing that is.
///
/// # What it asserts, and what it deliberately does not
///
/// Only that no word INSIDE the group reaches the walk. It does NOT assert
/// that the group is "one unit", because `collect_word_texts` cannot see the
/// difference: a container counted as one alignable position and a container
/// excluded outright both emit zero words here. That distinction is real and
/// lives in `count.rs` (a Pho group is 1 in the Pho domain and 0 in the Sin
/// domain), so it belongs in a counting test, not this one. Saying so matters:
/// an earlier version of this docstring claimed unit-hood through a probe
/// whose output is identical under both readings.
///
/// SURVIVES a type, as POLICY: that these groups are opaque to the domains
/// that measure them is a choice with a real alternative (descending into
/// their words), and no type refuses the alternative.
#[test]
fn phonological_and_sign_groups_are_opaque_to_both_measuring_domains() {
    /// Build `<group> after` and report which words the walk emits.
    fn emitted(group: UtteranceContent, domain: Option<TierDomain>) -> Vec<String> {
        let content = vec![group, UtteranceContent::Word(boxed_word("after"))];
        collect_word_texts(&content, domain)
    }
    fn pho() -> UtteranceContent {
        UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(vec![
            BracketedItem::Word(boxed_word("inside")),
        ])))
    }
    fn sin() -> UtteranceContent {
        UtteranceContent::SinGroup(SinGroup::new(BracketedContent::new(vec![
            BracketedItem::Word(boxed_word("inside")),
        ])))
    }

    // The measuring domains are opaque, BOTH kinds under BOTH of them: the
    // rule pairs Pho with Sin on each side, so the two cross cells (a Pho
    // group under Sin, a Sin group under Pho) are governed too.
    for domain in [TierDomain::Pho, TierDomain::Sin] {
        assert_eq!(
            emitted(pho(), Some(domain)),
            ["after"],
            "pho group / {domain:?}"
        );
        assert_eq!(
            emitted(sin(), Some(domain)),
            ["after"],
            "sin group / {domain:?}"
        );
    }

    // Every other domain descends, `Wor` included. `Wor` is the cell most
    // worth pinning: it is the one where `count.rs` also descends, so a drift
    // between the walk and the count would show up here first.
    for domain in [Some(TierDomain::Mor), Some(TierDomain::Wor), None] {
        assert_eq!(
            emitted(pho(), domain),
            ["inside", "after"],
            "pho group / {domain:?}"
        );
        assert_eq!(
            emitted(sin(), domain),
            ["inside", "after"],
            "sin group / {domain:?}"
        );
    }
}

#[test]
fn separator_yielded() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
        UtteranceContent::Word(boxed_word("world")),
    ];
    let mut count = 0;
    walk_words(&content, None, &mut |leaf| {
        if let WordItem::Separator(_) = leaf {
            count += 1;
        }
    });
    assert_eq!(count, 1);
}

#[test]
fn mut_walker_modifies_words() {
    let mut content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Word(boxed_word("world")),
    ];
    walk_words_mut(&mut content, None, &mut |leaf| {
        if let WordItemMut::Word(w) = leaf {
            w.inline_bullet = Some(crate::model::Bullet::new(0, 100));
        }
    });
    // Verify modification took effect
    let mut count = 0;
    walk_words(&content, None, &mut |leaf| {
        if let WordItem::Word(w) = leaf {
            assert!(w.inline_bullet.is_some());
            count += 1;
        }
    });
    assert_eq!(count, 2);
}

#[test]
fn nested_quotation_recursion() {
    let quot = crate::model::Quotation::new(BracketedContent::new(vec![BracketedItem::Word(
        boxed_word("quoted"),
    )]));
    let content = vec![UtteranceContent::Quotation(quot)];
    assert_eq!(collect_word_texts(&content, None), ["quoted"]);
}

// ---------------------------------------------------------------------------
// walk_content tests, verify non-word items are emitted
// ---------------------------------------------------------------------------

/// Helper: count how many items of each kind walk_content emits.
#[derive(Default, Debug)]
struct ContentCounts {
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

fn count_content_items(content: &[UtteranceContent], domain: Option<TierDomain>) -> ContentCounts {
    let mut counts = ContentCounts::default();
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

#[test]
fn walk_content_emits_events_and_pauses() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Event(Event::new("laughs")),
        UtteranceContent::Pause(Pause {
            duration: PauseDuration::Short,
            span: Span::DUMMY,
        }),
        UtteranceContent::Word(boxed_word("world")),
    ];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 2);
    assert_eq!(counts.events, 1);
    assert_eq!(counts.pauses, 1);
}

#[test]
fn walk_content_emits_annotated_event_inner() {
    let event = Event::new("coughs");
    let annotated = Annotated {
        inner: event,
        scoped_annotations: AnnotatedContentAnnotations::new(vec![ContentAnnotation::Uncertain])
            .expect("one annotation is not empty"),
        span: Span::DUMMY,
    };
    let content = vec![UtteranceContent::AnnotatedEvent(annotated)];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.events, 1);
}

#[test]
fn walk_content_emits_overlap_points() {
    let op = OverlapPoint::new(OverlapPointKind::TopOverlapBegin, None);
    let content = vec![
        UtteranceContent::Word(boxed_word("hi")),
        UtteranceContent::OverlapPoint(op),
    ];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 1);
    assert_eq!(counts.overlap_points, 1);
}

#[test]
fn walk_content_recurses_into_groups() {
    let event = Event::new("claps");
    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(boxed_word("inside")),
        BracketedItem::Event(event),
    ]));
    let content = vec![UtteranceContent::Group(group)];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 1);
    assert_eq!(counts.events, 1);
}

#[test]
fn walk_content_skips_pho_group_for_pho_domain() {
    let pho = PhoGroup::new(BracketedContent::new(vec![BracketedItem::Word(
        boxed_word("phonological"),
    )]));
    let content = vec![
        UtteranceContent::PhoGroup(pho),
        UtteranceContent::Word(boxed_word("after")),
    ];

    // Pho domain: PhoGroup skipped
    let counts = count_content_items(&content, Some(TierDomain::Pho));
    assert_eq!(counts.words, 1); // only "after"

    // No domain: PhoGroup recursed
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 2); // "phonological" + "after"
}

#[test]
fn walk_content_words_match_walk_words() {
    // Verify walk_content produces the same words as walk_words for simple content.
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
        UtteranceContent::Word(boxed_word("world")),
    ];

    let mut content_words = Vec::new();
    walk_content(&content, None, &mut |item| {
        if let ContentItem::Word(w) = item {
            content_words.push(w.cleaned_text().to_string());
        }
    });

    let walk_words_result = collect_word_texts(&content, None);
    assert_eq!(content_words, walk_words_result);
}

// ---------------------------------------------------------------------------
// Deprecated alias tests, verify they still work
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The shared container rule, read through both traversal families
// ---------------------------------------------------------------------------

/// The container descent table, measured through both traversal families.
///
/// # What it guards NOW, which is narrower than when it was written
///
/// It was written one commit before the unification and its docstring said so:
/// two modules deciding the rule, `count.rs` hand-writing it thirty more
/// times, `descend` returning an `Option` that could not serve a count. All
/// three were made false by the very next commit, and it named `walk/descent.rs`,
/// a path that commit deleted. Corrected in place rather than left standing.
///
/// `helpers::descent` owns the RULE for every traversal now. What is still
/// hand-written is the ARM GROUPING: `count.rs`'s four traversals and the
/// eight walkers each list the container variants themselves before handing
/// off, so a new container variant can still land in the wrong group and
/// compile. That is what this table catches, and it is why the rows below are
/// read through both families rather than through `descend` directly.
///
/// # The probe distinguishes three states, which is the point
///
/// Each container holds exactly ONE word, so the PAIR of answers says which of
/// three things the rule decided, where a walker's boolean view cannot:
///
/// | count | words | meaning |
/// |---|---|---|
/// | 1 | 1 | `Into`: the traversal descends and finds the word |
/// | 1 | 0 | `Atomic`: ONE alignable position, contributing no words |
/// | 0 | 0 | `Excluded`: contributes nothing at all |
/// | 0 | 1 | IMPOSSIBLE: a word reached the walk that the count does not know about |
///
/// Only four of the thirty-six cells are `Atomic`, all of them a phonological
/// or sign group under a domain that measures it, and half of those are
/// unreachable from any real corpus: zero files carry a `%sin` tier, so no
/// differential will ever exercise them. Reproduce with
/// `rg -l --no-messages '^%sin:' --glob '*.cha' <corpus-root> | wc -l`.
///
/// SURVIVES a type, and says which kind: a MEASUREMENT, of two arm groupings
/// against one shared rule.
#[test]
fn container_descent_table_is_one_rule_for_both_consumers() {
    use crate::alignment::helpers::count::{collect_tier_items, count_tier_positions};
    use crate::annotation::AnnotatedContentAnnotations;
    use crate::model::{ContentAnnotation, Quotation};

    /// What the rule decided, read off the pair of consumer answers.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum Decided {
        Into,
        Atomic,
        Excluded,
    }
    use Decided::{Atomic, Excluded, Into};

    fn one_word() -> BracketedContent {
        BracketedContent::new(vec![BracketedItem::Word(boxed_word("w"))])
    }
    fn annotated<T>(inner: T, exclude: bool) -> Annotated<T> {
        Annotated {
            inner,
            // Non-empty either way: an annotated construct always carries at
            // least one annotation now, and `Uncertain` is the neutral filler
            // that no descent rule reads.
            scoped_annotations: AnnotatedContentAnnotations::new(vec![if exclude {
                ContentAnnotation::Exclude
            } else {
                ContentAnnotation::Uncertain
            }])
            .expect("one annotation is not empty"),
            span: Span::DUMMY,
        }
    }

    /// One container, built at both levels, plus the row of verdicts it must
    /// produce under `DOMAINS`.
    ///
    /// `bracketed` is `None` only for the bare `Group`, which `BracketedItem`
    /// deliberately has no spelling of.
    struct Row {
        name: &'static str,
        utterance: fn() -> UtteranceContent,
        bracketed: Option<fn() -> BracketedItem>,
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
            utterance: || UtteranceContent::Group(Group::new(one_word())),
            bracketed: None,
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedGroup{}",
            utterance: || {
                UtteranceContent::AnnotatedGroup(annotated(Group::new(one_word()), false))
            },
            bracketed: Some(|| {
                BracketedItem::AnnotatedGroup(annotated(Group::new(one_word()), false))
            }),
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedGroup[e]",
            utterance: || UtteranceContent::AnnotatedGroup(annotated(Group::new(one_word()), true)),
            bracketed: Some(|| {
                BracketedItem::AnnotatedGroup(annotated(Group::new(one_word()), true))
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "Quotation",
            utterance: || UtteranceContent::Quotation(Quotation::new(one_word())),
            bracketed: Some(|| BracketedItem::Quotation(Quotation::new(one_word()))),
            expected: [Into, Into, Into, Into],
        },
        Row {
            name: "AnnotatedQuotation[e]",
            utterance: || {
                UtteranceContent::AnnotatedQuotation(annotated(Quotation::new(one_word()), true))
            },
            bracketed: Some(|| {
                BracketedItem::AnnotatedQuotation(annotated(Quotation::new(one_word()), true))
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "PhoGroup",
            utterance: || UtteranceContent::PhoGroup(PhoGroup::new(one_word())),
            bracketed: Some(|| BracketedItem::PhoGroup(PhoGroup::new(one_word()))),
            expected: [Into, Atomic, Excluded, Into],
        },
        Row {
            name: "SinGroup",
            utterance: || UtteranceContent::SinGroup(SinGroup::new(one_word())),
            bracketed: Some(|| BracketedItem::SinGroup(SinGroup::new(one_word()))),
            expected: [Into, Excluded, Atomic, Into],
        },
        Row {
            name: "Retrace",
            utterance: || {
                UtteranceContent::Retrace(Box::new(
                    Retrace::new(one_word(), RetraceKind::Full).as_group(),
                ))
            },
            bracketed: Some(|| {
                BracketedItem::Retrace(Box::new(
                    Retrace::new(one_word(), RetraceKind::Full).as_group(),
                ))
            }),
            expected: [Excluded, Into, Into, Into],
        },
        Row {
            name: "AnnotatedRetrace",
            utterance: || {
                UtteranceContent::AnnotatedRetrace(Box::new(annotated(
                    Retrace::new(one_word(), RetraceKind::Full).as_group(),
                    false,
                )))
            },
            bracketed: Some(|| {
                BracketedItem::AnnotatedRetrace(Box::new(annotated(
                    Retrace::new(one_word(), RetraceKind::Full).as_group(),
                    false,
                )))
            }),
            expected: [Excluded, Into, Into, Into],
        },
    ];

    /// Read the decision off both consumers, asserting they agree first.
    fn decide(content: &[UtteranceContent], domain: TierDomain, at: &str) -> Decided {
        let counted = count_tier_positions(content, domain);
        let extracted = collect_tier_items(content, domain).len();
        assert_eq!(
            counted, extracted,
            "{at} / {domain:?}: count_tier_positions and collect_tier_items disagree"
        );
        let words = collect_word_texts(content, Some(domain)).len();
        match (counted, words) {
            (1, 1) => Into,
            (1, 0) => Atomic,
            (0, 0) => Excluded,
            other => panic!("{at} / {domain:?}: impossible pair {other:?}"),
        }
    }

    for row in &rows {
        for (column, domain) in DOMAINS.iter().enumerate() {
            let top = decide(&[(row.utterance)()], *domain, row.name);
            assert_eq!(top, row.expected[column], "{} / {domain:?}", row.name);

            // One level down, through `BracketedItem`'s traversals, which are a
            // SECOND pair of hand-written arm sets in each module. A bare group
            // is entered under every domain, so the inner verdict shows through.
            if let Some(build) = row.bracketed {
                let nested = [UtteranceContent::Group(Group::new(BracketedContent::new(
                    vec![build()],
                )))];
                let inner = decide(&nested, *domain, row.name);
                assert_eq!(
                    inner, row.expected[column],
                    "{} / {domain:?} nested: the BracketedItem arms disagree with \
                     the UtteranceContent arms",
                    row.name
                );
            }
        }

        // Every container is entered when the walk is not tier-scoped at all.
        assert_eq!(
            collect_word_texts(&[(row.utterance)()], None).len(),
            1,
            "{}: a walk with no domain excludes nothing",
            row.name
        );
    }
}
