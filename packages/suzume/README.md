# @mahjong-sandbox/sdk 0.2.0

Rustの麻雀コアをWASMとして実行する、ブラウザ・Node.js向けの型付きAPIです。API版1。JavaScriptとTypeScriptの両方で使用できます。型定義、実行時の入出力検査、実際のWASMを同梱します。npm公開は行わず、リポジトリの配布手順で作るtarballからインストールします。

## Node.js

```javascript
import { createNodeEngine } from '@mahjong-sandbox/sdk/node';

const client = createNodeEngine();
try {
  let state = await client.newGame();
  const first = state.legal_actions[0];
  if (first) state = await client.apply(first.action);
  console.log(state.revision, state.players.map(player => player.points));
} finally {
  client.close();
}
```

## ブラウザ

```javascript
import { createBrowserEngine } from '@mahjong-sandbox/sdk/browser';

const client = await createBrowserEngine();
const state = await client.newGame();
console.log(state.own_hand);
client.close();
```

通常は配布物内の相対URLからWASMを読み込みます。取得済みの `BufferSource`、`WebAssembly.Module`、明示URLを引数へ渡すこともできます。Node専用の標準ライブラリはブラウザ入口へ混入しません。HTMLから直接使う最小例はリポジトリの `examples/browser/` にあります。

## 外部算法・強化学習

```javascript
import { createNodeEngine } from '@mahjong-sandbox/sdk/node';

const client = createNodeEngine();
try {
  let state = await client.environmentReset('18446744073709551615', 100);
  while (!state.terminated && !state.truncated) {
    const observation = state.observation;
    if (!observation) throw new Error('継続中の観測がありません。');
    const actionId = observation.legal_mask.findIndex(Boolean);
    if (actionId < 0) throw new Error('合法手がありません。');
    state = await client.environmentStep(actionId);
    console.log(state.actor, state.rewards, state.escrow_delta);
  }
} finally {
  client.close();
}
```

これは最初の合法手を選ぶ使用例です。実際の算法は、観測と189要素の合法手maskを入力として行動IDを選ぶ関数へ置換します。各stepは1席の1操作で、次の観測は次の手番の席のものです。報酬は4席の点数差、供託の差分は別項目、通常終了と操作上限の打切りは別の真偽値です。

## エラーと所有権

便利メソッドはAPIの失敗を `ApiFault` として送出します。`code` で分岐し、`message` は表示へ使用します。`command(request)` は生の型付き応答を返し、APIエラーも `status: "error"` として扱えます。構造が不正な応答、精度が失われる整数、未知版は `ProtocolError` です。要求・応答とも1MiBまでです。

`apply`、`stepBot`、`environmentStep` は最後に受け取ったセッションIDとrevisionを送ります。各操作をawaitしてから次へ進めてください。同時に作られた古い操作はサーバー側の検査で拒否され、自動再試行しません。`close()` はWASM領域を解放し、以降の呼出しを拒否します。

TOMLを使う `loadConfig(text)`、実効値を読む `inspectConfig(text)`、状態を変えず自身の手を評価する `analyze(config)` も提供します。配布物から型を読むだけで入出力のタグと形を確認できます。完全な仕様はリポジトリの `docs/api/` にあります。

## mjson牌譜を読む・解析する

`.mjson` は1行に1イベントを記録するmjai形式です。SDKの入力は展開済みUTF-8です。gzipの圧縮バイト列は、呼出し側で上限を設けて展開するか、圧縮を判定するネイティブの `suzume-api mjai analyze PATH` を使用してください。

| 入口 | 入力と結果 | 検査範囲 |
|---|---|---|
| `parseMjaiEvent(text)` | 1行の文字列から `MjaiEventRecord` | Rust生成Schemaによる既知イベントの項目・牌・席・配列長・型。追加項目は元のオブジェクトに保持 |
| `readMjaiEvents(chunks, limits?)` | `Uint8Array` の同期/非同期反復からイベントを逐次取得 | 上記に加え、開始・局結果・局終了・対局終了の順序、UTF-8、入力上限、末尾までの確認 |
| `parseMjaiRecord(text)` / `readMjaiRecords(chunks, limits?)` | 解析用の `event` と保存用の `json` | イベント原文を保持。逐次読込みではライフサイクル・上限も検査 |
| `client.analyzeMjai(mjson, viewer?, format?)` | 全文と任意の席0〜3から `{kind: 'mjai', summary, observation}` | 同じRust実装をWASMで実行し、牌の増減・手番・副露・点数/供託の整合を追加検査。format省略時は `mjai` |

構造パーサは `@mahjong-sandbox/sdk` から取り込みます。Node・ブラウザのどちらの `Client` でも `analyzeMjai` を利用できます。この操作は新しい対局を作らず、進行中の対局・研究環境のセッションID、操作回数、観測を変更しません。`viewer` を省略すると `observation.own_hand` はnull、指定するとその席の記録上の手牌だけを返します。他家の伏せ牌や後続イベントを観測に含めません。未記録の牌は `?` のままです。

`parseMjaiEvent`・`readMjaiEvents` が返す元イベント自体は秘匿処理をしません。入力に全席の配牌やツモが含まれていれば保持します。通常のAIの入力には、指定視点の `AgentObservation` を使用し、全情報の元イベントをそのまま渡さないでください。

`summary` はイベント数・局数・和了数・流局数・副露数・赤牌を含むイベント数、既知の場合の最終持点・供託本数、終了種別を返します。`complete` は正常終局の場合だけtrueです。`observation.record_ended` は明示された記録終端を読んだこと、`game_ended` は正常終局、`round_ended` は局結果が成立したことを示します。途中中断や操作上限の打切りを正常終局へ読み替えません。単に末尾が切れている入力はエラーで、途中までの反復を全体の成功と扱ってはいけません。

生成される `MjaiEvent` は既知項目の型です。`MjaiEventRecord` の追加項目の型は `unknown` であり、使用前に利用側で検査します。赤五は `5mr`・`5pr`・`5sr` として通常の五と区別します。追加メタデータの有限な小数・指数は読めますが、解析用の `Number` では丸めが起きる場合があります。保存には原文付きの入口を使い、解析値を `JSON.stringify` して保存し直さないでください。既知の整数項目では小数・指数・負のゼロ表記を拒否します。JavaScriptで正確に保持できない整数トークンも丸めず拒否します。重複キーは追加メタデータ内でもエラーです。

行読込みの既定上限は1行1,048,576バイト、10,002イベント、全体268,435,456バイトです。`limits` の `maxLineBytes`・`maxRecords`・`maxTotalBytes` は正の安全な整数を指定します。1行パーサ自体の1MiB上限は引き上げられません。`analyzeMjai` のAPI要求はJSONの囲み・エスケープを含め1MiBまでです。大きい牌譜ではネイティブファイル入口、逐次検査には `readMjaiEvents` を使用します。

構造検査だけでは牌の移動や和了の合法性を保証しません。Rustの追加検査も、外部牌譜の役・得点の全面的な再計算や、自作coreへの完全再生ではありません。外部の未対応規則について合法手マスクを推測しません。意味解析のエラーは `ApiFault` の `code: 'log_error'` と行番号・イベント型付きの説明で返ります。

## 原文を保持して注釈を付ける

`annotated_mjai` 版1は、`record`、`schema_version`、1始まりの `index`、元イベントの `event`、注釈オブジェクトの `annotations` を持ちます。推奨拡張子は `.annotated.jsonl` です。外側の未知項目・版違い・連番の欠落・重複を拒否します。注釈を含む記録は全情報を保持する保存用であり、AIプレイヤー向けの観測とは異なります。

| 入口 | 役割 |
|---|---|
| `parseAnnotatedMjaiRecord(text)` | 1行を解析し、`event` / `annotations` と保存用の `eventJson` / `annotationsJson` を分けて返す |
| `readAnnotatedMjaiRecords(chunks, limits?)` | 連番・イベント順序・入力上限・正常EOFを検査して逐次読む |
| `encodeAnnotatedMjaiRecord(eventJson, index, annotationsJson, limits?)` | 両方の原文を検査し、LFを付けない1行を返す |
| `wrapMjaiRecords(chunks, write, limits?)` | 空の注釈で包み、LF付きの行を保存関数へ渡す |
| `unwrapAnnotatedMjaiRecords(chunks, write, options?, limits?)` | 標準の原イベントへ戻す。非空注釈は既定で拒否する |

変換関数は全体の正常EOF後に `{ records, droppedAnnotationRecords }` を返します。`write` は同期・非同期のどちらでも利用でき、失敗を返すと変換も失敗します。途中の出力は暫定なので、呼出側は成功するまで完成物として公開しないでください。注釈を破棄する場合だけ `{ dropAnnotations: true }` を指定します。
破棄の指定は呼出時に確定し、変換途中の設定変更を反映しません。継承プロパティによる指定は採用しません。

次は小さい入力の往復例です。数値の原文をそのまま保ちます。

```javascript
import { wrapMjaiRecords, unwrapAnnotatedMjaiRecords } from '@mahjong-sandbox/sdk';

const original = '{"type":"start_game","extra":{"weight":1e-400}}\n{"type":"end_game"}\n';
const utf8 = new TextEncoder();
const wrapped = [];
await wrapMjaiRecords([utf8.encode(original)], line => { wrapped.push(line); });
const restored = [];
const report = await unwrapAnnotatedMjaiRecords(
  [utf8.encode(wrapped.join(''))],
  line => { restored.push(line); },
);
if (restored.join('') !== original) throw new Error('原イベントが変わりました。');
console.log(report.records, report.droppedAnnotationRecords);
```

注釈の追加・編集は、読み込んだ `eventJson` と新しい注釈JSONを `encodeAnnotatedMjaiRecord` へ渡します。変更しない注釈は `annotationsJson` を渡せば数値字句などを保持できます。原文内部の実CR/LFは1行形式を壊すため拒否し、文字列内のエスケープ済み改行は許容します。前後のJSON ASCII空白と行末の改行は保存の一致対象外です。

入力と出力へ同じ上限を適用し、包装の追加分も数えます。UTF-8の行バイト数は終端LFを除き、総量はLFを含みます。注釈付きの意味解析は `client.analyzeMjai(text, viewer, 'annotated_mjai')` で呼び、既存の席別観測へ原イベントや注釈を混ぜません。

## 従来形式のログと学習データを逐次読む

`readLogRecords(chunks)` はUTF-8バイト列の非同期反復から、型付きのJSONLレコードを1件ずつ返します。`readTransitions(chunks)` はRustが出力した学習用の遷移を返します。NodeのReadStreamやブラウザのストリームから利用でき、全ファイルを文字列へ展開する必要はありません。行・件数・全体バイト数に上限があります。途中までの反復では末尾までの検査は完了していません。

このSDKの検査は構造、範囲、連番、末尾、共有用の私的項目を対象とします。対局規則、局面ハッシュ、エンジン互換まで検査する完全再生はRustの `logging::replay`、または `suzume-api replay PATH` を使用します。SDKで1行を読めることと、正しい対局を再生できることを区別してください。

打切りの遷移は `final_observation` に最後の次手番の観測を残します。自然終局と継続中ではnullです。学習器は終了原因を区別し、時間制限時の価値推定にこの観測を使用できます。

## 開発

固定ツール導入後、リポジトリルートで `python3 scripts/build_wasm.py` を実行し、`npm ci --ignore-scripts --prefix packages/suzume`、`npm test --prefix packages/suzume` を実行します。Rust型を変更した場合はSchemaとTypeScriptの生成・同期検査も必要です。

ライセンス: MIT。
