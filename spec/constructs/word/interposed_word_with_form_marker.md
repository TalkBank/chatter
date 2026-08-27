# interposed_word_with_form_marker

Interposed word (`&*SPK:text`) whose payload is a full word: lengthening
inside it, and a `@` form marker after it.

The payload of an interposed word is an ordinary `standalone_word`, so
everything a word may carry, it may carry. This example carries two of those
things at once, which is the point: a rule that handles `&*SPK:` followed by a
bare segment can still be wrong about `&*SPK:` followed by a real word.

## Input

Attested in `rhd-data`, Spanish PerLA, as the close of a long narrative turn:
the speaker describes throwing her keys on the floor and the investigator
interjects `a:nda@i`, a lengthened interjection, without taking the turn.

```main_tier
*MAM:	las tiré al suelo &*VMO:a:nda@i .
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
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (other_spoken_event
            (ampersand)
            (star)
            (speaker)
            (colon)
            (standalone_word
              (word_body
                (word_segment)
                (lengthening)
                (word_segment))
              (form_marker)))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
