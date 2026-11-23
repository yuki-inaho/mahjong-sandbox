#!/usr/bin/env python3
"""公式の固定版をdist/toolsへ導入します。シェル設定やOS全体を変更しません。"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import tarfile
import tempfile
import time
import urllib.request
import zipfile

from platform_support import ROOT, SDK, TOOLS, Run, capture, digest, environment_report, load_versions, platform_key


def download(kind: str, specification: dict) -> Path:
    artifact = specification["artifacts"].get(kind, {}).get(platform_key())
    if artifact is None:
        raise RuntimeError(f"{kind}の検査済み配布物が未登録です: {platform_key()}")
    cache = TOOLS / "downloads" / kind
    cache.mkdir(parents=True, exist_ok=True)
    # rustup-initはargv[0]で動作を選ぶため、公式のファイル名を保持します。
    destination = cache / artifact["url"].rsplit("/", 1)[-1]
    if not destination.exists() or digest(destination) != artifact["sha256"]:
        temporary = destination.with_suffix(destination.suffix + ".partial")
        try:
            started = time.monotonic()
            with urllib.request.urlopen(artifact["url"], timeout=60) as response, temporary.open("wb") as output:
                total = 0
                while chunk := response.read(1024 * 1024):
                    total += len(chunk)
                    if total > 128 * 1024 * 1024 or time.monotonic() - started > 300:
                        raise RuntimeError(f"配布物の取得上限を超えました: {kind}")
                    output.write(chunk)
            if digest(temporary) != artifact["sha256"]:
                raise RuntimeError(f"公式配布物のSHA256が一致しません: {kind}")
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)
    print(f"配布物照合済み: {kind} / {artifact['sha256']}", flush=True)
    return destination


def unpack(archive: Path, destination: Path) -> None:
    if not destination.resolve().is_relative_to(TOOLS.resolve()):
        raise RuntimeError("道具の導入先がdist/toolsの外です")
    TOOLS.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TOOLS, prefix="unpack-") as temporary:
        stage = Path(temporary)
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as source:
                for name in source.namelist():
                    if not (stage / name).resolve().is_relative_to(stage.resolve()):
                        raise RuntimeError("配布物に不正な展開先があります")
                source.extractall(stage)
        else:
            with tarfile.open(archive) as source:
                source.extractall(stage, filter="data")
        entries = list(stage.iterdir())
        content = entries[0] if len(entries) == 1 and entries[0].is_dir() else stage
        if destination.exists():
            shutil.rmtree(destination)
        # 未完成の実行ファイルを別の処理から参照されないよう、展開後に移します。
        content.replace(destination)


def has_version(command: list[str], version: str, cwd: Path = ROOT) -> bool:
    code, output = capture(command, cwd=cwd)
    import re
    return code == 0 and re.search(r"(?<![\d.])" + re.escape(version) + r"(?![\d.])", output) is not None


def write_activation() -> None:
    """対話利用時にも作業用homeを選べるようにし、通常のシェル設定は変更しません。"""
    TOOLS.mkdir(parents=True, exist_ok=True)
    shell = '''# bash/zshでは source dist/tools/activate.sh を実行してください。
suzume_tools_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-${(%):-%x}}")" && pwd)"
if [ -d "$suzume_tools_dir/cargo/bin" ]; then
  export CARGO_HOME="$suzume_tools_dir/cargo"
  export RUSTUP_HOME="$suzume_tools_dir/rustup"
fi
if [ -d "$suzume_tools_dir/elan/bin" ]; then export ELAN_HOME="$suzume_tools_dir/elan"; fi
if [ -d "$suzume_tools_dir/kani" ]; then export KANI_HOME="$suzume_tools_dir/kani"; fi
for suzume_bin in "$suzume_tools_dir/../../packages/suzume/node_modules/.bin" "$suzume_tools_dir/elan/bin" "$suzume_tools_dir/wasm-bindgen" "$suzume_tools_dir/node" "$suzume_tools_dir/node/bin" "$suzume_tools_dir/cargo/bin"; do
  if [ -d "$suzume_bin" ]; then export PATH="$suzume_bin:$PATH"; fi
done
unset suzume_bin suzume_tools_dir
'''
    powershell = '''# PowerShellでは . ./dist/tools/activate.ps1 を実行してください。
$suzumeTools = $PSScriptRoot
if (Test-Path "$suzumeTools/cargo/bin") {
  $env:CARGO_HOME = "$suzumeTools/cargo"
  $env:RUSTUP_HOME = "$suzumeTools/rustup"
}
if (Test-Path "$suzumeTools/elan/bin") { $env:ELAN_HOME = "$suzumeTools/elan" }
if (Test-Path "$suzumeTools/kani") { $env:KANI_HOME = "$suzumeTools/kani" }
foreach ($suzumeBin in @("$suzumeTools/../../packages/suzume/node_modules/.bin", "$suzumeTools/elan/bin", "$suzumeTools/wasm-bindgen", "$suzumeTools/node", "$suzumeTools/node/bin", "$suzumeTools/cargo/bin")) {
  if (Test-Path $suzumeBin) { $env:PATH = "$suzumeBin$([IO.Path]::PathSeparator)$env:PATH" }
}
Remove-Variable suzumeTools, suzumeBin
'''
    (TOOLS / "activate.sh").write_text(shell, encoding="utf-8")
    (TOOLS / "activate.ps1").write_text(powershell, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("native", "frontend", "formal", "full"), default="frontend")
    parser.add_argument("--reuse-existing", action="store_true", help="指定版と一致する既存の道具を再利用します")
    args = parser.parse_args()
    specification = load_versions()
    versions = specification["versions"]
    run = Run("bootstrap")

    def command(name: str, argv: list[str], timeout: int = 1200) -> None:
        if not run.execute(name, argv, timeout=timeout)["success"]:
            raise RuntimeError(f"{name}が失敗しました。上記の実行記録を確認してください")

    try:
        if args.profile in ("formal", "full") and platform_key() != "linux-x64":
            raise RuntimeError("形式検証の自動導入はLinux x86_64が対象です。native/frontendは他OSでも使用できます")
        reuse_rust = (args.reuse_existing and has_version(["rustc", "--version"], versions["rust"])
                      and has_version(["rustup", "--version"], versions["rustup"]))
        local_rustup = TOOLS / "cargo" / "bin" / ("rustup.exe" if os.name == "nt" else "rustup")
        if not reuse_rust and not (local_rustup.exists() and has_version([str(local_rustup), "--version"], versions["rustup"])):
            installer = download("rustup", specification)
            installer.chmod(installer.stat().st_mode | 0o111)
            (TOOLS / "cargo" / "bin").mkdir(parents=True, exist_ok=True)
            (TOOLS / "rustup").mkdir(parents=True, exist_ok=True)
            command("rustup-install", [str(installer), "-y", "--no-modify-path", "--profile", "minimal",
                                      "--default-toolchain", versions["rust"]])
        command("rust-install", ["rustup", "toolchain", "install", versions["rust"], "--profile", "minimal",
                                 "--component", "rustfmt", "--component", "clippy", "--no-self-update"])
        if args.profile in ("frontend", "full"):
            command("rust-wasm-target", ["rustup", "target", "add", "wasm32-unknown-unknown", "--toolchain", versions["rust"]])
            if not (args.reuse_existing and has_version(["node", "--version"], versions["node"])):
                unpack(download("node", specification), TOOLS / "node")
            if not has_version(["npm", "--version"], versions["npm"]):
                command("npm-install", ["npm", "install", "--global", "--prefix", str(TOOLS / "node"),
                                        "--ignore-scripts", "--no-audit", "--no-fund", "npm@" + versions["npm"]])
            if not (args.reuse_existing and has_version(["wasm-bindgen", "--version"], versions["wasm_bindgen"])):
                unpack(download("wasm_bindgen", specification), TOOLS / "wasm-bindgen")
            if not (SDK / "package-lock.json").is_file():
                raise RuntimeError("packages/suzume/package-lock.jsonが必要です。依存を暗黙に更新せず終了します")
            command("sdk-dependencies", ["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund", "--prefix", str(SDK)])
        if args.profile in ("formal", "full"):
            if not (args.reuse_existing and has_version(["lake", "env", "lean", "--version"], versions["lean"], ROOT / "proofs")):
                unpack(download("elan", specification), TOOLS / "elan-installer")
                (TOOLS / "elan" / "bin").mkdir(parents=True, exist_ok=True)
                command("lean-install", [str(TOOLS / "elan-installer" / "elan-init"), "-y", "--no-modify-path",
                                         "--default-toolchain", "leanprover/lean4:v" + versions["lean"]], 1800)
            # Rustを別の作業用homeへ導入した場合、Kaniの専用nightlyもそのhomeへ
            # setupする必要があります。バージョン表示だけでは導入完了とみなしません。
            if (TOOLS / "cargo" / "bin").is_dir() or not (args.reuse_existing and has_version(["cargo", "kani", "--version"], versions["kani"])):
                (TOOLS / "kani").mkdir(parents=True, exist_ok=True)
                command("kani-install", ["cargo", "+" + versions["rust"], "install", "--locked", "--version",
                                         "=" + versions["kani"], "kani-verifier"], 1800)
                command("kani-setup", ["cargo", "kani", "setup"], 1800)
        report = environment_report(args.profile)
        if report["success"]:
            write_activation()
        return 0 if run.finish({"profile": args.profile, "environment": report, "success": report["success"]}) else 1
    except (OSError, RuntimeError, ValueError) as error:
        from platform_support import redact
        print(redact(str(error)), flush=True)
        run.finish({"profile": args.profile, "success": False, "error": redact(str(error))})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
