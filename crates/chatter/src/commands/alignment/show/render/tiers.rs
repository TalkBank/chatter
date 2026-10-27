//! Per-tier alignment rendering for `%mor`, `%gra`, `%pho`, and `%sin`.
//!
//! Each `render_*` function prints the aligned pairs for one tier on one
//! utterance, using the extraction helpers from [`crate::commands::alignment::helpers`]
//! to convert model indices to display text. Unresolved indices show `???`.
//! All renderers return `(shown, errors)` counts for the orchestrator's totals.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::super::AlignmentView;
use std::marker::PhantomData;
use talkbank_model::ParseError;
use talkbank_model::alignment::helpers::{PositionalDomain, TierPosition, collect_tier_items};
use talkbank_model::alignment::{
    AlignmentPair, GraIndex, MainWordIndex, MorChunkIndex, MorItemIndex, PhoItemIndex, SinItemIndex,
};
use talkbank_model::model::WriteChat;
use talkbank_model::{AlignmentSet, Utterance};

use crate::commands::alignment::helpers::{
    get_gra_relation_text, get_mor_item_text, get_pho_form_text, get_sin_item_text,
};

/// Render main-to-%mor alignments for one utterance.
pub(super) fn render_main_to_mor(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    view: AlignmentView,
) -> crate::exit_codes::CommandResult<(usize, usize)> {
    let main_to_mor = match &alignments.mor {
        Some(alignment) => alignment,
        None => return Ok((0, 0)),
    };

    let main = MainDisplay::<MorItemIndex>::new(&utterance.main);

    if let AlignmentView::Compact = view {
        out!("Utt#{} Main→Mor: ", utterance_index + 1);
        for (i, pair) in main_to_mor.pairs.iter().enumerate() {
            if i > 0 {
                out!(", ");
            }
            let main_text = main.for_pair(pair);
            let mor_text = mor_text(utterance, pair.target_index);
            out!("\"{}\" → \"{}\"", main_text, mor_text);
        }
        outln!();
    } else {
        outln!("Main → %mor alignment:");
        for pair in &main_to_mor.pairs {
            let main_text = main.for_pair(pair);
            let mor_text = mor_text(utterance, pair.target_index);
            outln!("  \"{}\" → \"{}\"", main_text, mor_text);
        }
    }

    let errors = render_errors(&main_to_mor.errors, view)?;
    Ok((1, errors))
}

/// Render %mor-to-%gra alignments for one utterance.
pub(super) fn render_mor_to_gra(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    view: AlignmentView,
) -> crate::exit_codes::CommandResult<(usize, usize)> {
    let mor_to_gra = match &alignments.gra {
        Some(alignment) => alignment,
        None => return Ok((0, 0)),
    };

    if let AlignmentView::Compact = view {
        out!("Utt#{} Mor→Gra: ", utterance_index + 1);
        for (i, pair) in mor_to_gra.pairs.iter().enumerate() {
            if i > 0 {
                out!(", ");
            }
            let mor_text = mor_chunk_text(utterance, pair.mor_chunk_index);
            let gra_text = gra_text(utterance, pair.gra_index);
            out!("\"{}\" → \"{}\"", mor_text, gra_text);
        }
        outln!();
    } else {
        outln!("%mor → %gra alignment:");
        for pair in &mor_to_gra.pairs {
            let mor_text = mor_chunk_text(utterance, pair.mor_chunk_index);
            let gra_text = gra_text(utterance, pair.gra_index);
            outln!("  \"{}\" → \"{}\"", mor_text, gra_text);
        }
    }

    let errors = render_errors(&mor_to_gra.errors, view)?;
    Ok((1, errors))
}

/// Render main-to-%pho alignments for one utterance.
pub(super) fn render_main_to_pho(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    view: AlignmentView,
) -> crate::exit_codes::CommandResult<(usize, usize)> {
    let main_to_pho = match &alignments.pho {
        Some(alignment) => alignment,
        None => return Ok((0, 0)),
    };

    let main = MainDisplay::<PhoItemIndex>::new(&utterance.main);

    if let AlignmentView::Compact = view {
        out!("Utt#{} Main→Pho: ", utterance_index + 1);
        for (i, pair) in main_to_pho.pairs.iter().enumerate() {
            if i > 0 {
                out!(", ");
            }
            let main_text = main.for_pair(pair);
            let pho_text = pho_text(utterance, pair.target_index);
            out!("\"{}\" → \"{}\"", main_text, pho_text);
        }
        outln!();
    } else {
        outln!("Main → %pho alignment:");
        for pair in &main_to_pho.pairs {
            let main_text = main.for_pair(pair);
            let pho_text = pho_text(utterance, pair.target_index);
            outln!("  \"{}\" → \"{}\"", main_text, pho_text);
        }
    }

    let errors = render_errors(&main_to_pho.errors, view)?;
    Ok((1, errors))
}

/// Render main-to-%sin alignments for one utterance.
pub(super) fn render_main_to_sin(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    view: AlignmentView,
) -> crate::exit_codes::CommandResult<(usize, usize)> {
    let main_to_sin = match &alignments.sin {
        Some(alignment) => alignment,
        None => return Ok((0, 0)),
    };

    let main = MainDisplay::<SinItemIndex>::new(&utterance.main);

    if let AlignmentView::Compact = view {
        out!("Utt#{} Main→Sin: ", utterance_index + 1);
        for (i, pair) in main_to_sin.pairs.iter().enumerate() {
            if i > 0 {
                out!(", ");
            }
            let main_text = main.for_pair(pair);
            let sin_text = sin_text(utterance, pair.target_index);
            out!("\"{}\" → \"{}\"", main_text, sin_text);
        }
        outln!();
    } else {
        outln!("Main → %sin alignment:");
        for pair in &main_to_sin.pairs {
            let main_text = main.for_pair(pair);
            let sin_text = sin_text(utterance, pair.target_index);
            outln!("  \"{}\" → \"{}\"", main_text, sin_text);
        }
    }

    let errors = render_errors(&main_to_sin.errors, view)?;
    Ok((1, errors))
}

/// The target index selects the shared main-tier projection domain.
trait MainDomain {
    const DOMAIN: PositionalDomain;
}
impl MainDomain for MorItemIndex {
    const DOMAIN: PositionalDomain = PositionalDomain::Mor;
}
impl MainDomain for PhoItemIndex {
    const DOMAIN: PositionalDomain = PositionalDomain::Pho;
}
impl MainDomain for SinItemIndex {
    const DOMAIN: PositionalDomain = PositionalDomain::Sin;
}

/// A display sequence can only consume alignment pairs from its own domain.
struct MainDisplay<T> {
    positions: Vec<TierPosition>,
    domain: PhantomData<T>,
}
impl<T: MainDomain> MainDisplay<T> {
    fn new(main: &talkbank_model::MainTier) -> Self {
        Self {
            positions: collect_tier_items(&main.content.content, T::DOMAIN),
            domain: PhantomData,
        }
    }
    fn for_pair(&self, pair: &AlignmentPair<MainWordIndex, T>) -> String {
        pair.source_index
            .and_then(|index| self.positions.get(index.as_usize()))
            .map(|position| position.text.clone())
            .unwrap_or_else(|| "???".to_string())
    }
}

/// GRA indices address MOR chunks, including clitics and the terminator.
fn mor_chunk_text(utterance: &Utterance, index: Option<MorChunkIndex>) -> String {
    index
        .and_then(|index| utterance.mor_tier()?.chunk_at(index.as_usize()))
        .map(|chunk| match chunk {
            talkbank_model::model::MorChunk::Main(item) => item.main.to_chat_string(),
            talkbank_model::model::MorChunk::PostClitic(_, word) => word.to_chat_string(),
            talkbank_model::model::MorChunk::Terminator(terminator) => terminator.to_chat_string(),
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %mor item for an alignment index (or `???` when missing).
fn mor_text(utterance: &Utterance, index: Option<MorItemIndex>) -> String {
    index
        .and_then(|idx| {
            utterance
                .mor_tier()
                .and_then(|m| get_mor_item_text(m, idx.as_usize()))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %gra relation for an alignment index (or `???` when missing).
fn gra_text(utterance: &Utterance, index: Option<GraIndex>) -> String {
    index
        .and_then(|idx| {
            utterance
                .gra_tier()
                .and_then(|g| get_gra_relation_text(g, idx.as_usize()))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %pho item for an alignment index (or `???` when missing).
fn pho_text(utterance: &Utterance, index: Option<PhoItemIndex>) -> String {
    index
        .and_then(|idx| {
            utterance
                .pho()
                .as_ref()
                .and_then(|p| get_pho_form_text(p, idx.as_usize()))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %sin item for an alignment index (or `???` when missing).
fn sin_text(utterance: &Utterance, index: Option<SinItemIndex>) -> String {
    index
        .and_then(|idx| {
            utterance
                .sin()
                .as_ref()
                .and_then(|s| get_sin_item_text(s, idx.as_usize()))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Print alignment errors for a tier pair and return the count.
fn render_errors(
    errors: &[ParseError],
    view: AlignmentView,
) -> crate::exit_codes::CommandResult<usize> {
    if errors.is_empty() {
        return Ok(0);
    }

    if let AlignmentView::Compact = view {
        outln!("  ⚠ {} error(s)", errors.len());
    } else {
        outln!("  Errors:");
        for err in errors {
            outln!("    • {}", err.message);
        }
    }

    Ok(errors.len())
}
