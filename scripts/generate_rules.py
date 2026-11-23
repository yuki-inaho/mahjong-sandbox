#!/usr/bin/env python3
"""宣言からRust/Leanの定数と契約式を生成する依存ゼロの検査可能な変換器。

生成器の正しさ自体をLeanで証明したものではありません。対応表で信頼境界を
明示し、--checkで生成物の改変・宣言とのずれを拒否します。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
# 対応済みプロファイルの型と値。設定可能であるかのように未対応値を受理しません。
SCHEMA = {
    "PROFILE_NAME": ("&str", "suzume-riichi-east-v1"),
    "PLAYER_COUNT": ("usize", 4), "TILE_KINDS": ("usize", 34),
    "COPIES_PER_TILE": ("u8", 4), "INITIAL_POINTS": ("i32", 25000),
    "RIICHI_STICK": ("i32", 1000), "RIICHI_MIN_WALL": ("usize", 4),
    "DEAD_WALL": ("usize", 14), "MAX_KANS": ("usize", 4),
    "ROUND_COUNT": ("usize", 4), "OPEN_TANYAO": ("bool", True),
    "RED_FIVES": ("bool", False), "MULTIPLE_RON": ("bool", False),
    "ALLOW_CHI": ("bool", True), "ALLOW_PON": ("bool", True),
    "ALLOW_OPEN_KAN": ("bool", True), "ALLOW_CLOSED_KAN": ("bool", True),
    "ALLOW_ADDED_KAN": ("bool", False), "ALLOW_KAN_AFTER_RIICHI": ("bool", False),
    "ALLOW_KUIKAE": ("bool", False), "ABORTIVE_DRAWS": ("bool", False),
    "NAGASHI_MANGAN": ("bool", False), "DOUBLE_WIND_PAIR_FU": ("u16", 4),
    "KIRIAGE_MANGAN": ("bool", False), "COUNTED_YAKUMAN": ("bool", True),
    "COMBINED_YAKUMAN": ("bool", True), "DOUBLE_YAKUMAN": ("bool", False),
    "HEAVENLY_HAND": ("bool", False), "EARTHLY_HAND": ("bool", False),
}
ARITH = {"add": "+", "sub": "-", "mul": "*", "div": "/", "mod": "%"}
COMPARE = {"eq": "==", "ne": "!=", "lt": "<", "le": "<=", "gt": ">", "ge": ">="}
LEAN_COMPARE = {"eq": "=", "ne": "≠", "lt": "<", "le": "≤", "gt": ">", "ge": "≥"}


def parse_rules(source: str) -> dict[str, object]:
    values: dict[str, object] = {}
    for number, raw in enumerate(source.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if "=" not in line:
            raise ValueError(f"riichi.rules:{number}: key = value が必要です")
        key, value = (part.strip() for part in line.split("=", 1))
        if key not in SCHEMA or key in values:
            raise ValueError(f"riichi.rules:{number}: 未知または重複した項目 {key}")
        kind, expected = SCHEMA[key]
        if kind == "&str":
            parsed: object = value
        elif kind == "bool":
            if value not in ("true", "false"):
                raise ValueError(f"{key}: 真偽値は true / false です")
            parsed = value == "true"
        else:
            if not re.fullmatch(r"[0-9]+", value):
                raise ValueError(f"{key}: 非負整数が必要です")
            parsed = int(value)
        if parsed != expected:
            raise ValueError(f"{key}: この版では未対応の値 {value}")
        values[key] = parsed
    missing = SCHEMA.keys() - values.keys()
    if missing:
        raise ValueError("不足している項目: " + ", ".join(sorted(missing)))
    return values


def literal(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    return str(value) if int(value) >= 0 else f"({value})"


def expression(node: object, language: str, args: dict[str, str], rules: dict[str, object]) -> str:
    if isinstance(node, (int, bool)):
        return literal(node)
    if isinstance(node, str):
        if node not in args:
            raise ValueError(f"未定義の変数: {node}")
        return f"«{node}»" if language == "lean" and node in {"from", "to", "match", "if"} else node
    if isinstance(node, dict) and set(node) == {"rule"}:
        name = node["rule"]
        if name not in rules or isinstance(rules[name], str):
            raise ValueError(f"数式で使えないルール: {name}")
        return literal(rules[name])
    if not isinstance(node, list) or not node:
        raise ValueError(f"不正な式: {node!r}")
    op, *operands = node
    if op == "not" and len(operands) == 1:
        return "(!" + expression(operands[0], language, args, rules) + ")"
    if op in ("and", "or") and len(operands) >= 2:
        joiner = " && " if op == "and" else " || "
        return "(" + joiner.join(expression(x, language, args, rules) for x in operands) + ")"
    if op in ARITH and len(operands) == 2:
        left, right = (expression(x, language, args, rules) for x in operands)
        return f"({left} {ARITH[op]} {right})"
    if op in COMPARE and len(operands) == 2:
        left, right = (expression(x, language, args, rules) for x in operands)
        if language == "rust":
            return f"({left} {COMPARE[op]} {right})"
        return f"decide ({left} {LEAN_COMPARE[op]} {right})"
    raise ValueError(f"未対応の演算子または引数数: {op}")


def outputs(root: Path) -> dict[Path, str]:
    rules_source = (root / "rules/riichi.rules").read_text(encoding="utf-8")
    contracts_source = (root / "rules/contracts.json").read_text(encoding="utf-8")
    rules = parse_rules(rules_source)
    contracts = json.loads(contracts_source)
    if contracts.get("version") != 1:
        raise ValueError("未対応の契約宣言版です")
    digest = hashlib.sha256((rules_source + "\0" + contracts_source).encode()).hexdigest()
    rust_constants = ["// @generated by scripts/generate_rules.py; do not edit.",
                      f"pub const DECLARATION_SHA256: &str =\n    \"{digest}\";"]
    lean_constants = ["-- @generated by scripts/generate_rules.py; do not edit.", "namespace Mahjong.Rules"]
    for key, (kind, _) in SCHEMA.items():
        rust_constants.append(f"pub const {key}: {kind} = {literal(rules[key])};")
        lean_kind = "String" if kind == "&str" else "Bool" if kind == "bool" else "Nat"
        lean_constants.append(f"def {key} : {lean_kind} := {literal(rules[key])}")
    rust_constants.extend(["#[rustfmt::skip]", "pub const RULE_SOURCE: &str = r###\"" + rules_source + '\"###;'])
    lean_constants.append("end Mahjong.Rules")
    rust_contracts = ["// @generated by scripts/generate_rules.py; do not edit.",
                      "// Inputs originate from u8/i32/usize contracts; widened arithmetic models mathematical integers.",
                      f"pub const DECLARATION_SHA256: &str =\n    \"{digest}\";",
                      "#[rustfmt::skip]", "pub const CONTRACT_SOURCE: &str = r###\"" + contracts_source + '\"###;']
    lean_contracts = ["-- @generated by scripts/generate_rules.py; do not edit.",
                      "import Mahjong.GeneratedRules", "namespace Mahjong.Spec"]
    names: set[str] = set()
    for contract in contracts["contracts"]:
        name = contract["name"]
        if not re.fullmatch(r"[a-z][a-z0-9_]*", name) or name in names:
            raise ValueError(f"不正または重複した契約名: {name}")
        names.add(name)
        arguments = contract["args"]
        if any(not re.fullmatch(r"[a-z][a-z0-9_]*", x[0]) or (x[1], x[2]) not in
               (("u16", "Nat"), ("u64", "Nat"), ("i64", "Int"), ("bool", "Bool")) for x in arguments):
            raise ValueError(f"{name}: 不正な引数宣言")
        args = {a[0]: a[1] for a in arguments}
        rust_args = ", ".join(f"{a[0]}: {a[1]}" for a in arguments)
        lean_args = " ".join(f"(«{a[0]}» : {a[2]})" for a in arguments)
        rust_contracts.extend([f"/// {contract['id']}: declarative relation, generated from rules/contracts.json.",
                               "// Keep the declaration's operator tree visible in both Rust and Lean.",
                               "#[rustfmt::skip]",
                               "#[allow(unused_parens, clippy::nonminimal_bool, clippy::manual_range_contains)]",
                               f"pub fn {name}({rust_args}) -> bool {{",
                               "    " + expression(contract["expr"], "rust", args, rules), "}"])
        lean_contracts.extend([f"/-- {contract['id']}: 共通の宣言式。 -/",
                               f"def {name} {lean_args} : Bool :=",
                               "  " + expression(contract["expr"], "lean", args, rules)])
    lean_contracts.append("end Mahjong.Spec")
    return {
        root / "src/rule_constants.rs": "\n".join(rust_constants) + "\n",
        root / "src/generated_contracts.rs": "\n".join(rust_contracts) + "\n",
        root / "proofs/Mahjong/GeneratedRules.lean": "\n".join(lean_constants) + "\n",
        root / "proofs/Mahjong/GeneratedContracts.lean": "\n".join(lean_contracts) + "\n",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="変更せず、生成物との不一致を失敗にする")
    parser.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        rendered = outputs(args.root)
        mismatches = []
        for path, content in rendered.items():
            if args.check:
                if not path.is_file() or path.read_text(encoding="utf-8") != content:
                    mismatches.append(str(path.relative_to(args.root)))
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
        if mismatches:
            print("宣言と生成物が不一致: " + ", ".join(mismatches), file=sys.stderr)
            return 1
        print(f"ルールと契約の{'同期検査' if args.check else '生成'}: {len(rendered)} ファイル成功")
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"ルール宣言を拒否しました: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
