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

from platform_support import ROOT, SDK, Run, capture, digest, distribution_files, environment_report, platform_key, private_path_matches, private_path_prefixes, redact, release_environment, source_hashes, validate_documentation, write_json


# 独自コアで生成した8操作の旧replayから変換する例。外部牌譜ではありません。
SAMPLE_MJAI_SUMMARY = {
    "records": 8, "rounds": 1, "wins": 0, "draws": 0, "calls": 0, "red_events": 0,
    "final_scores": [25000, 25000, 25000, 25000], "riichi_sticks": 0,
    "complete": False, "termination": "truncated",
}
SAMPLE_FILES = ("examples/eight-actions.mjson", "examples/replay-eight-actions.jsonl")


NATIVE_MJAI_CONSUMER = r'''"""展開した実行ファイルだけで形式間の保持を検査します。"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

binary = sys.argv[1]

def command(*arguments):
    result = subprocess.run([binary, "mjai", *arguments], capture_output=True, text=True, check=False)
    if result.returncode:
        raise RuntimeError("native_mjai_command_failed_" + arguments[0])
    return result.stdout

original = Path("examples/eight-actions.mjson")
summary = json.loads(command("analyze", str(original)))
command("wrap", str(original), "--output", "wrapped.annotated.jsonl")
annotated = json.loads(command("analyze", "wrapped.annotated.jsonl", "--format", "annotated_mjai"))
if summary != annotated:
    raise RuntimeError("native_annotated_summary_mismatch")
command("unwrap", "wrapped.annotated.jsonl", "--output", "restored.mjson")
restored = Path("restored.mjson")
if original.read_bytes() != restored.read_bytes():
    raise RuntimeError("native_event_bytes_mismatch")
events = [json.loads(line) for line in original.read_bytes().splitlines()]
if len(events) != summary["records"] or len(events) == 0:
    raise RuntimeError("native_roundtrip_count_mismatch")
observations = 0
for seat in range(4):
    standard_view = Path("standard-seat-" + str(seat) + ".jsonl")
    annotated_view = Path("annotated-seat-" + str(seat) + ".jsonl")
    command("observations", str(original), "--seat", str(seat), "--output", str(standard_view))
    command("observations", "wrapped.annotated.jsonl", "--seat", str(seat),
            "--output", str(annotated_view), "--format", "annotated_mjai")
    if standard_view.read_bytes() != annotated_view.read_bytes():
        raise RuntimeError("native_observation_bytes_mismatch")
    rows = [json.loads(line) for line in standard_view.read_bytes().splitlines()]
    if len(rows) != len(events):
        raise RuntimeError("native_observation_count_mismatch")
    for index, (row, event) in enumerate(zip(rows, events), 1):
        if (set(row) != {"schema_version", "line", "type", "observation"}
                or row["line"] != index or row["type"] != event["type"]
                or row["observation"]["viewer"] != seat):
            raise RuntimeError("native_observation_position_or_viewer_mismatch")
    observations += len(rows)
print(json.dumps({"marker": "NATIVE_MJAI_CONSUMER_OK", "summary": summary,
                  "roundtrip_records": len(events), "compared_observations": observations,
                  "original_sha256": hashlib.sha256(original.read_bytes()).hexdigest(),
                  "restored_sha256": hashlib.sha256(restored.read_bytes()).hexdigest()}))
'''


CONSUMER_JS = r'''import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { parseJson, ProtocolError, readMjaiEvents, parseMjaiRecord, parseAnnotatedMjaiRecord,
  readAnnotatedMjaiRecords, encodeAnnotatedMjaiRecord, wrapMjaiRecords,
  unwrapAnnotatedMjaiRecords } from "@mahjong-sandbox/sdk";
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
const mjson = await readFile(new URL("./eight-actions.mjson", import.meta.url), "utf8");
const expectedMjai = parseJson(await readFile(new URL("./mjai-expected.json", import.meta.url), "utf8"));
const encoded = new TextEncoder().encode(mjson);
async function* mjaiChunks() {
  // 日本語を含むUTF-8を文字境界に依存しない小片で渡します。
  for (let offset = 0; offset < encoded.byteLength; offset += 13) yield encoded.subarray(offset, offset + 13);
}
const events = [];
for await (const event of readMjaiEvents(mjaiChunks())) events.push(event);
assert.ok(events.length > 0);
assert.equal(events.length, expectedMjai.records);
assert.equal(events[0].type, "start_game");
assert.equal(events.at(-1).type, "end_game");
const beforeMjaiNode = await node.view();
const beforeMjaiWeb = await web.view();
const mjaiNode = await node.analyzeMjai(mjson, 0);
const mjaiWeb = await web.analyzeMjai(mjson, 0);
assert.equal(mjaiNode.kind, "mjai");
assert.deepEqual(mjaiNode, mjaiWeb);
assert.deepEqual(mjaiNode.summary, expectedMjai);
assert.equal(mjaiNode.observation.own_hand.length, 13);
assert.equal("legal_mask" in mjaiNode.observation, false);
await assert.rejects(node.analyzeMjai(mjson, 4), ProtocolError);
assert.deepEqual(await node.view(), beforeMjaiNode);
const spectator = await node.analyzeMjai(mjson);
assert.equal(spectator.observation.own_hand, null);
const wrappedLines = [];
const wrapReport = await wrapMjaiRecords(mjaiChunks(), line => { wrappedLines.push(line); });
assert.deepEqual(wrapReport, { records: expectedMjai.records, droppedAnnotationRecords: 0 });
const wrappedText = wrappedLines.join("");
function chunksOf(text) {
  const data = new TextEncoder().encode(text);
  return (async function* () {
    for (let offset = 0; offset < data.length; offset += 13) yield data.subarray(offset, offset + 13);
  })();
}
const wrappedRecords = [];
for await (const record of readAnnotatedMjaiRecords(chunksOf(wrappedText))) wrappedRecords.push(record);
assert.equal(wrappedRecords.length, expectedMjai.records);
for (const [index, line] of mjson.trimEnd().split("\n").entries()) {
  assert.equal(wrappedRecords[index].eventJson, parseMjaiRecord(line).json);
  assert.equal(wrappedRecords[index].index, index + 1);
}
const restoredLines = [];
const unwrapReport = await unwrapAnnotatedMjaiRecords(chunksOf(wrappedText), line => { restoredLines.push(line); });
assert.deepEqual(unwrapReport, { records: expectedMjai.records, droppedAnnotationRecords: 0 });
assert.equal(restoredLines.join(""), mjson);
const noteJson = '{"analysis":{"number":9007199254740993e0,"reason":"synthetic"}}';
const firstWithNote = encodeAnnotatedMjaiRecord(wrappedRecords[0].eventJson, 1, noteJson);
assert.equal(parseAnnotatedMjaiRecord(firstWithNote).annotationsJson, noteJson);
const annotatedText = firstWithNote + "\n" + wrappedLines.slice(1).join("");
await assert.rejects(unwrapAnnotatedMjaiRecords(chunksOf(annotatedText), () => {}), ProtocolError);
const droppedLines = [];
const dropped = await unwrapAnnotatedMjaiRecords(chunksOf(annotatedText), line => { droppedLines.push(line); },
                                                { dropAnnotations: true });
assert.deepEqual(dropped, { records: expectedMjai.records, droppedAnnotationRecords: 1 });
assert.equal(droppedLines.join(""), mjson);
for (let seat = 0; seat < 4; seat++) {
  const standardNode = seat === 0 ? mjaiNode : await node.analyzeMjai(mjson, seat);
  const standardWeb = seat === 0 ? mjaiWeb : await web.analyzeMjai(mjson, seat);
  assert.deepEqual(await node.analyzeMjai(annotatedText, seat, "annotated_mjai"), standardNode);
  assert.deepEqual(await web.analyzeMjai(annotatedText, seat, "annotated_mjai"), standardWeb);
}
assert.deepEqual(await node.analyzeMjai(annotatedText, undefined, "annotated_mjai"), spectator);
await assert.rejects(node.analyzeMjai(annotatedText, 0, "unknown_format"), ProtocolError);
assert.deepEqual(await node.view(), beforeMjaiNode);
assert.deepEqual(await web.view(), beforeMjaiWeb);
for (const Engine of [NodeEngine, WebEngine]) {
  const engine = new Engine();
  const rejected = JSON.parse(engine.request("{"));
  assert.equal(rejected.status, "error");
  engine.free();
}
await node.close();
await web.close();
console.log("PACKAGE_CONSUMER_OK node web mjai annotated");
'''

CONSUMER_TS = '''import { createNodeEngine } from "@mahjong-sandbox/sdk/node";
import { createBrowserEngine } from "@mahjong-sandbox/sdk/browser";
import { Engine } from "@mahjong-sandbox/sdk/wasm/node";
import { readMjaiEvents, parseMjaiRecord, parseAnnotatedMjaiRecord,
  readAnnotatedMjaiRecords, encodeAnnotatedMjaiRecord,
  wrapMjaiRecords, unwrapAnnotatedMjaiRecords } from "@mahjong-sandbox/sdk";
const raw = new Engine();
raw.free();
const client = createNodeEngine();
void client.newGame();
void client.view();
void client.analyzeMjai("", 0);
void client.analyzeMjai("", 0, "annotated_mjai");
void readMjaiEvents([new Uint8Array()]);
const parsedEventJson: string = parseMjaiRecord('{"type":"start_game"}').json;
const annotated = encodeAnnotatedMjaiRecord(parsedEventJson, 1, '{}');
const annotationJson: string = parseAnnotatedMjaiRecord(annotated).annotationsJson;
void annotationJson;
void readAnnotatedMjaiRecords([new Uint8Array()]);
const write = async (line: string): Promise<void> => { void line; };
const report: Promise<{ records: number; droppedAnnotationRecords: number }> =
  wrapMjaiRecords([new Uint8Array()], write);
void report;
void unwrapAnnotatedMjaiRecords([new Uint8Array()], write, { dropAnnotations: true });
void createBrowserEngine(new Uint8Array());
// @ts-expect-error 公開要求は任意オブジェクトやanyを受理しません。
void client.command({ unknownOperation: true });
// @ts-expect-error mjsonの閲覧席に文字列は指定できません。
void client.analyzeMjai("", "east");
// @ts-expect-error 公開する形式選択は閉じた列挙です。
void client.analyzeMjai("", 0, "unknown_format");
// @ts-expect-error 保存には注釈の原文が必要で、オブジェクトを暗黙に再直列化しません。
void encodeAnnotatedMjaiRecord(parsedEventJson, 1, { analysis: 1 });
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
    unexpected = sorted(name for name in names if "/workdocs/" in name or "/verification/logs/" in name
                        or name.startswith("node_modules/") or name.startswith("dist/mjai-real/")
                        or "/dist/mjai-real/" in name)
    if missing or unexpected:
        raise RuntimeError(f"配布範囲が不正です: {path.name}; 不足={sorted(missing)}; 不要な記録={unexpected}")


def scan_distribution_paths(archives: list[tuple[Path, set[str]]], env: dict[str, str] | None = None) -> dict:
    """書庫内の必須native/WASMを検査。実環境prefixそのものは返しません。"""
    prefixes = private_path_prefixes(env)
    entries = []
    missing = []
    required_count = sum(len(required) for _, required in archives)
    valid = bool(archives) and all(required for _, required in archives)
    for path, required in archives:
        seen = set()
        with tarfile.open(path) as archive:
            for member in archive:
                if member.name not in required or not member.isfile():
                    continue
                if member.name in seen:
                    valid = False
                seen.add(member.name)
                with archive.extractfile(member) as source:
                    data = source.read()
                matches = private_path_matches(data, prefixes)
                entries.append({"archive": path.name, "entry": member.name,
                                "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
                                "private_prefix_matches": matches, "success": bool(data) and matches == 0})
        missing.extend({"archive": path.name, "entry": name} for name in sorted(required - seen))
    success = valid and required_count > 0 and len(entries) == required_count and not missing
    success = success and all(entry["success"] for entry in entries)
    return {"success": bool(success), "required": required_count, "checked": len(entries),
            "entries": entries, "missing": missing}


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


def validate_mjai_sample(output: str) -> dict:
    """固定した自作例の非空件数・点数・打切りを配布バイナリで確認します。"""
    summary = json_output(output, lambda value: isinstance(value, dict) and "records" in value)
    counters = ("records", "rounds", "wins", "draws", "calls", "red_events", "riichi_sticks")
    valid = summary == SAMPLE_MJAI_SUMMARY and all(type(summary.get(key)) is int for key in counters)
    valid = valid and summary.get("records", 0) > 0 and summary.get("rounds", 0) > 0 and summary.get("complete") is False
    return {"success": valid, "detail": "自作例の8イベント・1局・各25000点・truncatedを確認", "summary": summary}


def validate_native_mjai_consumer(output: str) -> dict:
    """1つの配布消費段階で実行した各形式の照合結果を、固定母数で検査します。"""
    result = json_output(output, lambda value: isinstance(value, dict)
                         and value.get("marker") == "NATIVE_MJAI_CONSUMER_OK")
    summary = validate_mjai_sample(json.dumps(result.get("summary")))
    valid = summary["success"] and type(result.get("roundtrip_records")) is int
    valid = valid and result["roundtrip_records"] == SAMPLE_MJAI_SUMMARY["records"]
    valid = valid and type(result.get("compared_observations")) is int
    valid = valid and result["compared_observations"] == SAMPLE_MJAI_SUMMARY["records"] * 4
    expected_digest = digest(ROOT / SAMPLE_FILES[0])
    valid = valid and result.get("original_sha256") == result.get("restored_sha256") == expected_digest
    return {"success": valid, "detail": "配布nativeの原文往復・2形式の集計・8イベント後の全4席32観測を照合",
            "summary": summary["summary"], "roundtrip_records": result.get("roundtrip_records"),
            "compared_observations": result.get("compared_observations")}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-only", action="store_true", help="Rustの配布物だけを作成・消費検査します。SDK検査は含みません")
    args = parser.parse_args()
    run = Run("packages")
    before = source_hashes()
    report = environment_report("native" if args.native_only else "frontend")
    products: list[Path] = []

    def require(name: str, command: list[str], cwd: Path = ROOT, timeout: int = 900, validator=None, env=None) -> None:
        if not run.execute(name, command, cwd=cwd, timeout=timeout, validator=validator, env=env)["success"]:
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
        require("native-build", ["cargo", "build", "--locked", "--release", "--target-dir", str(ROOT / "target"), "-p", "suzume-cli", "-p", "suzume-api", "--bins"],
                env=release_environment())
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
        source_files = [(path, path.relative_to(ROOT).as_posix()) for path in distribution_files()]
        if any(relative.startswith("dist/") for _, relative in source_files):
            raise RuntimeError("検査用の外部牌譜や作業用出力をソース配布物へ含めません")
        distribution_hashes = archive_files(source_archive, f"suzume-source-{version}",
                                           source_files)
        products.append(source_archive)
        native_archive = run.directory / f"suzume-{version}-{platform_key()}.tar.gz"
        extension = ".exe" if os.name == "nt" else ""
        binaries = [(ROOT / "target" / "release" / (name + extension), name + extension) for name in ("suzume", "suzume-api")]
        native_readme = run.directory / "native-readme.md"
        native_readme.write_text(f"# すずめ {version}\n\n対象: {platform_key()}。作成環境の詳細はmanifest.jsonに記録しています。\n\n"
                                 "書庫を展開して、端末で次を実行してください。RustやNodeの導入は不要です。\n\n"
                                 f"```sh\n./suzume{extension} --plain --seed 0\n./suzume-api{extension} --help\n"
                                 f"./suzume-api{extension} mjai analyze examples/eight-actions.mjson\n"
                                 f"./suzume-api{extension} mjai wrap examples/eight-actions.mjson --output wrapped.annotated.jsonl\n"
                                 f"./suzume-api{extension} mjai analyze wrapped.annotated.jsonl --format annotated_mjai\n"
                                 f"./suzume-api{extension} mjai unwrap wrapped.annotated.jsonl --output restored.mjson\n"
                                 f"./suzume-api{extension} mjai convert examples/replay-eight-actions.jsonl --output converted.mjson\n```\n\n"
                                 "同梱例は本コアで生成した8操作の途中記録です。外部の実牌譜ではありません。\n"
                                 "解析結果は8イベント・1局・各25000点、complete=false、termination=truncatedです。\n\n"
                                 "APIとルールの説明: https://github.com/yuki-inaho/mahjong-sandbox\n"
                                 "Linux版は作成環境のlibcに依存します。未確認のOSやCPUへの互換性は保証していません。\n", encoding="utf-8")
        archive_files(native_archive, f"suzume-{version}", binaries + [(ROOT / "LICENSE", "LICENSE"), (native_readme, "README.md")]
                      + [(ROOT / relative, relative) for relative in SAMPLE_FILES],
                      executables={name for _, name in binaries})
        check_archive(native_archive, {name for _, name in binaries} | set(SAMPLE_FILES) | {"README.md", "LICENSE"})
        products.append(native_archive)
        path_scan_inputs = [(native_archive, {f"suzume-{version}/{name}" for _, name in binaries})]
        if not args.native_only:
            path_scan_inputs.append((npm_archive, {
                "package/wasm/node/suzume_wasm_bg.wasm", "package/wasm/web/suzume_wasm_bg.wasm"}))
        path_scan = scan_distribution_paths(path_scan_inputs)
        write_json(run.directory / "distribution-path-checks.json", path_scan)
        if not path_scan["success"]:
            raise RuntimeError("配布native/WASMの必須entryまたは既知の私的prefix検査が失敗しました。distribution-path-checks.jsonを確認してください")
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
            native_root = native / f"suzume-{version}"
            native_api = str(native_root / ("suzume-api" + extension))
            (native_root / "consume_mjai.py").write_text(NATIVE_MJAI_CONSUMER, encoding="utf-8")
            require("native-consumer-mjai", [sys.executable, "consume_mjai.py", native_api],
                    cwd=native_root, timeout=120, validator=validate_native_mjai_consumer)
            require("native-consumer-convert", [native_api, "mjai", "convert", "examples/replay-eight-actions.jsonl", "--output", "converted.mjson"],
                    cwd=native_root, timeout=60)
            require("native-consumer-converted-mjai", [native_api, "mjai", "analyze", "converted.mjson"],
                    cwd=native_root, timeout=60, validator=validate_mjai_sample)
            if not args.native_only:
                js = consumer / "javascript"
                js.mkdir()
                (js / "package.json").write_text(json.dumps({"name": "suzume-consumer", "private": True, "type": "module"}), encoding="utf-8")
                require("npm-consumer-install", ["npm", "install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", str(npm_archive)], cwd=js, timeout=180)
                (js / "consumer.mjs").write_text(CONSUMER_JS, encoding="utf-8")
                (js / "consumer.ts").write_text(CONSUMER_TS, encoding="utf-8")
                shutil.copy2(native_root / "examples/eight-actions.mjson", js / "eight-actions.mjson")
                (js / "mjai-expected.json").write_text(json.dumps(SAMPLE_MJAI_SUMMARY), encoding="utf-8")
                require("npm-consumer-types", ["tsc", "--strict", "--noUncheckedIndexedAccess", "--exactOptionalPropertyTypes", "--noEmit", "--module", "NodeNext", "--target", "ES2022", "--lib", "ES2022,DOM", "consumer.ts"], cwd=js, timeout=120)
                require("npm-consumer-runtime", ["node", "consumer.mjs"], cwd=js, timeout=120,
                        validator=lambda output: {"success": "PACKAGE_CONSUMER_OK node web mjai annotated" in output,
                                                  "detail": "公開SDKの原文往復・注釈破棄報告とNode/Web実WASMの4席解析一致・状態不変を確認"})
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
                    "external_corpus_included": False,
                    "private_path_scan": "distribution-path-checks.json",
                    "consumption": ["crateを固定lock付きbuild", "配布CLIを実行", "配布nativeのmjson・注釈付き原文往復と32観測を照合", "旧replayをmjsonへ変換して結果を照合",
                                    "source bundleをbuild", "source bundle内の文書リンクとコマンド入口を検査"]
                                   + ([] if args.native_only else ["npm tarballをoffline install", "公開TypeScript型を検査",
                                                                  "Nodeとweb用WASMで両形式の4席解析を照合", "公開SDKの原文保持・注釈読書き・明示破棄と件数報告を確認"])}
        write_json(run.directory / "manifest.json", manifest)
        with (run.directory / "SHA256SUMS").open("x", encoding="utf-8", newline="\n") as output:
            for path in products:
                output.write(f"{digest(path)}  {path.name}\n")
        return 0 if run.finish({"environment": report, "manifest": "manifest.json", "sources_unchanged": True,
                               "private_path_scan": "distribution-path-checks.json"}) else 1
    except (OSError, RuntimeError, ValueError, KeyError, StopIteration) as error:
        print(redact(str(error)), flush=True)
        run.finish({"environment": report, "success": False, "error": redact(str(error)),
                    "private_path_scan": "distribution-path-checks.json" if (run.directory / "distribution-path-checks.json").exists() else None,
                    "source_sha256_before": before, "source_sha256_after": source_hashes()})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
