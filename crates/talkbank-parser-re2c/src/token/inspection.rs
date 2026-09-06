//! Token text access and lexical error reporting.

use super::Token;

impl<'a> Token<'a> {
    /// Check if this is an error token.
    pub fn is_err(&self) -> bool {
        matches!(
            self,
            Token::ErrorUnrecognized(_)
                | Token::ErrorLine(_)
                | Token::ErrorHeaderAfterName(_)
                | Token::ErrorSpeaker(_)
                | Token::ErrorTierAfterLabel(_)
                | Token::ErrorTierSep(_)
                | Token::ErrorUnclosedParen(_)
                | Token::ErrorInMainContent(_)
                | Token::ErrorInMorContent(_)
                | Token::ErrorInGraContent(_)
                | Token::ErrorInPhoContent(_)
                | Token::ErrorInSinContent(_)
                | Token::ErrorInTierContent(_)
                | Token::ErrorInHeaderContent(_)
                | Token::ErrorInIdContent(_)
                | Token::ErrorInTypesContent(_)
                | Token::ErrorInLanguagesContent(_)
                | Token::ErrorInParticipantsContent(_)
                | Token::ErrorInMediaContent(_)
        )
    }

    /// Human-readable context description for error tokens.
    pub fn error_context(&self) -> Option<&'static str> {
        match self {
            Token::ErrorUnrecognized(_) => Some("unrecognized character"),
            Token::ErrorLine(_) => Some("invalid line (expected @, *, or %)"),
            Token::ErrorHeaderAfterName(_) => {
                Some("expected colon+tab or newline after header name")
            }
            Token::ErrorSpeaker(_) => Some("invalid speaker code"),
            Token::ErrorTierAfterLabel(_) => Some("expected colon+tab after tier label"),
            Token::ErrorTierSep(_) => Some("expected colon+tab after speaker"),
            Token::ErrorUnclosedParen(_) => Some("unclosed parenthesis"),
            Token::ErrorInMainContent(_) => Some("unexpected character in main tier"),
            Token::ErrorInMorContent(_) => Some("unexpected character in %mor tier"),
            Token::ErrorInGraContent(_) => Some("unexpected character in %gra tier"),
            Token::ErrorInPhoContent(_) => Some("unexpected character in %pho tier"),
            Token::ErrorInSinContent(_) => Some("unexpected character in %sin tier"),
            Token::ErrorInTierContent(_) => Some("unexpected character in dependent tier"),
            Token::ErrorInHeaderContent(_) => Some("unexpected character in header"),
            Token::ErrorInIdContent(_) => {
                Some("malformed @ID content (expected 10 pipe-delimited fields)")
            }
            Token::ErrorInTypesContent(_) => {
                Some("malformed @Types content (expected 3 comma-separated fields)")
            }
            Token::ErrorInLanguagesContent(_) => Some("unexpected character in @Languages content"),
            Token::ErrorInParticipantsContent(_) => {
                Some("unexpected character in @Participants content")
            }
            Token::ErrorInMediaContent(_) => Some("unexpected character in @Media content"),
            _ => None,
        }
    }

    /// The text slice this token carries.
    pub fn text(&self) -> &'a str {
        match self {
            Token::Postcode(postcode) => postcode.text(),
            // Use a macro-like approach: every variant carries &str
            Token::BOM(s)
            | Token::Newline(s)
            | Token::Continuation(s)
            | Token::Whitespace(s)
            | Token::HeaderContent(s)
            | Token::HeaderUtf8(s)
            | Token::HeaderBegin(s)
            | Token::HeaderEnd(s)
            | Token::HeaderBlank(s)
            | Token::HeaderNewEpisode(s)
            | Token::Star(s)
            | Token::Speaker(s)
            | Token::IncompleteTierPrefix(s)
            | Token::Period(s)
            | Token::Question(s)
            | Token::Exclamation(s)
            | Token::TrailingOff(s)
            | Token::Interruption(s)
            | Token::SelfInterruption(s)
            | Token::InterruptedQuestion(s)
            | Token::BrokenQuestion(s)
            | Token::QuotedNewLine(s)
            | Token::QuotedPeriodSimple(s)
            | Token::SelfInterruptedQuestion(s)
            | Token::TrailingOffQuestion(s)
            | Token::BreakForCoding(s)
            | Token::CaNoBreak(s)
            | Token::CaTechnicalBreak(s)
            | Token::LinkerLazyOverlap(s)
            | Token::LinkerQuickUptake(s)
            | Token::LinkerQuickUptakeOverlap(s)
            | Token::LinkerQuotationFollows(s)
            | Token::LinkerSelfCompletion(s)
            | Token::CaNoBreakLinker(s)
            | Token::CaTechnicalBreakLinker(s)
            | Token::RetraceComplete(s)
            | Token::RetracePartial(s)
            | Token::RetraceMultiple(s)
            | Token::RetraceReformulation(s)
            | Token::ScopedStressing(s)
            | Token::ScopedContrastiveStressing(s)
            | Token::ScopedUncertain(s)
            | Token::ExcludeMarker(s)
            | Token::CodeSwitchShortcut(s)
            | Token::CodeSwitchExplicit(s)
            | Token::Freecode(s)
            | Token::CaContinuationMarker(s)
            | Token::LongFeatureBegin(s)
            | Token::LongFeatureEnd(s)
            | Token::NonvocalBegin(s)
            | Token::NonvocalEnd(s)
            | Token::NonvocalSimple(s)
            | Token::ErrorMarkerAnnotation(s)
            | Token::OverlapPrecedes(s)
            | Token::OverlapFollows(s)
            | Token::ExplanationAnnotation(s)
            | Token::ParaAnnotation(s)
            | Token::AltAnnotation(s)
            | Token::PercentAnnotation(s)
            | Token::Langcode(s)
            | Token::Replacement(s)
            | Token::PauseLong(s)
            | Token::PauseMedium(s)
            | Token::PauseShort(s)
            | Token::PauseTimed(s)
            | Token::WordSegment(s)
            | Token::Shortening(s)
            | Token::Lengthening(s)
            | Token::StressPrimary(s)
            | Token::StressSecondary(s)
            | Token::OverlapTopBegin(s)
            | Token::OverlapTopEnd(s)
            | Token::OverlapBottomBegin(s)
            | Token::OverlapBottomEnd(s)
            | Token::SyllablePause(s)
            | Token::Tilde(s)
            | Token::CompoundMarker(s)
            | Token::UnderlineBegin(s)
            | Token::UnderlineEnd(s)
            | Token::PrefixFiller(s)
            | Token::PrefixNonword(s)
            | Token::PrefixFragment(s)
            | Token::Event(s)
            | Token::Zero(s)
            | Token::FormMarker(s)
            | Token::PosTag(s)
            | Token::UnknownAnnotation(s)
            | Token::Comma(s)
            | Token::Semicolon(s)
            | Token::Colon(s)
            | Token::TagMarker(s)
            | Token::VocativeMarker(s)
            | Token::UnmarkedEnding(s)
            | Token::UptakeSymbol(s)
            | Token::RisingToHigh(s)
            | Token::RisingToMid(s)
            | Token::LevelPitch(s)
            | Token::FallingToMid(s)
            | Token::FallingToLow(s)
            | Token::LessThan(s)
            | Token::GreaterThan(s)
            | Token::LeftDoubleQuote(s)
            | Token::RightDoubleQuote(s)
            | Token::IllegalCurlyQuote(s)
            | Token::PhoGroupBegin(s)
            | Token::PhoGroupEnd(s)
            | Token::SinGroupBegin(s)
            | Token::SinGroupEnd(s)
            | Token::Ampersand(s)
            | Token::LeftBracket(s)
            | Token::RightBracket(s)
            | Token::CaBlockedSegments(s)
            | Token::CaConstriction(s)
            | Token::CaHardening(s)
            | Token::CaHurriedStart(s)
            | Token::CaInhalation(s)
            | Token::CaLaughInWord(s)
            | Token::CaPitchDown(s)
            | Token::CaPitchReset(s)
            | Token::CaPitchUp(s)
            | Token::CaSuddenStop(s)
            | Token::CaUnsure(s)
            | Token::CaPrecise(s)
            | Token::CaCreaky(s)
            | Token::CaSofter(s)
            | Token::CaSegmentRepetition(s)
            | Token::CaFaster(s)
            | Token::CaSlower(s)
            | Token::CaWhisper(s)
            | Token::CaSinging(s)
            | Token::CaLowPitch(s)
            | Token::CaHighPitch(s)
            | Token::CaLouder(s)
            | Token::CaSmileVoice(s)
            | Token::CaBreathyVoice(s)
            | Token::CaYawn(s)
            | Token::MorTilde(s)
            | Token::PhoWord(s)
            | Token::PhoPlus(s)
            | Token::SinWord(s)
            | Token::LanguageCode(s)
            | Token::ParticipantWord(s)
            | Token::MediaFilename(s)
            | Token::MediaWord(s)
            | Token::TextSegment(s)
            | Token::InlinePic(s)
            | Token::ErrorUnrecognized(s)
            | Token::ErrorLine(s)
            | Token::ErrorHeaderAfterName(s)
            | Token::ErrorSpeaker(s)
            | Token::ErrorTierAfterLabel(s)
            | Token::ErrorTierSep(s)
            | Token::ErrorUnclosedParen(s)
            | Token::ErrorInMainContent(s)
            | Token::ErrorInMorContent(s)
            | Token::ErrorInGraContent(s)
            | Token::ErrorInPhoContent(s)
            | Token::ErrorInSinContent(s)
            | Token::ErrorInTierContent(s)
            | Token::ErrorInHeaderContent(s)
            | Token::ErrorInIdContent(s)
            | Token::ErrorInTypesContent(s)
            | Token::ErrorInLanguagesContent(s)
            | Token::ErrorInParticipantsContent(s)
            | Token::ErrorInMediaContent(s) => s,
            Token::Word { raw_text, .. } => raw_text,
            Token::WordLangSuffix(opt) => opt.unwrap_or("@s"),
            Token::OtherSpokenEvent { speaker, .. } => speaker,
            Token::MediaBullet { raw_text, .. } => raw_text,
            Token::HeaderPrefix(s)
            | Token::HeaderSep(s)
            | Token::HeaderBirthOf(s)
            | Token::HeaderBirthplaceOf(s)
            | Token::HeaderL1Of(s)
            | Token::TierPrefix(s)
            | Token::TierSep(s) => s.text(),
            Token::MorWord { pos, .. } => pos,
            Token::GraRelation { index, .. } => index,
            Token::IdFields { language, .. } => language,
            Token::TypesFields { design, .. } => design,
        }
    }
}

/// Result of lexing a line: tokens + any errors found.
#[derive(Debug)]
pub struct LexResult<'a> {
    pub tokens: Vec<(Token<'a>, std::ops::Range<usize>)>,
}

impl<'a> LexResult<'a> {
    /// Returns all error tokens with their positions and context.
    pub fn errors(&self) -> Vec<LexError<'a>> {
        self.tokens
            .iter()
            .filter(|(t, _)| t.is_err())
            .map(|(t, span)| LexError {
                token: t.clone(),
                span: span.clone(),
                context: t.error_context().unwrap_or("unknown error"),
            })
            .collect()
    }

    /// True if the token stream contains no error tokens.
    pub fn is_clean(&self) -> bool {
        !self.tokens.iter().any(|(t, _)| t.is_err())
    }

    /// Human-readable error report.
    pub fn error_report(&self, source: &str) -> String {
        let errors = self.errors();
        if errors.is_empty() {
            return String::new();
        }
        let mut report = String::new();
        for e in &errors {
            let snippet = &source[e.span.clone()];
            let escaped = snippet.escape_debug().to_string();
            report.push_str(&format!(
                "  [{}-{}] {}: {:?} (text: \"{}\")\n",
                e.span.start, e.span.end, e.context, e.token, escaped
            ));
        }
        report
    }
}

/// A single lexer error with context.
#[derive(Debug, Clone)]
pub struct LexError<'a> {
    pub token: Token<'a>,
    pub span: std::ops::Range<usize>,
    pub context: &'static str,
}
