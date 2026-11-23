# 注釈付きmjai：検証結果

この記録は [作業書](../workdocs/workdoc_Nov23-2026_mjai_annotated.md) の追加実装に対応します。
基準commitは `d44f5ee499102fd35d81fbba5eb1e42da2e0a318`、計画commitは `c9a5d5fa4458eb9f46db7827b52e0170eeed7be5` です。
前回の [mjson移行結果](mjson-results.md) は別の履歴として保持します。

## 検証の区分

局所検査、独立した実装レビュー、本体実装のソースを固定した総合検証、固定20事例、実配布消費は完了しています。その内容をcommit `cf70f1ccc40db5f2cb5804d1a9d84aa2e1ba0686` へ保存しました。その後、利用者の追加要求によりCIをLinux最小構成へ変更し、構成差分を別に検証しました。本体検証とCI構成検証の対象を区別し、以前の書庫を最終commitから作成したものとは扱いません。最新headのLinux1ジョブ成功とmainへのマージ、検証済みtreeとの一致も確認しました。完了記録のmain反映と文書のみの差分を照合し、作業書の全項目を検収しました。

## 初回の総合検査

実行 `20261123T110029Z-8a23908067` は20段階中19段階が成功し、`rust-format` だけが失敗しました。新規APIテスト3箇所の整形差分であり、Rust212件、SDK33件、32対局、Kani6契約、Lean18定理、否定対照5件は成功しています。検証対象ソースの前後一致も確認していますが、全体の判定は不合格です。

[初回集計](annotated-full-initial.json) と [整形差分](logs/annotated-full-initial-format.txt) を保持しました。該当テストを整形し、公開前の配布内部経路の是正も含めた最終ソースで総合検査を再実行して成功しました。

## 失敗先行と修正後の確認

| 対象 | 初期の結果 | 修正後の結果 |
|---|---|---|
| 赤牌集計の境界 | 既存実装で期待1に対して11、動作失敗101 | 名前・任意追加文字列を除外し、parser32件の一部として成功 |
| Rustの新公開入口 | 未定義APIのコンパイル失敗101。動作失敗とは区別 | 原文・包装・注釈・上限・EOF・I/O・深さの13件成功 |
| SDKの新公開入口 | export未提供の9件失敗、終了1 | 原文保存、包装、注釈、入力/出力上限を含む追加9件成功 |
| API形式選択 | format未対応の実動作3件失敗101 | 形式間・全5視点・既存セッション不変の3件成功 |
| CLI利用経路 | 未対応コマンドによる6件失敗101 | 同じ6件と従来8件の計14件成功 |
| 実例検査器 | 既存14件成功、新規15件は未実装入口エラー1 | 全29件成功。期待値は元原文・固定manifestを使用 |
| 内部CRからの再読不能出力 | 修正前の誤った成功を反例が拒否、終了101 | 保存時の実CR/LF拒否を追加し成功 |
| SDK互換と破棄方針 | 追加した4件が動作失敗、終了1 | 4件成功、単体合計28件と型検査成功 |
| 公開後の報告失敗 | 公開済みを伝えない診断で反例が失敗101 | 宛先付きの公開済み診断、出力の再解析、終了0のテスト結果を確認 |

Rust担当の最初のcargo起動失敗はテスト未実行の環境失敗です。固定ツールの環境を明示した再実行と混同せず、証跡に残しました。件数は実行されたテストの集計であり、個々の入力行や否定対照の数とは異なります。最終の全体合計はRust212件、SDK33件です。[局所・段階別証跡のSHA一覧](annotated-local-evidence.json)から44ファイルを照合できます。

## 独立レビュー

[総合レビュー](../reviews/annotated-final-review.md) はPASSです。Rust/CLIとSDK/APIを実装者とは別の担当が確認し、見つけた反例を修正後に同じ条件で再検査しました。計画レビューで不足した保存改行、完了報告の取得、上限の数え方、全観測の分母、ソース固定順序も修正しました。

## 固定データと比較基準

外部データは従前の固定20件・2,310イベント・23局を使用します。実牌譜由来確認は2件、残り18件は外部回帰入力です。原文の正解は入力ファイルそのもの、集計の期待値は既存manifest、観測の比較は各イベント後の4席分です。相互変換の一致と全麻雀規則の独立再計算は別です。外部牌譜本文を公開物へ含めません。

既知牌項目だけの赤牌数は、入力SHA照合後の独立走査で20件全件が期待値と一致し、合計58件でした。manifestの変更はありません。READMEの使用例もSDK2イベントとCLI8イベントの往復を実行し、原文一致を確認しました。

## 本体実装の固定ソースと総合検証

[最終総合実行](annotated-full-run.json)は `20261123T112111Z-6cc0ea319a` です。2026年11月23日20:21:11〜20:23:34 JSTに20段階を実行し、すべて成功しました。対象140ファイルのSHA一覧は実行前後と固定時点で一致し、変更は0件です。配布検査の `source_sha256` も同じ一覧です。

この一覧をキー順のJSON（区切りはcomma/colon、空白なし）にしたSHA256は `fec04bb4677267fdfaee73f3d3568cef47620ef5120ae507543c441cac82aef4` です。実行時のHEADは計画commit `c9a5d5f`、実装は未コミットの作業ツリーでした。後に保存した本体commit `cf70f1c` の全140ファイルと同じSHAであることも照合しました。HEADだけで対象を識別せず、内容SHAで実装を固定しています。検証・作業の記録追記は検証対象コードの変更と区別し、ソース書庫に実際に入った資料のSHAは配布manifestで確認します。

| 検査 | 実結果 | 証拠・範囲 |
|---|---|---|
| 総合入口 | 20/20段階成功 | [実行集計](annotated-full-run.json)、未実行0、ソース変更0 |
| Rust | 212件成功 | [実出力](logs/annotated-final-rust-tests.txt)、失敗・無視・絞込み0 |
| 整形・Clippy | 成功、警告0 | [整形](logs/annotated-final-rust-format.txt)、[静的解析](logs/annotated-final-rust-clippy.txt) |
| SDK・型・実WASM | 33件成功 | [実出力](logs/annotated-final-sdk-tests.txt)、単体28件とnative/Node/Web結合5件、無視0 |
| 共通・配布検査器 | 21件成功 | [実出力](logs/annotated-final-verifier-self-tests.txt)、経路検査の追加8件を含む |
| 実例検査器 | 29件成功 | [実出力](logs/annotated-corpus-green.txt)、改変・欠落・同一漏出・誤集計等の拒否 |
| 固定対局 | 32/32終了 | seed 0〜31、AIの強さの測定ではありません |
| Kani | 既存6契約成功 | [実出力](logs/annotated-final-kani.txt)、対象欠落・失敗0 |
| Lean | 既存18定理 | [公理検査](logs/annotated-final-lean-axioms.txt)、許可公理だけ、未証明なし |
| 契約の否定検査 | 5/5成功 | [実出力](logs/annotated-final-negative-controls.txt)、意図した不正を拒否 |
| 配布物の消費 | 17/17段階成功 | [配布実行](annotated-package-run.json)、別ディレクトリからの実利用 |

固定環境はRust 1.100.0、Node 24.22.3、npm 11.19.1、TypeScript 7.0.2、wasm-bindgen 0.2.130、Kani 0.68.0、Lean 4.36.0、ローカル補助Python 3.12.14です。Kani・Leanは既存の基本契約を再検査したもので、新しいJSONパーサ全体を形式証明したとは扱いません。

## 追加要求：Linux最小CI

本体実装の受入後、CIの実行量・保存容量を抑え、Linuxを前提にする追加要求を反映しました。自動実行は対象コードを含むPRの1 workflow・1 Linux jobへ縮小しました。mainへのpushでは起動しません。PR全体の差分が文書だけの場合も対象外です。コードを含むPRへの文書追記まで必ず除外される、という意味ではありません。

| 区分 | 実行条件・保存内容 |
|---|---|
| 基本検査 | PR自動、Ubuntu 24.04の1ジョブ。Rust整形・静的解析・212件、生成物同期、Python検査器、文書検査、SDK型・単体28件 |
| 全体・配布検査 | 手動のみ、Linux。WASM、Kani/Lean、32対局、ブラウザ、配布消費を必要時に実施 |
| 固定20事例 | 手動のみ、Linux。外部取得・標準/注釈付き・gzipを検査 |
| 自動成果物・キャッシュ | upload/cache保存なし。Nodeの自動依存キャッシュも無効 |
| 手動成果物 | 許可した集計JSONだけ、保存1日。書庫・実行形式・画像・外部原文は保存対象外 |

[構成差分の照合](annotated-ci-scope-change.json)では、本体commitからの変更をworkflow3件、SDKのnpm scriptと単体設定2件、CI説明文書3件の計9ファイルに限定できました。対象は140から142ファイルとなり、既存133ファイルは同一です。麻雀処理、公開API、Rust/SDKの実装とテスト、証明、実例入力、依存lockは変更していません。現対象一覧のSHA256は `488010d47e6c8f2774aedab30ce53ef03f37ed02038e75f6067ecd4c90b08539` です。

[追加の局所検証](annotated-ci-local-run.json)は、新しいcheckoutへ構成6ファイルだけを反映し、WASM・SDK dist・node_modules・Rust targetがない状態から実行しました。Rust212件、SDK型・単体28件、Python検査器50件、文書checker自己試験10件、整形・静的解析・生成物同期・文書本体検査はすべて成功しました。YAMLの構造16条件とshell17箇所も確認しています。SDKの既存全体ビルドがWASM宣言なしで失敗する前提確認を別記録に残し、その生成物を削除してから新しい単体入口の成功を確認しました。

局所検証のPythonは3.12.14、GitHub指定は従前どおり3.12.10です。Actionによるツール導入とGitHub実行は局所検証に含めていません。Node Actionの固定SHAとキャッシュ無効指定は公式実装で照合し、[独立したCIレビュー](../reviews/annotated-ci-review.md)へ記録しています。重い検査を重複実行せず、上記の本体不変と元の実測を組み合わせて検収します。

[旧CIの途中記録](annotated-pr-ci-initial.json)は追加指示前の構成の履歴です。旧headの9成功・1実行中という観測を保存し、新headの受入証拠には使用しません。取消操作は接続に存在せず、補助CLIも未認証で終了4となったため、取消済みとは扱っていません。新構成の手動実行・再実行は要求していません。

## 実例全20件の結果

[最終実例集計](annotated-real-summary.json)は `mjai-real-20261123T112426155214Z-summary.json` です。固定した20件をすべて評価し、成功20、失敗0、未評価0、除外0でした。参照変換器の固定commit、入力と変換後のSHA、検査器と実行ファイルのSHAも記録しています。検査器SHAは上の固定ソースと一致します。

| 入力区分 | 件数 | イベント | 局数 |
|---|---:|---:|---:|
| 元の対局成績まで照合する実牌譜 | 2 | 559 | 5 |
| 上流の回帰入力 | 18 | 1,751 | 18 |
| 合計 | 20 | 2,310 | 23 |

全2,310イベントについて、元の標準JSONLと包装・取出し後の各行・全バイト列が一致しました。各イベント後の4席分、計9,240観測も、許可項目の集合と値の型を含めて一致しています。標準と注釈付きの両形式で、拡張子に依存しないgzip入力も成功しました。

これは固定入力での原文保持と形式間一致の実測です。任意の外部派生形式、全役・符・振聴・全時点の合法性を別の正解実装で再計算した保証へ広げません。実牌譜の本文、変換物、席別観測そのものは公開集計へ含めていません。

## 配布物と公開前の是正

正式配布検査は `20261123T112458Z-bef50e0ea7` です。[manifest](annotated-package-manifest.json)の4 crate、SDKのnpm書庫、ソース書庫、Linux実行形式の7書庫と固定lockの全8成果物について、実ファイルのSHAとサイズを照合しました。公開registryへの登録やRelease作成は行っていません。

別ディレクトリからcrateとソースを構築し、実行ファイルを起動しました。nativeは自作8イベント例の原文往復と32観測を照合し、SDKは実npm書庫から導入して公開型、原文・注釈、明示破棄と件数報告、Node/Web用の実WASMによる4席解析を確認しました。

公開前の点検で、最新WASM等に依存取得先の絶対パスが残っていたため、配布ビルドの子環境へ経路置換を加えました。[修正前の実書庫検査](logs/annotated-release-artifacts-before.json)は必須4本を検査して不合格となり、[修正後の局所確認](logs/annotated-release-artifacts-after.json)では4/4本で残存0件でした。修正前のAPI実行ファイルだけは旧版であり、今回実装による増加と解釈していません。

正式書庫についても、[必須entryと経路の検査](annotated-distribution-paths.json)はnative2本とNode/Web WASM2本の全4本を確認し、欠落・空ファイル・既知の私的prefix残存は0件です。新規検査の[初期失敗](logs/annotated-release-paths-red.txt)、[最終21件成功](logs/annotated-release-paths-final.txt)を分けて保存しています。WASM再構築時のPATH不足も[失敗集計](logs/annotated-release-build-env-failure-summary.json)と[固定環境での成功](logs/annotated-release-wasm-fixed-env-summary.json)として保持しました。

[独立した公開前レビュー](../reviews/annotated-publication-review.md)はPASSです。[現物監査](annotated-package-audit.json)では7書庫の482ファイル、依存lock1件、付随metadata・ログ39件を確認し、私的経路・認証値・外部原文の有意な候補は0件でした。ソース書庫355ファイルもmanifestの個別SHAと一致しました。機械走査と内容・由来の確認を併用しており、既知prefixの検査だけをあらゆる秘密情報の検査や全OSでの無条件の保証とは扱いません。

## 利用側の変更と保持しない情報

JSON要求の形式省略、標準mjsonを読む既存入口は維持しています。Rustで `ParsedEvent` または `Request::AnalyzeMjai` を直接生成するコードは、新しい `json` または `format` 項目への対応が必要です。保存用の原文と、数値が丸められ得る解析値を混同しない使い方をcrateとSDKのREADMEへ記載しました。

原文保持の対象は受理したイベント・注釈オブジェクトの内部です。外側の空白、改行の種類、最終改行、gzipの圧縮バイト列は正規化され得ます。非空注釈を捨てる取出しは明示指定が必要で、破棄した注釈の復元や、元記録にない旧seed/action履歴の逆生成は提供しません。低水準の逐次出力は正常終端まで暫定です。CLIの公開前の失敗は未公開とし、公開後の報告失敗は公開済みの宛先を明示して完成物を残します。


## PRのCIとmainへのマージ

[PR #2](https://github.com/yuki-inaho/mahjong-sandbox/pull/2)は2026年11月23日20:59:28 JSTにマージされました。[CI証跡](annotated-pr-ci.json)と[完了照合記録](annotated-completion.json)から対象SHA、実行ID、treeの対応を確認できます。

| 対象 | 識別子・実結果 |
|---|---|
| 本体実装 | `cf70f1ccc40db5f2cb5804d1a9d84aa2e1ba0686`。総合・実例・配布の固定対象 |
| CI縮小後の検収head | `0ac5f9af2c7c10f16e5772dbfb6d49c359803fd2` |
| マージ先の基準 | `d44f5ee499102fd35d81fbba5eb1e42da2e0a318` |
| merge commit | `d66d6a2aae9645112b7ba955b26260452c5e16b1` |
| headとmergeのtree | `99d4a30d080a896de20676155167922a994999b4`、完全一致 |
| 最終CI | [run 45169714634](https://github.com/yuki-inaho/mahjong-sandbox/actions/runs/45169714634)、PR自動1 workflow、Linux1 job、成功、再実行なし |
| 成果物 | 同実行のartifact 0件 |

ジョブは20:56:57〜20:58:17 JSTの80秒で完了しました。GitHubのPR head・base・check-runs・run・artifactと、取得したローカルGitのmerge treeを別に照合しています。`expected_head_sha` を指定してマージし、未検収の更新を取り込んでいません。mainへのpushを起動条件に持たせていないため、マージ後や完了記録のための重複CIは要求していません。

完了記録commit `f2bad84d64a6958f906717efb411897ef24a58e1` は4文書だけの変更としてmainへ保存しました。公開後に再取得し、GitHub main、origin/main、2つのローカル作業コピーへの反映と変更残り0を確認しました。142項目のソース識別子もCI検収時と同じです。mergeと同記録commitに対応するworkflow runは各0件でした。この反映確認に基づき、作業書の全手順・D1〜D9を完了へ更新し、その確認結果を文書だけで追記しています。
