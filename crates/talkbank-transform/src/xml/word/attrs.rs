//! Stateless attribute-lookup helpers for word-level XML emission.

use quick_xml::events::BytesStart;

use talkbank_model::model::{
    CADelimiter, CADelimiterType, CAElement, CAElementType, FormType, RetraceKind, Separator,
    Terminator, Word,
};

/// Map the three [`Separator`] variants that render as `<tagMarker
/// type="…"/>` per the XSD: `comma`, `tag`, `vocative`. These are
/// the separators that participate in `%mor` alignment. All other
/// separator variants render as `<s type="…"/>` via
/// [`s_separator_label`].
pub(crate) fn separator_tag_type(sep: &Separator) -> Option<&'static str> {
    Some(match sep {
        Separator::Comma { .. } => "comma",
        Separator::Tag { .. } => "tag",
        Separator::Vocative { .. } => "vocative",
        _ => return None,
    })
}

/// Map every [`Terminator`] variant to its `<t type="…"/>` attribute
/// value per `baseTerminatorType` in `talkbank.xsd`. CA intonation
/// arrows and CA TCU markers are NOT terminators (they're
/// ``Separator`` variants); the corresponding XML rendering happens
/// in the separator path.
pub(crate) fn terminator_type_attr(terminator: &Terminator) -> &'static str {
    match terminator {
        Terminator::Period { .. } => "p",
        Terminator::Question { .. } => "q",
        Terminator::Exclamation { .. } => "e",
        Terminator::TrailingOff { .. } => "trail off",
        Terminator::TrailingOffQuestion { .. } => "trail off question",
        Terminator::BrokenQuestion { .. } => "question exclamation",
        Terminator::Interruption { .. } => "interruption",
        Terminator::InterruptedQuestion { .. } => "interruption question",
        Terminator::SelfInterruption { .. } => "self interruption",
        Terminator::SelfInterruptedQuestion { .. } => "self interruption question",
        Terminator::QuotedNewLine { .. } => "quotation next line",
        Terminator::QuotedPeriodSimple { .. } => "quotation precedes",
        Terminator::BreakForCoding { .. } => "broken for coding",
    }
}

/// Map the separator variants that serialize as utterance-boundary
/// `<t type="…"/>` markers rather than ordinary content `<s/>`
/// markers.
pub(crate) fn separator_terminator_type_attr(sep: &Separator) -> Option<&'static str> {
    Some(match sep {
        Separator::CaTechnicalBreak { .. } => "technical break TCU continuation",
        Separator::CaNoBreak { .. } => "no break TCU continuation",
        _ => return None,
    })
}

/// Map the [`Separator`] variants that render as `<s type="…"/>`
/// per `talkbank.xsd`. Covers semicolon/colon (structural),
/// CA intonation contours, uptake, unmarked ending, and
/// CaContinuation (`[^c]`, best-matched to `clause delimiter`).
/// The TCU continuation markers (`≈`, `≋`) are handled separately
/// as utterance-boundary `<t/>` values via
/// [`separator_terminator_type_attr`].
pub(crate) fn s_separator_label(sep: &Separator) -> Option<&'static str> {
    Some(match sep {
        Separator::Semicolon { .. } => "semicolon",
        Separator::Colon { .. } => "colon",
        Separator::CaContinuation { .. } => "clause delimiter",
        Separator::RisingToHigh { .. } => "rising to high",
        Separator::RisingToMid { .. } => "rising to mid",
        Separator::Level { .. } => "level",
        Separator::FallingToMid { .. } => "falling to mid",
        Separator::FallingToLow { .. } => "falling to low",
        Separator::UnmarkedEnding { .. } => "unmarked ending",
        Separator::Uptake { .. } => "uptake",
        _ => return None,
    })
}

/// Map [`talkbank_model::model::WordCategory`] to the `<w type="...">`
/// attribute value used by TalkBank XML. Returns `None` for
/// [`WordCategory::CAOmission`], `(parens)` is not an omission in
/// the schema sense; it renders as an all-content `<shortening>`
/// wrapper instead. See `emit_word_contents`.
pub(crate) fn word_category_attr(
    cat: &talkbank_model::model::WordCategory,
) -> Option<&'static str> {
    use talkbank_model::model::WordCategory;
    match cat {
        WordCategory::Omission => Some("omission"),
        WordCategory::CAOmission => None,
        WordCategory::Nonword => Some("nonword"),
        WordCategory::Filler => Some("filler"),
        WordCategory::PhonologicalFragment => Some("fragment"),
    }
}

/// Map [`talkbank_model::model::content::word::UntranscribedStatus`]
/// to the `<w untranscribed="...">` attribute value. Enum values
/// follow the XSD `<xs:enumeration>` on the `untranscribed`
/// attribute of `<w>`:
///
/// | CHAT | UntranscribedStatus | XML value |
/// |------|---------------------|-----------|
/// | `xxx` | `Unintelligible` | `"unintelligible"` |
/// | `yyy` | `Phonetic` | `"unintelligible-with-pho"` |
/// | `www` | `Untranscribed` | `"untranscribed"` |
pub(crate) fn untranscribed_attr(
    status: talkbank_model::model::content::word::UntranscribedStatus,
) -> &'static str {
    use talkbank_model::model::content::word::UntranscribedStatus;
    match status {
        UntranscribedStatus::Unintelligible => "unintelligible",
        UntranscribedStatus::Phonetic => "unintelligible-with-pho",
        UntranscribedStatus::Untranscribed => "untranscribed",
    }
}

/// Map a [`FormType`] to its `<w formType="…"/>` attribute value per
/// `talkbank.xsd`. Returns `None` for `FormType::UserDefined`, that
/// variant projects onto `user-special-form` instead and is handled
/// by the caller. The mapping is the CHAT `@-marker` → schema label
/// correspondence (e.g. `@b` → `"babbling"`, `@sas` → `"sign speech"`).
/// Apply the word-level `formType` / `user-special-form` attribute to
/// an open `<w>` element. Called from every place that emits an outer
/// `<w>`, plain `emit_word`, `emit_replaced_word` (top-level), and the
/// retrace-wrapped replaced-word path in `emit_bracketed_word_only`.
///
/// `FormType::UserDefined(String)` owns its label, so `code.as_str()`
/// can be passed through without cloning, the string lives as long as
/// `word.form_type` does, which is longer than `start.push_attribute`.
pub(crate) fn push_form_type_attrs(start: &mut BytesStart<'_>, form_type: Option<&FormType>) {
    let Some(form) = form_type else { return };
    if let Some(attr_value) = form_type_attr(form) {
        start.push_attribute(("formType", attr_value));
    } else if let FormType::UserDefined(code) = form {
        start.push_attribute(("user-special-form", code.as_str()));
    }
}

pub(crate) fn form_type_attr(form: &FormType) -> Option<&'static str> {
    Some(match form {
        // `@a` has no XSD enum of its own, doc string in Rust calls
        // it "approximate / phonologically consistent", matching the
        // XSD `"phonology consistent"` semantics. Collapse to that.
        FormType::A | FormType::P => "phonology consistent",
        FormType::B => "babbling",
        FormType::C => "child-invented",
        FormType::D => "dialect",
        FormType::F => "family-specific",
        FormType::FP => "filled pause",
        FormType::G => "generic",
        FormType::I => "interjection",
        FormType::K => "kana",
        FormType::L => "letter",
        FormType::LS => "letter plural",
        FormType::N => "neologism",
        FormType::O => "onomatopoeia",
        FormType::Q => "quoted metareference",
        FormType::SAS => "sign speech",
        FormType::SI => "singing",
        FormType::SL => "signed language",
        FormType::T => "test",
        FormType::U => "UNIBET",
        FormType::WP => "word play",
        FormType::X => "words to be excluded",
        FormType::UserDefined(_) => return None,
    })
}

/// CHAT-spec-strict check for the `untranscribed` XML attribute: only
/// the lowercase placeholders `xxx`, `yyy`, `www` trigger it. Bypasses
/// the model's case-insensitive `untranscribed()` helper (which is a
/// Stanza/MOR workaround, not the XML schema rule).
pub(crate) fn untranscribed_attribute_for_xml(word: &Word) -> Option<&'static str> {
    use talkbank_model::model::content::word::UntranscribedStatus;
    match word.cleaned_text() {
        "xxx" => Some(untranscribed_attr(UntranscribedStatus::Unintelligible)),
        "yyy" => Some(untranscribed_attr(UntranscribedStatus::Phonetic)),
        "www" => Some(untranscribed_attr(UntranscribedStatus::Untranscribed)),
        _ => None,
    }
}

/// Map a [`RetraceKind`] to the `<k type="…"/>` attribute value per
/// the `<k>` XSD enum. The enum variant names and the CHAT manual
/// names for each marker don't line up (historical drift in the
/// Rust model); the CHAT→XSD mapping below follows the XSD's own
/// documentation comments rather than the variant names:
///
/// | CHAT notation | Model variant | XSD `<k type=>` value |
/// |---|---|---|
/// | `[/]` | `Partial` | `retracing` |
/// | `[//]` | `Full` | `retracing with correction` |
/// | `[///]` | `Multiple` | `retracing reformulation` |
/// | `[/-]` | `Reformulation` | `false start` |
pub(crate) fn retrace_kind_attr(kind: RetraceKind) -> &'static str {
    match kind {
        RetraceKind::Partial => "retracing",
        RetraceKind::Full => "retracing with correction",
        RetraceKind::Multiple => "retracing reformulation",
        RetraceKind::Reformulation => "false start",
    }
}

/// Short display name for a [`ContentAnnotation`] variant. Used
/// inside `FeatureNotImplemented` diagnostics so the harness surfaces
/// each staged annotation kind as a distinct increment.
/// Map a [`CAElementType`] to the `<ca-element type="…"/>` attribute
/// value used by TalkBank XML. Values are lowercase with a space
/// separator where the Rust enum uses CamelCase (e.g. `PitchUp` →
/// `"pitch up"`, `BlockedSegments` → `"blocked segments"`).
pub(crate) fn ca_element_label(element: &CAElement) -> &'static str {
    match element.element_type {
        CAElementType::BlockedSegments => "blocked segments",
        CAElementType::Constriction => "constriction",
        CAElementType::Hardening => "hardening",
        CAElementType::HurriedStart => "hurried start",
        CAElementType::Inhalation => "inhalation",
        CAElementType::LaughInWord => "laugh in word",
        CAElementType::PitchDown => "pitch down",
        CAElementType::PitchReset => "pitch reset",
        CAElementType::PitchUp => "pitch up",
        CAElementType::SuddenStop => "sudden stop",
    }
}

/// Assign a unique `[0, 15)` bit index to each [`CADelimiterType`]
/// variant so `emit_word_contents` can track begin/end state in a
/// `u16` bitset instead of a per-word `HashMap` allocation.
pub(crate) fn ca_delimiter_bit_index(ty: CADelimiterType) -> u8 {
    match ty {
        CADelimiterType::Faster => 0,
        CADelimiterType::Slower => 1,
        CADelimiterType::Softer => 2,
        CADelimiterType::Louder => 3,
        CADelimiterType::LowPitch => 4,
        CADelimiterType::HighPitch => 5,
        CADelimiterType::SmileVoice => 6,
        CADelimiterType::BreathyVoice => 7,
        CADelimiterType::Unsure => 8,
        CADelimiterType::Whisper => 9,
        CADelimiterType::Yawn => 10,
        CADelimiterType::Singing => 11,
        CADelimiterType::SegmentRepetition => 12,
        CADelimiterType::Creaky => 13,
        CADelimiterType::Precise => 14,
    }
}

/// Map a [`CADelimiter`] to the `<ca-delimiter label="…"/>` attribute
/// value TalkBank XML emits. Labels are not a mechanical
/// transformation of the variant name, the pitch pair uses hyphens
/// (`"low-pitch"`), most two-word variants use spaces
/// (`"smile voice"`, `"breathy voice"`), and `SegmentRepetition`
/// renames to `"repeated-segment"`. Kept as an explicit table so the
/// mapping is visible.
pub(crate) fn ca_delimiter_label(delimiter: &CADelimiter) -> &'static str {
    match delimiter.delimiter_type {
        CADelimiterType::Faster => "faster",
        CADelimiterType::Slower => "slower",
        CADelimiterType::Softer => "softer",
        CADelimiterType::Louder => "louder",
        CADelimiterType::LowPitch => "low-pitch",
        CADelimiterType::HighPitch => "high-pitch",
        CADelimiterType::SmileVoice => "smile voice",
        CADelimiterType::BreathyVoice => "breathy voice",
        CADelimiterType::Unsure => "unsure",
        CADelimiterType::Whisper => "whisper",
        CADelimiterType::Yawn => "yawn",
        CADelimiterType::Singing => "singing",
        CADelimiterType::SegmentRepetition => "repeated-segment",
        CADelimiterType::Creaky => "creaky",
        CADelimiterType::Precise => "precise",
    }
}
