# mor_interruption

Authored combination of the existing `with_mor` example and the supported
`+/.` terminator. Both the main tier and MOR tier retain `interruption`;
this is a parsing control, not a claim of attestation in external corpus data.

## Input

```utterance
*CHI:	hello +/.
%mor:	co|hello +/.
```

## Expected CST

```cst
(utterance
  (main_tier
    (star)
    (speaker)
    (colon)
    (tab)
    (tier_body
      (contents
        (content_item
          (base_content_item
            (word_with_optional_annotations
              (standalone_word
                (word_body
                  (word_segment))))))
        (whitespaces))
      (utterance_end
        (interruption)
        (newline))))
  (mor_dependent_tier
    (mor_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (mor_contents
      (mor_content
        (mor_word
          (mor_pos)
          (pipe)
          (mor_lemma)))
      (whitespaces)
      (interruption))
    (newline)))
```

## Metadata

- **Level**: utterance
- **Category**: utterance
