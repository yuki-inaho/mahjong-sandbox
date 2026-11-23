"""再現・検証・配布の共通処理。Python 3.12以降の標準ライブラリのみを使います。"""

from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "dist" / "tools"
SDK = ROOT / "packages" / "suzume"


def load_versions() -> dict:
    return json.loads((ROOT / "tools" / "versions.json").read_text(encoding="utf-8"))


def platform_key() -> str:
    system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(platform.system())
    machine = {"x86_64": "x64", "AMD64": "x64", "aarch64": "arm64", "arm64": "arm64"}.get(platform.machine())
    if not system or not machine:
        raise RuntimeError(f"未対応の環境です: {platform.system()} / {platform.machine()}")
    return f"{system}-{machine}"


def tool_environment() -> dict[str, str]:
    """プロジェクト内に導入した道具を優先し、利用者の設定ファイルは変更しません。"""
    env = os.environ.copy()
    bins = [TOOLS / "cargo" / "bin", TOOLS / "node" / "bin", TOOLS / "node",
            TOOLS / "wasm-bindgen", TOOLS / "elan" / "bin", SDK / "node_modules" / ".bin"]
    env["PATH"] = os.pathsep.join(str(p) for p in bins if p.is_dir()) + os.pathsep + env.get("PATH", "")
    if (TOOLS / "cargo" / "bin").is_dir():
        env["CARGO_HOME"] = str(TOOLS / "cargo")
        env["RUSTUP_HOME"] = str(TOOLS / "rustup")
    if (TOOLS / "elan" / "bin").is_dir():
        env["ELAN_HOME"] = str(TOOLS / "elan")
    if (TOOLS / "kani").is_dir():
        env["KANI_HOME"] = str(TOOLS / "kani")
    env.update({"CARGO_TERM_COLOR": "never", "NO_COLOR": "1", "PYTHONUTF8": "1"})
    # doctorや検査の副作用として別のRust版を自動取得しません。
    # bootstrapのrustup install/target addは明示的な導入なのでこの指定に依存しません。
    env["RUSTUP_AUTO_INSTALL"] = "0"
    return env


def redact(text: str) -> str:
    for path, label in ((ROOT, "<repository>"), (ROOT.parent, "<workspace>"), (Path.home(), "<home>"), (Path(tempfile.gettempdir()), "<temporary>")):
        text = text.replace(str(path), label).replace(str(path).replace("\\", "/"), label)
    return text


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
    """検証の証跡は既存ファイルを上書きしません。"""
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def validate_documentation(output: str) -> dict:
    """開発時と配布書庫の双方で、文書検査の空実行や矛盾する集計を拒否します。"""
    report = json.loads(output)
    positive_counts = ("documents", "local_links", "repository_commands")
    valid = isinstance(report, dict) and report.get("success") is True and report.get("errors") == []
    valid = valid and all(type(report.get(name)) is int and report[name] > 0 for name in positive_counts)
    return {"success": valid, "detail": "文書・相対リンク・コマンド入口の非空な検査とエラー0件を確認",
            "report": report}


def source_files() -> list[Path]:
    files = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".gitattributes", "LICENSE", "README.md",
                                     "docs/architecture.md", "docs/development/reproducibility.md")]
    ignored = {"target", "dist", "node_modules", ".lake", "__pycache__", ".git", "wasm"}
    for name in ("src", "tests", "rules", "proofs", "scripts", "tools", "crates", "packages", "bindings", "evaluations", "schemas", "config", "configs", "examples", "fixtures", ".github"):
        directory = ROOT / name
        if directory.is_dir():
            for parent, directories, names in os.walk(directory):
                directories[:] = [item for item in directories if item not in ignored]
                files.extend(Path(parent) / item for item in names if not item.endswith(".pyc") and item != ".DS_Store")
    return sorted(set(p for p in files if p.is_file()))


def distribution_files() -> list[Path]:
    """検査入力と配布する資料を分離し、記録追記が検証対象のSHAを変えないようにします。"""
    files = source_files() + [ROOT / "AGENTS.md", ROOT / ".gitignore"]
    directory = ROOT / "docs"
    if directory.is_dir():
        files.extend(path for path in directory.rglob("*") if path.is_file() and "__pycache__" not in path.parts and path.name != ".DS_Store")
    return sorted(set(path for path in files if path.is_file()))


def source_hashes() -> dict[str, str]:
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in source_files()}


def capture(command: list[str], cwd: Path = ROOT, timeout: int = 30) -> tuple[int, str]:
    env = tool_environment()
    executable = shutil.which(command[0], path=env["PATH"])
    if not executable:
        return 127, f"コマンドが見つかりません: {command[0]}"
    try:
        result = subprocess.run([executable, *command[1:]], cwd=cwd, env=env,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                encoding="utf-8", errors="replace", timeout=timeout, check=False)
        return result.returncode, redact(result.stdout)
    except (OSError, subprocess.TimeoutExpired) as error:
        return 124 if isinstance(error, subprocess.TimeoutExpired) else 127, redact(str(error))


def environment_report(profile: str) -> dict:
    versions = load_versions()["versions"]
    commands = {"rust": (["rustc", "--version"], versions["rust"]),
                "cargo": (["cargo", "--version"], versions["rust"]),
                "rustup": (["rustup", "--version"], versions["rustup"])}
    if profile in ("frontend", "full"):
        commands.update({"node": (["node", "--version"], versions["node"]),
                         "npm": (["npm", "--version"], versions["npm"]),
                         "typescript": (["tsc", "--version"], versions["typescript"]),
                         "wasm_bindgen": (["wasm-bindgen", "--version"], versions["wasm_bindgen"])})
    if profile in ("formal", "full"):
        commands.update({"kani": (["cargo", "kani", "--version"], versions["kani"]),
                         "lean": (["lake", "env", "lean", "--version"], versions["lean"])})
    checks = {}
    for name, (command, wanted) in commands.items():
        code, output = capture(command, cwd=ROOT / "proofs" if name == "lean" else ROOT)
        observed = re.findall(r"(?<![\d.])(?:v)?(\d+\.\d+\.\d+)(?![\d.])", output)
        checks[name] = {"expected": wanted, "observed": observed[0] if observed else None,
                        "exit_code": code, "success": code == 0 and bool(observed) and observed[0] == wanted,
                        "detail": output.strip()}
    if profile in ("frontend", "full"):
        code, output = capture(["rustup", "target", "list", "--installed", "--toolchain", versions["rust"]])
        checks["wasm_target"] = {"success": code == 0 and "wasm32-unknown-unknown" in output.splitlines(), "detail": output.strip()}
    checks["python"] = {"expected_minimum": load_versions()["python_minimum"],
                        "observed": platform.python_version(), "success": tuple(map(int, platform.python_version_tuple()[:2])) >= (3, 12)}
    code, rust_details = capture(["rustc", "-vV"])
    return {"schema": "suzume-environment/1", "profile": profile, "os": platform.system(),
            "os_release": platform.release(), "architecture": platform.machine(), "libc": list(platform.libc_ver()),
            "python": platform.python_version(),
            "rust_details": rust_details.strip() if code == 0 else None, "checks": checks,
            "success": all(check["success"] for check in checks.values())}


class Run:
    """一回ごとの専用ディレクトリ、時刻、終了値、内容ハッシュを保持する実行器。"""

    def __init__(self, kind: str):
        self.started = datetime.now(timezone.utc).isoformat()
        self.id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + uuid.uuid4().hex[:10]
        self.directory = ROOT / "dist" / kind / self.id
        self.directory.mkdir(parents=True, exist_ok=False)
        self.results: list[dict] = []

    def execute(self, name: str, command: list[str], cwd: Path = ROOT, timeout: int = 600,
                validator=None) -> dict:
        if not re.fullmatch(r"[a-z0-9_-]+", name) or any(r["id"] == name for r in self.results):
            raise ValueError(f"検査IDが不正または重複しています: {name}")
        log = self.directory / f"{name}.log"
        env = tool_environment()
        executable = shutil.which(command[0], path=env["PATH"])
        started = time.monotonic()
        timed_out = False
        displayed = ["python3" if str(c) == sys.executable else redact(str(c)) for c in command]
        print(f"[{name}] {' '.join(displayed)}", flush=True)
        with log.open("x", encoding="utf-8", newline="\n") as stream:
            if not executable:
                code = 127
                stream.write(f"コマンドが見つかりません: {command[0]}\n")
            else:
                try:
                    process = subprocess.Popen([executable, *command[1:]], cwd=cwd, env=env,
                                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                               encoding="utf-8", errors="replace",
                                               start_new_session=os.name != "nt")
                    chunks: queue.Queue[str | None] = queue.Queue()

                    def reader():
                        assert process.stdout is not None
                        for line in process.stdout:
                            chunks.put(line)
                        chunks.put(None)

                    thread = threading.Thread(target=reader, daemon=True)
                    thread.start()
                    ended = False
                    while not ended:
                        if not timed_out and time.monotonic() - started > timeout:
                            timed_out = True
                            if os.name == "nt":
                                subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False, timeout=15)
                            else:
                                try:
                                    os.killpg(process.pid, signal.SIGKILL)
                                except ProcessLookupError:
                                    pass
                        try:
                            chunk = chunks.get(timeout=0.1)
                            if chunk is None:
                                ended = True
                            else:
                                stream.write(redact(chunk))
                                stream.flush()
                        except queue.Empty:
                            if timed_out and time.monotonic() - started > timeout + 20:
                                ended = True
                    code = process.wait(timeout=20)
                    if process.stdout is not None:
                        process.stdout.close()
                    if timed_out:
                        code = 124
                        stream.write(f"\nTIMEOUT: {timeout}秒の上限に到達しました。\n")
                except (OSError, subprocess.TimeoutExpired) as error:
                    code = 127
                    stream.write(redact(str(error)) + "\n")
        output = log.read_text(encoding="utf-8")
        semantic = {"success": True, "detail": "終了値を確認"}
        if code == 0 and validator is not None:
            try:
                semantic = validator(output)
            except (ValueError, KeyError, TypeError) as error:
                semantic = {"success": False, "detail": f"結果を解釈できません: {error}"}
        result = {"id": name, "command": displayed,
                  "cwd": redact(str(cwd)), "timeout_seconds": timeout, "exit_code": code,
                  "timed_out": timed_out, "seconds": round(time.monotonic() - started, 3),
                  "success": code == 0 and not timed_out and semantic["success"], "semantic": semantic,
                  "log": log.name, "log_sha256": digest(log)}
        write_json(self.directory / f"{name}.json", result)
        self.results.append(result)
        print(f"  {'成功' if result['success'] else '失敗'} / exit={code} / {result['seconds']}秒", flush=True)
        if not result["success"]:
            print(output[-4000:] + "\n" + str(semantic["detail"]), flush=True)
        return result

    def finish(self, extra: dict) -> bool:
        success = all(item["success"] for item in self.results) and bool(self.results) and extra.get("success", True)
        report = {"schema": "suzume-run/1", "id": self.id, "started_at": self.started,
                  "finished_at": datetime.now(timezone.utc).isoformat(), "checks": self.results, **extra,
                  "success": success}
        write_json(self.directory / "summary.json", report)
        print(f"検査記録: {self.directory.relative_to(ROOT).as_posix()}/summary.json", flush=True)
        return success
