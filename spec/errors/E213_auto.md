+++
code = 'E213'
name = 'Deprecated, replaced by E391'
kind = 'Invalidity'
status = 'deprecated'

[[example]]
level = 'word'
claim = { subsumed_by = 'E391' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	helo [: xxx] world .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

**Deprecated.** This error code was replaced by E391
(`ReplacementContainsUntranscribed`). The validation logic now emits E391
instead of E213 for the same condition.

## Notes

- E213 is deprecated. See E391 for the active error code.
- The example above triggers E391, not E213.
