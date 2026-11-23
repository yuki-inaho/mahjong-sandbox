# 同梱手順の出典と版

## 取込元と対象

2026年11月23日に、この作業で使った導入済みスキルを `SKILL.md` の `name` で照合し、8種類の手順と必要な参照資料を取り込みました。下表の「取込元」はスキル名からの相対パスです。個人環境の保存先や設定は収録していません。

公開参照先は [yuki-inaho/skills](https://github.com/yuki-inaho/skills) です。ただし、導入済み版に対応する上流のGitコミットは確定していないため、取込元のSHA-256で今回の内容を識別します。過去の開発記録で参照した [`d773bd147b6ac9130c7cca95ca3afa8ff473d623`](https://github.com/yuki-inaho/skills/commit/d773bd147b6ac9130c7cca95ca3afa8ff473d623) と、今回収録した全ファイルが一致するとは判定していません。

収録対象は16ファイルです。各スキルの主文書、必須のテンプレート・評価基準・参照資料を含みます。表示用の `agents/openai.yaml` と `assets/icon.svg` は手順の実行に不要なため含めません。実行スクリプトはこれら8種類の取込元にありません。

## ファイルごとの対応とSHA-256

| 取込元（スキル名／相対パス） | 収録先 | 取込元のSHA-256 | 収録後のSHA-256 | 変更 |
|---|---|---|---|---|
| `write-workdoc-uv/SKILL.md` | [write-workdoc-uv/SKILL.md](write-workdoc-uv/SKILL.md) | `09a69d7d1f18971940ff5bc3d1a318430d77bb30902184f916b305b5e4e7a239` | `09a69d7d1f18971940ff5bc3d1a318430d77bb30902184f916b305b5e4e7a239` | 本文・ファイル名とも変更なし |
| `write-workdoc-uv/references/workdoc-template.md` | [write-workdoc-uv/references/workdoc-template.md](write-workdoc-uv/references/workdoc-template.md) | `8e4f5cfb31d8d5b75403ad0b06aa0daa5ad0248454ed3cdf3a16deb6faee8b7b` | `8e4f5cfb31d8d5b75403ad0b06aa0daa5ad0248454ed3cdf3a16deb6faee8b7b` | 本文・ファイル名とも変更なし |
| `review-written-workdoc/SKILL.md` | [review-written-workdoc/SKILL.md](review-written-workdoc/SKILL.md) | `7816d31321fbbaf3c9d0c990cfd39d85ccab01291d055ab6cbedea0ee6874176` | `7816d31321fbbaf3c9d0c990cfd39d85ccab01291d055ab6cbedea0ee6874176` | 本文・ファイル名とも変更なし |
| `review-written-workdoc/references/review-rubric.md` | [review-written-workdoc/references/review-rubric.md](review-written-workdoc/references/review-rubric.md) | `c4ad06aab2135f5be3d42a6c0cb5d447dcaf53ba22806b3d65cdcbee4ca746e3` | `c4ad06aab2135f5be3d42a6c0cb5d447dcaf53ba22806b3d65cdcbee4ca746e3` | 本文・ファイル名とも変更なし |
| `start-with-workdocs/SKILL.md` | [start-with-workdocs/SKILL.md](start-with-workdocs/SKILL.md) | `f4e4536c38f583f99fda6888cd72777d480ef90f88048cf7f423f41224c204c4` | `f4e4536c38f583f99fda6888cd72777d480ef90f88048cf7f423f41224c204c4` | 本文・ファイル名とも変更なし |
| `start-with-workdocs/references/completion_integrity.md` | [start-with-workdocs/references/completion_integrity.md](start-with-workdocs/references/completion_integrity.md) | `6686b1d9a7562fd3e26496c396f3098413f86699ecd5397a3bffefd14041ad86` | `6686b1d9a7562fd3e26496c396f3098413f86699ecd5397a3bffefd14041ad86` | 本文・ファイル名とも変更なし |
| `start-work-with-docs/SKILL.md` | [start-work-with-docs/SKILL.md](start-work-with-docs/SKILL.md) | `4eb404a2034bd1644f7bed77c4ab33a530fe0914879485b5c2d761e07f4672ec` | `4eb404a2034bd1644f7bed77c4ab33a530fe0914879485b5c2d761e07f4672ec` | 本文・ファイル名とも変更なし |
| `supervisor-orchestration/SKILL.md` | [supervisor-orchestration/SKILL.md](supervisor-orchestration/SKILL.md) | `7b1428244713408c871c1884847b40981cd571801a1bcdca0b4ee328ed45bd30` | `7b1428244713408c871c1884847b40981cd571801a1bcdca0b4ee328ed45bd30` | 本文・ファイル名とも変更なし |
| `supervisor-orchestration/references/bloom-routing.md` | [supervisor-orchestration/references/bloom-routing.md](supervisor-orchestration/references/bloom-routing.md) | `81315b6badf752741326ca8d95a36978f84613bafb0bb8340a49796ec09ab848` | `81315b6badf752741326ca8d95a36978f84613bafb0bb8340a49796ec09ab848` | 本文・ファイル名とも変更なし |
| `supervisor-orchestration/references/growth-log.md` | [supervisor-orchestration/references/growth-log.md](supervisor-orchestration/references/growth-log.md) | `8d2951246e339627ba09f6499f66c4557f028c8780f80eb596796d40bc291d63` | `8d2951246e339627ba09f6499f66c4557f028c8780f80eb596796d40bc291d63` | 本文・ファイル名とも変更なし |
| `public-repository-privacy-review/SKILL.md` | [public-repository-privacy-review/SKILL.md](public-repository-privacy-review/SKILL.md) | `b28587bf3e066639f03db80ee643105041b5fd4f7b98f8bdbed27912ac45e7eb` | `b28587bf3e066639f03db80ee643105041b5fd4f7b98f8bdbed27912ac45e7eb` | 本文・ファイル名とも変更なし |
| `write-pull-request/SKILL.md` | [write-pull-request/SKILL.md](write-pull-request/SKILL.md) | `a7289a2201c2a36322ac64e5d8d4d5b92909ee5d1806959381986bb939aa4787` | `a7289a2201c2a36322ac64e5d8d4d5b92909ee5d1806959381986bb939aa4787` | 本文・ファイル名とも変更なし |
| `review-writing-clarity/SKILL.md` | [review-writing-clarity/SKILL.md](review-writing-clarity/SKILL.md) | `f128b487dac1af469138ddf79e2a720385b4c4409587ced005ce11d3ba3c1c20` | `f128b487dac1af469138ddf79e2a720385b4c4409587ced005ce11d3ba3c1c20` | 本文・ファイル名とも変更なし |
| `review-writing-clarity/references/upstream.md` | [review-writing-clarity/references/upstream.md](review-writing-clarity/references/upstream.md) | `49cbfd780097c98a19e197e56ccb828863189b8a64fa5dbfb1a5d707543b02df` | `7fa556be546e755eb890bd68c3cf0b5ffb88a2e08d46a3a72ea29be6d73552e9` | ライセンス名・相対リンクを `LICENSE.md` へ変更 |
| `review-writing-clarity/references/upstream/LICENSE` | [review-writing-clarity/references/upstream/LICENSE.md](review-writing-clarity/references/upstream/LICENSE.md) | `6f9a2fb009225de5cca1c2a24dfed7a68bc22657e455b4a3d63f065258b455d1` | `6f9a2fb009225de5cca1c2a24dfed7a68bc22657e455b4a3d63f065258b455d1` | 拡張子を `.md` へ変更。本文は変更なし |
| `review-writing-clarity/references/upstream/writing-rules.md` | [review-writing-clarity/references/upstream/writing-rules.md](review-writing-clarity/references/upstream/writing-rules.md) | `ad350e0e6f5630fd94d8ab98a6aee194053b0096fdc944c23e271d6dfa09ee68` | `ad350e0e6f5630fd94d8ab98a6aee194053b0096fdc944c23e271d6dfa09ee68` | 本文・ファイル名とも変更なし |

## 文章規範とライセンス

[日本語技術文書の文章規範](review-writing-clarity/references/upstream/writing-rules.md)は、k16shikano氏の公開Gistの固定版 `8f2d57610a73efc97d743c9b0b0ecb1002e09fa4` です。同梱元の[出典・固定版の記録](review-writing-clarity/references/upstream.md)に示されたSHA-256と、今回の取込元および収録後の本文が一致することを確認しました。

この文章規範の [Unlicense本文](review-writing-clarity/references/upstream/LICENSE.md)も、同記録の固定版 `1092e6b60296c760071c308624e344469ec345cf` とSHA-256の情報を引き継ぎ、取込元のバイト列を保持しています。変更したのは拡張子と、そのファイルへの相対リンクだけです。このライセンスの記載を、他のスキルのライセンス表示へ拡張していません。

## プロジェクトでの適用と更新

[使用手順](README.md)と[エージェントの開発手順](../docs/development/agent-workflow.md)は、このリポジトリ向けに作成した文書です。元の手順にある `temp/` やuvの例示と、実際の作業書配置・実行コマンドとの対応は、これらの文書に記載します。取込元の本文にある例示のファイル名を、このリポジトリの実在する依存先として扱いません。

更新時は、理由と対象を決めてから取込元と現行版を比較し、必要な参照資料も一緒に更新します。上流コミットを確認できた場合にだけ、その完全なIDを記録します。ローカルで変更する場合は、元の内容を示すハッシュ値を保持し、変更理由・収録後のハッシュ値・相対リンクを更新します。上流の最新版を実行時に自動取得しません。

この一覧のハッシュ値は取込元との対応を検査するための値で、手順の有効性や実行結果の一致を保証するものではありません。新規に作成した一覧・案内文書自体のハッシュ値は表の対象外です。
