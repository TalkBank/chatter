//! Post-parse normalization passes for CHAT line models.
//!
//! These passes rewrite parser output into canonical model forms expected by
//! validators and downstream alignment code.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Shortenings>

use crate::model::{CaOptionEffect, Header, Line, MainTier};

/// Return whether `@Options: CA` reinterprets a standalone parenthetical as a
/// CA omission.
///
/// This module uses it to decide whether to run CA-omission normalization. It
/// is a REINTERPRETATION rather than a leniency waiver: the same bytes mean
/// something different. The flag's other effect, waiving the terminator
/// requirement, is a separate variant, and neither says anything about which
/// notation the file uses.
pub(super) fn headers_enable_ca_mode(headers: &[Header]) -> bool {
    headers.iter().any(|header| {
        matches!(header, Header::Options { options }
            if options
                .iter()
                .any(|opt| opt.has_effect(CaOptionEffect::ParentheticalIsCaOmission)))
    })
}

/// Normalize CA omission shorthand across all utterances when CA mode is enabled.
///
/// Specifically, this pass targets words categorized as `WordCategory::CAOmission` whose content is a
/// standalone `WordContent::Shortening` token (the internal representation for parenthesized omission text).
/// It rewrites that shortening token into plain text so later passes operate on a canonical word shape.
/// Apply the CA-omission canonicalization to a whole file.
///
/// Both this and [`normalize_ca_omissions_main_tier`] are thin delegates: the
/// rule AND the traversal that finds words for it live in `talkbank-model`, so
/// the two parser backends cannot drift apart on either. They did drift, on
/// both, which is why the shared version exists.
pub(super) fn normalize_ca_omissions(lines: &mut [Line]) {
    talkbank_model::model::content::word::ca::normalize_ca_omissions_in_lines(lines);
}

/// The same normalization for one main tier, as the fragment APIs need.
pub(crate) fn normalize_ca_omissions_main_tier(main: &mut MainTier) {
    talkbank_model::model::content::word::ca::normalize_ca_omissions_in_main_tier(main);
}
