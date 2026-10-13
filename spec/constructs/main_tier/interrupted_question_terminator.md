# interrupted_question_terminator

Main tier ending with interrupted-question terminator `+/?`.

## Input

```main_tier
*CHI:	hello +/?
```

## Expected CST

```cst
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
      (interrupted_question)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
