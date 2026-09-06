//! Fingerprint this package's Rust parser implementation for cache invalidation.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src");
    let root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing CARGO_MANIFEST_DIR")?,
    );
    let fingerprint = talkbank_build::SourceFingerprint::read_tree(&root.join("src"))?;
    println!("cargo:rustc-env=TALKBANK_PARSER_SOURCE_FINGERPRINT={fingerprint}");
    Ok(())
}
