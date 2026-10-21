//! Exercise the source-only executable on absent, changed and rejected inputs.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::Result;

fn generate(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_model_sources_gen"))
        .arg("--repo-root")
        .arg(root)
        .output()
        .expect("launch source generator")
}

fn registry(summary: &str) -> String {
    format!(
        "[[code]]\ncode = 'E999'\nvariant = 'BootstrapExample'\n\
         summary = '{summary}'\nkind = 'Invalidity'\nstatus = 'implemented'\n"
    )
}

#[test]
fn absent_model_sources_bootstrap_then_follow_changed_input_without_rewriting_unchanged_files()
-> Result<()> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path();
    fs::create_dir_all(root.join("spec/errors"))?;
    fs::create_dir_all(root.join("spec/codes"))?;
    let errors = root.join("crates/talkbank-model/src/errors");
    fs::create_dir_all(&errors)?;
    fs::write(root.join("Cargo.toml"), "# isolated artifact fixture\n")?;
    fs::write(errors.join("handwritten.rs"), "preserve this owner\n")?;
    let input = root.join("spec/codes/error-codes.toml");
    fs::write(&input, registry("Initial bootstrap description."))?;
    let first = generate(root);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let codes = errors.join("codes/generated_error_code.rs");
    let kinds = errors.join("generated_diagnostic_kind.rs");
    assert!(fs::read_to_string(&codes)?.contains("Initial bootstrap description."));
    assert!(fs::read_to_string(&kinds)?.contains("ErrorCode::BootstrapExample"));

    let bytes = fs::read(&codes)?;
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(&codes)?
        .set_times(fs::FileTimes::new().set_modified(stamp))?;
    let second = generate(root);
    assert!(second.status.success());
    assert_eq!(fs::read(&codes)?, bytes);
    assert_eq!(fs::metadata(&codes)?.modified()?, stamp);

    fs::write(&input, registry("Changed bootstrap description."))?;
    let changed = generate(root);
    assert!(changed.status.success());
    let new_bytes = fs::read(&codes)?;
    assert_ne!(new_bytes, bytes);
    assert!(String::from_utf8_lossy(&new_bytes).contains("Changed bootstrap description."));
    assert_eq!(
        fs::read_to_string(errors.join("handwritten.rs"))?,
        "preserve this owner\n"
    );

    fs::write(&input, "invalid registry")?;
    assert!(!generate(root).status.success());
    assert_eq!(
        fs::read(&codes)?,
        new_bytes,
        "rejected inputs must not replace admitted output"
    );
    Ok(())
}
