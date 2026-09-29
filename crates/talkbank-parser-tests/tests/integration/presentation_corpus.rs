//! Presentation consumes computed spec diagnostics; it never certifies validity.
use super::*;
use talkbank_model::{ErrorCode, ErrorSink, Severity};
use talkbank_transform::{ConfigurableErrorSink, PresentationPolicy};

#[test]
fn reference_display_events_preserve_style_and_byte_mapping() {
    use talkbank_model::errors::chat_formatting::{
        ChatTextProcessor, DisplayEvent, process_for_plain_display_mapped,
    };
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut style_events = [0; 2];
    for fixture in corpus.fixtures() {
        let source = fixture.source();
        let lines = talkbank_model::errors::line_map::LineMap::new(source);
        let index = talkbank_model::errors::line_map::SourceIndex::new(source);
        assert_eq!(index.source(), source);
        let source_len = u32::try_from(source.len()).expect("reference fits source coordinates");
        let mut end = 0u32;
        for (line, text) in source.split_inclusive('\n').enumerate() {
            assert_eq!(lines.line_start(line), end);
            end += u32::try_from(text.len()).expect("reference line fits source coordinates");
            assert_eq!(lines.line_end(line, source_len), end);
        }
        assert_eq!(
            lines.line_count(),
            source.bytes().filter(|byte| *byte == b'\n').count() + 1
        );
        for beyond in [lines.line_count(), usize::MAX - 1, usize::MAX] {
            assert_eq!(
                lines.line_end(beyond, source_len),
                source_len,
                "out-of-range line requests clamp to EOF"
            );
        }
        let (mut line, mut column) = (0, 0);
        for (offset, byte) in source.bytes().enumerate() {
            let offset = u32::try_from(offset).expect("admitted reference offset");
            assert_eq!(index.line_col_of(offset), (line, column));
            if byte == b'\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        assert_eq!(index.line_col_of(source_len), (line, column));
        // Out-of-source offsets select the final line; columns remain byte
        // distances rather than silently becoming a clamped source position.
        assert_eq!(
            index.line_col_of(u32::MAX),
            (line, (u32::MAX - lines.line_start(line)) as usize)
        );
        let mapped = process_for_plain_display_mapped(source);
        let mut processor = ChatTextProcessor::new(source);
        let mut display = String::new();
        let mut underlined = false;
        assert_eq!(
            mapped.map_offset(0),
            0,
            "origin is fixed even without a stored breakpoint"
        );
        while let Some(event) = processor.next_event() {
            match event {
                DisplayEvent::Char(ch) => display.push(ch),
                DisplayEvent::TabSpaces(count) => {
                    assert_eq!(count, 8 - display.len() % 8);
                    display.extend(std::iter::repeat_n(' ', count));
                }
                DisplayEvent::Bullet => display.push('•'),
                DisplayEvent::UnderlineBegin => {
                    underlined = true;
                    style_events[0] += 1;
                    assert!(source[..processor.char_pos()].ends_with("\u{0002}\u{0001}"));
                }
                DisplayEvent::UnderlineEnd => {
                    underlined = false;
                    style_events[1] += 1;
                    assert!(source[..processor.char_pos()].ends_with("\u{0002}\u{0002}"));
                }
            }
            assert_eq!(processor.is_underlined(), underlined);
            assert!(source.is_char_boundary(processor.char_pos()));
            assert_eq!(processor.display_pos(), display.len());
            assert_eq!(mapped.map_offset(processor.char_pos()), display.len());
        }
        assert_eq!(processor.char_pos(), source.len());
        assert_eq!(mapped.text(), display);
        // Presentation accepts byte spans from external clients. Interior
        // UTF-8 bytes and collapsed/reversed selections still need a usable
        // display range, not a slicing panic or partial rendered character.
        for offset in 0..=source.len() {
            for end in [
                offset,
                offset.saturating_sub(1),
                (offset + 1).min(source.len()),
            ] {
                let (start, end) = mapped.map_span(offset, end);
                assert!(start <= end && end <= display.len());
                let selected = display
                    .get(start..end)
                    .expect("whole UTF-8 display characters");
                assert_eq!(selected.is_empty(), start == display.len());
            }
        }
        assert_eq!(
            mapped.map_span(source.len(), source.len()),
            (display.len(), display.len())
        );
        assert_eq!(
            mapped.map_span(usize::MAX, usize::MAX),
            (display.len(), display.len()),
            "out-of-source spans clamp to the owned display's EOF"
        );
        assert_eq!(mapped.into_text(), display);
    }
    assert!(
        style_events.iter().all(|count| *count > 0),
        "both style transitions need reference witnesses"
    );
}

#[test]
fn spec_findings_preserve_evidence_through_presentation_policies() {
    #[derive(Clone, Copy)]
    enum View {
        Original,
        Strict,
        Lenient,
        Target(Option<Severity>),
        StrictTarget(Severity),
    }
    #[derive(Debug)]
    enum Delivery {
        Individual,
        Batch,
        SmallBatch,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec corpus");
    let mut observed = [0; 2];
    let mut interior_span_witnesses = 0;
    let mut cached_source_witnesses = 0;
    for fixture in corpus.fixtures() {
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(fixture.source(), &errors);
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let computed = errors.into_vec();
        let Some(first) = computed.first() else {
            continue;
        };
        let target = first.code;
        for finding in &computed {
            // Rendering can initialize a private source cache. That transition
            // must not alter diagnostic evidence, debug logs or the wire value.
            let before = finding.clone();
            let debug = format!("{finding:?}");
            let wire = serde_json::to_value(finding).expect("diagnostic wire before rendering");
            cached_source_witnesses +=
                usize::from(miette::Diagnostic::source_code(finding).is_some());
            assert_eq!(finding, &before);
            assert_eq!(format!("{finding:?}"), debug);
            assert_eq!(format!("{:?}", finding.clone()), debug);
            assert_eq!(
                serde_json::to_value(finding).expect("diagnostic wire after rendering"),
                wire
            );
            use talkbank_model::errors::codes::CheckStatus;
            use talkbank_model::errors::diagnostic_kind::{
                DiagnosticKind, ValidationProfile, kind_of, severity,
            };
            assert_ne!(
                finding.code.check_status(),
                CheckStatus::Planned,
                "a producer cannot emit a rule advertised as unimplemented: {}",
                finding.code
            );
            let kind = kind_of(finding.code);
            assert_ne!(
                kind,
                DiagnosticKind::InternalFailure,
                "canonical CHAT must not stand in for producer-failure injection"
            );
            // Golden wire names and policy rows are external contracts, not
            // another implementation of the production pairwise dispatch.
            let error = Some(Severity::Error);
            let warning = Some(Severity::Warning);
            let (kind_wire, expected) = match kind {
                DiagnosticKind::InternalFailure => ("internal_failure", [error; 4]),
                DiagnosticKind::Invalidity => ("invalidity", [error, warning, error, None]),
                DiagnosticKind::Deprecation => ("deprecation", [warning, warning, warning, None]),
                DiagnosticKind::Style => ("style", [None, None, None, warning]),
                DiagnosticKind::Unmodeled => ("unmodeled", [None; 4]),
            };
            for ((profile, profile_wire), expected) in [
                (ValidationProfile::Strict, "strict"),
                (ValidationProfile::Editor, "editor"),
                (ValidationProfile::Pipeline, "pipeline"),
                (ValidationProfile::Lint, "lint"),
            ]
            .into_iter()
            .zip(expected)
            {
                assert_eq!(severity(kind, profile), expected);
                let wire = serde_json::to_value((kind, profile)).expect("public profile wire");
                assert_eq!(wire, serde_json::json!([kind_wire, profile_wire]));
                let restored: (DiagnosticKind, ValidationProfile) =
                    serde_json::from_value(wire).expect("public profile reconstruction");
                assert_eq!(restored, (kind, profile));
            }
            let span = finding.location.span;
            let byte_range: std::ops::Range<usize> = span.into();
            assert_eq!(byte_range, span.to_range());
            assert_eq!(talkbank_model::Span::from(byte_range), span);
            for range in [span.to_text_range(), span.into()] {
                assert_eq!(talkbank_model::Span::from(range), span);
            }
            assert!(span.contains_span(span));
            assert_eq!(span.contains_offset(span.start), !span.is_empty());
            assert!(!span.contains_offset(span.end));
            if span.len() > 1 {
                let point = talkbank_model::Span::new(span.start + 1, span.start + 1);
                assert!(span.contains_span(point));
                assert!(point.is_empty());
                assert!(
                    !span.overlaps(point),
                    "empty insertion span covers no bytes"
                );
                assert!(!point.overlaps(span), "overlap is symmetric");
                let byte = talkbank_model::Span::new(point.start, point.end + 1);
                assert!(
                    span.overlaps(byte) && byte.overlaps(span),
                    "a widened point covers a byte"
                );
                interior_span_witnesses += 1;
            }
            observed[usize::from(finding.severity == Severity::Warning)] += 1;
            // Offset/span entry points cannot invent line/column metadata.
            for located in [
                talkbank_model::ParseError::build(finding.code)
                    .message(finding.message.clone())
                    .at(
                        finding.location.span.start as usize,
                        finding.location.span.end as usize,
                    ),
                talkbank_model::ParseError::build(finding.code)
                    .at_span(finding.location.span)
                    .message(finding.message.clone()),
            ] {
                let located = located.finish();
                assert_eq!(located.location.span, finding.location.span);
                assert_eq!(located.location.line, None);
                assert_eq!(located.location.column, None);
            }
            if let Some(context) = &finding.context {
                let contextual = talkbank_model::ParseError::build(finding.code)
                    .message(finding.message.clone())
                    .location(finding.location)
                    .context_from_source(
                        context.source_text.clone(),
                        context.span.start as usize..context.span.end as usize,
                        context.found.clone(),
                    )
                    .finish();
                let actual = contextual.context.expect("requested source context");
                assert_eq!(actual.source_text, context.source_text);
                assert_eq!(actual.span, context.span);
                assert_eq!(actual.found, context.found);
                assert!(actual.expected.is_empty(), "no expected token was supplied");
                assert_eq!(
                    actual.line_offset, None,
                    "no source line offset was supplied"
                );
                let from_span = talkbank_model::ParseError::from_source_span(
                    finding.code,
                    finding.severity,
                    context.span,
                    context.source_text.clone(),
                    context.found.clone(),
                    finding.message.clone(),
                );
                assert_eq!(from_span.context, Some(actual));
                assert_eq!(from_span.location.span, context.span);
                assert_eq!(from_span.severity, finding.severity);
                assert_eq!(from_span.message, finding.message);
            }
            // Reconstruct actual producer findings through the public builder.
            // Only the complete message/location state permits `finish`.
            let mut builder = talkbank_model::ParseError::build(finding.code)
                .severity(finding.severity)
                .location(finding.location)
                .message(finding.message.clone());
            if let Some(context) = &finding.context {
                builder = builder.context(context.clone());
            }
            if let Some(suggestion) = &finding.suggestion {
                builder = builder.suggestion(suggestion.clone());
            }
            for label in &finding.labels {
                builder = builder.label(label.clone());
            }
            let mut rebuilt = builder.finish();
            assert_eq!(
                rebuilt.help_url.as_deref(),
                Some(finding.code.documentation_url().as_str())
            );
            // The builder supplies the canonical URL; an explicitly overridden
            // producer URL is a separate public diagnostic field.
            rebuilt.help_url = finding.help_url.clone();
            assert_eq!(
                serde_json::to_value(&rebuilt).expect("rebuilt diagnostic wire"),
                serde_json::to_value(finding).expect("producer diagnostic wire"),
                "{}: builder preserves every wire field",
                fixture.path().display()
            );
        }
        let policies = [
            (PresentationPolicy::new(), View::Original),
            (PresentationPolicy::strict(), View::Strict),
            (PresentationPolicy::lenient(), View::Lenient),
            (
                PresentationPolicy::new().downgrade(target, Severity::Warning),
                View::Target(Some(Severity::Warning)),
            ),
            (
                PresentationPolicy::new().upgrade(target, Severity::Error),
                View::Target(Some(Severity::Error)),
            ),
            (
                PresentationPolicy::new().disable(target),
                View::Target(None),
            ),
            (
                PresentationPolicy::strict().set_severity(target, Some(Severity::Warning)),
                View::StrictTarget(Severity::Warning),
            ),
            (
                PresentationPolicy::new()
                    .disable(target)
                    .set_severity(target, Some(Severity::Error)),
                View::Target(Some(Severity::Error)),
            ),
        ];
        for (policy, view) in policies {
            let expected: Vec<_> = computed
                .iter()
                .filter_map(|finding| {
                    let severity = match view {
                        View::Original => Some(finding.severity),
                        View::Strict => Some(if finding.severity == Severity::Warning {
                            Severity::Error
                        } else {
                            finding.severity
                        }),
                        View::Lenient => Some(
                            if matches!(
                                finding.code,
                                ErrorCode::IllegalUntranscribed | ErrorCode::InvalidOverlapIndex
                            ) {
                                Severity::Warning
                            } else {
                                finding.severity
                            },
                        ),
                        View::Target(severity) if finding.code == target => severity,
                        View::Target(_) => Some(finding.severity),
                        View::StrictTarget(severity) if finding.code == target => Some(severity),
                        View::StrictTarget(_) => Some(if finding.severity == Severity::Warning {
                            Severity::Error
                        } else {
                            finding.severity
                        }),
                    }?;
                    let mut displayed = finding.clone();
                    displayed.severity = severity;
                    Some(displayed)
                })
                .collect();
            assert_eq!(
                policy.apply_all(computed.clone()),
                expected,
                "{}",
                fixture.path().display()
            );
            for mode in [Delivery::Individual, Delivery::Batch, Delivery::SmallBatch] {
                let output = ErrorCollector::with_capacity(expected.len());
                let tracker = talkbank_model::ParseTracker::default();
                let delivered = talkbank_model::TeeErrorSink::new(&output, &tracker);
                let sink = ConfigurableErrorSink::new(&delivered, policy.clone());
                assert!(std::ptr::eq(sink.inner(), &delivered));
                assert_eq!(
                    sink.policy().is_disabled(target),
                    matches!(view, View::Target(None))
                );
                match mode {
                    Delivery::Individual => {
                        for finding in &computed {
                            sink.report(finding.clone());
                        }
                    }
                    Delivery::Batch => sink.report_all(computed.clone()),
                    Delivery::SmallBatch => sink.report_vec(computed.iter().cloned().collect()),
                }
                let errors = expected
                    .iter()
                    .filter(|finding| finding.severity == Severity::Error)
                    .count();
                let warnings = expected
                    .iter()
                    .filter(|finding| finding.severity == Severity::Warning)
                    .count();
                assert_eq!(tracker.error_count(), errors);
                assert_eq!(tracker.warning_count(), warnings);
                assert_eq!(tracker.has_error(), errors > 0);
                assert_eq!(tracker.has_warning(), warnings > 0);
                assert_eq!(
                    output.to_error_vec().as_slice(),
                    expected.as_slice(),
                    "snapshot preserves severity, order and multiplicity without draining"
                );
                assert_eq!(
                    output.into_vec(),
                    expected,
                    "{}: sink {mode:?}",
                    fixture.path().display()
                );
            }
        }
    }
    assert!(
        observed.iter().all(|count| *count > 0),
        "need computed errors and warnings: {observed:?}"
    );
    assert!(
        interior_span_witnesses > 0,
        "spec diagnostics must witness an interior insertion point"
    );
    assert!(
        cached_source_witnesses > 0,
        "spec diagnostics must exercise source rendering"
    );
}

#[test]
fn missing_end_spec_keeps_its_legitimate_eof_location_when_rendered() {
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCollector, SourceIndex, enhance_errors_with_index};
    use talkbank_parser::TreeSitterParser;
    let fixture = include_str!("../error_corpus/validation_errors/E502_1.cha");
    let parser = TreeSitterParser::new().expect("parser");
    for source in [fixture, fixture.strip_suffix('\n').expect("final newline")] {
        let sink = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(source, &sink);
        file.validate(&sink, TranscriptName::Anonymous);
        let mut diagnostics = sink.into_vec();
        let [error] = diagnostics.as_slice() else {
            panic!("one missing-end diagnosis")
        };
        assert_eq!(error.code.as_str(), "E502");
        let span = error.location.span;
        assert_eq!(span.start as usize, source.len());
        assert_eq!(span.start, span.end);
        let index = SourceIndex::new(source);
        let (line, column) = index.line_col_of(span.start);
        enhance_errors_with_index(&mut diagnostics, &index);
        let rendered = &diagnostics[0];
        assert_eq!(
            rendered.location.span, span,
            "EOF is not an out-of-bounds offset"
        );
        assert_eq!(rendered.location.line, Some(line + 1));
        assert_eq!(rendered.location.column, Some(column + 1));
        let context = rendered.context.as_ref().expect("display context");
        assert!(
            context
                .source_text
                .get(context.span.start as usize..context.span.end as usize)
                .is_some()
        );
    }
}

#[test]
fn deleting_spec_buffer_preserves_empty_source_diagnostics_when_rendered() {
    use std::sync::Arc;
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCollector, SourceIndex, enhance_errors_with_index};
    use talkbank_parser::TreeSitterParser;

    let fixture = include_str!("../error_corpus/validation_errors/E502_1.cha");
    let parser = TreeSitterParser::new().expect("parser");
    let (_, previous) =
        parser.parse_chat_file_revision(Arc::from(fixture), None, &ErrorCollector::new());
    let sink = ErrorCollector::new();
    let (file, _) = parser.parse_chat_file_revision(Arc::from(""), Some(&previous), &sink);
    file.validate(&sink, TranscriptName::Anonymous);
    let mut diagnostics = sink.into_vec();
    assert!(
        !diagnostics.is_empty(),
        "deleting all text must not certify valid CHAT"
    );
    let original = serde_json::to_value(&diagnostics).expect("diagnostic JSON");
    enhance_errors_with_index(&mut diagnostics, &SourceIndex::new(""));
    assert_eq!(
        serde_json::to_value(&diagnostics).expect("diagnostic JSON"),
        original,
        "without source bytes, display enrichment must not invent coordinates or context"
    );
}
