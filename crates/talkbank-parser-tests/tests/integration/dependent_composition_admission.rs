//! Private source-owned edit issuer for the finite dependent-tier deck.
#![allow(clippy::expect_used, clippy::panic)]

use super::*;
use std::ops::Range;

pub(super) struct Control {
    file: ChatFile,
}

impl Control {
    pub(super) fn new() -> Self {
        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(CONTROL, &errors);
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        assert_eq!(file.utterances().count(), TURN_COUNT);
        for utterance in file.utterances() {
            assert_eq!(utterance.dependent_tiers.len(), TIERS_PER_TURN);
            assert!(utterance.parse_health().is_clean());
        }
        Self { file }
    }

    pub(super) fn file(&self) -> &ChatFile {
        &self.file
    }

    pub(super) fn edit(&self, tier: Tier) -> Edit<'_> {
        let entry = self
            .file
            .utterances()
            .nth(MIDDLE)
            .expect("middle utterance")
            .dependent_tiers
            .iter()
            .find(|entry| tier.matches(entry))
            .expect("selected tier");
        let span = entry.location();
        let body = entry.content_span().expect("admitted nonempty body");
        assert!(
            span.span().expect("located").start() <= body.start()
                && body.end() < span.span().expect("located").end()
        );
        let range = body.start()..body.end();
        assert!(CONTROL.get(range.clone()).is_some());
        Edit {
            control: self,
            tier,
            range,
            span: span.span().expect("located"),
        }
    }
}

pub(super) struct Edit<'control> {
    control: &'control Control,
    tier: Tier,
    range: Range<usize>,
    span: Span,
}

impl<'control> Edit<'control> {
    pub(super) fn replace(self, replacement: &str, fixture: &'static str) -> Candidate<'control> {
        let source = format!(
            "{}{}{}",
            &CONTROL[..self.range.start],
            replacement,
            &CONTROL[self.range.end..]
        );
        assert_eq!(
            source, fixture,
            "spec must be exactly the owner-issued edit"
        );
        let replacement_end = self.range.start + replacement.len();
        assert_eq!(&source[..self.range.start], &CONTROL[..self.range.start]);
        assert_eq!(&source[replacement_end..], &CONTROL[self.range.end..]);
        let delta = isize::try_from(replacement.len()).expect("small replacement")
            - isize::try_from(self.range.len()).expect("small original body");
        let span = Span::new(
            self.span.start(),
            self.span
                .end()
                .checked_add_signed(delta)
                .expect("edited tier end"),
        );
        Candidate {
            control: self.control,
            tier: self.tier,
            source,
            range: self.range,
            delta,
            span,
        }
    }
}

pub(super) struct Candidate<'control> {
    control: &'control Control,
    tier: Tier,
    source: String,
    range: Range<usize>,
    delta: isize,
    span: Span,
}

impl Candidate<'_> {
    pub(super) fn control(&self) -> &Control {
        self.control
    }
    pub(super) fn source(&self) -> &str {
        &self.source
    }
    pub(super) fn tier(&self) -> Tier {
        self.tier
    }
    pub(super) fn span(&self) -> Span {
        self.span
    }
    pub(super) fn translated(&self, location: talkbank_model::Location) -> Span {
        let shift = |offset: usize, what: &str| {
            if offset >= self.range.end {
                offset.checked_add_signed(self.delta).expect(what)
            } else {
                offset
            }
        };
        Span::new(
            shift(
                location.span().expect("located").start(),
                "translated start",
            ),
            shift(location.span().expect("located").end(), "translated end"),
        )
    }
    pub(super) fn tier_input(&self) -> &str {
        self.source
            .get(self.span.to_range())
            .expect("edited tier range")
    }
}
