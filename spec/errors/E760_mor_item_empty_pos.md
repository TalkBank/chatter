# E760: %mor item has an empty part-of-speech field

## Description

A `%mor` item is `pos|stem` (with optional prefixes, clitics, and
suffixes). An item that BEGINS with the `|` separator (`|we`) declares
no part of speech at all: the field before the pipe is empty, which is
never meaningful %mor content. CLAN CHECK rejects it as error 11
("Symbol is not declared in the depfile."): in depfile-era terms the
empty symbol is undeclared; the modern reading is simply that the POS
field is required.

Currently such an item fails the %mor parse and surfaces as the
generic E316 unparsable-dependent-tier catch-all covering the whole
tier content. This spec gives it a dedicated code and pins the span to
the offending item, so the operator sees "this item is missing its
POS" rather than "something on this tier is unparsable".

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-23 22:27 EDT

- **Error Code**: E760
- **Category**: Dependent tier validation
- **Level**: tier
- **Layer**: parser

## Example 1

**Trigger**: a `%mor` item starts with the pipe separator (empty POS).

**Expected Error Codes**: E760

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	we go .
%mor:	|we v|go .
@Comment:	ERROR: the first mor item has an empty POS field
@End
```

## Example 2

**Trigger**: the empty-POS item is not the first item on the tier.

**Expected Error Codes**: E760

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	we go home .
%mor:	pro|we v|go |home .
@Comment:	ERROR: the third mor item has an empty POS field
@End
```

## Expected Behavior

- **Parser**: The %mor content does not parse as mor items; the
  dependent-tier error analysis recognizes a whitespace-delimited item
  beginning with `|` on a mor-family tier and reports E760 naming the
  item, instead of generic E316 over the whole tier content.
- **Validator**: No separate validation rule; the parse-layer
  diagnostic is the rejection.

## CHAT Rule

Every %mor item carries a non-empty part-of-speech before its `|`
separator. Parity: CLAN CHECK error 11 (`check.cpp` call site 1549),
interpreted into modern semantics (the depfile mechanism is legacy;
the invariant it enforced here, a declared symbol before `|`, is
real). A bare `|` inside a stem is a different construct and is not
covered by this spec. Wild-data impact at adoption: zero kept files
carry an empty-POS mor item (2026-07-23 rg scan over ~/0tb/data).
