+++
code = 'E600'
name = 'Tier alignment skipped due to parse errors'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
title = 'Non-integer index in %gra'
claim = 'violates'
notes = '**Expected**: E600 warning, could not fully parse dependent tier content.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	int|hello n|world .
%gra:	abc|0|ROOT 2|0|ROOT
@End
'''
+++

## Description

A dependent tier (typically `%mor`) had parse errors during lenient recovery, so the
validator cannot verify alignment between tiers. Alignment checks (main↔%mor, %mor↔%gra)
are skipped for the affected utterance. This is a **warning**, not an error, the file
still parses, but alignment correctness is unverified for tainted tiers.

E600 fires in pairs: if `%mor` is tainted, both main↔%mor and %mor↔%gra alignment
checks are skipped, producing two E600 warnings for the same utterance.

## Root Cause

E600 is a downstream consequence, not a primary error. The actual problem is in the
dependent tier content (e.g. malformed %gra relation). When tree-sitter encounters
a parse error in a `%mor`, `%gra`, `%pho`, or `%sin` tier, the error recovery marks
the tier as tainted and emits E600 as a warning.

## CHAT Rule

Dependent tiers must parse cleanly for alignment validation to run. See CHAT manual
sections on %gra tier format: each relation must be `index|head|RELATION`.

## Notes

- E600 fires when tree-sitter produces an ERROR node inside a %mor, %gra, %pho, or
  %sin tier. The tier content is recognized as belonging to a known tier type, but
  has internal parse errors.
- Fix the underlying tier parse error and E600 goes away.
- Re-running morphotag will regenerate tiers from scratch, eliminating bad content.
