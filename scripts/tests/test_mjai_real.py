"""外部牌譜検査が改変・欠落・失敗を成功にしないことを確認します。"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("verify_mjai_real", ROOT / "scripts/verify_mjai_real.py")
assert SPEC is not None and SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class MjaiRealChecks(unittest.TestCase):
    def manifest(self):
        return json.loads((ROOT / "tests/fixtures/mjai/manifest.json").read_text(encoding="utf-8"))

    def test_fixed_corpus_has_two_identifiable_games_and_eighteen_regressions(self):
        manifest = VERIFY.validate_manifest(self.manifest())
        self.assertEqual(len(manifest["cases"]), 20)
        self.assertEqual(sum(case["category"] == "real_game" for case in manifest["cases"]), 2)
        self.assertEqual(sum(case["expected"]["records"] for case in manifest["cases"]), 2310)

    def test_missing_case_cannot_shrink_the_denominator(self):
        manifest = self.manifest()
        manifest["cases"].pop()
        with self.assertRaisesRegex(VERIFY.VerificationError, "manifest_case_count"):
            VERIFY.validate_manifest(manifest)

    def test_duplicate_cases_are_rejected(self):
        manifest = self.manifest()
        manifest["cases"][-1] = copy.deepcopy(manifest["cases"][0])
        with self.assertRaisesRegex(VERIFY.VerificationError, "manifest_duplicate_case"):
            VERIFY.validate_manifest(manifest)

    def test_source_path_cannot_escape_the_pinned_repository(self):
        manifest = self.manifest()
        manifest["cases"][0]["source_path"] = "../../outside.json"
        with self.assertRaisesRegex(VERIFY.VerificationError, "manifest_source_path"):
            VERIFY.validate_manifest(manifest)

    def test_a_mutable_source_url_is_rejected(self):
        manifest = self.manifest()
        manifest["cases"][0]["source_url"] = "https://example.invalid/main/file.json"
        with self.assertRaisesRegex(VERIFY.VerificationError, "manifest_source_url"):
            VERIFY.validate_manifest(manifest)

    def test_corrupted_input_is_rejected_before_parsing(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "case.json"
            path.write_bytes(b"changed")
            with self.assertRaisesRegex(VERIFY.VerificationError, "source_sha256_mismatch"):
                VERIFY.check_digest(path, hashlib.sha256(b"original").hexdigest(), "source")

    def test_original_final_scores_are_an_independent_oracle(self):
        case = {"category": "real_game", "expected": {"final_scores": [25000] * 4}}
        source = {"ref": "not-published", "sc": [25000, 0, 25000, 0, 25000, 0, 25000, 0]}
        VERIFY.check_original_scores(source, case)
        source["sc"][0] = 24000
        with self.assertRaisesRegex(VERIFY.VerificationError, "source_final_scores_mismatch"):
            VERIFY.check_original_scores(source, case)

    def test_missing_original_scoreboard_is_not_silently_skipped(self):
        with self.assertRaisesRegex(VERIFY.VerificationError, "source_final_scores_missing"):
            VERIFY.check_original_scores({"ref": "not-published"}, {"category": "real_game"})

    def test_parser_result_must_match_the_frozen_expected_values(self):
        case = self.manifest()["cases"][0]
        actual = copy.deepcopy(case["expected"])
        actual["records"] -= 1
        with self.assertRaisesRegex(VERIFY.VerificationError, "analysis_mismatch_records"):
            VERIFY.check_analysis(actual, case["expected"])

    def test_summary_does_not_copy_unapproved_parser_fields(self):
        expected = self.manifest()["cases"][0]["expected"]
        actual = {**expected, "names": ["DO_NOT_PUBLISH"], "source": "/private/secret"}
        result = VERIFY.check_analysis(actual, expected)
        self.assertEqual(result, expected)
        self.assertNotIn("DO_NOT_PUBLISH", json.dumps(result))

    def test_invalid_summary_type_cannot_pass_as_integer(self):
        expected = self.manifest()["cases"][0]["expected"]
        actual = {**expected, "records": True}
        with self.assertRaisesRegex(VERIFY.VerificationError, "analysis_invalid_records"):
            VERIFY.check_analysis(actual, expected)

    def test_nonzero_command_fails_without_publishing_raw_output(self):
        runner = VERIFY.Runner()
        process = subprocess.CompletedProcess([], 9, "DO_NOT_PUBLISH", "/private/secret")
        with patch.object(VERIFY.subprocess, "run", return_value=process):
            with self.assertRaisesRegex(VERIFY.VerificationError, "command_failed"):
                runner.run("probe", ["dummy"], cwd=ROOT, timeout=1)
        text = json.dumps(runner.checks)
        self.assertNotIn("DO_NOT_PUBLISH", text)
        self.assertNotIn("/private", text)
        self.assertFalse(runner.checks[0]["success"])

    def test_timeout_is_recorded_as_failure(self):
        runner = VERIFY.Runner()
        with patch.object(VERIFY.subprocess, "run", side_effect=subprocess.TimeoutExpired("dummy", 1)):
            with self.assertRaisesRegex(VERIFY.VerificationError, "command_timeout"):
                runner.run("probe", ["dummy"], cwd=ROOT, timeout=1)
        self.assertEqual(runner.checks[0]["error"], "command_timeout")

    def test_reusing_upstream_still_checks_the_fixed_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            upstream = Path(directory)
            (upstream / ".git").mkdir()
            runner = VERIFY.Runner()
            with patch.object(runner, "run", return_value="0" * 40 + "\n"):
                with self.assertRaisesRegex(VERIFY.VerificationError, "upstream_revision_mismatch"):
                    VERIFY.check_upstream(runner, upstream, self.manifest())


if __name__ == "__main__":
    unittest.main()
