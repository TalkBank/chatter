"""Exercise regeneration's build boundary without modifying checkout inputs."""

from pathlib import Path
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[1]


class RegenerationBootstrapTests(unittest.TestCase):
    def test_regen_runs_producers_once_before_compiled_consumers(self):
        result = subprocess.run(
            ["just", "--dry-run", "regen"], cwd=ROOT,
            text=True, capture_output=True, check=True,
        )
        commands = result.stderr
        ordered = ["validate_symbol_registry.js", "generate_grammar.py",
                   "--bin gen_form_markers", "--bin model_sources_gen",
                   "generate-node-types.js", "regenerate_traversal.py",
                   "generate_chat_file_schema", "gen_conformance_inventory",
                   "--bin spec_gen", "--bin audit_check_parity"]
        positions = [commands.index(command) for command in ordered]
        self.assertEqual(positions, sorted(positions))
        self.assertEqual(commands.count("--bin model_sources_gen"), 1)

    def test_standalone_spec_generation_bootstraps_before_runtime_build(self):
        result = subprocess.run(
            ["just", "--dry-run", "spec-gen"], cwd=ROOT,
            text=True, capture_output=True, check=True,
        )
        self.assertLess(result.stderr.index("--bin model_sources_gen"),
                        result.stderr.index("--bin spec_gen"))

    def test_source_generator_does_not_link_the_production_model(self):
        result = subprocess.run(
            ["cargo", "tree", "--offline", "--manifest-path", "spec/Cargo.toml",
             "-p", "generators", "--edges", "normal", "--prefix", "none"],
            cwd=ROOT, text=True, capture_output=True, check=True,
        )
        packages = {line.split()[0] for line in result.stdout.splitlines() if line}
        self.assertNotIn("talkbank-model", packages)
        self.assertNotIn("talkbank-parser", packages)


if __name__ == "__main__":
    unittest.main()
