# AIの設定と判断の確認

すずめには、Random・Greedy・Ukeireの三つの算法があります。いずれも学習済みモデルではありません。
`Policy`へ渡す情報は、自分の手牌と公開観測、およびその局面の合法手です。
真の山順序や他家の手牌を判断へ渡しません。

| 算法 | 主な判断 | 残る制限 |
|---|---|---|
| `random` | 和了・次局・唯一の操作を優先し、それ以外は合法手の一覧から均等に選択 | 同じ牌の別添字や打牌とリーチは別の選択肢です。牌効率は評価しません |
| `greedy` | リーチを優先し、向聴数、リーチ者の河、残る牌のつながりを比較 | 有効牌の残枚数を使わず、残り0枚の待ちを選ぶことがあります |
| `ukeire` | 公開情報から残枚数を数え、有効牌、防御重み、ドラを含めて比較。設定により先読み | 相手の手牌推定や正確な和了確率・期待得点は計算しません |

**プリセットは算法と計算予算の組です。勝率の順位を保証しません。**

| プリセット | 算法 | `max_evaluations` | `lookahead_depth` | `risk_weight` |
|---|---|---:|---:|---:|
| `beginner` | random | 0 | 0 | 0 |
| `standard` | greedy | 1024 | 0 | 1 |
| `strong` | ukeire | 8192 | 2 | 4 |

## Rustから使う

`ConfiguredAi::new`は設定を検査し、`decide`は判断結果と根拠を返します。
従来の`GreedyAi`も維持しています。どちらも`Policy`を実装します。

```rust
use suzume::ai::{AiPreset, ConfiguredAi};
use suzume::game::Game;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game = Game::new(42)?;
    let seat = game.next_actor().ok_or("手番がありません")?;
    let observation = game.observation(seat)?;
    let legal = game.legal_actions(seat);
    let ai = ConfiguredAi::new(AiPreset::Strong.resolve(7))?;
    let decision = ai.decide(&observation, &legal)?;
    println!("理由={} 計算回数={}", decision.reason.as_str(), decision.evaluations);
    game.apply(seat, decision.action)?;
    Ok(())
}
```

ゲームの乱数種とAIの乱数種は別です。上の例では配牌が42、AIの選択が7です。
同じ版・観測・合法手一覧・実効設定なら、同じ判断結果を返します。
OS乱数や現在時刻を算法の内部から参照しません。

## TOMLと外部API

[example.toml](example.toml) は、人間1人と異なる設定のCPU3人を定めた完全な構成例です。
TOMLの読込とJSONへの変換は`suzume-api`が担当します。
Rust coreにはTOMLや直列化ライブラリの依存を加えていません。

`preset`と`algorithm`は同時に指定できません。選んだプリセットまたは算法の既定値へ、
明示した予算・深さ・防御重みを上書きし、最後に組合せを検査します。
64ビットの`seed`はTOML/JSONでは十進文字列、Rustの`AiConfig`では`u64`です。

| 項目 | 条件 |
|---|---|
| `max_evaluations` | 上限100,000。Randomは0、他の算法は1以上 |
| `lookahead_depth` | Random/Greedyは0、Ukeireは1または2 |
| `risk_weight` | 0〜1000。Randomは0 |
| `seed` | Rustでは全u64。外部設定では正規形の十進文字列 |

Greedyの防御は向聴数が等しい場合だけ使います。重み0で無効になり、正値同士では順位は変わりません。
Ukeireでは防御項を他の評価項から差し引くため、重みの大きさも判断へ反映されます。
これは放銃確率を表す数値ではありません。

## 判断記録の意味

| フィールド | 意味 |
|---|---|
| `config` | プリセットを解決し、検証した実効値 |
| `action` / `reason` | 選択した操作と固定された理由名 |
| `candidates` | 受け取った合法手と同じ順序の候補。未計算の数値は`None` |
| `shanten` | 候補を進めた後の向聴数。0はテンパイ、-1は和了形 |
| `ukeire` | 向聴数を下げる牌について、未公開枚数を足した値 |
| `danger` | 他家のリーチと河だけで計算する粗い危険度 |
| `lookahead_value` | 完了した標本探索の評価値。点数や確率ではありません |
| `candidates_evaluated` | 向聴数まで実際に求めた候補数 |
| `evaluations` | 実際に呼び出した向聴計算の回数。計算済み結果の再利用は含みません |
| `budget_exhausted` | 追加計算を必要とした時点で予算が尽きたか |

和了・次局・唯一の操作では探索をせず、計算回数0の判断になる場合があります。
深さ1の計算が途中ならGreedyの基準へ戻り、未計算の有効牌数を0枚とみなして比較しません。
深さ2の根のいずれかが未完了なら、完了した根だけを有利に扱わず、深さ1の比較へ戻ります。

`Decision`自体は局面IDを発行しません。APIとログの側で、観測・session・revision・操作列に結び付けます。
他家の判断記録にはその人の手牌を推測できる値が含まれるため、通常の共有用ログへそのまま載せません。

## 入力と責務

AIの入口では席・手牌番号・副露・構造枚数・公開河・各牌4枚などを検査し、
不整合を`AiError`で拒否します。`Policy::choose_action`を使う場合は不正入力に`None`を返します。
この検査は、役・振聴・応答中の手番まで含む合法手判定の代用ではありません。
合法手一覧は`Game::legal_actions`から取得し、選択の適用は`Game::apply`へ委譲してください。

詳細は [算法と予算](algorithms.md)、[追加算法と学習環境の接続](extension.md)、
[CLIでの設定と記録](cli.md)、[固定評価](evaluation.md)、[評価結果](results.md) を参照してください。
