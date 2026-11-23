# 作業書の検査名と実装の対応

[作業書](../workdocs/workdoc_Nov23-2026_suzume.md) の検査名は計画時の名称です。
以下は、2026年11月23日の実装と `cargo test --locked -- --list` の出力を照合した対応表です。
名前の対応は検査の存在を示します。検査に成功した根拠や失敗先行の履歴とは区別します。

## 計画した検査

| 作業書の名称 | 実装された検査 | 対象・所在 |
|---|---|---|
| `generator_rejects_unknown_rule` | 否定検査 `N-01 unknown rule` | 未知の宣言を追加した一時コピーを生成器が拒否。[check_contract_negatives.py](../../scripts/check_contract_negatives.py)。Cargoのテスト関数ではありません |
| `transfer_points_is_atomic` | `contracts::tests::rejected_transfer_leaves_every_account_unchanged`、`contracts::tests::points_transfer_preserves_total_including_escrow` | 拒否時の全口座不変、成功時の点数保存。[contracts.rs](../../src/contracts.rs) |
| `move_one_preserves_sum` | `contracts::tests::tile_move_preserves_count_and_rejects_empty_or_full` | 牌の移動、空・満杯・不正枚数の拒否。[contracts.rs](../../src/contracts.rs) |
| `invalid_tiles_are_rejected` | `tile::tests::every_byte_is_valid_exactly_for_34_tiles`、`tile::tests::parser_rejects_red_incomplete_and_fifth_copies` | 全u8の牌ID、表記、不正な5枚目。[tile.rs](../../src/tile.rs) |
| `tile_notation_round_trip` | `tile::tests::compact_text_round_trips_all_tiles` | 全34種類の表記往復。[tile.rs](../../src/tile.rs) |
| `standard_seven_pairs_orphans` | `hand::tests::recognizes_standard_seven_pairs_and_orphans`、`hand::tests::enumerates_both_waits_and_thirteen_orphan_waits` | 通常形・七対子・国士、和了前の向聴数と待ち。[hand.rs](../../src/hand.rs) |
| `no_yaku_is_rejected` | `scoring::tests::dora_alone_cannot_make_a_yaku` | ドラがあっても役なしは拒否。[scoring.rs](../../src/scoring.rs) |
| `pinfu_tsumo_twenty_fu` | `scoring::tests::pinfu_tsumo_is_twenty_fu_and_ron_is_thirty` | 平和ツモ20符と門前ロン30符。[scoring.rs](../../src/scoring.rs) |
| `ron_triplet_is_open_for_fu` | `scoring::tests::ron_triplet_is_open_for_fu_and_three_concealed_triplets` | ロンで完成した刻子の符と暗刻数。[scoring.rs](../../src/scoring.rs) |
| `ai_returns_legal_actions` | `ai::tests::ai_returns_only_legal_actions` | 実際の対局を進め、CPUの選択が合法手に含まれることを確認。[ai.rs](../../src/ai.rs) |
| `cli_help_and_invalid_arguments` | `tests::cli_help_and_invalid_arguments`、`help_is_available_without_starting_a_game`、`invalid_cli_arguments_fail_with_a_reason` | 引数解析の単体検査と実行ファイルを起動する検査。[main.rs](../../src/main.rs)、[cli.rs](../../tests/cli.rs) |
| `illegal_action_is_atomic` | 同名の結合テスト | 範囲外の席・牌番号、手番違い、成立しないロンを拒否し、対局全体を不変に保つ。[game_contracts.rs](../../tests/game_contracts.rs) |
| `seeded_games_preserve_invariants` | 同名の結合テスト | 種0〜3の4試合で各操作の不変条件と試合終了を検査。[game_contracts.rs](../../tests/game_contracts.rs) |

Kaniの6ハーネスとLeanの18定理は [契約対応表](../contracts/README.md) と
[形式検証の範囲](README.md) に実名・入力域・限界を記載しています。
これらをCargoの76テストへ加算しません。

## 名前付き単体テスト以外の受入検査

| 作業書の対象 | 実装・記録 | 検査の範囲 |
|---|---|---|
| 136枚・各牌4枚・点数と供託の保存 | `Game::apply` と `Game::validate`、上記の対局結合テスト、[17件の局面テスト](../../src/game/tests.rs) | 配牌、打牌、応答、鳴き、槓、和了、流局、次局。局面テストの初期状態は全牌を配分した整合する状態として構築し、その後を公開操作で進めます |
| 種0〜31の32試合 | `cargo run --release -- --simulate 32 --seed 0` | 上表の4試合の結合テストとは別の受入検査です。全32試合を数え、失敗や操作上限到達は非0終了とします |
| 配牌・打牌・局終了・次局の端末操作 | [ui-playthrough.txt](logs/ui-playthrough.txt) | 実端末で観測した入力と出力の記録です。CLIの引数検査だけで実操作を代用しません |
| 不正な宣言・生成物の不一致 | `N-01`〜`N-03`、`generate_rules.py --check` | 未知の規則、未対応の値、生成物の変更をそれぞれ拒否します |
| 誤実装・偽の定理の拒否 | `N-04`・`N-05` | 一時コピーの次席計算の変異をKaniが拒否し、偽の次席定理をLeanが拒否します |

## 件数と証跡の読み方

列挙結果はライブラリ61件、実行ファイル内6件、CLI結合5件、対局結合4件の計76件です。
文書テストは0件です。これは件数の照合であり、列挙コマンド自体はテスト本体を実行しません。
Rust 1.80.1で全76件が成功した実行記録は [msrv.txt](logs/msrv.txt) にあります。

失敗先行を実際に記録した契約検査は [contracts-red.txt](logs/contracts-red.txt) と
[contracts-green.txt](logs/contracts-green.txt)、役・符検査は [scoring-before.log](logs/scoring-before.log) と
[scoring-after.log](logs/scoring-after.log) を対応させます。河底の不具合については
[game-houtei-before.log](logs/game-houtei-before.log) と [game-scenarios-after.log](logs/game-scenarios-after.log) が対応します。
これらの部分検査の件数は当時の対象数です。後から加わった検査を、過去に失敗した検査として扱いません。
