//! Utterance-level language metadata computation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use super::super::Utterance;

use crate::model::language_metadata::WordLanguages;
use crate::validation::word::language::{GoverningMarker, LanguageResolution};
use crate::{LanguageCode, LanguageMetadata, UtteranceLanguage, UtteranceLanguageMetadata};

/// Convert validation-layer language resolution into persisted metadata representation.
fn resolution_to_metadata_languages(resolution: &LanguageResolution) -> WordLanguages {
    match resolution {
        LanguageResolution::Single(code) => WordLanguages::Single(code.clone()),
        LanguageResolution::Multiple(codes) => WordLanguages::Multiple(codes.clone()),
        LanguageResolution::Ambiguous(codes) => WordLanguages::Ambiguous(codes.clone()),
        LanguageResolution::Unresolved => WordLanguages::Unresolved,
    }
}

impl Utterance {
    /// Compute and store language metadata for all alignable words in this utterance.
    ///
    /// Baseline language is resolved first (`UtteranceLanguage`), then each
    /// alignable word resolves effective language using:
    /// - file default from `@Languages`
    /// - utterance-scoped override (`[- code]`)
    /// - word-level markers (`@s`, `@s:code`, ambiguous/multiple forms)
    ///
    /// # Parameters
    /// - `default_language`: primary language from `@Languages`
    /// - `declared_languages`: full ordered `@Languages` list for disambiguation
    pub fn compute_language_metadata(
        &mut self,
        default_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) {
        // Determine utterance baseline language state.
        self.utterance_language = if let Some(code) = self.main.content.language_code.as_ref() {
            UtteranceLanguage::ResolvedTierScoped { code: code.clone() }
        } else if let Some(code) = default_language {
            UtteranceLanguage::ResolvedDefault { code: code.clone() }
        } else {
            UtteranceLanguage::Unresolved
        };

        let tier_language = self.utterance_language.code();
        let mut metadata = LanguageMetadata::new(tier_language.cloned());

        use crate::alignment::helpers::WordItem;

        // `walk_words` with no domain IS this traversal, and chatter design
        // rule 4 names it as the meaning of in-order main-tier order. With
        // `domain: None` it recurses every container (verified against
        // `descent::descend`, which enters every container when the domain is
        // `None`) and yields word-like leaves.
        //
        // The previous version of this function hand-rolled the same descent,
        // which made it the fifth main-tier traversal in the crate. The bug it
        // was fixing was caused by exactly that: a private walk with its own
        // leaf set, disagreeing with the shared one.
        crate::alignment::helpers::walk_words_scoped(
            &self.main.content.content,
            None,
            &mut |leaf, scope| {
                // The PRODUCED form for a replacement: `dog [: cat]` records
                // the language of what was said, not of the correction.
                let word = match leaf {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    // A separator is not a word and gets no language record.
                    WordItem::Separator(_) => return,
                };
                let governor = GoverningMarker::of(word, scope.span());
                let outcome = governor.resolve(word, tier_language, declared_languages);
                let source = governor.source(&outcome.resolution, &self.utterance_language);
                metadata.add_word(
                    resolution_to_metadata_languages(&outcome.resolution),
                    source,
                );
            },
        );

        self.language_metadata = UtteranceLanguageMetadata::computed(metadata);
    }
}
