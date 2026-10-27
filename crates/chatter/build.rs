//! Source-bound diagnostic stamp for the CLI version display.
//!
//! This identifies the CLI source tree and build-script time. It is not an
//! executable checksum, dependency-closure identity, or Git provenance claim.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing CARGO_MANIFEST_DIR")?,
    );
    let source = talkbank_build::SourceFingerprint::read_tree(&root.join("src"))?;
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=BUILD_HASH={version}-source-{source}-{epoch}");
    println!("cargo:rerun-if-changed=src");
    Ok(())
}
