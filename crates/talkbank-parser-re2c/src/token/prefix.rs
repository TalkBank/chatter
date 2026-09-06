//! A lexer-owned prefix payload and its separator provenance.

use serde::Serialize;
use talkbank_model::{Span, model::TierSeparator};

/// Prefix payload retained together with the separator consumed by its lexer
/// rule. Some header prefixes have no separator; their provenance is clean.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrefixToken<'a> {
    text: &'a str,
    #[serde(skip)]
    separator: TierSeparator,
}

impl<'a> PrefixToken<'a> {
    /// Called only at lexical admission. `matched` includes any trailing
    /// separator spaces; `payload` may instead be a tag-extracted speaker.
    pub(crate) fn from_lexed(payload: &'a str, matched: &'a str, start: usize) -> Self {
        let prefix = matched.trim_end_matches(' ');
        let separator = if prefix.ends_with(":\t") && prefix.len() < matched.len() {
            TierSeparator::with_trailing_space(Span::from_usize(
                start + prefix.len(),
                start + matched.len(),
            ))
        } else {
            TierSeparator::CLEAN
        };
        let text = if prefix.ends_with(":\t") {
            payload.trim_end_matches(' ')
        } else {
            payload
        };
        Self { text, separator }
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn separator(&self) -> TierSeparator {
        self.separator
    }
}

/// Body grammar selected by the exact dependent-prefix lexer rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DependentBodyKind {
    /// Morphology with rejected-tier provenance.
    Mor,
    /// Translation morphology with its existing text recovery.
    Trn,
    /// Actual phonology.
    Pho,
    /// Model phonology.
    Mod,
    /// Grammatical relations.
    Gra,
    /// Sign and gesture structure.
    Sin,
    /// Timed words.
    Wor,
    /// Text or a specialized tier lowered from text tokens.
    Text,
}

/// Complete dependent prefix carrying the lexer-selected body grammar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DependentPrefixToken<'a> {
    #[serde(flatten)]
    prefix: PrefixToken<'a>,
    kind: DependentBodyKind,
}

impl<'a> DependentPrefixToken<'a> {
    pub(crate) fn from_lexed(prefix: PrefixToken<'a>, kind: DependentBodyKind) -> Self {
        Self { prefix, kind }
    }

    /// Original prefix payload, excluding trailing separator spaces.
    pub fn text(&self) -> &'a str {
        self.prefix.text()
    }

    /// Separator evidence admitted by the lexer.
    pub fn separator(&self) -> TierSeparator {
        self.prefix.separator()
    }

    /// Exact body grammar admitted by the lexer.
    pub fn kind(&self) -> DependentBodyKind {
        self.kind
    }
}
