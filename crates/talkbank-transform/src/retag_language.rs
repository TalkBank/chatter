//! Retag every occurrence of one language code as another, throughout a file.
//!
//! # Why this is a tool and not a search-and-replace
//!
//! A language code is named by FOUR notations, and a repair that moves some of
//! them leaves a file that contradicts itself:
//!
//! - `@Languages:` declares it;
//! - `[- code]` scopes a whole utterance to it;
//! - `word@s:code` marks one word;
//! - `<a b> [@s:code]` marks a span of words.
//!
//! And the code is a SUBSTRING of ordinary transcript content. Retagging `sun`
//! (the CHAT manual's long-standing slip for Finnish, which is `fin`) in the
//! ESF SwedFinn corpus has to leave the Finnish word "sun", colloquial for
//! "your", untouched, along with `messun`, `asunut` and `sung`. Only the typed
//! model can tell the four notations from the seventy-five occurrences of the
//! word.
//!
//! # Refuses rather than half-finishing
//!
//! The span notation has no mutable walk in the model, so a file naming the
//! code that way is REFUSED, not partially retagged. A tool that silently moves
//! three of four notations produces exactly the self-contradicting file this
//! exists to prevent, and it would do it quietly.

use talkbank_model::alignment::helpers::{WordItemMut, walk_code_switch_spans, walk_words_mut};
use talkbank_model::model::{
    ChatFile, CodeSwitchSpan, Header, LanguageCode, Line, WordLanguageMarker,
};
use talkbank_model::validation::ValidationState;

/// What a retag changed, per notation, so a caller can report the work done
/// rather than the work requested.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RetagStats {
    /// `@Languages` entries rewritten (0 or 1 per file).
    pub declarations: usize,
    /// `[- code]` utterance scopes rewritten.
    pub utterance_scopes: usize,
    /// `word@s:code` markers rewritten.
    pub word_markers: usize,
}

impl RetagStats {
    /// Whether anything changed.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

/// Why a file could not be retagged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetagRefusal {
    /// The file names `from` with a `<...> [@s:code]` span, which this cannot
    /// rewrite. Retagging the other notations would leave the span behind and
    /// the file self-contradicting, so nothing is written.
    NamesCodeInSpan,
}

/// Retag `from` as `to` everywhere in `chat_file`.
///
/// `to` is deduplicated in `@Languages`: retagging a code the file ALREADY
/// declares (the `sun` -> `fin` case, where `fin` is declared alongside it)
/// removes the entry rather than producing a duplicate.
///
/// # Errors
///
/// Returns [`RetagRefusal::NamesCodeInSpan`] and changes NOTHING when the file
/// names `from` in a span notation this cannot rewrite.
pub fn retag_language<S: ValidationState>(
    chat_file: &mut ChatFile<S>,
    from: &LanguageCode,
    to: &LanguageCode,
) -> Result<RetagStats, RetagRefusal> {
    if names_code_in_span(chat_file, from) {
        return Err(RetagRefusal::NamesCodeInSpan);
    }

    let mut stats = RetagStats::default();

    for line in &mut chat_file.lines {
        match line {
            Line::Header { header, .. } => {
                if let Header::Languages { codes } = header.as_mut() {
                    stats.declarations += usize::from(codes.retag(from, to));
                }
            }
            Line::Utterance(utterance) => {
                let content = &mut utterance.main.content;
                if content.language_code.as_ref() == Some(from) {
                    content.language_code = Some(to.clone());
                    stats.utterance_scopes += 1;
                }
                walk_words_mut(content.content.as_mut_slice(), None, &mut |item| {
                    stats.word_markers += retag_word_item(item, from, to);
                });
            }
        }
    }

    // The file-level view mirrors the header, so it moves with it. Both go
    // through the same named operation on the owner.
    chat_file.languages.retag(from, to);

    Ok(stats)
}

/// Whether any `<...> [@s:code]` span names `from`.
fn names_code_in_span<S: ValidationState>(chat_file: &ChatFile<S>, from: &LanguageCode) -> bool {
    let mut found = false;
    for line in &chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        walk_code_switch_spans(&utterance.main.content.content, &mut |span| {
            if let CodeSwitchSpan::Explicit(code) = span
                && code == from
            {
                found = true;
            }
        });
    }
    found
}

/// Retag one word's own `@s:code` marker.
fn retag_word_item(item: WordItemMut<'_>, from: &LanguageCode, to: &LanguageCode) -> usize {
    match item {
        WordItemMut::Word(word) => usize::from(retag_marker(&mut word.lang, from, to)),
        WordItemMut::ReplacedWord(replaced) => {
            let mut n = usize::from(retag_marker(&mut replaced.word.lang, from, to));
            for word in &mut replaced.replacement.words {
                n += usize::from(retag_marker(&mut word.lang, from, to));
            }
            n
        }
        WordItemMut::Separator(_) => 0,
    }
}

/// Retag one `Option<WordLanguageMarker>`, reporting whether it moved.
fn retag_marker(
    marker: &mut Option<WordLanguageMarker>,
    from: &LanguageCode,
    to: &LanguageCode,
) -> bool {
    let Some(WordLanguageMarker::Explicit(code)) = marker.as_mut() else {
        return false;
    };
    if code != from {
        return false;
    }
    *code = to.clone();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    fn parse(source: &str) -> ChatFile {
        let parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");
        let errors = talkbank_model::ErrorCollector::new();
        parser.parse_chat_file_streaming(source, &errors)
    }

    fn code(s: &str) -> LanguageCode {
        LanguageCode::new(s).expect("valid code")
    }

    /// All three notations move together, and the header DEDUPLICATES.
    ///
    /// This is the `sun` -> `fin` shape exactly: the right code is already
    /// declared alongside the wrong one, so the header must lose an entry
    /// rather than gain a duplicate.
    #[test]
    fn every_notation_moves_and_an_already_declared_target_deduplicates() {
        let source = "@UTF8\n@Begin\n@Languages:\tswe, fin, sun\n\
            @Participants:\tCHI Target_Child\n@ID:\tswe|c|CHI|||||Target_Child|||\n\
            *CHI:\t[- sun] ankka .\n*CHI:\tseuraava@s:sun on .\n@End\n";
        let mut file = parse(source);
        let stats = retag_language(&mut file, &code("sun"), &code("fin")).expect("no span");

        assert_eq!(stats.declarations, 1, "the header entry moved");
        assert_eq!(stats.utterance_scopes, 1, "the [- sun] scope moved");
        assert_eq!(stats.word_markers, 1, "the @s:sun marker moved");

        let out = file.to_chat_string();
        assert!(
            out.contains("@Languages:\tswe, fin\n"),
            "deduplicated: {out}"
        );
        assert!(out.contains("[- fin]"), "scope retagged: {out}");
        assert!(out.contains("seuraava@s:fin"), "marker retagged: {out}");
        assert!(!out.contains("sun"), "no `sun` anywhere: {out}");
    }

    /// A code NOT already declared is replaced in place, not removed.
    #[test]
    fn an_undeclared_target_replaces_rather_than_removes() {
        let source = "@UTF8\n@Begin\n@Languages:\tswe, sun\n\
            @Participants:\tCHI Target_Child\n@ID:\tswe|c|CHI|||||Target_Child|||\n\
            *CHI:\tankka@s:sun .\n@End\n";
        let mut file = parse(source);
        retag_language(&mut file, &code("sun"), &code("fin")).expect("no span");
        assert!(
            file.to_chat_string().contains("@Languages:\tswe, fin\n"),
            "replaced in place: {}",
            file.to_chat_string()
        );
    }

    /// The ordinary WORD "sun", colloquial Finnish for "your", is untouched.
    ///
    /// This is why the tool exists rather than a replace: the code is a
    /// substring of real transcript content, 27 times in the corpus this was
    /// built for, plus inside `asunut`, `messun` and `sung`.
    #[test]
    fn the_word_that_spells_the_code_is_untouched() {
        let source = "@UTF8\n@Begin\n@Languages:\tswe, fin, sun\n\
            @Participants:\tCHI Target_Child\n@ID:\tswe|c|CHI|||||Target_Child|||\n\
            *CHI:\tnyt olis sun vuoro asunut@s:sun .\n@End\n";
        let mut file = parse(source);
        retag_language(&mut file, &code("sun"), &code("fin")).expect("no span");

        let out = file.to_chat_string();
        assert!(
            out.contains("nyt olis sun vuoro"),
            "the word survives: {out}"
        );
        assert!(out.contains("asunut@s:fin"), "only the marker moved: {out}");
    }

    /// A file naming the code in a span is REFUSED and left alone, rather than
    /// three-quarters retagged into self-contradiction.
    #[test]
    fn a_span_notation_is_refused_and_changes_nothing() {
        let source = "@UTF8\n@Begin\n@Languages:\tswe, sun\n\
            @Participants:\tCHI Target_Child\n@ID:\tswe|c|CHI|||||Target_Child|||\n\
            *CHI:\t<ankka on> [@s:sun] .\n@End\n";
        let mut file = parse(source);
        let before = file.to_chat_string();
        assert_eq!(
            retag_language(&mut file, &code("sun"), &code("fin")),
            Err(RetagRefusal::NamesCodeInSpan)
        );
        assert_eq!(file.to_chat_string(), before, "nothing was changed");
    }
}
