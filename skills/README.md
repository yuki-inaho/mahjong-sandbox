# 開発に使う手順

作業書の作成、実装、レビュー、公開前の確認に使う8種類の手順を収録しています。このリポジトリ内の文書を読めば使用でき、個人用のスキル置き場への導入は不要です。プロジェクト固有の実行コマンドは[エージェントの開発手順](../docs/development/agent-workflow.md)、取込元と変更内容は[出典と版の一覧](MANIFEST.md)にまとめています。

## 一覧

| 手順 | 用途 | 同梱した参照資料 |
|---|---|---|
| [write-workdoc-uv](write-workdoc-uv/SKILL.md) | 目的・手順・完了条件を作業書にする | [作業書テンプレート](write-workdoc-uv/references/workdoc-template.md) |
| [review-written-workdoc](review-written-workdoc/SKILL.md) | 他の担当者が実行・判定できる作業書か確認する | [評価基準](review-written-workdoc/references/review-rubric.md) |
| [start-with-workdocs](start-with-workdocs/SKILL.md) | 作業書の項目を順に実行し、結果を記録する | [完了判定と中断時の扱い](start-with-workdocs/references/completion_integrity.md) |
| [start-work-with-docs](start-work-with-docs/SKILL.md) | 作業書を正本として、実行・確認・記録を進める | 主文書に収録 |
| [supervisor-orchestration](supervisor-orchestration/SKILL.md) | 複数の担当へ作業を分け、成果を確認して統合する | [役割の分け方](supervisor-orchestration/references/bloom-routing.md)、[手順の改善記録](supervisor-orchestration/references/growth-log.md) |
| [public-repository-privacy-review](public-repository-privacy-review/SKILL.md) | 公開差分に私的な情報が混入していないか確認する | 主文書に収録 |
| [write-pull-request](write-pull-request/SKILL.md) | 実際の変更と検証結果からPRの説明を書く | 主文書に収録 |
| [review-writing-clarity](review-writing-clarity/SKILL.md) | 意味と根拠を保ちながら文章を整理する | [文章規範](review-writing-clarity/references/upstream/writing-rules.md)、[出典](review-writing-clarity/references/upstream.md)、[Unlicense](review-writing-clarity/references/upstream/LICENSE.md) |

## 使用手順

1. [開発規約](../AGENTS.md)と依頼内容を読み、対象・成果物・作業書を確認します。
2. 必要な行の `SKILL.md` を開き、その中で指定された参照資料も読みます。ファイルの参照先は各スキルのディレクトリを起点に解決します。
3. 作業書を作る場合は `write-workdoc-uv` と `review-written-workdoc` を使います。実行手順は `start-with-workdocs` と `start-work-with-docs` のどちらか一方を選びます。
4. 分担が有効なら `supervisor-orchestration`、文章を直すなら `review-writing-clarity` を併用します。役割名は利用できる実行環境に合わせ、使えない仕組みを前提にしません。
5. 差分と検証結果を確認し、公開前に `public-repository-privacy-review`、PRの説明に `write-pull-request` を使います。確認した事実と未実行の範囲を分けて記録します。

## このリポジトリでの読み替え

| 元の手順にある記述 | このプロジェクトでの扱い |
|---|---|
| 作業書を `temp/` に置く | [docs/workdocs/](../docs/workdocs/)に置きます。既存作業の継続では対象の作業書を明示します。 |
| uv、pytest、just、Dockerなどのコマンド例 | テンプレートの例示です。RustにはCargo、証明にはLake、補助処理にはPythonを使います。実際の入口は[開発手順](../docs/development/agent-workflow.md)を参照します。 |
| 個人用のスキル置き場を探す、同名スキルを適用する | 本ディレクトリの対応する `SKILL.md` を読みます。外部への導入や個人設定の変更は不要です。 |
| `verify-workdoc-completion` が利用可能なら使う | 任意の補助手順です。同梱する[完了判定](start-with-workdocs/references/completion_integrity.md)に従い、成果物・実終了コード・要求との対応を直接確認できます。 |
| Herdr/OpenCodeを使う場合の `herdr-opencode-agmsg` | 特定の実行環境向けの任意の補助手順です。このプロジェクトの開発に必要な依存ではありません。 |
| テンプレートの例示リンク・仮のファイル名 | 作業に合わせて置き換えます。このリポジトリに存在する依存先や実行済みの証拠ではありません。 |
| スキル内の改善記録への追記 | 当該作業の記録は作業書へ残します。同梱手順自体を改善する場合は、内容と[出典・版の一覧](MANIFEST.md)を同じ変更で更新します。 |

同梱した原典や外部リンクを毎回取得する必要はありません。原典の固定版を使い、更新するときに出典と差分を確認します。説明用の `docs/PUBLIC_SKILLS_POLICY.md` など、元のスキル集に属する名前も、このリポジトリで実行時に読む必須ファイルではありません。
