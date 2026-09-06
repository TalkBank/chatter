"""Exercise generator publication through its actual command-line seam."""

import os
import io
from contextlib import redirect_stderr
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import generate_grammar

SCRIPT = Path(__file__).with_name("generate_if_changed.py")


class GeneratorOutputTests(unittest.TestCase):
    def run_generator(self, output: Path, code: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(output), sys.executable, "-c", code],
            capture_output=True, check=False,
        )

    def test_creation_changes_and_identical_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "generated.rs"
            self.assertEqual(self.run_generator(output, "print('first')").returncode, 0)
            self.assertEqual(output.read_text(), "first\n")
            output.chmod(0o640)
            os.utime(output, ns=(1_000_000_000, 1_000_000_000))
            before = output.stat()
            self.assertEqual(self.run_generator(output, "print('first')").returncode, 0)
            self.assertEqual(output.stat().st_mtime_ns, before.st_mtime_ns)
            self.assertEqual(self.run_generator(output, "print('second')").returncode, 0)
            self.assertEqual(output.read_text(), "second\n")
            self.assertEqual(output.stat().st_mode, before.st_mode)
            self.assertEqual(list(output.parent.iterdir()), [output])

    def test_failed_generator_preserves_existing_or_missing_target(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "generated.rs"
            for exists in [False, True]:
                if exists:
                    output.write_text("original")
                result = self.run_generator(output, "print('partial'); raise SystemExit(7)")
                self.assertEqual(result.returncode, 7)
                self.assertEqual(output.exists(), exists)
                if exists:
                    self.assertEqual(output.read_text(), "original")
                self.assertEqual(list(output.parent.iterdir()), [output] if exists else [])


    def test_grammar_check_and_failed_generation_do_not_publish(self):
        with tempfile.TemporaryDirectory() as directory:
            grammar = Path(directory)
            target = grammar / "src" / "parser.c"
            target.parent.mkdir()
            target.write_text("original")

            def generate(arguments, **kwargs):
                staged = Path(arguments[-1])
                for name in ["parser.c", "grammar.json", "node-types.json"]:
                    (staged / name).write_text("changed")
                return subprocess.CompletedProcess(arguments, returncode)

            with patch.object(generate_grammar, "GRAMMAR", grammar), patch.object(
                generate_grammar.subprocess, "run", side_effect=generate
            ):
                returncode = 0
                diagnostics = io.StringIO()
                with patch.object(sys, "argv", ["generate_grammar.py", "--check"]), redirect_stderr(diagnostics):
                    self.assertEqual(generate_grammar.main(), 1)
                self.assertIn("stale grammar/src/parser.c", diagnostics.getvalue())
                self.assertEqual(target.read_text(), "original")
                self.assertEqual(list(target.parent.iterdir()), [target])
                returncode = 7
                with patch.object(sys, "argv", ["generate_grammar.py"]):
                    self.assertEqual(generate_grammar.main(), 7)
                self.assertEqual(target.read_text(), "original")
                self.assertEqual(list(grammar.iterdir()), [target.parent])


if __name__ == "__main__":
    unittest.main()
