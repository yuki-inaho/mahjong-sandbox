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

## ログと学習データを逐次読む

`readLogRecords(chunks)` はUTF-8バイト列の非同期反復から、型付きのJSONLレコードを1件ずつ返します。`readTransitions(chunks)` はRustが出力した学習用の遷移を返します。NodeのReadStreamやブラウザのストリームから利用でき、全ファイルを文字列へ展開する必要はありません。行・件数・全体バイト数に上限があります。途中までの反復では末尾までの検査は完了していません。

このSDKの検査は構造、範囲、連番、末尾、共有用の私的項目を対象とします。対局規則、局面ハッシュ、エンジン互換まで検査する完全再生はRustの `logging::replay`、または `suzume-api replay PATH` を使用します。SDKで1行を読めることと、正しい対局を再生できることを区別してください。

打切りの遷移は `final_observation` に最後の次手番の観測を残します。自然終局と継続中ではnullです。学習器は終了原因を区別し、時間制限時の価値推定にこの観測を使用できます。

## 開発

固定ツール導入後、リポジトリルートで `python3 scripts/build_wasm.py` を実行し、`npm ci --ignore-scripts --prefix packages/suzume`、`npm test --prefix packages/suzume` を実行します。Rust型を変更した場合はSchemaとTypeScriptの生成・同期検査も必要です。

ライセンス: MIT。
