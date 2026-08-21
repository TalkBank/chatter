+++
code = 'E323'
name = 'Missing colon after speaker code'
kind = 'Invalidity'
status = 'not_implemented'
status_note = 'Unreachable via tree-sitter parser. Without the colon, tree-sitter does not recognize a main tier node, producing E316 instead.'

[[example]]
level = 'utterance'
source = 'error_corpus/parse_errors/E323_missing_colon.cha'
claim = { subsumed_by = 'E316' }
notes = '''
Note: E323 is emitted in the parser (`prefix.rs:128`) when a main tier is
partially parsed but the colon is missing as a child node. However, the
tree-sitter grammar requires the colon to match a main tier at all, so
`*CHI hello .` produces E316 (unparsable content) instead. The missing
`@UTF8` and `@End` add E502, E503, E504, E505.
'''
chat = '''
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	ERROR: Speaker must be followed by colon
@Comment:	Invalid: '*CHI hello' - Missing colon
*CHI hello .
@End
'''
+++

**Last updated:** 2026-04-04 08:28 EDT

## Description

Missing colon after speaker code on main tier. E323 (MissingColonAfterSpeaker)
fires in `prefix.rs` when the tree-sitter grammar parses a main tier but the
colon child node is missing. However, when the colon is absent, the grammar
typically fails to match the main tier pattern at all, producing an ERROR node
(E316 UnparsableContent) rather than a partial main tier with a missing colon.

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- E323 IS emitted in the parser but requires the grammar to partially match a
  main tier without the colon, which the tree-sitter grammar does not do,
  it produces a full ERROR node instead. E323 may be reachable through the
  re2c parser or through future grammar changes.
- The example produces E316 + header errors due to the unparsable main tier
  line and missing `@UTF8`/`@End` scaffolding.
