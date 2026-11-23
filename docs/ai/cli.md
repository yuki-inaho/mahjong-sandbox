# CLIでAI設定を使い、対局を記録する

CLIの`--config`は、共通APIの`AppConfig::from_toml`で設定を検査します。
各席の`AiConfigSpec::resolve`が返した実効値から`ConfiguredAi`を作成し、
そのAIが返した選択手を対局へ適用します。mjsonは対局イベントを記録し、
判断根拠まで保存する場合は `replay` 形式を明示します。
CLI独自のプリセット表やTOML解釈は設けていません。

## 人間1人のTUI

リポジトリのルートから実行します。

```sh
cargo run --locked -p suzume-cli -- --config docs/ai/example.toml --log match.mjson --log-format mjai
```

TUIと`--snapshot`は人間の席0、`viewer = 0`、他の3席AIに対応します。
不一致を起動時に拒否します。`--snapshot`は初期表示だけで終了し、
ログを指定した場合は0操作の中断記録を保存します。

## 全員AIの対局

```sh
cargo run --locked -p suzume-cli -- \
  --config docs/ai/simulation.toml --simulate 1 --log simulation.mjson --log-format mjai
```

[simulation.toml](simulation.toml)では4席に異なる算法・予算・種を設定しています。
この構成例の小さい計算予算は、[固定評価](evaluation.md)の候補設定とは異なります。

`--demo`は同じ全員AI設定で一局を表示して終了します。
`viewer = 0`では席0の手牌を表示し、`viewer = "public"`では全員の手牌を伏せます。
demoは席1〜3を固定表示する設定に対応せず、起動時に拒否します。
`--simulate`には表示席の制約がありません。

設定を明示した`--simulate`と`--demo`は全員AIを要求します。
設定を省略すると、この二つのモードは全員Greedy・AIの種0になります。
`--simulate N`は設定の配牌の種からN個の連続する種を使い、AI設定は各試合で維持します。

## 記録形式と終了理由

| 指定・終了状況 | 動作 |
|---|---|
| `--log PATH` | 新規ファイルだけを作成。既存ファイルを上書きしません |
| `--log-format mjai` | 配牌・ツモ・打牌・採用された副露・和了などをmjsonイベントで記録 |
| `--log-format mjai_public` | 配牌とツモの非公開牌を `?` にした共有用mjson |
| `--log-format replay` | 旧形式。配牌を復元できる種、完全な設定、実効AI設定、操作、AI判断を記録 |
| `--log-format public` | 旧形式。配牌・AIの種、伏せ手牌、手牌の添字、AI判断を含めない共有用記録 |
| 形式を省略 | TOMLの`logging.format`を使用。項目も省略した場合は`mjai` |
| 通常の東風戦終局 | `completed`、終了コード0 |
| `game.max_steps`到達 | `truncated`、終了コード1。通常終局として数えません |
| `q`、入力の終了、一局demo、初期表示 | `interrupted`、終了コード0 |

共有用ログは情報を省くため、他家の伏せ手牌や完全な対局を復元できません。
分析用の `mjai` と厳密再生用の `replay` は、非公開情報を含む記録として扱ってください。
上のコマンド例は `--log-format mjai` で設定値を上書きしています。
従来のTOML例はAI判断検査のため `replay` を明示しており、その指定を省略値で変更しません。
Ctrl+Cなどの強制終了や書込失敗では末尾の記録が欠ける場合があり、
そのファイルを完結したログとして再生しません。

`--log-format`には`--log`が必要です。単一ファイルへ複数試合を混在させないため、
`--simulate N`でNが2以上の場合は`--log`との併用を拒否します。
試合ごとに実行して保存先を分けてください。

`game.max_steps`は全モードで適用し、見送りと次局への操作も数えます。
TOMLはUTF-8、最大1MiBです。`--config`と`--seed`は併用できません。
配牌の種を指定しない場合は全モードで0です。

## 保存後の分析

```sh
cargo run --locked -p suzume-api --bin suzume-api -- mjai analyze examples/eight-actions.mjson
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai observations examples/eight-actions.mjson --seat 0 --output observations.jsonl
```

CLIはgzip圧縮された `.mjson` も読めます。視点別観測には自分の手牌と公開情報を含み、
他家の手牌や元イベント全文を含めません。外部牌譜の観測から合法手マスクは生成しません。
旧replayからの変換、終了の扱い、SDKからの利用は [対局ログの入口](../logging/README.md) を参照してください。

## 接続を確認した範囲

[CLI結合検査](../../crates/suzume-cli/tests/cli.rs)では、次を実プロセスで確認しています。

- TOMLの配牌の種、操作上限、各席の算法・予算・AIの種が反映されること。
- 明示した旧 `replay` の40操作を、元のTOMLで判断し直した操作列と照合できること。
- 全員Greedyの1東風戦が通常終局し、旧 `replay` を厳密再生できること。
- 上限・中断・一局demoの終了理由が状態と一致し、共有用へ非公開項目が出ないこと。
- 設定の不正値、競合引数、表示席の不一致、大きすぎるファイル、不正UTF-8、既存ログの上書きを拒否すること。

従前の基盤整備時の未対応による実失敗は[先行検査](evidence/platform-cli-before.txt)、
接続後は[成功記録](evidence/platform-cli-after.txt)と[整形・静的解析](evidence/platform-cli-quality.txt)へ保存しています。
二つのTOML構成例も[実行記録](evidence/platform-cli-examples.txt)で確認しています。
設定項目の定義は[共通設定仕様](../api/configuration.md)を参照してください。
mjsonの解析・保存・実牌譜の検査結果は[検証結果](../verification/mjson-results.md)を参照してください。
