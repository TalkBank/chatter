//! The admitted model alone issues edits to its own dependent-tier bodies.
#![allow(clippy::expect_used, clippy::panic)]

use super::*;
use std::ops::Range;

pub(super) struct Control {
    source: String,
    file: ChatFile,
}

impl Control {
    pub(super) fn new(layout: Layout) -> Self {
        let source = layout.source(CONTROL);
        let parser = TreeSitterParser::new().expect("parser");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "control parse: {:?}", errors.to_vec());
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "control validity: {:?}", errors.to_vec());
        assert_eq!(file.utterances().count(), 3);
        assert!(file.utterances().all(|turn| turn.parse_health().is_clean()));
        Self { source, file }
    }

    pub(super) fn source(&self) -> &str {
        &self.source
    }
    pub(super) fn file(&self) -> &ChatFile {
        &self.file
    }
    pub(super) fn middle(&self) -> &Utterance {
        self.file.utterances().nth(1).expect("middle")
    }

    pub(super) fn edit(&self, tier: Tier) -> Edit<'_> {
        let entry = entry(self.middle(), tier).expect("control tier");
        let span = entry.span();
        let body = entry.content_span().expect("nonempty control body");
        assert!(span.start <= body.start && body.end < span.end);
        let range = body.start as usize..body.end as usize;
        assert!(self.source.get(range.clone()).is_some());
        Edit {
            control: self,
            tier,
            span,
            range,
        }
    }
}

pub(super) struct Edit<'control> {
    control: &'control Control,
    tier: Tier,
    span: Span,
    range: Range<usize>,
}

impl<'control> Edit<'control> {
    pub(super) fn replace(self, body: &str, specimen: &str) -> Candidate<'control> {
        let source = format!(
            "{}{}{}",
            &self.control.source[..self.range.start],
            body,
            &self.control.source[self.range.end..]
        );
        assert_eq!(
            source, specimen,
            "spec must equal the model-issued body edit"
        );
        let delta = i32::try_from(body.len()).expect("body length")
            - i32::try_from(self.range.len()).expect("original length");
        let span = Span::new(
            self.span.start,
            self.span.end.checked_add_signed(delta).expect("edited end"),
        );
        Candidate {
            control: self.control,
            source,
            tier: self.tier,
            span,
            range: self.range,
            delta,
        }
    }
}

pub(super) struct Candidate<'control> {
    control: &'control Control,
    source: String,
    tier: Tier,
    span: Span,
    range: Range<usize>,
    delta: i32,
}

impl Candidate<'_> {
    pub(super) fn source(&self) -> &str {
        &self.source
    }
    pub(super) fn control(&self) -> &Control {
        self.control
    }
    pub(super) fn tier(&self) -> Tier {
        self.tier
    }
    pub(super) fn span(&self) -> Span {
        self.span
    }
    pub(super) fn input(&self) -> &str {
        &self.source[self.span.start as usize..self.span.end as usize]
    }
    pub(super) fn translated(&self, mut span: Span) -> Span {
        if span.start as usize >= self.range.end {
            span.start = span
                .start
                .checked_add_signed(self.delta)
                .expect("translated start");
        }
        if span.end as usize >= self.range.end {
            span.end = span
                .end
                .checked_add_signed(self.delta)
                .expect("translated end");
        }
        span
    }
}
