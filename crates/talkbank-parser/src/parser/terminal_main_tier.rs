//! Lower a grammar-proven flattened main tier at EOF without reparsing text.

use crate::generated_traversal::{
    AsRawNode, ColonNode, ContentsNode, FromNodeKind, ParsedSource, SpeakerNode, StarNode, TabNode,
    TerminatorChoice,
};
use crate::parser::tree_parsing::main_tier::structure::{
    contents::parse_main_tier_contents, terminator::terminator_from_new_choice,
};
use talkbank_model::model::MainTier;
use talkbank_model::{ErrorSink, ParseOutcome, Span};
use tree_sitter::Node;

/// Structural admission retains actual grammar nodes, not a text fragment
/// masquerading as a parsed main tier. Lowering additionally binds each node
/// to the original parse owner before constructing any model.
pub(crate) struct TerminalMainTier<'tree> {
    first: Node<'tree>,
    speaker: SpeakerNode<'tree>,
    contents: ContentsNode<'tree>,
    ending: TerminatorChoice<'tree>,
    colon: ColonNode<'tree>,
    tab: TabNode<'tree>,
}

impl<'tree> TerminalMainTier<'tree> {
    pub(crate) fn admit(
        first: Node<'tree>,
        mut remaining: impl Iterator<Item = Node<'tree>>,
        source: &str,
    ) -> Option<Self> {
        StarNode::from_node(first)?;
        let speaker = SpeakerNode::from_node(remaining.next()?)?;
        let colon = ColonNode::from_node(remaining.next()?)?;
        let tab = TabNode::from_node(remaining.next()?)?;
        let contents = ContentsNode::from_node(remaining.next()?)?;
        let ending = TerminatorChoice::from_node(remaining.next()?)?;
        if remaining.next().is_some()
            || ending.raw_node().end_byte() != source.len()
            || speaker.raw_node().is_missing()
            || ending.raw_node().is_missing()
        {
            return None;
        }
        source.get(first.start_byte()..ending.raw_node().end_byte())?;
        Some(Self {
            first,
            speaker,
            contents,
            ending,
            colon,
            tab,
        })
    }

    pub(crate) fn lower(
        self,
        parsed: &'tree ParsedSource<'_>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        // Refusal is reported once and cannot yield a model.
        let bound = (|| {
            parsed.bind(self.first)?;
            parsed.bind_typed(self.colon)?;
            parsed.bind_typed(self.tab)?;
            let speaker = parsed.bind_typed(self.speaker)?;
            let contents = parsed.bind_typed(self.contents)?;
            parsed.bind(self.ending.raw_node())?;
            Ok::<_, crate::generated_traversal::SourceBindingError>((speaker, contents))
        })();
        let (speaker, contents) = match bound {
            Ok(bound) => bound,
            Err(error) => {
                crate::parser::typed_cst::report_cst_failure(
                    self.first,
                    parsed.source(),
                    error,
                    errors,
                );
                return ParseOutcome::Rejected;
            }
        };
        let end = self.ending.raw_node().end_byte();
        let Ok(content) = crate::parser::typed_cst::report_reconstruction(
            parse_main_tier_contents(contents, errors),
            contents.raw_node(),
            contents.source(),
            errors,
        ) else {
            return ParseOutcome::Rejected;
        };
        let mut main = MainTier::new(
            speaker.text().to_owned(),
            content,
            Some(terminator_from_new_choice(&self.ending)),
        )
        .with_span(Span::from_usize(self.first.start_byte(), end))
        .with_speaker_span(Span::from_usize(
            self.speaker.raw_node().start_byte(),
            self.speaker.raw_node().end_byte(),
        ))
        .with_content_span(Span::from_usize(self.colon.raw_node().end_byte(), end));
        main.content.extract_terminal_bullet();
        ParseOutcome::Parsed(main)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_recovery_lowers_only_nodes_from_its_original_parse() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
        ))
        .trim_end_matches('\n');
        let parser = crate::TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let foreign = parser
            .parse_source_incremental(source, None)
            .expect("independent parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = false;
        while let Some(parent) = pending.pop() {
            let mut cursor = parent.walk();
            let children: Vec<_> = parent.children(&mut cursor).collect();
            pending.extend(children.iter().copied());
            for (index, first) in children.iter().copied().enumerate() {
                let tail = children[index + 1..].iter().copied();
                let Some(candidate) = TerminalMainTier::admit(first, tail.clone(), source) else {
                    continue;
                };
                let errors = talkbank_model::ErrorCollector::new();
                assert!(matches!(
                    candidate.lower(&foreign, &errors),
                    ParseOutcome::Rejected
                ));
                // Mixing independent producer sources is a tool fault, not
                // evidence that the otherwise identical CHAT text is invalid.
                assert_eq!(
                    errors.into_vec()[0].code,
                    talkbank_model::ErrorCode::InternalError
                );
                let admitted =
                    TerminalMainTier::admit(first, tail, source).expect("same structure");
                let errors = talkbank_model::ErrorCollector::new();
                assert!(matches!(
                    admitted.lower(&parsed, &errors),
                    ParseOutcome::Parsed(_)
                ));
                assert!(errors.into_vec().is_empty());
                witnessed = true;
            }
        }
        assert!(
            witnessed,
            "canonical missing-End specimen must reach flattened EOF recovery"
        );
    }
}
