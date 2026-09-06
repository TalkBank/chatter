// Build scripts run at build time, not runtime. Panics here fail
// `cargo build`, which is the intended behaviour for missing files
// or invalid embedded data. Per-crate `deny` panic lints would
// otherwise fire on standard `env::var().unwrap()` and
// `fs::File::create().unwrap()` patterns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Build script for talkbank-model.
//!
//! Generates a compile-time perfect hash set of ISO 639-3 language codes from
//! the derived file at `data/iso639-3.tsv` (committed inside this crate).
//!
//! ## Where the data comes from
//!
//! `data/iso639-3.tsv` is DERIVED from the code tables published by
//! iso639-3.sil.org, the ISO registration authority for ISO 639-3. It carries
//! only the identifiers chatter needs, unmodified, with attribution and the
//! release stamp in its own header; it is not the code tables.
//!
//! It holds three categories, all VALID: currently assigned codes, retired
//! codes (a CHAT file is a historical document, so a transcript must not become
//! invalid when a code is retired later), and the `qaa`..`qtz` block the
//! standard reserves for local use, which appears in no published table.
//!
//! Before 2026-08-11 the vendored list was instead a copy of a third party's
//! copy of the registry, unversioned and stale: it was missing 162 currently
//! assigned codes, which chatter therefore rejected.
//!
//! ## Refreshing it
//!
//! Run the script; never hand-edit the file:
//!
//! ```bash
//! python3 scripts/update_iso639_3.py --release YYYYMMDD \
//!     --out crates/talkbank-model/data/iso639-3.tsv
//! ```
//!
//! There is no automated staleness check. SIL publishes infrequently, and a
//! release that adds codes only ever widens what chatter accepts.
//!
//! The generated file is written to `$OUT_DIR/iso639_3_set.rs` and included
//! by `src/model/header/codes/iso639.rs` at compile time.
//!
//! Also generates a source-behaviour fingerprint of this crate's entire
//! `src/` tree (`MODEL_BEHAVIOR_FINGERPRINT`, written to
//! `$OUT_DIR/model_behavior_fingerprint.rs` and included by
//! `src/errors/codes/rules_fingerprint.rs`). See that module's doc comment
//! for why the fingerprint covers the whole tree rather than only the
//! validation submodule.

use std::env;
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    generate_iso639_3_set();
    generate_model_behavior_fingerprint();
}

/// Hash only this crate's own source, including in a published package.
fn generate_model_behavior_fingerprint() {
    println!("cargo:rerun-if-changed=src");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fingerprint = talkbank_build::SourceFingerprint::read_tree(&root.join("src"))
        .expect("model source fingerprint")
        .to_string();
    let destination =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("model_behavior_fingerprint.rs");
    fs::write(
        destination,
        format!(
            "/// Build-time fingerprint of this crate's complete source tree.\n\
         pub const MODEL_BEHAVIOR_FINGERPRINT: &str = {fingerprint:?};\n"
        ),
    )
    .unwrap();
}

/// Parse the ISO 639-3 registry and generate a `phf::Set<&str>`.
fn generate_iso639_3_set() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = Path::new(&manifest_dir);

    // The vendored copy committed inside this crate is the ONLY source.
    //
    // There used to be a fallback to a `clan-info` checkout beside this repo,
    // and then to an EMPTY set. Both are gone, for the same reason: a build
    // must not silently decide what counts as a real language based on what
    // happens to be on the developer's disk.
    //
    // The empty-set path was the dangerous one, and it did exactly what its own
    // comment said: "language code membership validation will be disabled".
    // `is_valid_iso639_3` opened with a guard returning `true` for EVERY input
    // when the set was empty, so a missing data file did not fail the build and
    // did not reject anything. It silently turned language validation off, and
    // `@Languages: xyzzy` would have passed, on the strength of a
    // `cargo:warning` nobody reads. A missing data file is now a build failure,
    // and the guard that accepted everything is gone with it.
    let iso_path = crate_root.join("data/iso639-3.tsv");
    if !iso_path.exists() {
        panic!(
            "ISO 639-3 data file missing at {}.\n\
             This file is committed to the repository and is the sole source of \
             valid language codes; regenerate it with scripts/update_iso639_3.py or \
             restore it (git checkout -- crates/talkbank-model/data/). Building without \
             it used to silently disable language validation entirely.",
            iso_path.display()
        );
    }

    println!("cargo:rerun-if-changed={}", iso_path.display());

    let content = fs::read_to_string(&iso_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read ISO 639-3 file at {}: {}",
            iso_path.display(),
            e
        )
    });

    // Derived-file format, written by scripts/update_iso639_3.py:
    //   `#` comment lines carrying provenance, then code<TAB>status<TAB>change_to.
    // Every status is a VALID code here. `retired` is deliberate: a CHAT file is
    // a historical document, so a transcript must not become invalid because a
    // code was retired afterwards. `private_use` covers qaa..qtz, which the
    // standard reserves and which therefore appear in no published table.
    let mut codes: Vec<&str> = Vec::with_capacity(9000);
    let mut release = String::new();

    for line in content.lines() {
        if let Some(stamp) = line.strip_prefix("# Release: ") {
            release = stamp.trim().to_owned();
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        // `split_once` is total here: a row with no tab is a bare code, which
        // the shape check below then judges. The previous `split().next()` form
        // needed an unreachable panic arm, because `split` always yields once.
        let code = line.split_once('\t').map_or(line, |(code, _)| code);
        // Fail loudly on a shape we do not recognise rather than skipping it:
        // a silently dropped row is a language chatter would start rejecting.
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_lowercase()) {
            panic!(
                "unexpected language code {code:?} in {}; regenerate it with \
                 scripts/update_iso639_3.py rather than editing it by hand",
                iso_path.display()
            );
        }
        codes.push(code);
    }

    if release.is_empty() {
        panic!(
            "{} carries no `# Release:` line. The release date is the only version \
             these tables have, so a file without one cannot be identified.",
            iso_path.display()
        );
    }

    // Generate the phf set.
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("iso639_3_set.rs");
    let file = fs::File::create(&dest_path).unwrap();
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "/// ISO 639-3 language code set ({} codes).",
        codes.len()
    )
    .unwrap();
    writeln!(
        writer,
        "/// Generated from data/iso639-3.tsv by build.rs.\n\
         /// ISO 639-3 release {release}; source: iso639-3.sil.org."
    )
    .unwrap();

    let mut set = phf_codegen::Set::new();
    for code in &codes {
        set.entry(*code);
    }

    writeln!(
        writer,
        "static ISO_639_3_CODES: phf::Set<&'static str> = {};",
        set.build()
    )
    .unwrap();
}
