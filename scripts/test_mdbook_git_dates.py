#!/usr/bin/env python3
"""Tests for `mdbook_git_dates.py`, at the seam mdBook actually uses.

Every test drives the script as a SUBPROCESS with `[context, book]` JSON on
stdin, against a real temporary git repository with commits at known dates,
because that is exactly how mdBook invokes it. Both wire layouts are covered
(mdBook 0.4 `sections`, mdBook 0.5 `items`), captured from real builds of a
scratch book under each version.

What these tests legitimately pin, since no type in a Python script can hold
it: the wire format between two processes, the git history the dates are read
from, the rendered footer wording that `verify` searches for, and (in
`BookTomlWiring` below) the actual `book.toml` command string against the two
cwd behaviors real mdBook releases use. They are stdlib `unittest` so the book
build in CI can run them with no test framework installed; `python3 -m pytest`
collects them too.

Run: `python3 -m unittest scripts/test_mdbook_git_dates.py`
"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().with_name("mdbook_git_dates.py")
REPO_ROOT = SCRIPT.resolve().parents[1]
BOOK_ROOT = REPO_ROOT / "book"
BOOK_TOML = BOOK_ROOT / "book.toml"
REPO_URL = "https://example.invalid/repo"

FIRST_DATE = "2026-01-10"
SECOND_DATE = "2026-02-20"


def git(args: list[str], cwd: Path, date: str | None = None) -> str:
    """Run git with a fixed identity, and a fixed commit date when given."""
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="Test",
        GIT_AUTHOR_EMAIL="test@example.invalid",
        GIT_COMMITTER_NAME="Test",
        GIT_COMMITTER_EMAIL="test@example.invalid",
        GIT_CONFIG_GLOBAL="/dev/null",
    )
    if date is not None:
        env["GIT_AUTHOR_DATE"] = f"{date}T12:00:00+00:00"
        env["GIT_COMMITTER_DATE"] = f"{date}T12:00:00+00:00"
    return subprocess.run(
        ["git", *args], cwd=cwd, env=env, capture_output=True, text=True, check=True
    ).stdout.strip()


def make_repo(root: Path) -> Path:
    """A repo whose book/src has introduction.md committed first and ch.md later.

    So the page date of introduction.md is FIRST_DATE while the book date is
    SECOND_DATE: the very divergence the preprocessor exists to show.
    """
    src = root / "book" / "src"
    src.mkdir(parents=True)
    git(["init", "-q", "-b", "main"], root)
    (src / "introduction.md").write_text(
        "# Intro\n\n**Book:** {{git-dates:book}}\n**Page:** {{git-dates:page}}\n",
        encoding="utf-8",
    )
    git(["add", "."], root)
    git(["commit", "-q", "-m", "intro"], root, date=FIRST_DATE)
    (src / "ch.md").write_text("# Ch\n\nbody\n", encoding="utf-8")
    git(["add", "."], root)
    git(["commit", "-q", "-m", "chapter"], root, date=SECOND_DATE)
    return root / "book"


def chapter(name: str, source_path: str | None, content: str) -> dict[str, object]:
    return {
        "Chapter": {
            "name": name,
            "content": content,
            "number": None,
            "sub_items": [],
            "path": source_path,
            "source_path": source_path,
            "parent_names": [],
        }
    }


def wire(
    shape: str,
    book_root: Path,
    items: list[dict[str, object]],
    repo_url: str | None = REPO_URL,
) -> str:
    """The `[context, book]` payload as mdBook 0.4 (`sections`) or 0.5 (`items`) sends it."""
    html: dict[str, object] = (
        {} if repo_url is None else {"git-repository-url": repo_url}
    )
    ctx: dict[str, object] = {
        "root": str(book_root),
        "config": {"book": {"title": "T", "src": "src"}, "output": {"html": html}},
        "renderer": "html",
        "mdbook_version": "0.0.0",
    }
    if shape == "0.4":
        book: dict[str, object] = {"sections": items, "__non_exhaustive": None}
    else:
        book = {"items": items}
    return json.dumps([ctx, book])


def run(book_root: Path, payload: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=book_root,
        input=payload,
        capture_output=True,
        text=True,
        check=False,
    )


def chapters_of(output: str) -> list[dict[str, object]]:
    """Chapter objects from the script's output, in document order, any layout."""
    book = json.loads(output)
    items = book.get("items", book.get("sections"))
    return [item["Chapter"] for item in items if isinstance(item, dict)]


class Preprocess(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.book_root = make_repo(self.root)
        self.src = self.book_root / "src"
        self.intro_hash = git(
            ["log", "-1", "--format=%h", "--", "introduction.md"], self.src
        )
        self.book_hash = git(["log", "-1", "--format=%h", "--", "."], self.src)

    def payload(self, shape: str, repo_url: str | None = REPO_URL) -> str:
        items = [
            chapter(
                "Intro", "introduction.md", (self.src / "introduction.md").read_text()
            ),
            chapter("Ch", "ch.md", (self.src / "ch.md").read_text()),
            chapter("Draft", None, ""),
        ]
        return wire(shape, self.book_root, items, repo_url)

    def test_substitutes_both_dates_and_footers_every_page_in_both_wire_shapes(
        self,
    ) -> None:
        for shape in ("0.4", "0.5"):
            with self.subTest(shape=shape):
                result = run(self.book_root, self.payload(shape))
                self.assertEqual(result.returncode, 0, result.stderr)
                intro, ch, draft = chapters_of(result.stdout)
                page = f"{FIRST_DATE} (commit [{self.intro_hash}]({REPO_URL}/commit/{self.intro_hash}))"
                book = f"{SECOND_DATE} (commit [{self.book_hash}]({REPO_URL}/commit/{self.book_hash}))"
                self.assertIn(
                    f"**Book:** {book}\n**Page:** {page}\n", str(intro["content"])
                )
                self.assertNotIn("{{git-dates:", str(intro["content"]))
                footer = f"<small>This page last changed: {page}. The whole book last changed: {book}.</small>"
                self.assertTrue(
                    str(intro["content"]).endswith(f"\n\n---\n\n{footer}\n")
                )
                # The chapter committed in the later commit dates the same as the book.
                self.assertTrue(
                    str(ch["content"]).endswith(
                        f"\n\n---\n\n<small>This page last changed: {book}. The whole book last changed: {book}.</small>\n"
                    )
                )
                # A draft chapter has no source file and is left exactly as it came.
                self.assertEqual(draft["content"], "")

    def test_without_a_repository_url_the_hash_is_plain_text(self) -> None:
        result = run(self.book_root, self.payload("0.5", repo_url=None))
        self.assertEqual(result.returncode, 0, result.stderr)
        intro = chapters_of(result.stdout)[0]
        self.assertIn(
            f"**Page:** {FIRST_DATE} (commit {self.intro_hash})\n",
            str(intro["content"]),
        )
        self.assertNotIn("](", str(intro["content"]))

    def test_a_never_committed_page_says_so_instead_of_inventing_a_date(self) -> None:
        (self.src / "new.md").write_text("# New\n", encoding="utf-8")
        items = [chapter("New", "new.md", "# New\n")]
        result = run(self.book_root, wire("0.5", self.book_root, items))
        self.assertEqual(result.returncode, 0, result.stderr)
        content = str(chapters_of(result.stdout)[0]["content"])
        self.assertIn("This page last changed: not yet committed.", content)
        self.assertIn(f"The whole book last changed: {SECOND_DATE} (commit", content)

    def test_an_unknown_placeholder_fails_the_build_and_names_it(self) -> None:
        items = [chapter("Bad", "ch.md", "# Ch\n\n{{git-dates:bogus}}\n")]
        result = run(self.book_root, wire("0.5", self.book_root, items))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("{{git-dates:bogus}}", result.stderr)
        self.assertIn("ch.md", result.stderr)

    def test_a_shallow_clone_is_refused_and_the_fix_is_named(self) -> None:
        shallow = self.root / "shallow"
        git(
            ["clone", "-q", "--depth", "1", f"file://{self.root}", str(shallow)],
            self.root,
        )
        shallow_book = shallow / "book"
        items = [chapter("Intro", "introduction.md", "# Intro\n")]
        result = run(shallow_book, wire("0.5", shallow_book, items))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("fetch-depth: 0", result.stderr)

    def test_supports_answers_yes_for_any_renderer(self) -> None:
        for renderer in ("html", "markdown", "anything"):
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "supports", renderer],
                cwd=self.book_root,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


class Verify(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.book_root = make_repo(Path(self.tmp.name))
        src = self.book_root / "src"
        self.intro_hash = git(
            ["log", "-1", "--format=%h", "--", "introduction.md"], src
        )
        self.book_hash = git(["log", "-1", "--format=%h", "--", "."], src)

    def verify(self, html: str) -> subprocess.CompletedProcess[str]:
        rendered = self.book_root / "index.html"
        rendered.write_text(html, encoding="utf-8")
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "verify",
                "--book-root",
                str(self.book_root),
                "--page",
                "introduction.md",
                str(rendered),
            ],
            capture_output=True,
            text=True,
            check=False,
        )

    def rendered(self, page_date: str, book_date: str) -> str:
        """What mdBook renders the footer to: the markdown link becomes an anchor."""
        return (
            f"<p><small>This page last changed: {page_date} (commit "
            f'<a href="{REPO_URL}/commit/{self.intro_hash}">{self.intro_hash}</a>). '
            f"The whole book last changed: {book_date} (commit "
            f'<a href="{REPO_URL}/commit/{self.book_hash}">{self.book_hash}</a>).</small></p>\n'
        )

    def test_accepts_html_carrying_both_git_derived_dates(self) -> None:
        result = self.verify(self.rendered(FIRST_DATE, SECOND_DATE))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_html_whose_book_date_is_the_page_date(self) -> None:
        # The original defect: the front page showing its own date as the book's.
        result = self.verify(self.rendered(FIRST_DATE, FIRST_DATE))
        self.assertEqual(result.returncode, 1)
        self.assertIn("book: expected", result.stderr)

    def test_rejects_html_where_the_preprocessor_did_not_run(self) -> None:
        result = self.verify("<p>**Last modified:** 2026-01-10</p>\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("page: expected", result.stderr)
        self.assertIn("book: expected", result.stderr)


class BookTomlWiring(unittest.TestCase):
    """Pins the root cause of the 2026-09-01 book.yml CI failure.

    mdBook 0.5.x always runs a preprocessor subprocess with the book
    directory as its cwd. mdBook 0.4.x (what `book.yml` pins, for
    mdbook-mermaid compatibility) sets NO cwd of its own for that subprocess
    at all: it just inherits whatever directory the `mdbook` PROCESS ITSELF
    was started from (verified directly against the `cmd.rs` source of both
    releases, and against real 0.4.52 and 0.5.4 binaries). Since book.toml's
    git-dates command is the relative path `../scripts/mdbook_git_dates.py`,
    it resolves under 0.5.x no matter where `mdbook` was invoked from, but
    under 0.4.x it resolves only when `mdbook` itself was started with the
    book directory as its cwd. `Makefile` and `book.yml` are the wiring that
    now guarantees that (`cd book && mdbook build` / `working-directory:
    book`, never `mdbook build book` from the repo root); this drives the
    ACTUAL `book.toml` command string, the way mdBook itself would run it,
    against both cwd shapes and proves the previously-broken one stays
    broken so nobody "fixes" the wiring back to it.
    """

    def command_words(self) -> list[str]:
        config = tomllib.loads(BOOK_TOML.read_text(encoding="utf-8"))
        command = config["preprocessor"]["git-dates"]["command"]
        return shlex.split(command)

    def payload(self) -> str:
        intro = (BOOK_ROOT / "src" / "introduction.md").read_text(encoding="utf-8")
        items = [chapter("Intro", "introduction.md", intro)]
        return wire("0.4", BOOK_ROOT, items)

    def test_resolves_when_mdbook_itself_is_started_from_the_book_directory(
        self,
    ) -> None:
        result = subprocess.run(
            self.command_words(),
            cwd=BOOK_ROOT,
            input=self.payload(),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        content = str(chapters_of(result.stdout)[0]["content"])
        self.assertIn("This page last changed: ", content)
        self.assertNotIn("{{git-dates:", content)

    def test_does_not_resolve_when_mdbook_itself_is_started_from_the_repo_root(
        self,
    ) -> None:
        # The exact 2026-09-01 CI shape: `mdbook build book` run from the
        # repo root, under mdBook 0.4.x, which never overrides the cwd it
        # inherited. If this test ever starts PASSING, the fix has been
        # silently undone and the `cd book &&` / `working-directory: book`
        # wiring in Makefile / book.yml needs restoring.
        result = subprocess.run(
            self.command_words(),
            cwd=REPO_ROOT,
            input=self.payload(),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No such file or directory", result.stderr)


if __name__ == "__main__":
    unittest.main()
