//! Native parsing carriers and remaining intermediate semantic types.
//!
//! MOR and text bodies use shared model values; other families remain under
//! review for direct construction or justified source-bound parsing mechanics.

use crate::token::Token;
use serde::Serialize;
use talkbank_model::model::OverlapPointKind;

pub(crate) mod chain;
mod chain_atoms;
mod content_ref;
mod enclosure;
mod file;
pub use chain::Chain;
pub use chain_atoms::{ParsedEvent, ZeroAction};
pub use content_ref::{ContentChildren, ContentRef};
pub use enclosure::Enclosure;

mod word;
pub use word::{
    LengtheningRun, ParsedAnnotation, ReplacementParsed, RetraceMarker, ScopedAnnotationParsed,
    ScopedOverlapIndex, WordBodyItem, WordWithAnnotations,
};

/// Observed nonempty label of a text-backed dependent tier.
/// Only the native prefix-admission edge constructs this carrier; lowering
/// cannot receive an arbitrary token or an empty label in its place.
///
/// ```compile_fail,E0308
/// use talkbank_parser_re2c::{ast::DependentTierParsed, token::Token};
/// let tier = DependentTierParsed::RejectedText { label: Token::Period(".") };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TextTierLabel(talkbank_model::NonEmptyString);

impl TextTierLabel {
    pub(crate) fn from_prefix_text(prefix: &str) -> Result<Self, talkbank_model::EmptyText> {
        let label = prefix.trim_start_matches('%').trim_end_matches(":\t");
        talkbank_model::NonEmptyString::new(label).map(Self)
    }

    /// The admitted label, without the prefix or separator.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn nonempty(&self) -> &talkbank_model::NonEmptyString {
        &self.0
    }
}

/// A parsed main tier: *SPEAKER:\t tier_body
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MainTier<'a> {
    #[serde(skip)]
    pub separator: talkbank_model::model::TierSeparator,
    pub speaker: Token<'a>,
    pub tier_body: TierBody<'a>,
}

/// grammar.js: tier_body = seq(
///   optional(linkers),
///   optional(seq(langcode, whitespaces)),
///   contents,
///   utterance_end
/// )
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TierBody<'a> {
    /// Linkers already admitted and located by the grammar, never arbitrary tokens.
    ///
    /// ```compile_fail,E0308
    /// use talkbank_parser_re2c::{ast::TierBody, token::Token};
    /// fn insert_non_linker(body: &mut TierBody<'_>) {
    ///     body.linkers.push(Token::Period("."));
    /// }
    /// ```
    pub linkers: Vec<talkbank_model::Linker>,
    /// Shared precode admitted by the grammar, never unchecked extracted text.
    ///
    /// ```compile_fail,E0308
    /// use talkbank_parser_re2c::{ast::TierBody, token::Extracted};
    /// fn insert_unchecked_precode(body: &mut TierBody<'_>, raw: Extracted<'_>) {
    ///     body.langcode = Some(raw);
    /// }
    /// ```
    pub langcode: Option<talkbank_model::LanguagePrecode>,
    pub contents: Vec<ContentItem<'a>>,
    pub terminator: Option<talkbank_model::Terminator>,
    pub postcodes: Vec<crate::token::PostcodeToken<'a>>,
    pub media_bullet: Option<crate::token::MediaBulletToken<'a>>,
}

/// grammar.js: contents = repeat1(choice(whitespaces, content_item, separator, overlap_point))
/// Whitespace is not stored, structural only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ContentItem<'a> {
    /// A word with optional trailing annotations.
    Word(WordWithAnnotations<'a>),
    /// grammar.js: pause_token
    Pause(talkbank_model::model::Pause),
    /// Freecode (`[^ text]`), a first-class content item. Its payload is the
    /// tag-extracted text, its lexeme the whole `[^ text]`.
    Freecode(crate::token::Extracted<'a>),
    /// An annotation with nothing to scope over, e.g. a `[/]` opening an
    /// utterance. Ungrammatical, and the subject of E759.
    ///
    /// `grammar.js` has no standalone-annotation production at all: an
    /// annotation is always attached to a word or nonword. This variant exists
    /// so the invalid construct survives far enough to be REPORTED, not
    /// because it is content.
    ///
    /// It carried a raw `Token` until 2026-08-09, justified in this comment by
    /// "E759 quotes the marker as written". That was false, and the lexer said
    /// so: three of the seven trigger tokens are tag-extracted, so re2c
    /// reported `Annotation '1'` where tree-sitter reported `Annotation '[<1]'`.
    /// A typed annotation retains the exact written form, exposed through
    /// [`ParsedAnnotation::chat_text`].
    OrphanAnnotation(ParsedAnnotation<'a>),
    /// Separator admitted directly to the shared model with its source location.
    Separator(talkbank_model::model::Separator),
    /// grammar.js: overlap_point, with its kind and index already resolved.
    ///
    /// The parser selects the
    /// four overlap tokens one by one, so it knows the kind at construction,
    /// and storing a raw `Token` forced the converter to re-derive it and
    /// invent a fallback for a shape that cannot arrive. Both call sites then
    /// turned an overlap marker into a WORD on that dead branch.
    OverlapPoint {
        kind: OverlapPointKind,
        /// `⌈2` and friends; the digit written after the marker.
        index: Option<u32>,
        /// The marker as written, borrowed from the source, so the converter
        /// can place it.
        text: &'a str,
    },
    /// Retrace: word(s) followed by [/], [//], [///], [/-], or [/?]
    /// The retraced content is wrapped here; the corrected content follows.
    Retrace(Retrace<'a>),
    /// grammar.js: group_with_annotations = seq(<, contents, >, annotations)
    Group(Group<'a>),
    /// grammar.js: quotation = seq(left_double_quote, contents, right_double_quote)
    Quotation(Quotation<'a>),
    /// A source-located event with its ordered, scoped annotations.
    /// Lowering chooses the shared bare or annotated shape once.
    Event(ParsedEvent<'a>, Vec<ScopedAnnotationParsed<'a>>),
    /// Media bullet with lexer-owned numeric admission and original spelling.
    /// Rejected timing remains present until lowering, never a zero sentinel.
    MediaBullet {
        bullet: crate::token::MediaBulletToken<'a>,
    },
    // `CaMarker` was deleted 2026-08-09: nothing constructed it. CA element
    // and delimiter tokens are consumed by the word-body scanner into
    // `WordBodyItem::CaElement` / `CaDelimiter`, so they never reach content
    // level, yet both converters carried an arm for the state and one of them
    // built a whole `Word` for it. The five label variants below carry
    // `&'a str` rather than a `Token`: every consumer read them as
    // `tok.text()` on an already tag-extracted token, so the token was a
    // wrapper around the one field anyone wanted.
    /// Underline begin marker (`␂␁`), as written, so the converter can place
    /// it.
    UnderlineBegin(&'a str),
    /// Underline end marker (`␂␂`). See [`ContentItem::UnderlineBegin`].
    UnderlineEnd(&'a str),
    /// Interposed speech admitted through the ordinary word grammar.
    OtherSpokenEvent(talkbank_model::OtherSpokenEvent),
    /// Phonological group: ‹ contents ›
    PhoGroup(EnclosedGroup<'a>),
    /// Sign group: 〔 contents 〕
    SinGroup(EnclosedGroup<'a>),
    /// Long feature begin (`&{l=LABEL`); its payload is the label alone.
    LongFeatureBegin(talkbank_model::LongFeatureBegin),
    /// Long feature end (`&}l=LABEL`); its payload is the label alone.
    LongFeatureEnd(talkbank_model::LongFeatureEnd),
    /// Nonvocal begin (`&{n=LABEL`); its payload is the label alone.
    NonvocalBegin(talkbank_model::NonvocalBegin),
    /// Nonvocal end (`&}n=LABEL`); its payload is the label alone.
    NonvocalEnd(talkbank_model::NonvocalEnd),
    /// Nonvocal simple, self-closing (`&{n=LABEL}`); its payload is the label alone.
    NonvocalSimple(talkbank_model::NonvocalSimple),
    /// Standalone zero (0), action without speech.
    /// grammar.js: nonword = choice(event, zero)
    /// With annotations: annotated_action; without: bare action.
    Action {
        /// The admitted standalone zero spelling and its source coordinates.
        zero: ZeroAction<'a>,
        annotations: Vec<ScopedAnnotationParsed<'a>>,
    },
}

impl<'a> ContentItem<'a> {
    /// The content this item encloses. Empty for a leaf.
    ///
    /// # Why this exists
    ///
    /// The one place that knows which variants of this enum contain other
    /// content. Every recursive walker needs that fact, and until this existed
    /// each answered it with its own match, which is how
    /// the missing-annotation walker came to descend into `Retrace`, `Group`
    /// and `Quotation` but not `PhoGroup` or `SinGroup`. A missing annotation
    /// inside `‹...›` was invisible to it.
    ///
    /// EXHAUSTIVENESS DOES NOT CATCH THAT. A walker can list every variant and
    /// still answer `false` for a container, so `_ => false` and
    /// `PhoGroup(_) => false` are the same defect and
    /// `clippy::wildcard_enum_match_arm` only sees the first. Descent has to be
    /// structural, which is what this accessor makes it: a caller recurses on
    /// `children()` and cannot skip a container, because it never enumerates
    /// them.
    ///
    /// The four container payloads have three different field spellings
    /// (`Retrace.content`, `Group.contents`/`Quotation.contents`, and the bare
    /// `Vec` of the pho/sin groups), which is precisely why a recursion author
    /// stops after the two that look alike.
    ///
    /// `talkbank-model` solved the same problem for its own content enums with
    /// `model::content::structure::ContentStructure`; this is that idea for
    /// re2c's separate AST, which cannot share the type.
    #[inline]
    pub fn children(&self) -> ContentChildren<'_, 'a> {
        match self {
            Self::Retrace(retrace) => retrace.body.items(),
            Self::Group(group) => ContentChildren::items(&group.contents),
            Self::Quotation(quotation) => ContentChildren::items(&quotation.contents),
            Self::PhoGroup(group) | Self::SinGroup(group) => {
                ContentChildren::items(&group.contents)
            }
            Self::Word(_)
            | Self::Pause(_)
            | Self::Freecode(_)
            | Self::OrphanAnnotation(_)
            | Self::Separator { .. }
            | Self::OverlapPoint { .. }
            | Self::Event(..)
            | Self::MediaBullet { .. }
            | Self::UnderlineBegin(_)
            | Self::UnderlineEnd(_)
            | Self::OtherSpokenEvent(_)
            | Self::LongFeatureBegin(_)
            | Self::LongFeatureEnd(_)
            | Self::NonvocalBegin(_)
            | Self::NonvocalEnd(_)
            | Self::NonvocalSimple(_)
            | Self::Action { .. } => ContentChildren::items(&[]),
        }
    }
}

/// An unannotated group retains its delimiters through model lowering.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct EnclosedGroup<'a> {
    #[serde(skip)]
    pub enclosure: Enclosure<'a>,
    pub contents: Vec<ContentItem<'a>>,
}

/// What a retrace marker scopes over: the two shapes the chain fold builds,
/// each carrying exactly what its extent needs, so "no brackets means one
/// item" is the type rather than a convention a converter has to re-check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RetraceBody<'a> {
    /// `<a b> [/]`: a bare group handed its brackets to the retrace (the
    /// model's `is_group`); the extent runs from the opening bracket.
    Enclosed(Enclosure<'a>, Vec<ContentItem<'a>>),
    /// `word [/]`, `<a b> [?] [/]`, `“a” [/]`: the one item the marker
    /// scopes over, whose own extent starts the retrace's. Only admitted
    /// marker-chain content can occupy this position.
    Single(Box<Chain<'a>>),
}

impl<'a> RetraceBody<'a> {
    /// The retraced items, in order.
    pub fn items(&self) -> ContentChildren<'_, 'a> {
        match self {
            Self::Enclosed(_, items) => ContentChildren::items(items),
            Self::Single(item) => ContentChildren::Single(Some(item.as_ref()).into_iter()),
        }
    }

    /// The retraced items, owned.
    pub fn into_items(self) -> Vec<ContentItem<'a>> {
        match self {
            Self::Enclosed(_, items) => items,
            Self::Single(item) => vec![item.into_content_item()],
        }
    }
}

/// Retraced content: words the speaker said then corrected.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Retrace<'a> {
    /// What the marker scopes over.
    pub body: RetraceBody<'a>,
    /// The retrace kind.
    pub kind: RetraceKindParsed,
    /// The retrace marker as written (`[/]`, `[//]`, ...), for placement.
    pub marker: RetraceMarker<'a>,
    /// Non-retrace annotations that followed the retrace marker (e.g., `[?]` after `[/]`).
    /// In grammar.js, annotations attach to `word_with_optional_annotations`, so
    /// they belong to the retrace, not the word inside it.
    pub annotations: Vec<ScopedAnnotationParsed<'a>>,
}

impl Retrace<'_> {
    /// Whether this retrace originated from a `<group> [/]` (angle brackets).
    pub fn is_group(&self) -> bool {
        matches!(self.body, RetraceBody::Enclosed(..))
    }
}

/// Retrace kind, matches grammar.js retrace variants exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RetraceKindParsed {
    /// `[/]`
    Partial,
    /// `[//]`
    Complete,
    /// `[///]`
    Multiple,
    /// `[/-]`
    Reformulation,
}

/// grammar.js: group_with_annotations
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group<'a> {
    /// The `<` and `>` as written.
    pub enclosure: Enclosure<'a>,
    pub contents: Vec<ContentItem<'a>>,
    pub annotations: Vec<ScopedAnnotationParsed<'a>>,
}

/// grammar.js: quotation
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quotation<'a> {
    /// The two quotation marks as written.
    pub enclosure: Enclosure<'a>,
    pub contents: Vec<ContentItem<'a>>,
    /// Markers attached to the quotation, retained by the ordered chain fold.
    pub annotations: Vec<ScopedAnnotationParsed<'a>>,
}

// ═══════════════════════════════════════════════════════════════
// Header ASTs
// ═══════════════════════════════════════════════════════════════

/// A parsed participant header or another header's preserved lexer tokens.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum HeaderParsed<'a> {
    /// Header already admitted to the shared model, including explicit recovery.
    Shared(talkbank_model::model::Header),
    /// Other header families retain their lexer representation for lowering.
    Other {
        /// Original header bytes, excluding the terminal line ending. Recovery
        /// must use this source slice rather than reconstructing lexical payloads.
        #[serde(skip)]
        source: &'a str,
        /// Header discriminator produced by the lexer.
        prefix: Token<'a>,
        /// Content tokens, including whitespace and continuation newlines.
        content: Vec<Token<'a>>,
    },
}

// ═══════════════════════════════════════════════════════════════
// Full file AST
// ═══════════════════════════════════════════════════════════════

pub use file::{
    ChatFile, DependentTierEntryParsed, Line, LocatedHeader, LocatedMainTier, Utterance,
};

/// A parsed dependent tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DependentTierParsed<'a> {
    /// Rejected morphology source, retained in the shared recovery model.
    RejectedMor(talkbank_model::UnparsedTier),
    /// Rejected grammar source, retained in the shared recovery model.
    RejectedGra(talkbank_model::UnparsedTier),
    /// Rejected actual phonology source.
    RejectedPho(talkbank_model::UnparsedTier),
    /// Rejected model phonology source.
    RejectedMod(talkbank_model::UnparsedTier),
    /// Rejected gesture/sign source.
    RejectedSin(talkbank_model::UnparsedTier),
    /// Rejected word-timing source.
    RejectedWor(talkbank_model::UnparsedTier),
    Mor(talkbank_model::MorTier),
    Gra(talkbank_model::GraTier),
    Pho(talkbank_model::PhoTier),
    Mod(talkbank_model::PhoTier),
    Sin(talkbank_model::SinTier),
    /// %wor tier: words with optional inline timing bullets.
    Wor(talkbank_model::WorTier),
    /// Text body rejected before any shortened content can be constructed.
    RejectedText {
        label: TextTierLabel,
    },
    /// Generic text tier (content is raw text segments + bullets).
    Text {
        label: TextTierLabel,
        content: Vec<Token<'a>>,
    },
}

#[cfg(test)]
mod children_tests {
    use super::*;

    fn enclosure(text: &str) -> Enclosure<'_> {
        let (split, _) = text.char_indices().nth(1).expect("two delimiter fixture");
        Enclosure::new(
            crate::source_text::SourceText::new(text),
            &text[..split],
            &text[split..],
        )
        .expect("ordered fixture delimiters")
    }

    /// A distinguishable leaf to look for on the far side of a container.
    fn marker<'a>() -> ContentItem<'a> {
        ContentItem::Pause(talkbank_model::model::Pause::new(
            talkbank_model::Location::Unlocated,
            talkbank_model::model::PauseDuration::Short,
        ))
    }

    fn retrace<'a>(content: Vec<ContentItem<'a>>) -> ContentItem<'a> {
        ContentItem::Retrace(Retrace {
            body: RetraceBody::Enclosed(enclosure("<>"), content),
            kind: RetraceKindParsed::Complete,
            marker: RetraceMarker::new(crate::source_text::SourceText::new("[//]").whole_slice()),
            annotations: vec![],
        })
    }

    /// SURVIVES: policy. WHICH variants of this enum enclose content is a fact
    /// about the CHAT model, not something a signature can carry: nothing stops
    /// `children()` returning `&[]` for a real container, which is exactly the
    /// defect it exists to prevent.
    ///
    /// This is the ONE test the class needs, and it is deliberately on the owner
    /// rather than on each walker. Before `children()` every recursion answered
    /// "which variants nest" itself, so guarding the class meant a test per
    /// walker, and the walker that never got one
    /// (the missing-annotation check) is the one that silently skipped
    /// `PhoGroup` and `SinGroup`.
    ///
    /// Exhaustiveness cannot replace it. A `match` listing every variant and
    /// answering `&[]` for a container compiles clean and passes clippy, so
    /// `_ => &[]` and `PhoGroup(_) => &[]` are the same bug and only the first
    /// is a lint. The compiler guards that every variant is CONSIDERED; this
    /// guards that each is considered CORRECTLY.
    #[test]
    fn every_container_yields_its_children() {
        let cases: Vec<(&str, ContentItem<'_>)> = vec![
            ("Retrace", retrace(vec![marker()])),
            (
                "Group",
                ContentItem::Group(Group {
                    enclosure: enclosure("<>"),
                    contents: vec![marker()],
                    annotations: vec![],
                }),
            ),
            (
                "Quotation",
                ContentItem::Quotation(Quotation {
                    enclosure: enclosure("\u{201C}\u{201D}"),
                    contents: vec![marker()],
                    annotations: vec![],
                }),
            ),
            (
                "PhoGroup",
                ContentItem::PhoGroup(EnclosedGroup {
                    enclosure: enclosure("‹›"),
                    contents: vec![marker()],
                }),
            ),
            (
                "SinGroup",
                ContentItem::SinGroup(EnclosedGroup {
                    enclosure: enclosure("〔〕"),
                    contents: vec![marker()],
                }),
            ),
        ];

        for (name, item) in cases {
            assert_eq!(
                item.children().len(),
                1,
                "{name} encloses content but children() did not yield it; a walker \
                 recursing through children() would silently skip everything inside it"
            );
        }
    }

    /// Every restricted container remains visible through a single retrace.
    #[test]
    fn single_retrace_descent_preserves_each_chain_container() {
        let enclosure = enclosure("<>");
        let cases = [
            Chain::Group(enclosure, vec![marker()], vec![]),
            Chain::Quotation(Quotation {
                enclosure,
                contents: vec![marker()],
                annotations: vec![],
            }),
            Chain::Wrapped(Retrace {
                body: RetraceBody::Enclosed(enclosure, vec![marker()]),
                kind: RetraceKindParsed::Complete,
                marker: RetraceMarker::new(
                    crate::source_text::SourceText::new("[//]").whole_slice(),
                ),
                annotations: vec![],
            }),
        ];
        for chain in cases {
            let outer = ContentItem::Retrace(Retrace {
                body: RetraceBody::Single(Box::new(chain)),
                kind: RetraceKindParsed::Partial,
                marker: RetraceMarker::new(
                    crate::source_text::SourceText::new("[/]").whole_slice(),
                ),
                annotations: vec![],
            });
            let mut children = outer.children();
            assert_eq!(children.len(), 1);
            let child = children.next().expect("single retrace child");
            assert_eq!(children.len(), 0);
            assert!(
                child
                    .children()
                    .any(|item| { matches!(item, ContentRef::Item(ContentItem::Pause(_))) })
            );
        }
    }

    /// SURVIVES: policy. That a leaf yields nothing is the other half of the
    /// same fact, and a `children()` that returned its own item would loop
    /// forever rather than answer wrong.
    #[test]
    fn a_leaf_yields_nothing() {
        assert!(
            ContentItem::Event(
                ParsedEvent::new(
                    crate::source_text::SourceText::new("&=laughs").whole_slice(),
                    talkbank_model::EventType::new("laughs").unwrap(),
                ),
                Vec::new()
            )
            .children()
            .next()
            .is_none()
        );
    }

    /// SURVIVES: behaviour. Descent through `children()` reaches content nested
    /// inside a pho group, which is the precise case
    /// the missing-annotation check used to miss.
    #[test]
    fn descent_reaches_content_inside_a_pho_group() {
        let nested = ContentItem::PhoGroup(EnclosedGroup {
            enclosure: enclosure("‹›"),
            contents: vec![retrace(vec![])],
        });
        let found = nested
            .children()
            .any(|item| matches!(item, ContentRef::Item(ContentItem::Retrace(_))));
        assert!(found, "a retrace inside a pho group must be reachable");
    }
}

#[cfg(test)]
mod text_tier_label_tests {
    use super::TextTierLabel;

    #[test]
    fn prefix_admission_preserves_labels_and_refuses_absence() {
        for (prefix, expected) in [
            ("%com:\t", "com"),
            ("%com", "com"),
            ("%xnote:\t", "xnote"),
            ("%phoaln:\t", "phoaln"),
        ] {
            assert_eq!(
                TextTierLabel::from_prefix_text(prefix).unwrap().as_str(),
                expected
            );
        }
        for prefix in ["", "%", "%:\t"] {
            assert!(TextTierLabel::from_prefix_text(prefix).is_err());
        }
    }
}
