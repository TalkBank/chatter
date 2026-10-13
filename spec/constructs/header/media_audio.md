# media_audio

`@Media` header with filename and media type.

## Input

```media_header
@Media:	sample, audio
```

## Expected CST

```cst
(media_header
  (media_prefix)
  (header_sep
    (colon)
    (tab))
  (media_contents
    (media_filename)
    (comma)
    (whitespaces)
    (media_type
      (audio_value)))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
