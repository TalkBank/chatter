//! Rendering orchestration with span tracking.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

mod bracketed;
mod utterance;

use super::types::RenderedSpans;
use crate::Span;
use crate::model::MainTier;
use crate::model::WriteChat;
use utterance::render_utterance_content;

/// Render a main tier into CHAT text while capturing retrace-marker spans.
///
/// The output spans are later joined with collected retrace annotations to build
/// precise diagnostics over rendered source fragments.
pub fn render_with_spans(main_tier: &MainTier) -> RenderedSpans {
    let mut rendered = String::new();
    let mut retrace_spans = Vec::new();

    rendered.push('*');
    main_tier.speaker.write_chat(&mut rendered).ok();
    rendered.push_str(":\t");

    for (i, linker) in main_tier.content.linkers.iter().enumerate() {
        if i > 0 {
            rendered.push(' ');
        }
        linker.write_chat(&mut rendered).ok();
    }

    if let Some(lang_code) = &main_tier.content.language_code {
        if !main_tier.content.linkers.is_empty() {
            rendered.push(' ');
        }
        rendered.push_str("[- ");
        lang_code.write_chat(&mut rendered).ok();
        rendered.push(']');
    }

    for (i, item) in main_tier.content.content.iter().enumerate() {
        let needs_space = i > 0
            || !main_tier.content.linkers.is_empty()
            || main_tier.content.language_code.is_some();
        if needs_space {
            rendered.push(' ');
        }
        render_utterance_content(item, &mut rendered, &mut retrace_spans);
    }

    if let Some(term) = &main_tier.content.terminator {
        if !main_tier.content.content.is_empty()
            || !main_tier.content.linkers.is_empty()
            || main_tier.content.language_code.is_some()
        {
            rendered.push(' ');
        }
        term.write_chat(&mut rendered).ok();
    }

    RenderedSpans { retrace_spans }
}

// ---------------------------------------------------------------------------
// Shared rendering primitives
// ---------------------------------------------------------------------------
//
// These three lived as byte-identical private copies in `bracketed.rs` and
// `utterance.rs`, and the retrace body was written FOUR times across the two
// (once per enum, twice per enum once `AnnotatedRetrace` arrived). One owner
// each, so a change to marker rendering or to span capture cannot land in
// three places and miss the fourth.

/// Render scoped annotations (none of which are retrace markers post-redesign).
pub(super) fn render_scoped_annotations<'a>(
    annotations: impl IntoIterator<Item = &'a crate::model::ContentAnnotation>,
    rendered: &mut String,
) {
    for ann in annotations {
        rendered.push(' ');
        ann.write_chat(rendered).ok();
    }
}

/// Write into `rendered` and return the written byte span.
///
/// This keeps span bookkeeping consistent across every rendering branch.
pub(super) fn write_with_span<F>(rendered: &mut String, mut write: F) -> Span
where
    F: FnMut(&mut String) -> std::fmt::Result,
{
    let start = rendered.len();
    write(rendered).ok();
    let end = rendered.len();
    Span::from_usize(start, end)
}

/// Render one retrace and record its marker span.
///
/// `Retrace::write_chat` cannot serve here: it has no way to hand back the
/// marker's byte span, which is the entire purpose of this renderer.
pub(super) fn render_retrace(
    retrace: &crate::model::Retrace,
    rendered: &mut String,
    retrace_spans: &mut Vec<Span>,
) {
    if retrace.is_group {
        rendered.push('<');
    }
    bracketed::render_bracketed_content(&retrace.content, rendered, retrace_spans);
    if retrace.is_group {
        rendered.push('>');
    }
    rendered.push(' ');
    let span = write_with_span(rendered, |w| retrace.kind.write_chat(w));
    retrace_spans.push(span);
}
