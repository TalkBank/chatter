//! In-place commands must preserve originals on real publication failures.

use crate::common::chatter_cmd;
use std::fs;

const FIX_SOURCE: &str = include_str!(
    "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E241_illegal_untranscribed_marker_3.cha"
);
#[cfg(unix)]
const TRANSFORM_SOURCE: &str = include_str!("../../../../corpus/reference/annotation/retrace.cha");

#[test]
fn repair_refuses_hard_link_without_changing_either_name() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.cha");
    let alias = directory.path().join("alias.cha");
    fs::write(&path, FIX_SOURCE).unwrap();
    fs::hard_link(&path, &alias).unwrap();
    let result = chatter_cmd()
        .arg("fix")
        .arg(&path)
        .arg("--apply")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(fs::read_to_string(&path).unwrap(), FIX_SOURCE);
    assert_eq!(fs::read_to_string(&alias).unwrap(), FIX_SOURCE);
    assert!(String::from_utf8_lossy(&result.stdout).contains("0 fix(es) applied"));
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[test]
fn refused_repair_does_not_stop_a_later_admitted_file() {
    let directory = tempfile::tempdir().unwrap();
    let refused = directory.path().join("refused.cha");
    let accepted = directory.path().join("accepted.cha");
    fs::write(&refused, FIX_SOURCE).unwrap();
    fs::hard_link(&refused, directory.path().join("alias.cha")).unwrap();
    fs::write(&accepted, FIX_SOURCE).unwrap();
    let result = chatter_cmd()
        .arg("fix")
        .arg(&refused)
        .arg(&accepted)
        .arg("--apply")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(fs::read_to_string(&refused).unwrap(), FIX_SOURCE);
    let expected = FIX_SOURCE.replace("I said xx today", "I said xxx today");
    assert_ne!(expected, FIX_SOURCE);
    assert_eq!(fs::read_to_string(&accepted).unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn kernel_write_failure_keeps_repair_and_transform_sources() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    enum Operation {
        Repair,
        Retag,
    }
    for operation in [Operation::Repair, Operation::Retag] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.cha");
        let (source, args): (&str, &[&str]) = match operation {
            Operation::Repair => (FIX_SOURCE, &["fix", "--apply"]),
            Operation::Retag => (
                TRANSFORM_SOURCE,
                &[
                    "experimental",
                    "transform",
                    "retag-language",
                    "--from",
                    "eng",
                    "--to",
                    "spa",
                    "--apply",
                ],
            ),
        };
        fs::write(&path, source).unwrap();
        let selected = chatter_cmd();
        let mut child = Command::new("/bin/sh");
        child
            .args(["-c", "ulimit -f 0; exec \"$@\"", "replacement-write-test"])
            .arg(selected.get_program())
            .args(args)
            .arg(&path);
        for (key, value) in selected.get_envs() {
            match value {
                Some(value) => {
                    child.env(key, value);
                }
                None => {
                    child.env_remove(key);
                }
            }
        }
        let result = child.output().unwrap();
        assert_eq!(result.status.signal(), Some(25), "{result:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}
