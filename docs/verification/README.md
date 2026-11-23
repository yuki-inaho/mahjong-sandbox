# 形式検証と実行検査

## 検証の目的

Leanは基本操作の数学的な性質を証明し、Kaniは実際のRust関数について共通宣言の数式に一致するかを検査します。具体的な手牌や対局全体はRustテストと実行時の不変条件検査で確認します。三つの結果は異なる範囲の証拠です。

| 層 | 対象 | 成功から分かること | 成功しても分からないこと |
|---|---|---|---|
| 宣言の同期 | ルール定数・基本契約の数式 | 同じ宣言からRust/Leanの生成物が再現される | 宣言が意図した麻雀規則そのものか |
| Lean 4.36.0 | `proofs/Mahjong/Contracts.lean` の18定理 | 共有宣言に対応する数学的モデルで、保存則・境界・拒否時の性質が成立する | Rust全体の意味論、牌譜全体や全役の正しさ |
| Kani 0.68.0 | `src/contracts.rs` の6ハーネス | ハーネスの入力域で、実Rust関数が共有宣言と対応し、パニック・算術エラーを起こさない | 対局処理全体・端末・乱数・役判定・最終バイナリの全挙動 |
| Rustの具体例・対局検査 | 役・符・点数、操作、牌と点数の総数 | 実行した例と対局で期待結果を満たす | 実行していない全入力に対する証明 |

## 共通宣言から実装までの対応

`rules/contracts.json` は、比較・論理積・論理和・加減乗除・剰余による小さな式木です。変数名、型、演算子、参照する定数を検査してから、`src/generated_contracts.rs` と `proofs/Mahjong/GeneratedContracts.lean` に変換します。

Leanの `Nat` は非負整数、`Int` は符号付き整数です。KaniハーネスはRust入力を安全に広い整数へ変換して生成式と比較します。`u8` の牌や枚数は `u16` へ、`i32` の点数は `i64` へ変換するため、検査式の加減算は元の入力域で溢れません。`usize` は `u64` へ変換します。この証拠を取得した対象は `x86_64-unknown-linux-gnu` です。32ビットや他OSの実行バイナリをこの結果だけで検証済みとは扱いません。

Kani側に `kani::assume` はありません。以下の入力を全域で扱い、不正入力も拒否結果と照合します。

| ハーネス | 入力域 | 繰り返しの上限 |
|---|---|---|
| `next_seat_refines_declaration` | 任意の `u8` | なし |
| `dora_refines_declaration` | 任意の `u8` | なし |
| `move_one_refines_declaration_and_conserves` | 任意の `u8` 2個 | なし |
| `point_transfer_refines_declaration_and_is_atomic` | 任意の `i32` 5口座、任意の `usize` 添字2個、任意の `i32` 金額 | 6。5口座の走査に終端判定を含む |
| `riichi_refines_declaration` | 真偽値3個、任意の `i32` 持点、任意の `usize` 残牌数 | なし |
| `discard_refines_declaration` | 真偽値1個、任意の `u8` 2個 | なし |

展開不足の検査は有効なままです。初回の点数ハーネスでは配列比較が内部の20バイトの `memcmp` を使い、上限6に収まらず失敗しました。5口座をそれぞれ比較する同値な検査に置き換え、成功を確認しています。初回失敗は [記録](logs/kani-initial-unwind-failure.txt) に残しています。

## 信頼する部分と証明の限界

この構成は、RustコードからLeanへ自動変換してプログラム全体の同値性を証明したものではありません。対応する**基本契約の数式**を共通化し、LeanのモデルとRustの各関数をそれぞれその式に結び付けています。

Python生成器の実装、Rust/Leanのコンパイラ、Kani/CBMC、標準ライブラリ、実行環境は信頼する部分です。生成器の変換そのものを証明したわけではありません。レビュー・再生成比較・否定検査を併用します。Leanの依存公理は標準の `propext`、`Quot.sound`、`Classical.choice` の範囲で、追加の独自公理や `sorryAx` はありません。[Audit.lean](../../proofs/Mahjong/Audit.lean) で再確認できます。

役の網羅性、待ち判定の網羅性、符計算、鳴きの優先順位、フリテン、局進行、乱数の公平性、UIの正しさについて、Lean/Kaniで全入力の証明を済ませたとは主張しません。対応範囲は [契約一覧](../contracts/README.md) のC-01〜C-06です。

## 再実行

Rust、Kani 0.68.0、Lean 4.36.0、Python 3を用意して、リポジトリのルートから実行します。Python補助ツールは外部パッケージ不要で、`uv run --no-project` からも実行できます。

```sh
python3 scripts/generate_rules.py --check
cargo test
cargo kani --output-format terse
cd proofs
lake build
lake env lean Mahjong/Audit.lean
cd ..
python3 scripts/check_contract_negatives.py
```

Kaniの導入は [公式手順](https://model-checking.github.io/kani/install-guide.html)、ハーネスと展開限界は [公式チュートリアル](https://model-checking.github.io/kani/tutorial-loop-unwinding.html) を参照します。Lean/Lakeの導入は [公式手順](https://lean-lang.org/install/manual/) に従います。Lean側には外部の数学ライブラリ依存を置いていません。

## 否定検査

[check_contract_negatives.py](../../scripts/check_contract_negatives.py) は一時コピーだけを書き換え、次の5件を実行します。単に非0終了しただけでは成功にせず、意図した拒否の出力まで照合します。

1. 未知のルール名を追加すると生成器が拒否する。
2. 未対応の赤牌設定を有効にすると生成器が拒否する。
3. 生成物だけを変えると同期検査が不一致を検出する。
4. 次席を一つ余計に進める誤実装をKaniが拒否する。
5. `nextSeat 0 = some 2` という偽の定理をLeanが拒否する。

これは完成後の検査器の否定検査です。契約実装の前に実行したテストの失敗→成功とは区別します。実結果は [negative-controls.txt](logs/negative-controls.txt)、実装前の失敗は [contracts-red.txt](logs/contracts-red.txt) に保存します。
