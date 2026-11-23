# 契約の一覧と境界

## 何を契約として扱うか

操作を受け付ける条件を**事前条件**、成功後に成り立つ値の関係を**事後条件**、操作の前後で保つ性質を**不変条件**と呼びます。本プロジェクトでは不正入力を `None` または `Err` として拒否します。点数を変更する操作は、すべての検査を終えてから書き込みます。

数値と採用規則の宣言は [riichi.rules](../../rules/riichi.rules)、基本契約の数式は [contracts.json](../../rules/contracts.json) が正本です。[生成器](../../scripts/generate_rules.py) が同じ式からRustの比較仕様とLeanの仕様を作ります。ゲームで使う [Rust実装](../../src/contracts.rs) は独立に記述し、Kaniで宣言との対応を検査します。

## 対応表

| ID | 実際に使うRust関数 | 事前条件と拒否条件 | 成功時の事後条件 | Lean側の主な定理 |
|---|---|---|---|---|
| C-01 | `next_seat` | 席は0〜3。範囲外は拒否 | 次席は `(席+1) mod 4`、引き続き0〜3 | `next_seat_refines_declaration`、`next_seat_remains_valid`、`four_turns_return_to_same_seat` |
| C-02 | `dora_from_indicator` | 牌IDは0〜33。範囲外は拒否 | 数牌は同じ種類内で1→…→9→1、風牌は東→南→西→北→東、三元牌は白→發→中→白 | `dora_refines_declaration`、`dora_remains_valid`、`dora_preserves_family` |
| C-03 | `move_one` | 元は1〜4枚、先は0〜3枚。空・満杯・不正枚数は拒否 | 元は1減少、先は1増加、合計保存、両側0〜4枚 | `move_one_refines_declaration`、`move_one_conserves_and_bounds` |
| C-04 | `transfer_points` | 口座0〜4、移動元≠先、金額≥0、両側がi32内に収まる。違反は拒否 | 指定額が片方から減り他方へ増える。総和保存、無関係な3口座は不変。失敗なら5口座とも不変 | `point_transfer_refines_declaration`、`point_transfer_conserves_total`、`point_transfer_preserves_other_accounts`、`point_transfer_remains_in_i32`、`rejected_transfer_is_atomic` |
| C-05 | `can_declare_riichi` | 門前、聴牌、1000点以上、山4枚以上、未宣言のすべてが必要 | 戻り値が5条件の論理積と一致し、供託後の持点は非負 | `riichi_refines_declaration`、`riichi_deposit_is_affordable` |
| C-06 | `discard_allowed` | ツモ牌と打牌候補が有効なID。リーチ後は両者が一致 | リーチ後の手出しを許さない | `discard_refines_declaration`、`riichi_forces_drawn_tile` |

口座0〜3は持点、口座4はリーチ供託です。支払いで負の持点になることはC-04では許します。飛びによる対局終了は対局処理の責務です。同じ口座への移動は金額0でも拒否します。金額0の異なる口座間移動は合法です。

`move_one` は、ある1種類の牌について二つの保管場所の枚数を移す契約です。この関数だけでは全136枚が保存されていること、牌山順序や鳴きの履歴が正しいことを証明しません。それらは対局状態の実行時検査と結合テストの対象です。

## 契約を呼び出す側の責任

契約が受け取る事実そのものの正しさは、呼び出し側が保証します。例えばC-05の `tenpai` が本当に聴牌を示すかは、和了形・向聴数の判定器の責務です。`closed` が門前を示すか、牌の移動先が本来の手牌であるか、誰が和了したか、符と翻が正しいかも、これらの小さな契約だけでは判断できません。

対局の公開操作は、合法手の列挙・事前条件の確認・状態の更新・不変条件の確認という順で適用します。下位関数の成功を、対局全体の正しさと読み替えません。契約の呼び出し箇所は `src/game.rs` と `src/tile.rs` から追跡できます。

## ルール変更時の手順

1. `rules/riichi.rules` または `rules/contracts.json` を変更します。現在の生成器は、初版で未対応の規則を受け付けません。対応範囲を広げるときは生成器の対応条件も変更します。
2. `python3 scripts/generate_rules.py` でRustとLeanの生成物を更新します。
3. 変更された契約に対応する実装、Kaniハーネス、Lean定理、具体例のテストを更新します。
4. `python3 scripts/generate_rules.py --check`、`cargo test`、`cargo kani`、`cd proofs && lake build` を実行します。
5. 影響した規則・操作・文書をレビューしてからADRと作業書を更新します。

生成された `.rs` / `.lean` を直接編集しません。`declarations_current` はビルドに含まれる宣言文字列と生成物の文字列を照合します。内容の正しさをハッシュの一致だけで保証するものではありません。
