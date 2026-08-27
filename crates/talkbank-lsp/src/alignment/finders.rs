//! Bridge between tree/model indices and alignment pair arrays.
//!
//! `UtteranceContent` positions in the AST are flat array indices that include
//! non-alignable items (pauses, events, overlap markers). Alignment pairs use a
//! filtered "alignable-only" index space. This module converts between the two
//! so hover and highlight handlers can look up the correct pair given a CST node
//! position.

use talkbank_model::model::UtteranceContent;

/// Return the top-level content item corresponding to `alignment_index`.
///
/// This helper is used for backward links (tier item -> main-tier content).
pub fn get_alignable_content_by_index(
    content: &[UtteranceContent],
    alignment_index: usize,
) -> Option<&UtteranceContent> {
    let mut current_alignment_idx = 0;

    for item in content {
        match item {
            UtteranceContent::Word(_)
            | UtteranceContent::AnnotatedWord(_)
            | UtteranceContent::ReplacedWord(_) => {
                if current_alignment_idx == alignment_index {
                    return Some(item);
                }
                current_alignment_idx += 1;
            }
            UtteranceContent::Group(_) | UtteranceContent::AnnotatedGroup(_) => {
                // Groups count as a single alignable unit for hover purposes.
                if current_alignment_idx == alignment_index {
                    return Some(item);
                }
                current_alignment_idx += 1;
            }
            // Non-alignable items do not advance the index.
            _ => {}
        }
    }

    None
}
