# 実行環境の再現と配布物の検査

この文書は0.2系の導入・検証・配布入口です。ゲームの核はRust標準ライブラリのみで動き、API・端末クライアント・WebAssemblyとの結合は別クレートです。初版の `scripts/verify.py` と過去の検証記録は履歴として保持します。現在の入口は `scripts/verify_platform.py` です。

## 導入

Python 3.12以降、対象OSのC/C++リンカー、公式配布先へ接続できるネットワークを用意してください。Linuxでは通常の開発用コンパイラ、macOSではCommand Line Tools、WindowsではMSVCのビルドツールが必要です。OSの開発環境自体をこのスクリプトが導入することはありません。

```sh
# Linux x86_64：Rust、Node、TypeScript、WASM結合、Lean、Kani
python3 scripts/bootstrap.py --profile full
python3 scripts/doctor.py --profile full
python3 scripts/verify_platform.py --profile full

# 形式検証を行わないAPI/SDK開発用。Linux・macOS・Windows
python3 scripts/bootstrap.py --profile frontend
python3 scripts/verify_platform.py --profile frontend

# Rustの通常検査だけを行う場合
python3 scripts/bootstrap.py --profile native
python3 scripts/verify_platform.py --profile native
```

Windowsなどで実行名が `python` の場合は、その実行名に読み替えます。Pythonの実版は各実行記録に保存します。`bootstrap.py` は公式配布物の版とSHA256を照合し、`dist/tools/` に導入します。シェルの設定ファイルやOS全体へ道具を追加する手順ではありません。以後の各スクリプトは、そのディレクトリの道具を優先します。

端末で `cargo run` などを直接実行するときは、導入成功時に生成する設定を、その端末で読み込みます。Pythonの補助スクリプトからの実行では読み込み不要です。

```sh
# bash/zsh
source dist/tools/activate.sh
cargo run --release
```

PowerShellでは `. ./dist/tools/activate.ps1` を実行してから `cargo run --release` を使います。

既存の道具がすべて指定版と一致する場合は `--reuse-existing` を付けられます。既存環境を再利用した検査と、何も導入されていない環境での検査は同じ実験ではありません。前者だけを根拠に後者の成功を主張しません。

形式検証の自動導入はLinux x86_64が対象です。macOS・Windowsでは通常のRust・SDK開発を扱い、形式検証はLinuxのCIで実行します。DockerやPodmanの導入は前提にしていません。

## 固定する版と固定しないもの

正本は [`tools/versions.json`](../../tools/versions.json) です。Rustの通常の実行は [`rust-toolchain.toml`](../../rust-toolchain.toml) でも固定します。

| 道具 | 指定版 | 役割 |
|---|---|---|
| Rust / Cargo | 1.100.0 | 通常のワークスペースの開発・配布 |
| Rust最小対応検査 | 1.80.1 | 依存のない核クレート `suzume` だけの検査 |
| rustup | 1.29.3 | 作業用Rust環境の導入 |
| Node.js / npm | 24.22.3 / 11.19.1 | SDKの構築・配布物の消費 |
| TypeScript | 7.0.2 | 公開型と実装の検査 |
| wasm-bindgen | 0.2.130 | RustとJavaScriptの結合コード生成 |
| Kani | 0.68.0 | 任意入力に対する実装の基本契約の検査 |
| Lean / elan | 4.36.0 / 4.4.0 | 宣言に対応する定理の検査 |
| Python | 3.12以降 | 標準ライブラリだけを使う補助処理。実版を保存 |

Kaniが使う専用のRustとCBMCはKaniの配布物に従います。通常のRustコンパイラと同一であるとは扱いません。`doctor.py` は必要な版が欠ける場合や一致しない場合に非0で終了します。WebAssembly対象 `wasm32-unknown-unknown` の存在もSDK用の確認に含めます。

Rustクレートの依存は `Cargo.lock`、SDKの依存は `packages/suzume/package-lock.json` で固定します。通常の構築・検査は `--locked` と `npm ci` を使います。最小Rust版の検査とAPI・CLI・WASMクレートの最小版は分けています。

```sh
rustup toolchain install 1.80.1 --profile minimal
cargo +1.80.1 test --locked -p suzume
cargo +1.80.1 check --locked --all-targets -p suzume
```

OSのカーネル、リンカー、GitHubの実行環境イメージ全体は固定していません。OS・CPU・libc・Rustの詳細を記録します。これは同じバイナリをビット単位で必ず再生成できるという保証ではありません。時刻と所有者を正規化したソース書庫については、同じ入力から同じ書庫を生成する構成です。

## 検証の合否

| 指定 | 含む対象 | 省略する対象 |
|---|---|---|
| `native` | 版、文書、宣言同期、生成器、検証器自身、Rust、固定32種の対局 | SDK、形式検証 |
| `frontend` | nativeの全対象、APIスキーマ同期、TypeScript生成同期、WASM、SDK | 形式検証 |
| `formal` | 版、文書、宣言同期、生成器、検証器自身、Lean、Kani、否定検査 | 通常の対局、SDK |
| `full` | 上記の全対象 | なし |

各実行の `success` はその指定範囲の合否です。全体の受入には `profile: "full"`、`full_verification: true`、`success: true` をすべて要求します。省略した範囲は `omitted_profiles` に明記します。

文書検査は全Markdownの相対リンクと、現行文書のPython・Cargo・npm・Lakeの入口を静的に照合します。空の検査やエラーのある集計を拒否し、検査器自身の正常例・否定例10件も実行します。ネットワークの疎通や、掲載コマンドをすべて実行したという意味ではありません。

終了値だけで合格にはしません。Rustでは基準の76件以上、失敗・無視・絞込0件を要求します。32種の対局では種0〜31が重複なくそろうことと母数32を確認します。SDKではNodeのTAP集計から、テスト0件・失敗・中断・省略を拒否します。

Kaniでは正本に記した6ハーネスの名前、各結果、集計の一致を検査します。Leanでは18定理の名前が重複なく存在し、公理依存が `propext`、`Quot.sound`、`Classical.choice` の範囲内であることを要求します。`sorryAx` と未知の公理は、Leanの終了値が0であっても不合格です。対象を意図的に追加・変更するときは正本の対象一覧と検査を一緒に更新します。

各処理には実行時間の上限があります。上限到達では子プロセス群を終了し、終了値124と `timed_out: true` を記録して失敗とします。検証中のソース変更も受入不可です。Rust・API・SDK・宣言・証明・スクリプト・版指定・CI定義を、開始前と終了後にSHA256で照合します。

## 証拠の保存

新しい実行は `dist/verification/<実行ID>/` に保存します。時刻と乱数を含む実行IDは再利用せず、各結果ファイルは上書きを拒否して作成します。

- `summary.json`：範囲、全体判定、開始・終了時刻、前後のソースSHA256、変更されたファイル。
- `environment.json`：道具の実版、OS、CPU、libc、Rustの詳細。
- `<検査ID>.json`：引数、終了値、経過時間、時間上限、意味の判定、ログのSHA256。
- `<検査ID>.log`：標準出力と標準エラーを合わせた実出力。

実行器に固有の作業パス、自宅ディレクトリ、一時ディレクトリは公開向けの表記へ置き換えます。環境変数の全内容や認証情報は保存しません。これらのハッシュは内容照合用であり、署名された出所証明の代用ではありません。

旧否定検査には `--log PATH` を追加し、新しい実行の出力をその専用ディレクトリへ向けます。既定動作は互換のため維持しますが、新しい検証処理から過去の成功記録を上書きすることはありません。

## 配布物の作成と消費検査

```sh
# Rust、SDK、WASMの配布物を作成し、消費側から検査
python3 scripts/package.py

# 対象OSのRust配布物のみ。SDK検査の成功を意味しません
python3 scripts/package.py --native-only
```

成功した実行の成果物は `dist/packages/<実行ID>/` にあります。

| 成果物 | 内容 |
|---|---|
| `*.crate` | 核・API・CLI・WASMのRustソース書庫 |
| `workspace-Cargo.lock` | 内部クレートをまとめて消費するための固定依存情報 |
| `*.tgz` | `@mahjong-sandbox/sdk` のnpm書庫。通常モードのみ |
| `suzume-source-<版>.tar.gz` | Rust、宣言、証明、SDK、結合仕様、評価資料、補助処理、公開docs全体を含むソース書庫 |
| `suzume-<版>-<OS/CPU>.tar.gz` | その環境でビルドした端末用とAPI用の実行ファイル、MIT本文、起動案内 |
| `manifest.json` / `SHA256SUMS` | 版、環境、Gitの参照と未コミット状態、ソースと各配布物のハッシュ |

配布用のnative・WASMビルドは、リポジトリ、利用者のホーム、Cargo・Rustupの保存先を `--remap-path-prefix` で一般化します。依存crateも対象にし、既存の `CARGO_ENCODED_RUSTFLAGS`、未設定なら `RUSTFLAGS` を引き継いだ子プロセスの環境だけへ設定します。Cargoの設定ファイルやbuild/target用の指定にあるflagsも必要な場合は、このいずれかへ明示してください。[Cargoの優先順位](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags)に従い、これらの指定は自動合成されません。

作成したnative書庫の実行ファイル2本とnpm書庫のNode/Web WASM2本について、必須項目の存在と、ビルド環境から得た既知の私的経路の残存を実バイト列で検査します。`--native-only` は実行ファイル2本を対象とします。残存や対象欠落があれば配布検査を失敗させ、公開する結果には私的経路を含めず、相対ファイル名・件数・ハッシュと判定を残します。

未公開の内部依存を含むため、Cargoの書庫作成時だけ `--no-verify --exclude-lockfile` を使用します。その後、作成した全書庫を**別の一時ディレクトリ**に展開し、内部クレートへ向ける `patch.crates-io` と元の `Cargo.lock` を設定します。そこから `cargo build --workspace --locked --offline` を成功させ、出来上がったCLIを起動するまでを必須にします。書庫作成だけを配布検証の成功には数えません。

SDKも別の一時ディレクトリで、実際のnpm書庫を `npm install --offline --ignore-scripts` して検査します。公開されたNode用・Web用の入口から同じ初期対局が得られ、打牌とCPUの一手の結果が一致することを確認します。古いリビジョンの要求は拒否され、状態が変わらないことも検査します。不正JSONのエラー応答を含め、配布したWASMそのものを実行します。公開型の消費試験は `strict`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes` を有効にし、要求型が `any` に退化した場合も失敗する例を含みます。

Web用の検査では、配布された実際のWASMバイト列をNodeのWebAssembly実装へ渡します。ブラウザの表示、HTTP配信、CSP、CORSを実機のブラウザで確認したという意味ではありません。生成されたWeb入口はESM、Node入口はCommonJSで、SDKの公開入口からそれぞれ読み込みます。

SDKの公開入口は `@mahjong-sandbox/sdk/node` と `@mahjong-sandbox/sdk/browser` です。WASM結合に生成される `Symbol.dispose` の宣言には、必要な `esnext.disposable` 標準型の参照を生成時に補います。消費側がES2022を指定していても、この型参照のためだけに設定変更を要求しません。

ソース書庫も別のディレクトリへ展開し、固定した依存でワークスペース全体を型検査します。展開した書庫に含まれる文書検査器で、文書リンクとコマンド入口が書庫内でも成立することを確認します。配布する実行ファイルそのものも展開後に起動します。いずれかが失敗した場合は完成した配布として扱いません。これらの処理はパッケージ登録先への公開やGitHub Releaseの作成を行いません。

ソース書庫にはREADMEから参照する公開資料を含め、`bindings/`、`evaluations/`、API・ログ・ルール・AI・ADR・作業記録も同梱します。Rustやnpmの実行用パッケージへ作業記録を混ぜることとは区別します。結合仕様と評価資料は検査入力のSHAへ含めますが、検証・作業の記録全体は入力SHAへ広げません。書庫へ実際に入れた各ファイルのSHAは `source_bundle_sha256_by_file` に別途保存するため、検証後の記録追記をコード変更と混同しません。

Linuxの実行ファイルは作成環境のlibcに依存します。現段階でmuslによる静的配布やすべてのLinux系列への互換性を確認したものではありません。Windows/macOS版の作成・実行結果も、そのCI記録に対応する環境に限って判断します。

## 監査で確認した欠陥と対応

初版 `f3fc61b` の監査では次を確認しました。初版の実行結果を否定するものではなく、変更後の失敗を自動で検出できるか、配布側から再現できるかという観点での欠陥です。

| 問題 | 実測・コード上の根拠 | 今回の対応 |
|---|---|---|
| Leanの未証明を終了値だけで排除できない | 独立ファイルの `False := by sorry` が `sorryAx` を表示し終了0 | 定理一覧と公理の許可一覧を照合 |
| 空になった検証や対象欠落を判定できない | 旧処理は実行の終了値とソース不変だけで集計 | Kani・Lean・Rust・対局・SDKの内容を検査 |
| 通常のRust版が環境任せ | Rustの版指定ファイルなし、CIのstable追従 | 通常版を固定し、stable互換検査とは分離 |
| 実行時間の上限がない | 旧 `verify.py` のsubprocessにtimeoutなし | 個別上限とプロセス群の終了、実際の時間切れ試験 |
| 成功記録を次の実行が上書きする | 固定ログ名と固定summary名 | 実行ID別の専用出力と上書き拒否 |
| 配布内容が広すぎる | `cargo package` は89ファイル、533,552 bytes。作業書と30個のログを含む | Cargoのinclude指定と配布書庫からの消費検査 |
| 書庫から利用した結果がない | 開発ディレクトリ内のビルドとテストだけでは欠落ファイルを発見できない | Rust・npm・ソース・実行形式の書庫を別ディレクトリから検査 |

道具の導入確認では、Rust 1.100.0のWASM対象で既存ライブラリの型検査が成功し、公式wasm-bindgen 0.2.130とTypeScript 7.0.2の固定導入を実行しました。検証・配布の回帰試験13件には、実プロセスの時間切れ、実行ファイル欠落、空検証、未証明、未知公理、対象欠落、同名証拠の上書き拒否を含みます。結合仕様・評価資料のハッシュ対象からの脱落、配布資料の欠落、ソース書庫がファイル時刻や権限に影響される問題、空の文書検査も確認します。

新しいCI定義を作成した事実と、そのCIが成功した事実は分けます。成功した実行IDと対象ソースは各summary、およびGitHub Actionsの完了結果で確認してください。実装中のAPIや未実行の配布消費を成功とみなしません。

## CIと公式資料

[`platform.yml`](../../.github/workflows/platform.yml) はRustの固定版でLinux/macOS/Windowsを検査し、各OSのRust配布を作成します。Linuxの別ジョブは作業用の道具を公式配布から導入し、形式検証・SDK・配布を一括検査します。さらに `tools/browser/package-lock.json` の固定依存からChromiumを導入し、`scripts/check_browser.py` で実ブラウザを検査してsummaryとPNGを保存します。このブラウザ検査はNodeのWebAssembly検査とは別の合否です。従来の [`ci.yml`](../../.github/workflows/ci.yml) は最新stableへの互換性と最小Rust版を確認します。

作業環境で実ブラウザの起動や公式配布物の取得が制限された場合は、その失敗を成功へ読み替えません。GitHub CIで実行したブラウザの版、assertion結果、PNGのSHA、対象コミットを確認してから、その条件での成功を記録します。

直接参照するActionは存在を確認したコミットに固定しています。`actions/checkout` は `3d3c42e5aac5ba805825da76410c181273ba90b1`、`actions/setup-python` は `6d4fd22b20465ccc0f7354476588e8df7c9ca0e0`、`actions/upload-artifact` は `cf430e030ddbb5b0abf93d22962f4752f3646cd9` です。CIで使用するPythonは3.12.10です。GitHub管理のOSイメージ内の全ファイルまで固定したという意味ではありません。

- [rustupのツールチェーン指定](https://rust-lang.github.io/rustup/overrides.html)
- [Cargoの配布とpackageの検査](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Node.js 24.22.3の公式SHA256一覧](https://nodejs.org/dist/v24.22.3/SHASUMS256.txt)
- [wasm-bindgen 0.2.130の公式配布](https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/0.2.130)
- [TypeScriptのstrict指定](https://www.typescriptlang.org/tsconfig/strict.html)
- [Kaniの公式導入手順](https://model-checking.github.io/kani/install-guide.html)
- [elan 4.4.0の公式配布](https://github.com/leanprover/elan/releases/tag/v4.4.0)
