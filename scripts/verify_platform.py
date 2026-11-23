#!/usr/bin/env python3
"""固定版・Rust・JSON/SDK・形式検証を実行ID別に保存し、対象の欠落も拒否します。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

from platform_support import ROOT, SDK, Run, environment_report, load_versions, source_hashes, validate_documentation, write_json


def decision(success: bool, detail: str, **values) -> dict:
    return {"success": success, "detail": detail, **values}


def validate_python_tests(output: str, minimum: int = 1) -> dict:
    counts = re.findall(r"^Ran (\d+) tests? in ", output, re.M)
    valid = len(counts) == 1 and int(counts[0]) >= minimum and re.search(r"^OK\s*$", output, re.M) is not None
    return decision(valid, f"Pythonテストの必要最小数{minimum}件と省略なしの成功を確認", counts=counts)


def validate_generator_semantics(output: str) -> dict:
    counts = re.findall(r"生成式の意味照合: (\d+)関係・(\d+)具体例が実Rust/Lean双方で期待値と一致", output)
    return decision(counts == [("7", "19")], "実Rust/Leanを使う7関係・19例の照合を確認", counts=counts)


def validate_negative_controls(output: str) -> dict:
    identifiers = re.findall(r"^N-(\d{2}) [^\n]+: 想定どおり拒否\s*$", output, re.M)
    valid = len(identifiers) == 5 and set(identifiers) == {"01", "02", "03", "04", "05"}
    valid = valid and "否定検査: 5 / 5 成功" in output
    return decision(valid, "5件の否定検査それぞれの拒否と集計を確認", identifiers=identifiers)


def validate_lean(output: str) -> dict:
    policy = load_versions()["verification"]
    findings = re.findall(r"'([^']+)' depends on axioms:\s*\[([^\]]*)\]", output, re.S)
    findings.extend((name, "") for name in re.findall(r"'([^']+)' does not depend on any axioms", output))
    names = [name for name, _ in findings]
    axioms = {axiom.strip() for _, values in findings for axiom in values.split(",") if axiom.strip()}
    allowed = set(policy["allowed_lean_axioms"])
    expected = set(policy["lean_theorems"])
    valid = len(names) == len(expected) and set(names) == expected and axioms <= allowed and "sorryAx" not in output
    return decision(valid, "全期待定理と許可公理だけを確認" if valid else "定理の欠落・重複、未証明、未許可公理があります",
                    theorem_count=len(names), missing=sorted(expected - set(names)), unexpected=sorted(set(names) - expected),
                    axioms=sorted(axioms), rejected_axioms=sorted(axioms - allowed))


def validate_kani(output: str) -> dict:
    expected = set(load_versions()["verification"]["kani_harnesses"])
    blocks = re.findall(r"Checking harness ([\w:]+)\.\.\.(.*?)(?=Checking harness |Manual Harness Summary:|\Z)", output, re.S)
    names = [name for name, _ in blocks]
    successful = [name for name, body in blocks if body.count("VERIFICATION:- SUCCESSFUL") == 1 and "VERIFICATION:- FAILED" not in body]
    summary = re.findall(r"Complete - (\d+) successfully verified harnesses, (\d+) failures, (\d+) total\.", output)
    valid = len(names) == len(expected) and set(names) == expected and set(successful) == expected
    valid = valid and summary == [(str(len(expected)), "0", str(len(expected)))]
    return decision(valid, "全期待ハーネスの成功を確認" if valid else "ハーネスの欠落・重複・失敗または集計不一致があります",
                    harness_count=len(names), missing=sorted(expected - set(names)), successful=sorted(successful))


def validate_rust_tests(output: str) -> dict:
    matches = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out;", output)
    rows = [tuple(map(int, row)) for row in matches]
    count = sum(row[0] for row in rows)
    minimum = load_versions()["verification"]["minimum_rust_tests"]
    valid = bool(rows) and count >= minimum and all(row[1:] == (0, 0, 0) for row in rows)
    return decision(valid, f"Rustテスト {count}件、必要最小数 {minimum}件。失敗・無視・絞込は0件を要求", passed=count)


def validate_simulation(output: str) -> dict:
    count = load_versions()["verification"]["simulation_count"]
    seeds = [int(value) for value in re.findall(r"^\d+/\d+\s+seed=(\d+)\s+成功\s+", output, re.M)]
    totals = re.findall(r"結果: 成功(\d+) / 総数(\d+)、失敗(\d+)", output)
    valid = len(seeds) == count and set(seeds) == set(range(count)) and totals == [(str(count), str(count), "0")]
    return decision(valid, f"種0〜{count - 1}の母数固定検査", successful_seeds=seeds)


def validate_node_tests(output: str) -> dict:
    counts = {name: re.findall(r"^# " + name + r" (\d+)\s*$", output, re.M)
              for name in ("tests", "pass", "fail", "cancelled", "skipped", "todo")}
    valid = all(len(value) == 1 for value in counts.values())
    valid = valid and int(counts["tests"][0]) > 0 and counts["pass"] == counts["tests"]
    valid = valid and all(counts[name] == ["0"] for name in ("fail", "cancelled", "skipped", "todo"))
    return decision(valid, "NodeのTAP集計を検査。0件・失敗・中断・省略を拒否", counts=counts)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("native", "frontend", "formal", "full"), default="full")
    args = parser.parse_args()
    run = Run("verification")
    before = source_hashes()
    report = environment_report(args.profile)
    write_json(run.directory / "environment.json", report)
    run.execute("tool-versions", [sys.executable, "scripts/doctor.py", "--profile", args.profile, "--json"], timeout=180,
                validator=lambda output: decision(json.loads(output)["success"], "固定版・対象・Python最小版を照合"))
    if report["success"]:
        run.execute("documentation-links", [sys.executable, "scripts/check_documentation.py", "--json"], timeout=120,
                    validator=validate_documentation)
        run.execute("documentation-checker-tests", [sys.executable, "scripts/check_documentation.py", "--self-test"], timeout=120,
                    validator=lambda output: validate_python_tests(output, 10))
        run.execute("rule-sync", [sys.executable, "scripts/generate_rules.py", "--check"], timeout=60)
        run.execute("generator-tests", [sys.executable, "scripts/test_generate_rules.py"], timeout=120,
                    validator=lambda output: validate_python_tests(output, 19))
        if args.profile in ("formal", "full"):
            run.execute("generator-rust-lean-semantics", [sys.executable, "scripts/check_generator_semantics.py"], timeout=300, validator=validate_generator_semantics)
        run.execute("verifier-self-tests", [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_platform_checks.py"], timeout=120,
                    validator=lambda output: validate_python_tests(output, 13))
        if args.profile in ("native", "frontend", "full"):
            run.execute("rust-format", ["cargo", "fmt", "--all", "--check"], timeout=120)
            run.execute("rust-clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"], timeout=900)
            run.execute("rust-tests", ["cargo", "test", "--workspace", "--locked"], timeout=900, validator=validate_rust_tests)
            run.execute("simulation-32", ["cargo", "run", "-p", "suzume-cli", "--bin", "suzume", "--locked", "--release", "--", "--simulate", "32", "--seed", "0"], timeout=600,
                        validator=validate_simulation)
        if args.profile in ("frontend", "full"):
            run.execute("api-schema-sync", ["cargo", "run", "--locked", "-p", "suzume-api", "--bin", "export-schema", "--", "--check"], timeout=600)
            run.execute("typescript-protocol-sync", [sys.executable, "scripts/generate_protocol_ts.py", "--check"], timeout=60)
            run.execute("wasm-build", [sys.executable, "scripts/build_wasm.py"], timeout=1200)
            run.execute("sdk-build", ["npm", "--prefix", str(SDK), "run", "build"], timeout=300)
            run.execute("sdk-tests", ["npm", "--prefix", str(SDK), "test"], timeout=300, validator=validate_node_tests)
        if args.profile in ("formal", "full"):
            run.execute("lean-build", ["lake", "build"], cwd=ROOT / "proofs", timeout=600)
            run.execute("lean-axioms", ["lake", "env", "lean", "Mahjong/Audit.lean"], cwd=ROOT / "proofs", timeout=180, validator=validate_lean)
            run.execute("kani", ["cargo", "kani", "-p", "suzume", "--lib", "--output-format", "terse"], timeout=1200, validator=validate_kani)
            run.execute("negative-controls", [sys.executable, "scripts/check_contract_negatives.py", "--log", str(run.directory / "negative-controls-detail.log")], timeout=900,
                        validator=validate_negative_controls)
    after = source_hashes()
    changed = sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))
    all_profiles = {"native", "frontend", "formal"}
    included = all_profiles if args.profile == "full" else ({"native", "frontend"} if args.profile == "frontend" else {args.profile})
    full_passed = args.profile == "full" and report["success"] and not changed and all(item["success"] for item in run.results)
    passed = run.finish({"profile": args.profile, "full_verification": full_passed, "omitted_profiles": sorted(all_profiles - included),
                         "environment": report, "source_sha256_before": before, "source_sha256_after": after,
                         "sources_unchanged": not changed, "changed_sources": changed, "success": report["success"] and not changed})
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
