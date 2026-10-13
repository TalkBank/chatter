# date_strict

`@Date` header using strict `DD-MMM-YYYY` format.

## Input

```date_header
@Date:	04-JAN-2020
```

## Expected CST

```cst
(date_header
  (date_prefix)
  (header_sep
    (colon)
    (tab))
  (date_contents
    (strict_date))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
