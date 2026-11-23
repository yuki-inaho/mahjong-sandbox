# suzume-api 0.2.0

Rust製麻雀コア`suzume`の、型付きJSON/TOML、閲覧席ごとの観測、AI判断説明、学習環境、対局記録を提供します。

```rust
let mut engine = suzume_api::Engine::new();
let response = engine.request(r#"{"version":1,"op":"new"}"#);
assert!(response.contains("\"status\":\"ok\""));
```

標準入出力では `suzume-api serve` を実行し、JSON要求を1行ずつ送ります。API版は1、64bitの乱数種は十進文字列です。不正入力と古い操作を拒否し、通常応答には他家の手牌や山の種を含めません。

通常の対局記録はmjaiの `.mjson` です。`mjai::analyze` はイベントと記録上の局面を検査し、
`mjai::visit` は各イベント後の状態から席別の観測を作れます。
赤五・未知牌・加槓・複数ロンを保持しますが、外部ルールの完全な合法性判定や合法手マスクの生成は行いません。

native CLIは `suzume-api mjai analyze PATH` で非圧縮またはgzipの牌譜を解析します。
`suzume-api mjai convert LEGACY --output PATH` は検証済みの旧replayをmjsonへ変換し、
`suzume-api mjai observations PATH --seat 0 --output PATH` は指定席の観測を出力します。
Rust reader、JSON APIの `analyze_mjai`、WASMへの入力は展開済みテキストです。
乱数種・内部操作の厳密再生とAI判断の保存には、旧 `replay` 形式を明示して使用できます。

仕様、再現手順、再生ログの信頼境界は同じリポジトリの `docs/api/`、`docs/logging/`、`docs/development/reproducibility.md` にあります。

ライセンス: MIT。
