+++
code = 'E316'
name = 'Unparsable content'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E309_speaker_in_same_bullet.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello . [+ bch] 2041689_2042652
*CHI:	world . [+ bch] 2051689_2052652
@End
'''

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E331_unexpected_node_helper.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello {{{ world }}} .
@End
'''

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E330_unexpected_node_content.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	<<< [= test] >>> .
@End
'''

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E330_unusual_content_marker.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	<<<<< hello >>>>> world .
@End
'''

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E303_syntax_error.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI	hello world .
@End
'''

[[example]]
level = 'utterance'
source = 'E5xx_header_errors/E501_duplicate_header.cha'
claim = { subsumed_by = 'E501' }
notes = '''
Note: The duplicate `@Begin` header is detected as E501 (DuplicateHeader)
rather than E316 (UnparsableContent) because the parser has a specific check
for duplicate headers.
'''
chat = '''
@UTF8
@Begin
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
'''

[[example]]
level = 'utterance'
source = 'E5xx_header_errors/E515_bullet_time_invalid.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: Timestamp shows 2052652_2041689 where start > end
*CHI:	hello world . [+ bch] 2052652_2041689
@End
'''

[[example]]
level = 'utterance'
source = 'E7xx_tier_parsing/E702_invalid_mor_format.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	hello n|world .
@End
'''
+++

## Description

Unparsable content

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
