//! Internal selective word rewrite. No whole-document output authority lives here.

use std::ops::Range;

use talkbank_model::model::content::word::LexicalContribution;
use talkbank_model::{Word, WordContent, WordText};

use super::{NameDecision, TranscriptNames};

/// A changed word with pronunciation material needs a separate safe policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordRefusal {
    /// Matched text shares a word with identifying repeated sounds.
    RepeatedMaterial,
    /// Matched text is pronunciation material, not an orthographic name.
    PhoneticMaterial,
    /// The proposed lexical pieces could not bind to the producing source.
    SourceCorrespondence,
    /// Corroborated word spellings have different lexical component structure.
    ComponentCorrespondence,
}

enum RewriteSafety {
    Ordinary,
    Repeated,
    Phonetic,
}

/// Sensitive decisions used by both rewrite planning and private receipts.
/// No Debug or serialization implementation implicitly exposes runtime names.
#[derive(Clone)]
pub enum ComponentDecision {
    /// This component has no mapped name.
    Keep,
    /// Report a case-only match without changing it.
    CaseNearMiss {
        /// Protected input spelling, for an explicitly private receipt only.
        original: String,
    },
    /// Replace exactly the selected typed lexical component.
    Replace {
        /// Protected matched spelling, for an explicitly private receipt only.
        original: String,
        /// Admitted replacement text used by the proposed rewrite.
        replacement: WordText,
    },
}

struct ComponentPlan {
    range: Range<usize>,
    decision: ComponentDecision,
}

/// Decisions bound to one immutable word; apply accepts no substitute word.
pub(super) struct WordPlan<'word> {
    original: &'word Word,
    components: Vec<ComponentPlan>,
}

impl<'word> WordPlan<'word> {
    pub(super) fn original(&self) -> &'word Word {
        self.original
    }

    pub(super) fn assess(
        original: &'word Word,
        names: &TranscriptNames,
    ) -> Result<Self, WordRefusal> {
        Self::assess_with(original, |text| Ok(decide(text, names)))
    }

    /// Carry decisions into an aligned word without consulting the name map.
    /// Cleaned-text agreement alone does not establish compound boundaries.
    pub(super) fn project_onto<'target>(
        &self,
        target: &'target Word,
    ) -> Result<WordPlan<'target>, WordRefusal> {
        let mut components = self.components.iter();
        let projected = WordPlan::assess_with(target, |text| {
            let source = components
                .next()
                .ok_or(WordRefusal::ComponentCorrespondence)?;
            let expected: String = self
                .original
                .lexical_parts()
                .skip(source.range.start)
                .take(source.range.len())
                .filter_map(|part| match part.contribution() {
                    LexicalContribution::Spoken(text) => Some(text),
                    LexicalContribution::Repeated(_) | LexicalContribution::Structural => None,
                })
                .collect();
            if text != expected {
                return Err(WordRefusal::ComponentCorrespondence);
            }
            Ok(source.decision.clone())
        })?;
        if components.next().is_some() {
            return Err(WordRefusal::ComponentCorrespondence);
        }
        Ok(projected)
    }

    fn assess_with(
        original: &'word Word,
        mut decision: impl FnMut(String) -> Result<ComponentDecision, WordRefusal>,
    ) -> Result<Self, WordRefusal> {
        let mut components = Vec::new();
        let mut start = 0;
        let mut text = String::new();
        let mut safety = RewriteSafety::Ordinary;
        for (index, part) in original.lexical_parts().enumerate() {
            if matches!(part.content(), WordContent::Phonetic(_)) {
                safety = RewriteSafety::Phonetic;
            }
            match part.contribution() {
                LexicalContribution::Spoken(piece) => text.push_str(piece),
                LexicalContribution::Repeated(_) => {
                    if matches!(safety, RewriteSafety::Ordinary) {
                        safety = RewriteSafety::Repeated;
                    }
                }
                LexicalContribution::Structural => {}
            }
            if matches!(
                part.content(),
                WordContent::CompoundMarker(_) | WordContent::CliticBoundary(_)
            ) {
                components.push(ComponentPlan {
                    range: start..index,
                    decision: decision(std::mem::take(&mut text))?,
                });
                start = index + 1;
            }
        }
        components.push(ComponentPlan {
            range: start..original.content().len(),
            decision: decision(text)?,
        });
        if components
            .iter()
            .any(|part| matches!(part.decision, ComponentDecision::Replace { .. }))
        {
            match safety {
                RewriteSafety::Ordinary => {}
                RewriteSafety::Repeated => return Err(WordRefusal::RepeatedMaterial),
                RewriteSafety::Phonetic => return Err(WordRefusal::PhoneticMaterial),
            }
        }
        Ok(Self {
            original,
            components,
        })
    }

    pub(super) fn decisions(&self) -> impl Iterator<Item = &ComponentDecision> {
        self.components.iter().map(|part| &part.decision)
    }

    /// Bind the same component decisions used by `apply` to exact source fields.
    /// Preserved markers can sit between those fields; they are never swallowed
    /// into a whole-component replacement range.
    pub(super) fn source_edits<'input>(
        &self,
        parsed: &'input talkbank_parser::generated_traversal::ParsedSource<'_>,
    ) -> Result<Vec<super::LexicalEdit<'input>>, WordRefusal> {
        if !self
            .decisions()
            .any(|decision| matches!(decision, ComponentDecision::Replace { .. }))
        {
            return Ok(Vec::new());
        }
        let fields = super::lexical_source::fields(parsed, self.original)
            .map_err(|_| WordRefusal::SourceCorrespondence)?;
        let mut edits = Vec::new();
        for component in &self.components {
            let ComponentDecision::Replace { replacement, .. } = &component.decision else {
                continue;
            };
            let mut pending = Some(replacement);
            for (index, source) in &fields {
                if component.range.contains(index) {
                    let text = pending
                        .take()
                        .map_or_else(String::new, |text| text.as_ref().to_owned());
                    edits.push(super::LexicalEdit::new(*source, text));
                }
            }
            if pending.is_some() {
                return Err(WordRefusal::SourceCorrespondence);
            }
        }
        Ok(edits)
    }

    /// Derive changed content from the admitted decisions. The original remains
    /// untouched; callers still owe source association and cross-tier agreement.
    pub(super) fn apply(&self) -> Word {
        if !self
            .decisions()
            .any(|decision| matches!(decision, ComponentDecision::Replace { .. }))
        {
            return self.original.clone();
        }
        let mut content = Vec::with_capacity(self.original.content().len());
        let mut cursor = 0;
        for component in &self.components {
            content.extend(
                self.original.content()[cursor..component.range.start]
                    .iter()
                    .cloned(),
            );
            let leaves = &self.original.content()[component.range.clone()];
            match &component.decision {
                ComponentDecision::Keep | ComponentDecision::CaseNearMiss { .. } => {
                    content.extend(leaves.iter().cloned());
                }
                ComponentDecision::Replace { replacement, .. } => {
                    let mut pending = Some(replacement);
                    for leaf in leaves {
                        match leaf {
                            WordContent::Text(_) | WordContent::Shortening(_) => {
                                if let Some(text) = pending.take() {
                                    content.push(WordContent::Text(text.clone()));
                                }
                            }
                            WordContent::Phonetic(_)
                            | WordContent::OverlapPoint(_)
                            | WordContent::CAElement(_)
                            | WordContent::CADelimiter(_)
                            | WordContent::StressMarker(_)
                            | WordContent::Lengthening(_)
                            | WordContent::SyllablePause(_)
                            | WordContent::UnderlineBegin(_)
                            | WordContent::UnderlineEnd(_)
                            | WordContent::CompoundMarker(_)
                            | WordContent::CliticBoundary(_) => content.push(leaf.clone()),
                        }
                    }
                }
            }
            cursor = component.range.end;
        }
        self.original.clone().with_content(content)
    }
}

fn decide(original: String, names: &TranscriptNames) -> ComponentDecision {
    match names.decide(&original) {
        NameDecision::Keep => ComponentDecision::Keep,
        NameDecision::CaseNearMiss => ComponentDecision::CaseNearMiss { original },
        NameDecision::Replace(replacement) => ComponentDecision::Replace {
            original,
            replacement: replacement.clone(),
        },
    }
}

#[cfg(test)]
mod tests;
