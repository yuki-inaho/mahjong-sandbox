# すずめ0.2.0の実行・配布・連携検証結果

検収日：2026年11月23日（日本時間）。対象はRustの対局処理を共有する端末、型付きAPI、WASM、TypeScript SDK、設定可能なAI、研究環境、対局ログです。[要求とDoD](../workdocs/workdoc_Nov23-2026_platform.md)、[実テストとの対応](platform-test-mapping.md)、[問題と修正根拠](../reviews/platform-review.md)を併せて保持します。

## 対象の識別

実装の公開コミットは `8405e74361978ed79a8aaa3bbac40ebd589ab147`、統合修正後は `c3b7458af3042905403cfc63a6170c0487b34ff5` です。後者のtreeは `6416bdab8fd6cd33ad0ae243f54fb7906aad12c5` です。以下はこの実装・設定に対する実行結果であり、その後の受入記録の追記と区別します。

検証入力120ファイルのパスとSHAは[総合実行記録](platform-run.json)に含めています。前後の入力は一致し、入力mapのSHA256は `3a9fe7a37c1026b203d9c43e0ced21e77e6d7a2e381915a6d931207b43edeb4f` です。配布書庫の文書は書庫作成時点の別manifestで識別します。最終公開のHEADとCIは作業書の外側の受入記録で再照合し、この文書自身のハッシュを本文へ埋め込む循環を避けます。

## 実行した検査

| 検査 | 実結果 | 範囲・根拠 |
|---|---|---|
| 総合入口 `verify_platform.py --profile full` | 20系列すべて成功、省略0 | [機械可読記録](platform-run.json)。終了値に加え、期待した検査名・母数・失敗や無視の有無を照合 |
| Rust単体・結合・doctest | 139件成功、失敗・無視・絞込み0 | [全出力](logs/platform-final-rust-tests.txt)。core、API、CLI、環境、ログを含む |
| 整形・Clippy | 成功、Clippy警告0 | 総合記録の `rust-format` / `rust-clippy` |
| 公開Rust文書 | 別の構築先で再生成し成功、警告0 | `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --locked`。CLIの同名文書による上書きを修正した後の実行 |
| 固定32種の自動対局 | 32/32東風戦が終了 | seed0〜31。AI強度評価とは別の対局進行検査 |
| TypeScript・実WASM・native結合 | strict型検査と12件成功 | [SDK出力](logs/platform-final-sdk-tests.txt)。同一64操作等の応答比較、不正入力・古い操作・ログ読込も含む |
| 宣言生成器 | 19件成功、生成差分0 | 実Rust/Leanを使う7関係19例の意味照合も成功 |
| Kani | 6ハーネスすべて成功 | [実出力](logs/platform-final-kani.txt)。実Rustの6種類の基本契約 |
| Lean | 18定理を検査、公理検査成功 | [公理出力](logs/platform-final-lean-axioms.txt)。許可した公理だけに依存 |
| 否定検査 | 5件すべて意図した拒否 | 不正宣言・生成物改変・誤実装・偽定理を受理しない |
| 検証器自身 | 13件成功 | 空の集計、検査の欠落、timeout、未証明などを拒否 |
| 最小Rust版 | Rust/Cargo 1.80.1、core83件と全対象確認が成功 | API・CLI・SDKはこの最小版の保証対象に含めない |

固定環境はRust 1.100.0、Node 24.22.3、npm 11.19.1、TypeScript 7.0.2、wasm-bindgen 0.2.130、Kani 0.68.0、Lean 4.36.0です。CIの補助Pythonは3.12.10です。ローカル補助検査はPython3.12.14で実行しました。導入の前提と手順は[再現文書](../development/reproducibility.md)にあります。

## 実ブラウザ

GitHub Actions上でHTTP配信した実際の画面を、Playwright CLI 0.1.25とChromium 159.0.8154.6（revision1255）で操作しました。[11項目の実結果](platform-browser-assertions.json)と[ログ・画像SHAを含む記録](platform-browser-summary.json)を保存しています。

初期配牌14枚、画面の牌数、非公開情報の制限、打牌、CPUの1操作、自分の手番への進行、不正seedで状態が変わらないこと、u64最大seedでの再開、配牌の変化、エラー表示の復旧、コンソールとページのエラー0件を確認しました。状態はsession1/revision0からrevision7へ進み、再開後はsession2/revision0です。

初期画面と再開後の2枚のPNGは異なる内容で、それぞれ91,020byteと95,522byteです。画像のSHAは上記JSONにあります。画面と原ログは[CIの配布・検査成果物](https://github.com/yuki-inaho/mahjong-sandbox/actions/runs/45133691666)に保存しました。ローカル環境でのブラウザ起動は制限により成功していません。この実績はCI上の実ブラウザ操作であり、NodeからWASMを呼ぶ検査とは別です。

## AIの固定条件評価

事前にseed1000〜1007と全4席、候補と対照各32東風戦を固定しました。他の3席はどちらもGreedyです。結果を見た後の設定調整や失敗試合の除外は行っていません。

| 対象 | 試合数・失敗 | 平均順位 | 平均最終点 | 1着/2着/3着/4着 |
|---|---:|---:|---:|---|
| Ukeire候補 | 32・0 | 2.5625 | 24,650 | 6 / 9 / 10 / 7 |
| Random対照 | 32・0 | 3.6875 | 15,384.375 | 0 / 0 / 10 / 22 |

同じ条件を別CSVへ64試合再実行し、時間以外の全列が初回と一致しました。再現実行を追加の独立した強度測定には数えません。詳細と全データは[評価結果](../ai/results.md)と[事前計画](../ai/evaluation.md)にあります。この比較からGreedyや人間への優越、プリセット間の勝率の単調な順序までは結論しません。

## ログ・分析・研究環境

完全ログと共有ログで各1東風戦を実行し、同じ最終点 `[11300, 19700, 49400, 19600]` を確認しました。完全ログの1,235操作を再生し、1,235行の分析用遷移へ変換して、正確な操作・189行動ID・4席の点数差・供託差・終了情報を全行照合しました。JavaScript側のストリーム解析にも37byteずつ渡し、完全・共有ログと遷移の読込を確認しました。

共有ログの再生、末尾欠落、状態要約の改変、既存出力への上書きは、4件とも所定の理由で拒否しました。解析途中のデータは末尾とEOFの検証完了まで暫定です。ファイル出力は全検証後に確定させ、既存ファイルを置換しません。

研究環境は1席の1操作を1stepとする多エージェント形式です。`terminated`は通常の対局終了、`truncated`は操作上限や外部中断です。打切り時は`final_observation`を残し、学習器がその局面の価値を推定できます。完全情報で分岐する実験用`fork`は信頼するRustコード向けであり、通常のAI観測に相手の手牌や実際の山を含める入口ではありません。

JSON SchemaとTypeScriptの構造検査だけでは、操作の合法性や全ログの再生一致を証明できません。これらは同じRustエンジンによる再生で検査します。[ログ仕様](../logging/README.md)、[環境仕様](../api/environment.md)、[算法の追加方法](../ai/extension.md)に境界を定義しています。

## 配布物とOS

配布処理の14工程が成功しました。4つの`.crate`、SDKのnpm書庫、ソース書庫、Linux実行形式、workspaceのlockを作成し、全SHAを照合しています。元の作業ディレクトリを参照しない別領域で、crateの構築と起動、実行形式の起動、npm書庫の導入・strict型検査・Node/ブラウザ入口の実WASM操作、ソース書庫の構築・文書リンクを検査しました。

このローカル配布時点のソース書庫は229ファイルでした。後から追加した受入記録はその書庫に遡及して含まれていたとは扱いません。最終公開のCIでも、そのコミットに含まれる文書とコードを再梱包し、独立消費を検査します。

[固定版のCI](https://github.com/yuki-inaho/mahjong-sandbox/actions/runs/45133691666)はLinux/macOS/Windowsの3ジョブと、Linuxの形式検証・SDK・実ブラウザ・全配布の1ジョブが成功しました。[互換性のCI](https://github.com/yuki-inaho/mahjong-sandbox/actions/runs/45133691652)は3OSの最新stable、coreのRust1.80.1、Lean/Kaniの計5ジョブが成功しました。

npm/crates.ioへの登録は行っていません。すべてのLinuxの動的ライブラリやCPU、未検査OSで同じバイナリが動くという保証でもありません。

## 失敗の保持と証明の限界

初回のCIではPython3.12.14のWindows/macOS用配布がなく導入に失敗し、ブラウザ検査ではCPU設定欄のラベル一致に失敗しました。[実失敗の記録](logs/platform-ci-integration-failures.txt)を保持し、配布のあるPython3.12.10と明示したアクセシブル名へ修正した後の別実行を成功記録としています。公開Rust文書の同名出力衝突も解消し、新しい構築先で再生成しました。

LeanがRustプログラム全体を直接証明した構成ではありません。型と入力域を定義した共通宣言からRust/Lean仕様を生成し、Rustの6契約との対応をKaniで検査し、Leanで18の性質を証明します。役判定、対局全体、AI、外部I/O、JSON処理全体は実行時契約とテストの対象です。ログの状態SHAも署名や完全な隠れ状態の証明ではありません。詳細は[形式検証の範囲](README.md)に記載しています。
