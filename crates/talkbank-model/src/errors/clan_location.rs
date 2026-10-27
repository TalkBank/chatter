//! CLAN-adjusted error location resolution.
//!
//! CLAN hides certain header lines (@UTF8, @PID, @Color words, @Window,
//! and @Font with a CAfont value)
//! from its editor display and line numbering. When sending an error location
//! to CLAN, the line number must be adjusted to account for these hidden lines.
//!
//! This module provides [`resolve_clan_location`], the single function that
//! both the TUI and desktop app use to convert a `ParseError`'s location into
//! CLAN-compatible coordinates.

use super::source_location::{DiagnosticLocation, LineColumn};
use std::num::NonZeroUsize;

/// CLAN-adjusted line and column for sending to the CLAN editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClanLocation {
    /// 1-indexed line number in CLAN's display (hidden headers subtracted).
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
}

/// Why a diagnostic has no CLAN coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClanLocationError {
    /// The diagnostic is on a line that CLAN hides from its editor.
    HiddenLine {
        /// The original 1-indexed line number in the source file.
        source_line: usize,
    },
    /// The diagnostic concerns a value with no source position.
    Unlocated,
}

impl std::fmt::Display for ClanLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HiddenLine { source_line } => write!(
                f,
                "Error is on line {source_line} which is a CLAN hidden header \
                 (@UTF8/@PID/@Color words/@Window or CAfont metadata), CLAN does not display this line",
            ),
            Self::Unlocated => write!(f, "Error has no source location to open in CLAN"),
        }
    }
}

impl std::error::Error for ClanLocationError {}

/// Resolve a `ParseError`'s location into CLAN-compatible coordinates.
///
/// Handles the full pipeline:
/// 1. If the location carries its line and column, use them directly
/// 2. Otherwise, compute them from the span start and the source text
/// 3. Subtract hidden CLAN headers from the line number
///
/// Returns `Err` if the error falls on a hidden header line, or if it has no
/// source location at all.
pub fn resolve_clan_location(
    location: &DiagnosticLocation,
    source: &str,
) -> Result<ClanLocation, ClanLocationError> {
    let position = match *location {
        DiagnosticLocation::Known {
            position: Some(position),
            ..
        } => position,
        DiagnosticLocation::Known {
            span,
            position: None,
        } => LineColumn::of_offset(span.start(), source),
        DiagnosticLocation::Unlocated => return Err(ClanLocationError::Unlocated),
    };
    let line = position.line.get();
    let clan_line =
        visible_line(source, line).ok_or(ClanLocationError::HiddenLine { source_line: line })?;

    Ok(ClanLocation {
        line: clan_line.get(),
        column: position.column.get(),
    })
}

/// Mac CLAN's document reader matches complete, case-insensitive header names.
/// Its font reader removes only CAfont metadata, not arbitrary font headers.
fn is_hidden_header(line: &str) -> bool {
    let Some((name, value)) = line.split_once(':') else {
        return line.eq_ignore_ascii_case("@UTF8");
    };
    if name.eq_ignore_ascii_case("@Font") {
        return value
            .trim_start_matches([' ', '\t'])
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("CAfont:"));
    }
    ["@PID", "@Color words", "@Window"]
        .iter()
        .any(|header| name.eq_ignore_ascii_case(header))
}

/// Admit only a visible, nonzero display row. A hidden source row has no
/// display coordinate, even when visible rows precede it. Unsigned checked
/// arithmetic also preserves cached coordinates larger than isize::MAX.
fn visible_line(source: &str, source_line: usize) -> Option<NonZeroUsize> {
    let mut hidden = 0;
    for (index, line) in super::physical_lines::PhysicalLines::new(source)
        .take(source_line)
        .enumerate()
    {
        if is_hidden_header(line.text()) {
            if index + 1 == source_line {
                return None;
            }
            hidden += 1;
        }
    }
    source_line.checked_sub(hidden).and_then(NonZeroUsize::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    fn loc(
        start: usize,
        end: usize,
        line: Option<usize>,
        column: Option<usize>,
    ) -> DiagnosticLocation {
        let span = Span::new(start, end);
        match (
            line.and_then(NonZeroUsize::new),
            column.and_then(NonZeroUsize::new),
        ) {
            (Some(line), Some(column)) => {
                DiagnosticLocation::known_at(span, LineColumn { line, column })
            }
            _ => DiagnosticLocation::new(span),
        }
    }

    #[test]
    fn physical_terminators_preserve_hidden_and_visible_coordinates() {
        for ending in ["\n", "\r\n", "\r"] {
            let source = [
                "@UTF8",
                "@Begin",
                "@Window:\t100,0",
                "*CHI:\thello .",
                "@End",
                "",
            ]
            .join(ending);
            let start = source.find("*CHI:").expect("main tier");
            for location in [
                DiagnosticLocation::new(Span::new(start, start)),
                DiagnosticLocation::known_at(
                    Span::new(start, start),
                    LineColumn::from_zero_based(3, 0),
                ),
            ] {
                assert_eq!(
                    resolve_clan_location(&location, &source).unwrap(),
                    ClanLocation { line: 2, column: 1 }
                );
            }
            let hidden = source.find("@Window").expect("hidden header");
            assert_eq!(
                resolve_clan_location(&DiagnosticLocation::new(Span::new(hidden, hidden)), &source),
                Err(ClanLocationError::HiddenLine { source_line: 3 })
            );
        }
    }

    #[test]
    fn explicit_line_col_used_when_present() {
        let source = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let result = resolve_clan_location(&loc(13, 27, Some(3), Some(1)), source).unwrap();
        // Line 3 (*CHI:), 1 hidden header (@UTF8) → CLAN line 2
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn byte_offset_used_when_line_col_missing() {
        let source = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        // byte 13 = start of "*CHI:" on line 3
        let result = resolve_clan_location(&loc(13, 27, None, None), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn multiple_hidden_headers() {
        let source = "@UTF8\n@PID:\t123\n@Font:\tCAfont:13:0\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 5 (*CHI:), 3 hidden headers (@UTF8, @PID, @Font) → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(5), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn error_on_hidden_line_is_rejected() {
        let source = "@UTF8\n@Begin\n";
        let result = resolve_clan_location(&loc(0, 5, Some(1), Some(1)), source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, ClanLocationError::HiddenLine { source_line: 1 });
        assert!(err.to_string().contains("hidden"));
    }

    #[test]
    fn unlocated_diagnostic_has_no_clan_coordinate() {
        let source = "@UTF8\n@Begin\n";
        assert_eq!(
            resolve_clan_location(&DiagnosticLocation::Unlocated, source),
            Err(ClanLocationError::Unlocated)
        );
    }

    #[test]
    fn no_hidden_headers() {
        let source = "@Begin\n*CHI:\thello .\n@End\n";
        let result = resolve_clan_location(&loc(0, 1, Some(2), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn window_header_is_hidden() {
        let source = "@UTF8\n@Window:\t100,0\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 4 (*CHI:), 2 hidden (@UTF8, @Window) → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(4), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }

    #[test]
    fn colorwords_header_is_hidden() {
        let source = "@UTF8\n@Color words:\t$BLU\n@Begin\n*CHI:\thello .\n@End\n";
        // Line 4, 2 hidden → CLAN line 2
        let result = resolve_clan_location(&loc(0, 1, Some(4), Some(1)), source).unwrap();
        assert_eq!(result, ClanLocation { line: 2, column: 1 });
    }
}
