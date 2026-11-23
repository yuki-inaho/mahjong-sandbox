# 麻雀スズメ

ひとりで麻雀を遊び、対局を振り返り、AIの判断を試すためのプロジェクトです。人間1人とAI3人で四人リーチ麻雀の東風戦を遊べます。

## 端末で遊ぶ

Linux、Python 3.12以降、C/C++の開発環境、ツール取得用のネットワークが必要です。リポジトリのルートで実行してください。

```sh
python3 scripts/bootstrap.py --profile native
source dist/tools/activate.sh
cargo run --locked -p suzume-cli --release
```

日本語を表示できる、横幅92列以上の端末を使用してください。入力後にEnterで確定します。

| 入力 | 操作 |
|---|---|
| `1`〜`14` | 表示された番号の牌を捨てる |
| `r 5` | 5番の牌を捨ててリーチする |
| `/1`、`/2`… | 表示された鳴き・和了などを選ぶ |
| `Enter` | 鳴きなどを見送る、局終了後に次局へ進む |
| `h` / `q` | 操作説明を表示する / 対局を中断する |

## 文書

| 目的 | 案内 |
|---|---|
| 詳しい操作・表示・設定 | [遊び方](docs/playing.md) |
| 採用しているルールを知る | [ルール仕様](docs/rules/riichi.md) |
| 対局を保存・分析する | [mjson牌譜と観測出力](docs/logging/README.md) |
| AIの仕組みや評価を知る | [AIの案内](docs/ai/README.md) |
| ブラウザで動かす・アプリに組み込む | [ブラウザ例](examples/browser/README.md)・[SDK](packages/suzume/README.md)・[API](docs/api/README.md) |
| 開発・検査する | [開発手順とスキル](docs/development/agent-workflow.md) |

設計や詳細仕様は[文書一覧](docs/README.md)から参照できます。

## 参考

- [tenhou-to-mjai](https://github.com/NikkeTryHard/tenhou-to-mjai)
- [MiHaJong](https://github.com/MihailJP/MiHaJong)

[MITライセンス](LICENSE)
