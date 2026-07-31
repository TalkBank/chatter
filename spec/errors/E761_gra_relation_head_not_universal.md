# E761: %gra relation head is not a Universal Dependencies relation

## Description

A `%gra` relation label is `HEAD` or `HEAD-SUBTYPE`. Universal Dependencies
fixes the HEAD set at 37 universal relations and deliberately defines
SUBTYPES as language-specific and open-ended, so the head is the only part of
a label that can be checked against a closed vocabulary. This rule checks the
head and never the subtype.

Nothing validated relation labels before, in chatter or in CLAN CHECK, so a
corrupted label rode silently into every downstream analysis that reads the
dependency graph. The motivating case was a real one found in the wild
corpora: `13|3|PUNCTT`, a hand-edit typo for `PUNCT` that both validators
passed.

The rule is deliberately about vocabulary alone. It does not depend on the
tier's tree shape (`E721`-`E724`) or on its cardinality agreeing with `%mor`
(`E720`), and it is not suppressed when those fail: a label is wrong or right
regardless, and hiding it behind an unrelated diagnostic would leave the
transcriber fixing the wrong thing.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-26 22:49 EDT

- **Error Code**: E761
- **Category**: Dependent tier validation
- **Level**: tier
- **Layer**: validation
- **Kind**: Invalidity

## Example 1

**Trigger**: a relation head that is a typo for a universal relation.

**Expected Error Codes**: E761

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the dog .
%mor:	det|the-Def-Art noun|dog .
%gra:	1|2|DET 2|0|ROOT 3|2|PUNCTT
@Comment:	ERROR: PUNCTT is a typo for the universal relation PUNCT
@End
```

## Example 2

**Trigger**: a truncated relation head.

**Expected Error Codes**: E761

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	give me it .
%mor:	verb|give pron|me-Prs-Acc-S1 pron|it-Prs-S3 .
%gra:	1|0|ROOT 2|1|IOB 3|1|OBJ 4|1|PUNCT
@Comment:	ERROR: IOB is a truncation of the universal relation IOBJ
@End
```

## Example 3

**Trigger**: a retired TalkBank relation label.

**Expected Error Codes**: E761

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I want cookies .
%mor:	pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres noun|cookie-Plur .
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
@Comment:	ERROR: SUBJ is the retired TalkBank label; UD writes NSUBJ
@End
```

## Expected Behavior

- **Parser**: unaffected. Relation labels are open text at the grammar layer
  on purpose (see the `gra_incroot` construct); a closed vocabulary is a
  validation policy, not a syntax.
- **Validator**: reports E761 once per offending relation, naming the head
  separately from the full label when the label carries a subtype. A tier
  produced by a broken tagger can hold several distinct bad labels and each
  is a separate thing to fix, so the check does not stop at the first.
- **Suppression**: none. A parse-tainted `%gra` tier is skipped, because its
  contents are recovery output rather than authored text.

## CHAT Rule

The `%gra` tier encodes Universal Dependencies. The 37 universal relations
are: ACL, ADVCL, ADVMOD, AMOD, APPOS, AUX, CASE, CC, CCOMP, CLF, COMPOUND,
CONJ, COP, CSUBJ, DEP, DET, DISCOURSE, DISLOCATED, EXPL, FIXED, FLAT,
GOESWITH, IOBJ, LIST, MARK, NMOD, NSUBJ, NUMMOD, OBJ, OBL, ORPHAN,
PARATAXIS, PUNCT, REPARANDUM, ROOT, VOCATIVE, XCOMP.

Membership is tested case-insensitively, and a label may append a
language-specific subtype after a hyphen (`NMOD-POSS`, `ACL-RELCL`,
`FLAT-FOREIGN`). The split is at the FIRST hyphen only, so a multi-part
subtype stays whole.

Wild-data impact at adoption (full-corpus survey, 2026-07-26, over the
pre-parsed JSON mirror: 106,158 files, 70,802 of them carrying a `%gra`
tier, 138,565,864 relation instances): 150 distinct labels and 40 distinct
heads occur. All 37 universal heads are attested. Exactly three heads fall
outside the set, in 99 files:

| head | instances | files | reading |
|---|---:|---:|---|
| IOB | 146 | 93 | truncation of IOBJ, mostly Italian corpora |
| PAD | 5 | 5 | Basque, `Other/Basque` |
| PUNCTT | 1 | 1 | typo for PUNCT, `Frogs/English-ECSC` |

No retired TalkBank label (SUBJ, JCT, COORD, INCROOT, POBJ, MOD) survives
anywhere in the corpora, which is what licenses treating the universal set as
closed rather than as a recommendation. All 152 flagged instances are
defects; they join the data-cleanup queue.
