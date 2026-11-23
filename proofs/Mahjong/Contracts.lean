import Std
import Mahjong.GeneratedContracts

/-!
麻雀の操作を構成する小さな契約の数学的モデル。
Rustコード全体の意味論ではなく、共有宣言に対応する値の関係を証明する。
Rustの有限整数からこのモデルへの対応は src/contracts.rs のKani検査が担う。
-/

set_option autoImplicit false

namespace Mahjong.Model

def nextSeat (seat : Nat) : Option Nat :=
  if seat < Rules.PLAYER_COUNT then some ((seat + 1) % Rules.PLAYER_COUNT) else none

theorem next_seat_refines_declaration (seat result : Nat) :
    nextSeat seat = some result ↔ Spec.next_seat_relation seat result = true := by
  by_cases h : seat < 4
  · simp [nextSeat, Rules.PLAYER_COUNT, Spec.next_seat_relation, h, eq_comm]
  · simp [nextSeat, Rules.PLAYER_COUNT, Spec.next_seat_relation, h]

theorem next_seat_remains_valid (seat result : Nat) (h : nextSeat seat = some result) :
    result < Rules.PLAYER_COUNT := by
  have relation := (next_seat_refines_declaration seat result).mp h
  simp [Spec.next_seat_relation] at relation
  simp [Rules.PLAYER_COUNT]
  omega

theorem four_turns_return_to_same_seat (seat : Nat) (h : seat < 4) :
    (((((seat + 1) % 4) + 1) % 4 + 1) % 4 + 1) % 4 = seat := by
  omega

def dora (indicator : Nat) : Option Nat :=
  if indicator < 27 then some (indicator / 9 * 9 + (indicator + 1) % 9)
  else if indicator < 31 then some (27 + (indicator - 26) % 4)
  else if indicator < Rules.TILE_KINDS then some (31 + (indicator - 30) % 3)
  else none

theorem dora_refines_declaration (indicator result : Nat) :
    dora indicator = some result ↔ Spec.dora_relation indicator result = true := by
  by_cases h27 : indicator < 27
  · have n27 : ¬indicator ≥ 27 := by omega
    have n31 : ¬indicator ≥ 31 := by omega
    simp [dora, Spec.dora_relation, h27, n27, n31, eq_comm]
  · by_cases h31 : indicator < 31
    · have y27 : indicator ≥ 27 := by omega
      have n31 : ¬indicator ≥ 31 := by omega
      simp [dora, Spec.dora_relation, h27, h31, y27, n31, eq_comm]
    · by_cases h34 : indicator < 34
      · have y27 : indicator ≥ 27 := by omega
        have y31 : indicator ≥ 31 := by omega
        simp [dora, Rules.TILE_KINDS, Spec.dora_relation, h27, h31, h34, y27, y31, eq_comm]
      · simp [dora, Rules.TILE_KINDS, Spec.dora_relation, h27, h31, h34]

theorem dora_remains_valid (indicator result : Nat) (h : dora indicator = some result) :
    result < Rules.TILE_KINDS := by
  have relation := (dora_refines_declaration indicator result).mp h
  simp [Spec.dora_relation] at relation
  simp [Rules.TILE_KINDS]
  omega

theorem dora_preserves_family (indicator result : Nat) (h : dora indicator = some result) :
    (indicator < 27 → result / 9 = indicator / 9) ∧
    (27 ≤ indicator ∧ indicator < 31 → 27 ≤ result ∧ result < 31) ∧
    (31 ≤ indicator ∧ indicator < 34 → 31 ≤ result ∧ result < 34) := by
  have relation := (dora_refines_declaration indicator result).mp h
  simp [Spec.dora_relation] at relation
  omega

def moveOne (source destination : Nat) : Option (Nat × Nat) :=
  if 0 < source ∧ source ≤ Rules.COPIES_PER_TILE ∧ destination < Rules.COPIES_PER_TILE
  then some (source - 1, destination + 1) else none

theorem move_one_refines_declaration (source destination newSource newDestination : Nat) :
    moveOne source destination = some (newSource, newDestination) ↔
      Spec.move_one_relation source destination newSource newDestination = true := by
  by_cases h : 0 < source ∧ source ≤ 4 ∧ destination < 4
  · simp [moveOne, Rules.COPIES_PER_TILE, Spec.move_one_relation, h]
    omega
  · simp [moveOne, Rules.COPIES_PER_TILE, Spec.move_one_relation, h]
    omega

theorem move_one_conserves_and_bounds (source destination newSource newDestination : Nat)
    (h : moveOne source destination = some (newSource, newDestination)) :
    source + destination = newSource + newDestination ∧
      newSource ≤ 4 ∧ newDestination ≤ 4 := by
  have relation := (move_one_refines_declaration source destination newSource newDestination).mp h
  simp [Spec.move_one_relation] at relation
  omega

abbrev Ledger := Nat → Int

def ledgerTotal (accounts : Ledger) : Int :=
  accounts 0 + accounts 1 + accounts 2 + accounts 3 + accounts 4

def updatedLedger (accounts : Ledger) («from» to : Nat) (amount : Int) : Ledger :=
  fun index => if index = «from» then accounts index - amount
    else if index = to then accounts index + amount else accounts index

def transferPoints (accounts : Ledger) («from» to : Nat) (amount : Int) : Option Ledger :=
  if Spec.point_transfer_legal «from» to (accounts «from») (accounts to) amount = true
  then some (updatedLedger accounts «from» to amount) else none

theorem transfer_success_is_declared (accounts after : Ledger) («from» to : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = some after) :
    Spec.point_transfer_legal «from» to (accounts «from») (accounts to) amount = true ∧
      after = updatedLedger accounts «from» to amount := by
  unfold transferPoints at h
  split at h <;> simp_all

theorem point_transfer_refines_declaration (accounts after : Ledger) («from» to : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = some after) :
    Spec.point_transfer_relation (accounts «from») (accounts to) amount (after «from») (after to) = true := by
  obtain ⟨legal, rfl⟩ := transfer_success_is_declared accounts after «from» to amount h
  simp [Spec.point_transfer_legal] at legal
  have distinct : to ≠ «from» := by omega
  simp [Spec.point_transfer_relation, updatedLedger, distinct]
  omega

theorem point_transfer_conserves_total (accounts after : Ledger) («from» to : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = some after) :
    ledgerTotal accounts = ledgerTotal after := by
  obtain ⟨legal, rfl⟩ := transfer_success_is_declared accounts after «from» to amount h
  simp [Spec.point_transfer_legal] at legal
  have fromCases : «from» = 0 ∨ «from» = 1 ∨ «from» = 2 ∨ «from» = 3 ∨ «from» = 4 := by omega
  have toCases : to = 0 ∨ to = 1 ∨ to = 2 ∨ to = 3 ∨ to = 4 := by omega
  rcases fromCases with rfl | rfl | rfl | rfl | rfl <;>
    rcases toCases with rfl | rfl | rfl | rfl | rfl <;>
    simp_all [ledgerTotal, updatedLedger] <;> omega

theorem point_transfer_preserves_other_accounts (accounts after : Ledger)
    («from» to index : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = some after)
    (notFrom : index ≠ «from») (notTo : index ≠ to) : after index = accounts index := by
  obtain ⟨_, rfl⟩ := transfer_success_is_declared accounts after «from» to amount h
  simp [updatedLedger, notFrom, notTo]

theorem point_transfer_remains_in_i32 (accounts after : Ledger) («from» to : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = some after) :
    (-2147483648 ≤ after «from» ∧ after «from» ≤ 2147483647) ∧
      (-2147483648 ≤ after to ∧ after to ≤ 2147483647) := by
  obtain ⟨legal, rfl⟩ := transfer_success_is_declared accounts after «from» to amount h
  simp [Spec.point_transfer_legal] at legal
  have distinct : to ≠ «from» := by omega
  simp [updatedLedger, distinct]
  omega

/-- 拒否された遷移を適用しても旧状態をそのまま返す。 -/
def applyTransfer (accounts : Ledger) («from» to : Nat) (amount : Int) : Ledger :=
  (transferPoints accounts «from» to amount).getD accounts

theorem rejected_transfer_is_atomic (accounts : Ledger) («from» to : Nat) (amount : Int)
    (h : transferPoints accounts «from» to amount = none) :
    applyTransfer accounts «from» to amount = accounts := by
  simp [applyTransfer, h]

def CanDeclareRiichi (closed tenpai : Bool) (points : Int) (wall : Nat) (already : Bool) : Prop :=
  closed = true ∧ tenpai = true ∧ points ≥ Rules.RIICHI_STICK ∧
    wall ≥ Rules.RIICHI_MIN_WALL ∧ already = false

theorem riichi_refines_declaration (closed tenpai : Bool) (points : Int) (wall : Nat) (already : Bool) :
    Spec.riichi_legal closed tenpai points wall already = true ↔
      CanDeclareRiichi closed tenpai points wall already := by
  simp [Spec.riichi_legal, CanDeclareRiichi, Rules.RIICHI_STICK, Rules.RIICHI_MIN_WALL, and_assoc]

theorem riichi_deposit_is_affordable (closed tenpai : Bool) (points : Int) (wall : Nat) (already : Bool)
    (h : CanDeclareRiichi closed tenpai points wall already) :
    points - Rules.RIICHI_STICK ≥ 0 := by
  simp [CanDeclareRiichi, Rules.RIICHI_STICK] at h
  simp [Rules.RIICHI_STICK]
  omega

theorem discard_refines_declaration (riichi : Bool) (drawn discard : Nat) :
    Spec.discard_legal riichi drawn discard = true ↔
      drawn < 34 ∧ discard < 34 ∧ (riichi = true → drawn = discard) := by
  cases riichi <;> simp [Spec.discard_legal, and_assoc]

theorem riichi_forces_drawn_tile (drawn discard : Nat)
    (h : Spec.discard_legal true drawn discard = true) : drawn = discard := by
  simp [Spec.discard_legal] at h
  exact h.2

end Mahjong.Model
