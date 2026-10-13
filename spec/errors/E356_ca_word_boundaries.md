+++
code = 'E356'
name = 'Underline balance beside attached CA pitch'

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
title = 'Delete only the closing underline pair'
level = 'utterance'
claim = 'violates'
notes = 'Compared with the control, remove only U+0002 U+0002. Pitch and all spoken segments remain; pitch does not close an underline.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tSPK Speaker
@Options:\tCA
@ID:\teng|sample|SPK|||||Speaker|||
*SPK:\t↑ra\u0002\u0001inbow .
@End
"""
+++

## Description

Pitch shifts attach to a word independently of its underline markers. An
opening underline marker still requires a later closing underline marker
within the same utterance; a pitch marker cannot supply that close.

## CHAT Rule

Keep the `U+0002 U+0001` opening and `U+0002 U+0002` closing pairs balanced
in utterance order, including when underlining covers only part of a word
beside an attached pitch marker. The existing E356 rule applies unchanged.
