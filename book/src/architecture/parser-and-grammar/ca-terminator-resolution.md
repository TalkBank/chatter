# CA Terminator Resolution

**Status:** Current
**Last updated:** 2026-05-05 12:23 EDT

How CA markers are split between separators and linkers in the parser/model.

## Current rule

The parser/model no longer promotes CA markers into utterance terminators.

The supported split is:

1. **Standard utterance terminators** remain the CHAT terminators such as
   `.` `?` `!` `+...` `+/.` and related final punctuation tokens.
2. **CA intonation arrows** (`⇗ ↗ → ↘ ⇘`) stay `Separator` content items.
3. **CA TCU markers** (`≈ ≋`) stay `Separator` content items.
4. **CA TCU linker forms** (`+≈ +≋`) stay `Linker` items.

This means a trailing `→`, `≈`, or `≋` remains in main-tier content rather
than being retyped as `Terminator`.

## Parser/model consequences

1. Tree-sitter grammar keeps arrows and `≈/≋` on the `separator` path.
2. The tree parser converts those nodes directly into `Separator` variants.
3. The re2c parser classifies `≈/≋` as separators and `+≈/+≋` as linkers.
4. The old post-hoc `resolve_ca_terminator()` promotion pass was removed.
5. `Terminator::try_from_chat_str()` intentionally rejects CA arrows,
   `≈`, `≋`, `+≈`, and `+≋`.

## Data Model

The active surface split is:

| Kind | CHAT tokens |
|---|---|
| `Terminator` | `.` `?` `!` `+...` `+/.` `+//.` `+/?` `+!?` `+"/.` `+".` `+//?` `+..?` `+.` |
| `Separator` | `⇗` `↗` `→` `↘` `⇘` `≈` `≋` plus the other CA/content separators |
| `Linker` | `+≈` `+≋` plus the other utterance linkers |

Legacy CA-only `Terminator` variants still exist in the type for backward
compatibility with older serialized data, but new parser/classifier code does
not construct them from CHAT text.

## Regression coverage

The regression surface for this split is:

- `ca_symbols_are_not_chat_terminators` in `talkbank-model`
- `trailing_ca_arrow_stays_separator` in `talkbank-parser`
- `trailing_ca_no_break_stays_separator` in `talkbank-parser`
- `trailing_ca_technical_break_stays_separator` in `talkbank-parser`
