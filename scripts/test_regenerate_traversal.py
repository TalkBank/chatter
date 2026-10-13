"""Source admission via real repositories; no Cargo or generator substitution."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from regenerate_traversal import ConsumerInputs, PinnedCheckout, RegenerationError


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
        self.git(
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        )
        self.revision = self.git("rev-parse", "HEAD")
        (self.consumer / "tsgu-revision").write_text(self.revision + "\n")
        (self.consumer / "Cargo.toml").write_text(
            '[workspace.package]\nedition = "2024"\n'
        )
        (self.consumer / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "stable"\n'
        )

    def git(self, *args):
        return subprocess.run(
            ["git", "-C", str(self.generator), *args],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def inputs(self):
        return ConsumerInputs(self.consumer)

    def test_clean_pinned_root_and_revalidation(self):
        checkout = PinnedCheckout(self.generator, self.inputs())
        checkout.revalidate()
        (self.generator / "Cargo.lock").write_text("changed\n")
        with self.assertRaisesRegex(RegenerationError, "uncommitted"):
            checkout.revalidate()

    def test_wrong_full_revision_is_not_clean_source_admission(self):
        (self.consumer / "tsgu-revision").write_text("0" * 40 + "\n")
        with self.assertRaisesRegex(RegenerationError, "differs"):
            PinnedCheckout(self.generator, self.inputs())

    def test_malformed_pin_is_refused_before_git(self):
        for value in [self.revision[:12], "stable", "", self.revision.upper()]:
            with self.subTest(pin=value):
                (self.consumer / "tsgu-revision").write_text(value + "\n")
                with self.assertRaisesRegex(RegenerationError, "full lowercase"):
                    self.inputs()

    def test_untracked_dirt_is_refused(self):
        (self.generator / "untracked").write_text("dirt\n")
        with self.assertRaisesRegex(RegenerationError, "uncommitted"):
            PinnedCheckout(self.generator, self.inputs())

    def test_nested_directory_is_not_the_repository_root(self):
        nested = self.generator / "nested"
        nested.mkdir()
        with self.assertRaisesRegex(RegenerationError, "repository root"):
            PinnedCheckout(nested, self.inputs())

    def test_missing_checkout_reports_git_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            PinnedCheckout(self.generator / "missing", self.inputs())

    def test_explicit_missing_selection_has_no_default_fallback(self):
        result = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).with_name("regenerate_traversal.py")),
                "--generator",
                str(self.generator / "missing"),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("traversal regeneration failed", result.stderr)
        self.assertNotIn("generator source", result.stdout)

    def test_clean_source_without_lockfile_is_refused(self):
        self.git("rm", "Cargo.lock")
        self.git(
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "remove lock fixture",
        )
        (self.consumer / "tsgu-revision").write_text(
            self.git("rev-parse", "HEAD") + "\n"
        )
        with self.assertRaisesRegex(RegenerationError, "Cargo.lock"):
            PinnedCheckout(self.generator, self.inputs())

    def test_invalid_consumer_formatting_configuration_is_refused(self):
        (self.consumer / "Cargo.toml").write_text(
            '[workspace.package]\nedition = "future"\n'
        )
        with self.assertRaisesRegex(RegenerationError, "edition"):
            self.inputs()
        (self.consumer / "Cargo.toml").write_text(
            '[workspace.package]\nedition = "2024"\n'
        )
        (self.consumer / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = ""\n'
        )
        with self.assertRaisesRegex(RegenerationError, "channel"):
            self.inputs()


if __name__ == "__main__":
    unittest.main()
