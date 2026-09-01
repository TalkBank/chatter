//! Cross-utterance balance checks for scoped begin/end markers.
//!
//! This module validates:
//! - LongFeatureBegin/End markers (&{l=LABEL / &}l=LABEL)
//! - NonvocalBegin/End markers (&{n=LABEL / &}n=LABEL)
//!
//! Both types can span multiple utterances and must have matching labels.
//!
//! The third scoped family, underline (`␂␁`/`␂␂`, E356/E357), is NOT merged in
//! yet, and the honest reason is narrower than it first looks. Being
//! within-utterance rather than across-file, and carrying no label, are both
//! properties of the ACCUMULATOR, not of the traversal: one is where you reset
//! the stack, the other is `Vec<Span>` instead of `Vec<(&str, Span)>`.
//!
//! The real obstacle is that underline markers can appear INSIDE a word, and
//! [`walk_content`] emits `ContentItem::Word` without descending into
//! `word.content()`, so word-internal markers are invisible to every consumer of
//! that walker. That is a gap in the walker rather than a property of
//! underline, and closing it (a `ContentItem` variant for word-internal
//! underline markers, plus descent into a replacement's words) would let all
//! three families share this one algorithm.
//!
//! Until then `validation::utterance::underline` keeps its own ~250-line
//! descent over the same seven containers, kept in step with this one by
//! inspection alone. That is exactly the arrangement that produced the spurious
//! E359 above, so it is a to-do and not a design. Nor is that copy fully
//! exhaustive: its word-content loop ends in `_ => {}`, which no ratchet
//! currently sees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

#![deny(clippy::wildcard_enum_match_arm)]

use super::FileUtterances;
use crate::alignment::helpers::{ContentItem, walk_content};
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

/// A family of labelled scoped markers whose begins and ends balance over a file.
///
/// Long features and nonvocals differ in exactly three things: which content
/// variants carry them, which two codes they report, and one letter in their
/// messages. Everything else, the LIFO-per-label matching, the traversal, the
/// unclosed-scope sweep, is common, so it is written once and selected by this
/// enum.
///
/// It is written once BECAUSE it was written twice. Until 2026-08-08 these were
/// two copies of one loop, and both copies carried the same defect: each walked
/// only the top level of the main tier and matched with `_ => {}`, so a begin
/// marker inside a group, retrace or quotation was never recorded. That failed
/// in both directions on valid CHAT. A nested begin paired with a top-level end
/// reported a SPURIOUS E359/E368 against a correct transcript, and a nested end
/// was silently missed. One algorithm cannot drift from itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeFamily {
    /// `&{l=LABEL` ... `&}l=LABEL`, reported as E358 / E359.
    LongFeature,
    /// `&{n=LABEL` ... `&}n=LABEL`, reported as E367 / E368.
    Nonvocal,
}

impl ScopeFamily {
    /// The noun used for this family in diagnostic text.
    fn noun(self) -> &'static str {
        match self {
            Self::LongFeature => "long feature",
            Self::Nonvocal => "nonvocal",
        }
    }

    /// The letter that distinguishes the family's markers: `&{l=` versus `&{n=`.
    fn sigil(self) -> char {
        match self {
            Self::LongFeature => 'l',
            Self::Nonvocal => 'n',
        }
    }

    /// Code for a begin marker that is never closed.
    fn unmatched_begin(self) -> ErrorCode {
        match self {
            Self::LongFeature => ErrorCode::UnmatchedLongFeatureBegin,
            Self::Nonvocal => ErrorCode::UnmatchedNonvocalBegin,
        }
    }

    /// Code for an end marker with no open begin.
    fn unmatched_end(self) -> ErrorCode {
        match self {
            Self::LongFeature => ErrorCode::UnmatchedLongFeatureEnd,
            Self::Nonvocal => ErrorCode::UnmatchedNonvocalEnd,
        }
    }
}

/// Which side of a scope a marker opens or closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeSide {
    Begin,
    End,
}

/// One scoped-marker occurrence, with its family already resolved.
struct ScopeMarker<'a> {
    family: ScopeFamily,
    side: ScopeSide,
    label: &'a str,
    span: Span,
}

/// Classify a walked content item as a scoped marker, or as not one.
///
/// Exhaustive on purpose, and this module denies wildcard match arms: adding a
/// scoped-marker variant to [`ContentItem`] without classifying it here is a
/// hard error. That is what makes the family list airtight; the tests below
/// only demonstrate it.
///
/// Note where that guarantee lives. `wildcard_enum_match_arm` is a CLIPPY lint,
/// so it fires in CI's clippy pass and NOT under `cargo test`; a local green
/// test run says nothing about it. Verified by replacing the arms below with a
/// bare `_ => None` and watching clippy fail, because a guard nobody has seen
/// fail is a guard nobody knows runs.
///
/// `NonvocalSimple` (`&{n=LABEL}`) is listed with the non-markers deliberately:
/// it opens and closes in one token, so it must NOT push a scope. Treating it
/// as a begin would report every one of them as unclosed.
fn classify<'a>(item: &ContentItem<'a>) -> Option<ScopeMarker<'a>> {
    match item {
        ContentItem::LongFeatureBegin(begin) => Some(ScopeMarker {
            family: ScopeFamily::LongFeature,
            side: ScopeSide::Begin,
            label: begin.label.as_str(),
            span: begin.span,
        }),
        ContentItem::LongFeatureEnd(end) => Some(ScopeMarker {
            family: ScopeFamily::LongFeature,
            side: ScopeSide::End,
            label: end.label.as_str(),
            span: end.span,
        }),
        ContentItem::NonvocalBegin(begin) => Some(ScopeMarker {
            family: ScopeFamily::Nonvocal,
            side: ScopeSide::Begin,
            label: begin.label.as_str(),
            span: begin.span,
        }),
        ContentItem::NonvocalEnd(end) => Some(ScopeMarker {
            family: ScopeFamily::Nonvocal,
            side: ScopeSide::End,
            label: end.label.as_str(),
            span: end.span,
        }),
        ContentItem::Word(_)
        | ContentItem::ReplacedWord(_)
        | ContentItem::Separator(_)
        | ContentItem::Event(_)
        | ContentItem::Pause(_)
        | ContentItem::Action(_)
        | ContentItem::OverlapPoint(_)
        | ContentItem::OtherSpokenEvent(_)
        | ContentItem::Freecode(_)
        | ContentItem::InternalBullet(_)
        | ContentItem::UnderlineBegin(_)
        | ContentItem::UnderlineEnd(_)
        | ContentItem::NonvocalSimple(_) => None,
    }
}

/// Balance one family's begin/end markers across every utterance in the file.
///
/// Descent is [`walk_content`]'s responsibility, not this module's. That is the
/// whole point of the rewrite: there is no container list here to get wrong, so
/// a container added to the model reaches this check for free.
fn check_scope_balance<'f>(
    utterances: &FileUtterances<'f>,
    errors: &impl ErrorSink,
    family: ScopeFamily,
) {
    // Open scopes for this family, in encounter order, so the unclosed sweep
    // below reports them at their own begin markers.
    let mut open_scopes: Vec<(&'f str, Span)> = Vec::new();

    for utterance in utterances.iter() {
        walk_content(&utterance.main.content.content, None, &mut |item| {
            let Some(marker) = classify(&item).filter(|m| m.family == family) else {
                return;
            };
            match marker.side {
                ScopeSide::Begin => open_scopes.push((marker.label, marker.span)),
                ScopeSide::End => {
                    // Last-in-first-out for the same label, so nested scopes
                    // with distinct labels are independent.
                    match open_scopes.iter().rposition(|(l, _)| *l == marker.label) {
                        Some(pos) => {
                            open_scopes.remove(pos);
                        }
                        None => errors.report(
                            ParseError::at_span(
                                family.unmatched_end(),
                                Severity::Error,
                                marker.span,
                                format!(
                                    "Unmatched {} end marker for label '{}'",
                                    family.noun(),
                                    marker.label
                                ),
                            )
                            .with_suggestion(format!(
                                "Add a matching &{{{}={} marker before this &}}{}={} marker",
                                family.sigil(),
                                marker.label,
                                family.sigil(),
                                marker.label
                            )),
                        ),
                    }
                }
            }
        });
    }

    for (label, span) in open_scopes {
        errors.report(
            ParseError::at_span(
                family.unmatched_begin(),
                Severity::Error,
                span,
                format!(
                    "Unmatched {} begin marker: &{{{}={} without matching &}}{}={}",
                    family.noun(),
                    family.sigil(),
                    label,
                    family.sigil(),
                    label
                ),
            )
            .with_suggestion(format!(
                "Add a matching &}}{}={} marker",
                family.sigil(),
                label
            )),
        );
    }
}

/// Validate that long feature markers are properly matched across all utterances.
///
/// Checks:
/// - E358: Every LongFeatureBegin has a matching LongFeatureEnd
/// - E359: Every LongFeatureEnd has a matching LongFeatureBegin
///
/// Matching is label-specific and uses LIFO behavior per label, so nested scopes
/// with distinct labels are handled independently. This does NOT check that a
/// begin/end pair's labels agree beyond that label-specific matching itself
/// (there is no separate label-MISMATCH diagnostic here): the code once
/// reserved for that, E366 (`LongFeatureLabelMismatch`), was retired
/// 2026-07-31 as dead code with no emit site.
pub fn check_long_feature_balance(utterances: &FileUtterances<'_>, errors: &impl ErrorSink) {
    check_scope_balance(utterances, errors, ScopeFamily::LongFeature);
}

/// Validate that nonvocal markers are properly matched across all utterances.
///
/// Checks:
/// - E367: Every NonvocalBegin has a matching NonvocalEnd
/// - E368: Every NonvocalEnd has a matching NonvocalBegin
///
/// The algorithm mirrors long-feature balancing so both scoped-marker families
/// share consistent cross-utterance semantics and diagnostics. As with
/// `check_long_feature_balance`, there is no separate label-MISMATCH
/// diagnostic: the code once reserved for that, E369
/// (`NonvocalLabelMismatch`), was retired 2026-07-31 as dead code with no
/// emit site.
pub fn check_nonvocal_balance(utterances: &FileUtterances<'_>, errors: &impl ErrorSink) {
    check_scope_balance(utterances, errors, ScopeFamily::Nonvocal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::{
        BracketedContent, BracketedItem, ChatFile, Group, Line, LongFeatureBegin, LongFeatureEnd,
        MainTier, NonvocalBegin, NonvocalEnd, NonvocalSimple, Retrace, RetraceKind, Terminator,
        Utterance, UtteranceContent,
    };

    /// A one-file sequence built the way production builds one.
    ///
    /// There was a `#[cfg(test)]` constructor taking a slice directly; it was
    /// the only door into the invariant `FileUtterances` exists to hold, and it
    /// turned out to be unnecessary, because `Line::Utterance` and
    /// `ChatFile::new` are both public. A test that goes through the real
    /// constructor also exercises the real path.
    fn sequence(utterances: Vec<Utterance>) -> ChatFile {
        ChatFile::new(
            utterances
                .into_iter()
                .map(|utt| Line::Utterance(Box::new(utt)))
                .collect(),
        )
    }

    /// Unmatched long-feature begin markers emit `E358`.
    ///
    /// The diagnostic should include the unmatched label for easier repair.
    #[test]
    fn test_e358_unmatched_long_feature_begin() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedLongFeatureBegin);
        assert!(errors[0].message.contains("singing"));
    }

    /// Unmatched long-feature end markers emit `E359`.
    ///
    /// This catches stray closing markers with no prior opening scope.
    #[test]
    fn test_e359_unmatched_long_feature_end() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedLongFeatureEnd);
        assert!(errors[0].message.contains("singing"));
    }

    // -----------------------------------------------------------------------
    // Descent
    //
    // Every test above this line puts its markers at the top level of the main
    // tier, and every one of them passed against the flat loop these checks
    // used until 2026-08-08, which never descended into any container at all.
    // They pin the LIFO algorithm; the tests below pin the traversal, which is
    // the half that shipped a false positive to users.
    //
    // SURVIVES: behaviour. Descent itself is now structural, owned by
    // `walk_content` and closed by this module's `deny(wildcard_enum_match_arm)`
    // over `ContentItem`. What no type can state is that these checks CONSULT
    // that traversal rather than walking the top level themselves, which is
    // precisely the mistake that was made, so it is asserted here.
    // -----------------------------------------------------------------------

    /// Run one balance check over a single utterance's content.
    ///
    /// The five descent tests below differ only in their content and their
    /// assertion; this is the block they all repeated, extracted so the sixth
    /// cannot copy it slightly wrong.
    fn errors_from(
        check: fn(&FileUtterances<'_>, &ErrorCollector),
        content: Vec<UtteranceContent>,
    ) -> Vec<ParseError> {
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let errors = ErrorCollector::new();
        check(
            &FileUtterances::of(&sequence(vec![Utterance::new(main)])),
            &errors,
        );
        errors.into_vec()
    }

    /// Wrap content items in a `<...>` group, as `<the &{l=X dog>` parses to.
    fn group(items: Vec<BracketedItem>) -> UtteranceContent {
        UtteranceContent::Group(Group::new(BracketedContent::new(items)))
    }

    /// A begin marker inside a group closes a later top-level end marker.
    ///
    /// The regression this pins: `*CHI: <the &{l=soft dog> [/] the dog &}l=soft .`
    /// is valid CHAT, and chatter reported E359 "Unmatched long feature end
    /// marker for label 'soft'" against it, because the nested begin was never
    /// recorded. A spurious error on a correct transcript is worse than a
    /// missed one: it sends a researcher looking for a defect that is not there.
    #[test]
    fn a_begin_inside_a_group_closes_a_top_level_end() {
        let errors = errors_from(
            check_long_feature_balance,
            vec![
                group(vec![BracketedItem::LongFeatureBegin(
                    LongFeatureBegin::new("soft"),
                )]),
                UtteranceContent::LongFeatureEnd(LongFeatureEnd::new("soft")),
            ],
        );

        assert_eq!(errors, Vec::new());
    }

    /// A begin inside a retrace closes a top-level end, as with a plain group.
    ///
    /// A second container kind, because the defect was never about groups
    /// specifically: the old loop saw none of the seven.
    #[test]
    fn a_begin_inside_a_retrace_closes_a_top_level_end() {
        let errors = errors_from(
            check_long_feature_balance,
            vec![
                UtteranceContent::Retrace(Box::new(Retrace::new(
                    BracketedContent::new(vec![BracketedItem::LongFeatureBegin(
                        LongFeatureBegin::new("soft"),
                    )]),
                    RetraceKind::Partial,
                ))),
                UtteranceContent::LongFeatureEnd(LongFeatureEnd::new("soft")),
            ],
        );

        assert_eq!(errors, Vec::new());
    }

    /// An unmatched end marker inside a group is still reported.
    ///
    /// The other direction of the same defect. Descent must not be bought by
    /// going quiet: a genuinely stray `&}l=` nested in a container is exactly
    /// as invalid as one at the top level, and the old loop missed it.
    #[test]
    fn an_unmatched_end_inside_a_group_is_reported() {
        let errors = errors_from(
            check_long_feature_balance,
            vec![group(vec![BracketedItem::LongFeatureEnd(
                LongFeatureEnd::new("soft"),
            )])],
        );

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedLongFeatureEnd);
    }

    /// Nonvocal markers descend identically, because it is now one algorithm.
    ///
    /// Both families shared the defect because they were two copies of one
    /// loop. They now share the fix by being one function, and this asserts
    /// that the second family really does route through it.
    #[test]
    fn a_nonvocal_begin_inside_a_group_closes_a_top_level_end() {
        let errors = errors_from(
            check_nonvocal_balance,
            vec![
                group(vec![BracketedItem::NonvocalBegin(NonvocalBegin::new(
                    "THUMP",
                ))]),
                UtteranceContent::NonvocalEnd(NonvocalEnd::new("THUMP")),
            ],
        );

        assert_eq!(errors, Vec::new());
    }

    /// A self-closing `&{n=LABEL}` opens no scope, nested or not.
    ///
    /// `NonvocalSimple` sits in `classify`'s non-marker list, and the cost of
    /// getting that wrong is one spurious E367 per occurrence. Now that descent
    /// reaches inside containers, a nested one would be reported too, so the
    /// case is worth holding.
    #[test]
    fn a_self_closing_nonvocal_opens_no_scope() {
        let errors = errors_from(
            check_nonvocal_balance,
            vec![group(vec![BracketedItem::NonvocalSimple(
                NonvocalSimple::new("BANG"),
            )])],
        );

        assert_eq!(errors, Vec::new());
    }

    /// Long-feature begin and end markers with differing labels are both unmatched.
    ///
    /// SURVIVES: policy. Reporting two independent diagnostics rather than one
    /// "label mismatch" is a choice with a real alternative, and it is the
    /// choice that outlived E366: that code was retired 2026-07-31 with no emit
    /// site. Named for the behaviour rather than the dead code, because a test
    /// named after a retired number sends the next reader looking for it.
    #[test]
    fn differing_long_feature_labels_report_both_sides() {
        let main1 = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(
                "singing",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );
        let main2 = MainTier::new(
            "CHI",
            vec![UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(
                "whisper",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main1), Utterance::new(main2)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 2); // Unmatched begin (singing) and unmatched end (whisper)
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::UnmatchedLongFeatureBegin && e.message.contains("singing")
        ));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::UnmatchedLongFeatureEnd
                    && e.message.contains("whisper"))
        );
    }

    /// Balanced long-feature scopes produce no errors.
    ///
    /// This is the baseline valid path for cross-utterance long-feature tracking.
    #[test]
    fn test_balanced_long_features() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::LongFeatureBegin(LongFeatureBegin::new("singing")),
                UtteranceContent::LongFeatureEnd(LongFeatureEnd::new("singing")),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_long_feature_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 0);
    }

    /// Unmatched nonvocal begin markers emit `E367`.
    ///
    /// Label text should be preserved in the resulting error message.
    #[test]
    fn test_e367_unmatched_nonvocal_begin() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                "crying",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedNonvocalBegin);
        assert!(errors[0].message.contains("crying"));
    }

    /// Unmatched nonvocal end markers emit `E368`.
    ///
    /// This catches closing markers that do not correspond to an open scope.
    #[test]
    fn test_e368_unmatched_nonvocal_end() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalEnd(NonvocalEnd::new("crying"))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::UnmatchedNonvocalEnd);
        assert!(errors[0].message.contains("crying"));
    }

    /// Nonvocal begin and end markers with differing labels are both unmatched.
    ///
    /// SURVIVES: policy. The nonvocal half of the same choice; E369 was retired
    /// alongside E366 for the same reason.
    #[test]
    fn differing_nonvocal_labels_report_both_sides() {
        let main1 = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                "crying",
            ))],
            Terminator::Period { span: Span::DUMMY },
        );
        let main2 = MainTier::new(
            "CHI",
            vec![UtteranceContent::NonvocalEnd(NonvocalEnd::new("laughing"))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main1), Utterance::new(main2)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 2); // Unmatched begin (crying) and unmatched end (laughing)
        assert!(
            errors.iter().any(
                |e| e.code == ErrorCode::UnmatchedNonvocalBegin && e.message.contains("crying")
            )
        );
        assert!(
            errors.iter().any(
                |e| e.code == ErrorCode::UnmatchedNonvocalEnd && e.message.contains("laughing")
            )
        );
    }

    /// Balanced nonvocal scopes produce no errors.
    ///
    /// This confirms begin/end matching works for the happy path.
    #[test]
    fn test_balanced_nonvocal() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::NonvocalBegin(NonvocalBegin::new("crying")),
                UtteranceContent::NonvocalEnd(NonvocalEnd::new("crying")),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterances = vec![Utterance::new(main)];
        let errors = ErrorCollector::new();
        check_nonvocal_balance(&FileUtterances::of(&sequence(utterances)), &errors);
        let errors = errors.into_vec();

        assert_eq!(errors.len(), 0);
    }
}
