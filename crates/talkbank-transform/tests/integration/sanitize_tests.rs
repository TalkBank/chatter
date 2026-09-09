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

//! Integration tests for the CHAT-file sanitizer.
//!
//! All fixtures here are synthetic, built from scratch as string
//! literals so the test suite never depends on real contributor
//! transcripts. Every test asserts a specific structural-preservation
//! or content-redaction property; together they cover the v1 leak
//! surface documented in `book/src/user-guide/sanitize.md`.

use talkbank_model::WriteChat;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::redact::{SanitizationPolicy, sanitize};

/// Parses `cha` and runs the strict sanitizer; returns the serialized output.
fn sanitize_to_string(cha: &str) -> String {
    let parser = TreeSitterParser::new().expect("parser construction");
    let parsed = parser.parse_chat_file(cha).expect_built();
    let policy = SanitizationPolicy::strict();
    let sanitized = sanitize(parsed, &policy).expect("sanitize");
    sanitized.to_chat_string()
}

/// Parses `cha` and re-serializes it through the model without sanitizing,
/// gives the round-trip baseline for byte comparisons.
fn roundtrip_only(cha: &str) -> String {
    let parser = TreeSitterParser::new().expect("parser construction");
    let parsed = parser.parse_chat_file(cha).expect_built();
    let mut out = String::new();
    parsed.write_chat(&mut out).expect("write_chat");
    out
}

const BULLET: char = '\u{0015}';

/// Wraps a single-participant PAR-only test body in the standard CHAT
/// header skeleton. `body` is the text between `@ID` and `@End`,
/// callers supply just the part their test cares about.
fn solo_par(body: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult\n\
@ID:\teng|test|PAR|||||Adult|||\n\
{body}\
@End\n"
    )
}

/// Fixture: 4 utterances with distinct `•start_end•` patterns.
fn fixture_four_utterances_with_bullets() -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult, INV Investigator\n\
@ID:\teng|test|PAR|||||Adult|||\n\
@ID:\teng|test|INV|||||Investigator|||\n\
*PAR:\tthe cat sat . {BULLET}1000_2000{BULLET}\n\
*INV:\tand then ? {BULLET}2100_2900{BULLET}\n\
*PAR:\tit ran away . {BULLET}3000_4500{BULLET}\n\
*INV:\twhy ? {BULLET}4600_5000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    )
}

#[test]
fn t01_bullets_byte_exact() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    for ms in ["1000_2000", "2100_2900", "3000_4500", "4600_5000"] {
        let needle = format!("{BULLET}{ms}{BULLET}");
        assert!(
            out.contains(&needle),
            "bullet pattern {needle:?} not preserved byte-exactly in:\n{out}"
        );
    }
}

/// `%wor` carries each main-tier word again beside its bullet, so the
/// sanitizer gives each `%wor` word the placeholder of the main-tier word
/// it aligns to and keeps every bullet byte-exact. The pairing is the
/// model's own (`WorMainTierProjection::bind_timing`, then
/// `corroborate_wor_timing`, decided before the main tier is rewritten),
/// so an `xxx` on the main tier, which `%wor` never lists, shifts nothing;
/// a tier whose word count drifts from the main tier's slots (here, one
/// that lists a fragment the policy excludes), or whose words did not all
/// match the main tier's (`dog` for `the`; an `xxx` written where the main
/// tier says `cat`), pairs with nothing and takes fresh placeholders, so no
/// false correspondence is written, the `xxx` passing through as it does
/// on the main tier; and a compound
/// takes the main word's whole display text once, the form a generated
/// `%wor` carries. Sanitizing the output again reproduces it.
///
/// Until 2026-09-08 this test's fixture wrote the timings as bare
/// `1000_1100` tokens, which is not CHAT (`%wor` bullets carry the same
/// `\u{15}` delimiters as main-tier bullets) and which the model parses as
/// six WORDS; the test then checked that those "offsets" survived, which
/// they did only because the sanitizer wrote `%wor` out untouched, words
/// and all. The whole line is pinned now, so a leaked word shows.
#[test]
fn t02_wor_per_word_offsets_byte_exact() {
    let b = BULLET;
    let rows: [(&str, String, String); 6] = [
        (
            "plain",
            format!(
                "*PAR:\tthe cat sat . {b}1000_2000{b}\n%wor:\tthe {b}1000_1100{b} cat {b}1100_1500{b} sat {b}1500_2000{b} .\n"
            ),
            format!(
                "*PAR:\tw1 w2 w3 . {b}1000_2000{b}\n%wor:\tw1 {b}1000_1100{b} w2 {b}1100_1500{b} w3 {b}1500_2000{b} .\n"
            ),
        ),
        (
            "an xxx on the main tier, which %wor does not list",
            format!(
                "*PAR:\tthe xxx cat sat . {b}1000_2000{b}\n%wor:\tthe {b}1000_1100{b} cat {b}1100_1500{b} sat {b}1500_2000{b} .\n"
            ),
            format!(
                "*PAR:\tw1 xxx w2 w3 . {b}1000_2000{b}\n%wor:\tw1 {b}1000_1100{b} w2 {b}1100_1500{b} w3 {b}1500_2000{b} .\n"
            ),
        ),
        (
            "an xxx on %wor itself: uncorroborated, fresh placeholders, the xxx stays",
            format!(
                "*PAR:\tthe cat sat . {b}1000_2000{b}\n%wor:\tthe {b}1000_1100{b} xxx {b}1100_1500{b} sat {b}1500_2000{b} .\n"
            ),
            format!(
                "*PAR:\tw1 w2 w3 . {b}1000_2000{b}\n%wor:\tw4 {b}1000_1100{b} xxx {b}1100_1500{b} w5 {b}1500_2000{b} .\n"
            ),
        ),
        (
            "a %wor that lists a fragment the policy excludes: drifted, fresh placeholders",
            format!(
                "*PAR:\tthe &+fr cat sat . {b}1000_2000{b}\n%wor:\tthe {b}900_1000{b} fr {b}1000_1050{b} cat {b}1100_1500{b} sat {b}1500_2000{b} .\n"
            ),
            format!(
                "*PAR:\tw1 &+w2 w3 w4 . {b}1000_2000{b}\n%wor:\tw5 {b}900_1000{b} w6 {b}1000_1050{b} w7 {b}1100_1500{b} w8 {b}1500_2000{b} .\n"
            ),
        ),
        (
            "a count-matched %wor whose words never agreed: no agreement is manufactured",
            format!(
                "*PAR:\tthe cat . {b}1000_2000{b}\n%wor:\tdog {b}1000_1100{b} cat {b}1100_1500{b} .\n"
            ),
            format!(
                "*PAR:\tw1 w2 . {b}1000_2000{b}\n%wor:\tw3 {b}1000_1100{b} w4 {b}1100_1500{b} .\n"
            ),
        ),
        (
            "a compound takes the main word's display text once",
            format!(
                "*PAR:\tmail+man cat . {b}1000_2000{b}\n%wor:\tmail+man {b}1000_1100{b} cat {b}1100_1500{b} .\n"
            ),
            format!(
                "*PAR:\tw1+w1 w2 . {b}1000_2000{b}\n%wor:\tw1w1 {b}1000_1100{b} w2 {b}1100_1500{b} .\n"
            ),
        ),
    ];
    for (what, body, expected_body) in &rows {
        let out = sanitize_to_string(&solo_par(body));
        assert_eq!(out, solo_par(expected_body), "{what}");
        assert_eq!(sanitize_to_string(&out), out, "idempotent: {what}");
    }
}

#[test]
fn t03_structural_counts_preserved() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);

    let count_lines =
        |s: &str, prefix: &str| -> usize { s.lines().filter(|l| l.starts_with(prefix)).count() };
    assert_eq!(count_lines(&out, "*PAR:"), 2, "main *PAR utterance count");
    assert_eq!(count_lines(&out, "*INV:"), 2, "main *INV utterance count");
}

#[test]
fn t04_speaker_codes_preserved() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    assert!(out.contains("*PAR:"), "*PAR speaker code missing");
    assert!(out.contains("*INV:"), "*INV speaker code missing");
    assert!(
        !out.contains("*MAR:") && !out.contains("*JOH:"),
        "no fabricated speaker codes"
    );
}

#[test]
fn t05_word_content_replaced_with_placeholders() {
    let cha = solo_par(&format!(
        "*PAR:\tthe cat sat on the mat . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    for word in ["the ", "cat ", "sat ", "on ", "mat "] {
        assert!(
            !out.contains(&format!("\t{word}")) && !out.contains(&format!(" {word}")),
            "source word {word:?} leaked into output:\n{out}"
        );
    }
    assert!(out.contains("w1"), "expected w1 placeholder, got:\n{out}");
    assert!(out.contains("w6"), "expected w6 placeholder, got:\n{out}");
}

#[test]
fn t06_compound_and_clitic_markers_preserved() {
    let cha = solo_par(&format!(
        "*PAR:\tice+cream and dog~s . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        out.contains('+'),
        "compound marker '+' not preserved:\n{out}"
    );
    assert!(
        out.contains('~'),
        "clitic boundary '~' not preserved:\n{out}"
    );
    assert!(
        !out.contains("ice") && !out.contains("cream") && !out.contains("dog"),
        "lexical content leaked:\n{out}"
    );
}

#[test]
fn t07_id_custom_field_anonymized() {
    // t07 has its own skeleton because it puts a name into the @ID
    // custom_field slot, that's the whole point of this test.
    let cha = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult\n\
@ID:\teng|test|PAR|||||Adult||Jane Smith|\n\
*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    );
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Jane Smith") && !out.contains("Jane") && !out.contains("Smith"),
        "@ID custom_field name leaked:\n{out}"
    );
}

#[test]
fn t08_participants_name_anonymized() {
    // Names land in @Participants' name field. @ID role slot is reserved for
    // closed-set roles (Adult, Target_Child, Mother, Investigator, etc.) and
    // is intentionally preserved.
    let cha = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Mary_Jones Adult, INV John_Doe Investigator\n\
@ID:\teng|test|PAR|||||Adult|||\n\
@ID:\teng|test|INV|||||Investigator|||\n\
*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    );
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Mary")
            && !out.contains("Jones")
            && !out.contains("John")
            && !out.contains("Doe"),
        "@Participants name leaked:\n{out}"
    );
}

#[test]
fn t09_mor_lemma_replaced_pos_preserved() {
    let cha = solo_par(&format!(
        "*PAR:\tthe cat sat . {BULLET}1000_2000{BULLET}\n\
%mor:\tdet|the n|cat v|sit-Past .\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("|the") && !out.contains("|cat") && !out.contains("|sit"),
        "%mor lemma leaked:\n{out}"
    );
    assert!(out.contains("det|"), "POS det| missing:\n{out}");
    assert!(out.contains("n|"), "POS n| missing:\n{out}");
    assert!(out.contains("v|"), "POS v| missing:\n{out}");
    assert!(out.contains("-Past"), "feature -Past missing:\n{out}");
}

#[test]
fn t10_phonological_tier_dropped() {
    let cha = solo_par(&format!(
        "*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
%pho:\thəloʊ .\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("%pho"),
        "%pho tier should be dropped from output:\n{out}"
    );
    assert!(
        !out.contains("həloʊ"),
        "phonological content should not appear:\n{out}"
    );
}

#[test]
fn t11_freetext_dependent_tier_redacted() {
    let cha = solo_par(&format!(
        "*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
%com:\tchild was tired and stopped responding\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("tired") && !out.contains("stopped") && !out.contains("responding"),
        "%com free-text leaked:\n{out}"
    );
    assert!(
        out.contains("[redacted]"),
        "expected '[redacted]' marker in %com:\n{out}"
    );
}

#[test]
fn t12_untranscribed_words_preserved() {
    let cha = solo_par(&format!("*PAR:\txxx and yyy . {BULLET}1000_2000{BULLET}\n"));
    let out = sanitize_to_string(&cha);
    assert!(
        out.contains("xxx"),
        "untranscribed marker xxx must be preserved:\n{out}"
    );
    assert!(
        out.contains("yyy"),
        "untranscribed marker yyy must be preserved:\n{out}"
    );
}

#[test]
fn t13_sanitized_output_parses_back() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    let parser = TreeSitterParser::new().expect("parser construction");
    let product = parser.parse_chat_file(&out);
    assert!(
        !product.has_error_diagnostics(),
        "sanitized output should re-parse cleanly, got diagnostics: {:?}",
        product.diagnostics()
    );
    assert!(
        product.is_built(),
        "sanitized output should re-parse to a model"
    );
}

#[test]
fn t14_idempotent_under_repeat() {
    let cha = fixture_four_utterances_with_bullets();
    let once = sanitize_to_string(&cha);
    let twice = sanitize_to_string(&once);
    assert_eq!(
        once, twice,
        "sanitize must be idempotent (sanitize(sanitize(x)) == sanitize(x))"
    );
}

#[test]
fn t15_deterministic_across_runs() {
    let cha = fixture_four_utterances_with_bullets();
    let a = sanitize_to_string(&cha);
    let b = sanitize_to_string(&cha);
    assert_eq!(
        a, b,
        "two runs of sanitize on the same input must be byte-identical"
    );
}

#[test]
fn t16_event_and_freecode_redacted() {
    let cha = solo_par(&format!(
        "*PAR:\thello &=imitates:Mary [^ aside about Jane] . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Mary")
            && !out.contains("Jane")
            && !out.contains("imitates")
            && !out.contains("aside"),
        "Event/Freecode free text leaked:\n{out}"
    );
}

#[test]
fn t17_replaced_word_both_sides_sanitized() {
    // ReplacedWord carries the actually-spoken word AND the intended
    // replacement(s), both contain lexical content and both must be
    // redacted. Parser shape: `actual [: replacement]`.
    let cha = solo_par(&format!(
        "*PAR:\the goed [: went] home . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("goed") && !out.contains("went"),
        "ReplacedWord lexical content leaked:\n{out}"
    );
    assert!(
        !out.contains("home") && !out.contains(" he "),
        "surrounding words leaked:\n{out}"
    );
}

/// Sanity test on the round-trip helper itself, parser must round-trip
/// our fixture's bullet patterns without sanitization, so other test
/// failures aren't parsing problems disguised as sanitization bugs.
#[test]
fn t00_fixture_roundtrips_without_sanitize() {
    let cha = fixture_four_utterances_with_bullets();
    let out = roundtrip_only(&cha);
    for ms in ["1000_2000", "2100_2900", "3000_4500", "4600_5000"] {
        let needle = format!("{BULLET}{ms}{BULLET}");
        assert!(
            out.contains(&needle),
            "round-trip without sanitize lost bullet {needle:?}:\n{out}"
        );
    }
}

/// Every dependent-tier kind the model has, through the strict sanitizer.
///
/// The redaction rule is per tier kind (`redact/dependent_tier.rs`): the
/// bullet-payload tiers and the free-text tiers become the placeholder, the
/// `%x` tier too, `%mor` lemmas become placeholders, the numeric tiers
/// (`%gra`, `%tim`) keep their content, `%wor` words take the main tier's
/// placeholders beside their timings, and the phonological tiers
/// are dropped whole. Until 2026-09-08 the tests here reached three of the
/// twenty-odd arms; this row table reaches each one and pins the output line
/// by line, so an arm that silently kept content would show.
#[test]
fn every_dependent_tier_kind_is_redacted_or_dropped_as_its_rule_says() {
    let tiers = "\
%mor:\tn|hello .\n\
%gra:\t1|0|ROOT 2|1|PUNCT\n\
%wor:\thello \u{15}0_100\u{15} .\n\
%act:\twaves\n\
%cod:\tsecret code\n\
%com:\ta private remark\n\
%add:\tMOT\n\
%exp:\tan explanation\n\
%gpx:\ta gesture\n\
%int:\tan intonation\n\
%sit:\ta situation\n\
%spa:\ta speech act\n\
%alt:\tan alternative\n\
%coh:\ta cohesion note\n\
%def:\ta definition\n\
%eng:\tan English gloss\n\
%err:\tan error note\n\
%fac:\ta facial note\n\
%flo:\ta flow note\n\
%gls:\ta gloss\n\
%ort:\tan orthography\n\
%par:\ta paralinguistic note\n\
%tim:\t17:30-18:00\n\
%xfoo:\ta user-defined note\n\
%pho:\th\u{259}\u{2C8}lo\u{28A}\n\
%mod:\th\u{259}\u{2C8}lo\u{28A}\n";
    let cha = solo_par(&format!("*PAR:\thello .\n{tiers}"));
    let out = sanitize_to_string(&cha);
    let lines: Vec<&str> = out.lines().collect();
    let expect_line = |prefix: &str, expected: &str| {
        let found = lines
            .iter()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("no {prefix} line in:\n{out}"));
        assert_eq!(*found, expected, "the {prefix} line after sanitizing");
    };
    // Bullet-payload tiers and free-text tiers: the placeholder.
    for prefix in [
        "%act:", "%cod:", "%com:", "%add:", "%exp:", "%gpx:", "%int:", "%sit:", "%spa:", "%alt:",
        "%coh:", "%def:", "%eng:", "%err:", "%fac:", "%flo:", "%gls:", "%ort:", "%par:", "%xfoo:",
    ] {
        expect_line(prefix, &format!("{prefix}\t[redacted]"));
    }
    // Numeric and structural tiers keep their content.
    expect_line("%gra:", "%gra:\t1|0|ROOT 2|1|PUNCT");
    expect_line("%tim:", "%tim:\t17:30-18:00");
    // `%wor` repeats the main tier's words beside their timings, so it
    // carries the main tier's placeholders (the same token per aligned
    // word, which `%wor` corroboration compares) and keeps the timings.
    expect_line("*PAR:", "*PAR:\tw1 .");
    expect_line("%wor:", "%wor:\tw1 \u{15}0_100\u{15} .");
    // `%mor` keeps its shape and loses its lemma.
    let mor = lines
        .iter()
        .find(|line| line.starts_with("%mor:"))
        .unwrap_or_else(|| panic!("no %mor line in:\n{out}"));
    assert!(!mor.contains("hello"), "the %mor lemma is redacted: {mor}");
    assert!(
        mor.starts_with("%mor:\tn|"),
        "the %mor shape survives: {mor}"
    );
    // Phonological tiers are dropped whole.
    for prefix in ["%pho:", "%mod:"] {
        assert!(
            !lines.iter().any(|line| line.starts_with(prefix)),
            "{prefix} is dropped, but the output has it:\n{out}"
        );
    }
}
