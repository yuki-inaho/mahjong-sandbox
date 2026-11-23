# 可搬性と最小Rust版の確認

## 確認結果

Rustの最小対応系列を1.80とし、**Linux x86_64でRust 1.80.1による全76テストと全対象の型検査が成功**しました。実行ログは [msrv.txt](logs/msrv.txt) です。検査前後のRustソースのSHA256が一致したことも同じログに記録しています。

| 対象 | 条件 | 結果 |
|---|---|---|
| 最小対応系列 | Rust 1.80.1 / Cargo 1.80.1 / Linux x86_64 | `cargo +1.80.1 test --locked`：76件成功、失敗・無視0件 |
| ライブラリ・実行形式・テストの型検査 | 同上 | `cargo +1.80.1 check --locked --all-targets`：終了0 |
| 依存固定ファイル | Cargo.lock形式3、外部Rust依存なし | 1.80.1で読込成功。変更不要 |
| Rust 1.80.0 | 同系列の最初の版 | 未実行。実測した最小版は1.80.1 |
| macOS | GitHub Actionsに通常のRust検査を定義 | この作業環境では未実行 |
| Windows | GitHub Actionsに通常のRust検査を定義 | この作業環境では未実行 |
| 色付き端末表示 | ANSI色指定と画面消去、日本語を含む横92桁の表示 | Linux端末で確認。他OSの端末・フォントは未確認 |
| 制御文字を使わない表示 | `--plain`、EOF、不正入力からの復帰 | 外部プロセスによるCLIテストに含め、1.80.1でも成功 |

最終検査の開始時刻は2026年11月23日14:00:48 JSTです。これは指定したソースに対する互換性の確認であり、すべてのOSや今後のRust版についての保証ではありません。

## 初回失敗と修正後の確認

最初の1.80.1検査では、ビルドと全対象の型検査は成功しましたが、嶺上ツモ後の最終打牌に河底撈魚が付く場合のテストが失敗しました。Rust版の非互換ではなく、対局処理の役判定条件に対する失敗です。初回の結果は [msrv-initial-failure.txt](logs/msrv-initial-failure.txt) に保持しています。

主担当が該当条件を修正した後、同じRust 1.80.1で全テストと型検査を再実行し、成功を確認しました。初回失敗を削除したり、ビルド成功だけをテスト成功に読み替えたりしていません。

さらにCPUの断么九見込み判定が修正され、回帰テストが1件追加されたため、その版でも全76件と全対象の型検査を再実行しました。変更前の75件成功は [msrv-before-ai-fix.txt](logs/msrv-before-ai-fix.txt) に分けて保持しています。

## 改行コードの具体的な問題と対処

実行時の宣言照合は、生成Rustファイルの文字列と `include_str!` で読み込む宣言を比較します。Rust 1.80.1の独立した再現で、CRLFのソースに含まれる生文字列はLFへ正規化される一方、`include_str!` は元のCRLFを保持することを確認しました。

再現した値は、生文字列が `[97, 10, 98, 10]`、読み込んだ文字列が `[97, 13, 10, 98, 13, 10]` で、同一ではありません。この差があると、生成物を改変していなくても宣言照合が失敗します。

そのため、リポジトリの [.gitattributes](../../.gitattributes) に `* text=auto eol=lf` を定めています。CIだけで改行を変更するのではなく、各OSで取得するテキストの改行規則をリポジトリに保持します。対応元は [contracts.rs](../../src/contracts.rs) の `declarations_current` です。Windows上の最終確認はCIの実行結果で別途判断します。

## CIの定義と実行状況

[ci.yml](../../.github/workflows/ci.yml) に次を定義しました。この文書の作成時点では、新しいワークフローのGitHub上での実行結果は未確認です。定義した検査を、実行済みの成功として数えません。

| ジョブ | 実行環境 | 検査 |
|---|---|---|
| Rust stable | Linux / macOS / Windows | 整形、静的解析、単体・結合・CLIテスト |
| Rust stableの追加検査 | Linux | 宣言の同期、固定した種0〜31の32試合 |
| Rust 1.80.1 | Linux | `test --locked`、`check --locked --all-targets` |
| 形式検証 | Linux | Lean 4.36.0、Leanの公理依存、Kani 0.68.0、契約違反の否定検査 |

利用するGitHub Actionは存在を確認したコミットへ固定しました。

| 配布元 | 固定した参照 |
|---|---|
| [actions/checkout v7.0.1](https://github.com/actions/checkout/tree/3d3c42e5aac5ba805825da76410c181273ba90b1) | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| [astral-sh/setup-uv v10.4.0](https://github.com/astral-sh/setup-uv/tree/1db35dd8eee11c5894872e0b383ae6c406cf738d) | `1db35dd8eee11c5894872e0b383ae6c406cf738d`、uv 0.13.9 |
| [model-checking/kani-github-action v1.1](https://github.com/model-checking/kani-github-action/tree/f838096619a707b0f6b2118cf435eaccfa33e51f) | `f838096619a707b0f6b2118cf435eaccfa33e51f`、Kani 0.68.0 |
| [公式elan v4.4.0](https://github.com/leanprover/elan/releases/tag/v4.4.0) | Linux x86_64配布物の公開SHA256を照合し、Lean 4.36.0を導入 |

elanの導入方法は[公式README](https://github.com/leanprover/elan/blob/v4.4.0/README.md)の配布物からの導入に従っています。Leanの版は [lean-toolchain](../../proofs/lean-toolchain) とCIの両方で一致を検査します。

CIファイルについては、YAMLの構文、3種類のOS、読取専用の権限、直接使用するActionの全参照が40桁のコミットSHAであることを静的に確認しました。elan 4.4.0のLinux配布物は実際に取得して公開SHA256との一致を確認し、インストーラのヘルプで指定する引数が存在することを確認しました。KaniはCargoの厳密な版指定 `=0.68.0` を渡し、実バージョンも確認します。これらの確認はGitHub上のジョブ実行の代わりにはなりません。

通常のRustジョブは最新stableの変更を検出するため、stableとGitHub管理実行環境を追従します。また、固定したKani Action自身の内部にはRust導入用の `stable` 参照があります。したがって、このCI定義は実行環境全体の完全な固定を主張するものではありません。失敗したOS・Rust版を記録し、その条件で再検証してください。
