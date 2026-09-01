#!/usr/bin/env python3
"""mdBook preprocessor: git-derived "last changed" dates for the book and every page.

# Why this exists

Every page of the book carries a hand-written `**Last modified:**` header, and
the front page's header was being read as the date of the BOOK. It is not: it
is the date that one page was last edited, so a book whose front page has been
stable for two months looks two months stale on the day a dozen other pages
changed. A hand-maintained date can only ever describe the file it sits in.

The honest answer to "when did this change" is git's, for two different
questions:

- **this page**: the last commit that touched the page's own source file;
- **the book**: the last commit that touched anything under the book's source
  tree (`src/`, which includes `SUMMARY.md`).

This preprocessor computes both from `git log` at build time, substitutes them
where a page asks for them, and appends both to every page as a footer. Nothing
about a date is committed as a value, so nothing can go stale.

# What a page can ask for

Two placeholders, substituted anywhere in a chapter's markdown:

    {{git-dates:page}}   the page's own last change
    {{git-dates:book}}   the whole book's last change

Each renders as `YYYY-MM-DD (commit <short hash>)`, with the hash linked to the
commit when `output.html.git-repository-url` is configured. Any other
`{{git-dates:...}}` key is a typo and FAILS the build rather than rendering a
literal placeholder that nobody would notice.

A page never committed (a new file in a working tree) renders as
`not yet committed`, a distinct variant, never a fabricated date.

# Why it refuses a shallow clone

`git log -1 -- <path>` in a depth-1 checkout reports the checkout commit for
every file, so every page would carry today's date and the build would look
fine. That is the failure this script exists to end, so it refuses to run and
names the fix (`fetch-depth: 0`), following `check_doc_dates.py`.

# Verify mode

`verify` recomputes the two dates from git and asserts that a rendered HTML
file carries them, so the build target can prove the preprocessor ran and
substituted the right values, rather than trusting that book.toml still lists
it. It is the test at the real boundary: the rendered page.

# Wire format

The walk over the book JSON is generic (any object carrying a string `content`
and a `source_path` is a chapter) so the same file serves mdBook 0.4 (`sections`)
and 0.5 (`items`) without knowing either layout.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

#: Placeholder prefix; the key after the colon selects which date.
PLACEHOLDER_PREFIX = "{{git-dates:"
PAGE_PLACEHOLDER = PLACEHOLDER_PREFIX + "page}}"
BOOK_PLACEHOLDER = PLACEHOLDER_PREFIX + "book}}"

#: Footer wording. One owner: the preprocessor writes it and `verify` looks for it.
PAGE_LABEL = "This page last changed: "
BOOK_LABEL = "The whole book last changed: "

#: mdBook's documented default for `[book] src`. mdBook 0.5 omits the key from
#: the context it sends preprocessors, so the default is applied here on the
#: same terms mdBook itself applies it.
DEFAULT_SRC = "src"


@dataclass(frozen=True)
class Commit:
    """The last commit that touched a path: its committer date and short hash."""

    date: str
    short_hash: str


@dataclass(frozen=True)
class NeverCommitted:
    """No commit has ever touched the path: it exists only in the working tree."""


LastChange = Commit | NeverCommitted


def git(args: list[str], cwd: Path) -> str:
    """Run git in `cwd`; a failure aborts the build with git's own message."""
    result = subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        sys.exit(
            f"mdbook_git_dates: git {' '.join(args)} failed in {cwd}:\n{result.stderr}"
        )
    return result.stdout.strip()


def refuse_if_shallow(src_dir: Path) -> None:
    """A shallow clone would date every page with the checkout commit; refuse."""
    if git(["rev-parse", "--is-shallow-repository"], src_dir) == "true":
        sys.exit(
            "mdbook_git_dates: this is a SHALLOW clone, so per-file commit dates are "
            "not knowable and every page would carry the checkout's date. "
            "Check out with full history (`fetch-depth: 0`)."
        )


def last_change(src_dir: Path, rel_path: str) -> LastChange:
    """The last commit touching `rel_path` (relative to `src_dir`; `.` is the whole tree)."""
    out = git(["log", "-1", "--format=%cs %h", "--", rel_path], src_dir)
    if not out:
        return NeverCommitted()
    date, short_hash = out.split(" ", 1)
    return Commit(date=date, short_hash=short_hash)


def render(change: LastChange, repo_url: str | None) -> str:
    """Markdown for one date; the hash links to the commit when the repo URL is known."""
    match change:
        case Commit(date=date, short_hash=short_hash):
            if repo_url is None:
                return f"{date} (commit {short_hash})"
            return f"{date} (commit [{short_hash}]({repo_url.rstrip('/')}/commit/{short_hash}))"
        case NeverCommitted():
            return "not yet committed"


def chapters(node: object) -> Iterator[dict[str, object]]:
    """Every chapter object in the book JSON, whatever the top-level layout.

    Draft chapters carry `source_path: null` and are skipped: there is no file
    whose history could be asked for.
    """
    if isinstance(node, dict):
        if isinstance(node.get("content"), str) and node.get("source_path") is not None:
            yield node
        for value in node.values():
            yield from chapters(value)
    elif isinstance(node, list):
        for item in node:
            yield from chapters(item)


def repo_url_of(ctx: dict[str, object]) -> str | None:
    """`output.html.git-repository-url` from the preprocessor context, if configured."""
    config = ctx.get("config")
    if not isinstance(config, dict):
        return None
    output = config.get("output")
    if not isinstance(output, dict):
        return None
    html = output.get("html")
    if not isinstance(html, dict):
        return None
    url = html.get("git-repository-url")
    return url if isinstance(url, str) else None


def src_dir_of(ctx: dict[str, object]) -> Path:
    """The book's source directory: `root` joined with `[book] src` (or mdBook's default)."""
    root = ctx.get("root")
    if not isinstance(root, str):
        sys.exit("mdbook_git_dates: preprocessor context carries no `root`")
    config = ctx.get("config")
    book = config.get("book") if isinstance(config, dict) else None
    src = book.get("src") if isinstance(book, dict) else None
    return Path(root) / (src if isinstance(src, str) else DEFAULT_SRC)


def stamp(content: str, page: str, book: str, source_path: str) -> str:
    """Substitute the placeholders and append the footer; an unknown key fails."""
    content = content.replace(PAGE_PLACEHOLDER, page).replace(BOOK_PLACEHOLDER, book)
    if PLACEHOLDER_PREFIX in content:
        start = content.index(PLACEHOLDER_PREFIX)
        end = content.find("}}", start)
        bad = content[start : end + 2 if end != -1 else start + 40]
        sys.exit(
            f"mdbook_git_dates: {source_path}: unknown placeholder {bad!r}; "
            f"the keys are {PAGE_PLACEHOLDER!r} and {BOOK_PLACEHOLDER!r}"
        )
    return f"{content.rstrip()}\n\n---\n\n<small>{PAGE_LABEL}{page}. {BOOK_LABEL}{book}.</small>\n"


def preprocess() -> int:
    """Read `[context, book]` from stdin, stamp every chapter, write the book to stdout."""
    ctx, book = json.load(sys.stdin)
    if not isinstance(ctx, dict):
        sys.exit("mdbook_git_dates: preprocessor context is not an object")
    src_dir = src_dir_of(ctx)
    refuse_if_shallow(src_dir)
    repo_url = repo_url_of(ctx)
    book_rendered = render(last_change(src_dir, "."), repo_url)
    for chapter in chapters(book):
        source_path = chapter["source_path"]
        content = chapter["content"]
        if not isinstance(source_path, str) or not isinstance(content, str):
            continue
        page_rendered = render(last_change(src_dir, source_path), repo_url)
        chapter["content"] = stamp(content, page_rendered, book_rendered, source_path)
    json.dump(book, sys.stdout)
    return 0


def verify(book_root: Path, page: str, rendered: Path) -> int:
    """Assert the rendered HTML for `page` carries both git-derived dates."""
    src_dir = book_root / DEFAULT_SRC
    refuse_if_shallow(src_dir)
    html = rendered.read_text(encoding="utf-8")
    expectations = {
        "page": (PAGE_LABEL, last_change(src_dir, page)),
        "book": (BOOK_LABEL, last_change(src_dir, ".")),
    }
    missing: list[str] = []
    for what, (label, change) in expectations.items():
        match change:
            case Commit(date=date, short_hash=short_hash):
                needle = f"{label}{date} (commit "
                if needle not in html or f">{short_hash}<" not in html:
                    missing.append(f"{what}: expected {needle!r} and hash {short_hash}")
            case NeverCommitted():
                missing.append(f"{what}: {page} has never been committed")
    if missing:
        for line in missing:
            print(f"mdbook_git_dates verify: {rendered}: {line}", file=sys.stderr)
        return 1
    print(f"mdbook_git_dates verify: ok ({rendered})")
    return 0


def main(argv: list[str]) -> int:
    # mdBook's protocol: `<cmd> supports <renderer>` asks whether to run for a
    # renderer; exit 0 says yes. This preprocessor only edits markdown, which
    # every renderer consumes, so it supports all of them.
    if argv[:1] == ["supports"]:
        return 0
    if argv[:1] == ["verify"]:
        parser = argparse.ArgumentParser(prog="mdbook_git_dates.py verify")
        parser.add_argument(
            "--book-root",
            type=Path,
            required=True,
            help="the directory holding book.toml",
        )
        parser.add_argument(
            "--page", required=True, help="page path relative to the book's src/"
        )
        parser.add_argument(
            "rendered", type=Path, help="the rendered HTML file for that page"
        )
        args = parser.parse_args(argv[1:])
        return verify(args.book_root, args.page, args.rendered)
    if argv:
        sys.exit(f"mdbook_git_dates: unexpected arguments {argv!r}")
    return preprocess()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
