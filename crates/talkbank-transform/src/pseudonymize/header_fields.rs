//! Name decisions over generated, source-bound header fields.

use talkbank_model::Header;
use talkbank_parser::ParticipantWordRoles;
use talkbank_parser::generated_traversal::{
    AsRawNode, FromNodeKind, IdHeaderNode, ParticipantNode, ParticipantsHeaderNode, SourceBound,
    SourceSlotView,
};

use super::{FreeTextDecision, PseudonymizationInput};

/// Supported typed name field; speaker codes and roles are not names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderNameField {
    /// Name word from a participant declaration, excluding its code and role.
    Participant,
    /// Corpus-specific free-text extension.
    Custom,
    /// Free-text education description.
    Education,
    /// Participant group label.
    Group,
}

/// Original header and typed metadata field selected by the producer.
/// The index is zero-based in `headers_with_locations`, including structural headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderFieldLocation {
    header: usize,
    field: HeaderNameField,
}

impl HeaderFieldLocation {
    /// Original header's zero-based document position.
    pub fn header(self) -> usize {
        self.header
    }
    /// Typed metadata field, not a raw pipe index.
    pub fn field(self) -> HeaderNameField {
        self.field
    }
}

/// Source binding failed; no raw-text fallback may authorize a rewrite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderFieldRefusal;

/// Sensitive typed name-field finding tied to its producing CST.
/// No serializer or output-writing authority is provided.
pub struct HeaderFieldReview<'input> {
    location: HeaderFieldLocation,
    matched: super::free_text::TextMatch<'input>,
}

impl HeaderFieldReview<'_> {
    /// Typed field classification, not a guessed pipe position.
    pub fn field(&self) -> HeaderNameField {
        self.location.field()
    }
    /// Producer-assigned header/field context for private receipts.
    pub fn location(&self) -> HeaderFieldLocation {
        self.location
    }
    /// Exact protected matched text within the field.
    pub fn original(&self) -> &str {
        self.matched.original()
    }
    /// Source byte range produced by the generated field traversal.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.matched.range()
    }
    /// Shared Unicode decision for a word within a participant or ID name field.
    pub fn decision(&self) -> &FreeTextDecision<'_> {
        self.matched.decision()
    }
}

pub(super) fn plan<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
) -> Result<Vec<HeaderFieldReview<'input>>, HeaderFieldRefusal> {
    let parsed = input.parsed_source();
    let mut findings = Vec::new();
    for (header_index, (header, span)) in input
        .document()
        .document()
        .headers_with_locations()
        .enumerate()
    {
        if !matches!(header, Header::ID(_) | Header::Participants { .. }) {
            continue;
        }
        // A header that cannot be placed in the source cannot be edited in it.
        let span = span.span().ok_or(HeaderFieldRefusal)?;
        let raw = parsed
            .root_node()
            .named_descendant_for_byte_range(span.start(), span.end())
            .ok_or(HeaderFieldRefusal)?;
        if matches!(header, Header::Participants { .. }) {
            let node = ParticipantsHeaderNode::from_node(raw).ok_or(HeaderFieldRefusal)?;
            let header = parsed
                .bind_typed(node)
                .map_err(|_| HeaderFieldRefusal)?
                .extract();
            let contents = match header.field_child_2().slot().view() {
                SourceSlotView::Present(field) => field.read().map_err(|_| HeaderFieldRefusal)?,
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => return Err(HeaderFieldRefusal),
            };
            let contents = contents.extract();
            match contents.field_child_0().slot().view() {
                SourceSlotView::Present(field) => participant(
                    input,
                    header_index,
                    field.read().map_err(|_| HeaderFieldRefusal)?,
                    &mut findings,
                )?,
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => return Err(HeaderFieldRefusal),
            }
            for repeated in contents.field_child_1().slot().iter() {
                let group = match repeated.slot().view() {
                    SourceSlotView::Present(group) => group,
                    SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
                        return Err(HeaderFieldRefusal);
                    }
                };
                match group.field_child_2().slot().view() {
                    SourceSlotView::Present(field) => participant(
                        input,
                        header_index,
                        field.read().map_err(|_| HeaderFieldRefusal)?,
                        &mut findings,
                    )?,
                    SourceSlotView::Missing(_)
                    | SourceSlotView::Error(_)
                    | SourceSlotView::Absent(_) => return Err(HeaderFieldRefusal),
                }
            }
            continue;
        }
        let node = IdHeaderNode::from_node(raw).ok_or(HeaderFieldRefusal)?;
        let header = parsed
            .bind_typed(node)
            .map_err(|_| HeaderFieldRefusal)?
            .extract();
        let contents = match header.field_child_2().slot().view() {
            SourceSlotView::Present(field) => field.read().map_err(|_| HeaderFieldRefusal)?,
            SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
                return Err(HeaderFieldRefusal);
            }
        };
        let fields = contents.extract();
        // The generated repeats distinguish structural whitespace from each
        // concrete field. Erase only after canonical source-range admission.
        macro_rules! inspect {
            ($accessor:ident, $choice:ident, $field:ident, $kind:expr) => {
                for element in fields.$accessor().slot().iter() {
                    use talkbank_parser::generated_traversal::$choice;
                    let field = match element.slot().view() {
                        SourceSlotView::Present(choice) => match choice.view() {
                            $choice::Whitespaces(_) => continue,
                            $choice::$field(field) => field,
                        },
                        SourceSlotView::Missing(_)
                        | SourceSlotView::Error(_)
                        | SourceSlotView::Absent(_) => return Err(HeaderFieldRefusal),
                    };
                    let source = field.read().map_err(|_| HeaderFieldRefusal)?.source_slice();
                    for matched in super::free_text::matches(source, input.names())
                        .map_err(|_| HeaderFieldRefusal)?
                    {
                        findings.push(HeaderFieldReview {
                            location: HeaderFieldLocation {
                                header: header_index,
                                field: $kind,
                            },
                            matched,
                        });
                    }
                }
            };
        }
        inspect!(
            field_child_10,
            IdContentsChild10ChoiceSourceView,
            IdGroup,
            HeaderNameField::Group
        );
        inspect!(
            field_child_16,
            IdContentsChild16ChoiceSourceView,
            IdEducation,
            HeaderNameField::Education
        );
        inspect!(
            field_child_18,
            IdContentsChild18ChoiceSourceView,
            IdCustomField,
            HeaderNameField::Custom
        );
    }
    Ok(findings)
}

fn participant<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    header_index: usize,
    node: SourceBound<'input, '_, ParticipantNode<'input>>,
    findings: &mut Vec<HeaderFieldReview<'input>>,
) -> Result<(), HeaderFieldRefusal> {
    let fields = node.extract();
    let mut words = Vec::new();
    for repeated in fields.field_child_1().slot().iter() {
        let group = match repeated.slot().view() {
            SourceSlotView::Present(group) => group,
            SourceSlotView::Error(_) | SourceSlotView::Absent(_) => return Err(HeaderFieldRefusal),
        };
        let word = match group.field_child_1().slot().view() {
            SourceSlotView::Present(field) => field.read().map_err(|_| HeaderFieldRefusal)?,
            SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
                return Err(HeaderFieldRefusal);
            }
        };
        words.push(
            input
                .parsed_source()
                .bind(word.raw_node())
                .map_err(|_| HeaderFieldRefusal)?,
        );
    }
    let roles = ParticipantWordRoles::from_words(words).map_err(|_| HeaderFieldRefusal)?;
    for source in roles.names() {
        for matched in
            super::free_text::matches(*source, input.names()).map_err(|_| HeaderFieldRefusal)?
        {
            findings.push(HeaderFieldReview {
                location: HeaderFieldLocation {
                    header: header_index,
                    field: HeaderNameField::Participant,
                },
                matched,
            });
        }
    }
    Ok(())
}
