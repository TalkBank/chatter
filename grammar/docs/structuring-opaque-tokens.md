# Structuring Opaque Tokens, Progress Log

**Status:** Historical planning fragment, not completion evidence
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/docs/structuring-opaque-tokens.md)

The March 23 plan below stops at its first progress entry. Its former
“Complete” label was unsupported by this log. Preserve it as design history,
not an inventory of current rules or an acceptance receipt. Current structure
belongs to `grammar.js` and generated node types; use the
[grammar workflow](../../book/src/contributing/grammar-workflow.md) and
[word internals](../../book/src/chat-format/word-internals.md) for maintained guidance.
The risk labels below were planning judgments, not proofs that a rewrite is safe.

## Plan

Structure opaque `token()` rules into children where:
1. There's no ambiguity problem (the prefix disambiguates)
2. The Rust parser currently re-parses the token text
3. The structured form gives useful CST children for editors/LSP

## Ambiguity Assessment

### LOW RISK, prefix is unique, no competing rules

| Token | Prefix | Risk | Plan |
|-------|--------|------|------|
| `age_format` | digits `;` | Low, only appears inside `id_age` | Structure: year `;` month [`.` day] |
| `inline_bullet` | `\u0015` | None, unique control char | Structure: start_time `_` end_time |
| `postcode` | `[+` | Low, `+` after `[` is unique | Structure: `[+` space content `]` |
| `freecode` | `[^` | Low, `^` after `[` is unique | Structure: `[^` space content `]` |
| `langcode` | `[-` | Low, `-` after `[` is unique | Structure: `[-` space lang_code `]` |

### MEDIUM RISK, need careful prec handling

| Token | Prefix | Risk | Plan |
|-------|--------|------|------|
| `explanation_annotation` | `[=` space | Medium, `=` vs `=!` vs `=?` | Structure if prefix tokens disambiguate |
| `para_annotation` | `[=!` space | Medium, shares `[=` prefix | Structure carefully |
| `alt_annotation` | `[=?` space | Medium, shares `[=` prefix | Structure carefully |
| `percent_annotation` | `[%` space | Low, unique prefix | Structure |
| `duration_annotation` | `[#` space | Low, unique prefix | Structure |

### HIGHER RISK, leave as-is

| Token | Why |
|-------|-----|
| `error_marker_annotation` | `[*]` vs `[* code]`, optional content after `*` |
| `word_segment` | Core word parsing, too risky to change |
| `event_segment` | Performance-critical, simple text |
| `mor_pos` / `mor_lemma` | Simple text, no benefit |
| `strict_time` | Complex format, grammar-level structure adds little |

## Progress Log

### [13:44] Starting with age_format (lowest risk)
