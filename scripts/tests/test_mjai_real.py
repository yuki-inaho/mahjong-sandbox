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

SYNTHETIC_RECORD = (
    '{ "type":"start_game", "producer":"SYNTHETIC", '
    '"extra":{"number":9007199254740993e0,"keep":null,"nested":[{"value":"synthetic"}]} }\n'
    '{"type":"none","extra":{"position":1}}\n'
    '{"type":"none","extra":{"position":2}}\n'
    '{"type":"end_game"}\n'
).encode("utf-8")


def synthetic_observations(seat: int = 0) -> list[dict]:
    """実牌譜を読まず、比較器へ渡す形と順番だけを用意します。"""
    rows = []
    for line, kind in enumerate(("start_game", "none", "none", "end_game"), 1):
        observation = {
            "viewer": seat, "own_hand": [], "own_last_draw": None,
            "players": [{"hand_count": 0, "melds": [], "discards": [],
                         "reach_declared": False, "reach_accepted": False} for _ in range(4)],
            "scores": None, "riichi_sticks": None, "bakaze": None, "kyoku": None,
            "honba": None, "dealer": None, "dora_markers": [], "last_discard": None,
            "round_ended": False, "game_ended": line == 4, "record_ended": line == 4,
        }
        rows.append({"schema_version": 1, "line": line, "type": kind, "observation": observation})
    return rows


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

    def compare_roundtrip(self, candidate: bytes):
        with tempfile.TemporaryDirectory() as directory:
            original = Path(directory) / "original.mjson"
            restored = Path(directory) / "restored.mjson"
            original.write_bytes(SYNTHETIC_RECORD)
            restored.write_bytes(candidate)
            return VERIFY.check_roundtrip(original, restored, 4)

    def test_roundtrip_checks_every_event_and_preserves_number_tokens(self):
        result = self.compare_roundtrip(SYNTHETIC_RECORD)
        self.assertEqual(result["records"], 4)
        self.assertEqual(result["original_sha256"], result["restored_sha256"])
        self.assertNotIn("synthetic", json.dumps(result))

    def test_roundtrip_rejects_one_deleted_extension_field(self):
        changed = SYNTHETIC_RECORD.replace(b'"producer":"SYNTHETIC", ', b'')
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_event_mismatch_1"):
            self.compare_roundtrip(changed)

    def test_roundtrip_rejects_loss_of_explicit_null(self):
        changed = SYNTHETIC_RECORD.replace(b'"keep":null,', b'')
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_event_mismatch_1"):
            self.compare_roundtrip(changed)

    def test_roundtrip_rejects_reordering_with_identical_type_counts(self):
        lines = SYNTHETIC_RECORD.splitlines(keepends=True)
        lines[1], lines[2] = lines[2], lines[1]
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_event_mismatch_2"):
            self.compare_roundtrip(b''.join(lines))

    def test_roundtrip_rejects_one_missing_event(self):
        lines = SYNTHETIC_RECORD.splitlines(keepends=True)
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_record_count_mismatch"):
            self.compare_roundtrip(b''.join(lines[:1] + lines[2:]))

    def test_roundtrip_does_not_treat_rounded_numbers_as_equal(self):
        changed = SYNTHETIC_RECORD.replace(b'9007199254740993e0', b'9007199254740992')
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_event_mismatch_1"):
            self.compare_roundtrip(changed)

    def test_frozen_corpus_roundtrip_checks_complete_bytes_as_well_as_lines(self):
        with self.assertRaisesRegex(VERIFY.VerificationError, "roundtrip_bytes_mismatch"):
            self.compare_roundtrip(SYNTHETIC_RECORD.replace(b'\n', b'\r\n'))

    def compare_observations(self, actual: list[dict], *, expected: list[dict] | None = None):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / name for name in ("standard.jsonl", "annotated.jsonl")]
            for path, rows in zip(paths, (expected if expected is not None else synthetic_observations(), actual)):
                path.write_text(''.join(json.dumps(row) + '\n' for row in rows), encoding="utf-8")
            return VERIFY.check_observations(*paths, ["start_game", "none", "none", "end_game"], 0)

    def test_observations_compare_all_event_positions(self):
        result = self.compare_observations(synthetic_observations())
        self.assertEqual(result["records"], 4)
        self.assertEqual(result["seat"], 0)
        self.assertEqual(result["standard_sha256"], result["annotated_sha256"])
        self.assertNotIn("own_hand", json.dumps(result))

    def test_equal_final_observation_cannot_hide_a_changed_prefix(self):
        rows = synthetic_observations()
        rows[1]["observation"]["own_hand"] = ["5mr"]
        with self.assertRaisesRegex(VERIFY.VerificationError, "observation_value_mismatch_2"):
            self.compare_observations(rows)

    def test_observations_reject_missing_prefix_even_if_final_matches(self):
        rows = synthetic_observations()
        rows.pop(1)
        with self.assertRaisesRegex(VERIFY.VerificationError, "observation_record_count_mismatch"):
            self.compare_observations(rows)

    def test_observations_require_the_requested_seat(self):
        rows = synthetic_observations()
        rows[2]["observation"]["viewer"] = 1
        with self.assertRaisesRegex(VERIFY.VerificationError, "observation_viewer_mismatch_3"):
            self.compare_observations(rows)

    def test_identical_outputs_cannot_hide_raw_record_leakage(self):
        rows = synthetic_observations()
        rows[1]["raw"] = {"type": "start_game", "names": ["DO_NOT_PUBLISH"]}
        with self.assertRaisesRegex(VERIFY.VerificationError, "observation_row_fields_2"):
            self.compare_observations(rows, expected=rows)

    def test_identical_outputs_cannot_hide_opponent_hand_fields(self):
        rows = synthetic_observations()
        rows[1]["observation"]["players"][1]["hand"] = ["5mr"]
        with self.assertRaisesRegex(VERIFY.VerificationError, "observation_player_fields_2"):
            self.compare_observations(rows, expected=rows)

    def test_independent_red_counter_ignores_names_and_extensions(self):
        events = [
            {"type": "start_game", "names": ["5mr"], "extra": {"pai": "5pr"}},
            {"type": "tsumo", "pai": "1m", "annotation": "5sr"},
            {"type": "end_game", "annotations": {"reason": "5mr"}},
        ]
        self.assertEqual(VERIFY.count_known_red_events(events), 0)

    def test_independent_red_counter_counts_events_and_accepts_the_ura_alias(self):
        events = [
            {"type": "start_kyoku", "tehais": [["5mr", "5pr"]], "dora_marker": "5sr"},
            {"type": "pon", "pai": "5m", "consumed": ["5mr", "5m"]},
            {"type": "ankan", "consumed": ["5mr", "5m", "5m", "5m"]},
            {"type": "hora", "uradora_markers": ["5pr"]},
            {"type": "hora", "ura_markers": ["5sr"]},
        ]
        self.assertEqual(VERIFY.count_known_red_events(events), 5)


if __name__ == "__main__":
    unittest.main()
