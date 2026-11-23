# 文書案内：麻雀スズメ

## 目的別の案内

| 目的 | 文書 |
|---|---|
| 端末で遊ぶ・対局を保存する | [起動と基本操作](../README.md#端末で遊ぶ)、[遊び方と表示設定](playing.md)、[AI設定とログ](ai/cli.md) |
| ブラウザで動かす | [ブラウザ例の起動方法](../examples/browser/README.md) |
| アプリへ組み込む | [JSON API](api/README.md)、[JavaScript・TypeScript SDK](../packages/suzume/README.md) |
| 席やAIを設定する | [共通設定](api/configuration.md)、[人間1人の設定例](ai/example.toml)、[全員AIの設定例](ai/simulation.toml) |
| 牌譜を記録・解析する | [mjsonの使い方](logging/README.md)、[外部牌譜との互換性](logging/mjai-compatibility.md) |
| 牌譜に解析注釈を付ける | [注釈付きmjai形式](logging/annotated-mjai.md) |
| 旧形式の対局を再生する | [replay形式](logging/legacy-jsonl.md) |
| AIを選ぶ・仕組みを調べる | [AIの種類と設定](ai/README.md)、[AIの仕組みと作り方](ai/algorithms.md)、[評価結果](ai/results.md) |
| AIを追加する・学習器につなぐ | [外部AIの接続](ai/extension.md)、[研究用環境](api/environment.md) |
| 採用ルールを調べる | [ルール仕様](rules/riichi.md)、[機械可読のルール宣言](../rules/riichi.rules) |
| 実装の構成を調べる | [コンセプト](concept/overview.md)、[情報の公開範囲](concept/platform.md)、[責務と依存関係](architecture.md) |
| 入出力の型を確認する | [JSON Schema](../bindings/protocol.schema.json) |
| 開発手順・スキル・文書の書き方を確認する | [開発手順](development/agent-workflow.md)、[同梱スキル](../skills/README.md)、[文章指針](development/writing.md) |
| 構築・検査・配布する | [実行環境とCI](development/reproducibility.md)、[基本契約](contracts/README.md)、[公開APIの境界](contracts/platform-boundaries.md)、[形式検証の範囲](verification/README.md) |

## 設計判断ADR

ADRは、設計を選んだ理由と前提をまとめた文書です。

| 文書 | 対象 |
|---|---|
| [0001](adr/0001-independent-engine.md) | Rustの対局処理を独立させる構成 |
| [0002](adr/0002-declared-rules.md) | ルール宣言と操作の確定 |
| [0003](adr/0003-formal-contracts.md) | Kani・Leanの役割と証明範囲 |
| [0004](adr/0004-platform-boundaries.md) | APIとパッケージの分割 |
| [0005](adr/0005-game-records.md) | 対局の再生・共有・学習用記録 |
| [0006](adr/0006-ai-environment-config.md) | AI、研究用環境、共通設定 |
| [0007](adr/0007-mjai-records.md) | 標準mjsonと旧形式の互換性 |
| [0008](adr/0008-annotated-mjai.md) | 原文と解析注釈を保持する記録形式 |
