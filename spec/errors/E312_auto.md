+++
code = 'E312'
name = 'Unclosed bracket'
kind = 'Invalidity'
status = 'not_implemented'
status_note = "Unreachable via tree-sitter parser. The E312 check in `helpers.rs` fires when an ERROR node's text starts with `[` and doesn't end with `]`. However, tree-sitter's error recovery for unclosed brackets produces E375 (ContentAnnotationParseError) or E304 (missing terminator) instead of creating a single ERROR node matching the `[`...not-`]` pattern. Tested with `[= explanation`, `[//`, `[*`, `[%`, `[<`, `[=!`, none trigger E312. The re2c parser may reach this code path."

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E312_unclosed_bracket.cha'
claim = { subsumed_by = 'E375' }
notes = '''
Note: The unclosed bracket `[= comment .` causes the parser to misparse the
line structure. The parser produces E304 (missing speaker code, because the
continuation after the bracket is misinterpreted as a new line) and E375
(ContentAnnotationParseError) rather than E312 (UnclosedBracket) or E316
(UnparsableContent).
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	word [= comment .
@End
'''
+++

## Description

Opening bracket `[` on the main tier has no matching closing bracket `]`.

## Expected Behavior

The parser should report E312 when an opening bracket `[` has no matching
closing bracket `]`. However, tree-sitter's error recovery routes these
cases through other error paths (E375, E304, E316).

## CHAT Rule

See CHAT manual on annotation brackets. All bracket notation must be
properly opened and closed: `[= comment]`, `[: replacement]`, etc.

## Notes

- E312 check exists in `helpers.rs:54` as a pattern match on ERROR node text
- Tree-sitter's error recovery never produces the expected ERROR node pattern
  for unclosed brackets; it splits the parse differently
- All tested unclosed bracket forms produce E375, E304, or E316 instead
