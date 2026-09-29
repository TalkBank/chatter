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

//! The OUTSIDE view of the model's closed collection newtypes.
//!
//! # Why this file exists, and why it cannot live in `talkbank-model`
//!
//! Closing a newtype's inner field is only half an API. The other half is the
//! set of operations a consumer needs once the field is gone: read the items,
//! MOVE them out, and rebuild. Leave any of those out and the type becomes
//! unusable from outside without a full clone, or unusable entirely.
//!
//! Nothing inside `talkbank-model` can detect that, and this is the important
//! part: a unit test in the defining crate CAN STILL SEE THE PRIVATE FIELD, so
//! it compiles whether or not the accessors exist, and passes while the public
//! API is unusable. Only a different crate exercises the consumer's view. This
//! crate is one.
//!
//! It was written after v0.9.0 shipped the field closure without the
//! consuming half. Compiling the downstream ML pipeline against it produced
//! 120 errors across two crates, of which the ones that could NOT be mended
//! downstream were: `BracketedItems` and `ChatFileLines` had no `into_vec`, so
//! rebuilding a content list or resegmenting a file could only be done by
//! cloning; and `TierContentItems` and `BracketedItems` were not re-exported
//! from `model`, so a consumer could not even NAME the type to reconstruct
//! one. Each is one line of API. None was visible from inside the crate.
//!
//! Adding a collection newtype without `into_vec` now fails HERE rather than
//! in a downstream repo, which is the whole point: the compiler enforces it
//! instead of a reviewer remembering.

use talkbank_parser_tests::test_error::TestError;

/// Collection newtypes that cannot use the general mutable collection API,
/// either because of their invariants, backing store or visibility.
///
/// Each exception names its reason. Visibility gaps require a deliberate API
/// decision; invariant-bearing types must not gain operations that erase their
/// guarantees. The census below requires new exceptions to be explicit.
const NOT_ON_THE_MACRO: &[(&str, &str)] = &[
    (
        "ReplacementWords",
        "nonempty admission forbids take and retain",
    ),
    // Admission evidence must not permit mutation that changes its verdict.
    // Consuming access discards that evidence; rebuilding requires admission.
    (
        "CompletedDiagnostics",
        "immutable completion evidence; checked admission",
    ),
    (
        "InternalFailure",
        "immutable failure evidence; checked admission",
    ),
    // NON-EMPTY by construction, which is incompatible with the macro rather
    // than merely unimplemented on it. `take` leaves a collection empty and
    // `retain` can shrink one to nothing, and both would reopen the exact
    // state this type was closed to forbid on 2026-08-26. It offers `as_slice`
    // for reading and nothing that can change its length; a consumer that
    // needs different annotations builds a new one through
    // `AnnotatedContentAnnotations::new`, which refuses the empty list.
    (
        "AnnotatedContentAnnotations",
        "non-empty invariant: length-shrinking ops would break it",
    ),
    // Publicly reachable, but SmallVec-backed, so the Vec-typed macro does not
    // fit. Worth revisiting if the macro gains a backing-store parameter.
    ("WordContents", "SmallVec-backed, not Vec"),
    // Crate-internal: never re-exported, so a consumer cannot hold one and
    // `pub` accessors on them would be dead code (rustc says so).
    ("MorItems", "crate-internal, never exported"),
    ("GraRelations", "crate-internal, never exported"),
    // On the macro, so they HAVE the accessors, but re-exported from nowhere,
    // so no consumer can name them to call one. That is a real gap in the
    // public surface rather than a property of these types; widening it is a
    // deliberate decision, not a cleanup, so it is recorded here instead.
    ("BulletContentSegments", "on the macro but not re-exported"),
    ("PhoGroupWords", "on the macro but not re-exported"),
    ("PhoItems", "on the macro but not re-exported"),
    ("SinItems", "on the macro but not re-exported"),
];

/// The round trip every closed collection newtype owes a consumer, asserted so
/// that `Deref` CANNOT satisfy it.
///
/// This is the subtle part. Every one of these types also implements
/// `Deref<Target = Vec<T>>`, so a naive `x.as_slice()` in a test resolves to
/// `Vec::as_slice` and passes even on a type with no consumer-facing accessor
/// at all, which is precisely the failure this file exists to catch. The
/// assertions therefore bind each method as a FUNCTION ITEM of the concrete
/// type (`$ty::as_slice`), which only resolves to an inherent method: an
/// inherited `Deref` method cannot satisfy it.
macro_rules! assert_consumer_api {
    ($ty:ty, $item:ty) => {{
        // Inherent, not deref-inherited. If the type stops carrying any of
        // these, this fails to COMPILE.
        let _as_slice: fn(&$ty) -> &[$item] = <$ty>::as_slice;
        let _as_mut_slice: fn(&mut $ty) -> &mut [$item] = <$ty>::as_mut_slice;
        let _into_vec: fn($ty) -> Vec<$item> = <$ty>::into_vec;
        let _take: fn(&mut $ty) -> Vec<$item> = <$ty>::take;
    }};
}

/// EVERY `Vec`-backed collection newtype in the model, with the round trip a
/// consumer needs.
///
/// The list is exhaustive as of this commit and is checked against the model by
/// `every_collection_newtype_is_listed_here` below, so it cannot silently fall
/// behind the way a hand-maintained list normally does.
#[test]
fn every_closed_collection_newtype_offers_the_consumer_api() {
    use talkbank_model::model::*;
    use talkbank_model::model::{annotation, content};

    // Reachable from the model root.
    assert_consumer_api!(ChatFileLines, Line);
    assert_consumer_api!(TierContentItems, UtteranceContent);
    assert_consumer_api!(BracketedItems, BracketedItem);
    assert_consumer_api!(ParticipantEntries, ParticipantEntry);
    assert_consumer_api!(LanguageCodes, LanguageCode);
    assert_consumer_api!(ChatOptionFlags, ChatOptionFlag);
    assert_consumer_api!(SinGroupGestures, SinToken);

    // Reachable only through their defining module. That they are NOT in the
    // model root is a smaller instance of the same defect this file guards:
    // a consumer holding one of these has to know where it lives. Left as-is
    // rather than widening the public surface in a cleanup pass, but named
    // here so the asymmetry is visible.
    assert_consumer_api!(content::TierLinkers, Linker);
    assert_consumer_api!(content::TierPostcodes, Postcode);
    let _read: fn(&annotation::ReplacementWords) -> &[Word] =
        annotation::ReplacementWords::as_slice;
    let _edit: fn(&mut annotation::ReplacementWords) -> &mut [Word] =
        annotation::ReplacementWords::as_mut_slice;
    let _consume: fn(annotation::ReplacementWords) -> Vec<Word> =
        annotation::ReplacementWords::into_vec;
    assert_consumer_api!(annotation::ReplacedWordAnnotations, ContentAnnotation);
    assert_consumer_api!(WordLanguageInfos, WordLanguageInfo);
}

/// Public boundary: consuming evidence retains findings, and re-admission
/// cannot turn an internal failure into a completed validation attempt.
#[test]
fn diagnostic_evidence_consumer_roundtrip_preserves_admission() {
    use std::io::Write;
    use talkbank_model::{
        CompletedDiagnostics, DiagnosticKind, ErrorCode, ParseError, Severity, Span,
        ValidationProfile,
    };

    let completed = CompletedDiagnostics::admit(Vec::new()).unwrap();
    assert!(completed.diagnostics().is_empty());
    assert!(CompletedDiagnostics::admit(completed.into_diagnostics()).is_ok());

    let failure = CompletedDiagnostics::admit(vec![ParseError::internal(
        "producer failure",
        Span::new(0, 1),
    )])
    .unwrap_err();
    assert_eq!(failure.diagnostics().len(), 1);
    let read: fn(&talkbank_model::InternalFailure) -> &[ParseError] =
        talkbank_model::InternalFailure::diagnostics;
    let consume: fn(talkbank_model::InternalFailure) -> Vec<ParseError> =
        talkbank_model::InternalFailure::into_diagnostics;
    assert_eq!(read(&failure).len(), 1);
    let failure = CompletedDiagnostics::admit(consume(failure))
        .expect_err("consuming evidence cannot erase the failed verdict");
    // This is injected API-boundary evidence, not invalid CHAT. The public
    // pipeline must retain that distinction in its type and user-facing text.
    let pipeline = talkbank_transform::PipelineError::InternalFailure(failure);
    assert_eq!(
        pipeline.to_string(),
        "internal tool failure; CHAT validity was not determined\n  E001 producer failure"
    );
    let talkbank_transform::PipelineError::InternalFailure(failure) = pipeline else {
        panic!("pipeline relabeled a tool failure as CHAT invalidity");
    };
    assert_eq!(read(&failure)[0].message, "producer failure");
    assert_eq!(read(&failure)[0].location.span, Span::new(0, 1));
    assert!(CompletedDiagnostics::admit(consume(failure)).is_err());

    // Completion is not validity: ordinary input errors remain admissible.
    let invalid = ParseError::at_span(
        ErrorCode::SyntaxError,
        Severity::Error,
        Span::new(2, 3),
        "input finding",
    );
    let completed = CompletedDiagnostics::admit(vec![invalid.clone()]).unwrap();
    assert_eq!(completed.diagnostics(), std::slice::from_ref(&invalid));
    assert_eq!(completed.into_diagnostics(), vec![invalid.clone()]);
    for severity in [Severity::Error, Severity::Warning] {
        let mut internal = ParseError::internal("producer failure", Span::new(4, 5));
        internal.severity = severity;
        for findings in [
            vec![invalid.clone(), internal.clone()],
            vec![internal.clone(), invalid.clone()],
        ] {
            let failure = CompletedDiagnostics::admit(findings.clone())
                .expect_err("neither ordering nor downgraded severity can conceal a tool failure");
            assert_eq!(failure.diagnostics(), findings.as_slice());
            let output = failure.to_string();
            assert!(
                output.starts_with("internal tool failure; CHAT validity was not determined\n")
            );
            // Standard bounded byte sinks exercise every refusal point without
            // manufacturing a second formatter or treating the failure as CHAT.
            for capacity in 0..output.len() {
                let mut bytes = vec![0; capacity];
                let error = bytes
                    .as_mut_slice()
                    .write_fmt(format_args!("{failure}"))
                    .expect_err("a truncated failure report must remain a failed write");
                assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
                assert_eq!(bytes, output.as_bytes()[..capacity]);
            }
            let mut bytes = vec![0; output.len()];
            bytes
                .as_mut_slice()
                .write_fmt(format_args!("{failure}"))
                .unwrap();
            assert_eq!(bytes, output.as_bytes());
            assert_eq!(failure.into_diagnostics(), findings);
        }
    }
    for profile in [
        ValidationProfile::Strict,
        ValidationProfile::Editor,
        ValidationProfile::Pipeline,
        ValidationProfile::Lint,
    ] {
        assert_eq!(
            talkbank_model::severity(DiagnosticKind::InternalFailure, profile),
            Some(Severity::Error)
        );
    }
}

/// The list above must cover every collection newtype the model defines.
///
/// Without this, the list is exactly the hand-maintained register that the
/// previous version of this file claimed to have replaced, and it was already
/// wrong on the commit that introduced it: it named 6 of 17 types and omitted
/// `TierLinkers`, the one type that was actually missing `into_vec`.
///
/// Reading the source is crude, but it is the only way a test can enumerate
/// types the language will not reflect over, and a crude check that fires beats
/// an elegant one that cannot.
#[test]
fn every_collection_newtype_is_listed_here() -> Result<(), TestError> {
    let model_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../talkbank-model/src");
    let mut defined: Vec<String> = Vec::new();
    let mut stack = vec![model_src.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| TestError::Failure(format!("read {}: {e}", dir.display())))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| TestError::Failure(format!("read {}: {e}", path.display())))?;
                // Match the DECLARATION, tolerating field visibility, a
                // SmallVec backing store, and a line break after the paren.
                // The previous pattern required `pub struct X(Vec<` on one
                // line and so missed `MorItems`, `GraRelations` (both
                // `pub(crate) Vec<`) and `WordContents` (multi-line SmallVec),
                // while the docstring claimed exhaustiveness.
                let squashed = text.replace('\n', " ");
                for chunk in squashed.split("pub struct ").skip(1) {
                    let Some((name, tail)) = chunk.split_once('(') else {
                        continue;
                    };
                    if name.trim().is_empty() || name.contains(' ') {
                        continue;
                    }
                    let tail = tail
                        .trim_start()
                        .trim_start_matches("pub(crate)")
                        .trim_start();
                    let tail = tail
                        .trim_start_matches("#[schemars(with = \"Vec<WordContent>\")]")
                        .trim_start();
                    if tail.starts_with("Vec<") || tail.starts_with("SmallVec<") {
                        defined.push(name.trim().to_string());
                    }
                }
            }
        }
    }
    defined.sort();
    defined.dedup();

    let listed = include_str!("closed_newtype_consumer_view.rs");
    let missing: Vec<&String> = defined
        .iter()
        .filter(|ty| {
            !listed.contains(&format!("assert_consumer_api!({ty},"))
                && !listed.contains(&format!("::{ty},"))
                && !NOT_ON_THE_MACRO.iter().any(|(name, _)| name == ty)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "collection newtypes defined in the model but not covered above: {missing:?}"
    );
    Ok(())
}
