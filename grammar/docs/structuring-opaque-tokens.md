# Structuring Opaque Tokens, Progress Log

**Status:** Complete
**Last updated:** 2026-03-23 16:54 EDT

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
