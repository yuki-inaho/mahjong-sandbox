# 旧JSONL形式：厳密再生と学習用遷移

通常の対局出力は [mjson](README.md) です。この文書は明示的に `replay` / `public` を選んだ場合の互換仕様を保持します。
乱数種と内部操作による厳密再生、AI判断説明、189行動IDの学習用遷移には `replay` を使用します。
既存の検査ログは当時の対象と結果として残し、新形式の検査結果へ読み替えません。

## 目的と利用範囲

感想戦、統計、AIエージェントによる読解、探索・学習の準備に使う版付き UTF-8 JSON Lines です。1行が1個の JSON オブジェクトであり、画面の日本語メッセージを逆解析する必要はありません。`crates/suzume-api/src/logging/` が形式・書込み・読込みの正本です。採用理由は[ADR-0005](../adr/0005-game-records.md)、入出力の条件は[公開APIの境界](../contracts/platform-boundaries.md)に記載しています。

完全ログは乱数種から全ての隠れた牌を復元できます。信頼するローカル分析者向けであり、対局中の相手へ渡す情報ではありません。共有ログは異なる型を使い、完全再生器が拒否します。どちらの形式も署名・本人認証・公平なオンライン対局の証明を提供しません。

## 二つの形式

| 項目 | `replay`：再現用 | `public`：共有用 |
|---|---|---|
| 先頭 | `replay_header` | `public_header` |
| 適用済みの操作 | `replay_action` | `public_step` |
| 末尾 | `replay_footer` | `public_footer` |
| 乱数種 | 先頭に十進文字列として保存。完全な `u64` を保持 | 対局・AIとも保存しない |
| 設定 | 入力設定と解決後のAI設定の両方 | 最大操作数と、人間/AI・算法・予算・深さ・防御重み |
| 操作 | 共通 `WireAction`。手牌の物理添字を保持 | 公開された牌種による打牌・リーチ・暗槓等。私的な添字はない |
| 判断説明 | 任意の `DecisionView`。選択操作・実効設定と一致する必要あり | 保存しない |
| 状態 | 決定的な局面投影の SHA-256 | 閲覧席を持たない `GameView` |
| 再生 | 同じ版の実ゲームへ順に操作を適用 | 再生不可を明示して拒否 |

共有ログの `ResponseSubmitted` は、他家の打牌への回答が提出されたことだけを示します。パス・ロン・ポン・チー・明槓の未確定の回答内容を記録しません。全員の回答が揃った後に、実際に採用された副露や和了結果が公開局面に現れます。採用されなかった回答は公開されません。

共有ログの `own_hand`、`own_last_draw`、`own_furiten`、`viewer` は常に `null`、`legal_actions` は空です。和了結果に現れる勝者の手牌と、卓上に公開された副露は公開情報として含みます。山の並び、王牌、他家の伏せた手牌、内部回答配列、非公開状態のハッシュは含みません。

## 版・連番・終了の契約

形式版は1、エンジン版は0.2.0です。規則の識別には `rule_constants::DECLARATION_SHA256` を使い、規則と契約宣言の組を識別します。シャッフルの識別子は `splitmix64-fisher-yates-v1` です。対応していない版・エンジン・規則・シャッフルは推測で読み替えません。

`sequence` と `revision` はどちらも適用済み操作数です。最初の操作は1、以後1ずつ増加します。ヘッダの局面は revision 0、フッタの `action_count` と `revision` は最後の操作数です。ログ内の観測用 `session_id` は常に1であり、外部アプリケーションの接続IDを引き継ぎません。

全ての操作の actor は、直前の `Game::next_actor()` と一致しなければなりません。core 自体が扱える任意順の回答受付を、ログでは順序の自由として公開しません。操作が不正、連番が欠落・重複、ヘッダが途中に再出現、フッタの後に別の記録がある場合は失敗です。

| `reason` | 必要な条件 | 解析用の終了フラグ |
|---|---|---|
| `completed` | 実局面が `Phase::MatchEnd` | `terminated=true, truncated=false` |
| `truncated` | 未終局で、操作数が設定 `max_steps` と等しい | `terminated=false, truncated=true` |
| `interrupted` | 未終局で、操作数が設定 `max_steps` より小さい | `terminated=false, truncated=true` |

操作が0件の明示中断も有効で、ヘッダとフッタの2行になります。単に入力が終わりフッタがない場合は、明示中断ではなく不完全なログです。最終行の改行省略と CRLF は受理します。空行、コメント、複数行に整形した JSON は受理しません。

## 状態の照合対象と限界

完全ログの先頭、各操作の前後、末尾に局面投影の SHA-256 を記録します。投影は4席それぞれの `GameView` を固定順の配列にし、`serde_json` の構造体フィールド順で直列化したバイト列です。各席の手牌・自分のツモ・振聴状態、公開局面、次の手番の合法手を含みます。キー順のある任意 JSON オブジェクトを外部から渡してハッシュする形式ではありません。

この投影には山・王牌の順序、core の全ての内部フラグ、乱数生成器の状態、累積 `history`、累積 `events` を含みません。完全な内部状態の同一性をハッシュだけで証明する設計ではありません。再生器は seed と操作列を同じ core に適用し、回帰テストでは終点の実 `Game` 全体と `history` の等値を直接確認します。

ハッシュと識別子は偶発的な不一致の検出に使います。攻撃者が操作列とハッシュを整合的に作り直すこと、虚偽の判断説明を作ることは防げません。判断説明は選択操作・実効設定・計算量の上限との整合を検査しますが、全候補の評価を再計算して説明の真実性を証明するものではありません。同じ版名で異なるソースを使うことまで、ログだけで排除することもできません。配布物とソースの SHA は[再現手順](../development/reproducibility.md)側で管理します。

## 書込みインターフェース

`LogWriter<W: Write>::new(writer, &config, &game)` は、初期状態から記録を始めます。履歴が空で、対局の seed が設定と一致し、設定と局面が有効であることが必要です。形式は `config.logging.format` で選びます。

成功した `Game::apply` ごとに `record(actor, &action, &game_after, decision)` を1回呼びます。直前に期待した手番、履歴件数、最後の actor と操作を検査します。同じ局面の二重記録、飛ばした操作、別の seed の局面を受理しません。渡す `Game` が同一の履歴から継続したという呼出側の責務もあり、完全な遷移の独立検査は再生器が行います。

`decision` を渡す場合、記録する操作と設定に一致しなければなりません。人間の席にAI判断を付ける場合も受理しません。公開形式を選ぶと説明は出力されません。`finish(&game, reason)` はフッタを書き、`flush` の成功後に元の書込み先 `W` を返します。途中の I/O 失敗では不完全な出力が残り得ます。一度部分書込みに失敗した書込み器は、その後の `record` を拒否します。

```rust
use suzume::game::Game;
use suzume_api::{config::{AppConfig, LogFormat}, logging::{EndReason, LogWriter}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.logging.format = LogFormat::Replay;
    let mut game = Game::new(config.game.seed.value())?;
    let mut log = LogWriter::new(Vec::new(), &config, &game)?;
    let actor = game.next_actor().ok_or("no actor")?;
    let action = game.legal_actions(actor).into_iter().next().ok_or("no action")?;
    game.apply(actor, action.clone())?;
    log.record(actor, &action, &game, None)?;
    let bytes = log.finish(&game, EndReason::Interrupted)?;
    assert!(!bytes.is_empty());
    Ok(())
}
```

書込み器は全ログを内部配列へ蓄積しません。例の `Vec` は利用者が選んだ保存先であり、実ファイルでは `BufWriter<File>` 等に置き換えます。局面要約を作る際も過去の表示記録や履歴を毎回複製・ハッシュしません。

## 読込みと上限

`replay(&mut reader, LogLimits)` は `BufRead` を受け取り、1行ずつ厳密に検査して `ReplayResult { game, metadata, termination }` を返します。全入力を先に文字列へ読む必要はありません。

| 制限 | 既定値 | 数える対象 |
|---|---:|---|
| `max_line_bytes` | 1,048,576 | 改行前のバイト列。CRLFのCRを含む |
| `max_records` | 10,002 | ヘッダ・全操作・フッタ |
| `max_total_bytes` | 268,435,456 | 改行を含む全入力 |
| 設定 `max_steps` | 1〜10,000 | 実ゲームに適用する操作 |

各上限は正でなければなりません。行を組み立てる前に長さを検査し、過大行全体を読み込んでから拒否する処理にはしません。重複 JSON キーは全ての入れ子で拒否し、その後に共通の型で未知項目、誤った数値、未知のタグ、範囲外の席・牌・添字を検査します。ログの数値は整数のみです。64bitの seed や符号付き評価値は共通APIの十進文字列を使います。

読込み器が保持するのは1行の JSON、現在のゲーム、必要に応じて1件の未出力遷移です。ただし実 `Game` 自体の履歴は操作数に比例して増えます。全プロセスのメモリーが対局長に無関係に一定であるという主張ではありません。

## 解析・学習用の遷移

`export_transitions(reader, limits, consumer)` は再生と同じ検査を行い、各操作を `DatasetTransition` として callback へ渡します。

| フィールド | 定義 |
|---|---|
| `observation`、`actor` | 操作前の手番の観測。自身の手牌と公開情報、189要素の合法手mask |
| `action` | 正確な `WireAction`。再生用の手牌添字を保持 |
| `action_id` | 共通 `actions::action_id` で符号化した0〜188の番号 |
| `rewards` | `points_after - points_before` を4席の順で保存。点数単位 |
| `escrow_delta` | リーチ供託の残高差。報酬ベクトルとの合計は0 |
| `next_actor`、`next_observation` | 次の手番の席と観測。操作前の席とは限らない |
| `final_observation` | 打切り時だけ保持する最後の次手番観測。価値推定の継続に使用できる |
| `terminated`、`truncated` | 自然な対局終了と、操作上限または外部中断を区別 |

同じ牌種の複数の物理添字は同じ `action_id` になります。正確な replay は `action` を使い、探索側の正規化された選択は `action_id` を使います。番号の定義をログだけで重複実装しません。

最終遷移では `next_observation` と `next_actor` を `None` にします。ただし打切りは麻雀の終局ではないため、その局面の次手番観測を `final_observation` に残します。これにより、時間制限で中断した局面の価値を継続して推定する学習器が利用できます。その観測の `actor` が視点の席を示します。自然な対局終了と通常の継続操作では `final_observation` は `None` です。

外部中断の `truncated` は、最後の操作を読んだ時点ではまだ分からないため、1件だけ未出力で保持し、検証済みフッタを読んでから確定します。0操作の中断では遷移を出力しません。割引率、終局以外の報酬整形、勝率、学習アルゴリズムはこの形式に暗黙に加えません。

途中まで callback で出力した後に、不正な後続行やフッタ、I/O障害が見つかる場合があります。callback の実行回数を、全ログが検証済みである証拠にしてはいけません。完全な成果物として保存する処理は一時ファイルへ出力し、関数全体が成功してから最終名へ移します。callback 自身の失敗もそのまま返します。

## 検査入口

```sh
cargo test -p suzume-api --test logging
cargo clippy -p suzume-api --all-targets -- -D warnings
```

再生終点と履歴の一致、設定の一致、不明版・誤値・重複・順序違反・欠落・改変・巨大行・I/O障害、公開情報の制限、環境APIとの観測・行動・報酬の一致を名前付き回帰で検査します。最初の[未実装による失敗](../verification/logs/platform-logging-before.txt)はモジュールが存在しないためのコンパイル失敗であり、実装済みの動作が12件失敗したという意味ではありません。修正後の実結果は検証ログで別に保存します。

打切り後の最後の観測を保持する追加条件は、[動作上の失敗](../verification/logs/platform-log-final-observation-before.txt)を先に確認し、[全16検査成功](../verification/logs/platform-log-final-observation-after.txt)まで実行しました。CLI/SDK/実WASM/配布物を含む全体の受入は、この部分検査とは別の総合入口で確認します。
