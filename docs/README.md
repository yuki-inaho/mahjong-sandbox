# 文書案内：すずめ 0.2.0

端末で遊ぶ、アプリに組み込む、対局を分析する、AIを追加する、規則と実装を検証するための入口です。
現行の作業の正本は[mjson移行作業書](workdocs/workdoc_Nov23-2026_mjson.md)です。
0.1系の検査結果と作業書は、当時の対象を示す履歴として残しています。

## 使う目的から読む

| 目的 | 入口 |
|---|---|
| 端末で遊ぶ | [READMEの起動と操作](../README.md#端末で遊ぶ)、[AI設定とログ](ai/cli.md) |
| 画面や別のアプリへ組み込む | [API版1](api/README.md)、[JS/TS SDK](../packages/suzume/README.md)、[ブラウザ例](../examples/browser/README.md) |
| TOMLで席と算法を設定する | [共通設定](api/configuration.md)、[人間1人の例](ai/example.toml)、[全員AIの例](ai/simulation.toml) |
| 感想戦・外部牌譜解析・観測出力 | [mjsonログ仕様](logging/README.md)、[外部形式と実牌譜検査](logging/mjai-compatibility.md)、[自作8操作のmjson例](../examples/eight-actions.mjson) |
| 解析注釈の保存・標準mjsonへの往復 | [注釈付きmjai仕様](logging/annotated-mjai.md)、[SDKの保存入口](../packages/suzume/README.md#原文を保持して注釈を付ける) |
| 同じコアでの厳密再生・AI判断保存 | [旧replayの互換仕様](logging/legacy-jsonl.md)、[旧8操作の再生例](../examples/replay-eight-actions.jsonl) |
| 木探索・強化学習を試す | [189行動の環境](api/environment.md)、[算法追加の接続点](ai/extension.md) |
| AIの判断と評価を調べる | [算法・予算](ai/algorithms.md)、[事前固定した評価条件](ai/evaluation.md)、[全64東風戦の結果](ai/results.md) |
| 同じ環境で構築・検査・配布する | [再現手順](development/reproducibility.md) |

## 規則・コンセプト・構成

- [コンセプト](concept/overview.md)：利用者、体験、採用範囲、参考資料。
- [対局・画面・分析・AI開発の共有基盤](concept/platform.md)：内部状態、公開観測、完全記録を分ける理由。
- [ルール仕様](rules/riichi.md)：現行の固定ルール。AIの実行設定とは別です。
- [責務と依存関係](architecture.md)：core/API/CLI/WASM/SDKの境界と操作の確定。
- [共通契約](contracts/README.md)と[0.2の公開境界・生成仕様](contracts/platform-boundaries.md)。
- [機械可読Schema](../bindings/protocol.schema.json)：要求・応答・設定・ログ・解析用遷移の型を同じ生成物へ収録。

## 設計判断ADR

| ADR | 扱う判断 |
|---|---|
| [0001](adr/0001-independent-engine.md) | 独立したRustの対局処理 |
| [0002](adr/0002-declared-rules.md) | 宣言した規則と原子的な操作 |
| [0003](adr/0003-formal-contracts.md) | 契約、Kani、Leanの役割と証明範囲 |
| [0004](adr/0004-platform-boundaries.md) | 公開APIとパッケージの境界 |
| [0005](adr/0005-game-records.md) | 完全再生ログ、共有記録、解析用遷移 |
| [0006](adr/0006-ai-environment-config.md) | AI算法、研究環境、共通設定 |
| [0007](adr/0007-mjai-records.md) | mjsonへの既定出力移行、外部解析、情報制限、旧形式との互換性 |
| [0008](adr/0008-annotated-mjai.md) | 注釈付き記録、原文を保持する往復、数値・注釈・集計の境界 |

## 開発と検査の根拠

[作業手順](development/workflow.md)は、作業書の作成、独立レビュー、実装、実測、検収の対応を説明します。
現行の[作業書](workdocs/workdoc_Nov23-2026_mjai_annotated.md)、[着手時レビュー](reviews/annotated-baseline-review.md)、
[作業書の独立レビュー](workdocs/review_Nov23-2026_mjai_annotated.md)を入口にしてください。
[前回のmjson作業書](workdocs/workdoc_Nov23-2026_mjson.md)、[独立実装レビュー](reviews/mjson-final-review.md)、[mjson検証結果](verification/mjson-results.md)は移行時の記録です。
従前の[基盤整備作業書](workdocs/workdoc_Nov23-2026_platform.md)、[作業書レビュー](workdocs/review_Nov23-2026_platform.md)、
[実装監査](reviews/platform-review.md)は当時の要求と結果を保持しています。

全体検査は`python3 scripts/verify_platform.py --profile full`、配布物の作成と消費検査は
`python3 scripts/package.py`です。いずれもリポジトリルートで実行します。
外部牌譜の固定入力に対する検査は `python3 scripts/verify_mjai_real.py` です。
配布検査には自作mjson例のnative解析・旧形式からの変換・公開SDKと実WASMによる解析を含めます。
前提と成果物の場所は[再現手順](development/reproducibility.md)にあります。
文書の相対リンク、見出し、画像、リポジトリ内のコマンド入口は`python3 scripts/check_documentation.py`で確認できます。
外部URLの疎通や、掲載コマンドすべての実行成功をこの静的検査だけで保証するものではありません。

[形式検証の範囲](verification/README.md)には、任意入力の対象、展開の上限、Leanの前提と公理、
通常テストで補う範囲を記載します。AIの実測値と、形式的に証明した性質は別に評価します。

## 初版の履歴

- [初版の作業書](workdocs/workdoc_Nov23-2026_suzume.md)と[作業書レビュー](workdocs/review_Nov23-2026_suzume.md)。
- [初版の検証結果](verification/results.md)と[検査名の対応](verification/test-mapping.md)。
- [初版で確認したOS・Rust版](verification/portability.md)。現0.2.0全体の結果へ読み替えません。
- [初版の完了条件](verification/completion-scope.md)と[受入条件](verification/completion-acceptance.md)。

履歴のログを新しい検査で上書きせず、対象の版と実行IDを区別します。

## 0.2.0の基盤整備時の検収記録

- [実行・配布・連携の検証結果](verification/platform-results.md)
- [DoDと実テストの対応表](verification/platform-test-mapping.md)
