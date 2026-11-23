#!/usr/bin/env python3
"""型付き生成式の具体例を実RustとLeanで照合します。生成器全体の証明ではありません。"""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("suzume_generator", ROOT / "scripts/generate_rules.py")
GENERATOR = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(GENERATOR)

# 条件と期待値を生成器とは独立した具体例として保持します。
CASES = [
    ("constant_signed_subtraction", [], ["eq", ["sub", 0, 1], -1], [((), True)]),
    ("guarded_unsigned_subtraction", [["value", "u16", "Nat"]],
     ["and", ["gt", "value", 0], ["eq", ["sub", "value", 1], 0]],
     [((0,), False), ((1,), True), ((65535,), False)]),
    ("guarded_unsigned_multiplication", [["value", "u16", "Nat"]],
     ["and", ["le", "value", 255], ["eq", ["mul", "value", "value"], 65025]],
     [((0,), False), ((255,), True), ((256,), False)]),
    ("guarded_signed_division", [["value", "i64", "Int"]],
     ["and", ["ge", "value", 0], ["eq", ["div", "value", 2], 2]],
     [((-1,), False), ((0,), False), ((4,), True), ((5,), True)]),
    ("unsigned_remainder", [["value", "u64", "Nat"]],
     ["eq", ["mod", "value", 3], 1],
     [((0,), False), ((1,), True), ((2**64 - 1,), False)]),
    ("or_short_circuit_bound", [["value", "u16", "Nat"]],
     ["or", ["ge", "value", 65535], ["eq", ["add", "value", 1], 2]],
     [((0,), False), ((1,), True), ((65535,), True)]),
    ("boolean_equality", [["value", "bool", "Bool"]],
     ["eq", "value", ["not", ["not", "value"]]], [((False,), True), ((True,), True)]),
]


def value_text(value: int | bool, kind: str | None = None) -> str:
    if type(value) is bool:
        return str(value).lower()
    literal = str(value) + ("_" + kind if kind else "")
    return f"({literal})" if value < 0 else literal


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="suzume-generated-semantics-") as directory:
        temporary = Path(directory)
        (temporary / "rules").mkdir()
        (temporary / "rules/riichi.rules").write_text((ROOT / "rules/riichi.rules").read_text())
        declaration = {
            "version": 1,
            "description": 'Rustの固定raw string区切りも検査します: "###',
            "contracts": [{"id": f"C-{index:02}", "name": name, "args": args, "expr": expression}
                          for index, (name, args, expression, _) in enumerate(CASES, 1)],
        }
        (temporary / "rules/contracts.json").write_text(json.dumps(declaration, ensure_ascii=False))
        rendered = GENERATOR.outputs(temporary)
        for path, content in rendered.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        rust = ['#![allow(dead_code)]', '#[path = "src/generated_contracts.rs"] mod generated;', 'fn main() {']
        lean = ['import Std', rendered[temporary / "proofs/Mahjong/GeneratedRules.lean"],
                rendered[temporary / "proofs/Mahjong/GeneratedContracts.lean"].replace(
                    'import Mahjong.GeneratedRules\n', '')]
        case_count = 0
        for name, args, _, examples in CASES:
            for values, expected in examples:
                rust_args = ', '.join(value_text(value, declaration[1])
                                      for value, declaration in zip(values, args))
                lean_args = ' '.join(value_text(value) for value in values)
                result = value_text(expected)
                rust.append(f'    assert_eq!(generated::{name}({rust_args}), {result}, "{name}");')
                lean.append(f'example : Mahjong.Spec.{name} {lean_args} = {result} := by decide')
                case_count += 1
        rust.append('}')
        rust_path, lean_path = temporary / 'semantics.rs', temporary / 'Semantics.lean'
        rust_path.write_text('\n'.join(rust) + '\n')
        lean_path.write_text('\n'.join(lean) + '\n')
        executable = temporary / 'semantics'
        commands = [(['rustc', '--edition=2021', '-C', 'overflow-checks=yes', str(rust_path), '-o', str(executable)], ROOT),
                    ([str(executable)], ROOT), (['lake', 'env', 'lean', str(lean_path)], ROOT / 'proofs')]
        for command, cwd in commands:
            process = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT, check=False, timeout=60)
            if process.returncode != 0:
                output = process.stdout.replace(str(temporary), '<temporary>').replace(str(ROOT), '<repository>')
                raise RuntimeError(f'{command[0]} exit {process.returncode}:\n{output}')
        print(f'生成式の意味照合: {len(CASES)}関係・{case_count}具体例が実Rust/Lean双方で期待値と一致')
        print('対象: 整数型、符号付き定数、境界、短絡条件、除算・剰余、真偽値、raw string区切り')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f'生成式の意味照合に失敗しました: {error}') from error
