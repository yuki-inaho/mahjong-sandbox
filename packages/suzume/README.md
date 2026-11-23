# JavaScript・TypeScript SDK

麻雀スズメの対局操作と牌譜解析を、ブラウザ・Node.jsから利用するSDKです。パッケージ名は `@mahjong-sandbox/sdk` です。

## 導入

Node.jsは24以降が必要です。npmレジストリには公開していません。[配布手順](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/development/reproducibility.md#配布物の作成と消費検査)で作成した `.tgz` をインストールしてください。

## Node.jsで対局を始める

```javascript
import { createNodeEngine } from '@mahjong-sandbox/sdk/node';

const client = createNodeEngine();
try {
  const state = await client.newGame();
  console.log(state.own_hand);
} finally {
  client.close();
}
```

## 文書

| 目的 | 案内 |
|---|---|
| 対局操作・設定・エラー処理 | [SDKの使い方](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/sdk.md) |
| ブラウザで動かす | [ブラウザ例](https://github.com/yuki-inaho/suzume-mahjong/blob/main/examples/browser/README.md) |
| 牌譜を読む・原文と注釈を保存する | [mjsonの読込み](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/sdk.md#mjson牌譜を読む解析する)・[注釈の保存](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/sdk.md#原文を保持して注釈を付ける) |
| AIや学習器を接続する | [研究用環境](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/environment.md) |

処理は1操作ずつ `await` し、使い終わったら `close()` で解放します。逐次読み込んだ牌譜は、末尾まで検査が成功してから完全な記録として扱ってください。

[MITライセンス](LICENSE)
