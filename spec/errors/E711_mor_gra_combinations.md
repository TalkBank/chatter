+++
code = 'E711'
name = 'Repeated post-clitic feature boundaries'

[[example]]
title = 'Admitted repeated-clitic control'
level = 'tier'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Authored MOR/GRA combinations: repeated post-clitics, POS subcategories,
	ordered flat/keyed/multivalue features, Unicode lemmas and a compound lemma.
	The middle turn has four main words and MOR items but seven GRA word chunks,
	including two post-clitics and the terminator. Neighboring turns are sentinels.
*CHI:	start .
%mor:	verb|start .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before café
*CHI:	we'd've found café icecream .
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Empty host feature value'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Authored MOR/GRA combinations: repeated post-clitics, POS subcategories,
	ordered flat/keyed/multivalue features, Unicode lemmas and a compound lemma.
	The middle turn has four main words and MOR items but seven GRA word chunks,
	including two post-clitics and the terminator. Neighboring turns are sentinels.
*CHI:	start .
%mor:	verb|start .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before café
*CHI:	we'd've found café icecream .
%mor:	pron:pers|we-Nom-Person=~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Empty second post-clitic first feature'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Authored MOR/GRA combinations: repeated post-clitics, POS subcategories,
	ordered flat/keyed/multivalue features, Unicode lemmas and a compound lemma.
	The middle turn has four main words and MOR items but seven GRA word chunks,
	including two post-clitics and the terminator. Neighboring turns are sentinels.
*CHI:	start .
%mor:	verb|start .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before café
*CHI:	we'd've found café icecream .
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Both second post-clitic features empty'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Authored MOR/GRA combinations: repeated post-clitics, POS subcategories,
	ordered flat/keyed/multivalue features, Unicode lemmas and a compound lemma.
	The middle turn has four main words and MOR items but seven GRA word chunks,
	including two post-clitics and the terminator. Neighboring turns are sentinels.
*CHI:	start .
%mor:	verb|start .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before café
*CHI:	we'd've found café icecream .
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=-Polarity= verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Empty multivalue feature after the Unicode item'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Authored MOR/GRA combinations: repeated post-clitics, POS subcategories,
	ordered flat/keyed/multivalue features, Unicode lemmas and a compound lemma.
	The middle turn has four main words and MOR items but seven GRA word chunks,
	including two post-clitics and the terminator. Neighboring turns are sentinels.
*CHI:	start .
%mor:	verb|start .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before café
*CHI:	we'd've found café icecream .
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType= .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''
+++

## Description

Every feature requires a nonempty value independently, including ordered features on later repeated post-clitics and a comma-separated value on a compound lemma. Keyed emptiness remains a validation defect; it does not erase the word or key.

## CHAT Rule

Every feature requires a nonempty value independently, including ordered features on later repeated post-clitics and a comma-separated value on a compound lemma. Keyed emptiness remains a validation defect; it does not erase the word or key.

## Expected Behavior

The legal control is the authored `corpus/reference/tiers/mor-gra-combinations.cha`.
Every variant replaces only the middle MOR or GRA body. The typed control's
checked body span issues the corresponding edit in the public-workflow contract;
all other bytes, turns and dependent tiers are preserved. Claims follow existing
policy; the observation snapshot separately records actual stage-specific codes.
