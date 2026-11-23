# 文書案内

## 何を作るか

- [コンセプト](concept/overview.md)：利用者、対局体験、初版の範囲。
- [ルール](rules/riichi.md)：採用規則、非対応規則、設定の意味。
- [構成](architecture.md)：責務、依存方向、操作の適用経路。

## なぜこの構成にしたか

- [ADR 0001：独立したRust対局処理](adr/0001-independent-engine.md)
- [ADR 0002：宣言した規則と原子的な操作](adr/0002-declared-rules.md)
- [ADR 0003：契約と形式検証](adr/0003-formal-contracts.md)

## どのように確認するか

- [作業手順とスキルの出典](development/workflow.md)
- [作業計画書兼記録書](workdocs/workdoc_Nov23-2026_suzume.md)
- [作業書レビュー](workdocs/review_Nov23-2026_suzume.md)
- `contracts/`：事前条件・事後条件・不変条件と実装の対応。
- `verification/`：Kani、Lean、通常テストの対象と実行結果。

起動と操作はルートの [README](../README.md) を参照してください。
