//! Scoped semantic search for `talkbank/scopedFind`.

use std::collections::HashSet;

use regex::{Regex, RegexBuilder};
use serde::Serialize;
use talkbank_model::model::{ChatFile, Line};
use tower_lsp::lsp_types::Range;

use crate::backend::LspBackendError;
use crate::backend::execute_commands::ScopedFindRequest;
use crate::backend::state::Backend;

use super::shared::get_document_and_chat_file;
use crate::backend::utils::LineIndex;

/// One scoped-find match returned to the extension.
#[derive(Serialize)]
struct ScopedFindMatch {
    /// Exact UTF-16 source range, including both lines for a multiline match.
    range: Range,
    /// Tier label such as `main` or `mor`.
    tier: String,
    /// Speaker code for the containing utterance.
    speaker: String,
    /// Source line text used in pickers.
    line_text: String,
}

/// Handle `talkbank/scopedFind`.
pub(crate) fn handle_scoped_find(
    backend: &Backend,
    request: &ScopedFindRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let (text, chat_file) = get_document_and_chat_file(backend, &request.uri)?;
    let matcher = build_matcher(&request.query, request.regex)?;
    let speaker_filter: HashSet<&str> = request
        .speakers
        .iter()
        .map(|speaker| speaker.as_str())
        .collect();
    let document = SearchDocument::new(&text);

    let matches = find_in_scoped_tiers(
        &chat_file,
        &document,
        &request.scope,
        &speaker_filter,
        &matcher,
    )?;

    serde_json::to_value(&matches).map_err(Into::into)
}

/// Plain queries overlap; explicit regex queries use regex iteration semantics.
pub(super) enum Matcher {
    Empty,
    Plain(Regex),
    Regex(Regex),
}

fn build_matcher(query: &str, is_regex: bool) -> Result<Matcher, crate::backend::LspBackendError> {
    if query.is_empty() && !is_regex {
        return Ok(Matcher::Empty);
    }
    let compiled = if is_regex {
        Regex::new(query).map(Matcher::Regex)
    } else {
        RegexBuilder::new(&regex::escape(query))
            .case_insensitive(true)
            .build()
            .map(Matcher::Plain)
    };
    compiled.map_err(|e| crate::backend::LspBackendError::InvalidRegex {
        reason: e.to_string(),
    })
}

/// The source and coordinate index are created together, never supplied as
/// independent arguments to match projection.
struct SearchDocument<'a> {
    text: &'a str,
    index: LineIndex<'a>,
}

/// A regex-issued match borrowing the admitted original source span.
struct SourceHit<'a> {
    origin: usize,
    matched: regex::Match<'a>,
}

impl SourceHit<'_> {
    fn offset(&self) -> usize {
        self.origin + self.matched.start()
    }
}

impl<'a> SearchDocument<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            index: LineIndex::new(text),
        }
    }

    fn find(
        &self,
        start: usize,
        end: usize,
        matcher: &Matcher,
    ) -> Result<Vec<SourceHit<'a>>, LspBackendError> {
        let slice = self
            .text
            .get(start..end)
            .ok_or(LspBackendError::InvalidSourceSpan {
                start,
                end,
                source_len: self.text.len(),
            })?;
        if slice.is_empty() {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();
        match matcher {
            Matcher::Empty => {}
            Matcher::Plain(re) => {
                let mut cursor = 0;
                while let Some(matched) = re.find_at(slice, cursor) {
                    // A plain query is nonempty. Advance by the first matched
                    // scalar to retain overlapping hits without splitting UTF-8.
                    let Some(first) = matched.as_str().chars().next() else {
                        break;
                    };
                    cursor = matched.start() + first.len_utf8();
                    hits.push(SourceHit {
                        origin: start,
                        matched,
                    });
                }
            }
            Matcher::Regex(re) => {
                hits.extend(re.find_iter(slice).map(|matched| SourceHit {
                    origin: start,
                    matched,
                }));
            }
        }
        Ok(hits)
    }

    fn project(
        &self,
        hit: SourceHit<'a>,
        tier: &str,
        speaker: &str,
    ) -> Result<ScopedFindMatch, LspBackendError> {
        let start_offset = hit.offset();
        let end_offset = hit.origin + hit.matched.end();
        let ((start, line_text), (end, _)) = self
            .index
            .exact_position_and_line(start_offset)
            .zip(self.index.exact_position_and_line(end_offset))
            .ok_or(LspBackendError::UnrepresentableSearchMatch {
                start: start_offset,
                end: end_offset,
            })?;
        Ok(ScopedFindMatch {
            range: Range { start, end },
            tier: tier.to_owned(),
            speaker: speaker.to_owned(),
            line_text: line_text.to_owned(),
        })
    }
}

/// Collect all tier matches for the requested scope and speaker filter.
fn find_in_scoped_tiers(
    chat_file: &ChatFile,
    document: &SearchDocument<'_>,
    scope: &str,
    speaker_filter: &HashSet<&str>,
    matcher: &Matcher,
) -> Result<Vec<ScopedFindMatch>, LspBackendError> {
    let mut results = Vec::new();
    let search_main = scope == "main" || scope == "all";
    let search_dependent = scope != "main";

    for model_line in &chat_file.lines {
        let Line::Utterance(utterance) = model_line else {
            continue;
        };

        let speaker = utterance.main.speaker.as_str();
        if !speaker_filter.is_empty() && !speaker_filter.contains(speaker) {
            continue;
        }

        if search_main {
            // Only a located main tier has text to search.
            if let Some(span) = utterance.main.location.span() {
                for hit in document.find(span.start(), span.end(), matcher)? {
                    results.push(document.project(hit, "main", speaker)?);
                }
            }
        }

        if search_dependent {
            for entry in &utterance.dependent_tiers {
                let dt = &entry.tier;
                let tier_label = dt.kind();
                if scope != "all" && tier_label != scope {
                    continue;
                }

                let Some(span) = dt.location().span() else {
                    continue;
                };

                for hit in document.find(span.start(), span.end(), matcher)? {
                    results.push(document.project(hit, tier_label, speaker)?);
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
/// Test helper that exposes matcher construction without the full handler path.
pub(super) fn test_build_matcher(
    query: &str,
    is_regex: bool,
) -> Result<Matcher, crate::backend::LspBackendError> {
    build_matcher(query, is_regex)
}

#[cfg(test)]
/// Test helper that exposes span matching without the full handler path.
pub(super) fn test_find_matches_in_span(
    text: &str,
    start: usize,
    end: usize,
    matcher: &Matcher,
) -> Vec<(usize, usize)> {
    SearchDocument::new(text)
        .find(start, end, matcher)
        .expect("valid source span")
        .into_iter()
        .map(|hit| (hit.offset(), hit.matched.len()))
        .collect()
}

#[cfg(test)]
/// Exercise source projection using an actual literal regex match.
pub(super) fn test_make_match(
    byte_offset: usize,
    match_len: usize,
    tier: &str,
    speaker: &str,
    text: &str,
) -> Option<(Range, String, String)> {
    let selected = text.get(byte_offset..byte_offset.checked_add(match_len)?)?;
    let matcher = Matcher::Regex(Regex::new(&regex::escape(selected)).ok()?);
    let document = SearchDocument::new(text);
    let hit = document
        .find(byte_offset, byte_offset + match_len, &matcher)
        .ok()?
        .into_iter()
        .next()?;
    let m = document.project(hit, tier, speaker).ok()?;
    Some((m.range, m.tier, m.speaker))
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn malformed_source_spans_are_errors_not_empty_searches() {
        let document = SearchDocument::new("a😀z");
        let matcher = build_matcher("z", false).unwrap();
        for (start, end) in [(2, 6), (0, 2), (5, 4), (0, 7)] {
            assert!(matches!(
                document.find(start, end, &matcher),
                Err(LspBackendError::InvalidSourceSpan { .. })
            ));
        }
        assert!(document.find(5, 5, &matcher).unwrap().is_empty());
    }
}
