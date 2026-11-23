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
INTEGER_BOUNDS = {"u16": (0, 2**16 - 1), "u64": (0, 2**64 - 1),
                  "i64": (-(2**63), 2**63 - 1)}
RESERVED = set("""as async await break const continue crate dyn else enum extern false fn for
if impl in let loop match mod move mut pub ref return self static struct super trait true type
unsafe use where while abstract become box do final gen macro override priv try typeof unsized
virtual yield axiom def theorem lemma namespace end import open opaque inductive structure
class instance variable variables universe universes example abbrev syntax macro_rules
deriving private protected noncomputable section set_option attribute export local notation
prefix infix postfix forall fun by have show from suffices calc obtain cases simp rfl
termination_by decreasing_by partial unsafe mutual initialize
""".split()) - {"from"}  # 既存の口座引数fromはLean側で常に引用します。
MAX_EXPRESSION_DEPTH = 64
MAX_EXPRESSION_NODES = 4096


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


class Expression:
    """型付けした式木。整数リテラルだけは親の整数型へ具体化します。"""

    def __init__(self, kind: str, op: str, value: object = None,
                 operands: tuple[Expression, ...] = ()):
        self.kind, self.op, self.value, self.operands = kind, op, value, operands


def common_kind(left: str, right: str) -> str:
    if left == right:
        return left
    if left == "literal" and right in INTEGER_BOUNDS:
        return right
    if right == "literal" and left in INTEGER_BOUNDS:
        return left
    raise ValueError(f"式の型が一致しません: {left} と {right}")


def parse_expression(node: object, args: dict[str, str], rules: dict[str, object]) -> Expression:
    visits = 0

    def parse(value: object, depth: int) -> Expression:
        nonlocal visits
        visits += 1
        if depth > MAX_EXPRESSION_DEPTH or visits > MAX_EXPRESSION_NODES:
            raise ValueError("契約式の深さまたは節数の上限を超えました")
        if type(value) is bool:
            return Expression("bool", "boolean", value)
        if type(value) is int:
            return Expression("literal", "integer", value)
        if isinstance(value, str):
            if value not in args:
                raise ValueError(f"未定義の変数: {value}")
            return Expression(args[value], "variable", value)
        if isinstance(value, dict) and set(value) == {"rule"}:
            name = value["rule"]
            if not isinstance(name, str) or name not in rules or isinstance(rules[name], str):
                raise ValueError(f"数式で使えないルール: {name}")
            return parse(rules[name], depth + 1)
        if not isinstance(value, list) or not value or not isinstance(value[0], str):
            raise ValueError("不正な式の構造です")
        op, *raw_operands = value
        if not ((op == "not" and len(raw_operands) == 1)
                or (op in ("and", "or") and len(raw_operands) >= 2)
                or (op in ARITH | COMPARE and len(raw_operands) == 2)):
            raise ValueError(f"未対応の演算子または引数数: {op}")
        operands = tuple(parse(child, depth + 1) for child in raw_operands)
        if op in ("and", "or", "not"):
            if any(child.kind != "bool" for child in operands):
                raise ValueError(f"{op}: 真偽値の式が必要です")
            return Expression("bool", op, operands=operands)
        kind = common_kind(operands[0].kind, operands[1].kind)
        if op in ARITH or op not in ("eq", "ne"):
            if kind not in (*INTEGER_BOUNDS, "literal"):
                raise ValueError(f"{op}: 整数の式が必要です")
        return Expression("bool" if op in COMPARE else kind, op, operands=operands)

    result = parse(node, 0)
    if result.kind != "bool":
        raise ValueError("契約式全体は真偽値である必要があります")
    return result


def numeric_kind(expression: Expression, expected: str | None = None) -> str:
    return expression.kind if expression.kind != "literal" else expected or "i64"


def numeric_bounds(expression: Expression, expected: str,
                   ranges: dict[str, tuple[int, int]]) -> tuple[int, int]:
    kind = numeric_kind(expression, expected)
    if kind not in INTEGER_BOUNDS:
        raise ValueError(f"整数の式が必要です: {kind}")
    if expression.op == "integer":
        value = int(expression.value)
        result = (value, value)
    elif expression.op == "variable":
        result = ranges[str(expression.value)]
    else:
        left = numeric_bounds(expression.operands[0], kind, ranges)
        right = numeric_bounds(expression.operands[1], kind, ranges)
        if expression.op == "add":
            result = (left[0] + right[0], left[1] + right[1])
        elif expression.op == "sub":
            result = (left[0] - right[1], left[1] - right[0])
        elif expression.op == "mul":
            products = [a * b for a in left for b in right]
            result = (min(products), max(products))
        elif expression.op in ("div", "mod"):
            # 非負の被除数・正の除数だけなら、RustとNat/Intの除算・剰余が一致します。
            # 負数の除算を、各言語の既定の丸め規則へ暗黙に委ねません。
            if left[0] < 0 or right[0] <= 0:
                raise ValueError("除算・剰余には非負の被除数と正の除数の事前条件が必要です")
            result = ((left[0] // right[1], left[1] // right[0]) if expression.op == "div"
                      else (0, min(left[1], right[1] - 1)))
        else:
            raise ValueError(f"数値として評価できない演算子: {expression.op}")
    minimum, maximum = INTEGER_BOUNDS[kind]
    if result[0] < minimum or result[1] > maximum:
        raise ValueError(f"{kind}の算術域を保証できません: {expression.op} の範囲 {result}")
    return result


def constant_value(expression: Expression) -> int | None:
    if expression.op == "integer":
        return int(expression.value)
    if expression.op not in ARITH:
        return None
    values = [constant_value(child) for child in expression.operands]
    if any(value is None for value in values):
        return None
    left, right = values
    if expression.op == "add":
        return left + right
    if expression.op == "sub":
        return left - right
    if expression.op == "mul":
        return left * right
    if right <= 0 or left < 0:
        return None
    return left // right if expression.op == "div" else left % right


def refined_ranges(expression: Expression, truth: bool,
                   ranges: dict[str, tuple[int, int]]) -> dict[str, tuple[int, int]]:
    """短絡評価で確実に成立した単純な境界だけを後続の式へ渡します。"""
    result = dict(ranges)
    if expression.op == "not":
        return refined_ranges(expression.operands[0], not truth, result)
    if (expression.op == "and" and truth) or (expression.op == "or" and not truth):
        for child in expression.operands:
            result = refined_ranges(child, truth, result)
        return result
    if expression.op not in COMPARE:
        return result
    left, right = expression.operands
    op = expression.op
    if left.op != "variable" and right.op == "variable":
        left, right = right, left
        op = {"lt": "gt", "le": "ge", "gt": "lt", "ge": "le", "eq": "eq", "ne": "ne"}[op]
    value = constant_value(right)
    if left.op != "variable" or left.kind not in INTEGER_BOUNDS or value is None:
        return result
    if not truth:
        op = {"lt": "ge", "le": "gt", "gt": "le", "ge": "lt", "eq": "ne", "ne": "eq"}[op]
    name = str(left.value)
    lower, upper = result[name]
    if op == "eq":
        lower, upper = max(lower, value), min(upper, value)
    elif op == "lt":
        upper = min(upper, value - 1)
    elif op == "le":
        upper = min(upper, value)
    elif op == "gt":
        lower = max(lower, value + 1)
    elif op == "ge":
        lower = max(lower, value)
    elif op == "ne":
        if value == lower:
            lower += 1
        elif value == upper:
            upper -= 1
    result[name] = (lower, upper)
    return result


def validate_arithmetic(expression: Expression, ranges: dict[str, tuple[int, int]]) -> None:
    if any(lower > upper for lower, upper in ranges.values()):
        return  # 検査済みの先行条件が矛盾し、この枝へ到達するRust入力はありません。
    if expression.op in ("and", "or"):
        current = dict(ranges)
        for child in expression.operands:
            validate_arithmetic(child, current)
            current = refined_ranges(child, expression.op == "and", current)
    elif expression.op == "not":
        validate_arithmetic(expression.operands[0], ranges)
    elif expression.op in COMPARE:
        kind = common_kind(*(child.kind for child in expression.operands))
        for child in expression.operands:
            if kind == "bool":
                validate_arithmetic(child, ranges)
            else:
                numeric_bounds(child, "i64" if kind == "literal" else kind, ranges)


def render_expression(expression: Expression, language: str, expected: str | None = None) -> str:
    if expression.op == "boolean":
        return literal(expression.value)
    if expression.op == "integer":
        kind = numeric_kind(expression, expected)
        if language == "lean":
            # 変数を含まない式もRust側と同じ型にします。注釈がないとLeanがNatを
            # 選び、0 - 1などでIntを使うRust側と意味が異なることがあります。
            lean_kind = "Int" if kind == "i64" else "Nat"
            return f"({literal(expression.value)} : {lean_kind})"
        value = int(expression.value)
        return f"{value}_{kind}" if value >= 0 else f"({value}_{kind})"
    if expression.op == "variable":
        return f"«{expression.value}»" if language == "lean" else str(expression.value)
    op = expression.op
    if op == "not":
        return "(!" + render_expression(expression.operands[0], language, "bool") + ")"
    if op in ("and", "or"):
        joiner = " && " if op == "and" else " || "
        return "(" + joiner.join(render_expression(child, language, "bool")
                                 for child in expression.operands) + ")"
    kind = common_kind(*(child.kind for child in expression.operands))
    if kind == "literal":
        kind = expected if expected in INTEGER_BOUNDS else "i64"
    left, right = (render_expression(child, language, kind) for child in expression.operands)
    if op in ARITH:
        return f"({left} {ARITH[op]} {right})"
    return (f"({left} {COMPARE[op]} {right})" if language == "rust"
            else f"decide ({left} {LEAN_COMPARE[op]} {right})")


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"重複したJSON項目: {key}")
        result[key] = value
    return result


def fields(value: object, required: set[str], optional: set[str] | None = None) -> dict:
    if not isinstance(value, dict):
        raise ValueError("JSON objectが必要です")
    unknown = value.keys() - required - (optional or set())
    missing = required - value.keys()
    if unknown or missing:
        raise ValueError(f"不正なJSON項目: 未知={sorted(unknown)}, 不足={sorted(missing)}")
    return value


def identifier(value: object) -> bool:
    return isinstance(value, str) and bool(re.fullmatch(r"[a-z][a-z0-9_]*", value)) and value not in RESERVED


def raw_rust_string(value: str) -> str:
    # 宣言中のコメント/説明に区切り文字があっても、Rustソースを壊しません。
    hashes = "###"
    while '"' + hashes in value:
        hashes += "#"
    return 'r' + hashes + '"' + value + '"' + hashes


def outputs(root: Path) -> dict[Path, str]:
    rules_source = (root / "rules/riichi.rules").read_text(encoding="utf-8")
    contracts_source = (root / "rules/contracts.json").read_text(encoding="utf-8")
    rules = parse_rules(rules_source)
    try:
        contracts = fields(json.loads(contracts_source, object_pairs_hook=unique_object),
                           {"version", "contracts"}, {"description"})
    except RecursionError as error:
        raise ValueError("契約宣言の入れ子が深すぎます") from error
    if type(contracts["version"]) is not int or contracts["version"] != 1:
        raise ValueError("未対応の契約宣言版です")
    if "description" in contracts and not isinstance(contracts["description"], str):
        raise ValueError("契約宣言のdescriptionは文字列です")
    if not isinstance(contracts["contracts"], list) or not contracts["contracts"]:
        raise ValueError("contractsは空ではない配列である必要があります")
    digest = hashlib.sha256((rules_source + "\0" + contracts_source).encode()).hexdigest()
    rust_constants = ["// @generated by scripts/generate_rules.py; do not edit.",
                      f"pub const DECLARATION_SHA256: &str =\n    \"{digest}\";"]
    lean_constants = ["-- @generated by scripts/generate_rules.py; do not edit.", "namespace Mahjong.Rules"]
    for key, (kind, _) in SCHEMA.items():
        rust_constants.append(f"pub const {key}: {kind} = {literal(rules[key])};")
        lean_kind = "String" if kind == "&str" else "Bool" if kind == "bool" else "Nat"
        lean_constants.append(f"def {key} : {lean_kind} := {literal(rules[key])}")
    rust_constants.extend(["#[rustfmt::skip]", "pub const RULE_SOURCE: &str = " + raw_rust_string(rules_source) + ';'])
    lean_constants.append("end Mahjong.Rules")
    rust_contracts = ["// @generated by scripts/generate_rules.py; do not edit.",
                      "// Inputs originate from u8/i32/usize contracts; widened arithmetic models mathematical integers.",
                      f"pub const DECLARATION_SHA256: &str =\n    \"{digest}\";",
                      "#[rustfmt::skip]", "pub const CONTRACT_SOURCE: &str = " + raw_rust_string(contracts_source) + ';']
    lean_contracts = ["-- @generated by scripts/generate_rules.py; do not edit.",
                      "import Mahjong.GeneratedRules", "namespace Mahjong.Spec"]
    names: set[str] = set()
    ids: set[str] = set()
    for contract in contracts["contracts"]:
        contract = fields(contract, {"id", "name", "args", "expr"})
        name = contract["name"]
        if not identifier(name) or name in names:
            raise ValueError(f"不正または重複した契約名: {name}")
        names.add(name)
        contract_id = contract["id"]
        if (not isinstance(contract_id, str) or not re.fullmatch(r"C-[0-9]{2}[A-Z]?", contract_id)
                or contract_id in ids):
            raise ValueError(f"不正または重複した契約ID: {contract_id}")
        ids.add(contract_id)
        arguments = contract["args"]
        if not isinstance(arguments, list) or any(
                not isinstance(arg, list) or len(arg) != 3 or not identifier(arg[0])
                or (arg[1], arg[2]) not in (("u16", "Nat"), ("u64", "Nat"), ("i64", "Int"), ("bool", "Bool"))
                for arg in arguments):
            raise ValueError(f"{name}: 不正な引数宣言")
        if len({arg[0] for arg in arguments}) != len(arguments):
            raise ValueError(f"{name}: 重複した引数名")
        args = {a[0]: a[1] for a in arguments}
        expression = parse_expression(contract["expr"], args, rules)
        validate_arithmetic(expression, {arg: INTEGER_BOUNDS[kind] for arg, kind in args.items()
                                         if kind in INTEGER_BOUNDS})
        rust_args = ", ".join(f"{a[0]}: {a[1]}" for a in arguments)
        lean_args = " ".join(f"(«{a[0]}» : {a[2]})" for a in arguments)
        rust_contracts.extend([f"/// {contract['id']}: declarative relation, generated from rules/contracts.json.",
                               "// Keep the declaration's operator tree visible in both Rust and Lean.",
                               "#[rustfmt::skip]",
                               "#[allow(unused_parens, clippy::nonminimal_bool, clippy::manual_range_contains)]",
                               f"pub fn {name}({rust_args}) -> bool {{",
                               "    " + render_expression(expression, "rust"), "}"])
        lean_contracts.extend([f"/-- {contract['id']}: 共通の宣言式。 -/",
                               f"def {name} {lean_args} : Bool :=",
                               "  " + render_expression(expression, "lean")])
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
