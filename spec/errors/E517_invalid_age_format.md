+++
code = 'E517'
name = '@ID age field does not match a legal CHAT date pattern'
kind = 'Invalidity'
status = 'implemented'
status_note = 'The predecessor auto-generated spec `E517_auto.md` covered only the "missing semicolon" case (`2.6`); this spec adds the full depfile.cut test matrix. Implementation lives in `crates/talkbank-model/src/model/header/codes/age.rs` as `AgeValue::violates_depfile_pattern()`, invoked from `check_id_header`. Reference-corpus files carrying `3;0`, `2;06`, etc. now trip E517 and need their `@ID` age fields corrected to `3;00.`, `2;06.`, etc.'

[[example]]
level = 'header'
title = '(one-digit month without period)'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;0||||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
level = 'header'
title = '(two-digit month without period)'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06||||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
level = 'header'
title = '(one-digit month with period)'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;0.15||||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
level = 'header'
title = '(one-digit day with period)'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.5||||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
level = 'header'
title = '(missing semicolon, pre-existing case)'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2.6||||Target_Child|||
*CHI:	hello .
@End
'''
+++

## Description

The `@ID` header's fourth field (`age`) must conform to one of the
three legal CHAT date patterns defined by CLAN's authoritative
`depfile.cut`:

```
@ID:	@d<yy;>  @d<yy;mm.>  @d<yy;mm.dd>
```

That is, a legal age is exactly one of:

- `YY;`: year, followed immediately by `|` (field end)
- `YY;MM.`: year, semicolon, **two-digit** month, trailing period
- `YY;MM.DD`: year, semicolon, **two-digit** month, period, **two-digit** day

Anything else, one-digit month without period (`3;0`), two-digit
month without period (`2;06`), one-digit month with period (`3;0.15`),
one-digit day with period (`3;06.5`), missing semicolon (`2.6`),
etc., is an **illegal date representation**. CLAN CHECK emits
error 34 ("Illegal date representation") for these; Rust chatter
must match.

Rust chatter historically accepted `YY;M` (single-digit month
without period) and `YY;MM` (two-digit month without period)
silently, a regression versus CLAN CHECK.
Authoritative source: `clan-info/lib/depfile.cut:16`.

## CHAT background

The `@ID` header carries ten pipe-delimited fields; the fourth is
`age`. CLAN's `depfile.cut` declares the legal age patterns as
typed-date templates (`@d<...>`). The templates enumerate the only
three acceptable forms, and the `mm` / `dd` slots mean zero-padded
two-digit numerals, not "1 or 2 digits".

CLAN CHECK applies these patterns via its depfile-driven field
validator: `month` and `day` require exactly two digits, and `age`
requires a period after `month`. Rust
chatter's `AgeValue::needs_zero_padding()` only flagged one-digit
components *when a period was already present*, so `3;0` and `2;06`
slipped through.

## Expected Behavior

- **Parser**: accepts the malformed age as a string field (grammar
  treats `@ID` field 4 as free text up to the next `|`).
- **Validator**: inspects the age field value and reports E517 at
  the `@ID` header's age-field span when the value does not match
  any of:
  - empty string (age-unknown is valid)
  - `^\d+;$`
  - `^\d+;\d{2}\.$`
  - `^\d+;\d{2}\.\d{2}$`

The regexes encode the three depfile.cut patterns directly. `yy`
in depfile.cut admits any unsigned integer width (real corpora use
`3;`, `18;`, `22;`, `43;`), so the year is `\d+`, not `\d{2}`.

## Remediation guidance (for data maintainers)

When E517 fires, the `@ID` age field is malformed per CLAN's
authoritative rules. Fixes:

- `3;0` → `3;00.` (zero-pad month + trailing period)
- `2;6` → `2;06.`
- `2;06` → `2;06.` (add trailing period)
- `3;0.15` → `3;00.15` (zero-pad month)
- `3;06.5` → `3;06.05` (zero-pad day)
- `2.6` → `2;06.` (add missing semicolon + zero-pad)

CLAN ships a one-pass fixer: `chstring +q +1`. It targets this
exact error pattern in bulk corpus data.

## Notes

- Authoritative source: `clan-info/lib/depfile.cut:16`.
- CLAN CHECK emits error 34 ("Illegal date representation"); the rule
  is "months for age must be two digits".
- Regression discovered while auditing reference-corpus files against
  the depfile.cut date patterns.
- Implementation: `crates/talkbank-model/src/model/header/codes/age.rs`
, the `needs_zero_padding()` predicate's early-return on
  no-period is the root cause. It should be replaced with a direct
  three-pattern match.

## CHAT Rule

CLAN `depfile.cut`:

```
@ID:	@d<yy;> @d<yy;mm.> @d<yy;mm.dd>
```

CHAT manual: https://talkbank.org/0info/manuals/CHAT.html, `@ID`
header section, age-field specification.
