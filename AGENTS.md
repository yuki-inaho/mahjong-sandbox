# 開発規約

## 目的と作業入口

ひとりで麻雀を遊び、対局を振り返り、AIの判断を試すための「麻雀スズメ」です。
注釈付きmjai実装の記録は `docs/workdocs/workdoc_Nov23-2026_mjai_annotated.md` に保持します。
mjson移行の記録は `docs/workdocs/workdoc_Nov23-2026_mjson.md` に保持します。
API基盤拡張の記録は `docs/workdocs/workdoc_Nov23-2026_platform.md` に保持します。
初版の記録は `docs/workdocs/workdoc_Nov23-2026_suzume.md` に保持します。
コンセプト・仕様・契約・ADRは `docs/`、宣言されたルールは `rules/` に置きます。

## 同梱スキルと実行手順

開発で使用するスキルは [skills/README.md](skills/README.md) から選び、必要な `SKILL.md` と同梱資料を読みます。個人環境のスキル保存先を前提にせず、このリポジトリ内の手順を使ってください。

実行する検査と結果の扱いは [開発手順](docs/development/agent-workflow.md)、日本語の説明とREADMEの構成は [文章指針](docs/development/writing.md) に従います。新しい作業で、完了済みの記録を未完了に戻したり、過去の検証結果を上書きしたりしません。

## 実装と検証

- ルールを先に宣言し、操作の事前条件・事後条件・不変条件をコードと文書で対応付けます。
- Rustの実行処理、Kaniの検証、Leanの仕様・定理を別の責務として扱います。
- ルールの変更は宣言、契約、証明、テスト、関連文書を同じ変更で更新します。
- SOLID / KISS / DRYを守ります。不要な抽象化や複数箇所でのルール定数の手修正を避けます。
- 判定・対局処理から端末入出力、乱数源、ファイル保存を分離します。
- 不正な操作では状態を変更しません。公開された操作ごとに不変条件を検査します。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo kani -p suzume --lib`、`cd proofs && lake build` を検証します。
- 未実行、失敗、時間切れを成功と記録しません。証明の前提と対象範囲を明記します。
- 文書は日本語で記述し、作業書のチェックを更新した直後に実時刻付きの記録を追記します。
- 公開物に実行環境の私的な絶対パス、認証情報、会話ログを含めません。
- 利用者の依頼に従い、検証可能な区切りでコミット・pushします。force pushしません。
