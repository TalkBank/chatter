# com_continuation_leading_space

Dependent tier continued onto a second line whose tab is followed by an
incidental space. The space is absorbed by the `continuation` token and is
not content.

A CHAT tier continues when the next line begins with a tab. Transcribers
routinely leave a space after that tab, so the continuation reads
`\t <content>`. CHAT declares no implicit extras, so before 2026-07-29 that
space was left as unparsable tier content: E316, then E330, and the resulting
parse taint SUPPRESSED main/%mor and %mor/%gra alignment checking (E600). One
meaningless keystroke therefore disabled real validation on the file.

Adjudicated a chatter defect rather than deliberate strictness: CLAN CHECK's
verdict is byte-identical with and without the space, and a space before free
text on a comment tier cannot fail to make sense. Fixed in `grammar.js`'s
`continuation` token and its re2c mirror in `lexer.re`.

Scope is narrow on purpose. Spaces only, never a second tab, which is
structural (`tier_sep` is colon plus tab). A space BEFORE the tab is not a
continuation at all. Only the three rules referencing `$.continuation`
(`text_with_bullets_and_pics`, `text_with_bullets`, `free_text`) are affected,
all free-text surfaces where no interior spacing is load-bearing.

Found in: childes-romance-germanic (German/Szagun/CI), childes-eng-na
(Eng-NA/Rowe). 8 files across 2 corpora.

## Input

```com_dependent_tier
%com:	Mot und Cla reden .
	 00:45:00
```

## Expected CST

```cst
(com_dependent_tier
  (com_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (text_with_bullets_and_pics
    (text_segment)
    (continuation)
    (text_segment)
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
