"""Traversal regeneration's own admission; checkout admission is tested with
its module in `test_pinned_checkout.py`."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from regenerate_traversal import TSGU, ConsumerInputs, RegenerationError


class ConsumerInputsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.consumer = Path(self.temporary.name)
        (self.consumer / "Cargo.toml").write_text('[workspace.package]\nedition = "2024"\n')
        (self.consumer / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "stable"\n')

    def test_invalid_consumer_formatting_configuration_is_refused(self):
        (self.consumer / "Cargo.toml").write_text('[workspace.package]\nedition = "future"\n')
        with self.assertRaisesRegex(RegenerationError, "edition"):
            ConsumerInputs(self.consumer)
        (self.consumer / "Cargo.toml").write_text('[workspace.package]\nedition = "2024"\n')
        (self.consumer / "rust-toolchain.toml").write_text('[toolchain]\nchannel = ""\n')
        with self.assertRaisesRegex(RegenerationError, "channel"):
            ConsumerInputs(self.consumer)


class SelectionTests(unittest.TestCase):
    def run_script(self, environment):
        return subprocess.run(
            [sys.executable, str(Path(__file__).with_name("regenerate_traversal.py"))],
            capture_output=True,
            text=True,
            check=False,
            env=environment,
        )

    def test_an_unset_generator_selection_has_no_default(self):
        environment = {key: value for key, value in os.environ.items() if key != TSGU.variable}
        result = self.run_script(environment)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("TSGU_DIR must name", result.stderr)
        self.assertNotIn("generator source", result.stdout)

    def test_a_missing_selected_checkout_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_script({**os.environ, TSGU.variable: str(Path(directory) / "missing")})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("traversal regeneration failed", result.stderr)
        self.assertNotIn("generator source", result.stdout)


if __name__ == "__main__":
    unittest.main()
