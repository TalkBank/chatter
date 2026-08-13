//! ISO 639-3 language code lookup.
//!
//! A compile-time perfect hash set built by `build.rs` from
//! `data/iso639-3.tsv`, which is derived from the code tables published by
//! iso639-3.sil.org, the ISO registration authority for the standard. It holds
//! every currently assigned code, every retired one, and the `qaa`..`qtz` block
//! the standard reserves for local use; `scripts/update_iso639_3.py` explains
//! why each category is in there.
//!
//! Used by `LanguageCode::validate()` to check membership.

// Include the generated phf::Set from build.rs.
include!(concat!(env!("OUT_DIR"), "/iso639_3_set.rs"));

/// Check whether a 3-letter code is a valid ISO 639-3 language code.
///
/// There is no empty-set case to handle. This used to begin with a check that
/// returned `true` for every input when the set was empty, described as
/// graceful degradation, and it was worse than it sounds: a missing data file
/// turned language validation OFF entirely while the build reported success
/// with a `cargo:warning` nobody reads, so `@Languages: xyzzy` would have
/// passed. `build.rs` now fails the build when the file is absent, which makes
/// the empty set unreachable, so the branch that silently accepted everything
/// is gone rather than left as a comment warning about itself.
pub fn is_valid_iso639_3(code: &str) -> bool {
    ISO_639_3_CODES.contains(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_codes_are_valid() {
        assert!(is_valid_iso639_3("eng"));
        assert!(is_valid_iso639_3("spa"));
        assert!(is_valid_iso639_3("zho"));
        assert!(is_valid_iso639_3("fra"));
        assert!(is_valid_iso639_3("deu"));
        assert!(is_valid_iso639_3("jpn"));
        assert!(is_valid_iso639_3("yue")); // Cantonese
        assert!(is_valid_iso639_3("cym")); // Welsh
    }

    #[test]
    fn invalid_codes_are_rejected() {
        assert!(!is_valid_iso639_3("cye")); // not in ISO 639-3
        assert!(!is_valid_iso639_3("jjj")); // not in ISO 639-3
        assert!(!is_valid_iso639_3("zzz")); // not a real code
        assert!(!is_valid_iso639_3("xyz")); // placeholder
    }

    /// POLICY, not an invariant a type could carry: a retired code stays valid.
    ///
    /// A CHAT file is a historical document. A transcript recorded in 2005 and
    /// tagged `tze` (Chenalhó Tzotzil) must not become invalid because SIL
    /// merged that code into `tzo` in 2009. Rejecting it would invalidate the
    /// past every time the registry moves.
    ///
    /// This test replaced an assertion that `tze` was INVALID, which had held
    /// only because the previous code list was a stale third-party copy that
    /// happened to omit it. The premise was false: `tze` is a real ISO 639-3
    /// code with a recorded retirement date and replacement.
    #[test]
    fn retired_codes_remain_valid() {
        assert!(is_valid_iso639_3("tze")); // retired 2009-01-16, change_to tzo
        assert!(is_valid_iso639_3("fri")); // retired 2005-11-16, change_to fry
    }

    /// POLICY: the private-use range the standard reserves is accepted.
    ///
    /// `qaa` through `qtz` appear in no published table, because they are
    /// reserved rather than assigned, so they are generated into the derived
    /// file. Dropping them would reject legitimate local codes.
    #[test]
    fn private_use_range_is_valid() {
        assert!(is_valid_iso639_3("qaa"));
        assert!(is_valid_iso639_3("qtz"));
        // Past the reserved block AND unassigned. `qua` would be the obvious
        // choice and is wrong: it is Quapaw, a real assigned code, which is
        // why this assertion is pinned to one verified against the tables.
        assert!(!is_valid_iso639_3("quo"));
    }

    #[test]
    fn technically_valid_but_suspicious_codes() {
        // These ARE in ISO 639-3 but are unlikely in TalkBank context.
        // They may be typos for common codes but are not our job to reject.
        assert!(is_valid_iso639_3("nle")); // East Nyala (probably meant nld)
        assert!(is_valid_iso639_3("enh")); // Tundra Enets (probably meant eng)
        assert!(is_valid_iso639_3("ena")); // Apali (probably meant eng)
    }

    /// MEASUREMENT: the set is the size the derived file says it is.
    ///
    /// A range rather than an exact number, because the count moves with each
    /// SIL release. It is a floor against the set silently collapsing, which is
    /// the failure this module used to hide.
    #[test]
    fn set_has_expected_size() {
        let count = ISO_639_3_CODES.len();
        assert!(
            (8500..10_000).contains(&count),
            "expected the ISO 639-3 set to hold roughly 8,800 codes \
             (current + retired + the reserved qaa..qtz block); got {count}"
        );
    }
}
