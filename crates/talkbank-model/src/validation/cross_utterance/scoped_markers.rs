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
