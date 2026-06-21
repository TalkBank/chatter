//! Error rendering functions using miette
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use miette::{GraphicalReportHandler, GraphicalTheme, NamedSource, Report};
use std::fmt::Write as _;
use std::sync::Arc;
use talkbank_model::{ParseError, enhance_errors_with_source};

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
    /// The error after [`enhance_errors_with_source`]: `location.line`/`column`
    /// are populated and a windowed source context is attached. Surfaces should
    /// expose THIS error (not the raw pre-enhancement one) so line/column are
    /// available downstream (e.g. Open-in-CLAN).
    pub error: ParseError,
    /// miette plain-text render. This is the CLI's stderr output and the
    /// desktop's "Copy" form; it always points the caret at the true file line.
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
/// `source_name` is the fallback display name miette uses only for errors that
/// carry no embedded source; enhanced errors supply their own `"input"`-named
/// windowed source, so for normal validation errors the name is not shown.
pub fn render_diagnostics(
    errors: &[ParseError],
    source_name: &str,
    source: &str,
    mode: RenderMode,
) -> Vec<RenderedDiagnostic> {
    // Enhance a clone with line/column + windowed context (the raw errors a
    // caller holds are left untouched). `enhance_errors_with_source` is NOT
    // idempotent, so callers must pass RAW (pre-enhancement) errors.
    let mut enhanced = errors.to_vec();
    enhance_errors_with_source(&mut enhanced, source);

    enhanced
        .into_iter()
        .map(|error| {
            let text = render_error_with_miette_with_source(&error, source_name, source);
            let ansi = match mode {
                RenderMode::Plain => None,
                RenderMode::Ansi => Some(render_error_with_miette_with_source_colored(
                    &error,
                    source_name,
                    source,
                )),
            };
            RenderedDiagnostic { error, text, ansi }
        })
        .collect()
}

/// Render a ParseError using miette for beautiful diagnostics.
pub fn render_error_with_miette(error: &ParseError) -> String {
    let mut output = String::new();
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode());

    if let Err(_e) = handler.render_report(&mut output, error) {
        // Fallback if miette rendering fails
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Render a ParseError using miette with a shared source buffer (no error mutation).
///
/// Uses `{:?}` (Debug) formatting which delegates to miette's installed handler.
/// This respects terminal color detection, produces ANSI codes when stderr is a
/// terminal, plain text otherwise. For forced ANSI output regardless of terminal,
/// use [`render_error_with_miette_with_source_colored`].
pub fn render_error_with_miette_with_source(
    error: &ParseError,
    source_name: &str,
    source: &str,
) -> String {
    let mut output = String::new();
    let named_source = NamedSource::new(source_name, source.to_string());
    let report = Report::new(error.clone()).with_source_code(named_source);

    if let Err(_e) = write!(&mut output, "{:?}", report) {
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Render a ParseError with miette, forcing ANSI color output regardless of terminal.
///
/// Used by the desktop app (Tauri) where output is converted to HTML, not displayed
/// in a terminal. The standard [`render_error_with_miette_with_source`] would produce
/// uncolored output because miette detects no terminal.
///
/// Source resolution is identical to the plain renderer: `with_source_code`
/// supplies the full-file `NamedSource` only as a FALLBACK, so an enhanced
/// error's own windowed `"input"` source wins and the caret lands on the true
/// line. (Returning the full-file source unconditionally was the original
/// wrong-line bug: the error's window-RELATIVE label span got resolved against
/// the FULL file, so an error on line 8 rendered its caret at line 1.) The ONLY
/// difference from the plain path is the explicit handler below, which forces
/// ANSI color instead of relying on terminal detection.
pub fn render_error_with_miette_with_source_colored(
    error: &ParseError,
    source_name: &str,
    source: &str,
) -> String {
    let mut output = String::new();
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode())
        .with_links(false)
        .with_footer(String::new());

    let named_source = NamedSource::new(source_name, source.to_string());
    let report = Report::new(error.clone()).with_source_code(named_source);

    if let Err(_e) = handler.render_report(&mut output, report.as_ref()) {
        write!(&mut output, "{}", error).ok();
    }

    output
}

/// Render a ParseError using miette with a shared NamedSource.
pub fn render_error_with_miette_with_named_source(
    error: &ParseError,
    source: &NamedSource<Arc<String>>,
) -> String {
    let mut output = String::new();
    let report = Report::new(error.clone()).with_source_code(source.clone());

    if let Err(_e) = write!(&mut output, "{:?}", report) {
        write!(&mut output, "{}", error).ok();
    }

    output
}
