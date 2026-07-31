// Build scripts run at build time, not runtime. Panics here fail
// `cargo build`, which is the intended behaviour for missing files
// or invalid embedded data. Per-crate `deny` panic lints would
// otherwise fire on standard `env::var().unwrap()` and
// `fs::File::create().unwrap()` patterns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Build script for talkbank-model.
//!
//! Generates a compile-time perfect hash set of ISO 639-3 language codes from
//! the vendored registry at `data/iso639-3.txt` (committed inside this crate).
//!
//! The ISO 639-3 data file was extracted from `clan-info/lib/fixes/ISO 639-3.txt`
//! and vendored into this crate so CI and fresh clones always have it without
//! needing to clone the private `clan-info` submodule.
//!
//! ## Syncing the vendored list
//!
//! The ISO 639-3 standard is updated infrequently (new codes are occasionally
//! added for newly-documented languages; retired codes are deprecated but kept).
//! When the master list in `clan-info/lib/fixes/ISO 639-3.txt` is updated,
//! sync `data/iso639-3.txt` manually:
//!
//! ```bash
//! cp clan-info/lib/fixes/ISO\ 639-3.txt chatter/crates/talkbank-model/data/iso639-3.txt
//! ```
//!
//! There is no automated check for this, syncing is a periodic maintenance
//! task, not a CI gate.
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

/// FNV-1a 64-bit offset basis.
///
/// Duplicated from `src/errors/codes/rules_fingerprint.rs` rather than
/// shared: a build script cannot depend on the crate it builds (that would
/// be circular), and this constant is a two-line algorithm, not a module
/// worth a separate build-dependency crate.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime. See [`FNV_OFFSET_BASIS`] for why this is duplicated.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Unit separator byte, matching the discipline in `rules_fingerprint.rs`:
/// mixed in after every hashed component (a path, a file's contents) so
/// that two different splits of the same byte stream cannot collide. ASCII
/// Unit Separator (`0x1F`) never appears in a source path.
const UNIT_SEPARATOR: u8 = 0x1F;

/// Fold one byte into an FNV-1a accumulator.
const fn fnv1a_byte(mut hash: u64, byte: u8) -> u64 {
    hash ^= byte as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash
}

/// Fold every byte of a slice into an FNV-1a accumulator.
fn fnv1a_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = fnv1a_byte(hash, *byte);
    }
    hash
}

/// Recursively collect every regular file under `dir`, as paths relative to
/// `root`, using forward slashes regardless of host OS so the fingerprint
/// does not depend on which platform built it.
///
/// Panics on I/O failure: build scripts run at build time, not runtime (see
/// the crate-level `#![allow]` above), and a crate whose own `src/` tree
/// cannot be read is not buildable anyway.
fn collect_source_files_relative(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read directory {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| {
            panic!(
                "failed to read directory entry under {}: {e}",
                dir.display()
            )
        });
        let path = entry.path();
        if path.is_dir() {
            collect_source_files_relative(&path, root, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap_or_else(|e| {
                    panic!(
                        "{} is not under crate root {}: {e}",
                        path.display(),
                        root.display()
                    )
                })
                .to_str()
                .unwrap_or_else(|| panic!("non-UTF-8 source path: {}", path.display()))
                .replace(std::path::MAIN_SEPARATOR, "/");
            out.push(relative);
        }
    }
}

/// Generate a behaviour fingerprint of this crate's entire `src/` tree.
///
/// Walks `src/` in sorted (relative-path) order and hashes, for every file,
/// its relative path followed by its bytes, each followed by a
/// [`UNIT_SEPARATOR`]. Sorted order and relative (not absolute) paths make
/// the result independent of directory iteration order and of where the
/// repository is checked out, so the same source tree fingerprints
/// identically on every machine.
///
/// This intentionally hashes the *whole* crate, not just the validation
/// submodule: see `src/errors/codes/rules_fingerprint.rs` for why splitting
/// "validation behaviour" from "serialization behaviour" would be a false
/// precision here, and why over-invalidating is the deliberate, honest
/// choice.
fn generate_model_behavior_fingerprint() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = Path::new(&manifest_dir);
    let src_dir = crate_root.join("src");

    // Recomputed whenever ANY file under src/ changes (added, removed, or
    // edited), not merely the files enumerated by a previous run: a
    // directory-level watch, not a per-file one.
    println!("cargo:rerun-if-changed=src");

    let mut relative_paths: Vec<String> = Vec::new();
    collect_source_files_relative(&src_dir, &src_dir, &mut relative_paths);
    relative_paths.sort();

    let mut hash = FNV_OFFSET_BASIS;
    for relative_path in &relative_paths {
        hash = fnv1a_bytes(hash, relative_path.as_bytes());
        hash = fnv1a_byte(hash, UNIT_SEPARATOR);

        let full_path = src_dir.join(relative_path);
        let content = fs::read(&full_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", full_path.display()));
        hash = fnv1a_bytes(hash, &content);
        hash = fnv1a_byte(hash, UNIT_SEPARATOR);
    }

    let fingerprint = format!("{hash:016x}");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("model_behavior_fingerprint.rs");
    let mut file = fs::File::create(&dest_path).unwrap();
    writeln!(
        file,
        "/// Source-behaviour fingerprint of this crate's `src/` tree at build \
         time, over {} files. Generated by build.rs; see its \
         `generate_model_behavior_fingerprint` doc comment.\n\
         pub const MODEL_BEHAVIOR_FINGERPRINT: &str = {fingerprint:?};",
        relative_paths.len()
    )
    .unwrap();
}

/// Parse the ISO 639-3 registry and generate a `phf::Set<&str>`.
fn generate_iso639_3_set() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = Path::new(&manifest_dir);

    // Primary: vendored copy committed inside this crate (data/iso639-3.txt).
    // Always present in CI and fresh clones, no external submodule needed.
    let vendored = crate_root.join("data/iso639-3.txt");

    // Fallback: a `clan-info` repo cloned as a sibling of this repository.
    // Used when a developer clones clan-info alongside chatter.
    let clan_info_path = crate_root
        .parent() // crates/
        .and_then(|p| p.parent()) // chatter/
        .and_then(|p| p.parent()) // parent dir that may hold sibling checkouts
        .map(|workspace| workspace.join("clan-info/lib/fixes/ISO 639-3.txt"));

    let iso_path = if vendored.exists() {
        vendored
    } else if let Some(ref p) = clan_info_path {
        if p.exists() {
            p.clone()
        } else {
            // Neither source found, emit an empty set (graceful degradation).
            eprintln!(
                "cargo:warning=ISO 639-3 file not found at data/iso639-3.txt or \
                 clan-info/lib/fixes/. Language code membership validation will be disabled."
            );
            generate_empty_set();
            return;
        }
    } else {
        eprintln!(
            "cargo:warning=ISO 639-3 file not found, generating empty set. \
             Language code membership validation will be disabled."
        );
        generate_empty_set();
        return;
    };

    println!("cargo:rerun-if-changed={}", iso_path.display());

    let content = fs::read_to_string(&iso_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read ISO 639-3 file at {}: {}",
            iso_path.display(),
            e
        )
    });

    let mut codes: Vec<&str> = Vec::with_capacity(8500);

    for line in content.lines() {
        // Format: `aaa\t|...|...|Language Name
        // The backtick prefix + 3-letter code is positions 0..4.
        if line.starts_with('`') && line.len() >= 4 {
            let code = &line[1..4];
            if code.len() == 3 && code.chars().all(|c| c.is_ascii_lowercase()) {
                codes.push(code);
            }
        }
    }

    if codes.is_empty() {
        eprintln!("cargo:warning=No codes parsed from ISO 639-3 file, generating empty set.");
        generate_empty_set();
        return;
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
        "/// Generated from clan-info/lib/fixes/ISO 639-3.txt by build.rs."
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

    eprintln!(
        "cargo:warning=Generated ISO 639-3 set with {} codes",
        codes.len()
    );
}

/// Generate an empty set for environments without the ISO file.
fn generate_empty_set() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("iso639_3_set.rs");
    let mut file = fs::File::create(&dest_path).unwrap();
    writeln!(
        file,
        "/// Empty ISO 639-3 set (file not available at build time).\n\
         static ISO_639_3_CODES: phf::Set<&'static str> = {};",
        phf_codegen::Set::<&str>::new().build()
    )
    .unwrap();
}
