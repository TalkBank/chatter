//! The byte span of a CST node.
//!
//! One conversion, one name, one home. Every parser region needs the byte range
//! of a `tree_sitter::Node` in order to attach a [`Span`] to the model value it
//! builds, so before this module existed the same two-line conversion had grown
//! three different names in three different places:
//!
//! - `span_of` in `tree_parsing::main_tier::structure::terminator`
//! - `tier_span` in `tier_parsers::text::helpers`
//! - `node_span` in `chat_file_parser::dependent_tier_dispatch::parsed`
//!
//! None was reachable from the others by name, so each new caller either
//! searched, guessed, or wrote a fourth copy. The cost is not the duplicated
//! arithmetic (it is two field reads) but that a reader cannot tell whether the
//! three spellings mean the same thing, and a change to how spans are derived
//! would have to find all of them.
//!
//! Roughly 55 call sites still inline `Span::new(node.start_byte() as u32, ...)`
//! directly. Those are correct and are not a bug; they simply predate this
//! module. Prefer [`span_of`] in new code, and convert the inline ones when you
//! are already editing the surrounding region.

use talkbank_model::Span;
use tree_sitter::Node;

/// Byte span covering `node`, from its start byte to its end byte.
///
/// The offsets come straight from tree-sitter, which reports byte offsets into
/// the same source text the model's spans index, so no adjustment is needed.
#[inline]
pub(crate) fn span_of(node: Node<'_>) -> Span {
    Span::from_usize(node.start_byte(), node.end_byte())
}
