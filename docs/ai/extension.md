# 木探索・学習器を接続する

## 対局規則と判断処理を分ける

`Policy`は`Observation`と合法手を借用し、`Option<Action>`を返す差替え点です。
新しい算法はこの型を実装し、ルール・点数・鳴き・振聴を再実装せず、選んだ手を`Game::apply`へ渡します。
既定の三算法は選択肢の列挙型です。新しいRustのPolicyを追加するだけなら、その列挙型を変更する必要はありません。
外部APIの組込み算法名に追加する場合は、設定型・生成スキーマ・SDK・ログ互換を同じ変更で更新します。

次は、公開観測の検査を再利用する最小の接続例です。強い戦略や学習済みモデルの例ではありません。

```rust
use suzume::ai::{unseen_tile_counts, Policy};
use suzume::game::{Action, Game, Observation};

struct FirstLegal;

impl Policy for FirstLegal {
    fn choose_action(&self, observation: &Observation, legal: &[Action]) -> Option<Action> {
        let _unseen = unseen_tile_counts(observation, legal).ok()?;
        legal.first().cloned()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game = Game::new(7)?;
    let seat = game.next_actor().ok_or("手番がありません")?;
    let view = game.observation(seat)?;
    let legal = game.legal_actions(seat);
    let action = FirstLegal.choose_action(&view, &legal).ok_or("操作を選べません")?;
    game.apply(seat, action)?;
    Ok(())
}
```

## 木探索を追加する場合

観測から既知牌を引き、残る牌の割当てを仮説として作ります。
探索の各仮説で合法な遷移を進め、根の操作ごとの評価値を集めます。
仮説の乱数種、展開回数、終了時の集計方法を実効設定として記録してください。
深さや計算回数の上限を満たし、停止した理由と未評価値を説明できる設計にします。

信頼する実験コードでは、研究用の`Environment::fork`で状態を分岐できます。
この分岐は真の山と他家手牌を保持する完全情報の複製です。
通常の不完全情報プレイヤーを評価するときに、この内部状態をPolicyへ渡してはいけません。
仮説からの探索と、真の状態を知る参考実験は別の条件として記録します。

## 強化学習の接続

`crates/suzume-api`の研究用環境は、現在の手番の観測、189個の固定行動番号と合法手mask、
1操作のstep、4席の点数差報酬、`terminated`と`truncated`を提供します。
同じ牌の複数コピーは固定行動番号でまとめます。予約された無効番号は常に選択不可です。
行動番号と正規化の規則は [ADR 0006](../adr/0006-ai-environment-config.md) に記録しています。

学習器側では、観測の符号化、mask後の行動選択、割引率、報酬整形、更新処理、重みの保存を担当します。
次の観測の手番は別の席になり得るため、遷移を無条件に同一エージェントの連続した行動として扱いません。
報酬は点数の差で、リーチ供託の支払いも含みます。供託は卓の別口座にあるため、
4席の報酬の和が各操作で常に0になるとは限りません。

対局の通常終了と、実験上限による打切りは区別します。
打切りを勝敗が確定した終了として学習させるか、後続価値を扱うかは、学習目的に応じて明示してください。
完全ログからの再生と、非公開情報を除いた共有ログも別形式です。
他家の判断説明や完全再生用seedを、通常プレイヤーの観測特徴へ混ぜないでください。

## 参照した観点

[clarkwkw/mahjong-ai](https://github.com/clarkwkw/mahjong-ai/blob/master/README.md)は香港麻雀を対象とし、
経験則・木探索・Deep Q学習・方策勾配という複数方式と対戦評価を分けています。
本件では判断方式の境界と比較評価という観点を参考にしました。
学習コード・学習済み重み・香港麻雀の規則は流用していません。
