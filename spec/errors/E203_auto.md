+++
code = 'E203'
name = 'Invalid form type marker'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'word'
source = 'E2xx_word_errors/E203_invalid_form_marker.cha'
claim = { subsumed_by = 'E316' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	dog@b@c .
@End
'''
+++

## Description

Word contains an invalid or undeclared `@` form type marker (e.g., `dog@b@c` has multiple stacked markers).

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
