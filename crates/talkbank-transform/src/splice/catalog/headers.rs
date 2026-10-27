//! Whole-file header proposals preserve the source's first line-ending style.
//!
//! These are catalog proposals, not admission to the utterance splice engine.

use super::{CatalogFix, EditProvenance, EditTarget, ErrorCode, Replacement, SpliceEdit};

/// Each repair owns its marker, insertion boundary and diagnostic provenance.
/// Callers cannot pair an arbitrary header string with another repair's code.
pub(super) enum HeaderRepair {
    End,
    Utf8,
}

#[derive(Clone, Copy)]
enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn for_source(source: &str) -> Self {
        match source.find(['\r', '\n']).map(|at| &source[at..]) {
            Some(rest) if rest.starts_with("\r\n") => Self::CrLf,
            Some(rest) if rest.starts_with('\r') => Self::Cr,
            Some(_) => Self::Lf,
            // No observed style: creating a new line deliberately uses LF.
            None => Self::Lf,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

impl HeaderRepair {
    pub(super) fn propose(self, source: &str) -> CatalogFix {
        let ending = LineEnding::for_source(source).as_str();
        let (offset, text, code) = match self {
            Self::End => {
                let prefix = if source.is_empty() || source.ends_with(['\r', '\n']) {
                    ""
                } else {
                    ending
                };
                (
                    source.len(),
                    format!("{prefix}@End{ending}"),
                    ErrorCode::MissingEndHeader,
                )
            }
            Self::Utf8 => (0, format!("@UTF8{ending}"), ErrorCode::MissingUTF8Header),
        };
        CatalogFix::mechanical(vec![SpliceEdit::new(
            EditTarget::InsertAt(offset),
            Replacement::new(text),
            EditProvenance::Diagnostic(code),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splice::FixKind;

    #[test]
    fn first_ending_and_absent_style_are_explicit_in_header_proposals() {
        for (source, repair, expected) in [
            ("@Begin\r\n@End\n", HeaderRepair::Utf8, "@UTF8\r\n"),
            ("@Begin\r@End\r\n", HeaderRepair::Utf8, "@UTF8\r"),
            ("@Begin", HeaderRepair::Utf8, "@UTF8\n"),
            ("", HeaderRepair::End, "@End\n"),
        ] {
            let FixKind::Mechanical(edits) = repair.propose(source).into_kind() else {
                panic!("header repair is mechanical");
            };
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0].replacement().as_str(), expected);
        }
    }
}
