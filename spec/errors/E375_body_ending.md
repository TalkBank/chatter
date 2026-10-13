+++
code = 'E375'
name = 'Unmatched delimiters across body positions'

[[example]]
title = 'Admitted body and ending control'
level = 'utterance'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "alder: unmatched closing annotation delimiters"
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder ]]]] cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "maple: unmatched closing annotation delimiters"
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple ]]]] drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "pine: unmatched closing annotation delimiters"
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine ]]]] . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "larch: unmatched closing annotation delimiters"
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch ]]]] elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"
+++

## Description

Closing square brackets without an opening annotation are not valid spoken material. These specimens retain an unparsed ERROR region, not a typed scoped annotation whose decoder could justify E375. They preserve the established E375 unmatched-delimiter subsumption policy: E316 owns rejection. These paired variants exercise the existing scoped-annotation boundary at distinct body positions, including a nested group.

## CHAT Rule

Closing square brackets without an opening annotation are not valid spoken material. These paired variants exercise the existing scoped-annotation boundary at distinct body positions, including a nested group.

## Notes

The clean source is `corpus/reference/content/body-ending-controls.cha`.
The source-bound integration contracts verify each generated example against
an edit capability issued by that parsed and validated control. Observed CST
placement and emitted stages are evidence separate from these authored claims.
