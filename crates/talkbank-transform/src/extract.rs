//! NLP word extraction from CHAT AST.
//!
//! Walks the parsed CHAT content tree and collects words that are
//! "alignable" for a given domain (Mor, Pho, Sin).
//!
//! # One walk, three sinks
//!
//! `visit_extractable` (private) is the one owner of WHICH items are
//! extracted and where a `%mor` run ends. It rides the model's bounded word walk
//! ([`walk_words_bounded`]), which hands over every word-like leaf and a
//! boundary wherever something else stood, and turns that into a stream of
//! `Step`s: a word that may join its neighbours, an item that stands alone,
//! or a break. Three sinks consume that one stream:
//!
//! - [`collect_utterance_content`] pushes every extracted item into a flat
//!   vector and ignores run ends;
//! - [`collect_mor_runs`] pushes the same items, built by the same function,
//!   into runs that close at every run end;
//! - [`count_utterance_content`] counts the items without building them.
//!
//! Because the flat vector is the run partition with its run ends forgotten,
//! flattening the runs yields the flat extraction exactly: there is no second
//! classification of content items that could disagree with the first.

use std::num::NonZeroUsize;

use talkbank_model::alignment::helpers::{
    BoundedWordStep, LanguageScope, PositionalDomain, TierDomain, WordItem,
    annotations_have_alignment_ignore, counts_for_tier, is_tag_marker_separator,
    should_align_replaced_word_in_pho_sin, walk_words_bounded,
};
use talkbank_model::model::{
    ChatFile, LanguageCode, ReplacedWord, Separator, Utterance, UtteranceContent, Word,
};
use talkbank_model::validation::{GoverningMark, GoverningMarkKind, LanguageResolutionOutcome};
use talkbank_model::{ChatCleanedText, ChatRawText, SpeakerCode, UtteranceIdx, WordIdx};

/// A word extracted from the CHAT AST for NLP processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedWord {
    /// Cleaned text suitable for NLP (no CHAT markers).
    pub text: ChatCleanedText,
    /// Raw text as it appeared in the transcript.
    pub raw_text: ChatRawText,
    /// Zero-based index among extracted alignable words in this utterance.
    pub utterance_word_index: WordIdx,
    /// Special form marker if the word has @c, @b, @s, etc.
    pub form_type: Option<talkbank_model::model::FormType>,
    /// The mark that GOVERNS this word's language: its own `@s`, an enclosing
    /// `<...> [@s]` span, or the utterance.
    ///
    /// **Not a bare `Option<WordLanguageMarker>` carrying only the word's own
    /// mark, and the difference is a bug.** Extraction WALKS the tree, so it
    /// knows whether a `<...> [@s:hin]` span encloses a word; carrying only the
    /// own-marker discarded that, and Batchalign's morphotag consequently saw
    /// every unmarked word inside a Hindi span as unlanguaged, dropped it from
    /// second-language dispatch, and tagged it against the tier language.
    ///
    /// [`GoverningMark`] carries the word's span with it, so
    /// [`ExtractedWord::resolve_language`] needs no span argument and a caller
    /// cannot pair this mark with a different word's position.
    /// PRIVATE, and that is the point rather than encapsulation for its own
    /// sake. While it was `pub`, `a.language = b.language.clone()` moved one
    /// word's mark and position onto another in a single line, so the claim
    /// that a mark "cannot be paired with a different word's position" was
    /// false at the struct level even though `GoverningMark::resolve` takes no
    /// span. Read it through [`ExtractedWord::language_kind`] or resolve it
    /// through [`ExtractedWord::resolve_language`].
    language: GoverningMark,
}

impl ExtractedWord {
    /// Which kind of mark governs this word's language.
    ///
    /// The payload-free view, for consumers that must branch on the kind
    /// (Batchalign restores a word's surface text only when the WORD itself
    /// carried markers) without being handed a mark they could move elsewhere.
    #[must_use]
    pub fn language_kind(&self) -> GoverningMarkKind {
        self.language.kind()
    }

    /// Resolve this word's language.
    ///
    /// The operation belongs on the word rather than on the mark, so that the
    /// mark and the position it resolves at cannot come from different words.
    #[must_use]
    pub fn resolve_language(
        &self,
        tier_language: Option<&LanguageCode>,
        declared_languages: talkbank_model::DeclaredLanguagesRef<'_>,
    ) -> LanguageResolutionOutcome {
        self.language
            .resolve_with_declaration(tier_language, Ok(declared_languages))
    }

    /// Resolve while retaining the declaration's explicit admission failure.
    pub fn resolve_language_with_declaration(
        &self,
        tier_language: Option<&LanguageCode>,
        declaration: Result<
            talkbank_model::DeclaredLanguagesRef<'_>,
            talkbank_model::model::LanguageDeclarationError,
        >,
    ) -> LanguageResolutionOutcome {
        self.language
            .resolve_with_declaration(tier_language, declaration)
    }
}

/// Per-utterance extraction result.
#[derive(Debug, Clone)]
pub struct ExtractedUtterance {
    /// Speaker code (e.g., "CHI", "MOT").
    pub speaker: SpeakerCode,
    /// Zero-based utterance index in the file.
    pub utterance_index: UtteranceIdx,
    /// Extracted words.
    pub words: Vec<ExtractedWord>,
}

/// Extract NLP-ready words from all utterances in a ChatFile.
///
/// Walks every utterance in the file and collects words that are
/// "alignable" for the given `domain`. Non-utterance lines (headers,
/// comments, etc.) are skipped.
///
/// * `chat_file` - The parsed CHAT file to extract words from.
/// * `domain` - The alignment domain governing which words are
///   considered alignable (`Mor`, `Pho`, or `Sin`).
pub fn extract_words(chat_file: &ChatFile, domain: PositionalDomain) -> Vec<ExtractedUtterance> {
    indexed_utterances(chat_file)
        .map(|(utterance_index, utterance)| {
            let mut words = Vec::new();
            collect_utterance_content(&utterance.main.content.content, domain, &mut words);
            ExtractedUtterance {
                speaker: SpeakerCode::new(&utterance.main.speaker),
                utterance_index,
                words,
            }
        })
        .collect()
}

/// Collect NLP-extractable words from a slice of utterance content items.
///
/// This is the inner workhorse called by [`extract_words`] and also used
/// directly by other modules (morphosyntax, utseg, translate, coref) to
/// extract words from a single utterance's content without iterating the
/// entire file.
///
/// * `content` - The top-level content items of an utterance.
/// * `domain` - The alignment domain that determines which words are
///   collected (e.g., `Mor` includes tag-marker separators; `Pho` does not).
///   `%wor` is not a value of the type: its words are the projection's
///   (`MainTier::wor_projection`), the one owner of that selection.
/// * `out` - Accumulator that extracted words are pushed into.
///
/// For `Mor`, the words pushed are exactly the words of
/// [`collect_mor_runs`], in the same order and with the same values: both are
/// sinks on one extraction stream, and this one forgets where runs end.
pub fn collect_utterance_content(
    content: &[UtteranceContent],
    domain: PositionalDomain,
    out: &mut Vec<ExtractedWord>,
) {
    extract_into(content, domain, out);
}

/// How many items [`collect_utterance_content`] pushes for `content` in
/// `domain`, without building any of them.
///
/// The same walk as extraction, with a counter for its sink, so the count is
/// extraction's by construction rather than by agreement with another walk's
/// rules. Building each item costs two owned strings, which a caller that
/// needs only the number should not pay for.
#[must_use]
pub fn count_utterance_content(content: &[UtteranceContent], domain: PositionalDomain) -> usize {
    let mut count = 0usize;
    visit_extractable(content, domain, &mut |step| match step {
        Step::Joining(..) | Step::Alone(_) => count += 1,
        Step::Break => {}
    });
    count
}

// ---------------------------------------------------------------------------
// Run-partitioned %mor extraction
// ---------------------------------------------------------------------------

/// One run of `%mor` words: a non-empty sequence of extracted words that a
/// tokenizer may treat as one stretch of text.
///
/// # The run contract
///
/// A run is a maximal sequence of plain, retokenizable `%mor` words in the
/// same container with no other main-tier content between them. Each of
/// these is a run of its own: an annotated word that counts (`b [!]`,
/// `b [*]`, `b [= x]`, `b [=! x]`), each counting word of a replacement
/// (`b [: c d]` gives `c` and `d` one run each), and a tag-marker separator.
/// These end a run without contributing to one: retraces, untranscribed
/// material (`xxx`, `yyy`, `www`) and every other word `%mor` does not count,
/// pauses, events, actions, freecodes, overlap points, internal bullets,
/// long-feature, underline and nonvocal markers, a scope whose annotations
/// exclude it from `%mor`, and every container edge.
///
/// # Why `<b c> [!]` is one run of two and `b [!]` is a run of one
///
/// Both follow from one rule: **a run never crosses the edge of a scope.** A
/// scoped annotation on a group annotates the group, so `<b c> [!]` is one
/// scope holding two plain words, and the run `b c` lies inside it. A scoped
/// annotation written on one word opens a scope of that ONE word (the walk
/// already reads `hallo [@s]` exactly as `<hallo> [@s]`), so `b [!]` is a
/// scope whose only run is `b`. The same rule makes
/// `a <b &+c [=! laughs]> [!] d .` three runs of one: `a` before the group's
/// opening edge; `b`, ended by the fragment `&+c`, which is itself an
/// annotated scope and a word `%mor` does not count; and `d` after the
/// group's closing edge.
///
/// Non-emptiness is structural: a run is created holding its first word and
/// only ever grows, so no empty run can be built and no consumer checks for
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorRun {
    /// PRIVATE so a run can only come from extraction: non-empty, and
    /// partitioned by the walk that chose the words.
    words: Vec<ExtractedWord>,
}

impl MorRun {
    /// A run holding exactly `first`. The only constructor.
    fn starting_with(first: ExtractedWord) -> Self {
        // Room for a typical run, so its second word does not reallocate.
        let mut words = Vec::with_capacity(4);
        words.push(first);
        Self { words }
    }

    /// The run's words, in utterance order. Never empty.
    #[must_use]
    pub fn words(&self) -> &[ExtractedWord] {
        &self.words
    }

    /// How many words the run holds: at least one.
    fn word_count(&self) -> NonZeroUsize {
        // `starting_with` is the only constructor and holds one word, and
        // words are only ever appended, so the length is never zero. Stated
        // without a panic: the minimum plus the words after the first.
        NonZeroUsize::MIN.saturating_add(self.words.len().saturating_sub(1))
    }
}

/// One utterance's `%mor` words, partitioned into [`MorRun`]s.
///
/// The runs are the words: there is no separate word list or length vector
/// beside them to fall out of step. [`MorRuns::words`] flattens them, and the
/// flattened sequence is exactly what [`collect_utterance_content`] extracts
/// for [`PositionalDomain::Mor`] (same words, same order, same values,
/// including each word's `utterance_word_index`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorRuns {
    /// PRIVATE so the partition can only come from extraction.
    runs: Vec<MorRun>,
}

impl MorRuns {
    /// The runs, in utterance order. Empty when the utterance has no `%mor`
    /// word.
    #[must_use]
    pub fn runs(&self) -> &[MorRun] {
        &self.runs
    }

    /// Every word of every run, in utterance order: the flat `%mor`
    /// extraction.
    pub fn words(&self) -> impl Iterator<Item = &ExtractedWord> {
        self.runs.iter().flat_map(|run| run.words.iter())
    }

    /// The length of each run, in order.
    pub fn run_lengths(&self) -> impl Iterator<Item = NonZeroUsize> + '_ {
        self.runs.iter().map(MorRun::word_count)
    }
}

/// One utterance's `%mor` runs with the utterance they came from.
///
/// The run-partitioned counterpart of [`ExtractedUtterance`]. The runs and
/// the utterance identity travel together and are read through accessors, so
/// a consumer cannot pair one utterance's runs with another's identity, or
/// with words taken from a different call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedMorUtterance {
    speaker: SpeakerCode,
    utterance_index: UtteranceIdx,
    runs: MorRuns,
}

impl ExtractedMorUtterance {
    /// Speaker code (e.g., "CHI", "MOT").
    #[must_use]
    pub fn speaker(&self) -> &SpeakerCode {
        &self.speaker
    }

    /// Zero-based utterance index in the file.
    #[must_use]
    pub fn utterance_index(&self) -> UtteranceIdx {
        self.utterance_index
    }

    /// The utterance's `%mor` words, partitioned into runs.
    #[must_use]
    pub fn runs(&self) -> &MorRuns {
        &self.runs
    }
}

/// Extract every utterance's `%mor` words, partitioned into runs.
///
/// The run-partitioned form of `extract_words(chat_file, PositionalDomain::Mor)`:
/// one entry per utterance, in file order, with the same speaker and index,
/// and runs whose flattened words equal that entry's `words`. See [`MorRun`]
/// for where runs end.
#[must_use]
pub fn extract_mor_runs(chat_file: &ChatFile) -> Vec<ExtractedMorUtterance> {
    indexed_utterances(chat_file)
        .map(|(utterance_index, utterance)| ExtractedMorUtterance {
            speaker: SpeakerCode::new(&utterance.main.speaker),
            utterance_index,
            runs: collect_mor_runs(&utterance.main.content.content),
        })
        .collect()
}

/// Extract one utterance's `%mor` words from its content, partitioned into
/// runs. See [`MorRun`] for the contract.
///
/// The run-partitioned form of
/// `collect_utterance_content(content, PositionalDomain::Mor, &mut words)`
/// into an empty `words`: flattening the result gives exactly those words.
#[must_use]
pub fn collect_mor_runs(content: &[UtteranceContent]) -> MorRuns {
    let mut builder = MorRunsBuilder::default();
    extract_into(content, PositionalDomain::Mor, &mut builder);
    builder.finish()
}

// ---------------------------------------------------------------------------
// The one extraction stream and its sinks
// ---------------------------------------------------------------------------

/// Every utterance of the file with its zero-based index, in file order.
/// Shared by both per-file entry points so their indices cannot differ.
fn indexed_utterances(chat_file: &ChatFile) -> impl Iterator<Item = (UtteranceIdx, &Utterance)> {
    chat_file
        .utterances()
        .enumerate()
        .map(|(index, utterance)| (UtteranceIdx::new(index), utterance))
}

/// One item extraction emits, before it is built.
enum Extractable<'a> {
    /// An alignable word, with the language scope enclosing it.
    Word(&'a Word, LanguageScope<'a>),
    /// A tag-marker separator, which `%mor` aligns as a position.
    TagSeparator(&'a Separator, LanguageScope<'a>),
}

/// One step of the extraction stream.
///
/// The run decision is made HERE, at the point where the item is chosen,
/// rather than by a consumer re-inspecting the items afterwards.
enum Step<'a> {
    /// A plain counting word, which shares a run with the plain counting
    /// words next to it. Only a word can join: a joining separator is not a
    /// value of this type.
    Joining(&'a Word, LanguageScope<'a>),
    /// An extracted item that is a run by itself: a counting word of a
    /// replacement (two of which must not join each other, though the walk
    /// sees them as one leaf) or a tag-marker separator. An annotated word
    /// arrives as `Joining` between two breaks, which the walk's scope edges
    /// supply.
    Alone(Extractable<'a>),
    /// Something that is not extracted stood here; the current run ends.
    Break,
}

/// Hand `sink` every step extraction takes for `content` in `domain`, in
/// document order. The one owner of WHICH leaves are extracted and of where a
/// run ends; building, run partitioning and counting are its sinks.
fn visit_extractable<'a>(
    content: &'a [UtteranceContent],
    domain: PositionalDomain,
    sink: &mut impl FnMut(Step<'a>),
) {
    // The SCOPED walk, in its bounded form. A walk that discards the enclosing
    // `<...> [@s]` span loses what downstream NLP needs to resolve a word's
    // language, and one that discards boundaries loses where runs end.
    walk_words_bounded(content, Some(domain.into()), &mut |step| match step {
        BoundedWordStep::Leaf(leaf, scope) => match leaf {
            WordItem::Word(word) => visit_alignable_word(word, domain, scope, sink),
            WordItem::ReplacedWord(replaced) => {
                visit_replaced_word(replaced, domain, scope, sink);
            }
            WordItem::Separator(sep) => visit_separator(sep, domain, scope, sink),
        },
        BoundedWordStep::Boundary => sink(Step::Break),
    });
}

/// Where extracted words go, in order, and where runs end.
///
/// Two implementations: the flat accumulator, which ignores run ends, and the
/// run builder. Both receive every word from [`push_extracted`], so they hold
/// the same words with the same indices and differ only in partitioning.
trait ExtractionSink {
    /// How many words this sink has received; the next word's index.
    fn received(&self) -> usize;
    /// Accept the next word.
    fn push(&mut self, word: ExtractedWord);
    /// The current run, if one is open, is complete.
    fn end_run(&mut self);
}

/// The flat accumulator. Run ends mean nothing to it.
///
/// `received` is the vector's length, as the index always was: a caller that
/// accumulates several calls into one vector keeps the numbering it had.
impl ExtractionSink for Vec<ExtractedWord> {
    fn received(&self) -> usize {
        self.len()
    }

    fn push(&mut self, word: ExtractedWord) {
        Vec::push(self, word);
    }

    fn end_run(&mut self) {}
}

/// Builds a [`MorRuns`] from the extraction stream.
#[derive(Default)]
struct MorRunsBuilder {
    /// Runs already closed.
    closed: Vec<MorRun>,
    /// The run being extended, if any. `None` is "between runs", a state of
    /// the builder rather than an empty run: an empty run is not
    /// representable.
    open: Option<MorRun>,
    /// Words received so far, across all runs.
    received: usize,
}

impl MorRunsBuilder {
    /// Close any open run and hand over the partition.
    fn finish(mut self) -> MorRuns {
        self.end_run();
        MorRuns { runs: self.closed }
    }
}

impl ExtractionSink for MorRunsBuilder {
    fn received(&self) -> usize {
        self.received
    }

    fn push(&mut self, word: ExtractedWord) {
        self.received += 1;
        match &mut self.open {
            Some(run) => run.words.push(word),
            None => self.open = Some(MorRun::starting_with(word)),
        }
    }

    fn end_run(&mut self) {
        if let Some(run) = self.open.take() {
            self.closed.push(run);
        }
    }
}

/// Run the extraction stream for `content` in `domain` into `sink`.
///
/// The one place a [`Step`] becomes words and run ends: a joining word is
/// pushed into the open run, an item that stands alone is fenced by run ends
/// on both sides, and a break ends the open run.
fn extract_into(
    content: &[UtteranceContent],
    domain: PositionalDomain,
    sink: &mut impl ExtractionSink,
) {
    visit_extractable(content, domain, &mut |step| match step {
        Step::Joining(word, scope) => push_word(sink, word, scope),
        Step::Alone(item) => {
            sink.end_run();
            match item {
                Extractable::Word(word, scope) => push_word(sink, word, scope),
                Extractable::TagSeparator(sep, scope) => push_extracted(
                    sink,
                    ChatCleanedText::from_separator(sep),
                    ChatRawText::from_separator(sep),
                    None,
                    // A tag separator is not a word, so there is no precedence
                    // question: only the enclosing scope to record.
                    GoverningMark::of_separator(sep, scope.span()),
                ),
            }
            sink.end_run();
        }
        Step::Break => sink.end_run(),
    });
}

/// Push one extracted item, deriving its index from the sink.
///
/// THE ONLY PLACE `utterance_word_index` IS COMPUTED. Five call sites built the
/// struct literally, each repeating `WordIdx::new(out.len())`, and a test
/// existed solely to notice if one drifted out of step: a test whose only job
/// is to detect that two things diverged, which is the tell for a missing
/// owner.
///
/// The first version of this owned only the WORD sites and left the separator
/// site writing the index by hand, so the count went from five to two rather
/// than to one, and the test was deleted on the strength of an ownership that
/// did not yet exist. The public `synthetic` constructor, which took the index
/// as a PARAMETER and had no callers at all, was a third route and is gone.
///
/// `language` is private, so `ExtractedWord { .. }` is unwritable outside this
/// module, and the only literal in the codebase is the one below. The flat and
/// run sinks both receive their words here, which is why a run partition's
/// flattened indices are the flat extraction's indices.
fn push_extracted(
    sink: &mut impl ExtractionSink,
    text: ChatCleanedText,
    raw_text: ChatRawText,
    form_type: Option<talkbank_model::model::FormType>,
    language: GoverningMark,
) {
    let utterance_word_index = WordIdx::new(sink.received());
    sink.push(ExtractedWord {
        text,
        raw_text,
        utterance_word_index,
        form_type,
        language,
    });
}

/// Push one extracted WORD, with the mark its enclosing scope implies.
fn push_word(sink: &mut impl ExtractionSink, word: &Word, scope: LanguageScope<'_>) {
    push_extracted(
        sink,
        ChatCleanedText::from_word(word),
        ChatRawText::from_word_raw(word),
        word.form_type().cloned(),
        GoverningMark::of(word, scope.span()),
    );
}

/// A bare or annotated word leaf: it joins its neighbours when it counts in
/// the domain, and ends the run when it does not (`xxx`, `&+w`, `&-um` under
/// `%mor`).
fn visit_alignable_word<'a>(
    word: &'a Word,
    domain: PositionalDomain,
    scope: LanguageScope<'a>,
    sink: &mut impl FnMut(Step<'a>),
) {
    // The scoped walk owns annotated-word exclusion before yielding this leaf,
    // and fences an annotated word with boundaries, so this arm needs no
    // knowledge of annotations. Replacement annotations travel on
    // ReplacedWord and are checked separately.
    if counts_for_tier(word, domain.into()) {
        sink(Step::Joining(word, scope));
    } else {
        sink(Step::Break);
    }
}

/// A separator leaf: a tag marker is a `%mor` position and a run of its own;
/// any other separator, or any separator outside `%mor`, ends the run.
fn visit_separator<'a>(
    sep: &'a Separator,
    domain: PositionalDomain,
    scope: LanguageScope<'a>,
    sink: &mut impl FnMut(Step<'a>),
) {
    match domain {
        PositionalDomain::Mor if is_tag_marker_separator(sep) => {
            sink(Step::Alone(Extractable::TagSeparator(sep, scope)));
        }
        PositionalDomain::Mor | PositionalDomain::Pho | PositionalDomain::Sin => {
            sink(Step::Break);
        }
    }
}

/// A replaced-word leaf. Under `%mor` each counting replacement word is a
/// run of its own; under `%pho` and `%sin` the spoken original is one item.
fn visit_replaced_word<'a>(
    entry: &'a ReplacedWord,
    domain: PositionalDomain,
    scope: LanguageScope<'a>,
    sink: &mut impl FnMut(Step<'a>),
) {
    // The walk fences a replaced word with boundaries on both sides, and
    // `Step::Alone` fences each replacement word, so nothing here needs to
    // emit a break of its own: an item that contributes nothing simply
    // leaves the walk's edges adjacent.
    if domain == PositionalDomain::Mor
        && annotations_have_alignment_ignore(&entry.scoped_annotations)
    {
        return;
    }

    match domain {
        PositionalDomain::Mor => {
            for word in &entry.replacement.words {
                if counts_for_tier(word, TierDomain::Mor) {
                    sink(Step::Alone(Extractable::Word(word, scope)));
                }
            }
        }
        PositionalDomain::Pho | PositionalDomain::Sin => {
            if should_align_replaced_word_in_pho_sin(entry) {
                sink(Step::Alone(Extractable::Word(&entry.word, scope)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::alignment::helpers::{PositionalDomain, count_tier_positions};
    use talkbank_model::validation::GoverningMarkKind;
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::test_error::ParseProductTestExt;

    /// Parse a CHAT string into a ChatFile, panicking on errors (test-only).
    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        parser.parse_chat_file(text).expect_built()
    }

    /// Minimal valid CHAT with one utterance containing the given main tier text.
    fn one_utterance(main_tier: &str) -> String {
        format!(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             *CHI:\t{main_tier}\n\
             @End\n"
        )
    }

    /// Counting is extraction's own walk with a counter for a sink: on reference
    /// CHAT carrying replacements, retraces, groups and tag-marker separators,
    /// every item in every domain counts exactly what collecting it pushes.
    #[test]
    fn counting_matches_what_extraction_pushes_item_by_item() {
        let sources = [
            include_str!("../../../corpus/reference/annotation/errors-and-replacements.cha"),
            include_str!("../../../corpus/reference/annotation/retrace.cha"),
            include_str!("../../../corpus/reference/annotation/groups-regular.cha"),
            include_str!("../../../corpus/reference/content/separators.cha"),
        ];
        let mut tag_separators = 0;
        for source in sources {
            let chat = parse_chat(source);
            for utterance in chat.utterances() {
                for item in &utterance.main.content.content {
                    let item = std::slice::from_ref(item);
                    for domain in [
                        PositionalDomain::Mor,
                        PositionalDomain::Pho,
                        PositionalDomain::Sin,
                    ] {
                        let mut words = Vec::new();
                        collect_utterance_content(item, domain, &mut words);
                        assert_eq!(count_utterance_content(item, domain), words.len());
                        tag_separators += words
                            .iter()
                            .filter(|word| word.raw_text.as_str() == "\u{201E}")
                            .count();
                    }
                }
            }
        }
        // The corpus reaches the separator branch, not only the word branches.
        assert!(tag_separators > 0);
    }

    // -----------------------------------------------------------------------
    // Basic word extraction
    // -----------------------------------------------------------------------

    #[test]
    fn simple_words_in_mor_domain() {
        let chat = parse_chat(&one_utterance("hello world ."));
        let result = extract_words(&chat, PositionalDomain::Mor);
        assert_eq!(result.len(), 1, "expected 1 utterance");
        assert_eq!(result[0].words.len(), 2, "expected 2 words (hello, world)");
        assert_eq!(result[0].words[0].text.as_str(), "hello");
        assert_eq!(result[0].words[1].text.as_str(), "world");
    }

    #[test]
    fn utterance_indices_are_sequential_across_file() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             *CHI:\tbye .\n\
             @End\n",
        );
        let result = extract_words(&chat, PositionalDomain::Mor);
        assert_eq!(result.len(), 3, "expected 3 utterances");
        assert_eq!(result[0].utterance_index, UtteranceIdx::new(0));
        assert_eq!(result[1].utterance_index, UtteranceIdx::new(1));
        assert_eq!(result[2].utterance_index, UtteranceIdx::new(2));
    }

    #[test]
    fn speaker_code_extracted_correctly() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             @End\n",
        );
        let result = extract_words(&chat, PositionalDomain::Mor);
        assert_eq!(result[0].speaker.as_str(), "CHI");
        assert_eq!(result[1].speaker.as_str(), "MOT");
    }

    #[test]
    fn non_utterance_lines_are_skipped() {
        // Headers and comments are not utterances, only *SPK: lines count.
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @Comment:\tthis is a comment\n\
             *CHI:\thello .\n\
             @End\n",
        );
        let result = extract_words(&chat, PositionalDomain::Mor);
        assert_eq!(result.len(), 1, "only the utterance, not the comment");
    }

    // -----------------------------------------------------------------------
    // Tag-marker separators: included in Mor, excluded from Wor/Pho/Sin
    // -----------------------------------------------------------------------

    #[test]
    fn comma_separator_included_in_mor_domain() {
        // CHAT: comma (,) between words is a tag-marker separator in Mor domain.
        let chat = parse_chat(&one_utterance("well , hello ."));
        let mor_result = extract_words(&chat, PositionalDomain::Mor);
        // Mor domain: "well", ",", "hello" = 3 items
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&","),
            "Mor domain should include comma separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_included_in_mor_domain() {
        // „ (U+201E) is the tag separator
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let mor_result = extract_words(&chat, PositionalDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"„"),
            "Mor domain should include tag separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn vocative_separator_included_in_mor_domain() {
        // ‡ (U+2021) is the vocative separator
        let chat = parse_chat(&one_utterance("‡ Mom ."));
        let mor_result = extract_words(&chat, PositionalDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"‡"),
            "Mor domain should include vocative separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_excluded_from_pho_domain() {
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let pho_result = extract_words(&chat, PositionalDomain::Pho);
        let pho_texts: Vec<&str> = pho_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            !pho_texts.contains(&"„"),
            "Pho domain should NOT include tag separator, got: {pho_texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ReplacedWord: Mor uses replacement; Pho/Sin/Wor use original
    // -----------------------------------------------------------------------

    #[test]
    fn replaced_word_uses_replacement_in_mor_domain() {
        // CHAT: "doggie [: dog]", in Mor domain, "dog" is used (the replacement).
        let chat = parse_chat(&one_utterance("doggie [: dog] ."));
        let result = extract_words(&chat, PositionalDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"dog"),
            "Mor domain should use replacement word 'dog', got: {texts:?}"
        );
    }

    #[test]
    fn replaced_word_uses_intended_alternative_in_mor() {
        let chat = parse_chat(&one_utterance("goed [: went] ."));
        let result = extract_words(&chat, PositionalDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"went"),
            "Mor domain should use replacement 'went', got: {texts:?}"
        );
        assert!(
            !texts.contains(&"goed"),
            "Mor domain should NOT use original 'goed', got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Alignment-ignore annotation ([e]) excludes words in Mor domain
    // -----------------------------------------------------------------------

    #[test]
    fn exclude_annotation_skips_word_in_mor_domain() {
        // [e] marks excluded content, skipped in Mor domain.
        let chat = parse_chat(&one_utterance("hello [e] world ."));
        let result = extract_words(&chat, PositionalDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        // "hello" should be excluded by [e]; "world" should remain.
        // Note: [e] applies to the preceding word.
        assert!(
            texts.contains(&"world"),
            "world should be present, got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Empty utterances
    // -----------------------------------------------------------------------

    #[test]
    fn empty_utterance_produces_empty_word_list() {
        // Utterance with only a terminator, no words.
        let chat = parse_chat(&one_utterance("0 ."));
        let result = extract_words(&chat, PositionalDomain::Mor);
        // "0" is a special CHAT symbol; whether it produces a word depends on
        // counts_for_tier(). We just verify no crash and at most 1 word.
        assert_eq!(result.len(), 1, "still produces 1 utterance");
    }

    /// A span governs the language of every word it encloses, and a word's own
    /// marker still wins inside it.
    ///
    /// Asserts BOTH which mark governs and what it resolves to. The kinds
    /// alone would pass if resolution were broken, and the languages alone
    /// would pass if a span were mistaken for a word's own marker, since both
    /// spell `Single(hin)` here.
    ///
    /// A test rather than a type: nothing in a signature can say the traversal
    /// actually reached the span. Its predecessor also asserted that a word's
    /// own marker beats the span, as a separate case; that was pinning an
    /// owned/borrowed MIRROR faithfully reproducing `GoverningMarker::of`'s
    /// precedence. The mirror is gone (extraction stores the owned mark
    /// directly), so the only thing left to check is the end-to-end answer, and
    /// one utterance exercises both halves.
    #[test]
    fn a_span_governs_the_words_it_encloses_and_an_own_marker_still_wins() {
        let file = parse_chat(&one_utterance("I said <rocket@s:eng kyaa hai> [@s:hin] ."));
        let utterances = extract_words(&file, PositionalDomain::Mor);
        let words = &utterances[0].words;

        let eng = talkbank_model::model::LanguageCode::new("eng").expect("valid code");
        let hin = talkbank_model::model::LanguageCode::new("hin").expect("valid code");
        let declared = [eng.clone()];

        use talkbank_model::validation::LanguageResolution::Single;
        // Keyed by the word's own text, so a failure names the word rather than
        // printing parallel vectors and a subscript to count.
        let expected = [
            ("I", GoverningMarkKind::Utterance, Single(eng.clone())),
            ("said", GoverningMarkKind::Utterance, Single(eng.clone())),
            ("rocket", GoverningMarkKind::Own, Single(eng.clone())),
            ("kyaa", GoverningMarkKind::Span, Single(hin.clone())),
            ("hai", GoverningMarkKind::Span, Single(hin.clone())),
        ];
        assert_eq!(words.len(), expected.len(), "word count");

        for (word, (text, kind, language)) in words.iter().zip(&expected) {
            assert_eq!(word.text.as_str(), *text, "word order");
            assert_eq!(word.language_kind(), *kind, "which mark governs `{text}`");
            assert_eq!(
                word.resolve_language(
                    Some(&eng),
                    talkbank_model::DeclaredLanguagesRef::new(&declared).unwrap()
                )
                .resolution,
                *language,
                "resolved language of `{text}`"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Run-partitioned %mor extraction
    // -----------------------------------------------------------------------

    /// The run lengths of one main tier, after checking that the runs are the
    /// flat `%mor` extraction partitioned (same words, same values, same
    /// order) and that the per-file entry point agrees with the per-content
    /// one.
    fn mor_run_lengths(main_tier: &str) -> Vec<usize> {
        let chat = parse_chat(&one_utterance(main_tier));
        let utterance = chat.utterances().next().expect("one utterance");
        let content = &utterance.main.content.content;

        let runs = collect_mor_runs(content);
        let mut flat = Vec::new();
        collect_utterance_content(content, PositionalDomain::Mor, &mut flat);
        assert_eq!(
            runs.words().cloned().collect::<Vec<_>>(),
            flat,
            "flattened runs are the flat extraction for `{main_tier}`"
        );
        assert_eq!(
            count_utterance_content(content, PositionalDomain::Mor),
            flat.len()
        );

        let per_file = extract_mor_runs(&chat);
        assert_eq!(per_file.len(), 1);
        assert_eq!(per_file[0].runs(), &runs);
        assert_eq!(per_file[0].utterance_index(), UtteranceIdx::new(0));
        assert_eq!(per_file[0].speaker().as_str(), "CHI");

        runs.run_lengths().map(NonZeroUsize::get).collect()
    }

    /// The twelve shapes of the run contract, each with its expected run
    /// lengths. Synthetic words, so a failure names the shape, not a corpus
    /// file. The last four are the shapes a consumer's own second walk got
    /// wrong: an annotated word that `%mor` does not count, inside and
    /// outside an annotated group.
    #[test]
    fn mor_runs_follow_the_run_contract() {
        let shapes: [(&str, &[usize]); 12] = [
            // A retrace ends the run; the retraced word is not extracted.
            ("a b [/] b c d .", &[1, 3]),
            ("a <b c> [/] b c d .", &[1, 3]),
            // Untranscribed material ends the run.
            ("a b xxx c .", &[2, 1]),
            // Pauses and events end the run.
            ("a (.) b &=laughs c .", &[1, 1, 1]),
            // An annotated group is one scope: its plain words run together.
            ("a <b c> [!] d .", &[1, 2, 1]),
            ("a b c d .", &[4]),
            // Each counting replacement word is a run of its own.
            ("a b [: c d] e f .", &[1, 1, 1, 2]),
            // An annotated word is a scope of one.
            ("a b [*] c d .", &[1, 1, 2]),
            // Annotated words `%mor` does not count end the run.
            ("a &+w [=! laughs] .", &[1]),
            ("a b xxx [*] .", &[2]),
            ("a &-um [=! shrugs] b .", &[1, 1]),
            ("a <b &+c [=! laughs]> [!] d .", &[1, 1, 1]),
        ];
        for (main_tier, expected) in shapes {
            assert_eq!(
                mor_run_lengths(main_tier),
                expected,
                "runs of `{main_tier}`"
            );
        }
    }

    /// Beyond the twelve: a tag-marker separator stands alone, and a word its
    /// annotations exclude from `%mor` ends the run without joining it.
    #[test]
    fn mor_runs_fence_tag_separators_and_excluded_words() {
        assert_eq!(mor_run_lengths("a , b c ."), [1, 1, 2]);
        assert_eq!(mor_run_lengths("a b [e] c ."), [1, 1]);
        assert_eq!(mor_run_lengths("0 ."), Vec::<usize>::new());
    }

    /// Over every utterance of the reference corpus, the runs are the flat
    /// `%mor` extraction partitioned: flattening them gives exactly the words
    /// `collect_utterance_content` pushes, value for value, and every run is
    /// non-empty. The corpus covers groups, retraces, replacements, quotations
    /// and separators that the synthetic shapes do not combine.
    #[test]
    fn mor_runs_partition_the_flat_extraction_over_the_reference_corpus() {
        let corpus =
            talkbank_parser_tests::chat_corpus::ChatCorpus::reference().expect("reference corpus");
        let (mut utterances, mut words, mut runs) = (0usize, 0usize, 0usize);
        for fixture in corpus.fixtures() {
            let chat = parse_chat(fixture.source());
            let flat_file = extract_words(&chat, PositionalDomain::Mor);
            let runs_file = extract_mor_runs(&chat);
            assert_eq!(
                flat_file.len(),
                runs_file.len(),
                "{}",
                fixture.path().display()
            );
            for ((flat, partitioned), utterance) in
                flat_file.iter().zip(&runs_file).zip(chat.utterances())
            {
                // The words extraction hands a tagger are the positions the
                // aligner pairs with `%mor` items, one for one, so a consumer
                // that writes `%mor` by position from these runs writes it
                // where alignment reads it.
                assert_eq!(
                    flat.words.len(),
                    count_tier_positions(&utterance.main.content.content, PositionalDomain::Mor),
                    "{} utterance {:?}: extraction and %mor alignment positions",
                    fixture.path().display(),
                    flat.utterance_index
                );
                assert_eq!(flat.utterance_index, partitioned.utterance_index());
                assert_eq!(&flat.speaker, partitioned.speaker());
                assert_eq!(
                    partitioned.runs().words().cloned().collect::<Vec<_>>(),
                    flat.words,
                    "{} utterance {:?}",
                    fixture.path().display(),
                    flat.utterance_index
                );
                assert!(
                    partitioned
                        .runs()
                        .runs()
                        .iter()
                        .all(|run| !run.words().is_empty()),
                    "empty run in {}",
                    fixture.path().display()
                );
                utterances += 1;
                words += flat.words.len();
                runs += partitioned.runs().runs().len();
            }
        }
        // Printed so a reader of the test log sees what was measured.
        println!("reference corpus: {utterances} utterances, {words} %mor words, {runs} runs");
        assert!(utterances > 0 && words > runs && runs > 0);
    }
}
