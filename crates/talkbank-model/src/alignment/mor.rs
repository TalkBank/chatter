//! Main-tier to `%mor` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::format::format_positional_mismatch;
use super::helpers::{
    MorAlignableWordCount, MorItemCount, PositionalDomain, TierPosition, collect_tier_items,
    count_tier_positions, to_chat_display_string as to_string,
};
use super::indices::{MainWordIndex, MorItemIndex};
use super::types::AlignmentPair;
use crate::model::{MainTier, MorTier, WriteChat};
use crate::{ErrorCode, ParseError, Severity};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for the main↔`%mor` alignment.
///
/// Target is a `%mor` **item** position ([`MorItemIndex`]), not a
/// `%mor` **chunk** position, the compiler rejects mixing with
/// [`MorChunkIndex`](crate::alignment::indices::MorChunkIndex), which
/// lives in `%gra` space and is a structurally different sequence
/// (items plus post-clitics plus terminator).
pub type MorAlignmentPair = AlignmentPair<MainWordIndex, MorItemIndex>;

/// Result of aligning main tier words to %mor tier morphological items.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct MorAlignment {
    /// Alignment pairs
    /// ([`MainWordIndex`]↔[`MorItemIndex`](crate::alignment::indices::MorItemIndex)).
    ///
    /// Indices are positions in the alignable content sequence.
    /// `None` in either position indicates a placeholder due to misalignment.
    pub pairs: Vec<MorAlignmentPair>,

    /// Errors produced while checking `%mor` alignment invariants.
    ///
    /// Includes count mismatches and terminator value mismatches.
    pub errors: Vec<ParseError>,
}

impl MorAlignment {
    /// Create an empty alignment with no pairs or diagnostics.
    ///
    /// Useful as a neutral accumulator in builder-style alignment assembly.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Append one index-pair mapping to the alignment result.
    pub fn with_pair(mut self, pair: MorAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Append one diagnostic to the alignment result.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Return `true` when the alignment contains no diagnostics.
    ///
    /// Callers typically use this as a fast pass/fail check after alignment.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for MorAlignment {
    /// Builds an empty main-to-`%mor` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl super::traits::TierAlignmentResult for MorAlignment {
    type Pair = MorAlignmentPair;

    fn pairs(&self) -> &[MorAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: MorAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

/// Align main tier to %mor tier.
///
/// Validates 1-1 alignment between main tier alignable words and %mor items,
/// including terminator presence and value matching.
/// The result includes placeholder pairs (`None` on one side) when lengths
/// differ so UI tools can still render positional mismatch tables.
pub fn align_main_to_mor(main: &MainTier, mor: &MorTier) -> MorAlignment {
    let mut alignment = MorAlignment::new();

    // Main-tier absence is permitted in CA mode; MOR always owns a terminator.
    // Only two present, unequal spellings admit this diagnostic and its preview.
    if let Some(mismatch) = TerminatorMismatch::between(main, mor) {
        alignment = alignment.with_error(mismatch.into_error());
    }

    // Extract alignable content indices from main tier
    let alignable_count = MorAlignableWordCount::new(count_tier_positions(
        &main.content.content,
        PositionalDomain::Mor,
    ));

    append_position_alignment(
        &mut alignment,
        main,
        mor,
        alignable_count,
        MorItemCount::new(mor.items.len()),
    );
    alignment
}

/// Keep expected main-tier positions distinct from observed morphology items.
/// A missing tier is not admitted here: the caller must supply an actual tier.
fn append_position_alignment(
    alignment: &mut MorAlignment,
    main: &MainTier,
    mor: &MorTier,
    expected: MorAlignableWordCount,
    observed: MorItemCount,
) {
    // Terminator is now a separate field, not counted in items
    let expected_mor_count = expected.get();

    let mor_count = observed.get();

    // Preserve the common prefix and represent excess positions on either side.
    for i in 0..expected_mor_count.max(mor_count) {
        alignment.pairs.push(AlignmentPair::new(
            (i < expected_mor_count).then(|| MainWordIndex::new(i)),
            (i < mor_count).then(|| MorItemIndex::new(i)),
        ));
    }

    let (code, suggestion) = match expected_mor_count.cmp(&mor_count) {
        std::cmp::Ordering::Equal => return,
        std::cmp::Ordering::Greater => (
            ErrorCode::MorCountMismatchTooFew,
            "Each alignable word in main tier must have corresponding %mor item",
        ),
        std::cmp::Ordering::Less => (
            ErrorCode::MorCountMismatchTooMany,
            "Remove extra %mor items or add corresponding words to main tier",
        ),
    };
    let main_items = collect_tier_items(&main.content.content, PositionalDomain::Mor);
    let mor_items: Vec<TierPosition> = mor
        .items
        .iter()
        .map(|item| TierPosition {
            text: to_string(item),
            description: None,
        })
        .collect();

    let detailed_message =
        format_positional_mismatch("Main tier", "%mor tier", &main_items, &mor_items);

    let error = ParseError::at_location(code, Severity::Error, main.location, detailed_message)
        .with_label_at(main.location, "Main tier")
        .with_label_at(mor.location, "%mor tier")
        .with_suggestion(suggestion);

    alignment.errors.push(error);
}

/// Present, unequal terminators bound to the tiers that produced them.
/// Lexical positions and terminator spellings remain separate through rendering.
struct TerminatorMismatch<'a> {
    main: &'a MainTier,
    mor: &'a MorTier,
    main_spelling: String,
    mor_spelling: String,
}

impl<'a> TerminatorMismatch<'a> {
    fn between(main: &'a MainTier, mor: &'a MorTier) -> Option<Self> {
        let main_spelling = main.content.terminator.as_ref()?.to_chat_string();
        let mor_spelling = mor.terminator.to_chat_string();
        (main_spelling != mor_spelling).then_some(Self {
            main,
            mor,
            main_spelling,
            mor_spelling,
        })
    }

    fn into_error(self) -> ParseError {
        let suggestion = format!(
            "The %mor tier terminator must match the main tier terminator exactly\n\nAlignment preview:\n{}",
            self.preview(),
        );
        ParseError::at_location(
            ErrorCode::MorTerminatorValueMismatch,
            Severity::Error,
            self.mor.location,
            format!(
                "Main tier terminator \"{}\" does not match %mor terminator \"{}\"",
                self.main_spelling, self.mor_spelling,
            ),
        )
        .with_suggestion(suggestion)
        .with_label_at(self.main.location, "Main tier")
        .with_label_at(self.mor.location, "%mor tier")
    }

    fn preview(&self) -> String {
        let main_items = collect_tier_items(&self.main.content.content, PositionalDomain::Mor);
        let mut out = String::from("Alignment (index: main | mor)\n");
        for idx in 0..main_items.len().max(self.mor.items.len()) {
            let index = idx.to_string();
            for _ in index.len()..3 {
                out.push(' ');
            }
            out.push_str(&index);
            out.push_str(": ");
            out.push_str(match main_items.get(idx) {
                Some(item) => &item.text,
                None => "<none>",
            });
            out.push_str(" | ");
            match self.mor.items.get(idx) {
                Some(item) => out.push_str(&to_string(item)),
                None => out.push_str("<none>"),
            }
            out.push('\n');
        }
        out.push_str("end: ");
        out.push_str(&self.main_spelling);
        out.push_str(" | ");
        out.push_str(&self.mor_spelling);
        out.push('\n');
        out
    }
}
