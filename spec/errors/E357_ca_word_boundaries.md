+++
code = 'E357'
name = 'Underline opener beside attached CA pitch'

[[example]]
title = 'Pitch and partial-word underlining coexist'
level = 'utterance'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tSPK Speaker
@Options:\tCA
@ID:\teng|sample|SPK|||||Speaker|||
*SPK:\t↑ra\u0002\u0001in\u0002\u0002bow .
@End
"""

[[example]]
title = 'Delete only the opening underline pair'
level = 'utterance'
claim = 'violates'
notes = 'Compared with the control, remove only U+0002 U+0001. Pitch and all spoken segments remain; pitch does not open an underline.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tSPK Speaker
@Options:\tCA
@ID:\teng|sample|SPK|||||Speaker|||
*SPK:\t↑rain\u0002\u0002bow .
@End
"""
+++

## Description

A closing underline marker needs a preceding opening underline marker in
the same utterance. An attached pitch shift is a different typed symbol,
not an underline opener.

## CHAT Rule

Keep the `U+0002 U+0001` opening and `U+0002 U+0002` closing pairs balanced
in utterance order, including when underlining covers only part of a word
beside an attached pitch marker. The existing E357 rule applies unchanged.
