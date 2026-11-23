# 注釈付きmjai記録（版1）

## 目的と保存対象

標準 `.mjson` の各イベントへ、解析結果、説明、解析器の版などを付けて保存する形式です。標準mjaiの既定出力は変えません。推奨拡張子は `.annotated.jsonl`、形式識別子は `annotated_mjai` です。記録には他家の手牌を含む場合があるため、AIプレイヤーへ渡す入力には引き続き席別の観測を使います。

1行は次の5項目を持つJSONオブジェクトです。`event` は文字列ではなく、標準mjaiイベントのオブジェクトです。

| 項目 | 版1の契約 |
|---|---|
| `record` | 文字列 `annotated_mjai` |
| `schema_version` | 整数字句 `1` |
| `index` | 1始まり、`1..=4294967295` の連続する整数字句。単一行の解析では範囲、全体の読み込みでは連番を検査 |
| `event` | 元のmjaiイベント。既存の構造・順序・局面検査を適用 |
| `annotations` | 必須のJSONオブジェクト。空なら `{}`。名前の衝突を避けるため、解析器ごとの下位キーの使用を推奨 |

外側の未知項目、未知版、重複キー、欠落項目、型違い、連番の欠落・重複を拒否します。注釈やイベントの入れ子内でも重複キーを拒否します。`schema_version:1.0` や `index:1e0` を整数へ補正しません。イベント内の `actor:1.0` も元の字句のまま既存検査へ渡し、拒否します。

## 原文と解析用の値

保存用の正本は、検証済みのイベントJSON原文です。イベント全体を `serde_json::Value` やJavaScriptの `Number` へ変換してから再生成する方式では、拡張数値の精度や表記を保持できません。型付きイベントは解析用の射影であり、未知項目、明示的な `null`、別名、キー順の保存には使いません。

RustとSDKは、解析用の値と原文を区別して返します。注釈も原文を保持し、保存時にはその原文を検証して埋め込みます。イベント原文の内部の空白、キー順、数値字句、未知項目、別名、`null` を維持します。行の前後のJSON空白、CRLF/LF、最後の改行、gzip圧縮のバイト列の一致は契約外です。出力は1行1イベント、LF終端です。

単一行の保存入口では、前後のJSONで許されたASCII空白だけを除き、原文内部に実際のCR/LFが残る入力を拒否します。文字列内のエスケープ済み `\n` は保持します。一般的なtrimでNBSPなどJSON外の空白を受理しません。包装から取り出したイベントにも既存の構造・深さ・数値字句の制限を適用します。

既存の数値受理範囲は維持します。Rustの解析値は有限の浮動小数点へ丸められる場合があり、SDKの整数字句はJavaScriptの安全整数範囲に制限されます。SDKで受理できる指数字句でも解析値が丸められる場合があるため、保存には必ず原文を使います。新しい形式は、入力として拒否された数値を許可するための迂回路ではありません。

## 変換と注釈の扱い

標準→注釈付きの `wrap` は、各イベントへ空の `annotations` と連番を付けます。注釈付き→標準の `unwrap` は、空でない注釈を既定で拒否します。CLIの `--drop-annotations`、Rustの `AnnotationPolicy::Drop`、SDKの明示指定でのみ注釈を捨てられます。破棄した注釈を持つ行数を報告します。注釈付き→標準→注釈付きで、捨てた注釈まで復元できるという意味ではありません。

全体の成功には、`end_game` と正常な入力終端が必要です。逐次読み込みで先に返された行は暫定結果です。後続の不正、上限超過、I/O失敗があれば変換全体を失敗とします。CLIは既存の一時ファイル方式を使用し、全検証・flush・同期後に出力を公開します。既存の宛先を置き換えません。

入力、出力の双方に `LogLimits` の行サイズ・行数・総バイト数を適用します。バイト数はUTF-8で数え、行上限は終端LFを除き、総量は終端LFを含めます。入力がCRLFならCRは行上限へ含みます。注釈付き形式では包装部分も上限へ含めます。ネイティブCLIは拡張子ではなく先頭のバイトでgzipを判定し、圧縮入力と展開後の両方を制限します。ライブラリとSDKは展開済みUTF-8を入力とします。

## 公開入口

Rustの入口は `mjai::annotated::{wrap, unwrap, visit}`、注釈を読む入口は `visit_records`、単一行の読み書きは `parse_record` / `encode_record` です。`wrap` と `unwrap` は `TranscodeReport { summary, dropped_annotation_records }` を返します。単一行の操作は構造検査であり、全対局の意味検査には `visit` / `visit_records` を使用します。

SDKは、原文を保持する `parseMjaiRecord` / `readMjaiRecords` と `parseAnnotatedMjaiRecord` / `readAnnotatedMjaiRecords`、単一行を出力する `encodeAnnotatedMjaiRecord` を公開します。逐次変換は `wrapMjaiRecords(chunks, write, limits?)` / `unwrapAnnotatedMjaiRecords(chunks, write, options?, limits?)` です。`write` はLF付きの各行を受け取り、同期または非同期で保存できます。関数は正常EOF後に `Promise<MjaiTranscodeReport>` を完了し、`{ records, droppedAnnotationRecords }` で件数を返します。保存先への出力は正常完了まで暫定です。unwrapのoptionsは `{ dropAnnotations?: boolean }`、既定はfalseです。既存の `parseMjaiEvent` / `readMjaiEvents` は互換のため保持します。SDKは構造・ライフサイクルを検査し、手牌移動・持点の意味検査はRust/WASMが担当します。

`AnalyzeMjai` 要求には `format` を追加し、省略時は `mjai`、明示値は `mjai` または `annotated_mjai` です。同じ入力イベントは同じ集計・観測を返します。`Client.analyzeMjai(mjson, viewer?, format?)` から使用します。外側の任意JSONを閉じたプロトコルSchemaへ混ぜず、生成器の制約を維持します。

CLIは次の入口を提供します。形式の自動推測は行わず、省略時は標準mjaiです。

| 操作 | コマンド |
|---|---|
| 空注釈で包む | `suzume-api mjai wrap INPUT --output OUTPUT` |
| 標準へ戻す | `suzume-api mjai unwrap INPUT --output OUTPUT [--drop-annotations]` |
| 注釈付きを解析 | `suzume-api mjai analyze INPUT --format annotated_mjai` |
| 注釈付きから席別観測 | `suzume-api mjai observations INPUT --seat 0 --output OUTPUT --format annotated_mjai` |

## 集計と情報公開の境界

`red_events` は、既知の牌項目に少なくとも1枚の赤牌があるイベントの数です。`start_kyoku` の手牌・ドラ表示牌、ツモ・打牌・副露・槓・ドラの牌、和了の裏ドラ表示牌を対象にします。イベントの追加文字列、対局者名、注釈は数えません。赤牌の枚数や固有牌数ではなく、同じ牌が複数イベントへ現れればそれぞれを数えます。

これは前版が明記していた「原JSON内の任意位置を数える」契約の変更です。固定20事例の期待値は実装から生成し直さず、独立した既知牌項目の走査で確認します。観測出力には原イベントや注釈を含めません。標準と注釈付きの全席観測が一致することを検査します。

## 検証範囲

参照変換器は従前と同じ固定コミット `f08b4e0f7d96b57d0099b9b2e5eb6dd5c4e5f4eb`、対象は実牌譜由来確認2件と外部回帰18件の全20件・2,310イベント・23局です。元→包装→取り出しの全イベント原文、順序、集計、各イベント後の4席の観測（2,310×4＝9,240観測）を比較します。最終観測だけの検査ではありません。成功事例だけを選び直しません。データ本文は公開物へ含めません。

合成事例では巨大値・微小値・明示null・別名・未知項目・重複・不正な版・末尾欠落・出力上限・I/O失敗・注釈破棄を検査します。実牌譜の全役・和了合法性を独立再計算した保証、mjsonから旧seed/action履歴への逆変換は含みません。
