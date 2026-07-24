# E759: Annotation at utterance start has nothing to attach to

## Description

Postfix annotations (retraces `[/]` `[//]` `[///]` `[/-]`, overlap
markers `[<]` `[>]` and their indexed forms, replacements `[: text]`,
and the quotation marker `["]`) scope over the material that PRECEDES
them. An utterance whose content BEGINS with one of these codes
(`*CHI:	[/] we go home .`) is malformed: the annotation has no host
item, so its meaning is undefined. This matches CLAN CHECK error 52
("Item '%s' must be preceded by text."), whose trigger set is exactly
a leading bracket code starting with `<`, `>`, `:`, `/`, or `"`.

The parse itself is genuinely broken (the marker binds leftward and
there is nothing to its left), so the grammar does not pretend to
parse it; the parser's error analysis names the failure precisely
instead of falling through to the generic E316 unparsable-content
catch-all. Legal LEADING codes (`[- lang]` precodes, `[^ ...]`) are
unaffected: they parse normally and never reach this path.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-23 22:27 EDT

- **Error Code**: E759
- **Category**: Main tier annotations
- **Level**: utterance
- **Layer**: parser

## Example 1

**Trigger**: utterance content begins with a retrace marker.

**Expected Error Codes**: E759

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[/] we go home .
@Comment:	ERROR: the leading retrace has no material to retrace
@End
```

## Example 2

**Trigger**: utterance content begins with an overlap marker.

**Expected Error Codes**: E759

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[<] no way .
@Comment:	ERROR: the leading overlap marker has no scoped material
@End
```

## Example 3

**Trigger**: utterance content begins with a replacement.

**Expected Error Codes**: E759

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[: because] we go .
@Comment:	ERROR: the leading replacement has no word to replace
@End
```

## Expected Behavior

- **Parser**: The utterance does not parse (the annotation cannot bind
  leftward); the file-level error analysis recognizes a main-tier
  region whose content starts with a CHECK-52-family code and reports
  E759 with the code named in the message, instead of generic E316.
- **Validator**: No separate validation rule; the parse-layer
  diagnostic is the rejection.

## CHAT Rule

Postfix annotations must be preceded by the content they scope over.
Parity: CLAN CHECK error 52 (`check.cpp` call site 4859: fires when no
first word has been found and the bracket item starts with `<`, `>`,
`:`, `/`, or `"`). Distinct from E757 (code glued to following
content) and from CHECK 73 (other leading codes), which is not covered
by this spec. Wild-data impact at adoption: zero kept files match a
leading postfix annotation (2026-07-23 rg scan over ~/0tb/data).
