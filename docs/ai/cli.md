# CLIでAI設定を使い、対局を記録する

CLIの`--config`は、共通APIの`AppConfig::from_toml`で設定を検査します。
各席の`AiConfigSpec::resolve`が返した実効値から`ConfiguredAi`を作成し、
そのAIが返した選択手と判断根拠を対局へ適用して記録します。
CLI独自のプリセット表やTOML解釈は設けていません。

## 人間1人のTUI

リポジトリのルートから実行します。

```sh
cargo run -p suzume-cli -- --config docs/ai/example.toml --log match.jsonl
```

TUIと`--snapshot`は人間の席0、`viewer = 0`、他の3席AIに対応します。
不一致を起動時に拒否します。`--snapshot`は初期表示だけで終了し、
ログを指定した場合は0操作の中断記録を保存します。

## 全員AIの対局

```sh
cargo run -p suzume-cli -- \
  --config docs/ai/simulation.toml --simulate 1 --log simulation.jsonl
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
| `--log-format replay` | 配牌を復元できる種、完全な設定、実効AI設定、操作、AI判断を記録 |
| `--log-format public` | 配牌・AIの種、伏せ手牌、手牌の添字、AI判断を含めない共有用記録 |
| 形式を省略 | TOMLの`logging.format`を使用。設定も省略した場合は`replay` |
| 通常の東風戦終局 | `completed`、終了コード0 |
| `game.max_steps`到達 | `truncated`、終了コード1。通常終局として数えません |
| `q`、入力の終了、一局demo、初期表示 | `interrupted`、終了コード0 |

共有用ログは情報を省くため対局の再生には使えません。
再生用のログは、配牌を復元できる全情報の記録として扱ってください。
Ctrl+Cなどの強制終了や書込失敗では末尾の記録が欠ける場合があり、
そのファイルを完結したログとして再生しません。

`--log-format`には`--log`が必要です。単一ファイルへ複数試合を混在させないため、
`--simulate N`でNが2以上の場合は`--log`との併用を拒否します。
試合ごとに実行して保存先を分けてください。

`game.max_steps`は全モードで適用し、見送りと次局への操作も数えます。
TOMLはUTF-8、最大1MiBです。`--config`と`--seed`は併用できません。
配牌の種を指定しない場合は全モードで0です。

## 接続を確認した範囲

[CLI結合検査](../../crates/suzume-cli/tests/cli.rs)では、次を実プロセスで確認しています。

- TOMLの配牌の種、操作上限、各席の算法・予算・AIの種が反映されること。
- 記録した40操作を、元のTOMLで判断し直した操作列と照合できること。
- 全員Greedyの1東風戦が通常終局し、そのログを再生できること。
- 上限・中断・一局demoの終了理由が状態と一致し、共有用へ非公開項目が出ないこと。
- 設定の不正値、競合引数、表示席の不一致、大きすぎるファイル、不正UTF-8、既存ログの上書きを拒否すること。

未対応時の実失敗は[先行検査](evidence/platform-cli-before.txt)、
接続後は[成功記録](evidence/platform-cli-after.txt)と[整形・静的解析](evidence/platform-cli-quality.txt)へ保存しています。
二つのTOML構成例も[実行記録](evidence/platform-cli-examples.txt)で確認しています。
設定項目の定義は[共通設定仕様](../api/configuration.md)を参照してください。
