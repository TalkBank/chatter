+++
code = 'E316'
name = 'Malformed body and ending recovery'

[[example]]
title = 'Admitted body and ending control'
level = 'utterance'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "alder: replace one word with a bare marker"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\t& cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "maple: replace one word with a bare marker"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor & drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "pine: replace one word with a bare marker"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow & . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "larch: replace one word with a bare marker"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <& elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "alder: unclosed replacement before the ending"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder [: branch cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "maple: unclosed replacement before the ending"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple [: branch drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "pine: unclosed replacement before the ending"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine [: branch . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "larch: unclosed replacement before the ending"
level = 'utterance'
claim = { subsumed_by = 'E342' }
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch [: branch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "A word after the final terminator"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . stray [+ fifth] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "Malformed first final postcode"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ ] [+ final] \u0015500_600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"

[[example]]
title = "Malformed terminal bullet syntax"
level = 'utterance'
claim = 'violates'
notes = 'One admitted word or ending token is replaced; all other source bytes remain fixed. Healthy native carriers, not raw unchanged lines alone, govern retention.'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tSPK Speaker\n@ID:\teng|sample|SPK|||||Speaker|||\n@Media:\tbody-ending-controls, audio\n@Comment:\tPréface café\n*SPK:\tcafé opening .\n%com:\topening note\n*SPK:\talder cedar . [+ bch] [+ first] \u0015100_200\u0015\n%com:\tbegin note\n*SPK:\tanchor maple drift ? [+ second] \u0015200_300\u0015\n%com:\tmiddle note\n*SPK:\twillow pine . [+ third] \u0015300_400\u0015\n%com:\ttail note\n*SPK:\t<moss <larch elm> [!]> [!] ! [+ fourth] \u0015400_500\u0015\n%com:\tnested note\n*SPK:\thazel . [+ fifth] [+ final] \u0015500x600\u0015\n%com:\tending note\n*SPK:\tfinch closing .\n%com:\tclosing note\n@End\n"
+++

## Description

Native ERROR recovery rejects malformed content without inferring narrower syntax from unparsed text. In the nested unclosed-replacement variant, the producer instead inserts a missing right bracket and emits E342, not E316. In the unnested variants, a later postcode bracket can close the replacement; the original punctuation and codes are not thereby unchanged ending payload. The paired source keeps the same prefixes, other turns and dependent notes. The native producer decides which carriers survive; retention is required for genuinely healthy source-owned carriers, not for invented boundaries.

## CHAT Rule

Native recovery rejects malformed content without inferring narrower syntax from unparsed text. The paired source keeps the same prefixes, other turns and dependent notes. The native producer decides which carriers survive; retention is required for genuinely healthy source-owned carriers, not for invented boundaries.

## Notes

The clean source is `corpus/reference/content/body-ending-controls.cha`.
The source-bound integration contracts verify each generated example against
an edit capability issued by that parsed and validated control. Observed CST
placement and emitted stages are evidence separate from these authored claims.
