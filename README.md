# すずめ — Rustでつくる、契約付きの麻雀

人間1人とCPU3人で東風戦を遊べる、端末用の四人リーチ麻雀です。
牌・和了形・役と符・点数・対局・CPU・画面を独自に実装しています。
ゲーム本体の外部クレート依存はありません。

ルールの宣言と契約を土台にし、**実際のRust関数をKaniで検査し、対応する数学的な性質をLeanで証明**します。
形式検証の対象は6種類の基本契約です。全役や対局全体を形式証明済みとするものではありません。
[検証の範囲と根拠](docs/verification/README.md) を参照してください。

## 起動

RustとCargoを用意し、リポジトリのルートで実行します。

```sh
cargo run --release
```

日本語を表示できる端末で、**横幅92列以上**を使用してください。
色と画面消去を使う行入力式の画面です。特殊な生入力モードには変更しません。
制御文字を使わない表示、乱数種を固定した対局も選べます。

```sh
cargo run --release -- --plain --seed 42
```

## 操作

| 入力 | 操作 |
|---|---|
| `1`〜`14` | 表示された手牌番号の牌を捨てる |
| `r 5` | 5番の牌を捨ててリーチ。成立条件を満たす場合のみ |
| `t` | ツモ和了 |
| `ron` | ロン和了 |
| `/1`、`/2`… | 画面の「可能な操作」の番号を選ぶ。鳴きの形も選択できる |
| `Enter` | 応答の見送り、局終了後の次局 |
| `h` | 操作方法を表示 |
| `q` | 終了 |

萬子は `m`、筒子は `p`、索子は `s` です。字牌は東・南・西・北・白・發・中で表示します。
リーチ、ツモ、ロン、鳴きは合法な場合だけ表示されます。不正な入力では対局状態が変わりません。
手牌は理牌済みで、ツモ牌を別途表示します。リーチ後の打牌はツモ牌に限定します。

## 実装範囲

- 136枚の牌、配牌、ツモ、打牌、四人の手番、東風戦と連荘。
- リーチ・ダブルリーチ・一発、ロン・ツモ、振聴。
- チー・ポン・大明槓・暗槓、嶺上牌、ドラ・裏ドラ・槓ドラ。
- 通常形・七対子・国士、一般的な役と役満、符と翻、本場・供託・流局精算。
- 向聴数と公開された牌を使うCPU。相手の手牌と山順序を判断へ渡しません。
- 乱数種と操作履歴による再現、自動対局、不変条件の実行時検査。

初版は赤牌・加槓・リーチ後の槓・途中流局・流し満貫・天和/地和・包を採用しません。
複数ロンは頭ハネです。喰い断あり、食い替えなしです。細則は [ルール仕様](docs/rules/riichi.md) と
[機械可読の宣言](rules/riichi.rules) にまとめています。

## 自動対局と表示確認

```sh
# 全員CPUで一局を表示
cargo run --release -- --demo --seed 0

# 配牌画面のみを表示
cargo run --release -- --snapshot --plain --seed 0

# 乱数種0〜31で32試合を検査。失敗と上限到達は非0で終了
cargo run --release -- --simulate 32 --seed 0

# 使用可能な起動引数
cargo run -- --help
```

CPUの強さや勝率を保証する評価は行っていません。向聴数、牌のつながり、公開された河を使う簡潔な判断方式です。

## 契約と検証

| 対象 | 仕組み |
|---|---|
| 採用する規則 | `rules/riichi.rules` |
| 基本操作の数式 | `rules/contracts.json` |
| 宣言の同期 | RustとLeanへ同じ式を生成し、`--check` で不一致を拒否 |
| 実行コード | `src/contracts.rs` の6関数を対局と牌処理が実際に使用 |
| Rustの検査 | Kaniで任意入力、境界、拒否条件、点数保存を検査 |
| 数学的な性質 | Leanの18定理で共通仕様との対応、保存則、境界を証明 |
| 対局の状態 | 各操作後に各牌4枚・136枚、構造枚数、供託込み総点を検査 |

### 通常の開発確認

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 scripts/generate_rules.py --check
```

### 形式検証を含む確認

[Kani 0.68.0](https://model-checking.github.io/kani/install-guide.html) と
[Lean 4.36.0 / Lake](https://lean-lang.org/install/manual/) を導入します。
Leanは `proofs/lean-toolchain` で版を固定し、外部数学ライブラリには依存しません。

```sh
cargo kani --lib --output-format terse
(cd proofs && lake build)
(cd proofs && lake env lean Mahjong/Audit.lean)
python3 scripts/check_contract_negatives.py

# 上記とRust検査・32試合をまとめて実行し、出力とソースSHAを保存
python3 scripts/verify.py
```

Pythonの補助スクリプトは標準ライブラリのみを使います。`uv run --no-project scripts/verify.py` でも実行できます。
`--quick` は形式検証と否定検査を明示的に省略する開発用モードで、全検証の代用にはしません。

## 設計と作業記録

- [文書案内](docs/README.md)
- [コンセプト](docs/concept/overview.md)
- [責務と依存関係](docs/architecture.md)
- [契約の対応表](docs/contracts/README.md)
- [形式検証の前提と限界](docs/verification/README.md)
- [作業計画書兼記録書](docs/workdocs/workdoc_Nov23-2026_suzume.md)
- [設計判断 ADR](docs/adr/)

Rust 1.80以上を対象にしています。実際に確認した版とOSは検証記録に明示します。
MITライセンスです。参照した公開プロジェクトと採用した観点は [コンセプトの出典](docs/concept/overview.md#参考にした公開資料) に記録しています。
