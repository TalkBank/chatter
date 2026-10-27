# wor_self_interrupted_question

Authored combination of the existing `with_mor` main-tier word and the
supported `%wor` `self_interrupted_question` construct. Both tiers retain the declared
ending or separator. This is a parsing control, not external corpus attestation.

## Input

```utterance
*CHI:	hello +//?
%wor:	hello +//?
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
        (self_interrupted_question)
        (newline))))
  (wor_dependent_tier
    (wor_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (wor_tier_body
      (wor_word_item
        (standalone_word
          (word_body
            (word_segment))))
      (whitespaces)
      (self_interrupted_question)
      (newline))))
```

## Metadata

- **Level**: utterance
- **Category**: utterance
