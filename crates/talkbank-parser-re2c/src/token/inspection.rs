//! Token text access and lexical error reporting.

use super::Token;

impl<'a> Token<'a> {
    /// Check if this is an error token.
    pub fn is_err(&self) -> bool {
        self.error_context().is_some()
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

    /// The token's content: the whole lexeme for a plain token, the payload
    /// (not the lexeme) for an `Extracted` one, the whole suffix for a word's
    /// `@s` suffix, the delimited raw text for a
    /// media bullet, the SPEAKER for an interposed-speaker prefix, and the leading
    /// field for the rich header and `%mor`/`%gra` tokens. Placement never
    /// goes through this; it reads the lexeme the token carries.
    pub fn text(&self) -> &'a str {
        match self {
            Token::Postcode(postcode) => postcode.text(),
            Token::CodeSwitchExplicit(x)
            | Token::Freecode(x)
            | Token::ErrorMarkerAnnotation(x)
            | Token::OverlapPrecedes(x)
            | Token::OverlapFollows(x)
            | Token::ExplanationAnnotation(x)
            | Token::ParaAnnotation(x)
            | Token::AltAnnotation(x)
            | Token::PercentAnnotation(x)
            | Token::Langcode(x)
            | Token::Event(x)
            | Token::FormMarker(x)
            | Token::PosTag(x)
            | Token::LongFeatureBegin(x)
            | Token::LongFeatureEnd(x)
            | Token::NonvocalBegin(x)
            | Token::NonvocalEnd(x)
            | Token::NonvocalSimple(x)
            | Token::UnknownAnnotation(x)
            | Token::InlinePic(x) => x.text(),
            Token::PauseLong(pause)
            | Token::PauseMedium(pause)
            | Token::PauseShort(pause)
            | Token::PauseTimed(pause) => pause.text(),
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
            | Token::CaContinuationMarker(s)
            | Token::ReplacementBegin(s)
            | Token::Lengthening(s)
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
            | Token::Zero(s)
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
            Token::WordLangSuffix(suffix) => suffix.lexeme(),
            Token::OtherSpokenPrefix(prefix) => prefix.text(),
            Token::MediaBullet { bullet } => bullet.text(),
            Token::HeaderPrefix(s) => s.text(),
            Token::HeaderSep(s)
            | Token::HeaderBirthOf(s)
            | Token::HeaderBirthplaceOf(s)
            | Token::HeaderL1Of(s)
            | Token::TierSep(s) => s.text(),
            Token::TierPrefix(s) => s.text(),
            Token::MorWord { pos, .. } => pos,
            Token::GraRelation { index, .. } => index,
            Token::IdFields { language, .. } => language,
            Token::TypesFields { design, .. } => design,
        }
    }
}

/// Source-bound result of lexing a line, with read-only tokens and error reports.
///
/// Reports cannot take a different source:
/// ```compile_fail
/// let result = talkbank_parser_re2c::lex("");
/// result.error_report("unrelated");
/// ```
/// Token ranges cannot be replaced after lexing:
/// ```compile_fail
/// let mut result = talkbank_parser_re2c::lex("");
/// result.tokens()[0].1 = 0..1;
/// ```
#[derive(Debug)]
pub struct LexResult<'a> {
    source: &'a str,
    tokens: Vec<(Token<'a>, std::ops::Range<usize>)>,
}

impl<'a> LexResult<'a> {
    /// Produce tokens and their source together at the lexer boundary.
    pub(crate) fn lex(source: &'a str, condition: crate::lexer::StartCondition) -> Self {
        Self {
            source,
            tokens: crate::lexer::Lexer::new(source, condition).collect(),
        }
    }

    /// Borrow tokens and their byte ranges in the original source.
    pub fn tokens(&self) -> &[(Token<'a>, std::ops::Range<usize>)] {
        &self.tokens
    }

    /// Returns all error tokens with their positions and context.
    pub fn errors(&self) -> Vec<LexError<'a>> {
        self.tokens
            .iter()
            .filter_map(|(token, span)| {
                Some(LexError {
                    context: token.error_context()?,
                    token: token.clone(),
                    span: span.clone(),
                })
            })
            .collect()
    }

    /// True if the token stream contains no error tokens.
    pub fn is_clean(&self) -> bool {
        !self.tokens.iter().any(|(t, _)| t.is_err())
    }

    /// Human-readable error report.
    pub fn error_report(&self) -> String {
        let errors = self.errors();
        if errors.is_empty() {
            return String::new();
        }
        let mut report = String::new();
        for e in &errors {
            let snippet = &self.source[e.span.clone()];
            let escaped = snippet.escape_debug().to_string();
            report.push_str(&format!(
                "  [{}-{}] {}: {:?} (text: \"{}\")\n",
                e.span.start, e.span.end, e.context, e.token, escaped
            ));
        }
        report
    }
}

/// An error token, source span and context admitted together by [`LexResult`].
///
/// Consumers cannot replace the admitted token with a successful token:
/// ```compile_fail
/// use talkbank_parser_re2c::{lex, token::Token};
/// let mut error = lex("oops\n").errors().remove(0);
/// error.token = Token::Zero("0");
/// ```
#[derive(Debug, Clone)]
pub struct LexError<'a> {
    token: Token<'a>,
    span: std::ops::Range<usize>,
    context: &'static str,
}

impl<'a> LexError<'a> {
    /// The error token emitted by the lexer.
    pub fn token(&self) -> &Token<'a> {
        &self.token
    }

    /// Byte range in the lexed source.
    pub fn span(&self) -> std::ops::Range<usize> {
        self.span.clone()
    }

    /// Context admitted by the token's error classification.
    pub fn context(&self) -> &'static str {
        self.context
    }
}

#[cfg(test)]
mod source_binding_tests {
    #[test]
    fn report_uses_original_unicode_source() {
        let source = "😀oops\n";
        let result = crate::lex(source);
        assert!(!result.is_clean());
        let report = result.error_report();
        assert!(report.contains("[0-8]"), "{report}");
        assert!(report.contains("😀oops"), "{report}");
        assert!(report.contains("invalid line"), "{report}");
        assert_eq!(result.tokens()[0].1, 0..8);
        let errors = result.errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span(), 0..8);
        assert_eq!(errors[0].context(), "invalid line (expected @, *, or %)");
        assert!(matches!(
            errors[0].token(),
            super::Token::ErrorLine("😀oops")
        ));
    }

    #[test]
    fn empty_source_has_no_error_report() {
        let result = crate::lex("");
        assert!(result.is_clean());
        assert!(result.error_report().is_empty());
    }
}
