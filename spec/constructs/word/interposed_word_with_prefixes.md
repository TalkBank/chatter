# interposed_word_with_prefixes

An interposed word uses the same `standalone_word` production as an ordinary
word. Its payload therefore permits filler (`&-`), nonword (`&~`), fragment
(`&+`) and omission (`0`) prefixes. Prefixes compose with form, language and
part-of-speech suffixes. The final period belongs to the utterance even when
there is no space before it.

## Input

```main_tier
*CHI:	&*INV:&-uh &*INV:&~hmm &*INV:&+sh &*INV:0is &*INV:&-uh@i &*INV:&~hmm@s:eng$n.
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
          (other_spoken_event
            (ampersand)
            (star)
            (speaker)
            (colon)
            (standalone_word
              (word_prefix)
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
              (word_prefix)
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
              (word_prefix)
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
              (zero)
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
              (word_prefix)
              (word_body
                (word_segment))
              (form_marker)))))
      (whitespaces)
      (content_item
        (base_content_item
          (other_spoken_event
            (ampersand)
            (star)
            (speaker)
            (colon)
            (standalone_word
              (word_prefix)
              (word_body
                (word_segment))
              (word_lang_suffix)
              (pos_tag))))))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
