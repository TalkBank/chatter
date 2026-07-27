# gra_incroot

The grammar accepts any relation label, here the retired `INCROOT`.

This construct exists to pin that property, and it is deliberately NOT a
statement that `INCROOT` is current CHAT: the label is extinct in the corpora
(zero occurrences in 138,565,864 relation instances, full-corpus survey
2026-07-26) and `chatter validate` rejects it as a non-UD relation head
(`E761`).

Relation labels are open text at the GRAMMAR layer on purpose. The legal head
vocabulary is a validation policy over a closed set of 37 Universal
Dependencies relations, with language-specific subtypes left open by UD's own
design, and a policy of that shape does not belong in a syntax. Keeping this
construct means a change that accidentally closed the label set in the grammar
would fail here rather than in a distant validation test.

## Input

```gra_dependent_tier
%gra:	1|0|INCROOT 2|1|PUNCT
```

## Expected CST

```cst
(gra_dependent_tier
  (gra_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (gra_contents
    (gra_relation
      (gra_index)
      (pipe)
      (gra_head)
      (pipe)
      (gra_relation_name)
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (gra_relation
      (gra_index)
      (pipe)
      (gra_head)
      (pipe)
      (gra_relation_name)
    )
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
