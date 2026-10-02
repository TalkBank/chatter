#!/usr/bin/env python3
"""Exercise date admission before committing or squashing real Git changes."""

from __future__ import annotations

import datetime
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.test_mdbook_git_dates import git


class ProspectiveDates(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        scripts = self.root / "scripts"
        scripts.mkdir()
        self.script = scripts / "check_doc_dates.py"
        shutil.copyfile(Path(__file__).with_name("check_doc_dates.py"), self.script)
        self.page = self.root / "a page.md"
        self.page.write_text("# Page\n\n**Last modified:** 2020-01-01\n")
        git(["init", "-q", "-b", "main"], self.root)
        git(["add", "."], self.root)
        git(["commit", "-qm", "published"], self.root, date="2020-01-01")
        git(["branch", "published"], self.root)
        git(["config", "branch.main.remote", "."], self.root)
        git(["config", "branch.main.merge", "refs/heads/published"], self.root)

    def check(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(self.script), *args],
            cwd=self.root, capture_output=True, text=True, check=False,
        )

    def test_pending_files_are_checked_before_commit_and_deleted_files_are_skipped(self) -> None:
        self.assertEqual(self.check("--prospective").returncode, 0)
        self.page.write_text(self.page.read_text() + "\nEdited.\n")
        new = self.root / "new page.md"
        new.write_text("# New\n\n**Last modified:** 2020-01-01\n")
        result = self.check("--prospective")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("STALE: a page.md", result.stderr)
        self.assertIn("STALE: new page.md", result.stderr)
        today = datetime.datetime.now().astimezone().date().isoformat()
        self.page.write_text(self.page.read_text().replace("2020-01-01", today))
        new.write_text(new.read_text().replace("2020-01-01", today))
        self.assertEqual(self.check("--prospective").returncode, 0)
        self.page.unlink()
        self.assertEqual(self.check("--prospective").returncode, 0)

    def test_clean_unpublished_commit_is_checked_for_its_future_squash(self) -> None:
        self.page.write_text(self.page.read_text() + "\nUnpublished.\n")
        git(["add", "."], self.root)
        git(["commit", "-qm", "unpublished"], self.root, date="2020-01-01")
        self.assertEqual(self.check().returncode, 0)
        result = self.check("--prospective")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("STALE: a page.md", result.stderr)

    def test_index_commit_does_not_redate_untouched_unpublished_documents(self) -> None:
        self.page.write_text(self.page.read_text() + "\nUnpublished.\n")
        git(["add", "."], self.root)
        git(["commit", "-qm", "unpublished"], self.root, date="2020-01-01")
        hook = self.root / ".git" / "hooks" / "pre-commit"
        shutil.copyfile(Path(__file__).resolve().parents[1] / ".githooks/pre-commit", hook)
        hook.chmod(0o755)
        (self.root / "code.txt").write_text("unrelated change\n")
        git(["add", "code.txt"], self.root)
        self.assertEqual(self.check("--prospective", "--snapshot", "index").returncode, 0)
        git(["commit", "-qm", "ordinary checkpoint"], self.root)
        self.assertNotEqual(self.check("--prospective").returncode, 0)

    def test_detached_ci_checks_committed_history_without_redating_it(self) -> None:
        git(["checkout", "--detach", "-q"], self.root)
        self.assertEqual(self.check("--prospective").returncode, 0)

    def test_real_commit_hook_rechecks_dates_after_an_earlier_gate(self) -> None:
        hook = self.root / ".git" / "hooks" / "pre-commit"
        shutil.copyfile(Path(__file__).resolve().parents[1] / ".githooks/pre-commit", hook)
        hook.chmod(0o755)
        self.page.write_text(self.page.read_text() + "\nChanged after the earlier check.\n")
        git(["add", "."], self.root)
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            git(["commit", "-qm", "stale date"], self.root)
        self.assertIn("prospective commit", caught.exception.stderr)

    def test_unstaged_repair_cannot_mask_stale_staged_content(self) -> None:
        hook = self.root / ".git" / "hooks" / "pre-commit"
        shutil.copyfile(Path(__file__).resolve().parents[1] / ".githooks/pre-commit", hook)
        hook.chmod(0o755)
        self.page.write_text(self.page.read_text() + "\nStaged change.\n")
        git(["add", "."], self.root)
        today = datetime.datetime.now().astimezone().date().isoformat()
        self.page.write_text(self.page.read_text().replace("2020-01-01", today))
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            git(["commit", "-qm", "old staged header"], self.root)
        self.assertIn("prospective commit", caught.exception.stderr)
        git(["add", "."], self.root)
        git(["commit", "-qm", "admitted staged header"], self.root)
        self.assertEqual(self.check().returncode, 0)

    def test_baseline_does_not_excuse_a_new_edit(self) -> None:
        self.page.write_text("# Page\n\n**Last modified:** 2019-01-01\n")
        (self.root / "scripts/doc-dates-baseline.txt").write_text("a page.md\n")
        git(["add", "."], self.root)
        git(["commit", "-qm", "old backlog"], self.root, date="2020-01-01")
        git(["branch", "-f", "published"], self.root)
        self.assertEqual(self.check("--prospective").returncode, 0)
        self.page.write_text(self.page.read_text() + "\nNew edit.\n")
        result = self.check("--prospective")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("STALE: a page.md", result.stderr)

    def test_git_history_metadata_survives_a_real_squash_without_restamping(self) -> None:
        self.page.write_text(
            "# Page\n\n**Last modified:** [Git history]"
            "(https://github.com/TalkBank/chatter/commits/main/a%20page.md)\n"
        )
        git(["add", "."], self.root)
        git(["commit", "-qm", "metadata owner"], self.root, date="2020-01-01")
        hook = self.root / ".git" / "hooks" / "pre-commit"
        shutil.copyfile(Path(__file__).resolve().parents[1] / ".githooks/pre-commit", hook)
        hook.chmod(0o755)
        self.assertEqual(self.check("--prospective").returncode, 0)
        git(["reset", "--soft", "published"], self.root)
        git(["commit", "-qm", "content-preserving squash"], self.root)
        self.assertEqual(self.check().returncode, 0)

    def test_a_placeholder_example_cannot_excuse_an_edited_manual_header(self) -> None:
        self.page.write_text(self.page.read_text() + "\nExample: {{git-dates:page}}\n")
        result = self.check("--prospective")
        self.assertNotEqual(result.returncode, 0)

    def test_a_history_link_must_name_the_document_it_dates(self) -> None:
        self.page.write_text(
            "# Page\n\n**Last modified:** [Git history]"
            "(https://github.com/TalkBank/chatter/commits/main/other.md)\n"
        )
        self.assertNotEqual(self.check("--prospective").returncode, 0)

    def test_a_book_date_placeholder_is_not_a_nonbook_date(self) -> None:
        self.page.write_text("# Page\n\n**Last modified:** {{git-dates:page}}\n")
        self.assertNotEqual(self.check("--prospective").returncode, 0)

    def test_a_rendered_chapter_uses_its_page_metadata(self) -> None:
        book = self.root / "book/src"
        book.mkdir(parents=True)
        chapter = book / "introduction.md"
        chapter.write_text(
            "# Intro\n\n**Book last changed:** {{git-dates:book}}\n"
            "**This page last changed:** {{git-dates:page}}\n"
        )
        self.assertEqual(self.check("--prospective").returncode, 0)
        summary = book / "SUMMARY.md"
        summary.write_text("# Summary\n\n**Last modified:** {{git-dates:page}}\n")
        self.assertNotEqual(self.check("--prospective").returncode, 0)
        summary.write_text(
            '# Summary\n\n**Last modified:** <a href="https://github.com/TalkBank/'
            'chatter/commits/main/book/src/SUMMARY.md">Git history</a>\n'
        )
        self.assertEqual(self.check("--prospective").returncode, 0)


if __name__ == "__main__":
    unittest.main()
