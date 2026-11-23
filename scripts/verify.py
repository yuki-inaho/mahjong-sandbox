#!/usr/bin/env python3
"""ルール同期・Rust・対局・Lean・Kaniを実行し、出力と対象SHAを保存します。"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / "docs/verification/logs"


def source_hashes() -> dict[str, str]:
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    for directory in ("src", "tests", "rules", "proofs", "scripts"):
        paths.extend(path for path in (ROOT / directory).rglob("*")
                     if path.is_file() and ".lake" not in path.parts and "__pycache__" not in path.parts)
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(set(paths)) if path.exists()}


def execute(name: str, command: list[str], cwd: Path) -> dict[str, object]:
    start = time.monotonic()
    displayed_command = ["python3" if value == sys.executable else value for value in command]
    print(f"[{name}] {' '.join(displayed_command)}", flush=True)
    try:
        process = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
        code, output = process.returncode, process.stdout + process.stderr
    except FileNotFoundError as error:
        code, output = 127, f"必要なコマンドが見つかりません: {command[0]}\n{error}\n"
    # 公開ログには端末環境のパスを残しません。
    output = output.replace(str(ROOT), "<repository>").replace(str(ROOT.parent), "<workspace>").replace(str(Path.home()), "<home>")
    log = LOGS / f"{name}.txt"
    if log.exists() and "EXIT CODE: 0\n" not in log.read_text():
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        log.rename(LOGS / f"{name}-previous-{stamp}.txt")
    elapsed = time.monotonic() - start
    content = f"COMMAND: {' '.join(displayed_command)}\nEXIT CODE: {code}\nSECONDS: {elapsed:.3f}\n\n{output}"
    log.write_text(content)
    print(f"  {'成功' if code == 0 else '失敗'} (exit {code}, {elapsed:.2f}s)", flush=True)
    if code != 0:
        print(output[-5000:], flush=True)
    return {"id": name, "command": command, "exit_code": code, "seconds": round(elapsed, 3),
            "log": str(log.relative_to(ROOT)), "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--quick", action="store_true", help="形式検証と否定検査を明示的に省略します")
    args = parser.parse_args()
    LOGS.mkdir(parents=True, exist_ok=True)
    before = source_hashes()
    checks = [
        ("rule-sync", [sys.executable, "scripts/generate_rules.py", "--check"], ROOT),
        ("rust-format", ["cargo", "fmt", "--check"], ROOT),
        ("rust-clippy", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], ROOT),
        ("rust-tests", ["cargo", "test"], ROOT),
        ("simulation-32", ["cargo", "run", "--release", "--", "--simulate", "32", "--seed", "0"], ROOT),
    ]
    if not args.quick:
        checks.extend([
            ("lean", ["lake", "build"], ROOT / "proofs"),
            ("lean-audit", ["lake", "env", "lean", "Mahjong/Audit.lean"], ROOT / "proofs"),
            ("kani", ["cargo", "kani", "--lib", "--output-format", "terse"], ROOT),
            ("negative-checks", [sys.executable, "scripts/check_contract_negatives.py"], ROOT),
        ])
    started = datetime.now(timezone.utc).isoformat()
    results = [execute(name, command, cwd) for name, command, cwd in checks]
    after = source_hashes()
    passed = all(result["exit_code"] == 0 for result in results) and before == after
    summary = {"started_at": started, "finished_at": datetime.now(timezone.utc).isoformat(),
               "mode": "quick" if args.quick else "full", "success": passed,
               "sources_unchanged_during_run": before == after, "source_sha256": after, "checks": results}
    # 実行器固有の絶対パスを再実行手順へ持ち出しません。
    for result in results:
        if result["command"][0] == sys.executable:
            result["command"][0] = "python3"
    path = ROOT / "docs/verification/run-summary.json"
    if path.exists() and not json.loads(path.read_text()).get("success", False):
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        path.rename(path.with_name(f"run-summary-previous-{stamp}.json"))
    path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    if before != after:
        print("検査中に対象ソースが変化しました。結果を受入に使用できません。", flush=True)
    print(f"{len(results)}検査：{'全成功' if passed else '未完了'}", flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
