+++
code = 'E702'
name = 'Internal recovery within repeated-clitic morphology'

[[example]]
title = 'Repeated-clitic recovery control'
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
title = 'Missing host lemma before its features'
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
%mor:	pron:pers|-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Missing first post-clitic lemma before its feature'
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
%mor:	pron:pers|we-Nom-Person=1~aux|-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'Missing second post-clitic POS'
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
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'An extra pipe inside the second post-clitic'
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
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have|again-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
%gra:	1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT
%com:	middle naïve
*CHI:	finish .
%mor:	verb|finish .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after résumé
@End
'''

[[example]]
title = 'A trailing tilde before a healthy independent item'
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
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos~ verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
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

Unparsable nonempty morphology content retains its MOR context even inside a later post-clitic. This applies the existing E702 recovery rule, not a text-based approximation of a recovered CST. Inspect actual producer states separately: document recovery, native carriers and strict fragments need not retain the same model.

## CHAT Rule

Unparsable nonempty morphology content retains its MOR context even inside a later post-clitic. This applies the existing E702 recovery rule, not a text-based approximation of a recovered CST. Inspect actual producer states separately: document recovery, native carriers and strict fragments need not retain the same model.

## Expected Behavior

The legal control is the authored `corpus/reference/tiers/mor-gra-combinations.cha`.
Every variant replaces only the middle MOR or GRA body. The typed control's
checked body span issues the corresponding edit in the public-workflow contract;
all other bytes, turns and dependent tiers are preserved. Claims follow existing
policy; the observation snapshot separately records actual stage-specific codes.
