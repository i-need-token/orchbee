namespace ChromeTabs

universe u

variable {α : Type u}

/--
The normal form of a contiguous block moving right. The four arguments are
the unchanged prefix, the moving block, the collections crossed by the move,
and the unchanged suffix.
-/
def moveRight (before moving crossed after : List α) : List α :=
  before ++ crossed ++ moving ++ after

def moveRightInput (before moving crossed after : List α) : List α :=
  before ++ moving ++ crossed ++ after

/-- The symmetric normal form of a contiguous block moving left. -/
def moveLeft (before crossed moving after : List α) : List α :=
  before ++ moving ++ crossed ++ after

def moveLeftInput (before crossed moving after : List α) : List α :=
  before ++ crossed ++ moving ++ after

theorem moveRight_preserves_elements
    (before moving crossed after : List α) :
    (moveRight before moving crossed after).Perm
      (moveRightInput before moving crossed after) := by
  unfold moveRight moveRightInput
  simpa only [List.append_assoc] using
    List.Perm.append_left before
      (List.Perm.append_right after
        (List.perm_append_comm : (crossed ++ moving).Perm (moving ++ crossed)))

theorem moveLeft_preserves_elements
    (before crossed moving after : List α) :
    (moveLeft before crossed moving after).Perm
      (moveLeftInput before crossed moving after) := by
  unfold moveLeft moveLeftInput
  simpa only [List.append_assoc] using
    List.Perm.append_left before
      (List.Perm.append_right after
        (List.perm_append_comm : (moving ++ crossed).Perm (crossed ++ moving)))

theorem moveRight_preserves_length
    (before moving crossed after : List α) :
    (moveRight before moving crossed after).length =
      (moveRightInput before moving crossed after).length :=
  (moveRight_preserves_elements before moving crossed after).length_eq

theorem moveLeft_preserves_length
    (before crossed moving after : List α) :
    (moveLeft before crossed moving after).length =
      (moveLeftInput before crossed moving after).length :=
  (moveLeft_preserves_elements before crossed moving after).length_eq

/--
The moving block itself is never transformed. It appears byte-for-byte in the
output normal form; only its position relative to `crossed` changes.
-/
theorem moveRight_keeps_block_order
    (before moving crossed after : List α) :
    moveRight before moving crossed after = before ++ crossed ++ moving ++ after := rfl

theorem moveLeft_keeps_block_order
    (before crossed moving after : List α) :
    moveLeft before crossed moving after = before ++ moving ++ crossed ++ after := rfl

end ChromeTabs
