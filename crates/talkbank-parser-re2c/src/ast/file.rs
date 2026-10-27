//! File lines and dependent-tier entries retain source separator provenance.

use super::{DependentTierParsed, HeaderParsed, MainTier};
use serde::Serialize;

/// A parsed CHAT file with its original input and source-owning lines.
///
/// The source cannot be replaced independently of the parse:
///
/// ```compile_fail
/// use talkbank_parser_re2c::ast::ChatFile;
/// fn replace_source<'a>(file: &mut ChatFile<'a>, foreign: &'a str) {
///     file.source = foreign;
/// }
/// ```
///
/// Inspect the parse through borrowed accessors, or consume its lines:
///
/// ```
/// use talkbank_parser_re2c::ast::{ChatFile, Line};
/// fn inspect<'a>(file: ChatFile<'a>) -> Vec<Line<'a>> {
///     let _original = file.source();
///     let _count = file.lines().len();
///     file.into_lines()
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatFile<'a> {
    lines: Vec<Line<'a>>,
    /// Original source text, needed for lossless raw_text reconstruction via spans.
    source: &'a str,
}

impl<'a> ChatFile<'a> {
    pub(crate) fn from_parsed(lines: Vec<Line<'a>>, source: &'a str) -> Self {
        Self { lines, source }
    }

    /// Lines produced by the parser, each retaining its source evidence.
    pub fn lines(&self) -> &[Line<'a>] {
        &self.lines
    }

    /// Consume the parsed file to inspect its source-owning lines.
    pub fn into_lines(self) -> Vec<Line<'a>> {
        self.lines
    }

    /// The original caller input, including unparsed recovery bytes.
    pub fn source(&self) -> &'a str {
        self.source
    }
}

/// A line in a CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Line<'a> {
    Header(Box<LocatedHeader<'a>>),
    Utterance(Box<Utterance<'a>>),
}

/// A main tier and the dependent tiers attached by the native line machine.
///
/// The parser's grouping is retained through lowering; callers cannot splice
/// independently parsed tiers into an observed utterance:
///
/// ```compile_fail,E0451
/// use talkbank_parser_re2c::ast::{DependentTierEntryParsed, LocatedMainTier, Utterance};
/// fn assemble<'a>(main_tier: LocatedMainTier<'a>, dependent_tiers: Vec<DependentTierEntryParsed<'a>>) -> Utterance<'a> {
///     Utterance { main_tier, dependent_tiers }
/// }
/// ```
///
/// Even an owned parsed utterance exposes its tier sequence only for reading:
///
/// ```compile_fail,E0599
/// use talkbank_parser_re2c::ast::Utterance;
/// fn discard_observed_tiers(utterance: &mut Utterance<'_>) {
///     utterance.dependent_tiers().clear();
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance<'a> {
    main_tier: LocatedMainTier<'a>,
    dependent_tiers: Vec<DependentTierEntryParsed<'a>>,
}

impl<'a> Utterance<'a> {
    /// Consume the main tier and following tiers from one line-machine pass.
    pub(crate) fn from_parsed(
        main_tier: LocatedMainTier<'a>,
        dependent_tiers: Vec<DependentTierEntryParsed<'a>>,
    ) -> Self {
        Self {
            main_tier,
            dependent_tiers,
        }
    }

    /// Main tier retained with its syntax, source and locations.
    pub fn main_tier(&self) -> &LocatedMainTier<'a> {
        &self.main_tier
    }

    /// Following dependent tiers in the order observed by the parser.
    pub fn dependent_tiers(&self) -> &[DependentTierEntryParsed<'a>] {
        &self.dependent_tiers
    }
}

/// A main tier together with the locations the line machine measured for
/// it. The chumsky main-tier grammar sees tokens only, and after recovery it
/// may see a filtered stream whose indices no longer match the lexer's, so
/// only the line machine, which owns tokens and spans together, can place the
/// line, its speaker and its content. Constructed there and nowhere else.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocatedMainTier<'a> {
    #[serde(flatten)]
    tier: MainTier<'a>,
    #[serde(skip)]
    locations: talkbank_model::MainTierLocations,
    #[serde(skip)]
    source: crate::source_text::SourceSlice<'a>,
}

impl<'a> LocatedMainTier<'a> {
    pub(crate) fn new(
        tier: MainTier<'a>,
        locations: talkbank_model::MainTierLocations,
        source: crate::source_text::SourceSlice<'a>,
    ) -> Self {
        Self {
            tier,
            locations,
            source,
        }
    }

    /// Original source retained by the line machine with its admitted extent.
    pub fn source(&self) -> crate::source_text::SourceText<'a> {
        self.source.source()
    }

    pub fn tier(&self) -> &MainTier<'a> {
        &self.tier
    }

    /// The line including its ending, the speaker and the content region
    /// (from the end of the separator's colon to the end of the line).
    pub fn locations(&self) -> talkbank_model::MainTierLocations {
        self.locations
    }
}

/// A parsed dependent tier retains its payload and source as one admitted unit.
///
/// A caller cannot substitute a payload while retaining another tier's source:
///
/// ```compile_fail
/// use talkbank_parser_re2c::ast::{DependentTierEntryParsed, DependentTierParsed};
/// fn replace_payload<'a>(entry: &mut DependentTierEntryParsed<'a>, tier: DependentTierParsed<'a>) {
///     entry.tier = tier;
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependentTierEntryParsed<'a> {
    tier: DependentTierParsed<'a>,
    #[serde(skip)]
    separator: talkbank_model::model::TierSeparator,
    #[serde(skip)]
    content_range: Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
    /// The complete original tier line, including its selected ending.
    #[serde(skip)]
    source: crate::source_text::SourceSlice<'a>,
}

impl<'a> DependentTierEntryParsed<'a> {
    pub(crate) fn from_lexed(
        tier: DependentTierParsed<'a>,
        separator: talkbank_model::model::TierSeparator,
        content_range: Option<talkbank_model::model::dependent_tier::DependentTierContentSpan>,
        source: crate::source_text::SourceSlice<'a>,
    ) -> Self {
        Self {
            tier,
            separator,
            content_range,
            source,
        }
    }

    /// The parsed payload cannot be replaced independently of its source.
    pub fn tier(&self) -> &DependentTierParsed<'a> {
        &self.tier
    }

    /// Separator retained by the lexer prefix.
    pub fn separator(&self) -> talkbank_model::model::TierSeparator {
        self.separator
    }

    /// Admitted content geometry, absent when recovery could not establish it.
    pub fn content_range(
        &self,
    ) -> Option<talkbank_model::model::dependent_tier::DependentTierContentSpan> {
        self.content_range
    }

    /// Original logical tier source and coordinates.
    pub fn source(&self) -> crate::source_text::SourceSlice<'a> {
        self.source
    }
}

/// Parsed header syntax and its original source admitted by the line machine.
/// Fields cannot be replaced independently after admission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocatedHeader<'a> {
    header: HeaderParsed<'a>,
    #[serde(skip)]
    source: crate::source_text::SourceSlice<'a>,
    #[serde(skip)]
    separator: talkbank_model::model::TierSeparator,
}

impl<'a> LocatedHeader<'a> {
    pub(crate) fn from_lexed(
        header: HeaderParsed<'a>,
        source: crate::source_text::SourceSlice<'a>,
        separator: talkbank_model::model::TierSeparator,
    ) -> Self {
        Self {
            header,
            source,
            separator,
        }
    }

    /// Syntax retained for native AST inspection.
    pub fn header(&self) -> &HeaderParsed<'a> {
        &self.header
    }

    /// Original full-buffer source, preserving document-relative coordinates.
    pub fn source(&self) -> crate::source_text::SourceText<'a> {
        self.source.source()
    }

    /// Complete logical header extent, including its line terminator.
    pub fn span(&self) -> talkbank_model::Span {
        self.source.span()
    }

    /// Separator recognized with this header's prefix.
    pub fn separator(&self) -> talkbank_model::model::TierSeparator {
        self.separator
    }
}
