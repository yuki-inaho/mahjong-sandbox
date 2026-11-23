# Linux最小CIへの変更：独立レビュー

**判定：PASS（2026年11月23日20:54:31 JST）。** 設定・局所検証・作業書の追加条件を受け入れます。未解決の重大指摘はありません。最終headに対するGitHub上のLinux1ジョブの成功は、後続の手順18で別途検収します。

## 対象と判断の根拠

利用者の追加指示によるCI最小化・Linux前提への変更を対象にしました。基準は `cf70f1ccc40db5f2cb5804d1a9d84aa2e1ba0686` です。独立担当は3 workflow、単体用TypeScript設定2つ、npm script、README・SDK README・再現手順、作業書のA-07・17-b・D9を読み取り確認し、製品・検証対象ソースは編集していません。

[差分とソース識別子](../verification/annotated-ci-scope-change.json)を実ファイルとGitの基準commitに対して独立に照合しました。従来の140項目は基準commitと全件一致し、現在の142項目との差はCI・単体設定6項目とCI説明3項目の計9項目だけです。残る133項目は同一で、麻雀処理、公開API、既存テスト、証明、固定事例、依存lockの変更はありません。従来の総合・実例・配布結果は基準ソースの証拠として保持し、その書庫をCI変更後の最終commitそのものの配布物とは扱わない区分が明記されています。

## 指摘と修正確認

**CI-01（Minor、解消）：** 初回の作業書12-b・17には、識別対象の変更を無条件に14〜16へ差し戻す文言が残っていました。新17-bで許容したCI専用差分にも重い再検査を要求すると誤読できるため、適用範囲の明確化を求めました。

修正後は、17-bで列挙したCI構成・SDK単体設定・CI説明文書だけの変更を17-b/D9で検収し、本体変更にはこの例外を適用しないことを両手順に明記しています。追加要求の根拠、差分・本体不変・局所検証、実GitHub上の最終head確認がつながり、既存の不合格履歴や対象SHAも書き換えていません。

## 設定・実行範囲の確認

| 対象 | 確認結果 |
|---|---|
| 自動実行 | `ci.yml` の `pull_request` だけ、`ubuntu-24.04` の1ジョブ。3定義のいずれにもmain pushの起動条件はありません。 |
| 文書の除外 | `paths-ignore` は**PR全体の差分**が `docs/**`、Markdown、LICENSEだけの場合に適用されます。コード変更を含むPRへ文書commitを追加しただけでは、文書のみのPRにはなりません。 |
| 自動保存 | artifact uploadと独立したcache処理はありません。固定SHAのsetup-node実装では `package-manager-cache: false` と `cache` 未指定により依存キャッシュを起動せず、setup-pythonも `cache` 未指定です。 |
| 自動検査 | Rustの整形・Clippy・全workspace検査、宣言・Schema・型生成物同期、Python検査器、文書、SDK型・単体を対象にします。WASM生成・Kani/Lean・32対局・ブラウザ・外部20事例・配布は自動ジョブから外れています。 |
| SDK単体設定 | `src/index.ts` から到達するSDKと公開型の利用例を、既存の厳格な型設定を継承して検査します。Node/Webのfactoryと実WASM結合5件は対象外と文書に明記し、従来のbuild/typecheck/test入口は維持しています。 |
| 手動実行 | `platform.yml` と `mjai.yml` は `workflow_dispatch` のみで、各Linux1ジョブです。保存は明示した集計・目録等のJSONに限定し、保持は1日。書庫・実行ファイル・WASM・PNG・全ログ・ブラウザ用の経路設定JSONは保存対象に含めません。 |
| 同時実行 | 同じworkflow・PRまたはrefの古い実行を取り消す設定があります。旧設定で既に起動した実行の取消が成功した証拠とは扱いません。取消未実施の履歴は作業書に保持されています。 |
| マージ後 | 検証済みheadとのtree一致、完了記録の文書のみの差分、mainへの反映を確認する手順です。重複CIを完了条件として復活させていません。 |

## 局所検証の確認

[局所検証記録](../verification/annotated-ci-local-run.json)の設定6ファイルと公開5証跡のSHAが現物と一致することを確認しました。独立にYAMLを読み取り、起動条件・ジョブ数・Linux指定・保存先を照合しています。

別担当のクリーンな配置で、Rust212件、SDK単体28件、Python検査器50件、文書自己試験10件が成功し、型・生成物同期・整形・Clippyも成功しています。WASM、SDKのNode/Web生成入口は実行前後とも存在しません。従来の全体buildがWASM宣言の欠落で失敗した記録は前提確認として保持され、製品の動作不具合を示す失敗先行テストへ読み替えていません。

局所実行のPythonは3.12.14、GitHub設定は3.12.10です。GitHubのランナーやAction導入手順を局所実行済みとする記述はなく、`github_execution: false` と未実施範囲が明記されています。構造検査の取消に関する項目も設定の評価であり、実際の取消実績を証明するものではありません。旧10ジョブや手動の重い検査は、本レビューのために再実行していません。

## 一次資料と残る検収

固定commitの[setup-node入力定義](https://github.com/actions/setup-node/blob/949feb2413d6458794dcd2491c4babbbce0c15c1/action.yml)と[実装](https://github.com/actions/setup-node/blob/949feb2413d6458794dcd2491c4babbbce0c15c1/src/main.ts)、[setup-python実装](https://github.com/actions/setup-python/blob/6d4fd22b20465ccc0f7354476588e8df7c9ca0e0/src/setup-python.ts)、[upload-artifact入力定義](https://github.com/actions/upload-artifact/blob/cf430e030ddbb5b0abf93d22962f4752f3646cd9/action.yml)をGitHub接続で直接確認しました。パス除外の判断は[GitHubのworkflow構文](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushpull_requestpull_request_targetpathspaths-ignore)とも照合しています。

本レビューのPASSは設定と局所証拠の受入です。手順18では新しいheadの1 workflow・1ジョブの実結果を確認し、旧headの成功、旧設定の実行、手動検査の結果で代用しない条件を維持します。
