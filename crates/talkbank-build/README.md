# talkbank-build

**Last modified:** 2026-09-06 03:03 EDT

Small, standard-library-only build support for deterministic source fingerprints.
Each consumer hashes its own packaged source directory. No repository root,
Git metadata, sibling checkout or runtime dependency is required.

`SourceFingerprint::read_tree` returns an opaque digest only after every source
file has been read. Paths are sorted and relative to the selected directory;
relocating identical bytes leaves the digest unchanged. Filesystem errors and
symbolic links fail the build instead of admitting a partial fingerprint.

The digest uses FNV-1a for cache invalidation, not cryptographic attestation.
Comments and test-only source edits deliberately invalidate too. This does not
fingerprint the compiler, dependency lockfile, enabled features or runtime state.
