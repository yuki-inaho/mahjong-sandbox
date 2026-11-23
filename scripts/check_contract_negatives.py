#!/usr/bin/env python3
"""一時コピーの誤りを検証器が拒否するか調べる。元の実装・宣言は変更しない。"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LOG = ROOT / "docs/verification/logs/negative-controls.txt"


def main() -> int:
    records = ["Contract negative controls: rejection is the expected result."]
    with tempfile.TemporaryDirectory(prefix="mahjong-contract-negative-") as directory:
        temporary = Path(directory)
        for subdir in ("rules", "src", "proofs/Mahjong"):
            (temporary / subdir).mkdir(parents=True, exist_ok=True)
        for relative in (
            "rules/riichi.rules", "rules/contracts.json", "src/contracts.rs",
            "src/rule_constants.rs", "src/generated_contracts.rs",
            "proofs/Mahjong/GeneratedRules.lean", "proofs/Mahjong/GeneratedContracts.lean",
        ):
            shutil.copy2(ROOT / relative, temporary / relative)

        def run(label: str, command: list[str], cwd: Path, expected: str) -> None:
            result = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=180)
            output = result.stdout.replace(str(temporary), "<temporary-copy>").replace(str(ROOT), "<project>")
            output = output.replace(str(ROOT.parent), "<tool-environment>")
            records.extend([f"\n{label}\nExit: {result.returncode}", output])
            if result.returncode == 0 or expected not in result.stdout:
                raise RuntimeError(f"{label}: 想定した拒否を確認できませんでした")
            print(f"{label}: 想定どおり拒否")

        generator = [sys.executable, str(ROOT / "scripts/generate_rules.py"), "--root", str(temporary), "--check"]
        original_rules = (temporary / "rules/riichi.rules").read_text(encoding="utf-8")
        (temporary / "rules/riichi.rules").write_text(original_rules + "UNKNOWN_RULE = true\n", encoding="utf-8")
        run("N-01 unknown rule", generator, ROOT, "未知")
        (temporary / "rules/riichi.rules").write_text(original_rules.replace("RED_FIVES = false", "RED_FIVES = true"), encoding="utf-8")
        run("N-02 unsupported rule value", generator, ROOT, "未対応")
        (temporary / "rules/riichi.rules").write_text(original_rules, encoding="utf-8")
        generated = temporary / "src/rule_constants.rs"
        generated.write_text(generated.read_text(encoding="utf-8") + "// unintended drift\n", encoding="utf-8")
        run("N-03 generated artifact drift", generator, ROOT, "不一致")
        shutil.copy2(ROOT / "src/rule_constants.rs", generated)

        # 成功時の手番を故意に一つずらします。Kaniが共有宣言との不一致を検出します。
        contract_file = temporary / "src/contracts.rs"
        original = contract_file.read_text(encoding="utf-8")
        if original.count("Some(seat + 1)") != 1:
            raise RuntimeError("変異対象の実装が変わりました。否定検査の対象を見直してください")
        contract_file.write_text(original.replace("Some(seat + 1)", "Some((seat + 2) % 4)"), encoding="utf-8")
        harness = temporary / "mutated.rs"
        harness.write_text(
            '#![allow(dead_code, unexpected_cfgs)]\n'
            '#[path = "src/rule_constants.rs"] mod rule_constants;\n'
            '#[path = "src/contracts.rs"] mod contracts;\n', encoding="utf-8")
        run("N-04 mutated next seat", ["kani", str(harness), "--harness", "next_seat_refines_declaration",
                                      "--output-format", "terse"], temporary, "VERIFICATION:- FAILED")

        false_claim = temporary / "FalseClaim.lean"
        false_claim.write_text(
            'import Mahjong.Contracts\n'
            'example : Mahjong.Model.nextSeat 0 = some 2 := by decide\n', encoding="utf-8")
        run("N-05 false Lean theorem", ["lake", "env", "lean", str(false_claim)], ROOT / "proofs", "is false")
    records.append("\nResult: 5 / 5 expected rejections observed; original project unchanged.")
    LOG.parent.mkdir(parents=True, exist_ok=True)
    LOG.write_text("\n".join(records), encoding="utf-8")
    print("否定検査: 5 / 5 成功")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"否定検査を完了できませんでした: {error}", file=sys.stderr)
        raise SystemExit(1) from error
