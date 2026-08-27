#!/usr/bin/env python3
"""Extract the COMPLETE CLAN CHECK error-code reference directly from source.

The hand-maintained ``OSX-CLAN/CHECK-rules.md`` claims to be a "Complete Error
Reference" but drifted stale: it listed ~87 codes while ``check.cpp`` actually
defines 161 and emits ~135. A documentation-level chatter<->CHECK parity audit
that diffs against that stale doc therefore cannot see the missing codes (e.g.
119 "Missing word after code", the dangling-retrace check). This generator
reads ``check.cpp`` itself so the reference is reproducible and never silently
stale: re-run it whenever the CLAN sources are refreshed.

Two things are parsed from ``src/clan/check.cpp``:

1. ``check_mess()`` print switch (``case N: fprintf(fpout, "...");``) -> the
   message text(s) for every defined code. Dual cases of the form
   ``if (err_itm[0] == EOS) <generic> else <specific>`` yield two messages.
2. ``check_err(N, ...)`` call sites -> which codes are actually emitted, and how
   many trigger sites each has (rough proxy for how reachable a code is).

Outputs (paths given on the command line): a JSON model and a Markdown table.

Usage:
    extract_check_codes.py CHECK_CPP OUT_JSON OUT_MD
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

# A `case N:` label. Comments are masked out before these run (see
# `mask_comments`), so a commented-out label cannot match.
CASE_RE = re.compile(r"^\s*case\s+(\d+)\s*:")
# The switch ends at its `default:` label.
DEFAULT_RE = re.compile(r"^\s*default\s*:")
# First string-literal argument of an fprintf (the format string). Handles
# escaped quotes inside the literal.
FPRINTF_RE = re.compile(r'fprintf\s*\(\s*\w+\s*,\s*"((?:\\.|[^"\\])*)"')
# Trailing 0xNN byte arguments that back `%c%c%c` runs in unmatched-char msgs.
BYTE_ARG_RE = re.compile(r"0x([0-9A-Fa-f]{2})")
TRAILING_CODE_RE = re.compile(r"\s*\(%d\)\s*$")


@dataclass
class CheckCode:
    """One CLAN CHECK error code: its defined message(s) and emission sites."""

    code: int
    messages: list[str] = field(default_factory=list)
    call_sites: list[int] = field(default_factory=list)  # source line numbers


def select_unix_build(text: str) -> str:
    """Blank the preprocessor regions the unix CLAN build does not compile.

    The generator answers "what does the COMPILED UNIX build do", and comments
    are only one of the ways source text is not compiled. `check.cpp` has 18
    `UNX`-conditioned regions, and unix CLAN builds with `-DUNX` (see the CFLAGS
    in the unix build's CFLAGS), so an `#ifndef UNX` block is GUI-only
    code that no unix binary contains. Counting a `check_err` inside one is the
    same error as counting a commented-out call site, arriving one stage later.

    `unifdef -b` does exactly this and preserves line numbers, so it composes
    with `mask_comments` and `call_site_lines` stays honest. It is used rather
    than a hand-written `#if` evaluator because nested conditionals, `#elif` and
    `defined()` expressions are easy to get subtly wrong, and a wrong answer
    here is invisible: it looks like an ordinary count.

    A missing `unifdef` is a hard error. Falling back to the unprocessed text
    would silently reintroduce the very over-counting this exists to remove.
    """
    if shutil.which("unifdef") is None:
        raise SystemExit(
            "unifdef not found. It is needed to exclude the GUI-only "
            "`#ifndef UNX` regions from the unix build's view of check.cpp; "
            "without it the reference over-counts call sites that no unix "
            "binary contains. Install it (macOS ships it at /usr/bin/unifdef; "
            "Debian/Ubuntu: apt install unifdef)."
        )
    # -b blanks excluded lines instead of deleting them, preserving line numbers.
    # unifdef exits 0 when unchanged, 1 when it changed something, >1 on error.
    result = subprocess.run(
        ["unifdef", "-b", "-DUNX", "-U_MAC_CODE", "-U_WIN32"],
        input=text,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode > 1:
        raise SystemExit(
            f"unifdef failed ({result.returncode}): {result.stderr.strip()}"
        )
    return result.stdout


def mask_comments(text: str) -> list[str]:
    """Blank out C comments, preserving line count, columns, and string literals.

    Every question this generator answers is "what does the COMPILED source do",
    so commented-out code must be invisible to it. Replacing comment bytes with
    spaces (rather than deleting them) keeps `call_site_lines` reporting real
    1-based line numbers in the original file.

    String literals are stepped over rather than masked, because the message
    switch is read out of `fprintf` format strings: a `/*` inside a literal is
    text, not a comment, and blanking literals would erase the messages.

    Why this exists: the earlier version skipped lines matching `^\\s*//` and
    nothing else. `check.cpp` retires code with dated `/* ... */` BLOCKS, so
    eleven codes retired between 1998 and 2025 still counted as live call sites,
    and CHECK 76's retirement on 2026-08-07 would have been the twelfth. A
    generator that cannot see a comment cannot answer "can CLAN still emit this".
    """
    out: list[str] = []
    line: list[str] = []
    i, n = 0, len(text)
    in_block = False
    in_line_comment = False
    quote: str | None = None  # active string/char delimiter, if any

    while i < n:
        ch = text[i]
        nxt = text[i + 1] if i + 1 < n else ""

        if ch == "\n":
            out.append("".join(line))
            line = []
            in_line_comment = False  # a line comment ends at the newline
            i += 1
            continue

        if in_block:
            if ch == "*" and nxt == "/":
                in_block = False
                line.append("  ")
                i += 2
                continue
            line.append(" ")
            i += 1
            continue

        if in_line_comment:
            line.append(" ")
            i += 1
            continue

        if quote is not None:
            line.append(ch)
            # A backslash escapes the next byte, including the delimiter itself.
            if ch == "\\" and nxt:
                line.append(nxt)
                i += 2
                continue
            if ch == quote:
                quote = None
            i += 1
            continue

        if ch in ('"', "'"):
            quote = ch
            line.append(ch)
            i += 1
            continue
        if ch == "/" and nxt == "*":
            in_block = True
            line.append("  ")
            i += 2
            continue
        if ch == "/" and nxt == "/":
            in_line_comment = True
            line.append("  ")
            i += 2
            continue

        line.append(ch)
        i += 1

    out.append("".join(line))
    return out


def _decode_byte_chars(fmt: str, line: str) -> str:
    """Substitute `%c%c%c` runs with the UTF-8 char from trailing 0xNN args.

    Messages like ``"Unmatched %c%c%c found on the tier."`` pass the three
    bytes of a single UTF-8 character (e.g. 0xE2,0x80,0xB9 -> the char) as
    separate ``%c`` args. Decode them so the reference shows the real glyph.
    """
    if "%c" not in fmt:
        return fmt
    byte_vals = [int(h, 16) for h in BYTE_ARG_RE.findall(line)]
    if not byte_vals:
        return fmt
    try:
        glyph = bytes(byte_vals).decode("utf-8")
    except UnicodeDecodeError:
        return fmt
    # Collapse each maximal run of %c into the decoded glyph (one glyph here).
    return re.sub(r"(?:%c)+", glyph, fmt)


def _clean(fmt: str, line: str) -> str:
    """Turn a raw fprintf format string into a human-readable message."""
    msg = fmt.replace('\\"', '"')
    msg = _decode_byte_chars(msg, line)
    msg = msg.replace("\\n", " ").replace("\\t", " ")
    msg = TRAILING_CODE_RE.sub("", msg)
    return " ".join(msg.split()).strip()


def parse_messages(lines: list[str]) -> dict[int, list[str]]:
    """Walk the check_mess() switch, mapping each code to its message text(s)."""
    codes: dict[int, list[str]] = {}
    current: int | None = None
    started = False
    for line in lines:
        m = CASE_RE.match(line)
        if m:
            current = int(m.group(1))
            started = True
            codes.setdefault(current, [])
            # The label line itself may carry the fprintf (single-line cases).
            for fmt in FPRINTF_RE.findall(line):
                msg = _clean(fmt, line)
                if msg:
                    codes[current].append(msg)
            continue
        if started and DEFAULT_RE.match(line):
            break
        if current is not None:
            for fmt in FPRINTF_RE.findall(line):
                msg = _clean(fmt, line)
                if msg and msg not in codes[current]:
                    codes[current].append(msg)
    return codes


def _split_args(text: str) -> list[str]:
    """Split a call's argument text on top-level commas."""
    args, depth, current = [], 0, []
    for ch in text:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        if ch == "," and depth == 0:
            args.append("".join(current).strip())
            current = []
        else:
            current.append(ch)
    args.append("".join(current).strip())
    return args


def _call_args(text: str, start: int) -> tuple[str, int] | None:
    """Return the argument text of the call whose `(` follows `start`."""
    open_paren = text.find("(", start)
    if open_paren == -1:
        return None
    depth = 0
    for i in range(open_paren, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return text[open_paren + 1 : i], i
    return None


# A `#define NAME(params)` or a C function definition `... NAME(params) {`.
MACRO_DEF_RE = re.compile(r"^\s*#\s*define\s+(\w+)\(", re.MULTILINE)
FUNC_DEF_RE = re.compile(r"^[A-Za-z_][\w \t*]*?\b(\w+)\s*\(", re.MULTILINE)


def find_code_carrying_aliases(text: str) -> dict[str, int]:
    """Find functions and macros that forward a CODE parameter to `check_err`.

    A code reaches `check_err` three ways in `check.cpp`, and only the first is
    visible to a literal scan:

    1. directly, `check_err(119, ...)`;
    2. through a MACRO that forwards its own parameter, as
       `#define check_trans_err(wh,...) check_err(wh,...)`;
    3. through a FUNCTION that takes the code as a parameter, as
       `check_isLangMatch(char *langs, long ln, int s, int wh, ...)`, whose body
       calls `check_err(wh, ...)`, invoked as `check_isLangMatch(word, ln, s, 122, FALSE)`.

    So this returns `{name: parameter_index}` for every such alias, iterated to a
    fixpoint so an alias of an alias is found too.

    Why it matters: without this, codes reachable ONLY through 2 or 3 read as
    "defined but never emitted". CHECK 16 (macro) and 122/152 (function) are
    exactly that, and each previously needed a hand-written per-entry exception
    in chatter's parity manifest. A rule the generator applies cannot be
    forgotten when a fourth code acquires a wrapper.
    """
    aliases: dict[str, int] = {}
    # Seed with the real emitter: `check_err`'s own code is its first argument.
    known: dict[str, int] = {"check_err": 0}

    definitions: list[tuple[str, list[str], str]] = []
    for match in MACRO_DEF_RE.finditer(text):
        name = match.group(1)
        parsed = _call_args(text, match.end() - 1)
        if not parsed:
            continue
        params = [p.strip() for p in _split_args(parsed[0])]
        # A macro body runs to the first line not ending in a backslash.
        body_lines = []
        for line in text[parsed[1] :].splitlines():
            body_lines.append(line)
            if not line.rstrip().endswith("\\"):
                break
        definitions.append((name, params, "\n".join(body_lines)))

    for match in FUNC_DEF_RE.finditer(text):
        name = match.group(1)
        parsed = _call_args(text, match.end() - 1)
        if not parsed:
            continue
        param_text, close = parsed
        rest = text[close + 1 :]
        if not rest.lstrip().startswith("{"):
            continue  # a declaration or a call, not a definition
        # Parameter NAMES are the last identifier of each declarator.
        params = []
        for param in _split_args(param_text):
            ident = re.findall(r"\w+", param)
            params.append(ident[-1] if ident else "")
        brace = rest.index("{")
        depth, end = 0, len(rest)
        for i in range(brace, len(rest)):
            if rest[i] == "{":
                depth += 1
            elif rest[i] == "}":
                depth -= 1
                if depth == 0:
                    end = i
                    break
        definitions.append((name, params, rest[brace:end]))

    changed = True
    while changed:
        changed = False
        for name, params, body in definitions:
            if name in known:
                continue
            for emitter, index in list(known.items()):
                for call in re.finditer(rf"\b{re.escape(emitter)}\s*\(", body):
                    parsed = _call_args(body, call.end() - 1)
                    if not parsed:
                        continue
                    args = _split_args(parsed[0])
                    if index < len(args) and args[index] in params and args[index]:
                        aliases[name] = params.index(args[index])
                        known[name] = aliases[name]
                        changed = True
                        break
                if name in known:
                    break
    return aliases


def parse_call_sites(lines: list[str], defined: set[int]) -> dict[int, list[int]]:
    """Map each code to the source lines that emit it.

    A code is emitted either directly via ``check_err(N, ...)`` or indirectly:
    helper functions ``return(N)`` an error code that the caller then feeds to
    ``check_err`` (e.g. 124 "remove unlinked" is only reached via ``return(124)``).
    ``return`` matches are restricted to codes that the switch actually defines,
    so ordinary ``return(0)`` / ``return TRUE`` control flow is not miscounted.
    """
    sites: dict[int, list[int]] = {}
    call_re = re.compile(r"check_err\s*\(\s*(\d+)")
    return_re = re.compile(r"return\s*\(?\s*(\d+)\s*\)?\s*;")

    # Codes that reach `check_err` through a forwarding macro or helper are
    # invisible to the literal scan; `aliases` maps each wrapper to the
    # parameter position carrying the code.
    aliases = find_code_carrying_aliases("\n".join(lines))
    text = "\n".join(lines)
    for name, index in aliases.items():
        for call in re.finditer(rf"\b{re.escape(name)}\s*\(", text):
            parsed = _call_args(text, call.end() - 1)
            if not parsed:
                continue
            args = _split_args(parsed[0])
            if index >= len(args) or not args[index].isdigit():
                continue  # forwarding another variable, not a literal code
            lineno = text.count("\n", 0, call.start()) + 1
            sites.setdefault(int(args[index]), []).append(lineno)

    for lineno, line in enumerate(lines, start=1):
        for m in call_re.finditer(line):
            sites.setdefault(int(m.group(1)), []).append(lineno)
        for m in return_re.finditer(line):
            code = int(m.group(1))
            if code in defined:
                sites.setdefault(code, []).append(lineno)
    return sites


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: extract_check_codes.py CHECK_CPP OUT_JSON OUT_MD",
            file=sys.stderr,
        )
        return 2
    src_path, out_json, out_md = (Path(a) for a in sys.argv[1:4])
    # Two stages, both before ANY parsing, and both for one reason: the
    # question is what the COMPILED UNIX BUILD does, not what the file says.
    source = src_path.read_text(encoding="utf-8", errors="replace")
    lines = mask_comments(select_unix_build(source))

    messages = parse_messages(lines)
    defined_codes = {code for code, msgs in messages.items() if msgs}
    call_sites = parse_call_sites(lines, defined_codes)

    codes: dict[int, CheckCode] = {}
    for code in sorted(set(messages) | set(call_sites)):
        codes[code] = CheckCode(
            code=code,
            messages=messages.get(code, []),
            call_sites=call_sites.get(code, []),
        )

    defined = [c for c in codes.values() if c.messages]
    emitted = [c for c in codes.values() if c.call_sites]
    defined_not_emitted = [c.code for c in defined if not c.call_sites]
    emitted_not_defined = [c.code for c in emitted if not c.messages]

    model = {
        "source": str(src_path),
        "summary": {
            "codes_defined_in_switch": len(defined),
            "codes_emitted_via_call_sites": len(emitted),
            "defined_but_never_emitted": sorted(defined_not_emitted),
            "emitted_but_no_message": sorted(emitted_not_defined),
        },
        "codes": [
            {
                "code": c.code,
                "messages": c.messages,
                "n_call_sites": len(c.call_sites),
                "call_site_lines": c.call_sites,
            }
            for c in codes.values()
        ],
    }
    out_json.write_text(
        json.dumps(model, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    md: list[str] = []
    md.append("# CLAN CHECK error codes (generated from check.cpp)\n")
    md.append(
        "Generated by `scripts/extract_check_codes.py` from "
        f"`{src_path}`. Do not edit by hand; re-run after a CLAN source refresh.\n"
    )
    md.append(
        f"- Codes defined in `check_mess()`: **{len(defined)}**\n"
        f"- Codes emitted by `check_err()` call sites: **{len(emitted)}**\n"
        f"- Defined but never emitted: {sorted(defined_not_emitted)}\n"
        f"- Emitted but no message text: {sorted(emitted_not_defined)}\n"
    )
    md.append("| code | emitted | message(s) |")
    md.append("|------|---------|------------|")
    for c in codes.values():
        emitted_flag = "yes" if c.call_sites else "no"
        msg = " / ".join(c.messages) if c.messages else "(no message; control case)"
        msg = msg.replace("|", "\\|")
        md.append(f"| {c.code} | {emitted_flag} | {msg} |")
    out_md.write_text("\n".join(md) + "\n", encoding="utf-8")

    print(
        f"defined={len(defined)} emitted={len(emitted)} wrote {out_json} and {out_md}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
