import Mahjong.Contracts

-- 外部公理や未証明の穴が混入していないことを、実検証記録で点検する入口。
#print axioms Mahjong.Model.next_seat_refines_declaration
#print axioms Mahjong.Model.next_seat_remains_valid
#print axioms Mahjong.Model.four_turns_return_to_same_seat
#print axioms Mahjong.Model.dora_refines_declaration
#print axioms Mahjong.Model.dora_remains_valid
#print axioms Mahjong.Model.dora_preserves_family
#print axioms Mahjong.Model.move_one_refines_declaration
#print axioms Mahjong.Model.move_one_conserves_and_bounds
#print axioms Mahjong.Model.transfer_success_is_declared
#print axioms Mahjong.Model.point_transfer_refines_declaration
#print axioms Mahjong.Model.point_transfer_conserves_total
#print axioms Mahjong.Model.point_transfer_preserves_other_accounts
#print axioms Mahjong.Model.point_transfer_remains_in_i32
#print axioms Mahjong.Model.rejected_transfer_is_atomic
#print axioms Mahjong.Model.riichi_refines_declaration
#print axioms Mahjong.Model.riichi_deposit_is_affordable
#print axioms Mahjong.Model.discard_refines_declaration
#print axioms Mahjong.Model.riichi_forces_drawn_tile
