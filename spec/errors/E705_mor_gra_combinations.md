+++
code = 'E705'
name = 'MOR item count is distinct from expanded chunks'

[[example]]
title = 'Repeated-clitic item-count control'
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
title = 'An independent item becomes another post-clitic'
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
%mor:	pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past~noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .
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

The main tier aligns to MOR items, not expanded clitic chunks. Replacing one admitted item boundary with a tilde decreases the item count while preserving every word chunk and every GRA relation. A matching MOR/GRA chunk count cannot make this main/MOR mismatch valid.

## CHAT Rule

The main tier aligns to MOR items, not expanded clitic chunks. Replacing one admitted item boundary with a tilde decreases the item count while preserving every word chunk and every GRA relation. A matching MOR/GRA chunk count cannot make this main/MOR mismatch valid.

## Expected Behavior

The legal control is the authored `corpus/reference/tiers/mor-gra-combinations.cha`.
Every variant replaces only the middle MOR or GRA body. The typed control's
checked body span issues the corresponding edit in the public-workflow contract;
all other bytes, turns and dependent tiers are preserved. Claims follow existing
policy; the observation snapshot separately records actual stage-specific codes.
