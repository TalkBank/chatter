use talkbank_model::{ErrorCollector, FragmentSemanticContext, LanguageCode, ParseOutcome};
use talkbank_parser::TreeSitterParser;

use super::{BuildChatError, TranscriptDescription};

/// Admitted nonempty language declaration and semantic context for one build.
pub(super) struct BuildChatContext<'a> {
    description: &'a TranscriptDescription,
    parser: TreeSitterParser,
    primary_lang: LanguageCode,
    additional_langs: Vec<LanguageCode>,
    semantics: FragmentSemanticContext,
    media_header: Option<talkbank_model::model::MediaHeader>,
}

impl<'a> BuildChatContext<'a> {
    /// Parse the stated languages and capture options once at the boundary.
    pub(super) fn new(desc: &'a TranscriptDescription) -> Result<Self, BuildChatError> {
        if desc.participants.is_empty() {
            return Err(BuildChatError::NoParticipants);
        }
        let (primary, additional) = desc
            .langs
            .split_first()
            .ok_or(BuildChatError::NoLanguages)?;
        let parser = TreeSitterParser::new()
            .map_err(|e| BuildChatError::Build(format!("Failed to create parser: {e}")))?;
        let parse = |language: &String| {
            LanguageCode::new(language).map_err(|e| {
                BuildChatError::Build(format!(
                    "invalid @Languages language code {language:?}: {e}"
                ))
            })
        };
        let primary_lang = parse(primary)?;
        let additional_langs = additional
            .iter()
            .map(parse)
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(participant) = desc
            .participants
            .iter()
            .find(|participant| participant.corpus.is_empty())
        {
            return Err(BuildChatError::EmptyCorpus {
                speaker: participant.id.clone(),
            });
        }

        Ok(Self {
            media_header: super::headers::admit_media_header(desc)?,
            description: desc,
            parser,
            primary_lang,
            additional_langs,
            semantics: FragmentSemanticContext::new().with_option_flags(
                desc.options
                    .iter()
                    .flat_map(|options| options.iter().cloned())
                    .collect(),
            ),
        })
    }

    pub(super) fn description(&self) -> &'a TranscriptDescription {
        self.description
    }

    /// Media values admitted once from this description, never reparsed at output.
    pub(super) fn media_header(&self) -> Option<&talkbank_model::model::MediaHeader> {
        self.media_header.as_ref()
    }

    /// Parse only through this description's declared semantic context.
    pub(super) fn parse_utterance(
        &self,
        input: &str,
    ) -> Result<talkbank_model::model::Utterance, String> {
        let errors = ErrorCollector::new();
        match self
            .parser
            .parse_utterance_fragment_with_context(input, 0, &self.semantics, &errors)
        {
            ParseOutcome::Parsed(utterance) if !errors.has_errors() => Ok(utterance),
            _ => Err(format!("failed to parse utterance: {:?}", errors.to_vec())),
        }
    }

    pub(super) fn langs(&self) -> impl Iterator<Item = &LanguageCode> {
        std::iter::once(&self.primary_lang).chain(&self.additional_langs)
    }

    pub(super) fn primary_lang(&self) -> &LanguageCode {
        &self.primary_lang
    }
}
