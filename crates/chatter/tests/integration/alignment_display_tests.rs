//! Display must use the same domain and index space as the alignment producer.

use crate::common::{chatter_cmd, reference_fixture};

#[derive(Clone, Copy)]
enum View {
    Full,
    Compact,
}

fn assert_pairs(fixture: &str, tier: &str, pairs: &[(&str, &str)]) {
    for view in [View::Full, View::Compact] {
        let mut command = chatter_cmd();
        command
            .args(["inspect", "alignment"])
            .arg(reference_fixture(&format!("corpus/reference/{fixture}")))
            .args(["--tier", tier]);
        match view {
            View::Full => {}
            View::Compact => {
                command.arg("--compact");
            }
        }
        let result = command.assert().success();
        let stdout = std::str::from_utf8(&result.get_output().stdout).unwrap();
        for (source, target) in pairs {
            let expected = format!("\"{source}\" → \"{target}\"");
            assert!(stdout.contains(&expected), "missing {expected}:\n{stdout}");
        }
    }
}

#[test]
fn gra_display_uses_clitic_chunks_and_terminator() {
    assert_pairs(
        "edge-cases/clitics-and-compounds.cha",
        "gra",
        &[
            ("pron|it", "1|4|NSUBJ"),
            ("aux|be-Pres-S3", "2|1|AUX"),
            ("det|a", "3|4|DET"),
            ("noun|cookie", "4|0|ROOT"),
            (".", "5|4|PUNCT"),
        ],
    );
}

#[test]
fn mor_display_skips_retraced_content() {
    assert_pairs(
        "annotation/retrace.cha",
        "mor",
        &[
            ("kitty", "noun|kitty"),
            ("is", "aux|be-Fin-Ind-Pres-S3"),
            ("nice", "adj|nice-S1"),
        ],
    );
}

#[test]
fn pho_display_preserves_atomic_group_positions() {
    assert_pairs(
        "annotation/groups-phonological.cha",
        "pho",
        &[("‹A›", "a"), ("B", "b"), ("C", "c")],
    );
}

#[test]
fn sin_display_preserves_action_and_atomic_group_positions() {
    assert_pairs(
        "annotation/groups-sign.cha",
        "sin",
        &[
            ("0", "g:toy:dpoint"),
            ("foo", "b"),
            ("0", "〔c d〕"),
            ("〔bar baz〕", "e"),
        ],
    );
}
