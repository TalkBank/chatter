//! Helper functions for tree-sitter parser
//!
//! This module contains utility functions extracted from the main parser
//! to keep individual files manageable:
//! - `error_checking` - Recursive error checking in parse trees
//! - `error_analysis` - Analysis and classification of ERROR nodes
//! - `node_dispatch` - Node kind dispatch helpers (separators, CA elements)
//! - `supertypes` - Supertype checking for grammar supertypes
//! - `cst_assertions` - CST structure validation (REQUIRED for robustness)
//! - `marked_token` - The text of a token past the marker its rule guarantees
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod cst_assertions;
pub(crate) mod error_analysis;
pub(crate) mod error_checking;
mod header_slots;
mod marked_token;
pub(crate) mod node_dispatch;
pub(crate) mod supertypes;

// Re-export commonly used functions
#[allow(unused_imports)]
pub(crate) use cst_assertions::{
    SlotState, assert_child_count_exact, assert_child_kind, check_not_missing, expect_child,
    expect_delimiter, expect_present, expect_structure, extract_utf8_text, find_child_by_kind,
    present,
};
pub(crate) use error_analysis::{analyze_dependent_tier_error, analyze_error_node};
pub(crate) use error_checking::{
    check_for_errors_recursive_with_context, collect_recovery_nodes, surface_displaced,
};
pub(crate) use header_slots::{
    ContentSlot, HeaderSite, Refused, read_simple_content, unknown_header_from_node,
};
pub(crate) use marked_token::after_marker;
// `parse_separator_node` is deliberately NOT re-exported. Removing the
// unreachable `separator` arm from `base/mod.rs` on 2026-08-20 revealed it has
// no production caller and had not had one: `base_content_item` has no
// `separator` alternative, so that arm was dead and was hiding the fact. It
// still has its own tests in `node_dispatch::separator`; deleting it is a
// separate decision from this one.
// The CA helpers dispatch through the GENERATED `from_char` tables. The word
// converter is their caller: until 2026-09-09 it carried its own copies of
// both tables while this file said the helpers had been removed, and the
// module holding them was not even declared, so the generated tables had no
// caller and the hand-written copies had no check.
pub(crate) use node_dispatch::{
    parse_ca_delimiter_node, parse_ca_element_node, parse_pause_node, parse_separator_like,
};
#[allow(unused_imports)]
pub(crate) use supertypes::{is_header, is_linker, is_pre_begin_header};
