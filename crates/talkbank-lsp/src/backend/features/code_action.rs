//! Quick-fix code actions derived from diagnostics.
//!
//! This module is the composition root for diagnostic-driven code actions.
//! Per-diagnostic fix routing lives in `code_action_fixes.rs`, while
//! `code_action_builders.rs` owns the repeated `WorkspaceEdit` construction.

#[path = "code_action_builders.rs"]
mod builders;
#[path = "code_action_fixes.rs"]
mod fixes;

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ErrorCode, RuleSelection};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::{CatalogResolution, FixDiagnosis, FixKind, SpliceEdit};
use tower_lsp::lsp_types::*;

use crate::backend::capabilities::VersionedEdits;
use crate::backend::diagnostics::{CurrentRevisionDiagnostics, RevisionDiagnostics};
use crate::backend::utils::{LineIndex, position_to_offset};

/// A fix policy's proposal has no URI or version and is not a workspace edit.
#[derive(Debug)]
struct ProposedQuickFix {
    title: String,
    edit: TextEdit,
    diagnostic: Diagnostic,
}

impl ProposedQuickFix {
    /// Admit exact editor coordinates against the same source that diagnosed it.
    fn into_action(
        self,
        revision: &RevisionDiagnostics,
        index: &LineIndex<'_>,
    ) -> Option<CodeActionOrCommand> {
        let range = self.edit.range;
        if (range.start.line, range.start.character) > (range.end.line, range.end.character) {
            return None;
        }
        for position in [range.start, range.end] {
            let offset = position_to_offset(revision.source(), position);
            if index.offset_to_position(offset) != position {
                return None;
            }
        }
        Some(CodeActionOrCommand::CodeAction(CodeAction {
            title: self.title,
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![self.diagnostic]),
            edit: Some(WorkspaceEdit {
                document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: revision.uri().clone(),
                        version: Some(revision.version()),
                    },
                    edits: vec![OneOf::Left(self.edit)],
                }])),
                ..Default::default()
            }),
            ..Default::default()
        }))
    }
}

/// Proposals remain bound to the revision used by their actual producers.
pub(crate) struct PreparedQuickFixes<'a> {
    revision: &'a RevisionDiagnostics,
    proposals: Vec<ProposedQuickFix>,
}

impl PreparedQuickFixes<'_> {
    pub(crate) fn finish(
        self,
        current: CurrentRevisionDiagnostics<'_>,
        _support: &VersionedEdits,
    ) -> Option<Vec<CodeActionOrCommand>> {
        if !std::ptr::eq(self.revision, current.snapshot()) {
            return None;
        }
        let index = LineIndex::new(self.revision.source());
        let actions: Vec<_> = self
            .proposals
            .into_iter()
            .filter_map(|proposal| proposal.into_action(self.revision, &index))
            .collect();
        (!actions.is_empty()).then_some(actions)
    }
}

/// Current diagnostics selected by an admitted editor range and action kind.
/// Client diagnostic payloads are hints, never authority for an edit.
pub(crate) struct QuickFixScope<'a> {
    revision: &'a RevisionDiagnostics,
    selected: Vec<&'a Diagnostic>,
}

impl<'a> QuickFixScope<'a> {
    pub(crate) fn admit(
        revision: &'a RevisionDiagnostics,
        range: Range,
        only: Option<&[CodeActionKind]>,
    ) -> Option<Self> {
        if only.is_some_and(|kinds| {
            !kinds
                .iter()
                .any(|kind| kind.as_str().is_empty() || *kind == CodeActionKind::QUICKFIX)
        }) {
            return None;
        }
        let key = |position: Position| (position.line, position.character);
        if key(range.start) > key(range.end) {
            return None;
        }
        let index = LineIndex::new(revision.source());
        for position in [range.start, range.end] {
            if index.offset_to_position(position_to_offset(revision.source(), position)) != position
            {
                return None;
            }
        }
        let selected = revision
            .items()
            .iter()
            .filter(|diagnostic| {
                let other = diagnostic.range;
                // Cursor requests and point diagnostics include their boundary.
                // Two nonempty ranges must share actual selected text.
                if range.start == range.end || other.start == other.end {
                    key(range.start) <= key(other.end) && key(other.start) <= key(range.end)
                } else {
                    key(range.start) < key(other.end) && key(other.start) < key(range.end)
                }
            })
            .collect();
        Some(Self { revision, selected })
    }
}

/// Catalog-backed routes own their presentation label; the catalog owns edits.
#[derive(Clone, Copy)]
enum CatalogAction {
    Terminator,
    EmptyUtterance,
    EndHeader,
    Utf8Header,
}

impl CatalogAction {
    fn for_code(code: ErrorCode) -> Option<Self> {
        match code {
            ErrorCode::MissingTerminator => Some(Self::Terminator),
            ErrorCode::EmptyUtterance => Some(Self::EmptyUtterance),
            ErrorCode::MissingEndHeader => Some(Self::EndHeader),
            ErrorCode::MissingUTF8Header => Some(Self::Utf8Header),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Terminator => "Insert missing terminator",
            Self::EmptyUtterance => "Delete empty utterance",
            Self::EndHeader => "Insert '@End' at end of file",
            Self::Utf8Header => "Insert '@UTF8' at start of file",
        }
    }

    fn matches_location(
        self,
        location: talkbank_model::DiagnosticLocation,
        diagnostic: &Diagnostic,
        index: &LineIndex<'_>,
    ) -> bool {
        match self {
            // Whole-file missing-header facts do not require a located token.
            // The current source, code and message still come from diagnosis.
            Self::EndHeader | Self::Utf8Header => true,
            Self::Terminator | Self::EmptyUtterance => location.span().is_some_and(|span| {
                Range::new(
                    index.offset_to_position(span.start()),
                    index.offset_to_position(span.end()),
                ) == diagnostic.range
            }),
        }
    }
}

/// Parse catalog evidence before acquiring the current-revision map guard.
pub(crate) fn prepare_code_actions<'a>(
    scope: QuickFixScope<'a>,
    parser: &TreeSitterParser,
) -> PreparedQuickFixes<'a> {
    let QuickFixScope { revision, selected } = scope;
    let index = LineIndex::new(revision.source());
    let needs_catalog = selected.iter().any(|diagnostic| {
        fixes::diagnostic_code(diagnostic)
            .and_then(CatalogAction::for_code)
            .is_some()
    });
    let path = revision
        .uri()
        .to_file_path()
        .ok()
        .filter(|path| path.extension().is_some_and(|ext| ext == "cha"));
    let diagnosis = needs_catalog.then(|| {
        FixDiagnosis::observe(
            parser,
            revision.source(),
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            path.as_deref()
                .map_or(TranscriptName::Anonymous, TranscriptName::for_path),
        )
    });
    let mut proposals = Vec::new();
    for diagnostic in selected {
        let code = fixes::diagnostic_code(diagnostic);
        if let Some(action) = code.and_then(CatalogAction::for_code) {
            if let Some(diagnosis) = &diagnosis {
                for finding in diagnosis.findings().filter(|finding| {
                    Some(finding.code) == code
                        && finding.message == diagnostic.message
                        && action.matches_location(finding.location, diagnostic, &index)
                }) {
                    if let CatalogResolution::Offered(fix) = finding.catalog() {
                        match fix.into_kind() {
                            FixKind::Alternatives(choices) => {
                                for choice in choices {
                                    proposals.extend(catalog_proposal(
                                        choice.label,
                                        choice.edits,
                                        diagnostic,
                                        revision.source(),
                                        &index,
                                    ));
                                }
                            }
                            FixKind::Semantic(edits) | FixKind::Mechanical(edits) => {
                                proposals.extend(catalog_proposal(
                                    action.label().into(),
                                    edits,
                                    diagnostic,
                                    revision.source(),
                                    &index,
                                ));
                            }
                        }
                    }
                }
            }
        } else {
            proposals.extend(fixes::actions_for_diagnostic(diagnostic));
        }
    }
    PreparedQuickFixes {
        revision,
        proposals,
    }
}

/// The selected catalog rules offer single edits. Never split a multi-edit
/// proposal into independently applicable actions if that contract changes.
fn catalog_proposal(
    title: String,
    edits: Vec<SpliceEdit>,
    diagnostic: &Diagnostic,
    source: &str,
    index: &LineIndex<'_>,
) -> Option<ProposedQuickFix> {
    let [edit] = edits.as_slice() else {
        return None;
    };
    let start = edit.target().start_offset();
    let end = edit.target().end_offset();
    if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return None;
    }
    let range = Range::new(
        index.offset_to_position(start),
        index.offset_to_position(end),
    );
    if position_to_offset(source, range.start) != start
        || position_to_offset(source, range.end) != end
    {
        return None;
    }
    Some(builders::replace_range(
        range,
        edit.replacement().as_str(),
        title,
        diagnostic,
    ))
}

#[cfg(test)]
#[path = "code_action_tests.rs"]
mod tests;
