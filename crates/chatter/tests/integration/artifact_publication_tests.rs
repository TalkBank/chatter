//! Real artifact writers replace a destination name without truncating aliases.

use crate::common::CliHarness;
use std::{fs, path::Path, process::Command};

#[derive(Clone, Copy, Debug)]
enum Artifact {
    New,
    ToJson,
    FromJson,
    DirectoryJson,
    Redacted,
    Speakers,
    Rediarized,
}

const ARTIFACTS: [Artifact; 7] = [
    Artifact::New,
    Artifact::ToJson,
    Artifact::FromJson,
    Artifact::DirectoryJson,
    Artifact::Redacted,
    Artifact::Speakers,
    Artifact::Rediarized,
];

#[test]
fn invalid_custom_form_json_refuses_chat_publication_without_touching_the_destination() {
    let harness = CliHarness::new().expect("isolated CLI");
    let root = tempfile::tempdir().expect("artifact directory");
    let source =
        crate::common::reference_fixture("corpus/reference/content/words-special-forms.cha");
    let exported = harness
        .streaming_command()
        .args(["convert", "to-json"])
        .arg(source)
        .output()
        .expect("export reference JSON");
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let original: serde_json::Value =
        serde_json::from_slice(&exported.stdout).expect("reference JSON");
    let input = root.path().join("input.json");
    let destination = root.path().join("output.cha");
    for label in ["", "two words", "x\ty", "x\n@End", "x@f", "x$n", "x:y"] {
        let mut wire = original.clone();
        let marker = wire["lines"]
            .as_array_mut()
            .expect("line array")
            .iter_mut()
            .find_map(|line| line.pointer_mut("/main/content/content/0/form_type"))
            .expect("reference form marker");
        assert!(marker.as_str().is_some(), "existing declared marker");
        *marker = serde_json::json!({ "z": label });
        fs::write(
            &input,
            serde_json::to_vec(&wire).expect("malformed label JSON"),
        )
        .expect("input");
        fs::write(&destination, b"retained destination bytes").expect("existing destination");
        let refused = harness
            .streaming_command()
            .args(["convert", "from-json"])
            .arg(&input)
            .arg("--output")
            .arg(&destination)
            .output()
            .expect("JSON import");
        assert_eq!(refused.status.code(), Some(1));
        assert!(refused.stdout.is_empty());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("custom form label"));
        assert_eq!(
            fs::read(&destination).expect("retained destination"),
            b"retained destination bytes"
        );
    }
}

#[test]
fn invalid_pos_hint_json_refuses_chat_publication_without_touching_the_destination() {
    let harness = CliHarness::new().expect("isolated CLI");
    let root = tempfile::tempdir().expect("artifact directory");
    let source = crate::common::reference_fixture("corpus/reference/word/pos-hint-vocabulary.cha");
    let exported = harness
        .streaming_command()
        .args(["convert", "to-json"])
        .arg(source)
        .output()
        .expect("export reference JSON");
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let original: serde_json::Value =
        serde_json::from_slice(&exported.stdout).expect("reference JSON");
    let input = root.path().join("input.json");
    let destination = root.path().join("output.cha");
    for payload in ["", "n\tv", "n\n@End", "n$adj"] {
        let mut wire = original.clone();
        let tag = wire["lines"]
            .as_array_mut()
            .expect("line array")
            .iter_mut()
            .find_map(|line| line.pointer_mut("/main/content/content/0/part_of_speech"))
            .expect("reference POS hint");
        assert!(tag.as_str().is_some(), "existing hint is a string");
        *tag = serde_json::json!(payload);
        fs::write(
            &input,
            serde_json::to_vec(&wire).expect("malformed payload JSON"),
        )
        .expect("input");
        fs::write(&destination, b"retained destination bytes").expect("existing destination");
        let refused = harness
            .streaming_command()
            .args(["convert", "from-json"])
            .arg(&input)
            .arg("--output")
            .arg(&destination)
            .output()
            .expect("JSON import");
        assert_eq!(refused.status.code(), Some(1));
        assert!(refused.stdout.is_empty());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("word POS tag"));
        assert_eq!(
            fs::read(&destination).expect("retained destination"),
            b"retained destination bytes"
        );
    }
}

#[test]
fn conflicting_word_suffix_json_refuses_chat_publication_without_touching_the_destination() {
    let harness = CliHarness::new().expect("isolated CLI");
    let root = tempfile::tempdir().expect("artifact directory");
    let source =
        crate::common::reference_fixture("corpus/reference/content/words-special-forms.cha");
    let exported = harness
        .streaming_command()
        .args(["convert", "to-json"])
        .arg(source)
        .output()
        .expect("export reference JSON");
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let mut wire: serde_json::Value =
        serde_json::from_slice(&exported.stdout).expect("reference JSON");
    let word = wire["lines"]
        .as_array_mut()
        .expect("line array")
        .iter_mut()
        .filter_map(|line| line.pointer_mut("/main/content/content/0"))
        .find(|word| word.get("form_type").is_some())
        .expect("reference form word");
    word["lang"] = serde_json::json!({"type": "explicit", "code": "eng"});
    let input = root.path().join("input.json");
    let destination = root.path().join("output.cha");
    fs::write(
        &input,
        serde_json::to_vec(&wire).expect("conflicting suffix JSON"),
    )
    .expect("input");
    fs::write(&destination, b"retained destination bytes").expect("existing destination");
    let refused = harness
        .streaming_command()
        .args(["convert", "from-json"])
        .arg(&input)
        .arg("--output")
        .arg(&destination)
        .output()
        .expect("JSON import");
    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("only one '@' suffix"));
    assert_eq!(
        fs::read(&destination).expect("retained destination"),
        b"retained destination bytes"
    );
}

fn command(harness: &CliHarness, artifact: Artifact, root: &Path) -> Command {
    let mut command = harness.streaming_command();
    let input = root.join("inputs/media-bullets.cha");
    match artifact {
        Artifact::New => {
            command.arg("new");
        }
        Artifact::ToJson => {
            command.args(["convert", "to-json"]).arg(&input);
        }
        Artifact::FromJson => {
            command
                .args(["convert", "from-json"])
                .arg(root.join("input.json"));
        }
        Artifact::DirectoryJson => {
            command
                .args(["convert", "to-json"])
                .arg(root.join("inputs"))
                .arg("--output-dir")
                .arg(root.join("outputs"))
                .arg("--rebuild");
            return command;
        }
        Artifact::Redacted => {
            command
                .args(["experimental", "transform", "redact"])
                .arg(&input);
        }
        Artifact::Speakers => {
            command
                .args(["experimental", "speakers", "map"])
                .arg(&input)
                .args(["--mapping", "CHI=INV:Investigator"]);
        }
        Artifact::Rediarized => {
            command
                .args(["experimental", "speakers", "rediarize"])
                .arg(&input)
                .arg("--turns")
                .arg(root.join("turns.json"));
        }
    }
    command
        .arg("-o")
        .arg(root.join("outputs/media-bullets.json"));
    command
}

fn fixture(harness: &CliHarness) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir(root.join("inputs")).unwrap();
    fs::create_dir(root.join("outputs")).unwrap();
    fs::write(
        root.join("inputs/media-bullets.cha"),
        include_str!("../../../../corpus/reference/content/media-bullets.cha"),
    )
    .unwrap();
    fs::write(
        root.join("turns.json"),
        r#"{"turns":[{"track":"PAR0","start_ms":0,"end_ms":6000000}]}"#,
    )
    .unwrap();
    harness
        .chatter_cmd()
        .args(["convert", "to-json"])
        .arg(root.join("inputs/media-bullets.cha"))
        .arg("-o")
        .arg(root.join("input.json"))
        .assert()
        .success();
    directory
}

#[test]
fn artifact_publication_preserves_existing_hard_link_siblings() {
    let harness = CliHarness::new().unwrap();
    let directory = fixture(&harness);
    let root = directory.path();
    let output = root.join("outputs/media-bullets.json");
    let sibling = root.join("retained");
    for artifact in ARTIFACTS {
        fs::write(&output, "previous artifact").unwrap();
        fs::hard_link(&output, &sibling).unwrap();
        let result = command(&harness, artifact, root).output().unwrap();
        assert!(result.status.success(), "{artifact:?}: {result:?}");
        assert_ne!(fs::read(&output).unwrap(), b"previous artifact");
        assert_eq!(
            fs::read(&sibling).unwrap(),
            b"previous artifact",
            "{artifact:?}"
        );
        fs::remove_file(&sibling).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn artifact_write_failure_preserves_destination_and_removes_stage() {
    use std::os::unix::process::CommandExt;
    let harness = CliHarness::new().unwrap();
    let directory = fixture(&harness);
    let root = directory.path();
    let output = root.join("outputs/media-bullets.json");
    for artifact in ARTIFACTS {
        fs::write(&output, "previous artifact").unwrap();
        let mut child = command(&harness, artifact, root);
        // SAFETY: the child calls only async-signal-safe libc functions before exec.
        unsafe {
            child.pre_exec(|| {
                libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let result = child.output().unwrap();
        assert_eq!(result.status.code(), Some(1), "{artifact:?}: {result:?}");
        assert_eq!(
            fs::read(&output).unwrap(),
            b"previous artifact",
            "{artifact:?}"
        );
        assert_eq!(
            fs::read_dir(root.join("outputs")).unwrap().count(),
            1,
            "{artifact:?}"
        );
    }
}
