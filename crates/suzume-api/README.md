# suzume-api

麻雀スズメの対局操作、設定、牌譜解析、席別観測を提供するAPIです。標準入出力とWASMから同じ処理を呼び出せます。

## 最初の要求

```rust
let mut engine = suzume_api::Engine::new();
let response = engine.request(r#"{"version":1,"op":"new"}"#);
assert!(response.contains("\"status\":\"ok\""));
```

実行ファイルでは `suzume-api serve` を起動し、JSON要求を1行ずつ送ります。API版は1です。乱数種は十進文字列で指定し、後続の操作には応答のセッションIDと更新番号を使います。

## 文書

| 目的 | 案内 |
|---|---|
| 対局を開始・操作する | [API仕様](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/README.md) |
| 席やAIを設定する | [共通設定](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/configuration.md) |
| mjsonを解析・変換する | [牌譜の使い方](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/logging/README.md) |
| 原文と解析注釈を保存する | [注釈付きmjai](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/logging/annotated-mjai.md) |
| 学習器を接続する | [研究用環境](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/environment.md) |

Rustから原文を保存する場合は `ParsedEvent.json` を使います。型付きの `event` や `raw` は解析用で、数値の丸めなどが起きる場合があります。[Rustからの利用](https://github.com/yuki-inaho/suzume-mahjong/blob/main/docs/api/README.md#rustから使う)に保存と互換性の注意をまとめています。

[MITライセンス](LICENSE)
