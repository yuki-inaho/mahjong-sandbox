# エージェントの開発手順

この文書は、麻雀スズメの変更を調査から検証・記録まで進めるための手順です。Linuxを作業環境とし、リポジトリ内の[開発規約](../../AGENTS.md)、[同梱スキル](../../skills/README.md)、`scripts/` と `tools/` を使用します。個人用のスキル導入や設定変更は必要ありません。

## 作業を始める

依頼内容と既存の変更を確認し、今回の対象、成果物、変更しない範囲を作業書に記載します。継続作業なら該当する作業書を正本とし、別の課題なら `docs/workdocs/` に作成します。追加指示が届いた場合も、残っている要求との対応を確認して反映します。

リポジトリのルートで、作業ブランチ、基点となるコミット、未確定の差分、実時刻を確認します。

```sh
git status --short
git branch --show-current
git rev-parse HEAD
TZ=Asia/Tokyo date "+%Y-%m-%d %H:%M:%S %Z%z"
```

他の担当者の差分を取り消さず、編集するファイルを割り当てます。作業を分ける場合は、成果物、担当範囲、確認方法、未解決事項の報告先を先に決めます。

## 作業書を作成・レビューする

[write-workdoc-uv](../../skills/write-workdoc-uv/SKILL.md)と[作業書テンプレート](../../skills/write-workdoc-uv/references/workdoc-template.md)を読み、目的、実行手順、完了条件、検証方法を対応付けます。[review-written-workdoc](../../skills/review-written-workdoc/SKILL.md)と[評価基準](../../skills/review-written-workdoc/references/review-rubric.md)で、別の担当者も実行・判定できるか確認します。

テンプレートは複数の開発環境向けの例を含みます。このプロジェクトでは次のように適用します。

| 項目 | 実際の扱い |
|---|---|
| 作業書の配置 | `docs/workdocs/`。参照する作業書を明示する |
| Rustの構築・テスト | Cargo。依存は[Cargo.lock](../../Cargo.lock)で固定する |
| 形式検証 | Kaniと、[proofs/](../../proofs/)内のLake・Lean |
| 補助処理と文書検査 | Python 3.12以上の標準ライブラリ。Python用の追加パッケージ導入は不要 |
| SDKの依存 | [packages/suzume/package-lock.json](../../packages/suzume/package-lock.json)を使用する |
| 道具の版と配布物 | [tools/versions.json](../../tools/versions.json)と[再現手順](reproducibility.md)を正本とする |

テンプレート内のuv、pytest、just、Docker、仮のモジュール名は、そのまま実行するコマンドではありません。必要な実行入口を以下から選び、該当しない項目をこのプロジェクトの必須条件に加えません。

## 実装と記録を進める

[start-with-workdocs](../../skills/start-with-workdocs/SKILL.md)または[start-work-with-docs](../../skills/start-work-with-docs/SKILL.md)の一方を使います。主担当は最初の未完了項目から進め、成果物と実際の検証結果を確認した後、その項目の完了印と実時刻付きの記録を更新します。準備、実行、検収を混同しません。

動作を変える場合は、要求を表す失敗する事例を先に確認し、実装後に同じ事例が通ることを確認します。牌譜を使う変更では、入力元、固定した版、期待するイベントや失敗位置を記録します。記録形式の変換、原文の保持、未知の拡張項目、壊れた入力など、変更が影響する境界を選んで確認します。

分担には[supervisor-orchestration](../../skills/supervisor-orchestration/SKILL.md)を使います。主担当は担当者の要約だけで完了とせず、差分、成果物、実出力を確認して統合します。同じファイルや正本の完了印を複数の担当で同時に編集しません。

記録には実行したコマンド、対象の版・入力、終了コード、結果、未確認の範囲を残します。失敗や時間切れは保持し、修正後の結果を別に追記します。時刻や成功件数を推測で埋めません。

`scripts/worklog.py` は初版の作業書と当時の見出しに対象を固定した補助処理です。新しい作業書に対する汎用の記録手段にはなっていません。新規作業では対象ファイルの完了印と記録を直接更新してください。

## 変更に応じて検証する

文書だけの変更では軽いローカル検査を使います。動作変更では該当する事例を優先し、影響が残る範囲や必要な受入条件に応じて検査を広げます。未実行の検査を、過去の成功や別の検査の成功で代用しません。

| 対象 | 入口と確認内容 |
|---|---|
| 文書・同梱スキル | 下記の文書検査と差分検査。参照資料の欠落、相対リンク、記載した実行対象、不要な空白を確認する |
| Rustの動作 | 変更した対象のCargoテストを実行する。通常の統合検査は [verify_platform.py](../../scripts/verify_platform.py) の `native` を使う |
| SDK・WebAssembly | 同スクリプトの `frontend` を使う。SDKの基本検査と生成したWASMの結合検査を区別する |
| ルール・契約・証明 | 宣言、Rust、Kani、Lean、関連文書をそろえて更新する。同スクリプトの `formal` または `full` を使う |
| 外部の実牌譜 | [verify_mjai_real.py](../../scripts/verify_mjai_real.py)を必要な場合に実行し、入力の固定情報と結果を確認する |
| 配布物・実ブラウザ | [package.py](../../scripts/package.py)、[check_browser.py](../../scripts/check_browser.py)を該当する変更時に使う。準備と対象範囲は[再現手順](reproducibility.md)を確認する |

### 文書だけを変更した場合

Pythonが使える環境で、リポジトリのルートから実行します。Rust、Node、証明用の道具を導入する必要はありません。

```sh
python3 scripts/check_documentation.py --json
git diff --check
```

文書検査は相対リンク・見出し・実行対象などの静的な整合を確認します。説明がコードの挙動や依頼に合っているかは、該当する仕様・実装・差分を読んで確認します。文章の修正には[review-writing-clarity](../../skills/review-writing-clarity/SKILL.md)を使います。同梱スキルを更新した場合は、[出典と版の一覧](../../skills/MANIFEST.md)にある内容のSHA-256も更新します。

### 動作を変更した場合

必要な道具を [bootstrap.py](../../scripts/bootstrap.py)で導入し、[doctor.py](../../scripts/doctor.py)で版を確認します。たとえばRustの変更に対する通常の検査は次のとおりです。

```sh
python3 scripts/bootstrap.py --profile native
python3 scripts/doctor.py --profile native
python3 scripts/verify_platform.py --profile native
```

`bootstrap.py` は外部の公式配布物を取得し、版とハッシュ値を照合して `dist/tools/` に置きます。毎回の文書編集に必要な処理ではありません。既に用意された道具を使う場合も、`doctor.py` の結果で適合を確認します。Cargoなどを直接実行するときは、導入後に `source dist/tools/activate.sh` でその道具を選びます。

SDKの基本検査は `npm --prefix packages/suzume run test:unit` です。固定したNodeとnpm、SDKの依存、および先行するRust検査で構築するnative API実行ファイルが必要です。`frontend` の検査はWebAssemblyを構築し、その結合も確認します。単体検査だけを全体の成功と記録しません。

### 総合検査が必要な場合

形式検証を含む導入はLinux x86_64を対象とします。総合検査は、変更の影響や完了条件から必要と判断した場合に手動で実行します。

```sh
python3 scripts/bootstrap.py --profile full
python3 scripts/doctor.py --profile full
python3 scripts/verify_platform.py --profile full
```

`verify_platform.py` は選択した検査群を実行し、環境、各検査の終了結果、開始前後のソースのハッシュ値を記録します。`full_verification` は、`full` を選び、環境と検査が適合し、検査中に対象ソースが変わらなかった場合に成立します。外部の実牌譜、配布物の利用検査、実ブラウザの検査は別の実行入口であり、この値だけでは成功を示しません。

生成された実行記録は `dist/verification/<実行ID>/summary.json` と同じ実行ディレクトリ内のログに保存されます。主担当は必要な結果を確認してから、公開可能な範囲を作業書へ転記します。ソースのハッシュ値の一致だけで、動作や証明の正しさを結論付けません。

## CIを必要最小限にする

[通常のCI](../../.github/workflows/ci.yml)は、プルリクエストに対するLinuxの1ジョブです。`docs/**`、Markdown、ルートのライセンスだけを変更する差分は対象から外れます。自動で成果物を保存せず、追加の配布・形式検証・実牌譜検査は手動の実行入口に分けています。

文書だけの修正で、確認表示を得るためにCIを起動しません。ローカルの文書検査とレビューを記録します。パスによる除外はPR全体の差分に対して判断されるため、同じPRにコード変更がある場合は、文書のコミットだけ追加しても除外されるとは限りません。

必要な場合の手動CIは [platform.yml](../../.github/workflows/platform.yml) と [mjai.yml](../../.github/workflows/mjai.yml)です。どちらもLinuxで実行し、許可したJSONだけを1日保存します。文書編集の完了条件には含めません。

## 公開前と完了時に確認する

[public-repository-privacy-review](../../skills/public-repository-privacy-review/SKILL.md)で、公開する差分、検証記録、PRの説明を確認します。公開記録はリポジトリからの相対パスを使い、実行環境の私的な絶対パス、認証情報、会話ログを含めません。コマンドの終了結果を見せるために、未確認の生ログをそのまま公開しません。

[write-pull-request](../../skills/write-pull-request/SKILL.md)に従い、解決する問題、変更後の挙動、実施した検証と残る制限を説明します。ローカル検証とCIの結果は区別します。ブランチ作成・コミット・push・マージは依頼された範囲で進め、対象のブランチと変更内容を確認します。force pushは行いません。

最後に[完了判定の手順](../../skills/start-with-workdocs/references/completion_integrity.md)で、当初と追加の要求、成果物、検証結果、作業記録を照合します。残る未確認事項を明記し、単なる完了印や担当者の報告を依頼全体の完了へ読み替えません。
