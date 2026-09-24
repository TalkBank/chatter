//! A leading timing bullet has no preceding utterance material to scope.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{Bullet, MainTier};
use crate::validation::ValidationContext;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Whether traversal has established any material in this utterance.
/// Later bullets never erase that knowledge: inter-bullet policy is separate.
#[derive(Clone, Copy)]
enum ScopeState {
    RecoveryUncertain,
    AwaitingMaterial,
    Established,
}

impl ScopeState {
    fn check_bullet(self, bullet: &Bullet, errors: &impl ErrorSink) {
        match self {
            Self::AwaitingMaterial => errors.report(
                ParseError::new(
                    ErrorCode::TimingBulletBeforeContent,
                    Severity::Error,
                    SourceLocation::new(bullet.span),
                    ErrorContext::new("", bullet.span, "timing bullet"),
                    "Timing bullet has no preceding utterance material",
                )
                .with_suggestion("Place the bullet after the material it times; use explicit 0 for a nonverbal utterance"),
            ),
            Self::RecoveryUncertain | Self::Established => {}
        }
    }
}

/// Keep uncertain recovery distinct from proven absence of preceding material.
pub(crate) fn check_leading_bullets(
    main: &MainTier,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    let mut state = if context.main_parse_tainted {
        ScopeState::RecoveryUncertain
    } else {
        ScopeState::AwaitingMaterial
    };
    walk_content(&main.content.content, None, &mut |item| match item {
        ContentItem::Word(_)
        | ContentItem::ReplacedWord(_)
        | ContentItem::Event(_)
        | ContentItem::Action(_)
        | ContentItem::Pause(_)
        | ContentItem::OtherSpokenEvent(_)
        | ContentItem::NonvocalSimple(_) => state = ScopeState::Established,
        ContentItem::InternalBullet(bullet) => state.check_bullet(bullet, errors),
        ContentItem::Freecode(_)
        | ContentItem::Separator(_)
        | ContentItem::OverlapPoint(_)
        | ContentItem::LongFeatureBegin(_)
        | ContentItem::LongFeatureEnd(_)
        | ContentItem::UnderlineBegin(_)
        | ContentItem::UnderlineEnd(_)
        | ContentItem::NonvocalBegin(_)
        | ContentItem::NonvocalEnd(_) => {}
    });
    if let Some(bullet) = &main.content.bullet {
        state.check_bullet(bullet, errors);
    }
}
