+++
code = 'E360'
name = 'Rejected terminal bullets retain their source owner'

[[example]]
title = 'Admitted body and ending control'
level = 'utterance'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "Unsupported legacy terminal skip bullet"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600-\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "Overflowing terminal timestamp"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_18446744073709551616\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"
+++

## Description

The structured terminal bullet rejects unsupported legacy skip notation and timestamp overflow. Rejection does not authorize invented timing or removal of healthy surrounding turns, dependent notes or final postcodes.

## CHAT Rule

The structured terminal bullet rejects unsupported legacy skip notation and timestamp overflow. Rejection does not authorize invented timing or removal of healthy surrounding turns, dependent notes or final postcodes.

## Notes

The clean source is `corpus/reference/content/body-ending-controls.cha`.
The source-bound integration contracts verify each generated example against
an edit capability issued by that parsed and validated control. Observed CST
placement and emitted stages are evidence separate from these authored claims.
