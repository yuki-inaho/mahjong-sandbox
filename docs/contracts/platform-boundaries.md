# 0.2.0の公開境界と生成仕様

## 公開入力と信頼する内部処理

`game::Seat` は0〜3だけを保持する型です。`new(usize)` は範囲外で `None`、`TryFrom<usize>` は `GameError` を返します。検査前に `u8` へ狭めないため、256や `usize::MAX` が別の席へ化けることはありません。`index()` と `id()` で検証済みの値を取り出します。

`Game::seat_wind(usize)` は0.2.0から `Option<Tile>` を返します。不正な席を剰余で丸めず、加算の前に拒否します。これは0.1.0のRust APIからの互換変更です。`Action::label` の返り値は `String` を維持し、不正な添字には理由を含む表示を返します。ラベルの生成は操作の合法性を判定する機能ではありません。操作の適用は引き続き `Game::apply` が受け付けます。

| 入口 | 利用者 | 保持する情報 |
|---|---|---|
| `Game::players`、`seed`、`last_draw`、`Debug` | 信頼するローカル実験・完全再生 | 他家の手牌、他家のツモ牌、山を再構築できる種などを含み得ます |
| `Game::observation(seat)` | 検証済みの自席を判断するAI | 自席の手牌と公開情報。山順序・他家手牌を含みません |
| 外部APIの観測型 | 固定した閲覧席のフロントエンド | 外部APIが定める情報制限。内部の `Game` をそのまま直列化しません |

coreの全情報getterが存在することは、外部の呼出し元にその情報の取得権限があるという意味ではありません。閲覧席と対局の版を検査する外部APIの境界は、別の責務です。

## 局面と履歴の確定

`Game::apply` は合法性を確認し、牌・点数・進行中の局面だけを候補へ複製します。候補への操作と `validate` が成功してから、元の `history` と表示用 `events` へ新しい記録だけを追記します。失敗として `Err` を返す経路では、元の局面と両方の履歴を変更しません。

過去の操作と文字列を毎回複製していた0.1.0では、操作数が増えると累積する複製量が二乗で増えていました。0.2.0の候補作成はその履歴を含みません。追記用 `Vec` の容量が足りない場合の再確保は残ります。速度の倍率は測定していないため主張しません。

[`committing_an_action_keeps_existing_log_storage`](../../src/game/core_boundary_tests.rs) は、合法な操作で履歴を作り、容量を確保してから次の操作を適用します。過去の内容と同じ領域を保持すること、追加した操作が一致すること、拒否した操作で全状態が不変なことを確認します。通常の `Game::clone` は完全な状態と履歴の複製であり、実験用に呼び出した場合のコストは別です。

非リーチの同巡振聴は、次の通常ツモに加え、鳴いてツモを省略した後の自己打牌でも解除します。リーチ後の見逃しを示す `riichi_furiten` は解除しません。[回帰局面](../../src/game/platform_tests.rs) は136枚を配分し、公開操作によって見送り・ポン・打牌・別の待ちへのロンを再現します。

## 生成器が受け付ける言語

[`rules/contracts.json`](../../rules/contracts.json) の宣言を次の順序で検査します。生成器は規則の自然言語を解釈するものではありません。

1. JSONの重複キー、未知・欠落項目、版、契約ID、識別子、予約語、引数の重複と型の組合せを検査します。
2. 式を再帰的に型付けします。型は `bool`、`u16`、`u64`、`i64` です。整数型同士の暗黙の混用を認めず、契約全体の結果は真偽値とします。
3. 整数の各演算が、その型の範囲に収まることを区間計算で確認します。先に評価した `and` の真条件、`or` の偽条件から、確実に成立する単純な変数境界だけを後続へ渡します。
4. RustとLeanに同じ型付きの式を生成します。数値リテラルにRustの整数型とLeanの `Nat` / `Int` を明示します。変数のない数値式の既定は両方とも符号付き整数です。

加算・減算・乗算には型の上下限を超えない条件が必要です。符号なしの減算で負になる可能性がある場合は拒否します。除算・剰余は被除数が非負、除数が正と確認できる場合だけを受け付けます。これにより、負数の除算やゼロ除算に関するRustとLeanの異なる規則を暗黙に混ぜません。

区間計算は保守的です。変数間の複雑な相関を使えば安全な式でも、この検査で範囲を確認できなければ拒否します。拒否された式のために検査を無効化せず、明示的な条件か別の対応方針を設計してください。式の深さは64、節数は4,096を上限にしています。

説明文と元の宣言はRustのraw文字列へ含めます。元の文字列に区切りが現れる場合は区切りを伸ばし、契約IDなどコードのコメントへ出力する値も構文を制限します。

## C-03とC-04Pの追加境界

| 関係 | 0.2.0で加えた条件 | 既存実処理との関係 |
|---|---|---|
| C-03 `move_one_relation` | 移動後の両側枚数を0〜4に制限し、加算の前に検査 | 成功した `move_one` が既に保証する範囲です。Leanの元の移動モデルと同じ関係を保ちます |
| C-04P `point_transfer_relation` | 前後の残高をi32の範囲、金額を0〜i32最大値に制限 | `transfer_points` の引数と成功結果の範囲です。任意の数学的整数の移転関係としては扱いません |

これらは生成比較仕様の計算を安全にする条件です。ゲームが使う実Rust関数の成功・拒否条件を狭めたものではありません。元の入力全域を扱うKaniの6ハーネスと、成功した移転を前提にするLeanの18定理が、更新後も成功しました。生成比較仕様は `pub(crate)` に限定し、外部ライブラリ利用者向けAPIとして公開しません。

## 検査と限界

```sh
python3 scripts/test_generate_rules.py
python3 scripts/check_generator_semantics.py
python3 scripts/generate_rules.py --check
cargo test -p suzume --lib
cargo test -p suzume --doc
cargo kani -p suzume --lib --output-format terse
(cd proofs && lake build)
```

生成器の回帰検査19件、生成したコードを実RustとLeanで調べる7関係・19具体例、coreの単体検査66件、生成仕様への外部アクセスを拒否する文書テスト1件、Kani6ハーネス、Lean18定理がこの変更段階で成功しました。これはAPI・SDK・配布を含むworkspace全体の完了判定ではありません。

先行失敗は [core](../verification/logs/platform-core-before.txt) と [生成器](../verification/logs/platform-generator-before.txt)、成功は [core](../verification/logs/platform-core-after-02.txt)、[生成器](../verification/logs/platform-generator-after.txt)、[実コードの意味照合](../verification/logs/platform-generator-semantics.txt)、[Kani](../verification/logs/platform-kani.txt)、[Lean](../verification/logs/platform-lean.txt) に残しています。途中のcore検査では並行作業中のAIを使ったfixtureの準備が失敗したため、その [結果](../verification/logs/platform-core-after.txt) を保持し、core検査を合法手の直接選択へ独立させました。

区間検査と具体例の意味照合を追加しても、Python生成器自体の意味保存を形式証明したことにはなりません。Rust全体とLeanモデルの完全同値、役判定・局進行・AIの全入力に対する形式証明という主張も行いません。
