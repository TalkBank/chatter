+++
code = 'E315'
name = 'Invalid control character'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E315_control_character.cha'
claim = { subsumed_by = 'E316' }
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	word\u0001test .
@End
"""
+++

## Description

Main tier or dependent tier contains an invalid control character (e.g., embedded NUL, SOH, or other non-printable ASCII).

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
