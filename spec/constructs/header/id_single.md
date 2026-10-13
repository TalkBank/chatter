# id_single

Minimal `@ID` header with identity fields populated.

## Input

```id_header
@ID:	eng|corpus|CHI|||||Target_Child|||
```

## Expected CST

```cst
(id_header
  (id_prefix)
  (header_sep
    (colon)
    (tab))
  (id_contents
    (id_languages
      (id_language
        (language_code)))
    (pipe)
    (id_corpus)
    (pipe)
    (id_speaker)
    (pipe)
    (pipe)
    (pipe)
    (pipe)
    (pipe)
    (id_role)
    (pipe)
    (pipe)
    (pipe))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
