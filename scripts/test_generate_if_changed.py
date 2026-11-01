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

    def stage(self, grammar: Path, names: list[str], returncode: int, mode):
        """Run the staging seam with a producer that writes `names`."""

        def generate(arguments, **kwargs):
            staged = Path(arguments[-1])
            for name in names:
                (staged / name).write_text("changed")
            return subprocess.CompletedProcess(arguments, returncode)

        diagnostics = io.StringIO()
        with patch.object(generate_grammar, "GRAMMAR", grammar), patch.object(
            generate_grammar.subprocess, "run", side_effect=generate
        ), redirect_stderr(diagnostics):
            status = generate_grammar.stage_and_publish(Path("tree-sitter"), mode)
        return status, diagnostics.getvalue()

    def test_grammar_check_and_failed_generation_do_not_publish(self):
        with tempfile.TemporaryDirectory() as directory:
            grammar = Path(directory)
            target = grammar / "src" / "parser.c"
            target.parent.mkdir()
            target.write_text("original")
            status, diagnostics = self.stage(
                grammar, list(generate_grammar.REQUIRED), 0, generate_grammar.Mode.CHECK
            )
            self.assertEqual(status, 1)
            self.assertIn("stale grammar/src/parser.c", diagnostics)
            self.assertEqual(target.read_text(), "original")
            self.assertEqual(list(target.parent.iterdir()), [target])
            status, _ = self.stage(
                grammar, list(generate_grammar.REQUIRED), 7, generate_grammar.Mode.PUBLISH
            )
            self.assertEqual(status, 7)
            self.assertEqual(target.read_text(), "original")
            self.assertEqual(list(grammar.iterdir()), [target.parent])

    def test_a_producer_without_the_contract_fails_closed(self):
        # An upstream CLI writes every file but the contract; publishing its
        # output would leave grammar-contract.json silently stale.
        with tempfile.TemporaryDirectory() as directory:
            grammar = Path(directory)
            (grammar / "src").mkdir()
            upstream = ["parser.c", "grammar.json", "node-types.json"]
            for mode in generate_grammar.Mode:
                status, diagnostics = self.stage(grammar, upstream, 0, mode)
                self.assertEqual(status, 1)
                self.assertIn("generator omitted grammar-contract.json", diagnostics)
                self.assertEqual(list((grammar / "src").iterdir()), [])

    def test_an_unset_producer_selection_fails_closed(self):
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop(generate_grammar.TREE_SITTER.variable, None)
            diagnostics = io.StringIO()
            with patch.object(sys, "argv", ["generate_grammar.py", "--check"]), redirect_stderr(
                diagnostics
            ):
                self.assertEqual(generate_grammar.main(), 1)
            self.assertIn("TREE_SITTER_DIR must name a clean checkout", diagnostics.getvalue())


if __name__ == "__main__":
    unittest.main()
