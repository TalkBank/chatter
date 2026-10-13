# annotation_payload_positions

Authored control: alternative transcription can be the first scoped annotation,
with a following comment retaining its own payload and position. Both apply to
the preceding word, not to a replacement or a new content item.
The [CHAT manual](https://talkbank.org/0info/manuals/CHAT.html) documents
alternative transcription and scoped comments separately from word replacement.

## Input

```main_tier
*CHI:	bird [=? lark] [% guess] .
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
                (word_segment)))
            (base_annotations
              (whitespaces)
              (alt_annotation
                (space)
                (annotation_content)
                (right_bracket))
              (whitespaces)
              (percent_annotation
                (space)
                (annotation_content)
                (right_bracket))))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
