# アプリケーションAPI（版1）

`suzume-api::Engine` はローカルの対局を所有し、JSON要求を受けてJSON応答を返します。Rust、標準入出力、WASMで同じ処理を使用します。通信、認証、複数利用者の管理は呼出し側の責務です。麻雀規則は[固定ルール](../rules/riichi.md)、責務の分離は[ADR-0004](../adr/0004-platform-boundaries.md)に定めます。

型の正本は `crates/suzume-api/src`、機械可読な契約は `bindings/protocol.schema.json`、TypeScript型は `packages/suzume/src/generated/types.ts` です。RustからSchema、SchemaからTypeScriptと実行時検査用データを生成します。手動で同じ列挙や範囲を書き直しません。

## 呼出しと版

要求はUTF-8のJSONオブジェクト、上限は1,048,576バイトです。`version` は必須で整数 `1` だけを受け付けます。未知の項目、不明な操作、重複したJSONキー、型や範囲の誤りを拒否します。正常応答は `{"status":"ok","version":1,"data":...}`、失敗は `{"status":"error","version":1,"code":...,"message":...,"session_id":...,"revision":...}` です。

`message` は人間向けの説明です。分岐には `code` を使用します。`invalid_request`、`invalid_config`、`no_game`、`stale_session`、`stale_revision`、`wrong_actor`、`illegal_action`、`invalid_ai`、`step_limit`、`wrong_mode`、`internal_error`、`log_error` を定義します。不正要求で対局や操作履歴は変わりません。

SchemaとSDKは構造・列挙・基本的な整数範囲を検査します。算法と予算の組合せ、閲覧席と人間席の対応、現在の局面での合法性はEngineが追加検査します。Schemaに適合するだけで、任意の対局操作が合法になるわけではありません。生のJSONに数値を記す場合は通常の整数表記を用い、小数・指数・負のゼロは使用しません。

| 値 | 範囲・意味 |
|---|---|
| `SeatId` | 0〜3。席番号は対局中固定。東南西北の自風は親の移動により変わる |
| `TileId` | 0〜8=一〜九萬、9〜17=一〜九筒、18〜26=一〜九索、27〜33=東南西北白發中 |
| `HandIndex` | 0〜13。現在の手牌配列の位置。操作時に実在と合法性を追加検査 |
| `Seed` | `"0"`〜`"18446744073709551615"` の十進文字列。先頭の0、符号、空白は不可 |
| `SignedValue` | 判断説明の64bit整数を正確に運ぶ十進文字列。JavaScriptのnumberへ自動変換しない |
| `session_id` | Engine内で新規開始ごとに増える32bit整数。Engineを越えた識別や認証には使わない |
| `revision` | 成功した操作の数。開始時0。各操作で1増える |

64bitの乱数種をJavaScriptのnumberで表すと精度が失われるため、文字列を共通の表現にしています。

## 通常の対局

| `op` | 追加項目 | 正常応答の `data.kind` と意味 |
|---|---|---|
| `new` | 任意の `config` | `game`。省略時は標準設定で開始 |
| `load_config` | `toml` | `game`。TOML検証後に新規開始。失敗時は既存対局を維持 |
| `view` | なし | `game`。現在の閲覧席の画面状態 |
| `apply` | `session_id`, `expected_revision`, `action` | `game`。固定した人間席の合法な1操作 |
| `step` | `session_id`, `expected_revision` | `game`。現在のAI席が設定に従い1操作 |
| `analyze` | 上記の識別2項目と `config` | `analysis`。閲覧席が手番のとき、状態を変えず候補と実効設定を返す |
| `inspect_config` | `toml` | `config`。対局を変更せず、設定と席別の実効AI値を返す |

`GameView` は4席の得点、公開された副露と捨て牌、親・局・本場・供託・残り山枚数・ドラ表示牌・局面種別を含みます。閲覧席には自身の手牌、最後のツモ牌、振聴状態を追加します。合法手は閲覧席が現在の手番であるときだけ返します。他家の手牌、山の順序、乱数種、確定前の鳴き・ロンへの回答は返しません。共有観戦は設定の `viewer = "public"` です。

開始後に閲覧席を切り替える要求はありません。学習環境では手番ごとの席を観測しますが、別モードとして明示して使用します。

通常の操作はタグ付きの `WireAction` です。打牌とリーチは手牌の位置、暗槓は牌種、チーは消費する2牌種を指定します。`discard`、`riichi`、`tsumo`、`ron`、`pon`、`chi`、`open_kan`、`closed_kan`、`pass`、`next_round` があり、構造は生成Schemaを参照します。同じ牌種を複数持つ場合、通常画面の合法手には位置の異なる選択肢が存在します。[学習用の行動ID](environment.md)では同じ牌種へ正規化します。

最小要求は次のとおりです。

```json
{"version":1,"op":"new"}
```

返された `session_id`、`revision` と `legal_actions` の実値を使います。値を固定して再送すると `stale_revision` となり、アプリが状態を再取得するまで操作しません。自動で同じ操作を再試行すると別の手牌位置を切る危険があるため、SDKも再試行しません。

## 標準入出力とJavaScript

### mjson牌譜の解析

`{"version":1,"op":"analyze_mjai","mjson":"...","viewer":0}` は、渡された展開済みUTF-8のmjaiイベント列を解析します。`mjson` は必須文字列、`viewer` は省略またはnullで公開観測、0〜3で指定席の観測です。正常応答の `data.kind` は `mjai`、`summary` はイベント・局・結果の集計、`observation` は終端時の `AgentObservation` です。開始済み対局は不要で、実行中の対局・研究環境・識別子・操作履歴を変更しません。

要求のJSON全体は既存の1,048,576バイト上限に従います。圧縮バイト列やファイルパスを文字列の代わりに解釈しません。大きい入力とgzipはネイティブの `suzume-api mjai analyze PATH` を使用します。各時点の観測を逐次出力する場合は `suzume-api mjai observations PATH --seat N --output PATH` を使用します。

Rustの `mjai::visit` を通じて型・上限・重複キー・イベント順序・牌の増減・点数と供託を検査します。形式や席型が不正なAPI要求は `invalid_request`、mjson内の構文・意味・末尾の不整合は `log_error` となり、牌譜内の行番号とイベント型を説明に含めます。不正な入力を読み飛ばした集計は返しません。

| 終了項目 | 意味 |
|---|---|
| `summary.complete` | 正常な `end_game` を読んだときだけtrue |
| `summary.termination` | `completed`、`interrupted`、`truncated`を区別 |
| `observation.record_ended` | 明示された記録終端を読み込んだ状態 |
| `observation.game_ended` | 記録上の対局が正常終局した状態 |
| `observation.round_ended` | 最終局の和了・流局という結果が成立した状態 |

記録終端と正常終局は別です。`_suzume.reason` で明示する中断・打切りは、まだ終わっていない局を終局と扱いません。終端行そのものが欠落している場合は不完全入力として拒否します。既知の最終持点・供託だけを返し、入力にない情報はnullで示します。

JavaScript/TypeScriptでは `Client.analyzeMjai(mjson, viewer?)` がこの要求を実行します。`parseMjaiEvent` と `readMjaiEvents` は展開済みデータを構造・順序の面から読む入口で、牌の移動や点数の意味検査はRust/WASMへ委ねます。Schemaの `MjaiEvent` は既知項目の投影です。生のmjsonは追加項目を保持するため、その全体へ閉じたSchemaを直接適用しません。通常のAPI要求とは異なり、mjsonの追加メタデータには有限な小数を許容します。詳細な上限と型は[SDK](../../packages/suzume/README.md)に記載しています。

この解析は外部規則のすべての役・得点の証明、合法手マスクの生成、自作coreへの完全再生を提供しません。従来のseedと操作列を用いた完全再生は、独自replay形式の入口として維持します。

### 標準入出力の起動

```sh
cargo run --locked -p suzume-api --bin suzume-api -- serve
```

1行の要求につき1行の応答を返します。空行も不正要求として応答します。大きすぎる行は上限を越えて保持せず、その行を読み終えてから次の要求へ進みます。UTF-8の誤りや不正JSONで後続行との対応を失いません。読み書き自体が失敗した場合は非0で終了します。

Node.jsとブラウザでは[SDKの手順](../../packages/suzume/README.md)を使用します。生成型に加え、実行時にも応答の版・構造・範囲を検査します。型を `as` で付けるだけの検証ではありません。

## 更新手順

```sh
cargo run --locked -p suzume-api --bin export-schema
python3 scripts/generate_protocol_ts.py
```

両コマンドへ `--check` を渡すと変更せず同期だけを検査します。外部契約の破壊的変更はAPIの版を上げます。エンジン0.2.0の再生ログは同じエンジン・規則・シャッフル識別を要求し、API版が同じだけで過去ログの再生互換を保証しません。
