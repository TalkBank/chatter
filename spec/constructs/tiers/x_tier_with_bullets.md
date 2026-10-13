# x_tier_with_bullets

User-defined `%x...` dependent tier containing media bullet timing markers.
The NAK-delimited timing (`\u0015start_end\u0015`) must be preserved as
inline bullets, not consumed as plain text.

Found in: aphasia-data/English/Dysphagia (643+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```utterance
*SLP:	0 .
%xs1:	swallow 3093_4877
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
            (nonword_with_optional_annotations
              (nonword
                (zero)))))
        (whitespaces))
      (utterance_end
        (period)
        (newline))))
  (x_dependent_tier
    (x_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (text_with_bullets
      (text_segment)
      (bullet
        (bullet_start)
        (bullet_timestamp)
        (bullet_timestamp)
        (bullet_end)))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
