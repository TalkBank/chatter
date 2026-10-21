# CA Terminator Resolution

**Status:** Current
**Last updated:** {{git-dates:page}}

How CA markers are split between separators and linkers in the parser/model.

CA-originated notation does not require `@Options: CA`. The header can waive
the ordinary terminator requirement; it does not change the symbol categories
below. See [the CA option](../../chat-format/options.md) for its separate
validation, interpretation and downstream-processing effects.

## Current rule

The parser/model does not promote CA markers into utterance terminators.

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
4. No post-hoc pass promotes CA markers to terminators.
5. `Terminator::try_from_chat_str()` intentionally rejects CA arrows,
   `≈`, `≋`, `+≈`, and `+≋`.

## Data Model

The active surface split is:

| Kind | CHAT tokens |
|---|---|
| `Terminator` | `.` `?` `!` `+...` `+/.` `+//.` `+/?` `+!?` `+"/.` `+".` `+//?` `+..?` `+.` |
| `Separator` | `⇗` `↗` `→` `↘` `⇘` `≈` `≋` plus the other CA/content separators |
| `Linker` | `+≈` `+≋` plus the other utterance linkers |

The current `Terminator` enum contains the ordinary CHAT variants above, not
legacy CA-only variants. Do not infer that old serialized CA-terminator tags
remain accepted. CHAT parsing and JSON compatibility are separate boundaries;
CA symbols use their separator or linker representation.

## Regression coverage

The regression surface for this split is:

- `ca_symbols_are_not_chat_terminators` in `talkbank-model`
- `trailing_ca_arrow_stays_separator` in `talkbank-parser`
- `trailing_ca_no_break_stays_separator` in `talkbank-parser`
- `trailing_ca_technical_break_stays_separator` in `talkbank-parser`
- `trailing_ca_no_break_stays_separator_in_model_conversion` and
  `trailing_ca_technical_break_stays_separator_in_model_conversion` in
  `talkbank-parser-re2c`

These controls inspect the typed separator and absent terminator. They do not
make a terminator-free utterance valid under every option or prove acceptance
of a legacy JSON representation.
