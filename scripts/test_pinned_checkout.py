"""Checkout admission against real repositories; no Cargo substitution."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pinned_checkout import Pin, PinError, PinnedCheckout, PinnedRevision

PIN = Pin(file="generator-revision", variable="GENERATOR_DIR")


class CheckoutAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.generator = root / "generator with spaces"
        self.generator.mkdir()
        self.consumer = root / "consumer"
        self.consumer.mkdir()
        self.git("init", "--quiet")
        (self.generator / "Cargo.lock").write_text("version = 4\n")
        self.git("add", "Cargo.lock")
        self.commit("fixture")
        self.pin_head()

    def git(self, *args):
        return subprocess.run(
            ["git", "-C", str(self.generator), *args],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def commit(self, message):
        self.git(
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            message,
        )

    def pin_head(self):
        (self.consumer / PIN.file).write_text(self.git("rev-parse", "HEAD") + "\n")

    def admit(self, directory=None):
        return PinnedCheckout(directory or self.generator, PIN.revision(self.consumer))

    def test_clean_pinned_root_and_revalidation(self):
        checkout = self.admit()
        checkout.revalidate()
        (self.generator / "Cargo.lock").write_text("changed\n")
        with self.assertRaisesRegex(PinError, "uncommitted"):
            checkout.revalidate()

    def test_wrong_full_revision_is_refused(self):
        (self.consumer / PIN.file).write_text("0" * 40 + "\n")
        with self.assertRaisesRegex(PinError, "differs"):
            self.admit()

    def test_malformed_pin_is_refused_before_git(self):
        head = self.git("rev-parse", "HEAD")
        for value in [head[:12], "stable", "", head.upper()]:
            with self.subTest(pin=value):
                (self.consumer / PIN.file).write_text(value + "\n")
                with self.assertRaisesRegex(PinError, "full lowercase"):
                    PIN.revision(self.consumer)

    def test_untracked_dirt_is_refused(self):
        (self.generator / "untracked").write_text("dirt\n")
        with self.assertRaisesRegex(PinError, "uncommitted"):
            self.admit()

    def test_nested_directory_is_not_the_repository_root(self):
        nested = self.generator / "nested"
        nested.mkdir()
        with self.assertRaisesRegex(PinError, "repository root"):
            self.admit(nested)

    def test_missing_checkout_reports_git_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.admit(self.generator / "missing")

    def test_clean_source_without_lockfile_is_refused(self):
        self.git("rm", "--quiet", "Cargo.lock")
        self.commit("remove lock fixture")
        self.pin_head()
        with self.assertRaisesRegex(PinError, "Cargo.lock"):
            self.admit()

    def test_the_selection_variable_has_no_default(self):
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop(PIN.variable, None)
            with self.assertRaisesRegex(PinError, "GENERATOR_DIR must name"):
                PIN.checkout(self.consumer)
        with patch.dict(os.environ, {PIN.variable: str(self.generator)}):
            self.assertEqual(PIN.checkout(self.consumer).directory, self.generator.resolve())

    def test_a_revision_is_made_only_by_its_pin(self):
        with self.assertRaises(TypeError):
            PinnedRevision(PIN, "0" * 40, object())


if __name__ == "__main__":
    unittest.main()
