+++
code = 'E307'
name = 'Speaker code length and character boundaries'

[[example]]
title = 'Seven ASCII characters are within the supported limit'
level = 'file'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	ABCDEFG Target_Child
@ID:	eng|corpus|ABCDEFG|||||Target_Child|||
*ABCDEFG:	hello .
@End
'''

[[example]]
title = 'Appending one ASCII character exceeds the limit'
level = 'file'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	ABCDEFGH Target_Child
@ID:	eng|corpus|ABCDEFGH|||||Target_Child|||
*ABCDEFGH:	hello .
@End
'''

[[example]]
title = 'Replacing the seventh character with an accent violates ASCII only'
level = 'file'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	ABCDEFÉ Target_Child
@ID:	eng|corpus|ABCDEFÉ|||||Target_Child|||
*ABCDEFÉ:	hello .
@End
'''

[[example]]
title = 'An eighth character and an accent violate both independent rules'
level = 'file'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	ABCDEFÉH Target_Child
@ID:	eng|corpus|ABCDEFÉH|||||Target_Child|||
*ABCDEFÉH:	hello .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-09-23 17:41 EDT

## Description

Speaker codes must be ASCII and at most seven characters long. These are
independent constraints: an overlong non-ASCII code violates both, and reporting
one fault must not hide the other. The same code is checked in participant
declarations, ID headers and the main-tier prefix.

## Expected Behavior

The seven-character ASCII control emits no E307. Appending one character adds
the length diagnostic; replacing one character with É adds the non-ASCII
diagnostic. Combining the mutations requires both findings. Character counting
does not mistake É's two UTF-8 bytes for two characters.

## CHAT Rule

The seven-character maximum is described in the CHAT manual's
[Speaker ID section](https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID).
Non-ASCII speaker IDs are rejected consistently by Chatter; Unicode participant
names remain separate from speaker codes.

## Notes

CHECK 21-Sep-2026 accepts both ASCII examples in this matrix, including the
eight-character code. It rejects the non-ASCII examples with error 16 and also
reports an overlong tier name for the combined mutation. Chatter's existing
seven-character ceiling is retained: this matrix does not claim identical
CHECK length policy or diagnostic multiplicity.
