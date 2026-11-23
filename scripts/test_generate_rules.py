#!/usr/bin/env python3
"""宣言の型・算術域・構造を、生成物を書き込む前に検査する回帰テスト。"""
from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("suzume_generator", ROOT / "scripts/generate_rules.py")
GENERATOR = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(GENERATOR)


class DeclarationValidationTests(unittest.TestCase):
    def setUp(self):
        self.declaration = json.loads((ROOT / "rules/contracts.json").read_text())

    def render(self, declaration=None, raw=None):
        with tempfile.TemporaryDirectory(prefix="suzume-declaration-test-") as directory:
            root = Path(directory)
            (root / "rules").mkdir()
            (root / "rules/riichi.rules").write_text((ROOT / "rules/riichi.rules").read_text())
            (root / "rules/contracts.json").write_text(
                raw if raw is not None else json.dumps(declaration or self.declaration)
            )
            return GENERATOR.outputs(root)

    def first_expression(self, expression, arguments=None):
        declaration = copy.deepcopy(self.declaration)
        declaration["contracts"][0]["expr"] = expression
        if arguments is not None:
            declaration["contracts"][0]["args"] = arguments
        return declaration

    def test_accepts_checked_in_declarations(self):
        self.assertEqual(len(self.render()), 4)

    def test_rejects_boolean_arithmetic(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["eq", ["add", True, 1], 2]))

    def test_rejects_non_boolean_contract_result(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["add", "seat", 1]))

    def test_rejects_mixed_integer_types(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["eq", "seat", "result"],
                        [["seat", "u16", "Nat"], ["result", "u64", "Nat"]]))

    def test_rejects_duplicate_json_keys(self):
        raw = json.dumps(self.declaration).replace('"version": 1', '"version": 1, "version": 1', 1)
        with self.assertRaises(ValueError):
            self.render(raw=raw)

    def test_rejects_unknown_contract_fields(self):
        declaration = copy.deepcopy(self.declaration)
        declaration["contracts"][0]["argument_typo"] = []
        with self.assertRaises(ValueError):
            self.render(declaration)

    def test_rejects_duplicate_argument_names(self):
        arguments = [["seat", "u16", "Nat"], ["seat", "u16", "Nat"]]
        with self.assertRaises(ValueError):
            self.render(self.first_expression(True, arguments))

    def test_rejects_reserved_identifiers(self):
        for reserved in ("type", "match", "self", "axiom", "namespace", "gen"):
            with self.subTest(reserved=reserved), self.assertRaises(ValueError):
                self.render(self.first_expression(True, [[reserved, "u16", "Nat"]]))

    def test_rejects_malformed_argument_shape_with_value_error(self):
        for arguments in ([[]], [["seat", "u16"]], ["seat"], "seat"):
            with self.subTest(arguments=arguments), self.assertRaises(ValueError):
                self.render(self.first_expression(True, arguments))

    def test_rejects_invalid_or_duplicate_contract_ids(self):
        for invalid in ("C-01\nfn injected() {}", "C-02"):
            declaration = copy.deepcopy(self.declaration)
            declaration["contracts"][0]["id"] = invalid
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                self.render(declaration)

    def test_rejects_non_integer_version(self):
        for version in (True, 1.0, "1"):
            declaration = copy.deepcopy(self.declaration)
            declaration["version"] = version
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.render(declaration)

    def test_rejects_unbounded_addition(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["eq", ["add", "seat", 1], "result"]))

    def test_rejects_unsigned_subtraction_without_guard(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["eq", ["sub", "seat", 1], 0]))

    def test_accepts_guarded_unsigned_subtraction(self):
        expression = ["and", ["gt", "seat", 0], ["eq", ["sub", "seat", 1], 0]]
        self.assertEqual(len(self.render(self.first_expression(expression))), 4)

    def test_rejects_zero_or_unbounded_divisors(self):
        for divisor in (0, "result"):
            with self.subTest(divisor=divisor), self.assertRaises(ValueError):
                self.render(self.first_expression(["eq", ["div", "seat", divisor], 0]))

    def test_accepts_positive_guarded_divisor(self):
        expression = ["and", ["gt", "result", 0], ["eq", ["div", "seat", "result"], 0]]
        self.assertEqual(len(self.render(self.first_expression(expression))), 4)

    def test_rejects_negative_division_or_remainder(self):
        for operator in ("div", "mod"):
            with self.subTest(operator=operator), self.assertRaises(ValueError):
                self.render(self.first_expression(["eq", [operator, "value", 2], 0],
                            [["value", "i64", "Int"]]))

    def test_rejects_literal_outside_argument_type(self):
        with self.assertRaises(ValueError):
            self.render(self.first_expression(["eq", "seat", 65536]))

    def test_rejects_unknown_root_fields_and_empty_contracts(self):
        for mutation in ({"unknown": 1}, {"contracts": []}):
            declaration = copy.deepcopy(self.declaration)
            declaration.update(mutation)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.render(declaration)


if __name__ == "__main__":
    unittest.main()
