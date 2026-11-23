# TOMLとJSONの共通設定

完全な標準例は [config/default.toml](../../config/default.toml)、席別の算法を変える例は [docs/ai/example.toml](../ai/example.toml) です。読込みは `AppConfig::from_toml` へ集約し、TUI、API、再生ログのメタデータが同じ型を使います。TOML 1.0の構文を使用します。

```sh
cargo run --locked -p suzume-api --bin suzume-api -- config config/default.toml
```

このコマンドは省略値を補った構成と、各席の実効AI値を表示します。

## 項目と省略値

| 項目 | 意味と省略時の値 |
|---|---|
| `schema_version` | 必須の整数1 |
| `game.seed` | 山の再現用の十進文字列。省略 `"0"` |
| `game.viewer` | 0〜3または `"public"`。省略0。TOMLにnullがないため明示文字列を使用 |
| `game.max_steps` | 成功操作数の上限1〜10000。省略10000 |
| `players` | 席順のちょうど4要素。省略時は席0が人間、他3席が標準AIでAI種1/2/3 |
| `players[].kind` | `"human"` または `"ai"`。人間席はviewerと一致する必要がある |
| `players[].config` | AIの設定。人間席には指定できない |
| `logging.format` | `"mjai"`（省略時）、`"mjai_public"`、旧形式 `"replay"` / `"public"`。ファイルを開くかどうかはCLIの指定で決める |

不明なキー、重複キー、型の誤り、4席以外、人間席とviewerの矛盾、範囲外、未対応版を拒否します。JSONでは省略項目を同じ値へ解決します。JSONの設定をTOMLへ戻す際、任意項目のnullは省略して表現します。

`mjai` は全席の手牌を含むmjsonイベント、`mjai_public` は伏せ牌を `?` にした公開用mjsonです。
乱数種・内部操作の厳密再生、AI判断説明、189行動IDの学習用遷移には `replay` を明示します。
既存TOMLが `replay` / `public` を明示している場合はその指定を維持します。
CLIの `--log-format` は設定の値を上書きし、拡張子だけでは形式を切り替えません。
詳しくは [mjsonの利用方法](../logging/README.md) と [旧形式の互換仕様](../logging/legacy-jsonl.md) を参照してください。

## AI設定の解決順序

`preset` と `algorithm` の同時指定はエラーです。どちらもなければstandardです。算法だけならrandom→beginner、greedy→standard、ukeire→strongの既定値を起点にします。その後に明示された `max_evaluations`、`lookahead_depth`、`risk_weight` を上書きし、算法ごとの条件を検査します。`seed` は省略 `"0"` で、山のseedとは独立です。

| preset | algorithm | max_evaluations | lookahead_depth | risk_weight |
|---|---|---:|---:|---:|
| beginner | random | 0 | 0 | 0 |
| standard | greedy | 1024 | 0 | 1 |
| strong | ukeire | 8192 | 2 | 4 |

予算は1判断で実行する、キャッシュされていないシャンテン計算の回数です。時間の上限とは異なります。randomは予算・深さ・防御重みをすべて0、greedyは正の予算と深さ0、ukeireは正の予算と深さ1または2を要求します。具体的な上限と探索内容は [算法仕様](../ai/algorithms.md)に記載します。

プリセット名は選択しやすくするための名前で、任意の相手・条件で勝率が単調に増える保証ではありません。[固定比較結果](../ai/results.md)では設定を変更せずに全64東風戦を集計しています。

設定の拡張時は、共通Rust型・検証・Schema生成・TS生成・設定例・不正入力検査を同じ変更で更新してください。任意の文字列を外部プロセスとして実行する設定は提供していません。外部算法はRustのPolicy、または環境APIを呼ぶ独立したプログラムとして接続できます。
