# E342: Angle-bracket group with no following annotation is invalid

## Description

An angle-bracket group `<...>` on the main tier must be followed by an
annotation (a retrace marker such as `[//]`, a scope code such as `[?]`,
an explanation `[= ...]`, etc.). A bare `<...>` group with nothing after
it is malformed CHAT. CLAN's `check` reports it as
`expected [ ]; < > should be followed by [ ]`.

The tree-sitter grammar's `group_with_annotations` rule requires at least
one annotation after the group, so on a bare group tree-sitter recovers by
inserting a synthetic `MISSING retrace_complete` node (visible in
`tree-sitter parse` as `(MISSING retrace_complete [r,c]-[r,c])`) and
continues. **Recovery is not validity**: the document did not conform to
the grammar, so the parser surfaces E342 (MissingRequiredElement) on the
recovery node while still producing a usable AST for downstream repair and
the LSP. The recovered model shape is
`Retrace { kind: Full, is_group: true }`, model-indistinguishable from a
real `<...> [//]`, which is why the invalidity is detected at the parser
(recovery-node) level rather than from the AST.

This pattern is a data error. The fix is in the corpus, not the parser:
add the intended annotation after the group (e.g., `<I don't> [//]`), or
remove the angle brackets if no scope was meant.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-06-23 13:21 EDT

- **Error Code**: E342
- **Category**: Main tier structure
- **Level**: main_tier
- **Layer**: parser

## Example 1

**Trigger**: A `<...>` group on the main tier is not followed by any
annotation. Tree-sitter inserts a synthetic `MISSING retrace_complete`
recovery node; the parser surfaces E342 on it.

**Expected Error Codes**: E342

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	PAR Participant
@ID:	eng|corpus|PAR|||||Participant|||
*PAR:	<I don't> &-uh I know xxx .
@End
```

## Expected Behavior

The parser must emit E342 on the recovery position (the point where the
required annotation was expected). The AST is still produced (the group is
promoted to a complete retrace) so downstream tooling can repair the file;
the diagnostic carries the file path, line, and column so a corpus
maintainer can locate and correct the data.

## Remediation guidance (for data maintainers)

When E342 fires on this pattern, add the annotation the group needs (most
often a retrace marker `[//]`), or drop the `<...>` if no grouping was
intended. A bare `<...>` group is never valid CHAT.
