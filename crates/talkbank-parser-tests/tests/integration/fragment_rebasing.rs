//! Source-bound diagnostic and model rebasing contracts shared by corpus families.

use talkbank_model::{ErrorCollector, ParseOutcome, SpanShift};

pub(super) fn assert_fragment_rebases<T: SpanShift + PartialEq + std::fmt::Debug>(
    path: &std::path::Path,
    input: &str,
    parse: impl Fn(usize, &ErrorCollector) -> ParseOutcome<T>,
) -> bool {
    let local_errors = ErrorCollector::new();
    let local = parse(0, &local_errors);
    let shifted_errors = ErrorCollector::new();
    let shifted = parse(17, &shifted_errors);
    match (local, shifted) {
        (ParseOutcome::Parsed(mut expected), ParseOutcome::Parsed(actual)) => {
            expected.shift_spans_after(0, 17);
            assert_eq!(actual, expected, "model: {} {input:?}", path.display());
        }
        (ParseOutcome::Rejected, ParseOutcome::Rejected) => {
            assert!(
                !local_errors.is_empty(),
                "rejection needs evidence: {}",
                path.display()
            );
        }
        _ => panic!("offset changed admission: {} {input:?}", path.display()),
    }
    let mut expected_errors = local_errors.to_vec();
    for error in &mut expected_errors {
        let span = error.location.span;
        assert!(
            input.get(span.start as usize..span.end as usize).is_some(),
            "diagnostic must address caller bytes: {} {input:?} {error:?}",
            path.display()
        );
        for label in &mut error.labels {
            let span = label.span;
            assert!(
                input.get(span.start as usize..span.end as usize).is_some(),
                "label must address caller bytes: {} {input:?} {label:?}",
                path.display()
            );
        }
        // Retained diagnostic context remains snippet-relative.
        if input.is_empty() {
            // The sole point of an admitted empty caller is known; do not
            // reinterpret it as legacy unlocated model metadata.
            error.location.span = talkbank_model::Span::from_usize(17, 17);
            for label in &mut error.labels {
                label.span = talkbank_model::Span::from_usize(17, 17);
            }
        } else {
            error.shift_spans_after(0, 17);
        }
    }
    assert_eq!(
        shifted_errors.to_vec(),
        expected_errors,
        "diagnostics: {} {input:?}",
        path.display()
    );
    let mut restored_errors = shifted_errors.to_vec();
    restored_errors.shift_spans_after(17, -17);
    assert_eq!(
        restored_errors,
        local_errors.to_vec(),
        "inverse diagnostic rebase: {}",
        path.display(),
    );
    !expected_errors.is_empty()
}
