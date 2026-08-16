//! Unified dependent-tier model used by utterance storage/serialization.
//!
//! CHAT reference anchors:
//! - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use super::{
    ActTier, AddTier, CodTier, ComTier, ExpTier, GpxTier, GraTier, IntTier, MorTier, PhoTier,
    SinTier, SitTier, SpaTier, WorTier,
};
use crate::model::NonEmptyString;

/// Unified representation of all dependent tier types.
///
/// This enum consolidates all dependent tier types into a single structure
/// for efficient storage. Most utterances have 0-3 dependent tiers, so a Vec
/// is more memory-efficient than 25+ Option fields.
///
/// # Tier Organization
///
/// Tiers are organized by their structural characteristics:
///
/// **Structured linguistic tiers** (full tier types with rich parsing):
/// - Morphological: %mor → [`MorTier`]
/// - Grammatical: %gra → [`GraTier`]
/// - Phonological: %pho, %mod → [`PhoTier`]
/// - Gesture: %sin → [`SinTier`]
/// - Word timing: %wor → [`WorTier`]
/// - Action: %act, Coding: %cod
///
/// **Text-based tiers with bullets** (BulletContent):
/// - %add, %com, %exp, %gpx, %int, %sit, %spa
///
/// **Simple text-only tiers** (String):
/// - %alt, %coh, %def, %eng, %err, %fac, %flo, %gls, %ort, %par, %tim
///
/// # Memory Efficiency
///
/// Using a `Vec<DependentTier>` instead of separate Option fields reduces
/// memory overhead for the common case where utterances have 0-3 dependent
/// tiers rather than the full set of 25+ possible tiers.
///
/// # CHAT Format Examples
///
/// Morphological tier:
/// ```text
/// %mor: pro:sub|I v|want n|cookie-PL .
/// ```
///
/// Grammatical relations tier:
/// ```text
/// %gra: 1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
/// ```
///
/// Phonological tier:
/// ```text
/// %pho: aɪ wɑnt kʊkiz .
/// ```
///
/// Action tier with bullets:
/// ```text
/// %act: picks up toy 1000_2000 drops it 3000_4000
/// ```
///
/// Simple text tier:
/// ```text
/// %com: Child is tired
/// ```
///
/// # References
///
/// - [CHAT Manual: Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", content = "data")]
pub enum DependentTier {
    // Structured tiers storing full parsed tier types
    /// Morphology tier (%mor)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
    Mor(MorTier),

    /// Grammatical relations tier (%gra)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
    Gra(GraTier),

    /// Phonology tier (%pho)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
    Pho(PhoTier),
    /// Model phonology tier (%mod)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Model_Tier>
    Mod(PhoTier),

    /// Gesture/sign tier (%sin)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
    Sin(SinTier),
    /// Action tier (%act)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Action_Tier>
    Act(ActTier),
    /// Coding tier (%cod)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
    Cod(CodTier),

    // Text-based standard tiers with structured content (BulletContent)
    /// Addressee tier (%add)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
    Add(AddTier),
    /// Comment tier (%com)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
    Com(ComTier),
    /// Explanation tier (%exp)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
    Exp(ExpTier),
    /// Gesture tier (%gpx)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Gestural_Tier>
    Gpx(GpxTier),
    /// Intonation tier (%int)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
    Int(IntTier),
    /// Situation tier (%sit)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
    Sit(SitTier),
    /// Speech act tier (%spa)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
    Spa(SpaTier),

    // Simple text-only tiers (string content, no bullets)
    /// Alternative tier (%alt)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Alternate_Tier>
    Alt(TextTier),
    /// Cohesion tier (%coh)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Cohesion_Tier>
    Coh(TextTier),
    /// Definition tier (%def)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Definitions_Tier>
    Def(TextTier),
    /// English translation tier (%eng)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#English_Tier>
    Eng(TextTier),
    /// Error tier (%err)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Error_Tier>
    Err(TextTier),
    /// Facial expression tier (%fac)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#FacialGesture_Tier>
    Fac(TextTier),
    /// Flow tier (%flo)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Flow_Tier>
    Flo(TextTier),
    /// Syllabified model phonology tier (%modsyl), Phon project.
    /// Syllable-structured representation of the target/model pronunciation.
    /// Aligns content-based with `%mod`.
    Modsyl(super::phon::SylTier),
    /// Syllabified actual phonology tier (%phosyl), Phon project.
    /// Syllable-structured representation of the actual production.
    /// Aligns content-based with `%pho`.
    Phosyl(super::phon::SylTier),
    /// Phonological alignment tier (%phoaln), Phon project.
    /// Segmental alignment between target and actual IPA, word-by-word.
    Phoaln(super::phon::PhoalnTier),
    /// Phone interval tier (%xphoint), Phon project.
    /// Per-phone time intervals (`\u{0015}` bullets) segmenting each `%pho` word.
    Xphoint(super::phon::XphointTier),
    /// Gloss tier (%gls)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Gloss_Tier>
    Gls(TextTier),
    /// Orthography tier (%ort)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Orthography_Tier>
    Ort(TextTier),
    /// Paralinguistics tier (%par)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Paralinguistics_Tier>
    Par(TextTier),
    /// Timing tier (%tim)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier>
    Tim(super::TimTier),
    /// Word timing tier (%wor)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
    Wor(WorTier),

    /// User-defined tier (%xLABEL)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#User_Tier>
    UserDefined(UserDefinedDependentTier),

    /// Unsupported/unknown tier (e.g. %foo, not a standard tier, not %x-prefixed)
    ///
    /// Distinct from `UserDefined` which is intentional (`%xLABEL`). This variant
    /// captures tiers that the grammar's catch-all matched but that are not
    /// recognized CHAT tiers. Validators reject these as an error (E605): the
    /// supported dependent-tier set is closed, so an unrecognized non-`%x` tier
    /// makes the file invalid.
    Unsupported(UserDefinedDependentTier),
}

/// A simple text-only dependent tier with span tracking.
///
/// Used for tiers that carry free-form text without inline bullets or
/// structural alignment: %alt, %coh, %def, %eng, %err, %fac, %flo,
/// %gls, %ort, %par, %tim.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
pub struct TextTier {
    /// Plain text payload, absent when the tier declares nothing.
    ///
    /// Optional because an empty tier is something a FILE can contain, and a
    /// parser's job is to say what the file contains. `NonEmptyString` alone
    /// forced a parser meeting `%eng:` with no content to invent one: the re2c
    /// backend substituted a single space, which made the tier look well
    /// formed and the whole file read as VALID where the tree-sitter backend
    /// reported errors. A type that cannot express what the input says is a
    /// type that makes its producer lie.
    ///
    /// `UserDefinedDependentTier` in this same module already models it this
    /// way, and E756 already judges it: "a tier whose content is empty
    /// declares nothing; the line asserts an annotation that is not there".
    /// That reasoning was never `%x`-specific.
    ///
    /// Recovery is not validity: representing the empty tier is what lets a
    /// validator reject it, rather than the parser hiding it.
    ///
    /// E756 judges this state. The rule ("a tier whose content is empty
    /// declares nothing") was never `%x`-specific, and the maintainer widened
    /// it on 2026-08-16 rather than write E330's spec or mint a third code.
    /// `DependentTier::empty_content_span` is where a tier answers it.
    ///
    pub content: Option<NonEmptyString>,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl TextTier {
    /// Creates a text-only dependent tier with default span metadata.
    ///
    /// This constructor is used for `%alt/%coh/%def/...` style tiers that do
    /// not carry structured token alignment. Parser paths typically call
    /// [`Self::with_span`] immediately after construction.
    pub fn new(content: NonEmptyString) -> Self {
        Self {
            content: Some(content),
            span: crate::Span::DUMMY,
        }
    }

    /// Creates a text-only dependent tier that declares nothing.
    ///
    /// Named rather than reached by passing `None` to [`Self::new`], so that
    /// producing an empty tier is a deliberate act a reader can grep for. The
    /// only legitimate caller is a parser that met a tier line with no content
    /// and must say so; every other path has content and uses [`Self::new`].
    ///
    /// The result is INVALID CHAT, and deliberately representable anyway:
    /// recovery is not validity, and a parser that cannot express what the file
    /// says is a parser that will invent something instead. E756 states the
    /// rule this exists to let the validator apply.
    pub fn empty() -> Self {
        Self {
            content: None,
            span: crate::Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    ///
    /// Keeping span assignment explicit lets test fixtures stay concise while
    /// preserving precise offsets in parser-produced values.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Borrows the raw tier payload text, empty when the tier declares nothing.
    ///
    /// The returned view excludes `%tag:\t` prefixes and any serialization
    /// wrapper logic from [`write_chat`](crate::model::WriteChat::write_chat).
    ///
    /// An empty tier reads as `""` here rather than forcing every caller to
    /// unwrap, because "the text of this tier" genuinely is nothing. Callers
    /// that must distinguish "empty" from "absent" read [`Self::content`]
    /// directly; the validator is the one that cares.
    pub fn as_str(&self) -> &str {
        self.content.as_deref().unwrap_or("")
    }
}

impl std::fmt::Display for TextTier {
    /// Formats only the raw tier payload text (without `%tag:\t` prefix).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A user-defined dependent tier (%xLABEL).
///
/// Stores the label (e.g., "xmor" for %xmor) and free-form text content.
/// User-defined tiers allow researchers to add custom annotation layers
/// beyond the standard CHAT dependent tier types.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct UserDefinedDependentTier {
    /// The tier label including the 'x' prefix (e.g., "xmor" for %xmor).
    pub label: NonEmptyString,
    /// The free-form text content of the tier, or `None` when the line
    /// declared a tier and gave it nothing.
    ///
    /// `Option` because the empty case is REAL CHAT that a parser can meet, and
    /// E756 exists to report it. While this was `NonEmptyString` the state the
    /// rule checks for was unrepresentable, so each parser had to invent a way
    /// round it: the re2c parser fabricated a single space (which tripped E756
    /// by accident, right answer for the wrong reason) and the tree-sitter
    /// parser dropped the tier and reported a generic parse error instead. The
    /// two disagreed because the model gave them no honest option.
    pub content: Option<NonEmptyString>,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip, default = "crate::Span::dummy")]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl super::WriteChat for DependentTier {
    /// Serializes one complete dependent-tier line in CHAT syntax.
    ///
    /// Structured variants delegate to their own `WriteChat` implementations so
    /// tier-specific formatting rules stay local. Simple text variants use a
    /// fixed `%tag:\t` prefix followed by raw payload text.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            // Structured tiers, delegate to tier's WriteChat impl
            DependentTier::Mor(t) => t.write_chat(w),
            DependentTier::Gra(t) => t.write_chat(w),
            DependentTier::Pho(t) => t.write_chat(w),
            DependentTier::Mod(t) => t.write_chat(w),
            DependentTier::Sin(t) => t.write_chat(w),
            DependentTier::Act(t) => t.write_chat(w),
            DependentTier::Cod(t) => t.write_chat(w),

            // Text tiers with structured content (have WriteChat impl)
            DependentTier::Add(t) => t.write_chat(w),
            DependentTier::Com(t) => t.write_chat(w),
            DependentTier::Exp(t) => t.write_chat(w),
            DependentTier::Gpx(t) => t.write_chat(w),
            DependentTier::Int(t) => t.write_chat(w),
            DependentTier::Sit(t) => t.write_chat(w),
            DependentTier::Spa(t) => t.write_chat(w),

            // Simple text tiers (just string content)
            DependentTier::Alt(s) => write!(w, "%alt:\t{}", s),
            DependentTier::Coh(s) => write!(w, "%coh:\t{}", s),
            DependentTier::Def(s) => write!(w, "%def:\t{}", s),
            DependentTier::Eng(s) => write!(w, "%eng:\t{}", s),
            DependentTier::Err(s) => write!(w, "%err:\t{}", s),
            DependentTier::Fac(s) => write!(w, "%fac:\t{}", s),
            DependentTier::Flo(s) => write!(w, "%flo:\t{}", s),
            DependentTier::Modsyl(t) => t.write_chat(w),
            DependentTier::Phosyl(t) => t.write_chat(w),
            DependentTier::Phoaln(t) => t.write_chat(w),
            DependentTier::Xphoint(t) => t.write_chat(w),
            DependentTier::Gls(s) => write!(w, "%gls:\t{}", s),
            DependentTier::Ort(s) => write!(w, "%ort:\t{}", s),
            DependentTier::Par(s) => write!(w, "%par:\t{}", s),
            DependentTier::Tim(t) => write!(w, "%tim:\t{}", t),

            // Word timing tier, delegate to WriteChat
            DependentTier::Wor(t) => t.write_chat(w),

            // User-defined tier (%xLABEL:\tcontent)
            // Label already includes 'x' prefix (e.g., "xmor" for %xmor)
            DependentTier::UserDefined(tier) => match &tier.content {
                Some(content) => write!(w, "%{}:\t{}", tier.label, content),
                // An empty tier roundtrips as the bare line it came from.
                None => write!(w, "%{}:\t", tier.label),
            },

            // Unsupported/unknown tier, same serialization as UserDefined
            DependentTier::Unsupported(tier) => match &tier.content {
                Some(content) => write!(w, "%{}:\t{}", tier.label, content),
                None => write!(w, "%{}:\t", tier.label),
            },
        }
    }
}

impl std::fmt::Display for DependentTier {
    /// Formats the complete dependent tier line in CHAT syntax.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        super::WriteChat::write_chat(self, f)
    }
}
