# suzume-api 0.2.0

Rust製麻雀コア`suzume`の、型付きJSON/TOML、閲覧席ごとの観測、AI判断説明、学習環境、対局記録を提供します。

```rust
let mut engine = suzume_api::Engine::new();
let response = engine.request(r#"{"version":1,"op":"new"}"#);
assert!(response.contains("\"status\":\"ok\""));
```

標準入出力では `suzume-api serve` を実行し、JSON要求を1行ずつ送ります。API版は1、64bitの乱数種は十進文字列です。不正入力と古い操作を拒否し、通常応答には他家の手牌や山の種を含めません。

仕様、再現手順、再生ログの信頼境界は同じリポジトリの `docs/api/`、`docs/logging/`、`docs/development/reproducibility.md` にあります。

ライセンス: MIT。
