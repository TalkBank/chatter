//! Parser-state seam for the document/line entry point.
//!
//! This is the OUTERMOST region of the visitor-driven parser migration: the
//! walk over `full_document` -> lines. Historically the entry point hand-walked
//! `root_node.children()` with `match child.kind()` string dispatch (see the
//! pre-migration `parse_lines_with_old_tree`). That hand-walk is banned by the
//! "CST Traversal Rules" in the repo `CLAUDE.md`: it silently dropped recovery
//! nodes, which is the root cause of the recurring missing-node bugs that the
//! generated typed CST traversal was created to end.
//!
//! `DocumentLowering` is the parser-state type that drives the NEW backend's
//! free function [`extract_full_document`] and processes its
//! `FullDocumentChildren` slots EXHAUSTIVELY (no `_` catch-all): every
//! `NodeSlot` variant (`Present` / `Missing` / `Error` / `Unexpected` /
//! `Absent`) at every child position is handled or explicitly accounted for,
//! and every carrier's `unexpected` sink is surfaced. Each `full_document`
//! member is a `Positioned<..>` (its `leading_extras` plus its `slot`); a
//! `repeat(..)` member's `slot` is a `Vec<Positioned<NodeSlot<..>>>`, so the
//! repeats re-nest one level. This cluster migrates only the document level and
//! leaves each line's INNER content (headers, utterances) on the existing parse
//! functions until the later clusters migrate them (behavior-preserving).
//!
//! # Behavior preservation
//!
//! For valid CHAT the produced `Vec<Line>` is identical to the hand-walk's, and
//! recovery diagnostics are preserved exactly:
//!
//! - A document-level `ERROR` node (e.g. a stray `@Date:`) is routed through the
//!   SAME error path as before (top-level dependent-tier reporting, then
//!   `@Date:`/unknown-header recovery, then `analyze_error_node`), so it can be
//!   recovered into a `Line` AND remain visible to the whole-tree
//!   `collect_recovery_nodes` backstop (which still runs in this task).
//! - A `Missing`/`Absent` ANCHOR (utf8/begin/end header) is intentionally NOT
//!   flagged here: the pre-migration loop emitted no diagnostic for a missing
//!   anchor either; the validation layer (missing `@Begin`/`@End`) and the
//!   backstop cover those. Emitting one here would be a NEW diagnostic and a
//!   regression of the "preserve, do not change" invariant.
//!
//! # WATCH-ITEM: double-emission
//!
//! Because `collect_recovery_nodes` STILL runs as a whole-tree backstop in this
//! task, any recovery diagnostic emitted from a `NodeSlot::Error`/`Missing` here
//! MUST be emitted at the EXACT span of the offending node. The backstop's call
//! site dedups by span overlap, so an exact-span emission is auto-suppressed in
//! the backstop and diagnostics never double up. The shared error helpers reused
//! here already emit at the node span, so this property holds by construction.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, BeginHeaderNode, EndHeaderNode, FullDocumentChild1Choice, FullDocumentNode,
    LineChoice, LineNode, NodeSlot, Utf8HeaderNode, extract_full_document, extract_line,
};
use crate::model::{Header, Line};
use crate::node_types::{BLANK_LINE, PRE_BEGIN_HEADER, UNSUPPORTED_LINE};
use crate::parser::chat_file_parser::header_parser::{
    handle_pre_begin_header, helpers::header_separator, parse_header_node,
};
use crate::parser::chat_file_parser::utterance_parser::parse_utterance_node;
use crate::parser::tree_parsing::parser_helpers::{
    analyze_error_node, analyze_line_error, collect_recovery_nodes, is_pre_begin_header,
};
use talkbank_model::ParseOutcome;
use tracing::trace;

use super::helpers::{recover_top_level_error_node, report_top_level_dependent_tier_error};

/// Parser-state for the document/line entry point.
///
/// Carries the source text (needed by the line/error helpers that read node text
/// by byte offset) and the error sink that recovery diagnostics stream to. The
/// accumulated [`Line`] values are owned here during the walk and handed back via
/// [`Self::into_lines`].
///
/// Generic over `S: ErrorSink` so it composes with the entry point's
/// `TeeErrorSink` (which records diagnostics for the backstop's span-dedup)
/// without boxing.
pub(super) struct DocumentLowering<'a, S: ErrorSink> {
    /// Full source text of the CHAT file being parsed.
    source: &'a str,
    /// Diagnostic sink that recovery diagnostics are reported to.
    errors: &'a S,
    /// File-order `Line` values accumulated during the walk.
    lines: Vec<Line>,
}

impl<'a, S: ErrorSink> DocumentLowering<'a, S> {
    /// Construct a `DocumentLowering` over `source`, reporting to `errors`, with
    /// `capacity` reserved for the line accumulator (the `full_document` child
    /// count is a good upper bound).
    pub(super) fn new(source: &'a str, errors: &'a S, capacity: usize) -> Self {
        Self {
            source,
            errors,
            lines: Vec::with_capacity(capacity),
        }
    }

    /// Consume the lowering and return the accumulated file-order lines.
    pub(super) fn into_lines(self) -> Vec<Line> {
        self.lines
    }

    /// Drive the free function [`extract_full_document`] on the `full_document`
    /// root node and process every slot exhaustively.
    ///
    /// The five slots map to the `full_document` production
    /// `seq(utf8_header, repeat(pre_begin_header), begin_header, repeat(line),
    /// end_header)`:
    /// - `child_0`: the `@UTF8` anchor
    /// - `child_1`: the pre-begin-header repeat (`@PID`/`@Font`/`@Window`/`@Color words`)
    /// - `child_2`: the `@Begin` anchor
    /// - `child_3`: the line repeat (the transcript body)
    /// - `child_4`: the `@End` anchor
    pub(super) fn lower_document(&mut self, root: tree_sitter::Node<'_>) {
        let children = extract_full_document(FullDocumentNode(root));

        // Every field of the NEW `FullDocumentChildren` carrier is a
        // `Positioned<..>`: the position's `leading_extras` (whitespace/comments,
        // no-op for CHAT) plus its `slot`. A required member's `slot` is a
        // `NodeSlot`; a `repeat(..)` member's `slot` is a
        // `Vec<Positioned<NodeSlot<..>>>`, so each repeat element is itself a
        // `Positioned` re-nesting.

        // child_0: @UTF8 anchor.
        self.lower_utf8_anchor(&children.child_0.slot);

        // child_1: repeat(pre_begin_header). Each element is a concrete
        // pre-begin header choice, an ERROR, or a MISSING placeholder.
        for element in &children.child_1.slot {
            self.lower_pre_begin_header_slot(&element.slot);
        }

        // child_2: @Begin anchor.
        self.lower_begin_anchor(&children.child_2.slot);

        // child_3: repeat(line). Each element is a `line` node (Present/Missing)
        // or an ERROR absorbed among the lines (the recovery-aware repeat keeps
        // consuming the trailing valid lines, so a mid-document ERROR does not
        // strand the tail into `unexpected`).
        for element in &children.child_3.slot {
            self.lower_line_slot(&element.slot);
        }

        // child_4: @End anchor.
        self.lower_end_anchor(&children.child_4.slot);

        // The carrier's `unexpected` sink holds any direct `full_document` child
        // that filled no grammar position. Surface it as the SAME recovery
        // diagnostic the whole-tree backstop emits; the backstop's span-dedup
        // suppresses the duplicate (WATCH-ITEM below). For valid CHAT and for the
        // recovery fixtures this sink is empty, so this is a no-op today; it is
        // the per-carrier mechanism that lets the whole-tree backstop be deleted
        // once every region surfaces its own recovery (migration Task D).
        self.surface_unexpected(&children.unexpected);
    }

    /// Push a header `Line` for an anchor node at its exact span.
    fn push_anchor_header(&mut self, node: tree_sitter::Node<'_>, header: Header) {
        let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
        self.lines.push(Line::header_with_span(header, span));
    }

    /// Lower the `@UTF8` anchor slot (`child_0`).
    ///
    /// `Present` pushes the `Utf8` header line, matching the hand-walk's
    /// `UTF8_HEADER` arm. `Missing`/`Error`/`Unexpected`/`Absent` are NOT flagged
    /// here: the pre-migration loop emitted no diagnostic for a missing anchor,
    /// and the validation layer plus the whole-tree backstop cover that case;
    /// emitting one here would be a new diagnostic (regression). An `Error` here
    /// is still surfaced because the backstop walks the whole tree.
    fn lower_utf8_anchor(&mut self, slot: &NodeSlot<'_, Utf8HeaderNode<'_>>) {
        match slot {
            NodeSlot::Present(node) => self.push_anchor_header(node.0, Header::Utf8),
            NodeSlot::Missing(_) | NodeSlot::Absent => {
                // Layout omission; backstop + validation report missing headers.
            }
            NodeSlot::Error(error_node) => self.handle_top_level_error(*error_node),
            NodeSlot::Unexpected(node) => {
                trace!("Unexpected node at @UTF8 anchor: {}", node.kind());
            }
        }
    }

    /// Lower the `@Begin` anchor slot (`child_2`). See [`Self::lower_utf8_anchor`]
    /// for the recovery rationale; this mirrors the `BEGIN_HEADER` arm.
    fn lower_begin_anchor(&mut self, slot: &NodeSlot<'_, BeginHeaderNode<'_>>) {
        match slot {
            NodeSlot::Present(node) => self.push_anchor_header(node.0, Header::Begin),
            NodeSlot::Missing(_) | NodeSlot::Absent => {
                // Backstop + validation (missing @Begin) cover this.
            }
            NodeSlot::Error(error_node) => self.handle_top_level_error(*error_node),
            NodeSlot::Unexpected(node) => {
                trace!("Unexpected node at @Begin anchor: {}", node.kind());
            }
        }
    }

    /// Lower the `@End` anchor slot (`child_4`). See [`Self::lower_utf8_anchor`]
    /// for the recovery rationale; this mirrors the `END_HEADER` arm.
    fn lower_end_anchor(&mut self, slot: &NodeSlot<'_, EndHeaderNode<'_>>) {
        match slot {
            NodeSlot::Present(node) => self.push_anchor_header(node.0, Header::End),
            NodeSlot::Missing(_) | NodeSlot::Absent => {
                // Backstop + validation (missing @End) cover this.
            }
            NodeSlot::Error(error_node) => self.handle_top_level_error(*error_node),
            NodeSlot::Unexpected(node) => {
                trace!("Unexpected node at @End anchor: {}", node.kind());
            }
        }
    }

    /// Lower one element of the pre-begin-header repeat (`child_1`).
    ///
    /// `Present` dispatches to `handle_pre_begin_header` exactly as the hand-walk
    /// did for a concrete pre-begin header. `Error` routes through the shared
    /// top-level error path. `Missing` is a layout omission (backstop covers it).
    fn lower_pre_begin_header_slot(&mut self, slot: &NodeSlot<'_, FullDocumentChild1Choice<'_>>) {
        match slot {
            // The NEW `child_1` repeat is typed as the 4-way
            // `FullDocumentChild1Choice` supertype (color-words / font / pid /
            // window header), NOT a bare `NodeSlot<Node>` as the OLD API had.
            // Its concrete raw node (the same node the OLD API yielded) is reached
            // through the generated `AsRawNode::raw_node`, handed to the unchanged
            // pre-begin handler.
            NodeSlot::Present(choice) => self.lower_pre_begin_header_node(choice.raw_node()),
            NodeSlot::Error(error_node) => self.handle_top_level_error(*error_node),
            NodeSlot::Missing(_) | NodeSlot::Absent => {
                // Layout omission; nothing to build, backstop reports content MISSING.
            }
            NodeSlot::Unexpected(node) => {
                trace!(
                    "Unexpected node in pre-begin-header repeat: {}",
                    node.kind()
                );
            }
        }
    }

    /// Build pre-begin header lines for a present pre-begin-header node.
    ///
    /// `extract_full_document` only admits the CONCRETE pre-begin kinds into the
    /// `child_1` repeat (`pid_header`/`color_words_header`/`window_header`/
    /// `font_header`), never the `pre_begin_header` supertype wrapper, so the
    /// wrapper branch is dead in practice. It is preserved verbatim from the
    /// hand-walk for defensive equivalence: should the supertype node ever
    /// surface, its concrete children are each handled.
    fn lower_pre_begin_header_node(&mut self, node: tree_sitter::Node<'_>) {
        if node.kind() == PRE_BEGIN_HEADER {
            let mut pre_cursor = node.walk();
            for pre_child in node.children(&mut pre_cursor) {
                let span = Span::new(pre_child.start_byte() as u32, pre_child.end_byte() as u32);
                handle_pre_begin_header(pre_child, span, self.source, self.errors, &mut self.lines);
            }
        } else if is_pre_begin_header(node.kind()) {
            let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
            handle_pre_begin_header(node, span, self.source, self.errors, &mut self.lines);
        } else {
            trace!(
                "Non-pre-begin-header node in pre-begin repeat: {}",
                node.kind()
            );
        }
    }

    /// Lower one element of the line repeat (`child_3`).
    ///
    /// `Present` dispatches the `line` node to `dispatch_line`, which now drives
    /// the typed `extract_line` visitor (Task 2a). `Error` routes through the
    /// shared top-level error path. `Missing` is a layout omission.
    fn lower_line_slot(&mut self, slot: &NodeSlot<'_, LineNode<'_>>) {
        match slot {
            NodeSlot::Present(line_node) => self.dispatch_line(line_node.0),
            NodeSlot::Error(error_node) => self.handle_top_level_error(*error_node),
            NodeSlot::Missing(_) | NodeSlot::Absent => {
                // Layout omission; backstop reports content MISSING nodes.
            }
            NodeSlot::Unexpected(node) => {
                trace!("Unexpected node in line repeat: {}", node.kind());
            }
        }
    }

    /// Handle a document-level `ERROR` node, preserving the hand-walk's order:
    /// 1. top-level dependent-tier reporting (taints a prior utterance, emits a
    ///    tier diagnostic at the node span);
    /// 2. `@Date:` / unknown-`@Header:` recovery into a `Line` (no diagnostic);
    /// 3. otherwise `analyze_error_node` (emits at the node span).
    ///
    /// Every emission is at the exact ERROR node span, so the whole-tree backstop
    /// dedups against it (WATCH-ITEM: no double-emission).
    fn handle_top_level_error(&mut self, error_node: tree_sitter::Node<'_>) {
        if report_top_level_dependent_tier_error(
            error_node,
            self.source,
            &mut self.lines,
            self.errors,
        ) {
            return;
        }
        if recover_top_level_error_node(error_node, self.source, &mut self.lines) {
            return;
        }
        analyze_error_node(error_node, self.source, self.errors);
    }

    /// Dispatch a present `line` node through the NEW backend's free
    /// [`extract_line`] function.
    ///
    /// The `line` grammar rule is a choice with one meaningful child (a header,
    /// an utterance, a blank line, or an unsupported line). `extract_line` places
    /// it in the carrier's single `content` position as `NodeSlot<LineChoice>`,
    /// and this method matches every variant exhaustively (no `_` catch-all). All
    /// arms preserve the behaviour of the pre-migration `node.kind()`
    /// string-dispatch loop:
    ///
    /// - `Present(ActivitiesHeader(_))`: the header case is the NESTED supertype
    ///   choice `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)`
    ///   (34 concrete header subtypes, named after its first alternative), NOT a
    ///   `LineChoice::Header(node)`. The concrete header raw node is reached via
    ///   `AsRawNode::raw_node`, delegated to `parse_header_node`, and pushed as a
    ///   `Line::header_with_separator` (with the `header_sep`'s E758
    ///   trailing-space provenance, see [`header_separator`]) on `Parsed`.
    /// - `Present(Utterance(_))`: delegates to `parse_utterance_node` and pushes
    ///   `Line::utterance` on `Parsed`.
    /// - `Present(UnsupportedLine(_))`: reports E326 `UnexpectedLineType`
    ///   "Unsupported line skipped: ..." at the node span.
    /// - `Present(BlankLine(_))`: reports E747 `BlankLineNotAllowed` "Blank
    ///   lines are not allowed" at the node span.
    /// - `Error(error_node)`: calls `analyze_line_error` with the ERROR node and
    ///   this `line_node` as context, matching the old `is_error()` branch.
    /// - `Missing(_)` / `Absent`: no diagnostic; matches the old
    ///   `is_missing() -> continue` and the empty-loop case.
    /// - `Unexpected(node)`: reports E326 `UnexpectedLineType`
    ///   "Unknown node type '...' in line", matching the old `else` arm.
    ///
    /// After the content match, the carrier's `unexpected` sink is surfaced (see
    /// [`Self::surface_unexpected`]).
    fn dispatch_line(&mut self, line_node: tree_sitter::Node<'_>) {
        let children = extract_line(LineNode(line_node));
        match &children.content.slot {
            NodeSlot::Present(LineChoice::ActivitiesHeader(header_choice)) => {
                // The `line` header case is the NESTED supertype choice
                // `LineChoice::ActivitiesHeader(LineActivitiesHeaderChoice)` (34
                // concrete header subtypes), NOT a `LineChoice::Header(node)` as
                // the OLD API had. The concrete header raw node handed to the
                // unchanged `parse_header_node` is reached through the generated
                // `AsRawNode::raw_node` on the nested choice (header internals
                // stay on the current parse function until the headers cluster
                // migrates).
                let node = header_choice.raw_node();
                if let ParseOutcome::Parsed(header) =
                    parse_header_node(node, self.source, self.errors)
                {
                    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
                    let separator = header_separator(node);
                    self.lines
                        .push(Line::header_with_separator(header, span, separator));
                }
            }
            NodeSlot::Present(LineChoice::Utterance(utterance)) => {
                let node = utterance.0;
                if let ParseOutcome::Parsed(utt) =
                    parse_utterance_node(node, self.source, self.errors)
                {
                    self.lines.push(Line::utterance(utt));
                }
            }
            NodeSlot::Present(LineChoice::UnsupportedLine(unsupported)) => {
                let node = unsupported.0;
                // Catch-all junk line: report and skip (CLAN-style unsupported line).
                let text = node
                    .utf8_text(self.source.as_bytes())
                    .unwrap_or("<invalid UTF-8>");
                self.errors.report(ParseError::new(
                    ErrorCode::UnexpectedLineType,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(
                        self.source,
                        node.start_byte()..node.end_byte(),
                        UNSUPPORTED_LINE,
                    ),
                    format!("Unsupported line skipped: {}", text.trim()),
                ));
            }
            NodeSlot::Present(LineChoice::BlankLine(blank)) => {
                let node = blank.0;
                // The grammar represents a blank line as a `blank_line` node
                // (CLAN CHECK 91: blank lines are not allowed). Reject it from the
                // tree with a specific diagnostic; no source/text scanning.
                self.errors.report(ParseError::new(
                    ErrorCode::BlankLineNotAllowed,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(self.source, node.start_byte()..node.end_byte(), BLANK_LINE),
                    "Blank lines are not allowed".to_string(),
                ));
            }
            NodeSlot::Error(error_node) => {
                // Tree-sitter recovery ERROR as the direct child of the `line`
                // node: route through the same error-analysis helper the old
                // `is_error()` branch used, passing the ORIGINAL `line_node` as
                // context so the helper can inspect its non-error siblings.
                analyze_line_error(*error_node, line_node, self.source, self.errors);
            }
            NodeSlot::Missing(_) => {
                // Tree-sitter inserted a MISSING placeholder; no diagnostic.
                // Matches the old `is_missing() -> continue`.
            }
            NodeSlot::Absent => {
                // No child at all (empty line node); no diagnostic.
                // Matches the old loop producing nothing when there is no child.
            }
            NodeSlot::Unexpected(node) => {
                // A child kind not listed in the `LineChoice` match table. This
                // indicates a grammar/parser mismatch; report at the node span.
                let kind = node.kind();
                self.errors.report(ParseError::new(
                    ErrorCode::UnexpectedLineType,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(self.source, node.start_byte()..node.end_byte(), kind),
                    format!("Unknown node type '{}' in line", kind),
                ));
            }
        }

        // Surface the `line` carrier's own `unexpected` sink (any extra child the
        // chosen `LineChoice` content did not consume). Same backstop-equivalent
        // mapping as the document carrier; empty in practice for CHAT lines.
        self.surface_unexpected(&children.unexpected);
    }

    /// Surface a carrier's `unexpected` sink as the SAME recovery diagnostics the
    /// whole-tree backstop emits.
    ///
    /// Each node that filled no grammar position is routed through the shared
    /// [`collect_recovery_nodes`] mapping (ERROR -> E316 `UnparsableContent`,
    /// MISSING -> E342 `MissingRequiredElement`, with the same
    /// `wraps_document_structure` / trailing-newline exemptions and localized
    /// recursion), reported at the offending node's exact span. Because the
    /// whole-tree backstop still runs in this task and dedups by span overlap, a
    /// node surfaced here auto-suppresses the backstop's duplicate, so the
    /// diagnostic count is unchanged (WATCH-ITEM: one E316 per error). A
    /// present-but-unexpected node contributes only the recovery nodes in its
    /// subtree, which the whole-tree backstop would find anyway, so this never
    /// introduces a NEW diagnostic while the backstop is present; it is the
    /// per-carrier mechanism that makes the backstop deletable in migration Task D.
    fn surface_unexpected(&self, unexpected: &[tree_sitter::Node<'_>]) {
        if unexpected.is_empty() {
            return;
        }
        let mut candidates = Vec::new();
        for node in unexpected {
            collect_recovery_nodes(*node, self.source, &mut candidates);
        }
        for candidate in candidates {
            self.errors.report(candidate);
        }
    }
}
