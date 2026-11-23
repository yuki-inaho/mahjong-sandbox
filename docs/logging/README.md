# mjsonの対局記録・解析・視点別観測

通常の対局ログは、mjaiのイベントを1行ずつ記録するUTF-8の `.mjson` です。
自作対局の記録と外部牌譜を同じ解析入口へ渡し、感想戦、統計、AIエージェントの入力作成に使用できます。
設計判断は [ADR-0007](../adr/0007-mjai-records.md)、参照実装との差と実牌譜の検査は
[外部mjsonとの互換性](mjai-compatibility.md) にまとめています。
解析理由や評価を付けた保存と標準形式への往復は [注釈付きmjai仕様](annotated-mjai.md) を参照してください。

## 形式の選択

| `logging.format` / `--log-format` | 内容 | 主な用途 |
|---|---|---|
| `mjai`（省略時） | 配牌・ツモ・打牌・成立した副露・和了・流局などの意味イベント。全席の手牌を含む | 感想戦、外部形式との交換、事後分析 |
| `mjai_public` | 配牌とツモの非公開牌を `?` にした同じイベント形式 | 公開情報に限った共有・観戦 |
| `replay` | 旧独自JSONL。乱数種、内部操作、局面照合、設定、任意のAI判断 | 同じコアによる厳密再生、判断の分析、189行動IDの学習用遷移 |
| `public` | 旧独自JSONLの共有形式 | 従来の利用者との互換性 |

`mjai` は各席の非公開情報を含むので、対局中の相手へそのまま渡す入力ではありません。
AIの不完全情報評価には視点別観測を使います。`mjai_public` では伏せた手牌を後から復元できません。
山の種・内部の見送り応答・AI判断の説明はmjsonへ記録しません。これらが必要な場合は `replay` を明示します。
旧形式の契約と利用方法は [旧JSONL仕様](legacy-jsonl.md) に保持しています。

## 対局を記録する

以下はリポジトリ直下から実行します。保存先は新規ファイルでなければなりません。

```sh
# 標準設定で、全員AIの1東風戦をmjsonへ記録
cargo run --locked -p suzume-cli --release -- --simulate 1 --seed 0 --log match.mjson

# 人間1人の対局を、公開情報だけのmjsonへ記録
cargo run --locked -p suzume-cli -- --plain --seed 42 --log public-match.mjson --log-format mjai_public

# AI判断と厳密再生を必要とする場合は旧replayを明示
cargo run --locked -p suzume-cli --release -- --simulate 1 --seed 0 --log replay-match.jsonl --log-format replay
```

TOMLで指定する場合は `[logging]` の `format = "mjai"` を使用します。
CLIの `--log-format` はTOMLの値を上書きします。ファイル出力の開始には `--log PATH` が必要です。
1ファイルは1対局に対応し、複数試合の `--simulate N` と単一の `--log` は併用できません。

## 解析・変換・観測の出力

```sh
# 同梱された自作対局例を解析
cargo run --locked -p suzume-api --bin suzume-api -- mjai analyze examples/eight-actions.mjson

# 旧replayを全件検査してからmjsonへ変換
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai convert examples/replay-eight-actions.jsonl --output converted.mjson

# 各イベント適用後の席0の観測を出力
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai observations examples/eight-actions.mjson --seat 0 --output observations.jsonl
```

`mjai analyze` はイベント数 `records`、局数 `rounds`、和了数 `wins`、流局数 `draws`、
副露・槓の件数 `calls`、既知の牌項目に赤牌を含むイベント数 `red_events`、最終点 `final_scores`、
供託本数 `riichi_sticks`、正常終了の有無 `complete`、終了種別 `termination` をJSONで返します。
点数差が省略された外部牌譜では、確定できない最終点・供託を `null` とします。

[eight-actions.mjson](../../examples/eight-actions.mjson) は、本コアで生成した8操作の
[旧replay例](../../examples/replay-eight-actions.jsonl) を変換したものです。外部の実牌譜ではありません。
内部の見送り操作は意味イベントを増やさないため、操作数とイベント数は一般に一致しません。
この例は8イベント・1局、全席25,000点で、操作上限による打切りです。
`complete=false`、`termination="truncated"` が正しい結果であり、完走した対局として扱いません。

`mjai observations` の各行は `schema_version`、入力行番号 `line`、イベント種別 `type`、
指定席の `observation` を含みます。元イベント全文は出力しません。
自分の手牌と公開情報を分け、他家の伏せ手牌を観測へ含めません。
`--seat` は0〜3です。Rust/APIの観戦者用観測では自分の手牌も省略できます。

変換と観測出力は、末尾とEOFまで検査した後に新しい出力ファイルを確定します。
既存ファイルを上書きせず、不正入力から部分的な結果を完成品として公開しません。
通常の対局ログは逐次保存されるため、強制終了・書込失敗では不完全なファイルが残る場合があります。

## Rust・API・TypeScriptの役割

Rustの `mjai::analyze(reader, limits)` は行ごとの構造と局面の意味を検査し、集計を返します。
`mjai::visit(reader, limits, consumer)` は検査した各イベントとその直後の状態を渡し、
`state.observe(viewer)` で視点別観測を取得できます。型付きフィールドと元の付加フィールドを併せて保持します。
途中まで呼ばれたconsumerは、後続行を含む全牌譜の正常性を保証しません。関数全体の成功を確認してください。

APIの `analyze_mjai` とSDKの `client.analyzeMjai(mjson, viewer?, format?)` は同じRust解析器を呼びます。
対局セッションの状態を変えずに集計と最終観測を返します。
`format` の省略時は `mjai`、注釈付き入力では `annotated_mjai` を明示します。
SDKの `readMjaiEvents(chunks)` は展開済みUTF-8の非同期または同期のバイト列を逐次読み、
構造と局の開始・終了を検査します。持牌消費・点数などの意味検査はRust側で実行します。
詳細は [API](../api/README.md) と [SDK](../../packages/suzume/README.md) を参照してください。

CLIは非圧縮・gzipを先頭バイトで判別するため、gzip圧縮済みで拡張子が `.mjson` のファイルも読めます。
Rustの `BufRead`、API/WASMへ渡す文字列、TSのバイト列は展開済み入力を要求します。
API要求全体は1MiBまでです。大きな牌譜はファイルを逐次読むCLIまたはRustのreaderを使います。
読み込みの既定上限は1行1MiB、10,002イベント、展開後の合計256MiBで、いずれも正である必要があります。
破損、重複キー、未知イベント、上限超過、予期しないEOFを成功扱いにしません。

## 終了と検査能力の境界

通常の `end_game` は局終了後に置きます。自作対局の手動中断・操作上限では、
`end_game._suzume.reason` をそれぞれ `interrupted` / `truncated` とし、`complete=false` を返します。
未完局へ架空の和了や流局を追加しません。終了イベントそのものが欠けた入力は不完全として拒否します。
中断の追加情報を扱わない外部消費者には、通常終局の牌譜を渡してください。

このパーサが復元するのは記録上の局面です。外部の赤牌、加槓、複数ロンを表現できますが、
コアの採用規則を拡張したことや、すべての役・振聴・ルールで合法だと証明したことにはなりません。
外部牌譜から合法手マスク、189行動ID、学習用報酬を自動生成しません。
それらには [研究用環境](../api/environment.md) または旧 `replay` の厳密再生を用いてください。

## 実牌譜と配布物の検査

```sh
cargo test --locked -p suzume-api --test mjai_parser
cargo test --locked -p suzume-api --test mjai_writer
cargo test --locked -p suzume-api --test mjai_cli
python scripts/verify_mjai_real.py
python scripts/package.py
```

実牌譜検査は固定した外部データ20件・23局を対象とし、うち2件は元対局の識別子と最終成績を確認しています。
選定、期待値、再取得方法、データの取り扱いは [互換性文書](mjai-compatibility.md) に記載しています。
牌譜本文はリポジトリ・配布物・CI成果物に含めません。
配布消費検査では、自作の同梱例を用いて、展開したnativeバイナリの解析・旧形式からの変換、
公開SDKの逐次読込みとNode/Web用WASMによる解析を実行します。

## 変換と拡張

| 入力と出力 | 現在の扱い | 保持する情報と境界 |
|---|---|---|
| 旧replay → mjai | CLI `mjai convert` | 旧記録全体を検証後、配牌・ツモ・確定した操作・精算を標準イベントへ変換します。内部の見送り、判断詳細、乱数種は交換用イベントには含めません。 |
| mjai → 型付きイベント＋保存原文 | Rust `visit` / SDK `readMjaiRecords` | 既知項目と追加値は解析用です。保存には `ParsedEvent.json` / `ParsedMjaiRecord.json` を使い、数値字句・null・別名・未知項目を保持します。 |
| mjai ↔ 注釈付きmjai | CLI `mjai wrap` / `mjai unwrap`、Rust・SDKの変換入口 | 原イベントの内部の原文を保持します。非空注釈の破棄は明示指定にし、破棄行数を返します。改行方式と圧縮バイト列は一致対象外です。 |
| mjai → AI向け観測 | CLI `mjai observations` / API `analyze_mjai` | ファイルは版番号・元の行番号・イベント種別付きで各イベント後の観測を返します。APIは最終観測と集計を返します。指定席以外の手牌は含めません。 |
| mjai → 完全再生可能な旧replay | 未提供 | 乱数種と内部操作が不足するため、一意に復元できません。 |
| 公開観測 → 元の完全情報牌譜 | 未提供 | 伏せた牌は観測から復元できません。元の記録を別に保持してください。 |

付加的な説明や評価は [版1の注釈付き記録](annotated-mjai.md) の `annotations` へ保存できます。元イベントの追加JSONと解析注釈を分け、注釈を通常の牌集計へ混ぜません。現在の観測ファイルは `schema_version`、`line`、`type`、`observation` を持ちます。完全情報の元イベントや注釈を自動添付せず、AIの閲覧範囲を維持します。

Rustの `raw` とSDKのイベントオブジェクトは数値を近似する場合があります。再保存の正本として `JSON.stringify` や `Value` の直列化を使わず、原文を保持する読込み・出力関数を使ってください。

新たな意味を持つイベントを導入するときは、Rustの `mjai::Event`、状態遷移、生成Schema、SDKの局順序検査を更新し、対応する正常・異常事例を追加してください。未知のイベントを黙って読み飛ばす拡張は行いません。既知イベントの追加JSON欄だけであれば、型の既存必須項目を変更せずに保持できます。
