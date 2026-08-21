+++
code = 'E360'
name = 'Deprecated Skip Bullet'
kind = 'Invalidity'
status = 'not_implemented'
status_note = "Unreachable via tree-sitter parser. The grammar's strict NAK-delimited media-bullet rule rejects the deprecated `start_end-` skip variant (dash before closing NAK) before Rust validation runs, producing E316 instead of E360. The Rust check (`InvalidMediaBullet`) only fires for bullets that parsed as `media_bullet` nodes but fail the structural check; the skip-dash form never parses as a media bullet."

[[example]]
level = 'utterance'
source = 'content/deprecated-skip-bullet.cha'
claim = { subsumed_by = 'E316' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello . 357000_357477-
@End
'''
+++

## Description

The media bullet contains a deprecated skip flag (dash before closing NAK delimiter). The skip flag is deprecated. Only a small number of occurrences exist across the corpus.

## Expected Behavior

The parser should successfully parse the file. The dash is silently stripped from the bullet timestamp. The validator should report E360 warning that the skip flag is deprecated.

## CHAT Rule

Media bullets use NAK delimiters: `\u0015start_end\u0015`. The legacy skip variant `\u0015start_end-\u0015` (dash before closing NAK) was used to mark segments that should be skipped during continuous playback. This feature is no longer supported.

## Notes

The affected files in the corpus are mostly in CHILDES recordings (Eng-NA/MacWhinney), plus a CA file (SCoSE/mary.cha) and an aphasia file (Menn/GW.cha). These files should have the dash removed from their bullet markers.
