#!/usr/bin/env python3
"""固定版のwasm-bindgenでブラウザ用ESMとNode用CommonJSを生成します。"""

import json

from platform_support import ROOT, SDK, Run, environment_report, release_environment


def complete_typescript_libraries(directory) -> None:
    # wasm-bindgenが生成するSymbol.disposeの型を、ES2022の消費側でも解決します。
    # SDK内部のtsconfigだけを緩めず、配布する宣言自身に必要な標準型を記します。
    reference = '/// <reference lib="esnext.disposable" />\n'
    for path in directory.glob("*.d.ts"):
        content = path.read_text(encoding="utf-8")
        if not content.startswith(reference):
            path.write_text(reference + content, encoding="utf-8")


def main() -> int:
    run = Run("wasm-build")
    report = environment_report("frontend")
    if not report["success"]:
        print("frontendの道具が不足または指定版と異なります。scripts/doctor.pyで確認してください")
        run.finish({"environment": report, "success": False})
        return 1
    result = run.execute("compile", ["cargo", "build", "--locked", "--release", "--target-dir", str(ROOT / "target"), "--target",
                                     "wasm32-unknown-unknown", "-p", "suzume-wasm"], timeout=900,
                         env=release_environment())
    if result["success"]:
        artifact = ROOT / "target" / "wasm32-unknown-unknown" / "release" / "suzume_wasm.wasm"
        for target, name in (("web", "web"), ("nodejs", "node")):
            destination = SDK / "wasm" / name
            run.execute(name, ["wasm-bindgen", str(artifact), "--target", target, "--out-name", "suzume_wasm",
                               "--out-dir", str(destination), "--typescript"], timeout=120)
            if run.results[-1]["success"]:
                complete_typescript_libraries(destination)
            if name == "node" and run.results[-1]["success"]:
                (destination / "package.json").write_text(json.dumps({"type": "commonjs"}) + "\n", encoding="utf-8")
    return 0 if run.finish({"environment": report}) else 1


if __name__ == "__main__":
    raise SystemExit(main())
