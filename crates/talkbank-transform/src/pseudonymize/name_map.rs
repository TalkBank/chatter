//! Private wire input becomes an immutable, collision-free name map.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome, WordContent, WordText};
use talkbank_parser::TreeSitterParser;

/// Admitted runtime map. No deserializer or mutable accessor bypasses admission.
///
/// Keys are caller-supplied transcript identities, compared exactly. In
/// particular, this API does not strip paths or extensions and thereby merge
/// distinct files. A CLI may explicitly select file stems as its key policy.
pub struct NameMap {
    transcripts: BTreeMap<String, TranscriptNames>,
}

/// Names for one transcript, admitted together so replacements cannot cascade.
pub struct TranscriptNames {
    by_name: BTreeMap<String, WordText>,
}

/// One decision about a lexical component; near misses never carry a rewrite.
pub enum NameDecision<'a> {
    /// Exact case-sensitive match with a parsed plain-text replacement.
    Replace(&'a WordText),
    /// Case differs from a mapped name; report it without rewriting.
    CaseNearMiss,
    /// Neither an exact match nor a case near miss.
    Keep,
}

/// Privacy-safe admission errors. Neither formatting nor error sources contain
/// the input, a transcript key, a name, or parser diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameMapError {
    /// The TOML schema is malformed or contains unknown fields.
    #[error("invalid name-map document")]
    InvalidDocument,
    /// Only the explicit version 1 schema is supported.
    #[error("unsupported name-map version")]
    UnsupportedVersion,
    /// A map or a transcript's name list is empty.
    #[error("name-map entries must not be empty")]
    EmptyEntries,
    /// A transcript key is empty or contains surrounding whitespace or controls.
    #[error("invalid name-map transcript key")]
    InvalidTranscriptKey,
    /// A transcript identity occurs more than once.
    #[error("duplicate name-map transcript key")]
    DuplicateTranscript,
    /// A name or replacement is not a single plain CHAT lexical token.
    #[error("name-map names and replacements must be plain lexical tokens")]
    InvalidToken,
    /// A source name occurs more than once within a transcript.
    #[error("duplicate source name in name map")]
    DuplicateName,
    /// A replacement contains a source-name match at Unicode word boundaries,
    /// including a case-only match or a report-only possessive.
    #[error("name-map replacement collides with a source name")]
    ReplacementCollision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMap {
    version: u32,
    transcripts: Vec<WireTranscript>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTranscript {
    key: String,
    names: Vec<WireName>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireName {
    original: String,
    replacement: String,
}

impl NameMap {
    /// Parse and admit version 1 TOML using the primary CHAT fragment parser.
    ///
    /// Each `[[transcripts]]` has a `key` and `[[transcripts.names]]` entries
    /// with `original` and `replacement`. Names describe complete lexical
    /// components, not CHAT notation: map `Name`, not `Nam(e)` or `Name+tag`.
    /// The document planner must match those through typed word content.
    ///
    /// Parser diagnostics and TOML errors can contain names, so they are
    /// deliberately converted to value-free boundary errors, never chained.
    /// This admits tokens for matching; cross-tier and output validity must
    /// still be established by the document rewrite pipeline.
    pub fn from_toml(input: &str, parser: &TreeSitterParser) -> Result<Self, NameMapError> {
        let wire: WireMap = toml::from_str(input).map_err(|_| NameMapError::InvalidDocument)?;
        if wire.version != 1 {
            return Err(NameMapError::UnsupportedVersion);
        }
        if wire.transcripts.is_empty() {
            return Err(NameMapError::EmptyEntries);
        }
        let mut transcripts = BTreeMap::new();
        for transcript in wire.transcripts {
            if transcript.key.is_empty()
                || transcript.key.trim() != transcript.key
                || transcript.key.chars().any(char::is_control)
            {
                return Err(NameMapError::InvalidTranscriptKey);
            }
            let names = TranscriptNames::admit(transcript.names, parser)?;
            if transcripts.insert(transcript.key, names).is_some() {
                return Err(NameMapError::DuplicateTranscript);
            }
        }
        Ok(Self { transcripts })
    }

    /// Select an exact transcript identity. Absence is distinct from zero hits.
    pub fn for_transcript(&self, key: &str) -> Option<&TranscriptNames> {
        self.transcripts.get(key)
    }
}

impl TranscriptNames {
    fn admit(names: Vec<WireName>, parser: &TreeSitterParser) -> Result<Self, NameMapError> {
        if names.is_empty() {
            return Err(NameMapError::EmptyEntries);
        }
        let mut by_name = BTreeMap::new();
        for name in names {
            let original = plain_token(&name.original, parser)?;
            let replacement = plain_token(&name.replacement, parser)?;
            if by_name
                .insert(original.as_ref().to_owned(), replacement)
                .is_some()
            {
                return Err(NameMapError::DuplicateName);
            }
        }
        let admitted = Self { by_name };
        for replacement in admitted.by_name.values() {
            match admitted.decide(replacement.as_ref()) {
                NameDecision::Keep => {}
                NameDecision::Replace(_) | NameDecision::CaseNearMiss => {
                    return Err(NameMapError::ReplacementCollision);
                }
            }
            // A plain CHAT token may contain several Unicode word segments.
            // Use the actual prose matcher too, so an embedded source name
            // cannot reappear when the placeholder is used in metadata/prose.
            if !super::free_text::matches_text(replacement.as_ref(), &admitted)
                .map_err(|_| NameMapError::InvalidToken)?
                .is_empty()
            {
                return Err(NameMapError::ReplacementCollision);
            }
        }
        Ok(admitted)
    }

    pub(super) fn patterns(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// Decide one complete lexical component, not a raw CHAT line or substring.
    ///
    /// Exact matches win. Near misses use Unicode lowercase comparison only
    /// for reporting; it is not locale-aware case folding or normalization and
    /// never authorizes a replacement. This API must receive the component
    /// selected by the typed traversal, including reconstructed shortenings.
    pub fn decide(&self, component: &str) -> NameDecision<'_> {
        if let Some(replacement) = self.by_name.get(component) {
            return NameDecision::Replace(replacement);
        }
        let lower = component.to_lowercase();
        if self.by_name.keys().any(|name| name.to_lowercase() == lower) {
            NameDecision::CaseNearMiss
        } else {
            NameDecision::Keep
        }
    }
}

fn plain_token(input: &str, parser: &TreeSitterParser) -> Result<WordText, NameMapError> {
    let errors = ErrorCollector::new();
    let parsed = ChatParser::parse_word(parser, input, 0, &errors);
    let ParseOutcome::Parsed(word) = parsed else {
        return Err(NameMapError::InvalidToken);
    };
    if !errors.is_empty() || word.untranscribed().is_some() {
        return Err(NameMapError::InvalidToken);
    }
    let [part] = word.content().as_slice() else {
        return Err(NameMapError::InvalidToken);
    };
    match part {
        WordContent::Text(text) => {
            if text.as_ref() == input {
                Ok(text.clone())
            } else {
                Err(NameMapError::InvalidToken)
            }
        }
        WordContent::Phonetic(_)
        | WordContent::Shortening(_)
        | WordContent::OverlapPoint(_)
        | WordContent::CAElement(_)
        | WordContent::CADelimiter(_)
        | WordContent::StressMarker(_)
        | WordContent::Lengthening(_)
        | WordContent::SyllablePause(_)
        | WordContent::UnderlineBegin(_)
        | WordContent::UnderlineEnd(_)
        | WordContent::CompoundMarker(_)
        | WordContent::CliticBoundary(_) => Err(NameMapError::InvalidToken),
    }
}

impl fmt::Debug for NameMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NameMap(<private>)")
    }
}

impl fmt::Debug for TranscriptNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TranscriptNames(<private>)")
    }
}

#[cfg(test)]
mod tests;
