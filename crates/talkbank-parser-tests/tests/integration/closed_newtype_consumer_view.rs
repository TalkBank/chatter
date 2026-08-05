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

/// Collection newtypes the model defines but does NOT make publicly reachable,
/// so no consumer can name them and their accessors are unreachable API.
///
/// This is a real gap, recorded rather than fixed: exporting them widens the
/// public surface, which is a deliberate decision and not a cleanup. The list
/// is here so `every_collection_newtype_is_listed_here` stays exhaustive and a
/// NEW unreachable type has to be added consciously.
const NOT_ON_THE_MACRO: &[(&str, &str)] = &[
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
    assert_consumer_api!(annotation::ReplacementWords, Word);
    assert_consumer_api!(annotation::ReplacedWordAnnotations, ContentAnnotation);
    assert_consumer_api!(annotation::AnnotatedContentAnnotations, ContentAnnotation);
    assert_consumer_api!(WordLanguageInfos, WordLanguageInfo);
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
