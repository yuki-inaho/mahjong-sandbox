#!/usr/bin/env python3
"""配布物を作成し、別の一時ディレクトリから消費してSHA256付きで保存します。公開はしません。"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile

from platform_support import ROOT, SDK, Run, capture, digest, distribution_files, environment_report, platform_key, redact, source_hashes, validate_documentation, write_json


CONSUMER_JS = r'''import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { createNodeEngine } from "@mahjong-sandbox/sdk/node";
import { createBrowserEngine } from "@mahjong-sandbox/sdk/browser";
import { Engine as NodeEngine } from "@mahjong-sandbox/sdk/wasm/node";
import { Engine as WebEngine } from "@mahjong-sandbox/sdk/wasm/web";
const require = createRequire(import.meta.url);
const browserModule = pathToFileURL(require.resolve("@mahjong-sandbox/sdk/wasm/web"));
const bytes = await readFile(new URL("./suzume_wasm_bg.wasm", browserModule));
const node = createNodeEngine();
const web = await createBrowserEngine(bytes);
const initialNode = await node.newGame();
const initialWeb = await web.newGame();
assert.ok(initialNode && typeof initialNode === "object");
assert.deepEqual(initialNode, initialWeb);
assert.deepEqual(await node.view(), initialNode);
assert.deepEqual(await web.view(), initialWeb);
const discard = initialNode.legal_actions.find(choice => choice.action.kind === "discard");
assert.ok(discard, "The initial human turn must have a discard");
const afterNode = await node.apply(discard.action);
const afterWeb = await web.apply(discard.action);
assert.deepEqual(afterNode, afterWeb);
assert.ok(afterNode.revision > initialNode.revision);
assert.deepEqual(await node.stepBot(), await web.stepBot());
const beforeRejected = await node.view();
const stale = await node.command({ version: 1, op: "apply", session_id: initialNode.session_id,
  expected_revision: initialNode.revision, action: discard.action });
assert.equal(stale.status, "error");
assert.equal(stale.code, "stale_revision");
assert.deepEqual(await node.view(), beforeRejected);
for (const Engine of [NodeEngine, WebEngine]) {
  const engine = new Engine();
  const rejected = JSON.parse(engine.request("{"));
  assert.equal(rejected.status, "error");
  engine.free();
}
await node.close();
await web.close();
console.log("PACKAGE_CONSUMER_OK node web");
'''

CONSUMER_TS = '''import { createNodeEngine } from "@mahjong-sandbox/sdk/node";
import { createBrowserEngine } from "@mahjong-sandbox/sdk/browser";
import { Engine } from "@mahjong-sandbox/sdk/wasm/node";
const raw = new Engine();
raw.free();
const client = createNodeEngine();
void client.newGame();
void client.view();
void createBrowserEngine(new Uint8Array());
// @ts-expect-error 公開要求は任意オブジェクトやanyを受理しません。
void client.command({ unknownOperation: true });
client.close();
'''


def archive_files(destination: Path, prefix: str, files: list[tuple[Path, str]], executables: set[str] | None = None) -> dict[str, str]:
    """所有者と時刻を正規化し、同一入力のソース書庫を再生成可能にします。"""
    hashes = {}
    with destination.open("xb") as output, gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for source, relative in sorted(files, key=lambda item: item[1]):
                if source.is_symlink():
                    raise RuntimeError(f"配布物へシンボリックリンクを含めません: {relative}")
                data = source.read_bytes()
                hashes[relative] = hashlib.sha256(data).hexdigest()
                info = tarfile.TarInfo(f"{prefix}/{relative}")
                info.size = len(data)
                info.mode = 0o755 if relative in (executables or set()) else 0o644
                info.mtime = 0
                archive.addfile(info, io.BytesIO(data))
    return hashes


def extract(archive: Path, directory: Path) -> None:
    with tarfile.open(archive) as source:
        source.extractall(directory, filter="data")


def check_archive(path: Path, required: set[str]) -> None:
    with tarfile.open(path) as archive:
        names = {entry.name.split("/", 1)[1] for entry in archive.getmembers() if entry.isfile() and "/" in entry.name}
    missing = required - names
    unexpected = sorted(name for name in names if "/workdocs/" in name or "/verification/logs/" in name or name.startswith("node_modules/"))
    if missing or unexpected:
        raise RuntimeError(f"配布範囲が不正です: {path.name}; 不足={sorted(missing)}; 不要な記録={unexpected}")


def json_output(output: str, is_expected) -> object:
    """npm等がJSONの前後へ警告を出しても、期待する構造のJSONだけを採用します。"""
    decoder = json.JSONDecoder()
    for index, character in enumerate(output):
        if character not in "[{":
            continue
        try:
            value, _ = decoder.raw_decode(output[index:])
        except json.JSONDecodeError:
            continue
        if is_expected(value):
            return value
    raise ValueError("期待する構造のJSON出力が見つかりません")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-only", action="store_true", help="Rustの配布物だけを作成・消費検査します。SDK検査は含みません")
    args = parser.parse_args()
    run = Run("packages")
    before = source_hashes()
    report = environment_report("native" if args.native_only else "frontend")
    products: list[Path] = []

    def require(name: str, command: list[str], cwd: Path = ROOT, timeout: int = 900, validator=None) -> None:
        if not run.execute(name, command, cwd=cwd, timeout=timeout, validator=validator)["success"]:
            raise RuntimeError(f"配布検査が失敗しました: {name}")

    try:
        if not report["success"]:
            raise RuntimeError("必要な固定版が揃っていません。scripts/doctor.pyで確認してください")
        require("metadata", ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], timeout=120)
        metadata = json_output((run.directory / "metadata.log").read_text(encoding="utf-8"),
                               lambda value: isinstance(value, dict) and "workspace_members" in value and "packages" in value)
        members = [package for package in metadata["packages"] if package["id"] in metadata["workspace_members"]]
        version = next(package["version"] for package in members if package["name"] == "suzume")
        if len({package["version"] for package in members}) != 1:
            raise RuntimeError("配布するworkspaceの版が一致していません")
        require("native-build", ["cargo", "build", "--locked", "--release", "--target-dir", str(ROOT / "target"), "-p", "suzume-cli", "-p", "suzume-api", "--bins"])
        if not args.native_only:
            require("wasm-build", [sys.executable, "scripts/build_wasm.py"], timeout=1200)
            require("sdk-build", ["npm", "--prefix", str(SDK), "run", "build"], timeout=300)
        # 内部クレートは公開registryに存在しないため、Cargoの自動registry検証を
        # 省略して書庫だけ作り、下で固定lock+local patchによる実消費検査を必須にします。
        require("crate-archives", ["cargo", "package", "--workspace", "--target-dir", str(ROOT / "target"), "--allow-dirty", "--no-verify", "--exclude-lockfile", "--offline"])
        for package in members:
            name = f"{package['name']}-{package['version']}.crate"
            destination = run.directory / name
            shutil.copy2(ROOT / "target" / "package" / name, destination)
            check_archive(destination, {"Cargo.toml", "LICENSE"})
            products.append(destination)
        if not args.native_only:
            require("npm-pack", ["npm", "pack", "--ignore-scripts", "--json", "--pack-destination", str(run.directory)], cwd=SDK, timeout=120)
            packed = json_output((run.directory / "npm-pack.log").read_text(encoding="utf-8"),
                                 lambda value: isinstance(value, list) and len(value) == 1 and isinstance(value[0], dict) and "filename" in value[0])
            npm_archive = run.directory / packed[0]["filename"]
            check_archive(npm_archive, {"package.json", "LICENSE", "wasm/node/suzume_wasm_bg.wasm", "wasm/web/suzume_wasm_bg.wasm"})
            products.append(npm_archive)
        source_archive = run.directory / f"suzume-source-{version}.tar.gz"
        distribution_hashes = archive_files(source_archive, f"suzume-source-{version}",
                                           [(path, path.relative_to(ROOT).as_posix()) for path in distribution_files()])
        products.append(source_archive)
        native_archive = run.directory / f"suzume-{version}-{platform_key()}.tar.gz"
        extension = ".exe" if os.name == "nt" else ""
        binaries = [(ROOT / "target" / "release" / (name + extension), name + extension) for name in ("suzume", "suzume-api")]
        native_readme = run.directory / "native-readme.md"
        native_readme.write_text(f"# すずめ {version}\n\n対象: {platform_key()}。作成環境の詳細はmanifest.jsonに記録しています。\n\n"
                                 "書庫を展開して、端末で次を実行してください。RustやNodeの導入は不要です。\n\n"
                                 f"```sh\n./suzume{extension} --plain --seed 0\n./suzume-api{extension} --help\n```\n\n"
                                 "APIとルールの説明: https://github.com/yuki-inaho/mahjong-sandbox\n"
                                 "Linux版は作成環境のlibcに依存します。未確認のOSやCPUへの互換性は保証していません。\n", encoding="utf-8")
        archive_files(native_archive, f"suzume-{version}", binaries + [(ROOT / "LICENSE", "LICENSE"), (native_readme, "README.md")],
                      executables={name for _, name in binaries})
        products.append(native_archive)
        workspace_lock = run.directory / "workspace-Cargo.lock"
        shutil.copy2(ROOT / "Cargo.lock", workspace_lock)
        products.append(workspace_lock)

        with tempfile.TemporaryDirectory(prefix="suzume-package-consumer-") as temporary:
            consumer = Path(temporary)
            rust = consumer / "rust"
            rust.mkdir()
            for path in products:
                if path.suffix == ".crate":
                    extract(path, rust)
            member_dirs = [f"{package['name']}-{package['version']}" for package in members]
            manifest = "[workspace]\nresolver = \"2\"\nmembers = " + json.dumps(member_dirs) + "\n\n[patch.crates-io]\n"
            for package, directory in zip(members, member_dirs):
                manifest += f"{package['name']} = {{ path = {json.dumps(directory)} }}\n"
            (rust / "Cargo.toml").write_text(manifest, encoding="utf-8")
            shutil.copy2(ROOT / "Cargo.lock", rust / "Cargo.lock")
            require("crate-consumer-build", ["cargo", "build", "--workspace", "--locked", "--offline", "--target-dir", str(rust / "target")], cwd=rust)
            require("crate-consumer-play", [str(rust / "target" / "debug" / ("suzume" + extension)), "--snapshot", "--plain", "--seed", "0"], cwd=rust, timeout=60,
                    validator=lambda output: {"success": "\x1b" not in output and "乱数種: 0" in output, "detail": "展開したCLIが制御文字なしの配牌を表示"})
            native = consumer / "native"
            native.mkdir()
            extract(native_archive, native)
            require("native-consumer-play", [str(native / f"suzume-{version}" / ("suzume" + extension)), "--snapshot", "--plain", "--seed", "0"], cwd=native, timeout=60,
                    validator=lambda output: {"success": "\x1b" not in output and "乱数種: 0" in output, "detail": "配布する実行ファイルを実際に起動"})
            if not args.native_only:
                js = consumer / "javascript"
                js.mkdir()
                (js / "package.json").write_text(json.dumps({"name": "suzume-consumer", "private": True, "type": "module"}), encoding="utf-8")
                require("npm-consumer-install", ["npm", "install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", str(npm_archive)], cwd=js, timeout=180)
                (js / "consumer.mjs").write_text(CONSUMER_JS, encoding="utf-8")
                (js / "consumer.ts").write_text(CONSUMER_TS, encoding="utf-8")
                require("npm-consumer-types", ["tsc", "--strict", "--noUncheckedIndexedAccess", "--exactOptionalPropertyTypes", "--noEmit", "--module", "NodeNext", "--target", "ES2022", "--lib", "ES2022,DOM", "consumer.ts"], cwd=js, timeout=120)
                require("npm-consumer-runtime", ["node", "consumer.mjs"], cwd=js, timeout=120,
                        validator=lambda output: {"success": "PACKAGE_CONSUMER_OK node web" in output, "detail": "公開SDKのNode/Web入口と配布された実WASMを実行"})
            source = consumer / "source"
            source.mkdir()
            extract(source_archive, source)
            require("source-consumer-build", ["cargo", "check", "--workspace", "--locked", "--offline", "--target-dir", str(source / "target")], cwd=source / f"suzume-source-{version}")
            require("source-consumer-documentation", [sys.executable, "scripts/check_documentation.py", "--json"],
                    cwd=source / f"suzume-source-{version}", timeout=120, validator=validate_documentation)
        after = source_hashes()
        if before != after:
            raise RuntimeError("作成中にソースが変化しました。この配布物は受入できません")
        code, head = capture(["git", "rev-parse", "HEAD"])
        _, status = capture(["git", "status", "--porcelain"])
        manifest = {"schema": "suzume-distribution/1", "version": version, "platform": platform_key(), "includes_sdk": not args.native_only,
                    "git_head": head.strip() if code == 0 else None, "working_tree_dirty": bool(status.strip()),
                    "environment": report, "source_sha256": after, "source_bundle_sha256_by_file": distribution_hashes,
                    "artifacts": [{"file": path.name, "bytes": path.stat().st_size, "sha256": digest(path)} for path in products],
                    "consumption": ["crateを固定lock付きbuild", "配布CLIを実行", "source bundleをbuild", "source bundle内の文書リンクとコマンド入口を検査"]
                                   + ([] if args.native_only else ["npm tarballをoffline install", "公開TypeScript型を検査", "Nodeとweb用WASMを実行"])}
        write_json(run.directory / "manifest.json", manifest)
        with (run.directory / "SHA256SUMS").open("x", encoding="utf-8", newline="\n") as output:
            for path in products:
                output.write(f"{digest(path)}  {path.name}\n")
        return 0 if run.finish({"environment": report, "manifest": "manifest.json", "sources_unchanged": True}) else 1
    except (OSError, RuntimeError, ValueError, KeyError, StopIteration) as error:
        print(redact(str(error)), flush=True)
        run.finish({"environment": report, "success": False, "error": redact(str(error)),
                    "source_sha256_before": before, "source_sha256_after": source_hashes()})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
