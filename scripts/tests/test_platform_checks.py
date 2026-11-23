"""検証器が空の検証、未証明、公理追加、対象欠落を成功扱いしないことを確認します。"""

from pathlib import Path
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import platform_support
import package as distribution
from platform_support import Run, load_versions, validate_documentation, write_json
from package import archive_files
from verify_platform import (validate_generator_semantics, validate_kani, validate_lean, validate_negative_controls,
                             validate_node_tests, validate_python_tests, validate_rust_tests, validate_simulation)


class ReleasePrivacyTests(unittest.TestCase):
    def test_release_environment_preserves_plain_flags_without_mutating_caller(self):
        original = platform_support.tool_environment()
        original.pop("CARGO_ENCODED_RUSTFLAGS", None)
        original["RUSTFLAGS"] = "-C opt-level=1\t--cfg user_build"
        before = original.copy()
        process_before = os.environ.copy()
        release = platform_support.release_environment(original)
        flags = release["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
        self.assertEqual(flags[:4], ["-C", "opt-level=1", "--cfg", "user_build"])
        self.assertTrue(any(flag.startswith("--remap-path-prefix=") for flag in flags))
        self.assertTrue(original == before)
        self.assertTrue(dict(os.environ) == process_before)
        self.assertEqual(release["RUSTFLAGS"], before["RUSTFLAGS"])

    def test_encoded_flags_keep_precedence_including_empty_and_embedded_spaces(self):
        for encoded in ("", '--cfg\x1fuser_value="two words"\x1f--remap-path-prefix=/old=/public'):
            with self.subTest(empty=not encoded):
                original = platform_support.tool_environment() | {
                    "RUSTFLAGS": "--cfg should_remain_ignored", "CARGO_ENCODED_RUSTFLAGS": encoded}
                release = platform_support.release_environment(original)
                flags = release["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
                prefix = encoded.split("\x1f") if encoded else []
                self.assertEqual(flags[:len(prefix)], prefix)
                self.assertNotIn("", flags)
                self.assertNotIn("should_remain_ignored", flags)
                self.assertEqual(release["RUSTFLAGS"], original["RUSTFLAGS"])

    def test_more_specific_remaps_follow_ancestors_and_preserve_spaces(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory) / "Build User"
            root = home / "workspace" / "project"
            cargo = home / "cargo registry"
            rustup = home / "rustup"
            with patch.object(platform_support, "ROOT", root), patch.object(Path, "home", return_value=home):
                env = {"PATH": os.defpath, "HOME": str(home), "CARGO_HOME": str(cargo), "RUSTUP_HOME": str(rustup)}
                flags = platform_support.release_environment(env)["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
                mappings = [flag.removeprefix("--remap-path-prefix=").rsplit("=", 1) for flag in flags]
                originals = [source for source, _ in mappings]
                self.assertIn(str(cargo), originals)
                self.assertIn(str(rustup), originals)
                self.assertLess(originals.index(str(home)), originals.index(str(cargo)))
                self.assertLess(originals.index(str(home)), originals.index(str(root)))
                self.assertEqual(mappings[originals.index(str(cargo))][1], "/suzume/cargo")

    def test_actual_home_and_userprofile_are_included_even_with_alternate_home(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            actual, profile, alternate = root / "actual", root / "profile", root / "alternate"
            with patch.object(Path, "home", return_value=actual):
                prefixes = platform_support.private_path_prefixes({
                    "HOME": str(alternate), "USERPROFILE": str(profile)})
            for path in (actual, profile, alternate):
                self.assertIn(str(path), prefixes)

    def test_run_uses_an_explicit_child_environment_without_process_changes(self):
        env = platform_support.tool_environment()
        env["SUZUME_TEST_CHILD_ONLY"] = "isolated"
        before = os.environ.get("SUZUME_TEST_CHILD_ONLY")
        run = Run("verification-selftest")
        result = run.execute("child-environment", [sys.executable, "-c",
                             "import os; print(os.environ.get('SUZUME_TEST_CHILD_ONLY', 'absent'))"], env=env,
                             validator=lambda output: {"success": output.strip() == "isolated", "detail": "子環境を確認"})
        self.assertTrue(result["success"])
        self.assertEqual(os.environ.get("SUZUME_TEST_CHILD_ONLY"), before)

    def test_private_prefix_detection_covers_windows_separators_and_utf16(self):
        prefix = r"C:\Build User\cargo"
        for spelling in (prefix, prefix.replace("\\", "/")):
            for encoding in ("utf-8", "utf-16-le", "utf-16-be"):
                with self.subTest(encoding=encoding, forward_slashes="/" in spelling):
                    data = (spelling + "/registry/source.rs").encode(encoding)
                    self.assertGreater(platform_support.private_path_matches(data, (prefix,)), 0)
        self.assertEqual(platform_support.private_path_matches(b"/suzume/cargo/registry/source.rs", (prefix,)), 0)

    def test_archive_scan_requires_every_binary_and_rejects_empty_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first, second = root / "first", root / "second"
            first.write_bytes(b"public binary one")
            second.write_bytes(b"public binary two")
            archive = root / "native.tar.gz"
            archive_files(archive, "release", [(first, "suzume"), (second, "suzume-api")])
            required = {"release/suzume", "release/suzume-api"}
            report = distribution.scan_distribution_paths([(archive, required)])
            self.assertTrue(report["success"])
            self.assertEqual(report["checked"], 2)
            self.assertTrue(all(len(entry["sha256"]) == 64 for entry in report["entries"]))
            self.assertFalse(distribution.scan_distribution_paths([])["success"])
            self.assertFalse(distribution.scan_distribution_paths([(archive, set())])["success"])
            self.assertFalse(distribution.scan_distribution_paths([(archive, required | {"release/missing"})])["success"])

    def test_archive_scan_rejects_embedded_prefix_without_disclosing_its_value(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo = root / "private-cargo"
            binary = root / "binary"
            binary.write_bytes((str(cargo) + "/registry/source.rs").encode())
            archive = root / "sdk.tgz"
            archive_files(archive, "package", [(binary, "wasm/node/suzume_wasm_bg.wasm"), (binary, "wasm/web/suzume_wasm_bg.wasm")])
            report = distribution.scan_distribution_paths([(archive, {
                "package/wasm/node/suzume_wasm_bg.wasm", "package/wasm/web/suzume_wasm_bg.wasm"})],
                env=platform_support.tool_environment() | {"CARGO_HOME": str(cargo)})
            self.assertFalse(report["success"])
            self.assertEqual(report["checked"], 2)
            self.assertTrue(all(entry["private_prefix_matches"] > 0 for entry in report["entries"]))
            self.assertNotIn(str(cargo), json.dumps(report))


class VerificationAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.policy = load_versions()["verification"]
        self.lean = "\n".join(f"'{name}' depends on axioms: [propext]" for name in self.policy["lean_theorems"])
        self.kani = "\n".join(f"Checking harness {name}...\nVERIFICATION:- SUCCESSFUL" for name in self.policy["kani_harnesses"])
        self.kani += "\nManual Harness Summary:\nComplete - 6 successfully verified harnesses, 0 failures, 6 total."

    def test_current_lean_and_kani_output_is_accepted(self):
        self.assertTrue(validate_lean(self.lean)["success"])
        self.assertTrue(validate_kani(self.kani)["success"])

    def test_lean_rejects_sorry_unknown_axiom_and_missing_theorem(self):
        for rejected in (self.lean.replace("propext", "sorryAx", 1),
                         self.lean.replace("propext", "Application.trusted_claim", 1),
                         "\n".join(self.lean.splitlines()[1:]), self.lean + "\n" + self.lean.splitlines()[0], ""):
            with self.subTest(output=rejected[:100]):
                self.assertFalse(validate_lean(rejected)["success"])

    def test_kani_rejects_failed_or_missing_harness_and_empty_output(self):
        for rejected in (self.kani.replace("SUCCESSFUL", "FAILED", 1),
                         self.kani.replace(self.policy["kani_harnesses"][0], "other_harness", 1),
                         self.kani.replace("6 total", "0 total"), ""):
            with self.subTest(output=rejected[:100]):
                self.assertFalse(validate_kani(rejected)["success"])

    def test_rust_rejects_zero_tests_ignored_and_filtered(self):
        accepted = "test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"
        self.assertTrue(validate_rust_tests(accepted)["success"])
        for rejected in (accepted.replace("76 passed", "0 passed"), accepted.replace("0 ignored", "1 ignored"),
                         accepted.replace("0 filtered", "1 filtered"), ""):
            self.assertFalse(validate_rust_tests(rejected)["success"])

    def test_simulation_rejects_missing_duplicate_and_failed_seeds(self):
        output = "\n".join(f"{seed + 1}/32  seed={seed}  成功  100操作" for seed in range(32))
        output += "\n結果: 成功32 / 総数32、失敗0"
        self.assertTrue(validate_simulation(output)["success"])
        for rejected in (output.replace("seed=31", "seed=30"), output.replace("成功32 /", "成功31 /"), ""):
            self.assertFalse(validate_simulation(rejected)["success"])

    def test_sdk_rejects_zero_skipped_and_cancelled_tests(self):
        accepted = "# tests 4\n# pass 4\n# fail 0\n# cancelled 0\n# skipped 0\n# todo 0\n"
        self.assertTrue(validate_node_tests(accepted)["success"])
        for rejected in (accepted.replace("4", "0"), accepted.replace("skipped 0", "skipped 1"),
                         accepted.replace("cancelled 0", "cancelled 1"), accepted.replace("todo 0", "todo 1"), ""):
            self.assertFalse(validate_node_tests(rejected)["success"])

    def test_process_timeout_is_failure_and_cannot_overwrite_evidence(self):
        run = Run("verification-selftest")
        result = run.execute("timeout", [sys.executable, "-c", "import time; time.sleep(30)"], timeout=1)
        self.assertFalse(result["success"])
        self.assertEqual(result["exit_code"], 124)
        self.assertTrue(result["timed_out"])
        with self.assertRaises(ValueError):
            run.execute("timeout", [sys.executable, "--version"])
        with self.assertRaises(FileExistsError):
            write_json(run.directory / "timeout.json", {"success": True})

    def test_missing_executable_is_not_success(self):
        run = Run("verification-selftest")
        result = run.execute("missing", ["suzume-deliberately-absent-executable"])
        self.assertFalse(result["success"])
        self.assertEqual(result["exit_code"], 127)

    def test_python_tests_cannot_be_empty_or_skipped(self):
        self.assertTrue(validate_python_tests("Ran 19 tests in 0.1s\n\nOK\n", 19)["success"])
        for output in ("Ran 0 tests in 0.1s\nOK\n", "Ran 19 tests in 0.1s\nOK (skipped=1)\n", ""):
            self.assertFalse(validate_python_tests(output, 19)["success"])

    def test_documentation_requires_nonempty_checks_and_no_errors(self):
        accepted = {"success": True, "errors": [], "documents": 42, "local_links": 269, "repository_commands": 78}
        self.assertTrue(validate_documentation(json.dumps(accepted))["success"])
        for changes in ({"success": False}, {"errors": ["missing.md"]}, {"documents": 0},
                        {"local_links": 0}, {"repository_commands": 0}, {"documents": True}):
            with self.subTest(changes=changes):
                self.assertFalse(validate_documentation(json.dumps(accepted | changes))["success"])
        for rejected in ({}, []):
            self.assertFalse(validate_documentation(json.dumps(rejected))["success"])

    def test_negative_controls_and_generator_need_all_cases(self):
        controls = "\n".join(f"N-{index:02} case: 想定どおり拒否" for index in range(1, 6)) + "\n否定検査: 5 / 5 成功"
        self.assertTrue(validate_negative_controls(controls)["success"])
        self.assertFalse(validate_negative_controls(controls.replace("N-05", "N-04"))["success"])
        self.assertFalse(validate_negative_controls("否定検査: 5 / 5 成功")["success"])
        self.assertTrue(validate_generator_semantics("生成式の意味照合: 7関係・19具体例が実Rust/Lean双方で期待値と一致")["success"])
        self.assertFalse(validate_generator_semantics("生成式の意味照合: 7関係・0具体例が実Rust/Lean双方で期待値と一致")["success"])

    def test_bindings_evaluations_and_distribution_docs_have_distinct_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = {"Cargo.toml": "package", "bindings/protocol.schema.json": "schema", "evaluations/protocol.json": "evaluation",
                     "docs/api/reference.md": "api", "docs/verification/run-summary.json": "evidence", "docs/workdocs/workdoc.md": "history",
                     "skills/README.md": "skills", "skills/example/SKILL.md": "instructions", "skills/example/references/ref.md": "reference",
                     "tools/browser/node_modules/ignored.txt": "dependency"}
            for relative, content in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            with patch.object(platform_support, "ROOT", root):
                before = platform_support.source_hashes()
                self.assertIn("bindings/protocol.schema.json", before)
                self.assertIn("evaluations/protocol.json", before)
                self.assertNotIn("docs/verification/run-summary.json", before)
                self.assertNotIn("skills/example/SKILL.md", before)
                distributed = {path.relative_to(root).as_posix() for path in platform_support.distribution_files()}
                self.assertTrue({"docs/api/reference.md", "docs/verification/run-summary.json", "docs/workdocs/workdoc.md"} <= distributed)
                self.assertTrue({"skills/README.md", "skills/example/SKILL.md", "skills/example/references/ref.md"} <= distributed)
                self.assertFalse(any("node_modules" in path for path in distributed))
                (root / "docs/verification/run-summary.json").write_text("new evidence", encoding="utf-8")
                (root / "skills/example/SKILL.md").write_text("updated instructions", encoding="utf-8")
                self.assertEqual(before, platform_support.source_hashes())
                (root / "bindings/protocol.schema.json").write_text("changed schema", encoding="utf-8")
                self.assertNotEqual(before, platform_support.source_hashes())

    def test_source_archive_is_independent_of_mtime_mode_and_argument_order(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first, second = root / "first", root / "second"
            first.write_bytes("麻雀\n".encode())
            second.write_bytes(b"rules\n")
            a, b = root / "a.tar.gz", root / "b.tar.gz"
            hashes = archive_files(a, "source", [(first, "first"), (second, "second")])
            first.chmod(0o700)
            os.utime(first, (1, 1))
            archive_files(b, "source", [(second, "second"), (first, "first")])
            self.assertEqual(a.read_bytes(), b.read_bytes())
            self.assertEqual(set(hashes), {"first", "second"})


if __name__ == "__main__":
    unittest.main()
