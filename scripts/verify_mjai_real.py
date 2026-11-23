#!/usr/bin/env python3
"""固定した外部牌譜を取得・変換・解析し、本文を含めない検査記録を保存します。"""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/fixtures/mjai/manifest.json"
REPOSITORY = "https://github.com/NikkeTryHard/tenhou-to-mjai.git"
REVISION = "f08b4e0f7d96b57d0099b9b2e5eb6dd5c4e5f4eb"
RUST_VERSION = "1.100.0"
SUMMARY_COUNTS = ("records", "rounds", "wins", "draws", "calls", "red_events")
SUMMARY_FIELDS = (*SUMMARY_COUNTS, "final_scores", "riichi_sticks", "complete", "termination")
EXAMPLE_PATH = "crates/convlog/examples/tenhou2mjai.rs"
EVENT_TYPES = {
    "none", "start_game", "start_kyoku", "tsumo", "dahai", "chi", "pon", "daiminkan",
    "ankan", "kakan", "dora", "reach", "reach_accepted", "hora", "ryukyoku", "end_kyoku", "end_game",
}


class VerificationError(Exception):
    """入力本文・コマンド出力を持たず、公開可能なエラー識別子だけを保持します。"""

    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


def require(condition: bool, code: str) -> None:
    if not condition:
        raise VerificationError(code)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_digest(path: Path, expected: str, label: str) -> str:
    actual = digest(path)
    require(actual == expected, label + "_sha256_mismatch")
    return actual


def check_analysis(actual: object, expected: dict) -> dict:
    """公開する値を許可した集計項目に限定し、型と凍結した期待値を検査します。"""
    require(isinstance(actual, dict), "analysis_not_object")
    for name in SUMMARY_FIELDS:
        require(name in actual, "analysis_missing_" + name)
        value = actual[name]
        if name in SUMMARY_COUNTS or name == "riichi_sticks":
            valid = type(value) is int and 0 <= value <= 2**32 - 1
            if name == "riichi_sticks":
                valid = valid or value is None
        elif name == "final_scores":
            valid = value is None or (
                isinstance(value, list) and len(value) == 4
                and all(type(score) is int and -(2**31) <= score < 2**31 for score in value)
            )
        elif name == "complete":
            valid = type(value) is bool
        else:
            valid = type(value) is str and value in {"completed", "interrupted", "truncated"}
        require(valid, "analysis_invalid_" + name)
        require(value == expected.get(name), "analysis_mismatch_" + name)
    return {name: actual[name] for name in SUMMARY_FIELDS}


def validate_manifest(manifest: object) -> dict:
    """ケース削減、重複、可変URL、範囲外パスによる検査のすり抜けを拒否します。"""
    require(isinstance(manifest, dict), "manifest_not_object")
    require(manifest.get("schema_version") == 1, "manifest_version")
    upstream = manifest.get("upstream", {})
    require(upstream.get("repository") == REPOSITORY, "manifest_repository")
    require(upstream.get("revision") == REVISION, "manifest_revision")
    require(re.fullmatch(r"[0-9a-f]{64}", upstream.get("cargo_lock_sha256", "")) is not None,
            "manifest_lock_digest")
    cases = manifest.get("cases")
    require(isinstance(cases, list) and len(cases) == 20, "manifest_case_count")
    seen = set()
    for case in cases:
        require(isinstance(case, dict), "manifest_case_type")
        identifier = case.get("id", "")
        require(isinstance(identifier, str) and re.fullmatch(r"[a-z0-9_]+", identifier) is not None,
                "manifest_case_id")
        require(identifier not in seen, "manifest_duplicate_case")
        seen.add(identifier)
        path = f"crates/convlog/tests/testdata/{identifier}.json"
        require(case.get("source_path") == path, "manifest_source_path")
        require(case.get("source_url") == f"https://raw.githubusercontent.com/NikkeTryHard/tenhou-to-mjai/{REVISION}/{path}",
                "manifest_source_url")
        for name in ("source_sha256", "mjson_sha256"):
            require(re.fullmatch(r"[0-9a-f]{64}", case.get(name, "")) is not None, "manifest_" + name)
        for name in ("source_bytes", "mjson_bytes"):
            require(type(case.get(name)) is int and case[name] > 0, "manifest_" + name)
        require(case.get("category") in {"real_game", "external_regression"}, "manifest_category")
        expected = case.get("expected", {})
        check_analysis(expected, expected)
        require(expected["complete"] and expected["termination"] == "completed", "manifest_incomplete_case")
        counts = case.get("event_counts", {})
        require(isinstance(counts, dict) and set(counts) <= EVENT_TYPES
                and all(type(n) is int and n > 0 for n in counts.values()), "manifest_event_counts")
        require(sum(counts.values()) == expected["records"], "manifest_event_total")
    categories = Counter(case["category"] for case in cases)
    require(categories == {"real_game": 2, "external_regression": 18}, "manifest_categories")
    require(sum(case["expected"]["records"] for case in cases) == 2310, "manifest_record_total")
    require(sum(case["expected"]["rounds"] for case in cases) == 23, "manifest_round_total")
    selection = manifest.get("selection", {})
    require(selection.get("cases") == 20 and selection.get("real_games") == 2
            and selection.get("external_regressions") == 18 and selection.get("records") == 2310
            and selection.get("rounds") == 23 and selection.get("exclusions") == [], "manifest_selection")
    return manifest


class Runner:
    """標準出力は検査にのみ使い、記録には終了状態と所要時間だけを残します。"""

    def __init__(self):
        self.checks: list[dict] = []

    def run(self, stage: str, command: list[str], *, cwd: Path, timeout: int) -> str:
        started = time.monotonic()
        check = {"stage": stage, "success": False}
        self.checks.append(check)
        environment = {**os.environ, "RUSTUP_TOOLCHAIN": RUST_VERSION, "CARGO_TERM_COLOR": "never"}
        try:
            process = subprocess.run(command, cwd=cwd, env=environment, capture_output=True,
                                     text=True, encoding="utf-8", errors="replace", timeout=timeout, check=False)
            check["exit_code"] = process.returncode
            require(process.returncode == 0, "command_failed")
            check["success"] = True
            return process.stdout
        except subprocess.TimeoutExpired:
            check["error"] = "command_timeout"
            raise VerificationError("command_timeout") from None
        except OSError:
            check["error"] = "command_unavailable"
            raise VerificationError("command_unavailable") from None
        finally:
            check["seconds"] = round(time.monotonic() - started, 3)


def check_upstream(runner: Runner, upstream: Path, manifest: dict) -> None:
    require((upstream / ".git").exists(), "upstream_not_repository")
    revision = runner.run("upstream-revision", ["git", "rev-parse", "HEAD"], cwd=upstream, timeout=30).strip()
    require(revision == manifest["upstream"]["revision"], "upstream_revision_mismatch")
    origin = runner.run("upstream-origin", ["git", "remote", "get-url", "origin"], cwd=upstream, timeout=30).strip()
    require(origin == REPOSITORY, "upstream_origin_mismatch")
    runner.run("upstream-tracked-clean", ["git", "diff", "--exit-code", "HEAD", "--"], cwd=upstream, timeout=30)
    untracked = runner.run("upstream-untracked", ["git", "ls-files", "--others", "--exclude-standard"],
                           cwd=upstream, timeout=30).splitlines()
    require(set(untracked) <= {EXAMPLE_PATH}, "upstream_untracked_files")
    check_digest(upstream / "Cargo.lock", manifest["upstream"]["cargo_lock_sha256"], "upstream_lock")


def prepare_converter(runner: Runner, upstream: Path, work: Path, manifest: dict) -> Path:
    if not upstream.exists():
        upstream.parent.mkdir(parents=True, exist_ok=True)
        runner.run("upstream-clone", ["git", "clone", "--no-checkout", "--depth", "1", REPOSITORY, str(upstream)],
                   cwd=ROOT, timeout=180)
        runner.run("upstream-fetch-pin", ["git", "fetch", "--depth", "1", "origin", REVISION], cwd=upstream, timeout=180)
        runner.run("upstream-checkout-pin", ["git", "checkout", "--detach", REVISION], cwd=upstream, timeout=30)
    check_upstream(runner, upstream, manifest)
    source = upstream / "src/bin/tenhou2mjai.rs"
    example = upstream / EXAMPLE_PATH
    if example.exists():
        require(example.read_bytes() == source.read_bytes(), "upstream_example_modified")
    else:
        example.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, example)
    target = work / "upstream-target"
    runner.run("build-converter", ["cargo", "build", "--locked", "--manifest-path", str(upstream / "Cargo.toml"),
                                  "-p", "convlog", "--example", "tenhou2mjai", "--target-dir", str(target)],
               cwd=upstream, timeout=1200)
    binary = target / "debug/examples" / ("tenhou2mjai.exe" if os.name == "nt" else "tenhou2mjai")
    require(binary.is_file(), "converter_binary_missing")
    return binary


def prepare_analyzer(runner: Runner, supplied_binary: Path | None) -> Path:
    if supplied_binary is not None:
        binary = supplied_binary.resolve()
    else:
        runner.run("build-analyzer", ["cargo", "build", "--locked", "-p", "suzume-api", "--bin", "suzume-api",
                                     "--target-dir", str(ROOT / "target")], cwd=ROOT, timeout=1200)
        binary = ROOT / "target/debug" / ("suzume-api.exe" if os.name == "nt" else "suzume-api")
    require(binary.is_file(), "analyzer_binary_missing")
    return binary


def check_original_scores(source: object, case: dict) -> None:
    if case["category"] != "real_game":
        return
    require(isinstance(source, dict), "source_not_object")
    scores = source.get("sc")
    require(isinstance(scores, list) and len(scores) == 8, "source_final_scores_missing")
    require(isinstance(source.get("ref"), str) and bool(source["ref"]), "source_game_identity_missing")
    require(all(type(score) is int for score in scores[::2]), "source_final_scores_type")
    require(scores[::2] == case["expected"]["final_scores"], "source_final_scores_mismatch")


def analyze(runner: Runner, binary: Path, path: Path, case: dict, stage: str) -> dict:
    output = runner.run(stage, [str(binary), "mjai", "analyze", str(path)], cwd=ROOT, timeout=60)
    try:
        actual = json.loads(output)
    except (json.JSONDecodeError, UnicodeError):
        raise VerificationError("analysis_invalid_json") from None
    return check_analysis(actual, case["expected"])


def verify_case(runner: Runner, case: dict, upstream: Path, converted: Path,
                converter: Path, analyzer: Path) -> dict:
    source_path = upstream / case["source_path"]
    source_hash = check_digest(source_path, case["source_sha256"], "source")
    require(source_path.stat().st_size == case["source_bytes"], "source_size_mismatch")
    source = json.loads(source_path.read_text(encoding="utf-8"))
    check_original_scores(source, case)
    output_path = converted / (case["id"] + ".mjson")
    runner.run("convert-" + case["id"], [str(converter), str(source_path), str(output_path)],
               cwd=upstream, timeout=60)
    output_hash = check_digest(output_path, case["mjson_sha256"], "mjson")
    require(output_path.stat().st_size == case["mjson_bytes"], "mjson_size_mismatch")
    events = [json.loads(line) for line in output_path.read_text(encoding="utf-8").splitlines()]
    counts = Counter(event["type"] for event in events)
    require(dict(counts) == case["event_counts"], "mjson_event_counts_mismatch")
    actual = analyze(runner, analyzer, output_path, case, "analyze-" + case["id"])
    return {"id": case["id"], "category": case["category"], "success": True,
            "source_sha256": source_hash, "mjson_sha256": output_hash,
            "original_scoreboard_checked": case["category"] == "real_game", "analysis": actual}


def failure_code(error: Exception) -> str:
    if isinstance(error, VerificationError):
        return error.code
    if isinstance(error, OSError):
        return "filesystem_error"
    if isinstance(error, (json.JSONDecodeError, UnicodeError)):
        return "input_encoding_or_json_error"
    return "verification_internal_error"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, help="取得済み参照リポジトリ。固定コミット・未改変・lockを毎回検査します。")
    parser.add_argument("--binary", type=Path, help="明示した解析用実行ファイルを再利用し、sha256を記録します。省略時は構築します。")
    parser.add_argument("--report", type=Path, help="dist/verification以下の集計JSON保存先。省略時は実行日時ごとに作成します。")
    args = parser.parse_args()
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    report_path = (args.report or ROOT / f"dist/verification/mjai-real-{run_id}-summary.json").resolve()
    if not report_path.is_relative_to(ROOT / "dist/verification"):
        parser.error("report_must_be_inside_dist_verification")
    if report_path.exists():
        parser.error("report_already_exists")
    report_path.parent.mkdir(parents=True, exist_ok=True)
    runner = Runner()
    report = {"schema_version": 1, "started_at_utc": datetime.now(timezone.utc).isoformat(),
              "manifest": "tests/fixtures/mjai/manifest.json", "upstream_revision": REVISION,
              "rust_version": RUST_VERSION, "expected_cases": 20, "cases": [], "checks": runner.checks,
              "gzip": {"success": False}, "success": False}
    work = ROOT / "dist/mjai-real"
    upstream = args.upstream.resolve() if args.upstream else work / "upstream"
    try:
        manifest = validate_manifest(json.loads(MANIFEST.read_text(encoding="utf-8")))
        report["manifest_sha256"] = digest(MANIFEST)
        work.mkdir(parents=True, exist_ok=True)
        converter = prepare_converter(runner, upstream, work, manifest)
        analyzer = prepare_analyzer(runner, args.binary)
        report["analyzer_sha256"] = digest(analyzer)
        converted = work / "converted"
        converted.mkdir(parents=True, exist_ok=True)
        for case in manifest["cases"]:
            try:
                result = verify_case(runner, case, upstream, converted, converter, analyzer)
            except Exception as error:
                result = {"id": case["id"], "category": case["category"], "success": False, "error": failure_code(error)}
            report["cases"].append(result)
        gzip_case = next(case for case in manifest["cases"] if case["id"] == "ranked_game")
        plain = converted / "ranked_game.mjson"
        try:
            check_digest(plain, gzip_case["mjson_sha256"], "gzip_source")
            compressed = converted / "ranked_game-gzip.mjson"
            compressed.write_bytes(gzip.compress(plain.read_bytes(), mtime=0))
            actual = analyze(runner, analyzer, compressed, gzip_case, "analyze-gzip-same-extension")
            report["gzip"] = {"case": "ranked_game", "success": True, "analysis": actual}
        except Exception as error:
            report["gzip"] = {"case": "ranked_game", "success": False, "error": failure_code(error)}
        report["success"] = len(report["cases"]) == 20 and all(case["success"] for case in report["cases"]) and report["gzip"]["success"]
    except Exception as error:
        report["error"] = failure_code(error)
    report["evaluated_cases"] = len(report["cases"])
    report["passed_cases"] = sum(case["success"] for case in report["cases"])
    report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"success": report["success"], "passed_cases": report["passed_cases"], "expected_cases": 20,
                      "report": report_path.relative_to(ROOT).as_posix()}, ensure_ascii=False))
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
