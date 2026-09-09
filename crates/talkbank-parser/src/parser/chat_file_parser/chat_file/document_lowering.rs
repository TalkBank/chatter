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
//! MUST stay within the offending node's span. The backstop's call site dedups
//! by span overlap, so a structurally narrowed emission is still suppressed in
//! the backstop and diagnostics never double up. The shared error helpers own
//! that source binding.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, BeginHeaderNode, ChildSlot, EndHeaderNode, FromNodeKind, FullDocumentChild1Choice,
    FullDocumentChildren, LineChoice, LineNode, MainTierNode, NoChild, NodeSlot, SlotView,
    Utf8HeaderNode, extract_line,
};
use crate::model::{Header, Line, Utterance};
use crate::node_types::{BLANK_LINE, UNSUPPORTED_LINE};
use crate::parser::ChildCapacity;
use crate::parser::chat_file_parser::header_parser::{
    handle_pre_begin_header, helpers::header_separator, parse_header_node,
};
use crate::parser::chat_file_parser::utterance_parser::{
    parse_recovered_main_tier, parse_utterance_node,
};
use crate::parser::terminal_main_tier::TerminalMainTier;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{analyze_error_node, collect_recovery_nodes};
use talkbank_model::ParseOutcome;

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
    /// Reused only when a proven terminal fragment lost its enclosing CST node.
    parser: &'a TreeSitterParser,
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
    pub(super) fn new(
        parser: &'a TreeSitterParser,
        source: &'a str,
        errors: &'a S,
        capacity: ChildCapacity,
    ) -> Self {
        Self {
            parser,
            source,
            errors,
            lines: capacity.into_vec(),
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
    /// `seq(optional(utf8_header), repeat(pre_begin_header), begin_header, repeat(line),
    /// end_header)`:
    /// - `child_0`: the optional `@UTF8` anchor (absence is E503 at validation)
    /// - `child_1`: the pre-begin-header repeat (`@PID`/`@Font`/`@Window`/`@Color words`)
    /// - `child_2`: the `@Begin` anchor
    /// - `child_3`: the line repeat (the transcript body)
    /// - `child_4`: the `@End` anchor
    ///
    /// Takes the CHILDREN, not the node. Which of the three things the root
    /// turned out to be is decided by the caller, where all three are visible at
    /// once; by the time the children exist the distinction has been made and
    /// carries no meaning here, because a document reconstructed from the ERROR
    /// standing in for one has the same type and the same content as a complete
    /// one. There were briefly two public entry points differing only in name.
    pub(super) fn lower_document(&mut self, children: FullDocumentChildren<'_>) {
        // Every field of the NEW `FullDocumentChildren` carrier is a
        // `Positioned<..>`: the position's `leading_extras` (whitespace/comments,
        // no-op for CHAT) plus its `slot`. A required member's `slot` is a
        // `NodeSlot`; a `repeat(..)` member's `slot` is a
        // `Vec<Positioned<NodeSlot<..>>>`, so each repeat element is itself a
        // `Positioned` re-nesting.

        // child_0: @UTF8 anchor.
        self.lower_utf8_anchor(children.child_0.slot());

        // child_1: repeat(pre_begin_header). Each element is a concrete
        // pre-begin header choice, an ERROR, or a MISSING placeholder.
        for element in children.child_1.slot() {
            self.lower_pre_begin_header_slot(element.slot());
        }

        // child_2: @Begin anchor.
        self.lower_begin_anchor(children.child_2.slot());

        // child_3: repeat(line). Each element is a `line` node (Present/Missing)
        // or an ERROR absorbed among the lines (the recovery-aware repeat keeps
        // consuming the trailing valid lines, so a mid-document ERROR does not
        // strand the tail into `unexpected`).
        for element in children.child_3.slot() {
            self.lower_line_slot(element.slot());
        }

        // child_4: @End anchor.
        self.lower_end_anchor(children.child_4.slot());

        // Missing @End can strand the complete final main tier outside a line
        // wrapper. Admit that typed EOF construct and reuse normal utterance
        // construction; arbitrary unexpected content still receives diagnostics.
        let mut unexpected = children.unexpected.iter().copied();
        while let Some(node) = unexpected.next() {
            if let Some(terminal) = TerminalMainTier::admit(node, unexpected.clone(), self.source) {
                if let ParseOutcome::Parsed(main) = terminal.lower(self.parser, self.errors) {
                    self.lines.push(Line::utterance(Utterance::new(main)));
                }
                // Admission consumed the complete remaining EOF sequence.
                break;
            }
            if node.end_byte() == self.source.len()
                && let Some(main) = MainTierNode::from_node(node)
            {
                if let ParseOutcome::Parsed(utterance) =
                    parse_recovered_main_tier(main, self.source, self.errors)
                {
                    self.lines.push(Line::utterance(utterance));
                }
            } else {
                self.surface_displaced(std::slice::from_ref(&node), "full_document");
            }
        }
    }

    /// Push a header `Line` for an anchor node at its exact span.
    fn push_anchor_header(&mut self, node: tree_sitter::Node<'_>, header: Header) {
        let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
        self.lines.push(Line::header_with_span(header, span));
    }

    /// Lower the `@UTF8` anchor slot (`child_0`).
    ///
    /// `Present` pushes the `Utf8` header line, matching the hand-walk's
    /// `UTF8_HEADER` arm. `Missing`/`Error`/`Absent` are NOT flagged
    /// here: the pre-migration loop emitted no diagnostic for a missing anchor,
    /// and the validation layer plus the whole-tree backstop cover that case;
    /// emitting one here would be a new diagnostic (regression). An `Error` here
    /// is still surfaced because the backstop walks the whole tree.
    fn lower_utf8_anchor(&mut self, slot: &Option<ChildSlot<'_, Utf8HeaderNode<'_>>>) {
        let Some(slot) = slot else {
            // Preserve the complete document without inventing an encoding
            // declaration. The shared header validator owns the E503 refusal.
            return;
        };
        match slot.view() {
            SlotView::Present(node) => self.push_anchor_header(node.raw_node(), Header::Utf8),
            SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                // Layout omission; backstop + validation report missing headers.
            }
            SlotView::Error(error_node) => self.handle_top_level_error(error_node),
        }
    }

    /// Lower the `@Begin` anchor slot (`child_2`). See [`Self::lower_utf8_anchor`]
    /// for the recovery rationale; this mirrors the `BEGIN_HEADER` arm.
    fn lower_begin_anchor(&mut self, slot: &ChildSlot<'_, BeginHeaderNode<'_>>) {
        match slot.view() {
            SlotView::Present(node) => self.push_anchor_header(node.raw_node(), Header::Begin),
            SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                // Backstop + validation (missing @Begin) cover this.
            }
            SlotView::Error(error_node) => self.handle_top_level_error(error_node),
        }
    }

    /// Lower the `@End` anchor slot (`child_4`). See [`Self::lower_utf8_anchor`]
    /// for the recovery rationale; this mirrors the `END_HEADER` arm.
    fn lower_end_anchor(&mut self, slot: &ChildSlot<'_, EndHeaderNode<'_>>) {
        match slot.view() {
            SlotView::Present(node) => self.push_anchor_header(node.raw_node(), Header::End),
            SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                // Backstop + validation (missing @End) cover this.
            }
            SlotView::Error(error_node) => self.handle_top_level_error(error_node),
        }
    }

    /// Lower one element of the pre-begin-header repeat (`child_1`).
    ///
    /// `Present` dispatches to `handle_pre_begin_header` exactly as the hand-walk
    /// did for a concrete pre-begin header. `Error` routes through the shared
    /// top-level error path. `Missing` is a layout omission (backstop covers it).
    fn lower_pre_begin_header_slot(&mut self, slot: &ChildSlot<'_, FullDocumentChild1Choice<'_>>) {
        match slot.view() {
            // The repeat is typed as the four-way `FullDocumentChild1Choice`
            // (color-words / font / pid / window header); the handler matches
            // it exhaustively.
            SlotView::Present(choice) => {
                let span = Span::new(
                    choice.raw_node().start_byte() as u32,
                    choice.raw_node().end_byte() as u32,
                );
                handle_pre_begin_header(choice, span, self.source, self.errors, &mut self.lines);
            }
            SlotView::Error(error_node) => self.handle_top_level_error(error_node),
            SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                // Layout omission; nothing to build, backstop reports content MISSING.
            }
        }
    }

    /// Lower one element of the line repeat (`child_3`).
    ///
    /// `Present` dispatches the `line` node to `dispatch_line`, which now drives
    /// the typed `extract_line` visitor (Task 2a). `Error` routes through the
    /// shared top-level error path. `Missing` is a layout omission.
    fn lower_line_slot(&mut self, slot: &ChildSlot<'_, LineNode<'_>>) {
        match slot.view() {
            SlotView::Present(line_node) => self.dispatch_line(*line_node),
            SlotView::Error(error_node) => self.handle_top_level_error(error_node),
            SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                // Layout omission; backstop reports content MISSING nodes.
            }
        }
    }

    /// Handle a document-level `ERROR` node, preserving the hand-walk's order:
    /// 1. top-level dependent-tier reporting (taints a prior utterance, emits a
    ///    tier diagnostic at the node span);
    /// 2. `@Date:` / unknown-`@Header:` recovery into a `Line` (no diagnostic);
    /// 3. otherwise `analyze_error_node` (emits within the node's source span;
    ///    a dedicated diagnostic may narrow to the exact malformed child).
    ///
    /// The whole-tree backstop uses the same structural classifier and dedups
    /// overlapping reported spans (WATCH-ITEM: no double-emission).
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
    ///   "Unsupported line skipped: ..." at the node span
    ///   ([`Self::report_unsupported_line`]), or E330 for a line whose bytes
    ///   are not UTF-8.
    /// - `Present(BlankLine(_))`: reports E747 `BlankLineNotAllowed` "Blank
    ///   lines are not allowed" at the node span.
    /// - `Error(error_node)`: calls `analyze_error_node`, the one owner of
    ///   ERROR analysis, which `handle_top_level_error` also uses.
    /// - `Missing(_)` / `Absent`: no diagnostic; matches the old
    ///   `is_missing() -> continue` and the empty-loop case.
    /// - `Unexpected(node)`: reports E326 `UnexpectedLineType`
    ///   "Unknown node type '...' in line", matching the old `else` arm.
    ///
    /// After the content match, the carrier's `unexpected` sink is surfaced (see
    /// [`Self::surface_displaced`]).
    fn dispatch_line(&mut self, line: LineNode<'_>) {
        let children = extract_line(line);
        match children.content.slot() {
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
                if let ParseOutcome::Parsed(utt) =
                    parse_utterance_node(*utterance, self.source, self.errors)
                {
                    self.lines.push(Line::utterance(utt));
                }
            }
            NodeSlot::Present(LineChoice::UnsupportedLine(unsupported)) => {
                let node = unsupported.raw_node();
                // Catch-all junk line: report and skip (CLAN-style unsupported line).
                // A line whose bytes are not UTF-8 is reported as that fact,
                // not classified over a text the parser invented; either way
                // the arm falls through to the carrier's sink like every other.
                match node.utf8_text(self.source.as_bytes()) {
                    Ok(text) => self.report_unsupported_line(node, text),
                    Err(error) => self.errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(
                            self.source,
                            node.start_byte()..node.end_byte(),
                            UNSUPPORTED_LINE,
                        ),
                        format!("UTF-8 decoding error in unsupported line: {error}"),
                    )),
                }
            }
            NodeSlot::Present(LineChoice::BlankLine(blank)) => {
                let node = blank.raw_node();
                // The grammar represents a blank line as a `blank_line` node
                // (CLAN CHECK 91: blank lines are not allowed). Under error
                // recovery, though, the node can cover just the trailing
                // newline of a NON-blank malformed line (a speaker-less
                // `*:` tier, IISRP-residue finding 3), and "blank lines are
                // not allowed" on a visibly non-blank line sends the reader
                // hunting for a line that does not exist. A genuine blank
                // line's newline sits at a line boundary: the node starts
                // the file or is preceded by another newline. One-byte
                // boundary probe, the same class of check as the
                // span-adjacency rules; the malformed line already carries
                // its own diagnostics.
                let start = node.start_byte();
                let at_line_boundary = start == 0
                    || matches!(self.source.as_bytes().get(start - 1), Some(b'\r' | b'\n'));
                if at_line_boundary {
                    self.errors.report(ParseError::new(
                        ErrorCode::BlankLineNotAllowed,
                        Severity::Error,
                        SourceLocation::from_offsets(start, node.end_byte()),
                        ErrorContext::new(self.source, start..node.end_byte(), BLANK_LINE),
                        "Blank lines are not allowed".to_string(),
                    ));
                }
            }
            NodeSlot::Error(error_node) => {
                // ONE owner for ERROR analysis, the same one `handle_top_level_error`
                // uses. This called `analyze_line_error`, a second analyser that
                // existed only for this arm and inspected `line_node`'s siblings
                // looking for a header among them.
                //
                // It could never find one. `line` is a UNIT CHOICE in the grammar
                // (`choice($.header, $.utterance, $.blank_line, $.unsupported_line)`),
                // so a `line` node has exactly one child, and a recovery ERROR is a
                // child of `full_document` instead, which routes to
                // `analyze_error_node`. Measured two ways: 199 regions across the
                // two functions with ZERO coverage from the whole suite, and a walk
                // of every `line` node in all 738 `.cha` files in this repository
                // finding no ERROR at that depth. Their two message strings,
                // "Syntax error in line" and "Syntax error in header", appeared
                // nowhere but their own emit sites: no fixture, spec or doc ever
                // expected either.
                //
                // So this arm keeps a diagnostic and loses a duplicate.
                analyze_error_node(*error_node, self.source, self.errors);
            }
            NodeSlot::Missing(_) => {
                // Tree-sitter inserted a MISSING placeholder; no diagnostic.
                // Matches the old `is_missing() -> continue`.
            }
            NodeSlot::Absent(NoChild) => {
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
        self.surface_displaced(&children.unexpected, "line");
    }
    /// E326 for an `unsupported_line` whose text could be read: the
    /// classification the arm in [`Self::dispatch_line`] used to carry inline.
    fn report_unsupported_line(&self, node: tree_sitter::Node<'_>, text: &str) {
        // An `unsupported_line` only matches lines that do NOT begin
        // with `*`, `@`, `%`, or a tab, so a line whose TRIMMED text
        // begins with one of those is a recognisable CHAT line pushed
        // off column 1 by leading whitespace. Say so: "Unsupported
        // line skipped" alone sends the reader hunting for junk when
        // the fix is deleting one space (IISRP residue finding 6).
        // This is a diagnostic hint derived from the already-reported
        // line text, not model construction.
        let report = |message: String| {
            ParseError::new(
                ErrorCode::UnexpectedLineType,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    self.source,
                    node.start_byte()..node.end_byte(),
                    UNSUPPORTED_LINE,
                ),
                message,
            )
        };
        let indented = |kind: &str| {
            report(format!(
                "Unsupported line skipped: {} (looks like {kind} line pushed off \
                 column 1; it must begin at column 1)",
                text.trim()
            ))
            .with_suggestion("Remove the leading whitespace so the line starts at column 1")
        };
        let error = match text.trim_start().chars().next() {
            Some('%') => indented("a dependent tier"),
            Some('*') => indented("a main tier"),
            Some('@') => indented("a header"),
            _ => report(format!("Unsupported line skipped: {}", text.trim())),
        };
        self.errors.report(error);
    }

    /// Surface a carrier's `unexpected` sink WITH its displaced nodes, at
    /// `context`, mirroring the shared free function
    /// [`surface_displaced`](crate::parser::tree_parsing::parser_helpers::surface_displaced).
    ///
    /// A node carrying recovery (`ERROR`/`MISSING`, or an `ERROR` beneath it)
    /// is routed through the shared [`collect_recovery_nodes`] mapping (a
    /// dedicated structural code or E316 `UnparsableContent` for ERROR; E342
    /// `MissingRequiredElement` for MISSING, with the same
    /// `wraps_document_structure` / trailing-newline exemptions and localized
    /// recursion), reported within the offending node's span. A WELL-FORMED
    /// node that filled no grammar position (one that parsed cleanly but this
    /// carrier's rule had no place for) is reported as unexpected at
    /// `context`, where the predecessor of this method silently dropped it.
    /// Because the whole-tree backstop still runs in this task and dedups by
    /// span overlap, a recovery node surfaced here auto-suppresses the
    /// backstop's duplicate diagnostic; a present-but-unexpected node
    /// contributes only the recovery nodes in its subtree, which the
    /// whole-tree backstop would find anyway, so this never introduces a NEW
    /// diagnostic for a recovery node while the backstop is present; it is the
    /// per-carrier mechanism that makes the backstop deletable in migration Task D.
    fn surface_displaced(&self, unexpected: &[tree_sitter::Node<'_>], context: &str) {
        for node in unexpected {
            if node.is_error() || node.is_missing() || node.has_error() {
                let mut candidates = Vec::new();
                collect_recovery_nodes(*node, self.source, &mut candidates);
                for candidate in candidates {
                    self.errors.report(candidate);
                }
            } else {
                self.errors
                    .report(unexpected_node_error(*node, self.source, context));
            }
        }
    }
}
