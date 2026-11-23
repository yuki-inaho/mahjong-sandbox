# 最小ブラウザ例

RustのAPIとWASMをそのまま使用する、依存フレームワークなしの例です。JavaScript内に麻雀ルールやAI算法を再実装しません。

リポジトリルートで次を実行します。

```sh
python3 scripts/build_wasm.py
npm ci --ignore-scripts --prefix packages/suzume
npm run build --prefix packages/suzume
python3 -m http.server 8080 --bind 127.0.0.1
```

`http://127.0.0.1:8080/examples/browser/` を開きます。HTTPを使用し、fileスキームでは開きません。乱数種とCPU設定を選び、手牌を押して打牌します。相手の操作は「あなたの手番まで進める」または「CPUを1手進める」で進行します。新規開始は以前の対局を置換します。

通常画面は席0の人間1人とAI3人に固定しています。TOMLを読み込む画面を作る場合はSDKの `loadConfig` と `inspectConfig` を使用できます。APIやWASMはネットワークへの対局送信を行いません。

`scripts/check_browser.py` は実Chromiumで配牌・打牌・AI進行・再開始・不正seed拒否を検査し、スクリーンショットと機械可読な結果を保存します。現在の表示状態だけを返す `window.__suzumeView` はこの検査用であり、他家の手牌やコアの内部状態を返しません。
