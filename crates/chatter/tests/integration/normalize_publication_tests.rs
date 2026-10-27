//! Real CLI publication controls; all destinations are isolated disposable copies.

use crate::common::chatter_cmd;
use std::fs;
use std::path::Path;

const REFERENCE: &str = include_str!("../../../../corpus/reference/annotation/retrace.cha");

fn normalize(input: &Path, output: &Path) -> std::process::Output {
    chatter_cmd()
        .arg("format")
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap()
}

fn refused(output: &std::process::Output) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("✓ Normalized"));
}

#[test]
fn new_output_publishes_the_reference() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    let output = directory.path().join("new.cha");
    fs::write(&input, REFERENCE).unwrap();
    let result = normalize(&input, &output);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(fs::read_to_string(&output).unwrap(), REFERENCE);
    assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
    assert!(String::from_utf8_lossy(&result.stderr).contains("✓ Normalized"));
}

#[test]
fn nonregular_output_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    let target = directory.path().join("directory.cha");
    fs::write(&input, REFERENCE).unwrap();
    fs::create_dir(&target).unwrap();
    refused(&normalize(&input, &target));
    assert!(target.is_dir());
}

#[cfg(unix)]
#[test]
fn symlink_and_dangling_symlink_outputs_are_refused() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    fs::write(&input, REFERENCE).unwrap();
    for target_name in ["input.cha", "missing.cha"] {
        let link = directory.path().join(format!("link-{target_name}"));
        symlink(directory.path().join(target_name), &link).unwrap();
        refused(&normalize(&input, &link));
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    }
    assert!(!directory.path().join("missing.cha").exists());
    assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
}

#[test]
fn multiply_linked_output_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    let alias = directory.path().join("alias.cha");
    fs::write(&input, REFERENCE).unwrap();
    fs::hard_link(&input, &alias).unwrap();
    refused(&normalize(&input, &alias));
    assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
    assert_eq!(fs::read_to_string(&alias).unwrap(), REFERENCE);
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::os::macos::fs::MetadataExt as MacMetadataExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::process::Command;

    fn system(program: &str, arguments: &[&str], path: &Path) -> std::process::Output {
        let output = Command::new(program)
            .args(arguments)
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        output
    }

    #[test]
    fn replacement_preserves_metadata_and_really_replaces_the_inode() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.cha");
        let output = directory.path().join("existing.cha");
        fs::write(&input, REFERENCE).unwrap();
        fs::write(&output, b"old output").unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
        system("/usr/bin/touch", &["-t", "202001020304.05"], &output);
        system(
            "/usr/bin/xattr",
            &["-w", "org.talkbank.normalize-test", "kept-value"],
            &output,
        );
        system(
            "/bin/chmod",
            &[
                "+a",
                "everyone allow read,readattr,readextattr,readsecurity",
            ],
            &output,
        );
        let before = fs::metadata(&output).unwrap();
        let acl_before = system("/bin/ls", &["-lde"], &output).stdout;
        let result = normalize(&input, &output);
        assert!(result.status.success(), "{result:?}");
        let after = fs::metadata(&output).unwrap();
        assert_ne!(before.ino(), after.ino());
        assert_eq!(before.mode(), after.mode());
        assert_eq!(before.uid(), after.uid());
        assert_eq!(before.gid(), after.gid());
        assert_eq!(
            (before.atime(), before.atime_nsec()),
            (after.atime(), after.atime_nsec())
        );
        assert_eq!(
            (before.mtime(), before.mtime_nsec()),
            (after.mtime(), after.mtime_nsec())
        );
        assert_eq!(
            (before.st_birthtime(), before.st_birthtime_nsec()),
            (after.st_birthtime(), after.st_birthtime_nsec())
        );
        assert_eq!(before.st_flags(), after.st_flags());
        assert_eq!(
            system(
                "/usr/bin/xattr",
                &["-p", "org.talkbank.normalize-test"],
                &output
            )
            .stdout,
            b"kept-value\n"
        );
        let acl_after = system("/bin/ls", &["-lde"], &output).stdout;
        // The first ls line contains length/mtime and must change with content;
        // the subsequent ACE lines must remain byte-identical.
        assert_eq!(
            String::from_utf8_lossy(&acl_before)
                .lines()
                .skip(1)
                .collect::<Vec<_>>(),
            String::from_utf8_lossy(&acl_after)
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
        );
        assert_eq!(fs::read_to_string(&output).unwrap(), REFERENCE);
    }

    #[test]
    fn inplace_reference_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.cha");
        fs::write(&path, REFERENCE).unwrap();
        let result = normalize(&path, &path);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), REFERENCE);
    }

    #[test]
    fn unsupported_file_flags_refuse_without_changing_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.cha");
        fs::write(&path, REFERENCE).unwrap();
        system("/usr/bin/chflags", &["hidden"], &path);
        refused(&normalize(&path, &path));
        assert_eq!(fs::read_to_string(&path).unwrap(), REFERENCE);
        assert_ne!(fs::metadata(&path).unwrap().st_flags(), 0);
    }

    #[test]
    fn late_rename_refusal_preserves_the_old_file() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.cha");
        let output = directory.path().join("existing.cha");
        fs::write(&input, REFERENCE).unwrap();
        fs::write(&output, b"old output").unwrap();
        system("/bin/chmod", &["+a", "everyone deny delete"], &output);
        let result = normalize(&input, &output);
        // Remove only this test's ACL before cleanup, even if an assertion fails.
        system("/bin/chmod", &["-N"], &output);
        for entry in fs::read_dir(directory.path()).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".chatter-output-")
            {
                system("/bin/chmod", &["-N"], &entry.path());
            }
        }
        refused(&result);
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains("Operation not permitted") || error.contains("Permission denied"),
            "{error}"
        );
        assert!(
            !error.contains("metadata could not be preserved"),
            "{error}"
        );
        assert_eq!(fs::read(&output).unwrap(), b"old output");
        assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
    }

    #[test]
    fn actual_limited_write_preserves_existing_and_absent_destinations() {
        use std::os::unix::process::ExitStatusExt;

        #[derive(Debug)]
        enum OutputCase {
            ExistingInput,
            NewOutput,
        }
        for case in [OutputCase::ExistingInput, OutputCase::NewOutput] {
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("input.cha");
            let output = match case {
                OutputCase::ExistingInput => input.clone(),
                OutputCase::NewOutput => directory.path().join("new.cha"),
            };
            fs::write(&input, REFERENCE).unwrap();
            assert!(REFERENCE.len() > 512);
            // Keep the harness's selected binary and cache isolation, even though
            // this subprocess needs a child-only kernel file-size limit.
            let selected = chatter_cmd();
            let mut command = Command::new("/bin/zsh");
            command
                .args([
                    "-c",
                    "ulimit -f 1; exec \"$1\" format \"$2\" -o \"$3\"",
                    "normalize-write-test",
                ])
                .arg(selected.get_program())
                .arg(&input)
                .arg(&output);
            for (name, value) in selected.get_envs() {
                match value {
                    Some(value) => {
                        command.env(name, value);
                    }
                    None => {
                        command.env_remove(name);
                    }
                }
            }
            let result = command.output().unwrap();
            refused(&result);
            // Darwin SIGXFSZ (25), not an arbitrary spawn/argument/parser error.
            // The actual kernel write-limit boundary must have been reached.
            assert_eq!(result.status.signal(), Some(25), "{case:?}: {result:?}");
            eprintln!(
                "write-limited {case:?}: {:?}; original bytes={}",
                result.status,
                REFERENCE.len()
            );
            assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
            match case {
                OutputCase::ExistingInput => {}
                OutputCase::NewOutput => assert!(!output.exists()),
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[test]
fn unqualified_existing_metadata_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    fs::write(&input, REFERENCE).unwrap();
    refused(&normalize(&input, &input));
    assert_eq!(fs::read_to_string(&input).unwrap(), REFERENCE);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_replacement_preserves_posix_metadata_attributes_and_acl() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    let output = directory.path().join("output.cha");
    fs::write(&input, REFERENCE).unwrap();
    fs::write(&output, "old output").unwrap();
    fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    xattr::set(&output, "user.chatter-test", b"retained value").unwrap();
    // Linux's version-2 POSIX ACL wire format. A named-user entry prevents
    // the kernel reducing this ACL to the ordinary mode bits.
    let mut acl = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in [
        (1_u16, 6_u16, u32::MAX),
        (2, 4, 4242),
        (4, 0, u32::MAX),
        (16, 4, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(permissions.to_le_bytes());
        acl.extend(id.to_le_bytes());
    }
    xattr::set(&output, "system.posix_acl_access", &acl).unwrap();
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_934_245);
    fs::File::open(&output)
        .unwrap()
        .set_times(fs::FileTimes::new().set_accessed(time).set_modified(time))
        .unwrap();
    let before = fs::metadata(&output).unwrap();
    let result = normalize(&input, &output);
    assert!(result.status.success(), "{result:?}");
    let after = fs::metadata(&output).unwrap();
    assert_ne!(before.ino(), after.ino());
    assert_eq!(before.mode(), after.mode());
    assert_eq!((before.uid(), before.gid()), (after.uid(), after.gid()));
    assert_eq!(before.accessed().unwrap(), after.accessed().unwrap());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(
        xattr::get(&output, "user.chatter-test").unwrap().unwrap(),
        b"retained value"
    );
    assert_eq!(
        xattr::get(&output, "system.posix_acl_access")
            .unwrap()
            .unwrap(),
        acl
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), REFERENCE);
}

#[cfg(windows)]
#[test]
fn windows_replacement_preserves_named_streams_and_timestamps() {
    use std::os::windows::fs::MetadataExt;
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.cha");
    let output = directory.path().join("output.cha");
    let stream = directory.path().join("output.cha:chatter-test");
    fs::write(&input, REFERENCE).unwrap();
    fs::write(&output, "old output").unwrap();
    fs::write(&stream, "retained stream").unwrap();
    let before = fs::metadata(&output).unwrap();
    let result = normalize(&input, &output);
    assert!(result.status.success(), "{result:?}");
    let after = fs::metadata(&output).unwrap();
    assert_eq!(before.creation_time(), after.creation_time());
    assert_eq!(before.last_write_time(), after.last_write_time());
    assert_eq!(before.file_attributes(), after.file_attributes());
    assert_eq!(fs::read_to_string(&output).unwrap(), REFERENCE);
    assert_eq!(fs::read_to_string(&stream).unwrap(), "retained stream");
}
