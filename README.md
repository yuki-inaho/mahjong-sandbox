# すずめ 0.2.0 — 契約と検証を備えたRust製麻雀

人間1人とAI3人で東風戦を遊べる四人リーチ麻雀です。同じRustの対局処理を、
端末、JavaScript/TypeScriptの画面、対局記録の再生、探索・強化学習の実験から使用できます。
牌・和了形・役と符・点数・局進行・AIを独自に実装しています。
外部クレートに依存しないのは核となる`suzume`クレートです。API・CLI・WASMは別クレートで必要な依存を持ちます。

採用ルールを宣言し、不正な操作を状態変更前に拒否し、成功した操作の不変条件を検査します。
**6種類の基本契約について実Rust関数をKaniで検査し、対応する性質をLeanの18定理で証明**します。
役判定・対局全体・AI・画面の全入力を形式証明済みとするものではありません。
[対象と前提](docs/verification/README.md)を明記しています。

## 端末で遊ぶ

リポジトリのルートから実行します。Python 3.12以降とOS用のC/C++リンカーが必要です。
以下はbash/zshの例です。Windowsの設定読込みと詳しい前提は[再現手順](docs/development/reproducibility.md)を参照してください。

```sh
python3 scripts/bootstrap.py --profile native
source dist/tools/activate.sh
python3 scripts/doctor.py --profile native
cargo run --locked -p suzume-cli --release -- --config config/default.toml
```

固定ツールを導入済みなら、最後のコマンドから開始できます。
通常のワークスペースはRust 1.100.0を固定し、依存のない核クレートだけをRust 1.80.1でも検査します。
形式検証・WASM・SDKの道具は用途別に導入でき、端末で遊ぶためにすべてを準備する必要はありません。

日本語を表示できる横幅92列以上の端末を使用してください。行入力式で、端末の生入力モードには変更しません。
`--plain`を付けると色・画面消去を使いません。標準入出力が端末でない場合と`NO_COLOR`指定時も同様です。
配牌の乱数種を省略した場合は、すべてのモードで0です。

```sh
cargo run --locked -p suzume-cli --release -- --plain --seed 42
```

TUIの人間席は0に固定し、他の3席はAIです。設定の`viewer`も0でなければ起動時に拒否します。
`--config`と`--seed`は併用できません。設定を使う場合はTOMLの`game.seed`を変更します。

### 操作

| 入力 | 操作 |
|---|---|
| `1`〜`14` | 表示された手牌番号の牌を捨てる |
| `r 5` | 5番の牌を捨ててリーチ。成立条件を満たす場合のみ |
| `t` / `ron` | ツモ和了 / ロン和了 |
| `/1`、`/2`… | 画面の補助操作を選ぶ。複数のチー・槓の形も区別できる |
| `Enter` | 応答の見送り、局終了後の次局 |
| `h` / `q` | 操作説明 / 対局の中断 |

萬子・筒子・索子は河では`m`・`p`・`s`、字牌は`z`で表します。
合法なリーチ・和了・鳴きだけを選べ、不正な入力で対局状態は変わりません。
手牌は理牌済みで、ツモ牌を別途表示します。リーチ後の打牌はツモ牌に限定します。

## AI設定と自動対局

TOMLはCLI・API・ログで共通の型を使用します。席ごとに`random`・`greedy`・`ukeire`、
計算予算、読みの深さ、防御重み、AIの乱数種を設定できます。
`beginner`・`standard`・`strong`は設定を選ぶためのプリセット名で、勝率の順序を保証しません。
いずれも学習済みモデルではなく、判断には自分の手牌と公開情報だけを渡します。

```sh
# 全員AIで一局を表示して終了
cargo run --locked -p suzume-cli --release -- --demo

# 配牌画面だけを表示
cargo run --locked -p suzume-cli -- --snapshot --plain

# 席別の設定で1東風戦を実行し、新しいログへ記録
cargo run --locked -p suzume-cli --release -- \
  --config docs/ai/simulation.toml --simulate 1 --log match.mjson --log-format mjai

# 設定を省略すると全員Greedy。種0〜31の32東風戦を実行
cargo run --locked -p suzume-cli --release -- --simulate 32 --seed 0

cargo run --locked -p suzume-cli -- --help
```

明示した設定で`--demo`・`--simulate`を実行する場合は全員AIが必要です。
操作上限は`game.max_steps`、省略時10,000です。見送りと次局への進行も数え、
上限到達は通常終局と区別して非0で終了します。単一の`--log`は複数試合と併用できません。

[固定条件の評価](docs/ai/results.md)は各群32東風戦、合計64試合を全件報告しています。
Ukeire候補の平均順位は2.5625、Random参照は3.6875でした。どちらも他の3席はGreedyです。
この比較だけではUkeireがGreedyより強いことや、人間相手の強さを示していません。
詳細は[算法・予算](docs/ai/algorithms.md)、[設定とCLI](docs/ai/cli.md)、[実験計画](docs/ai/evaluation.md)に分けています。

## JavaScript・TypeScriptとブラウザ

| 部分 | 場所 | 責務 |
|---|---|---|
| Rustの核 | [src](src/) | 牌、役と点数、対局、公開観測を使うAI。標準ライブラリのみ |
| 型付きAPI | [suzume-api](crates/suzume-api/) | JSON/TOMLの検証、閲覧席、古い操作の拒否、環境、ログ |
| 端末CLI | [suzume-cli](crates/suzume-cli/) | 入出力、席別AI、共通設定・ログへの接続 |
| WASM結合 | [suzume-wasm](crates/suzume-wasm/) | 同じAPIをWebAssemblyから呼ぶ薄い入口 |
| JS/TS SDK | [packages/suzume](packages/suzume/) | 生成型、実行時検査、Node.jsとブラウザ用の入口 |

SDKの入口は`@mahjong-sandbox/sdk/node`と`@mahjong-sandbox/sdk/browser`です。
[SDKの使用例](packages/suzume/README.md)から、対局開始・打牌・AI操作・研究環境を呼び出せます。

最小ブラウザ例は[examples/browser/index.html](examples/browser/index.html)です。
固定ツールを用意した後、リポジトリのルートから次を実行します。

```sh
python3 scripts/bootstrap.py --profile frontend
source dist/tools/activate.sh
python3 scripts/build_wasm.py
npm ci --ignore-scripts --prefix packages/suzume
npm run build --prefix packages/suzume
python3 -m http.server 8080 --bind 127.0.0.1
```

`http://127.0.0.1:8080/examples/browser/`を開きます。
[ブラウザ例の案内](examples/browser/README.md)に操作と検査方法があります。
ルールやAIをJavaScript側で再実装する必要はありません。

## JSON API、対局記録、研究環境

ネイティブのAPI入口は、標準入力の1行につきJSON応答を1行返します。

```sh
cargo run --locked -p suzume-api --bin suzume-api -- serve
```

開始要求は`{"version":1,"op":"new"}`です。後続の操作には返されたセッションIDと更新番号を使います。
通常の画面には指定席の手牌と公開情報だけを返し、他家の手牌・山順序・配牌の種を返しません。
詳しい入出力は[API版1](docs/api/README.md)、設定は[共通設定](docs/api/configuration.md)を参照してください。

```sh
# 自作対局のmjson例を解析。8操作で打切りのためcomplete=false
cargo run --locked -p suzume-api --bin suzume-api -- mjai analyze examples/eight-actions.mjson

# 旧replayを全件検査してmjsonへ変換
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai convert examples/replay-eight-actions.jsonl --output converted.mjson

# 席0から見える情報を各イベント後に出力
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai observations examples/eight-actions.mjson --seat 0 --output observations.jsonl

# 原イベントを保持した注釈付き形式へ包み、標準mjsonへ戻す
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai wrap examples/eight-actions.mjson --output analysis.annotated.jsonl
cargo run --locked -p suzume-api --bin suzume-api -- \
  mjai unwrap analysis.annotated.jsonl --output restored.mjson

# 旧replayの厳密再生と学習用の遷移も引き続き利用可能
cargo run --locked -p suzume-api --bin suzume-api -- replay examples/replay-eight-actions.jsonl
cargo run --locked -p suzume-api --bin suzume-api -- \
  dataset examples/replay-eight-actions.jsonl --output transitions.jsonl
```

[標準の対局ログ](docs/logging/README.md)は、1行1イベントの `.mjson` です。
`mjai`は全席の手牌を含む分析用、`mjai_public`は伏せ牌を省いた共有用です。
CLIはgzip圧縮された `.mjson` も内容から判別します。SDKでは `readMjaiEvents` で構造を逐次読み、
`client.analyzeMjai` で同じRustの意味解析を呼びます。API/WASM・TSへの入力は展開済みデータです。

[注釈付きmjai](docs/logging/annotated-mjai.md) は、元イベントのJSON原文と解析用の注釈を分けて保存します。
未知の追加項目、数値字句、明示null、別名を原文のまま取り出せます。
非空の注釈を捨てて標準へ戻す場合だけ `--drop-annotations` を指定します。
解析にはCLIの `--format annotated_mjai` またはSDKの第3引数を使い、注釈を席別観測へ混ぜません。

乱数種・内部操作による厳密再生とAI判断の保存には、`--log-format replay` を明示します。
[旧replay/publicの互換仕様](docs/logging/legacy-jsonl.md)を維持し、旧replayからmjsonへ変換できます。
外部牌譜の解析は記録上の局面を復元する機能であり、全ルールの合法性検証や合法手マスクの生成ではありません。
[実牌譜との互換性](docs/logging/mjai-compatibility.md)では、固定した20ケース・23局を使う再現検査を定義しています。

[研究用環境](docs/api/environment.md)は`reset`・`step`・観測・189要素の合法行動マスクを提供します。
1回の`step`は1席の1操作で、報酬は4席の点数差です。通常終局と操作上限による打切りを区別します。
[外部算法の接続例](docs/ai/extension.md)には、木探索や学習器を追加するときの情報境界と責務を記載しています。

## 麻雀ルールの範囲

136枚、東風戦と連荘、リーチ・ダブルリーチ・一発、ロン・ツモ、振聴、
チー・ポン・大明槓・暗槓、嶺上牌、ドラ・裏ドラ・槓ドラを扱います。
通常形・七対子・国士、一般的な役と役満、符と翻、本場・供託・流局精算を実装しています。

現行の固定ルールでは、赤牌・加槓・リーチ後の槓・途中流局・流し満貫・天和/地和・包を採用しません。
複数ロンは頭ハネ、喰い断あり、食い替えなしです。通信対戦は含みません。
外部mjsonの赤牌・加槓・複数ロンを記録として解析する機能は、この対局コアの採用規則とは別です。
TOMLでAIを設定することと、麻雀ルールの変更は別です。
細則は[ルール仕様](docs/rules/riichi.md)と[機械可読の宣言](rules/riichi.rules)にまとめています。

## 契約・検証・配布

| 対象 | 正本・検査 |
|---|---|
| 規則と基本契約 | [riichi.rules](rules/riichi.rules)、[contracts.json](rules/contracts.json) |
| 宣言とコードの対応 | 同じ型付き式からRust・Leanを生成し、再生成差分を拒否 |
| 実Rustの基本契約 | Kaniの6ハーネス。ゲームが使用する契約関数を検査 |
| 数学的な性質 | Leanの18定理。モデル・前提・許可公理を明記 |
| 対局の実行時検査 | 各種4枚・計136枚、局面ごとの構造枚数、供託込み総点など |
| 外部境界 | Rust/TSの型と実行時検査、API/WASM比較、mjsonの実牌譜検査、旧ログ再生、CLI・ブラウザ検査 |

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/check_documentation.py

# 固定した外部牌譜を取得し、解析・原文の往復・全イベント後の4席観測を検査
python3 scripts/verify_mjai_real.py

# 必要な形式検証ツールも導入した環境で実行
python3 scripts/verify_platform.py --profile full

# 配布物を作成し、別の構築領域で実際に消費して検査
python3 scripts/package.py
```

[導入・再現・配布手順](docs/development/reproducibility.md)に、ツール版、依存lock、各検査の範囲、
`.crate`・SDKのnpm書庫・ソース書庫・実行ファイル・SHA256一覧の作成方法をまとめています。
全体検証の結果は実行IDごとに保存します。未実行のOSや配布環境の互換性まで保証しません。
初版の`scripts/verify.py`と検査ログは履歴として保持し、現行の全体検証には`verify_platform.py`を使います。

## 文書と作業記録

- [文書案内](docs/README.md)：用途別の入口と現行仕様。
- [コンセプト](docs/concept/overview.md)と[共有する開発基盤](docs/concept/platform.md)。
- [責務と依存関係](docs/architecture.md)、[設計判断ADR](docs/adr/)。
- [契約の対応表](docs/contracts/README.md)、[公開境界](docs/contracts/platform-boundaries.md)、[形式検証の限界](docs/verification/README.md)。
- [注釈付き記録の作業計画書兼記録書](docs/workdocs/workdoc_Nov23-2026_mjai_annotated.md)と[現状の厳格レビュー](docs/reviews/annotated-baseline-review.md)。
- [mjson移行の作業書](docs/workdocs/workdoc_Nov23-2026_mjson.md)と[当時のレビュー](docs/reviews/mjson-baseline-review.md)は前回の記録です。
- [基盤整備時の作業書](docs/workdocs/workdoc_Nov23-2026_platform.md)と[その独立レビュー](docs/workdocs/review_Nov23-2026_platform.md)は当時の記録です。
- [初版の作業書](docs/workdocs/workdoc_Nov23-2026_suzume.md)、[初版検証結果](docs/verification/results.md)、[初版のOS確認](docs/verification/portability.md)は履歴です。

MITライセンスです。外部コードを取り込まずに参照した観点は[コンセプトの出典](docs/concept/overview.md#参考にした公開資料)に記録しています。
