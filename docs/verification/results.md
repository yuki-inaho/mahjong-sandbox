# 初版の検証結果

2026年11月23日、日本時間14:06:13〜14:08:28に、固定したソースに対する一括検証を実行しました。
`scripts/verify.py` の実際の処理を記録器で包み、9検査すべてが終了コード0となりました。
検査前後のソース一致と各ログのSHA256も照合しています。
機械可読の結果は [run-summary.json](run-summary.json) です。

## 実行した検査

| 対象 | 結果 | 証跡 |
|---|---|---|
| 宣言と生成物の同期 | Rust・Leanの4生成物に差分なし | [rule-sync.txt](logs/rule-sync.txt) |
| Rustの整形 | `cargo fmt --check` 成功 | [rust-format.txt](logs/rust-format.txt) |
| Rustの静的解析 | 全対象を警告なしで検査 | [rust-clippy.txt](logs/rust-clippy.txt) |
| Rustの単体・結合・CLIテスト | 76件成功、失敗・無視0件 | [rust-tests.txt](logs/rust-tests.txt) |
| 固定した種による自動対局 | 種0〜31の32試合すべて終了、失敗0件 | [simulation-32.txt](logs/simulation-32.txt) |
| Kani 0.68.0 | 実Rust関数を対象とする6ハーネスすべて成功 | [kani.txt](logs/kani.txt) |
| Lean 4.36.0 | 18定理を含むプロジェクトのビルド成功 | [lean.txt](logs/lean.txt) |
| Leanの公理依存 | 標準公理のみ。追加独自公理・`sorryAx` なし | [lean-audit.txt](logs/lean-audit.txt) |
| 否定検査 | 5件すべてで意図した不正を検出 | [negative-checks.txt](logs/negative-checks.txt) |

Rustの76件は、ライブラリ61件、実行ファイル内6件、CLI結合5件、対局結合4件です。
Kani・Lean・自動対局をこの76件に加算していません。
事前に計画した検査名と実装された名前は [検査名の対応表](test-mapping.md) で追跡できます。

32試合の検査では、各操作が各牌4枚・総数136枚、手牌の構造枚数、供託込み100,000点の不変条件を通っています。
1試合10,000操作に達した場合は失敗となり、母数から除外しません。今回の32試合に上限到達はありませんでした。
これは実行した対局の検査であり、すべての牌譜を尽くした証明ではありません。

## 実端末での操作

主担当が `./target/release/suzume --plain --seed 0` を実際のPTYで操作しました。
配牌、人間の打牌、CPUの応答、流局、東2局1本場への移行、`q` による終了コード0を観測しています。
入力列と出力は [ui-playthrough.txt](logs/ui-playthrough.txt) に保存しました。

[ui-evidence.txt](logs/ui-evidence.txt) は、この手動観測の受入記録と出力のSHA256を照合した結果です。
この照合コマンド自体が人間向けの端末操作を行ったと扱うものではありません。

## ソースと環境

一括検証のソース一覧とSHA256は `run-summary.json` にあります。
別の [source-hashes.txt](logs/source-hashes.txt) では、改行規則を含めて固定した31ファイルの一致と、
Kani・Leanの実行記録が同じ対象ファイルに結び付いていることを確認しました。
公開ログは実行環境の私的な絶対パスを置換し、終了状態と検査結果を保持しています。

Rust 1.80.1でも全76件と全対象の型検査が成功しました。
実装コミット [406986c](https://github.com/yuki-inaho/mahjong-sandbox/commit/406986c988ea0e654b5dbd2eee9a14ccfa36916e) に対する
[GitHub Actions](https://github.com/yuki-inaho/mahjong-sandbox/actions/runs/45115572572) は全5ジョブが成功し、
Linux・macOS・WindowsでRustの整形・静的解析・全76件を確認しています。
OSごとの範囲と端末目視の未確認事項は [可搬性](portability.md) に記載しています。

## 検査で見つかった問題

- 点数のKaniハーネスで、配列比較に使われた内部処理が展開上限を超えました。5口座それぞれを比較する同じ意味の検査へ直し、展開不足の検査を有効にしたまま成功させました。
- 役のない和了形を見送った場合も同巡振聴になる条件と、嶺上牌取得後の最後の打牌に対する河底の条件を修正しました。
- CPUがチーで使う一九牌を断么九の見込み判定から落としていたため、呼び出す面子の全牌を確認するよう修正しました。
- CRLFによる宣言文字列の不一致を独立再現し、`.gitattributes` でテキストの改行をLFへ統一しました。

初回失敗、修正後の部分検査、最終検査を別の記録として保持しています。
後から意図的に壊す否定検査は、実装前のテスト失敗とは区別しています。

## 証明と対応の範囲

形式検証は、次席・ドラ・1枚の移動・点数移転・リーチ条件・打牌条件の6契約です。
共通の宣言に対して、Rustの実関数をKaniで照合し、対応する数学的な性質をLeanで証明します。
役判定、対局全体、画面、乱数の公平性、生成器自体の意味保存を形式証明済みとは扱いません。
詳細は [契約一覧](../contracts/README.md) と [検証範囲](README.md) を参照してください。

実装範囲は [初版の規則](../rules/riichi.md) です。赤牌・加槓・リーチ後の槓・途中流局などの非対応を明記しています。
全体の完了照合には [固定した受入条件](completion-acceptance.md) を使い、最終公開後のHEAD一致と未保存変更の有無まで検査します。
