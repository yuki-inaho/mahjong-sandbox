# 0.2.0の完了条件と検査の対応表

この文書は、[追加作業書](../workdocs/workdoc_Nov23-2026_platform.md)のDOD-P01〜DOD-P12と、実在するテスト・検査入口を対応付けます。実装の照合基準は `9d2081abd14c8d8e1c05a220260bd9ac04843af7` です。初版の対応表は[初版テスト対応](test-mapping.md)として保持します。

**テストの存在、開発中の成功、最終受入の成功を区別します。** 本文にテスト名があることだけでDoDを完了とは判定しません。作成時点で確認した補助検査4系列と、まだ完了を主張しない全体検証・配布消費・実ブラウザ・最終CIを末尾に分けて記載します。以後の完了判定は、作業書に追記する対象ソース・実行ID・実出力と照合してください。

不具合の再現と修正理由は[独立レビュー](../reviews/platform-review.md)、採用規則は[四人リーチ麻雀の固定規則](../rules/riichi.md)、基本契約の意味は[契約一覧](../contracts/README.md)が正本です。この対応表は、それらの仕様を新しく定義し直すものではありません。

## 1. DoDから検査への対応

| 完了条件 | 主な検査対象 | 最終判定で追加して確認すること |
|---|---|---|
| DOD-P01：再現した重大欠陥の修正 | 第2節の席・操作ラベル・同巡振聴・履歴、第3節の生成器、第4・7・8節の外部境界。レビューF01〜F16との対応 | 修正前後の実記録と、固定した最終ソースでの回帰成功。速度の改善倍率など未測定の効果を含めない |
| DOD-P02：固定環境から再現可能 | `bootstrap.py`、`doctor.py`、固定版と依存ロック、第9節の独立消費、補助 `platform-bootstrap-evidence` | 新規導入と既存道具の再利用を区別。別ディレクトリでの構築を要求し、OS全体や全バイナリのビット一致とは扱わない |
| DOD-P03：版付きAPIの境界 | 第4節の要求・応答・設定・標準入出力、原子性、閲覧席の情報制限 | JSON Schema適合と、その局面での操作合法性を区別。認証サーバーや敵対するRust呼出し元への隔離は対象外 |
| DOD-P04：実WASM・JS/TS・最小画面 | 第8節のnative/WASM比較、SDK、型否定例、実ブラウザ検査 | Node内のWebAssembly実行とブラウザ画面の操作を別々に受け入れる |
| DOD-P05：TOML設定が実動作へ反映 | 第4節の設定往復と拒否、第7節のCLI設定・判断記録・再生比較 | CLI・API・完全ログの実効算法・予算・乱数種が一致すること |
| DOD-P06：算法と固定評価 | 第5節の入力・合法手・決定性・計算予算、固定64試合、補助 `platform-ai-reproduction` | 全試合と失敗を母数に残す。計算予算と対戦強度を区別し、測定外の強さを保証しない |
| DOD-P07：探索・学習用の環境 | 第6節の189行動・mask・reset・分岐・点数差報酬・終了と打切り | 同牌の物理添字を復元する一対一符号化ではないこと、打切り時の `final_observation`、全情報の `fork` の用途を明記 |
| DOD-P08：ログ・再生・遷移出力 | 第7節の完全／共有ログ、I/O、改変拒否、環境との一致、補助 `platform-log-cli-integration` | 末尾とEOFを検証するまで途中出力は暫定。共有ログから完全再生できるとは扱わない |
| DOD-P09：配布物を独立に消費 | 第9節のcrate・npm・source・実行ファイルの消費とSHA256一覧 | 作成だけで完了にせず、生成した書庫を別領域へ展開・導入して使用する。登録先への公開は対象外 |
| DOD-P10：品質・形式検証・誤成功の拒否 | 第3節のKani6ハーネス・Lean18定理・否定検査、第9節のRust/TS・検証器自身 | 空検証・対象欠落・未証明・時間切れを拒否。作業書19-dのrustdocは全体検証スクリプトとは別に実行する |
| DOD-P11：自己完結する文書と証拠 | 本表、[文書入口](../README.md)、[再現手順](../development/reproducibility.md)、文書検査器、レビュー | リンクの存在に加え、説明と実装の一致、実行例、証明・可搬性・AI評価の限界を独立に確認 |
| DOD-P12：公開・最終CI・完了監査 | 第10節の `platform-final-git-ci` と固定された完了監査 | 最終HEADとリモートHEAD、作業ツリー、対象コミットのCI結果、監査の必要成果物・実行証拠をすべて照合 |

## 2. ゲームの核・型・採用規則

### 2.1 今回追加した欠陥の回帰検査

| 実在するファイル | テスト名 | 確認する契約と限界 |
|---|---|---|
| [core_boundary_tests.rs](../../src/game/core_boundary_tests.rs) | `seat_lookup_handles_the_full_usize_domain_without_panicking`、`validated_seat_rejects_before_narrowing_the_integer` | 代表的な不正席と `usize::MAX` を拒否し、整数を狭めて別の合法席に変えない。名前に「full domain」があっても、全usize値の列挙や形式証明ではない |
| 同上 | `action_labels_handle_invalid_indexes_without_panicking` | 空手牌・範囲外添字・最大添字の表示でpanicしない。ラベルが表示できても操作を合法とはみなさない |
| 同上 | `committing_an_action_keeps_existing_log_storage` | 十分な容量がある履歴・イベント領域を成功操作で維持し、旧記録を消さない。経過時間や速度倍率の測定ではない |
| [platform_tests.rs](../../src/game/platform_tests.rs) | `temporary_furiten_expires_after_called_discard` | 全136枚を配分した局面から、見逃し→ポン→自己打牌→別の待ちへのロンを公開操作で再現する |
| [game_contracts.rs](../../tests/game_contracts.rs) | `illegal_action_is_atomic`、`a_seed_and_action_history_replay_identically` | 拒否時の状態不変、同じ乱数種と正確な操作列からの再生一致 |
| 同上 | `seeded_games_preserve_invariants`、`opponent_observation_exposes_only_public_information` | 試した対局の牌・点数不変条件と観測の情報制限。任意の全対局を証明する検査ではない |

作業書5-a〜5-dの名前は、そのまま上記の実在名に対応します。修正前の[core失敗記録](logs/platform-core-before.txt)、[同巡振聴失敗記録](logs/platform-furiten-before.txt)と、[core修正後記録](logs/platform-core-after-02.txt)を区別して保存しています。途中の結合失敗を、最終成功や未実装時の実行失敗へ読み替えません。

### 2.2 牌・和了形・得点

これらは初版から継続する規則検査です。現在のワークスペース検査とcore最小Rust版検査にも含めます。

| 実在するファイル | 代表的なテスト名 | 確認範囲 |
|---|---|---|
| [tile.rs](../../src/tile.rs) | `every_byte_is_valid_exactly_for_34_tiles`、`parser_rejects_red_incomplete_and_fifth_copies`、`counting_arbitrary_input_does_not_overflow` | 34牌種、未採用の赤牌、文法と物理4枚の上限、入力枚数の加算 |
| [hand.rs](../../src/hand.rs) | `recognizes_standard_seven_pairs_and_orphans`、`quads_are_not_two_pairs`、`validates_size_multiplicity_and_open_meld_count` | 通常形・七対子・国士、四枚を二対子にしないこと、枚数と副露数の境界 |
| 同上 | `enumerates_both_waits_and_thirteen_orphan_waits`、`keeps_ambiguous_decompositions_for_scoring`、`a_wait_requiring_a_fifth_tile_is_not_tenpai` | 待ちの列挙、複数分解、5枚目を必要とする形の拒否 |
| [scoring.rs](../../src/scoring.rs) | `pinfu_tsumo_is_twenty_fu_and_ron_is_thirty`、`closed_edge_inside_and_pair_waits_add_fu`、`double_wind_pair_counts_four_fu`、`seven_pairs_uses_fixed_twenty_five_fu` | 平和・待ち・連風牌の雀頭・七対子の符 |
| 同上 | `ron_triplet_is_open_for_fu_and_three_concealed_triplets`、`closed_kan_keeps_menzen_and_adds_thirty_two_fu`、`dora_alone_cannot_make_a_yaku` | ロン完成刻子、暗槓、役なしをドラで補わない条件 |
| 同上 | `yakuman_can_stack_but_special_waits_are_not_doubled`、`four_kans_accept_eighteen_physical_tiles`、`points_follow_declared_limits_without_kiriage` | 役満複合・ダブル役満不採用、四槓の実枚数、切上げなしの点数上限 |
| 同上 | `malformed_melds_and_fifth_physical_tile_are_rejected`、`inconsistent_context_and_absent_winning_tile_are_rejected` | 外部から呼べる得点判定への不整合入力の拒否 |

役・符のテストは具体例による回帰検査です。役判定器全体、向聴数の全牌姿、全支払組合せをKaniやLeanで証明したとは扱いません。

### 2.3 局進行・鳴き・精算

以下はすべて [src/game/tests.rs](../../src/game/tests.rs) にあります。

| 規則 | 実在するテスト名 |
|---|---|
| リーチ成立と供託 | `riichi_is_deferred_until_declaration_tile_survives`、`ron_on_declaration_tile_cancels_the_riichi_deposit`、`calling_the_declaration_tile_establishes_riichi_but_cancels_ippatsu` |
| 見逃しと振聴 | `yaku_less_completion_still_causes_temporary_furiten`、`temporary_furiten_expires_on_the_next_draw`、`riichi_furiten_survives_the_next_draw_and_discard_is_locked` |
| 応答の優先順位 | `ron_has_priority_over_pon_and_chi`、`head_bump_uses_distance_from_the_discarder`、`pon_has_priority_over_chi_and_does_not_create_a_tsumo` |
| チー・食い替え・槓 | `chi_is_only_from_the_left_and_forbids_both_kuikae_tiles`、`open_kan_draws_from_the_dead_wall_and_preserves_inventory` |
| 海底・河底・嶺上 | `rinshan_does_not_combine_with_haitei_when_the_live_wall_becomes_empty`、`last_discard_after_rinshan_can_award_houtei` |
| 支払・本場・供託 | `dealer_tsumo_rounds_each_payment_and_continues_the_dealership`、`nondealer_tsumo_includes_honba_and_the_riichi_pot` |
| 流局・親の進行 | `exhaustive_draw_with_two_ready_players_transfers_fifteen_hundred_each`、`exhaustive_draw_rotates_a_not_ready_dealer_but_keeps_honba` |

これらは宣言した初版プロファイルに対する検査です。加槓・複数ロン・別の途中流局規則など、採用していない規則の成功を意味しません。

## 3. 宣言・Kani・Lean・検証器

### 3.1 同じ基本契約の三つの確認方法

[rules/contracts.json](../../rules/contracts.json)から比較用Rust仕様とLean仕様を生成し、[src/contracts.rs](../../src/contracts.rs)の実処理をKaniで比較します。Lean定理は[Contracts.lean](../../proofs/Mahjong/Contracts.lean)、公理の列挙は[Audit.lean](../../proofs/Mahjong/Audit.lean)、期待名の正本は[tools/versions.json](../../tools/versions.json)です。

| 契約 | Kaniの実在ハーネス名 | 対応する通常テストの例 |
|---|---|---|
| C-01 次席 | `next_seat_refines_declaration` | `next_seat_wraps_and_rejects_invalid` |
| C-02 ドラ | `dora_refines_declaration` | `dora_wraps_inside_each_family` |
| C-03 1種類の牌の移動 | `move_one_refines_declaration_and_conserves` | `tile_move_preserves_count_and_rejects_empty_or_full` |
| C-04 点数・供託口座の移動 | `point_transfer_refines_declaration_and_is_atomic` | `points_transfer_preserves_total_including_escrow`、`rejected_transfer_leaves_every_account_unchanged` |
| C-05 リーチ事前条件 | `riichi_refines_declaration` | `riichi_requires_every_declared_precondition` |
| C-06 リーチ後の打牌 | `discard_refines_declaration` | `riichi_discard_is_drawn_tile_and_ids_are_valid` |

Leanの18定理は、上記の宣言との対応、席・牌の範囲、保存則、口座の不変性、拒否時の不変性などを扱います。Kani6件とLean18件は別々の成果であり、件数を足して24個のゲーム規則を証明したとは数えません。聴牌・門前などの引数が実際の対局を正しく表すことは、呼出し側の責務です。

### 3.2 生成・演算・誤った成功の拒否

| 作業書の対象／実在する入口 | 実在する検査名・検査ID | 確認範囲と限界 |
|---|---|---|
| 宣言同期：[generate_rules.py](../../scripts/generate_rules.py) | `--check`、通常テスト `checked_in_declarations_match_generated_outputs` | 宣言と生成物のずれを拒否する。宣言自体の麻雀上の正しさは別途レビュー |
| 生成器の型：[test_generate_rules.py](../../scripts/test_generate_rules.py) | `test_rejects_boolean_arithmetic`、`test_rejects_non_boolean_contract_result`、`test_rejects_mixed_integer_types`、`test_rejects_duplicate_json_keys`、`test_rejects_unknown_root_fields_and_empty_contracts` など19件 | 型・未知項目・重複・引数形・識別子を拒否 |
| 生成器の整数域：同上 | `test_rejects_unbounded_addition`、`test_rejects_unsigned_subtraction_without_guard`、`test_accepts_guarded_unsigned_subtraction`、`test_rejects_zero_or_unbounded_divisors`、`test_rejects_negative_division_or_remainder` | Rust固定幅整数とLeanの演算が一致するための前提を検査 |
| [check_generator_semantics.py](../../scripts/check_generator_semantics.py) | 実Rust・実Leanで7関係・19例を照合 | 生成した式を両言語で実行する有限例。生成器一般の形式証明ではない |
| `strict_lean_negative`：[test_platform_checks.py](../../scripts/tests/test_platform_checks.py) | `test_lean_rejects_sorry_unknown_axiom_and_missing_theorem`、`test_current_lean_and_kani_output_is_accepted` | `sorryAx`・未知公理・定理欠落を拒否。期待18定理と許可する3公理の名前を照合 |
| Kani集計：同上 | `test_kani_rejects_failed_or_missing_harness_and_empty_output` | 期待6ハーネスと個別結果を照合。空出力を成功にしない |
| 契約の否定検査：[check_contract_negatives.py](../../scripts/check_contract_negatives.py) | `N-01 unknown rule`、`N-02 unsupported rule value`、`N-03 generated artifact drift`、`N-04 mutated next seat`、`N-05 false Lean theorem` | 一時コピーの誤りを拒否理由まで確認。時間切れや実行ファイル不在を期待拒否に数えない |

開発時の[生成器の失敗記録](logs/platform-generator-before.txt)、[修正後記録](logs/platform-generator-after.txt)、[両言語の演算検査](logs/platform-generator-semantics.txt)、[Kani](logs/platform-kani.txt)、[Lean](logs/platform-lean.txt)、[公理出力](logs/platform-lean-axioms.txt)を保持しています。最終ソースの受入には、これらの過去記録に加え、第9節の固定実行が必要です。

## 4. API・設定・標準入出力

仕様は[API版1](../api/README.md)と[TOML設定](../api/configuration.md)です。

| 作業書の識別名／実在するファイル | 実在するテスト名 | 確認範囲 |
|---|---|---|
| `api_boundary`：[api_boundary.rs](../../crates/suzume-api/tests/api_boundary.rs) | `new_game_hides_other_hands_and_seed` | 閲覧席以外の手牌と再現用seedを外部画面へ出さない |
| 同上 | `stale_and_illegal_requests_leave_every_observable_field_unchanged`、`old_session_action_cannot_affect_a_restarted_game` | 古いrevision・古いsession・不正操作を拒否し、観測される状態を維持 |
| 同上 | `malformed_unknown_and_unsupported_requests_are_explicit_errors` | 不正JSON・未知項目・未知操作・未対応版などを明示エラーにする |
| `config_roundtrip`：[config_environment.rs](../../crates/suzume-api/tests/config_environment.rs) | `configuration_toml_roundtrip_including_public_viewer` | 共有観戦の設定も含むTOML往復 |
| 同上 | `configuration_rejects_unknown_duplicate_conflicting_and_out_of_range_values`、`seeds_preserve_the_entire_unsigned_64_bit_domain` | 不明・重複・設定競合・範囲外の拒否、全u64域を表現できる十進文字列の代表境界値 |
| `oversize`／標準入出力：[transport.rs](../../crates/suzume-api/tests/transport.rs) | `oversized_invalid_utf8_and_malformed_lines_do_not_desynchronize_transport` | 大きすぎる行・不正UTF-8・不正JSONの後でも、次の要求と応答の対応を維持 |
| 同上 | `too_large_or_invalid_configuration_preserves_the_active_game`、`output_failure_is_propagated` | 不正設定で既存対局を壊さない。出力I/O失敗を呼出し元へ返す |

構造の検証だけで、任意の牌姿に対する合法性や権限を証明するものではありません。Rustの信頼するローカル利用者はcoreの全情報へアクセスできます。情報制限の対象は、通常のAPI観測・SDK応答・共有ログです。研究用の手番別観測とは区別します。

## 5. AIの算法・予算・固定評価

### 5.1 事前の識別名と実テスト名

以下は [tests/ai_contracts.rs](../../tests/ai_contracts.rs) にあります。

| 作業書の識別名 | 実在するテスト名 | 確認範囲 |
|---|---|---|
| `ai_invalid_inputs` | `ai_rejects_invalid_observations_and_action_indices`、`ai_config_rejects_conflicting_algorithm_parameters` | 偽の観測・不正添字・設定の不整合を拒否 |
| `ai_random_determinism`／`ai_determinism` | `ai_is_deterministic_for_same_inputs_and_seed` | Random・Greedy・Ukeireで同じ入力と設定の判断が一致 |
| `ai_budget` | `ai_never_exceeds_declared_evaluation_budget`、`ai_depth_two_reports_measured_lookahead` | 実際の向聴計算呼出し数と上限、実施した先読みの説明 |
| `dead_wait_regression` | `ai_ukeire_avoids_publicly_exhausted_wait` | 公開情報で残り0枚の待ちを避ける具体局面 |
| 公開枚数 | `ai_public_tile_counts_do_not_double_count_called_discard`、`ai_does_not_treat_a_fifth_owned_tile_as_tenpai` | 鳴かれた捨て牌の二重計上、5枚目を必要とする待ちの除外 |
| 合法手選択 | `ai_all_algorithms_stay_legal_on_real_transitions` | 実際の状態遷移で渡された合法手から選ぶ |

従来Greedyの局面は [src/ai/tests.rs](../../src/ai/tests.rs) の `ai_returns_only_legal_actions`、`win_has_priority_over_call_and_pass`、`chi_with_a_terminal_is_not_treated_as_open_tanyao` で維持します。AI入口の構造検査は、役・振聴・手番を含むゲームの合法手判定の代わりではありません。

予算単位は計算済み結果の再利用を除く向聴計算呼出し回数です。OS時刻による探索打切りではありません。[算法と予算](../ai/algorithms.md)に記した限定的な先読みを検査しており、MCTS全体や学習済みモデルを実装・検証したとは扱いません。`4 - visible_counts` は未公開の枚数で、実際の山の残枚数ではありません。

### 5.2 評価条件と再現

[ai_evaluation_cli.rs](../../tests/ai_evaluation_cli.rs) の実在名は `evaluation_rejects_holdout_seeds_in_development_mode`、`evaluation_rejects_overriding_the_frozen_experiment`、`evaluation_reports_frozen_denominators_and_resolved_configs` です。これらは評価入口が条件を守る検査であり、対戦成績そのものとは分けます。

固定実験は [ai-protocol.json](../../evaluations/ai-protocol.json)、全件は [ai-results.csv](../../evaluations/ai-results.csv)、集計は [ai-summary.json](../../evaluations/ai-summary.json)にあります。候補Ukeire対Greedy3人と、対照Random対同じGreedy3人を、それぞれseed1000〜1007×4席の32東風戦、合計64東風戦で測定します。[初回実行記録](../ai/evidence/platform-ai-holdout.txt)と[結果・限界](../ai/results.md)を保持しています。

補助検査 `platform-ai-reproduction` は固定後のソースで64件を別ファイルへ再実行し、時間以外の全列一致と、初回64行からの集計再計算を確認しています。初回結果や時間を上書きせず、調整用の再試行にも使いません。異なる牌山seedは8個なので、32対応ペアを32個の独立した牌山実験とは数えません。Greedy全般・人間・別ルールへの優越や、プリセットの勝率順位を保証しません。

## 6. 探索・強化学習向け環境

仕様は[研究環境](../api/environment.md)、追加算法の入口は[AI拡張](../ai/extension.md)です。

| 作業書の識別名／実在するファイル | 実在するテスト名 | 確認範囲と限界 |
|---|---|---|
| `environment_action_roundtrip`：[environment_match.rs](../../crates/suzume-api/tests/environment_match.rs) | `every_legal_action_roundtrips_to_a_legal_canonical_action_through_a_complete_match` | 完全な対局中の合法手をIDへ変換し、同じ意味の最初の合法操作へ戻す。全牌姿の全行動を網羅する証明ではない |
| `environment_contracts`：[config_environment.rs](../../crates/suzume-api/tests/config_environment.rs) | `environment_reset_branch_masks_rewards_and_truncation_are_explicit` | 189要素maskと逆変換、同seedでのreset、独立分岐、4席の点数差、供託差、操作上限とその後の不変性 |
| 同上 | `invalid_and_reserved_environment_actions_are_atomic` | 範囲外189、予約・局面で非合法なID、操作上限0を拒否 |
| 終了：[logging.rs](../../crates/suzume-api/tests/logging.rs) | `a_complete_match_is_distinct_from_an_external_cutoff` | 自然終局と外部打切りを別の状態として返す |
| 環境とログの一致：同上 | `dataset_uses_the_same_observations_actions_and_rewards_as_environment`、`truncated_logs_keep_the_last_observation_for_value_bootstrapping` | 同一操作列の観測・行動・報酬・終了情報と、打切り時の `final_observation` を照合 |

1 stepは1席の1操作です。見送りと次局への進行も数えます。報酬は席別の点数差、別項目の供託差を足した総和は0です。順位報酬・割引率・学習性能は検査対象に含めません。`fork` は信頼する研究コード向けの全状態複製であり、AIへ非公開情報を見せない機構や低費用の探索木を自動で提供するものではありません。

## 7. JSONLログ・分析用遷移・CLI

### 7.1 writer／parser／replay／transition export

仕様は[ログ形式](../logging/README.md)、以下の実テストは特記を除き [crates/suzume-api/tests/logging.rs](../../crates/suzume-api/tests/logging.rs)です。

| 作業書の識別名 | 実在するテスト名 | 確認範囲 |
|---|---|---|
| `log_writer`：設定・順序 | `full_log_records_resolved_configuration_and_roundtrips_schema_types`、`writer_rejects_skips_wrong_actor_and_false_termination` | 完全設定・実効値、操作連番と実履歴、終了理由 |
| `log_writer`：短い書込み・I/O | `io_and_callback_failures_are_not_successful_logs` | 短い書込み、書込・flush・読込・変換先の失敗を成功にしない |
| `log_writer`：情報制限 | `public_log_contains_only_public_views_and_semantic_actions`、`decision_metadata_is_consistent_and_never_appears_in_public_output` | 共有用にseed・私的添字・非公開回答・AIの私的判断を出さない。完全ログの判断と実効設定を照合 |
| `log_parser`：型・版・連番 | `malformed_versions_metadata_fields_and_sequences_are_rejected`、`duplicate_keys_at_top_level_and_inside_nested_types_are_rejected` | 未知項目・版・メタデータ・連番・入れ子の重複キーを拒否 |
| `log_parser`：上限・末尾 | `line_count_total_and_buffer_limits_are_enforced`、`missing_reordered_extra_or_truncated_records_never_count_as_complete`、`complete_final_line_without_newline_and_crlf_are_accepted` | 行長・件数・総量・保持上限、欠落・並替え・余分な行、正当な改行の違い |
| `log_replay` | `replay_restores_full_game_and_exact_hand_index_history`、`malformed_versions_metadata_fields_and_sequences_are_rejected` | 完全Gameと正確な添字履歴の再生一致、操作や状態ハッシュなどの改変拒否 |
| `transition_export` | `dataset_uses_the_same_observations_actions_and_rewards_as_environment`、`interrupted_empty_and_nonempty_logs_are_explicitly_truncated_datasets` | 環境と同じ観測・行動・報酬、0操作を含む外部中断の扱い |
| `transition_export`：公開の確定 | `streaming_output_is_provisional_until_the_footer_and_eof_are_validated`、`truncated_logs_keep_the_last_observation_for_value_bootstrapping` | 末尾検証前の出力は暫定、打切り後の価値推定に必要な最終観測を保持 |

APIコマンドの実ファイル経路は [file_cli.rs](../../crates/suzume-api/tests/file_cli.rs) の `replay_and_dataset_use_the_shipped_recording` と `malformed_late_input_does_not_publish_a_partial_dataset` で検査します。さらに [suzume-api.rs](../../crates/suzume-api/src/bin/suzume-api.rs) の `refuses_to_remove_a_temporary_file_it_did_not_create` が、作成に失敗した一時ファイルを削除しない所有権の回帰検査です。

JSON SchemaやSDKの構造検査だけでは、操作の合法性、状態ハッシュ、末尾と対局の一致を確定できません。完全ログの正当性には実再生が必要です。共有ログは公開情報の記録であり、完全再生と分析遷移の材料にはしません。対局更新とOSへの書込みは単一の原子的処理ではなく、I/O失敗では呼出し側を停止させ、不完全ログを完成と報告しません。

### 7.2 CLI設定と既存TUI

以下は [crates/suzume-cli/tests/cli.rs](../../crates/suzume-cli/tests/cli.rs) にあります。

| 作業書 `cli_config/log/replay/environment smoke` の対象 | 実在するテスト名 |
|---|---|
| 実際の設定使用 | `cli_config_applies_seed_to_snapshot`、`cli_config_applies_the_operation_limit`、`cli_configured_policies_and_replay_match_actual_actions`、`cli_default_seed_is_reproducible` |
| 共通検証と境界 | `cli_config_and_seed_are_rejected_as_conflicting`、`cli_config_rejects_incompatible_controllers`、`cli_config_reuses_shared_validation`、`cli_config_rejects_oversize_utf8_and_fixed_viewer_mismatch` |
| 自然終局・共有用・中断 | `cli_complete_simulation_log_replays`、`cli_public_log_omits_private_data`、`cli_early_stops_write_valid_interrupted_logs`、`cli_demo_stops_after_one_round_and_records_interruption` |
| 出力先の契約 | `cli_log_rejects_multiple_matches_and_existing_files` |
| 既存の端末操作 | `help_is_available_without_starting_a_game`、`invalid_cli_arguments_fail_with_a_reason`、`plain_snapshot_has_no_terminal_control_sequences`、`eof_closes_the_game_without_waiting`、`invalid_human_input_recovers_and_quit_exits` |

設定の検査は、ログのヘッダを見るだけでなく、TOMLから別に再計算した操作列・Gameとの一致を確認します。TUIの手牌選択・表示幅・曖昧な鳴きの選択は [ui.rs](../../crates/suzume-cli/src/ui.rs) の `numbered_discards_and_riichi_are_distinct`、`ambiguous_calls_require_an_explicit_menu_choice`、`invalid_input_does_not_become_a_discard`、`japanese_width_and_color_do_not_break_the_frame` に対応します。研究環境の実行経路はTUIに再実装せず、第6節とAPI/SDKの検査で確認します。

## 8. 生成型・SDK・WASM・実ブラウザ

| 作業書の識別名／実在するファイル | 実在するテスト名・実行項目 | 確認範囲と限界 |
|---|---|---|
| `schema_sync`：[export-schema.rs](../../crates/suzume-api/src/bin/export-schema.rs)、[generate_protocol_ts.py](../../scripts/generate_protocol_ts.py) | `export-schema --check`、`generate_protocol_ts.py --check` | Rust由来SchemaとTS型・実行時検査データの同期 |
| `ts_negative`：[types.ts](../../packages/suzume/tests/types.ts) | `wrongVersion`、`unknownOperation`、`lossySeed`、`badChi`、`mixedAction` の `@ts-expect-error` | 版、操作の識別、64bit seed、チーの長さ、通常操作と環境IDの形。値が現在の局面で合法かは実行時検査 |
| `sdk_native_wasm`：[integration.test.mjs](../../packages/suzume/tests/integration.test.mjs) | `native Rust and real Node WASM produce identical versioned responses` | 同じ正常・不正要求、64通常操作、解析・設定、8環境操作をnative実プロセスと実WASMで比較 |
| 同上 | `typed client checks stale actions, private views and terminal masks using real WASM` | SDK経由の古い操作拒否、私的情報の制限、189mask・報酬・打切り最終観測 |
| 同上 | `browser ESM binding instantiates packaged bytes without Node-specific browser code` | Web用結合を実WASMバイト列から起動する。実行環境はNodeで、ブラウザ表示の検査ではない |
| 不正応答：[validation.test.mjs](../../packages/suzume/tests/validation.test.mjs) | `strict parsing rejects duplicates, malformed text and excessive structure`、`runtime request validation enforces bounds, literal tags and exact seed text`、`untrusted responses and closed clients do not enter application state` | 型・版・構造上限・seed表現を検査し、不正応答をアプリ状態へ取り込まない |
| 同上 | `cyclic, accessor and sparse values are rejected before JSON serialization`、`prototype names cannot bypass schema property and definition lookup`、`integer wire values reject lossy decimal and exponent spellings like Rust` | 循環・アクセサ・疎配列、継承したプロパティ名、丸めを伴う数値表記の拒否 |
| ストリームSDK：[logs.test.mjs](../../packages/suzume/tests/logs.test.mjs) | `streaming parser reads native JSONL over arbitrary UTF-8 chunk boundaries`、`streaming parser refuses missing footer, duplicate keys and configured limits`、`native replay exports transitions consumed by the typed streaming parser` | 実nativeログを分割読込みし、型付き遷移を消費する。SDK単体で麻雀操作を再生検証するものではない |

実ブラウザは [check_browser.py](../../scripts/check_browser.py) から [examples/browser/index.html](../../examples/browser/index.html)を開く別の検査です。実在するassertion名は `initial-deal`、`rendered-fourteen-tiles`、`private-state-not-exposed`、`discard-from-visible-control`、`one-cpu-operation`、`advance-to-human`、`invalid-seed-is-atomic`、`restart-with-full-u64-seed`、`different-seed-changes-deal`、`error-cleared-after-recovery`、`no-console-or-page-errors` です。対象コミット、実ブラウザの版、assertion結果、画面PNGを保存して受け入れます。検査コードの存在やNodeの成功だけで、この系列の実行済み・成功を主張しません。

## 9. 環境・全体検証・配布・文書

### 9.1 固定環境と全体検証

実行手順と対象OSは[再現と配布](../development/reproducibility.md)にあります。[bootstrap.py](../../scripts/bootstrap.py)は導入、[doctor.py](../../scripts/doctor.py)は存在と版の診断です。後者の成功だけで、すべてを空の環境へ新規導入したとは言えません。

[verify_platform.py](../../scripts/verify_platform.py)の `--profile full` に存在する検査IDは次のとおりです。

| 分類 | 実在する検査ID |
|---|---|
| 版・文書 | `tool-versions`、`documentation-links`、`documentation-checker-tests` |
| 宣言・生成・検証器 | `rule-sync`、`generator-tests`、`generator-rust-lean-semantics`、`verifier-self-tests` |
| Rustと対局 | `rust-format`、`rust-clippy`、`rust-tests`、`simulation-32` |
| API・TS・WASM | `api-schema-sync`、`typescript-protocol-sync`、`wasm-build`、`sdk-build`、`sdk-tests` |
| 形式検証 | `lean-build`、`lean-axioms`、`kani`、`negative-controls` |

全体受入では `profile = full`、`full_verification = true`、`success = true` と前後のソース不変を確認します。`native`・`frontend`・`formal` の成功を、残りの省略範囲を含む成功にしません。`simulation-32` はseed0〜31の32東風戦で、AI固定評価のseed1000〜1007による64東風戦とは目的・母数が異なります。

**作業書19-dの公開Rust文書ビルドは、この全体スクリプトに含まれていません。** `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --locked` の別実行記録が必要です。`cargo test` のdoctestだけで、公開文書全体の警告検査を代用しません。

[scripts/tests/test_platform_checks.py](../../scripts/tests/test_platform_checks.py)には13件の検査があります。第3節の形式出力検査に加え、`test_rust_rejects_zero_tests_ignored_and_filtered`、`test_simulation_rejects_missing_duplicate_and_failed_seeds`、`test_sdk_rejects_zero_skipped_and_cancelled_tests`、`test_process_timeout_is_failure_and_cannot_overwrite_evidence`、`test_missing_executable_is_not_success`、`test_python_tests_cannot_be_empty_or_skipped` が、実行の欠落や空の集計を拒否します。単なる終了値0だけでは成功と数えません。

### 9.2 配布物からの独立消費

作業書の `archive_contents`、`isolated_consumer`、`isolated_crate_consumer`、`isolated_sdk_consumer`、`isolated_source_consumer` は [package.py](../../scripts/package.py) の次の実処理に対応します。

| 実在する処理・検査ID | 確認範囲 |
|---|---|
| `check_archive` と `manifest.json`／`SHA256SUMS` 作成 | crate・npmの必要物、配布物と同梱ソースのハッシュ。ファイルの存在だけで実使用成功にしない |
| `crate-consumer-build`、`crate-consumer-play` | 書庫を別領域へ展開し、内部依存を明示したpatchと元のlockでoffline build、出来上がったCLIを起動 |
| `npm-consumer-install`、`npm-consumer-types`、`npm-consumer-runtime` | npm書庫を別領域へ導入し、公開型と `/node`・`/browser` 入口、実WASM操作を検査 |
| `source-consumer-build`、`source-consumer-documentation` | ソース書庫だけからworkspaceを型検査し、同梱した文書検査器で相対リンクと入口を照合 |
| `native-consumer-play` | 展開した対象OS用の実行ファイルを起動 |

配布の回帰は `test_bindings_evaluations_and_distribution_docs_have_distinct_scope` と `test_source_archive_is_independent_of_mtime_mode_and_argument_order` にも対応します。ソース書庫に必要資料が入ること、検証入力のSHA対象と公開記録の同梱対象を分けること、時刻・権限・引数順の違いに左右されない書庫を検査します。

crate作成で使用する `--no-verify` は、続く独立消費の省略許可ではありません。npm/crates.ioへの登録、GitHub Release、全Linux環境での動的ライブラリ互換、すべてのCPUでの同一バイナリは、この検査の主張に含めません。

### 9.3 文書と公開

[check_documentation.py](../../scripts/check_documentation.py) の通常実行はMarkdownの相対リンク・アンカー・画像と、現行文書にあるPython/Cargo/npm/Lakeの入口の存在を確認します。`--self-test`の10件には `test_missing_path_anchor_image_and_reference_fail`、`test_excluded_directories_and_root_relative_links`、`test_python_cargo_and_npm_entries_are_checked_without_execution`、`test_historical_commands_are_preserved_but_links_are_checked` などがあります。検証器側の `test_documentation_requires_nonempty_checks_and_no_errors` は、0件やエラーのある検査結果を拒否します。

この静的検査は、外部URLへの到達、全文の意味の正確さ、掲載した全コマンドの実行を保証しません。実例の実行記録と[独立レビュー](../reviews/platform-review.md)を別に照合します。初版の作業書・ログは当時の事実を保持し、旧コマンドを現行の結果に書き換えません。

CIの入口は [platform.yml](../../.github/workflows/platform.yml) の `fixed-native` と `fixed-full-linux`、従来の [ci.yml](../../.github/workflows/ci.yml) です。定義ファイルがあること、ローカルで同じコマンドが成功すること、指定コミットのCIが完了することは別の事実です。DOD-P12では最後のものまで確認します。

## 10. 実行証拠と完了判定

以下は、固定ソースに対する補助実行 `aux-01` と、CLI文書設定変更後のログ結合 `aux-02` の機械可読結果を照合した記録です。4系列はいずれも `success = true`、検査前後のソースSHA不変でした。実行環境の私的な絶対パスは転載しません。これらのcheck IDと実行IDは、完了監査で原出力・ハッシュに対応付ける識別子です。

| 成功を確認したcheck ID | 実際に確認した内容 | 主張しない範囲 |
|---|---|---|
| `platform-core-msrv` | Rust/Cargo 1.80.1で `suzume` の66単体・9AI・3評価CLI・4ゲーム・1doctest、合計83件成功。core全対象の型検査も成功 | API・CLI・SDK・WASMをRust 1.80.1で検証したとは言わない |
| `platform-bootstrap-evidence` | 保存済みの実導入出力と現在の固定版を照合。別の道具用領域へのRust/rustup/rustfmt/clippy、Kani/CBMCの導入。TypeScript固定lockと公式wasm-bindgen書庫のSHAも確認 | LeanとNode/npmは既存の指定版を再利用しているため、全道具を空のOSへ導入した成功ではない。過去の取得失敗も保持 |
| `platform-ai-reproduction` | 初回64件と再実行64件を比較し、時間以外の全列一致。初回集計を64行から再計算。候補・対照とも32成功・0失敗 | 再現実行を追加の独立した強度測定と数えない。初回の時間や成績を置換しない |
| `platform-log-cli-integration` | 完全／共有の2東風戦が自然終局。完全ログ1,235操作をnative再生し、同数の遷移・点数差・供託差を照合。SDKは37byte分割で読込。共有再生・末尾欠落・状態改変・既存出力上書きの4負例を拒否 | 2試合から任意の全ログの正しさや全環境のI/O耐性を保証しない |

次の5系列には、それぞれ独立した受入証拠を要求します。総合20系列、別の警告なしRust文書生成、配布14工程、実ブラウザ11項目の成功は[0.2.0の実結果](platform-results.md)に記録しています。文書の照合と最終HEADの確認も別に実施し、実装・単体成功・開発時ログだけで全体の完了を代替しません。

| 最終check ID | 必要な証拠 |
|---|---|
| `platform-full-verification` | fullの全項目・意味上の集計・対象ソース不変、および別途必要なrustdoc実行 |
| `platform-package-consumption` | 生成物のSHAと、crate・npm・source・実行形式の独立消費結果 |
| `platform-browser-acceptance` | 実ブラウザの版、全assertion、PNG、対象コミット |
| `platform-documentation-source-review` | 本表を含む文書の静的検査、実装との独立照合、指摘への対応 |
| `platform-final-git-ci` | 最終HEAD、リモートとの一致、作業ツリー、最終対象コミットのCI完了と固定監査の成立 |

固定した完了監査方針は109成果物と9検査系列を要求しています。本表はその文書・実装照合のための説明資料であり、必要成果物の存在検査、実行結果のハッシュ照合、監査の完了判定を代替しません。4補助系列の成功から残る5系列やDOD-P01〜P12全体の完了を導きません。
