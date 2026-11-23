# 開発で使用したスキルの照合記録

この文書は初版から注釈付きmjaiの実装までの照合記録です。現在の進め方は[開発手順とスキル](agent-workflow.md)、実際に読む技能一式は[同梱スキル](../../skills/README.md)を参照してください。

## 採用したスキルと出典

利用者が指定した [yuki-inaho/skills](https://github.com/yuki-inaho/skills) の内容を、導入済みの同名スキルと照合しました。照合対象を固定するため、2026年11月23日に取得したコミット [`d773bd147b6ac9130c7cca95ca3afa8ff473d623`](https://github.com/yuki-inaho/skills/commit/d773bd147b6ac9130c7cca95ca3afa8ff473d623) を参照しています。

| 使用する手順 | 指定リポジトリとの照合結果 | 今回の採用 |
|---|---|---|
| [write-workdoc-uv](https://github.com/yuki-inaho/skills/blob/d773bd147b6ac9130c7cca95ca3afa8ff473d623/skills/write-workdoc-uv/SKILL.md) | `SKILL.md` と `references/workdoc-template.md` の全文が一致 | 導入済みの内容を使用 |
| [review-written-workdoc](https://github.com/yuki-inaho/skills/blob/d773bd147b6ac9130c7cca95ca3afa8ff473d623/skills/review-written-workdoc/SKILL.md) | `SKILL.md` と `references/review-rubric.md` の全文が一致 | 導入済みの内容を使用 |
| [start-with-workdocs](https://github.com/yuki-inaho/skills/blob/d773bd147b6ac9130c7cca95ca3afa8ff473d623/skills/start-with-workdocs/SKILL.md) | 指定リポジトリの全文を導入済み版が保持し、完了判定と中断時の扱いを追加 | 追加規則と同梱資料を含む導入済み版を使用 |

同じ名前の重複導入や、導入済み版の上書きは行っていません。必要な主文書・テンプレート・評価基準はすべて読める状態でした。参照元のファイルをこのリポジトリへコピーするのではなく、手順を今回の作業に適用しています。

### 照合したファイル

| ファイル | 指定リポジトリのGit blob SHA | 結果 |
|---|---|---|
| `write-workdoc-uv/SKILL.md` | `d34b654e062c69607d15e22db46fda218f041b22` | 全文一致 |
| `write-workdoc-uv/references/workdoc-template.md` | `c6af0c3b7db3917c75155a37d9ec03f47bcadd4d` | 全文一致 |
| `review-written-workdoc/SKILL.md` | `71ef739d90397bdb9f5c6d693ec8946dc381883e` | 全文一致 |
| `review-written-workdoc/references/review-rubric.md` | `092bc71cc12998ef15bd0633f5b2152a6132a2bd` | 全文一致 |
| `start-with-workdocs/SKILL.md` | `e95c114dc485578d35b72a0d4a3e7f1268607125` | 指定リポジトリの末尾まで一致し、導入済み版に追記あり |

照合は同じコミットを指定して取得した全文と、現在有効な個人スキルの全文との文字列比較です。Git blob SHAは指定リポジトリ側の識別子です。

導入済みの `start-with-workdocs` に追加されているのは、実終了コードに基づく完了判定、過去の失敗を消さない記録、追加指示・取消・停止の区別、受入条件の維持、終了前の全体照合です。同梱の `references/completion_integrity.md` も確認しました。この同梱資料は指定コミットにはなく、取得結果は404でした。導入済み版の追加資料が欠落しているという意味ではありません。

## このプロジェクトでの適用

注釈付きmjaiの実装で使った作業書は [注釈付きmjai作業計画書兼記録書](../workdocs/workdoc_Nov23-2026_mjai_annotated.md) です。[mjson作業書](../workdocs/workdoc_Nov23-2026_mjson.md)、[platform作業書](../workdocs/workdoc_Nov23-2026_platform.md)、初版の [suzume作業書](../workdocs/workdoc_Nov23-2026_suzume.md) は当時の実装・受入を示す履歴として保持します。作成、独立レビュー、指摘の修正、順次実行、完了照合という順番を守ります。

| 元の手順にある前提 | 今回の適用 | 根拠 |
|---|---|---|
| 作業書を `temp/` に置く | `docs/workdocs/` に置く | 利用者の明示指示を優先します。 |
| uvを中心に実装する | RustはCargo、証明はLake、補助Pythonは `uv run --no-project` を使用する | 主実装言語がRustであるためです。 |
| 主担当が一項目ずつ実行・記録する | 正本の完了印は主担当のみが順に更新する | 委任先の準備完了と主担当の検収を区別します。 |
| レビュー後は原則として直接修正する | 独立レビュー担当は指摘文書を作り、主担当が正本を修正する | 同じ文書の同時編集を避け、修正の責任を明確にします。 |
| 失敗後に原因を記録して続行する | 反例、制約不足、環境エラーを区別し、影響した検査だけを再実行する | 実行していない検査を成功にしないためです。 |

これらはこの案件への適用であり、個人スキルそのものを書き換えたものではありません。

## 順次検収と分担

主担当は正本の最初の未完了項目を選びます。補助担当は割り当てられたファイルを準備できますが、正本の完了印を変更しません。主担当が成果物・実際のコマンド出力・前提を確認した後に、その項目だけを完了へ変更し、直後に日本時間の実時刻と結果を記録します。

チェックリストの操作、確認、テスト、エラー時対処は別々に確認します。エラー時対処の項目は「想定したエラーが実際に発生した」という意味ではありません。対処手順の妥当性を確認した場合には、その確認として記録します。未実行の復旧操作を実行済みとは記録しません。

広すぎる項目は枝番へ分割します。当初の要求を削除して完了数を増やさず、追加要求は目的・手順・DoD・証跡に対応付けます。通常の修正と再検証は、すでに依頼された範囲の中で続行します。

現行の固定ツール・全体検査・配布物の消費検査は [再現手順](reproducibility.md) に従います。検査の終了コードと内容の両方を確認し、省略した範囲を全体成功へ読み替えません。

## 時刻と証跡

作業の開始と完了を記録する前に、`TZ=Asia/Tokyo date "+%Y-%m-%d %H:%M:%S %Z%z"` で実時刻を取得します。公開記録ではリポジトリからの相対パスを使い、実行環境の私的な絶対パスや認証情報を含めません。

自動検査は、実行したコマンド、対象の版、終了コード、出力内容を組として残します。後続のログ保存が成功しても、検査自体の失敗や時間切れを成功に読み替えません。修正後の再実行は旧記録と区別します。

作業書の検査先は `docs/verification/`、設計判断は `docs/adr/`、注釈付きmjai作業書の独立レビューは [注釈付きmjaiレビュー](../workdocs/review_Nov23-2026_mjai_annotated.md) へ集約します。[mjsonレビュー](../workdocs/review_Nov23-2026_mjson.md)、[platformレビュー](../workdocs/review_Nov23-2026_platform.md)、初版の [suzumeレビュー](../workdocs/review_Nov23-2026_suzume.md) は履歴です。文書を作成したことと、アプリが要求を満たしたことは別に判定します。

## 初版着手時の検証環境の動作確認（履歴）

2026年11月23日に、Rust 1.100.0、Cargo 1.100.0、rustfmt 1.11.0、Clippy 0.1.100、Kani 0.68.0、CBMC 6.11.0、Lean 4.36.0、Lake 5.0.0の起動を確認しました。Kani用のRustは `nightly-2026-08-21` です。

アプリと独立したKaniの例では、任意の2つの8ビット符号なし整数を16ビットへ拡張して加算し、結果の上下限を検査しました。1ハーネス・3検査が成功しました。Leanでは `Std` と `omega` を用い、支払額以下の残高を仮定した加減算の総和保存定理が終了コード0で検査されました。

これは検証ツールが動作する証拠です。麻雀の実処理・Kaniハーネス・Lean定理については、プロジェクト内の検査を別に実行し、その結果でDoDを判定します。
