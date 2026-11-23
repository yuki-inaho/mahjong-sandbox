# ブラウザで遊ぶ

人間1人とAI3人の対局を、ブラウザで操作する例です。

## 起動

LinuxでPython 3.12以降とC/C++の開発環境を用意し、リポジトリのルートで実行します。導入の詳細は[開発環境](../../docs/development/reproducibility.md#導入)を参照してください。

```sh
python3 scripts/bootstrap.py --profile frontend
source dist/tools/activate.sh
python3 scripts/build_wasm.py
npm ci --ignore-scripts --prefix packages/suzume
npm run build --prefix packages/suzume
python3 -m http.server 8080 --bind 127.0.0.1
```

[対局画面](http://127.0.0.1:8080/examples/browser/)を開きます。WASMを読み込むため、HTTPで配信してください。

## 操作

| 操作 | 動作 |
|---|---|
| 手牌・補助操作を押す | 打牌、鳴き、和了などを選ぶ |
| 「あなたの手番まで進める」 | AIをまとめて進める |
| 「CPUを1手進める」 | AIを1操作だけ進める |
| 配牌の種・CPU設定を選び「新しい対局」を押す | 設定を反映して対局を始め直す |

表示席は0です。画面を開くと種0・standardで対局が始まり、「新しい対局」は現在の対局を置き換えます。

[SDKの使い方](../../docs/api/sdk.md)・[AIの仕組み](../../docs/ai/algorithms.md)・[ブラウザ検査](../../docs/development/reproducibility.md#ciと公式資料)
