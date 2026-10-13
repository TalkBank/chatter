+++
code = 'E209'
name = 'Spoken material in shortened words with other features'

[[example]]
title = 'Repeated shortenings retain intervening spoken segments'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	ca(f)é(s) (a)mi(go) (a)mi(g)o ca(f)é(s)ito .
@End
'''

[[example]]
title = 'Terminal and initial shortenings retain form suffixes'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	chir(p)@o (bu)ba@b ca(f)é@c .
@End
'''

[[example]]
title = 'Language suffixes follow terminal initial and internal shortenings'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	say pla(n)@s (a)migo@s ami(g)o@s .
@End
'''

[[example]]
title = 'Replacement annotations preserve spoken-side shortening and form'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	chir(p)@o [: chirp] ca(f)é [: café] (a)migo [: amigo] .
@End
'''

[[example]]
title = 'Lengthening and shortening do not hide spoken segments'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	ca:(fé) (a)ga:in ca(f)é: .
@End
'''

[[example]]
title = 'A form suffix does not supply spoken material'
level = 'word'
claim = 'violates'
notes = 'Remove all spoken text from the chir(p)@o control, retaining the form suffix outside the shortening. This is E209, not an embedded-marker recovery case.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	(chirp)@o .
@End
'''

[[example]]
title = 'A language suffix does not supply spoken material'
level = 'word'
claim = 'violates'
notes = 'Remove all spoken text from the (a)migo@s word without changing the declared languages or suffix. An unmarked spoken control keeps this a word-level insertion rather than a whole-utterance language switch. Omitted sounds cannot become spoken through language metadata.'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	say (amigo)@s .
@End
'''
+++

## Description

These independently authored controls combine shortening with repeated omitted
segments, form and language suffixes, replacement annotations and lengthening.
The letters are synthetic controls, not excerpts or renamed corpus utterances.
The corresponding reference file reuses these combinations in one valid
document so source-coordinate, wire and transformation contracts consume them.

## CHAT Rule

Outside CA mode, shortening records omitted sounds. Only the remaining spoken
segments supply spoken material; neither a form nor a language suffix can do
so. Removing those segments therefore preserves the typed suffix but violates
E209. The existing E203 specs separately cover markers embedded inside a
shortening; the existing E246 specs cover lengthening without prior spoken text.
These controls do not change either policy or the CA-omission interpretation.
