# registry_event_segments

Event descriptions use their own registry character class. Stress marks, the low quotation mark, the raised comma and equals are description text here; they do not split an event into a separate word.

These are authored lexical boundary controls, not claims of attestation.

## Input

```utterance
*CHI:	&=ˈcough &=ˌcough &=„cough &=‡cough &=ca=fé .
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
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: utterance
- **Category**: word
