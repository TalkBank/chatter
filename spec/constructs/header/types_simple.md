# types_simple

`@Types` header with language and activity entries.

## Input

```types_header
@Types:	cross, narrative, TD
```

## Expected CST

```cst
(types_header
  (types_prefix)
  (header_sep
    (colon)
    (tab))
  (types_design)
  (comma)
  (whitespaces)
  (types_activity)
  (comma)
  (whitespaces)
  (types_group)
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
