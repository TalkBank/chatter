//! Error rendering functions using miette
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use miette::{GraphicalReportHandler, GraphicalTheme, NamedSource, Report};
use std::sync::Arc;
use talkbank_model::{ParseError, SourceDiagnostic, enhance_errors_with_source};

/// Which decorations a caller wants out of [`render_diagnostics`].
///
/// The rendering algorithm is identical across surfaces; the only variability is
/// *data* (does the caller also need an ANSI-colored form?), so an enum argument
/// is the right abstraction here rather than a trait. `Plain` is the CLI's
/// stderr/clipboard form; `Ansi` additionally produces the colored form the
/// desktop converts to HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Plain miette text only (CLI stderr, "Copy to clipboard").
    Plain,
    /// Plain text plus a forced-ANSI form (desktop GUI, converted to HTML).
    Ansi,
}

/// One rendered diagnostic: the enhanced error plus its rendered form(s).
///
/// This is the single typed result every surface consumes, so the CLI and the
/// desktop GUI cannot drift apart in how an error is enhanced or rendered.
#[derive(Debug, Clone)]
pub struct RenderedDiagnostic {
    /// Original evidence with a separate checked presentation or an explicit
    /// context failure. This cannot re-enter an absolute-source enhancement API.
    pub error: SourceDiagnostic,
    /// miette plain-text render. This is the CLI's stderr output and the
    /// desktop's "Copy" form. Failed context admission shows no source caret.
    pub text: String,
    /// Forced-ANSI render. `Some` exactly when [`RenderMode::Ansi`] was
    /// requested; `None` under [`RenderMode::Plain`]. The desktop converts the
    /// ANSI to HTML; it is byte-aligned with `text` on line numbering.
    pub ansi: Option<String>,
}

/// THE single error-rendering orchestration shared by every surface.
///
/// Replaces the duplicated "clone -> enhance -> render" inline blocks that the
/// CLI (`output.rs`) and the desktop bridge (`events.rs`) used to each maintain
/// separately, and which silently diverged (the desktop rendered carets at the
/// wrong line). Both now route through here, so a divergence is impossible by
/// construction and is locked by the `render_parity` cross-surface test.
///
/// Presentation owns its windowed source. An unavailable context never falls
/// back to rendering stale coordinates against the supplied source.
pub fn render_diagnostics(
    errors: &[ParseError],
    _source_name: &str,
    source: &str,
    mode: RenderMode,
) -> Vec<RenderedDiagnostic> {
    enhance_errors_with_source(errors, source)
        .map(|error| {
            let text = format!("{:?}", Report::new(error.clone()));
            let ansi = match mode {
                RenderMode::Plain => None,
                RenderMode::Ansi => Some(render_error_with_miette(&error)),
            };
            RenderedDiagnostic { error, text, ansi }
        })
        .collect()
}

/// Render an already checked diagnostic. Reusing it is safe and idempotent.
/// A failed graphical render discards its partial output before returning the
/// diagnostic's plain message.
pub fn render_error_with_miette(error: &SourceDiagnostic) -> String {
    let mut output = String::new();
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode())
        .with_links(false)
        .with_footer(String::new());

    match handler.render_report(&mut output, error) {
        Ok(()) => output,
        Err(_) => error.to_string(),
    }
}

/// Admit an absolute-source diagnostic, then render without mutating evidence.
///
/// Uses `{:?}` (Debug) formatting which delegates to miette's installed handler.
/// This respects terminal color detection, produces ANSI codes when stderr is a
/// terminal, plain text otherwise. For forced ANSI output regardless of terminal,
/// use [`render_error_with_miette_with_source_colored`].
pub fn render_error_with_miette_with_source(
    error: &ParseError,
    _source_name: &str,
    source: &str,
) -> String {
    let checked =
        SourceDiagnostic::with_index(error.clone(), &talkbank_model::SourceIndex::new(source));
    format!("{:?}", Report::new(checked))
}

/// Render a ParseError with miette, forcing ANSI color output regardless of terminal.
///
/// Used by the desktop app (Tauri) where output is converted to HTML, not displayed
/// in a terminal. The standard [`render_error_with_miette_with_source`] would produce
/// uncolored output because miette detects no terminal.
///
/// Both paths use the same checked projection. No full-file fallback can
/// reinterpret display-local spans or revive an unavailable source context.
pub fn render_error_with_miette_with_source_colored(
    error: &ParseError,
    _source_name: &str,
    source: &str,
) -> String {
    let checked =
        SourceDiagnostic::with_index(error.clone(), &talkbank_model::SourceIndex::new(source));
    render_error_with_miette(&checked)
}

/// Render a ParseError using miette with a shared NamedSource.
pub fn render_error_with_miette_with_named_source(
    error: &ParseError,
    source: &NamedSource<Arc<String>>,
) -> String {
    render_error_with_miette_with_source(error, source.name(), source.inner().as_str())
}
