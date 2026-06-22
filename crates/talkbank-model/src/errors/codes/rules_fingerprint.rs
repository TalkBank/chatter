//! Stable fingerprint of the active validation rule set.
//!
//! # Why this exists
//!
//! The validation-result cache (`talkbank-cache`) stores a pass/fail verdict
//! keyed on file content. For that cache to be sound, its key must also
//! capture *which validation rules produced the verdict*. Otherwise, when the
//! rule set changes (a new code such as E370 "retrace marker must be followed
//! by material" is added), a stale "Valid" entry from the old rules keeps being
//! served, silently undermining `chatter validate` (the authority on CHAT
//! validity). See the regression test
//! `talkbank-cache/tests/rules_version_invalidation.rs`.
//!
//! [`validation_rules_fingerprint`] derives a stable identifier from the full
//! set of [`ErrorCode`] variants the validator can emit. Adding, removing, or
//! renaming any code changes the fingerprint, which changes the cache key,
//! which invalidates prior entries. This is the single source of truth for
//! "what rule set is active": the codes ARE the enumeration of checks.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use super::error_code::ErrorCode;

/// FNV-1a 64-bit offset basis.
///
/// We use FNV-1a rather than `std::hash::DefaultHasher` because the latter's
/// output is explicitly NOT stable across Rust versions or process runs in the
/// general case; a cache-compatibility fingerprint must be reproducible so that
/// two builds of chatter with the *same* rule set agree on the same key.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A unit separator byte mixed between codes so that the boundary between two
/// adjacent codes is itself part of the hashed stream. Without it, the streams
/// `["E1", "23"]` and `["E12", "3"]` would hash identically; with it they do
/// not. Value `0x1F` is ASCII Unit Separator, which never appears in a code.
const CODE_SEPARATOR: u8 = 0x1F;

/// Fold one byte into an FNV-1a accumulator.
const fn fnv1a_byte(mut hash: u64, byte: u8) -> u64 {
    hash ^= byte as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash
}

/// Compute a stable fingerprint of the entire validation rule set.
///
/// The fingerprint is a lowercase hex string of an FNV-1a hash over every
/// [`ErrorCode`]'s canonical short code (e.g. `"E370"`), in declaration order,
/// each followed by a [`CODE_SEPARATOR`]. Declaration order is fixed by the
/// `ErrorCode` enum, so the same rule set always yields the same fingerprint,
/// and any edit to the code list (new variant, removed variant, renamed code
/// string) changes it.
///
/// This is intentionally cheap and allocation-light: it walks the `'static`
/// slice from [`ErrorCode::all`] and returns a short hex token suitable for
/// folding into a cache-compatibility version. It is NOT a cryptographic hash;
/// it only needs to change reliably when the rule set changes, not to resist
/// adversarial collision.
pub fn validation_rules_fingerprint() -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for code in ErrorCode::all() {
        for byte in code.as_str().bytes() {
            hash = fnv1a_byte(hash, byte);
        }
        hash = fnv1a_byte(hash, CODE_SEPARATOR);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fingerprint is deterministic: the same rule set hashes identically
    /// every time it is computed.
    #[test]
    fn fingerprint_is_deterministic() {
        assert_eq!(
            validation_rules_fingerprint(),
            validation_rules_fingerprint(),
            "fingerprint must be stable across calls within a build"
        );
    }

    /// The fingerprint actually depends on the code set: hashing a list that
    /// drops a code (simulating "before E370 was added") must differ from the
    /// real fingerprint. This is the property the cache relies on.
    #[test]
    fn fingerprint_changes_when_a_code_is_added_or_removed() {
        // Recompute the fingerprint over the full set, but skip E370. If the
        // hash ignored the code list this would equal the real fingerprint.
        let mut hash = FNV_OFFSET_BASIS;
        for code in ErrorCode::all() {
            if *code == ErrorCode::StructuralOrderError {
                continue; // pretend E370 does not exist yet
            }
            for byte in code.as_str().bytes() {
                hash = fnv1a_byte(hash, byte);
            }
            hash = fnv1a_byte(hash, CODE_SEPARATOR);
        }
        let without_e370 = format!("{hash:016x}");

        assert_ne!(
            without_e370,
            validation_rules_fingerprint(),
            "removing a validation code must change the rules fingerprint"
        );
    }
}
