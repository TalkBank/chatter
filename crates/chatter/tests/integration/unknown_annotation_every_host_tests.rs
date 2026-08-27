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

//! An unrecognised scoped annotation is refused whatever it is attached to.
//!
//! # The regression these pin
//!
//! 0.16.0 taught the re2c backend to report an unrecognised annotation as E207
//! ("unknown annotation", a statement about the FILE) instead of E321
//! ("unparsable utterance", a statement about the PARSER). That fix reached
//! exactly one host construct, the WORD, and turned the other five into
//! SILENCE: measured on 2026-08-27, `<hello world> [qq] .`, `&=laughs [qq] .`,
//! `0 [qq] .`, `“hello” [qq] .` and `hello (.) [qq] .` all validated CLEAN
//! under `--parser re2c`, where v0.15.0 refused every one of them and the
//! default backend still does.
//!
//! Three of those five are fixed here. The other two are a DIFFERENT defect at
//! a different layer, and are pinned separately below.
//!
//! The cause, and why it bit one backend only, is recorded once, on
//! `talkbank_model::validation::main_tier::report_unknown_annotations`.
//!
//! These drive the real CLI subprocess under BOTH backends, because the defect
//! was precisely that the two disagreed.

use crate::common::{CliHarness, combined_output, write_fixture};
use talkbank_parser_tests::test_error::TestError;

/// A minimal CHAT header used by the inline fixtures below.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|2;||||Target_Child|||\n";

/// Every construct an annotation can attach to, with an unrecognised one on it.
///
/// `[qq]` is not a marker any rule knows, on either backend. The list is the
/// host constructs, which is the axis the bug ran along: the annotation was
/// always the same and only its host decided whether anything noticed.
/// The two backends every case below is run under.
const BACKENDS: &[&str] = &["tree-sitter", "re2c"];

const HOSTS: &[(&str, &str)] = &[
    ("word", "hello [qq] ."),
    ("group", "<hello world> [qq] ."),
    ("event", "&=laughs [qq] ."),
    ("action", "0 [qq] ."),
];

/// Hosts the re2c parser DISCARDS the annotation from, before validation ever
/// sees it. Refused by the default backend; accepted by re2c.
///
/// A different defect from the one above, at a different layer, which is why
/// they are a separate list rather than a longer one. The validation coupling
/// is fixed; this is information lost at PARSE time:
///
/// ```text
/// crates/talkbank-parser-re2c/src/ast.rs
///   pub struct Group    { contents: ..., annotations: Vec<ParsedAnnotation> }
///   pub struct Quotation{ contents: ... }                  // <- no field
/// ```
///
/// Six lines apart. A quotation's annotations are not captured, so no
/// validation can report them and `chatter normalize --parser re2c` would
/// serialize the quotation without them. The pause is the same shape: the
/// default backend refuses an annotated pause categorically, annotated with a
/// KNOWN marker or not, so the construct is not representable there at all.
///
/// Pinned as the CURRENT behaviour, not endorsed. When the re2c parser learns
/// to carry these, this test fails, and the fix is to move the host into
/// `HOSTS` above and delete it here.
const HOSTS_RE2C_DISCARDS: &[(&str, &str)] = &[
    ("quotation", "“hello” [qq] ."),
    ("pause", "hello (.) [qq] ."),
];

/// Validate one host body under one backend, and hand back the process output.
///
/// Five copies of this stood in the tests below: format the source from
/// `HEADER`, write the fixture, then spell out the `validate` argv with a
/// `path.to_str().unwrap()`. `CliHarness::run_validate` already owns the verb
/// and the path, so the argv here is only the flags that differ.
fn validate_host(
    harness: &CliHarness,
    name: &str,
    body: &str,
    backend: &str,
) -> Result<std::process::Output, TestError> {
    let source = format!("{HEADER}*CHI:\t{body}\n@End\n");
    let path = write_fixture(harness.home_dir(), &format!("{name}.cha"), &source)?;
    harness.run_validate(&path, &["--parser", backend, "--force"])
}

#[test]
fn an_unrecognised_annotation_is_refused_on_every_host_and_backend() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let mut accepted = Vec::new();

    for (host, body) in HOSTS {
        for backend in BACKENDS {
            let output = validate_host(&harness, host, body, backend)?;
            if output.status.success() {
                accepted.push(format!(
                    "  {backend:<12} accepted {host:<10} {body:?}\n{}",
                    combined_output(&output)
                ));
            }
        }
    }

    assert!(
        accepted.is_empty(),
        "an unrecognised scoped annotation was ACCEPTED. Silence is the defect here: \
         v0.15.0 refused every one of these, and a backend that answers \"valid\" on \
         input the authority refuses is useless as a specification oracle.\n{}",
        accepted.join("\n")
    );
    Ok(())
}

/// A REPLACED word's unknown annotation is reported ONCE, not twice.
///
/// The adversarial case the author of the fix above did not enumerate, and it
/// was a regression: `dog [: cat] [@ xyz] .` reported ONE E321 under
/// `--parser re2c` at v0.15.0 and TWO E207 after the fix, because the model
/// grew a second emitter without the first being removed.
/// `ReplacedWordAnnotations` has its own `Validate` impl that reports the same
/// annotations the new tier traversal reports, and the two carry different
/// spans, so nothing downstream collapses them.
///
/// The count is the assertion. A test that only checked "E207 is present"
/// passes in both the correct and the doubled world, which is precisely how
/// this shipped.
#[test]
fn a_replaced_words_unknown_annotation_is_reported_once() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let source = format!("{HEADER}*CHI:\tdog [: cat] [@ xyz] .\n@End\n");
    let path = write_fixture(harness.home_dir(), "replaced.cha", &source)?;

    for backend in BACKENDS {
        let output = harness.run_validate(&path, &["--parser", backend, "--force"])?;
        let rendered = combined_output(&output);
        let count = rendered.matches("error[E207]").count();
        assert_eq!(
            count, 1,
            "{backend}: expected exactly one E207 for one unknown annotation, got {count}.\n{rendered}"
        );
    }
    Ok(())
}

/// The two hosts re2c still discards, pinned so the gap is visible and so the
/// eventual parser fix is forced to come here and say it is done.
#[test]
fn re2c_still_discards_an_annotation_on_a_quotation_or_pause() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    for (host, body) in HOSTS_RE2C_DISCARDS {
        let authority = validate_host(&harness, &format!("gap_{host}"), body, "tree-sitter")?;
        assert!(
            !authority.status.success(),
            "the default backend is supposed to refuse {host}; if it now accepts, \
             this whole test is about the wrong thing"
        );

        let oracle = validate_host(&harness, &format!("gap_{host}"), body, "re2c")?;
        assert!(
            oracle.status.success(),
            "re2c now REFUSES an annotation on a {host}. That is the fix this \
             gap was waiting for: move {host:?} from HOSTS_RE2C_DISCARDS into \
             HOSTS above and delete it here.\n{}",
            combined_output(&oracle)
        );
    }
    Ok(())
}

/// The annotations a rule DOES know must still be accepted, on the same hosts.
///
/// The control. A refusal that fires on everything is not a fix, and the
/// cheapest way to "pass" the test above is to reject all annotations.
#[test]
fn a_recognised_annotation_is_still_accepted_on_every_host() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let mut refused = Vec::new();

    // `[= explanation]` is ordinary valid CHAT and attaches to every host in
    // `HOSTS`. The pause is not in that list: the default backend refuses an
    // annotated pause categorically, with a known marker or an unknown one, so
    // it has no positive case to control against.
    for (host, body) in HOSTS {
        let ok_body = body.replace("[qq]", "[= a note]");
        for backend in BACKENDS {
            let output = validate_host(&harness, &format!("ok_{host}"), &ok_body, backend)?;
            if !output.status.success() {
                refused.push(format!(
                    "  {backend:<12} refused {host:<10} {ok_body:?}\n{}",
                    combined_output(&output)
                ));
            }
        }
    }

    assert!(
        refused.is_empty(),
        "a RECOGNISED annotation was refused; the unknown-annotation check has \
         become a reject-everything check:\n{}",
        refused.join("\n")
    );
    Ok(())
}
