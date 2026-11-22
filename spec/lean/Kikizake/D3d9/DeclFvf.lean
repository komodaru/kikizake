import Kikizake.D3d9.Fvf
/-! D3DXFVFFromDeclarator's refusals (spec/d3dx9.txt D3DX9.DECLFVF.INVALID), of `declFvf` in
Fvf.lean, the model's `decl_fvf` (mirror `d3dx9.declfvf`). -/
namespace Kikizake.D3d9.Fvf

/-- (posPart refuses with INVALIDCALL only) -/
private theorem posPart_error (e : List (Nat × Nat)) (c : Nat) (h : posPart e = .error c) : c = INVALIDCALL := by
  unfold posPart at h
  repeat' split at h
  all_goals first | (injection h with h; exact h.symm) | (simp at h)

/-- (texPart refuses with INVALIDCALL only) -/
private theorem texPart_error (e : List (Nat × Nat)) : ∀ fuel n i f c, texPart e fuel n i f = .error c → c = INVALIDCALL := by
  intro fuel
  induction fuel with
  | zero => intro n i f c h; simp [texPart] at h
  | succ k ih =>
    intro n i f c h
    unfold texPart at h
    split at h
    · cases h
    · split at h
      · cases h; rfl
      · exact ih _ _ _ _ h

/-- D3DX9.DECLFVF.INVALID: a declaration whose elements are not all in stream 0, each at the
offset packing the ones before it gives (an offset not packed, a stream other than 0), is
refused, whatever its elements are -/
theorem unpacked_refused (e : List (Nat × Nat)) (h : packed e 0 = false) : declFvf e = .error INVALIDCALL := by
  unfold declFvf
  cases hp : posPart e with
  | error c => simp [posPart_error e c hp, bind, Except.bind]
  | ok pi =>
    cases ht : texPart e 8 0 (flagPart e pi).2 (flagPart e pi).1 with
    | error c => simp [ht, texPart_error e _ _ _ _ c ht, bind, Except.bind]
    | ok r => simp [ht, h, bind, Except.bind]

/-- D3DX9.DECLFVF.INVALID: an element in a stream other than 0, or not at the packed offset,
makes the declaration unpacked from it on -/
theorem stream_or_offset_unpacked (x : Nat × Nat) (r : List (Nat × Nat)) (off : Nat)
    (he : x.1 % 65536 ≠ 0xff) (h : x.1 % 65536 ≠ 0 ∨ x.1 >>> 16 ≠ off) : packed (x :: r) off = false := by
  rcases h with h | h <;> simp [packed, he, h]

/-- D3DX9.DECLFVF.INVALID: past the position, blend and flag elements, an element that is not
the next texture coordinate set (usage 5, index the sets before it, FLOAT1..FLOAT4) and not the
end, before the eighth, is refused: an element out of that order, a set out of turn -/
theorem out_of_turn_refused (e : List (Nat × Nat)) (fuel n i f : Nat) (he : isEnd e i = false) (hn : n < 8)
    (hk : ∀ k, k < 4 → is e i k 5 (some n) = false) : texPart e (fuel + 1) n i f = .error INVALIDCALL := by
  have : (List.range 4).find? (fun k => is e i k 5 (some n)) = none := by
    rw [List.find?_eq_none]
    intro k hk'
    simp [hk k (List.mem_range.mp hk')]
  simp [texPart, he, Nat.not_le.mpr hn, this]

/-- D3DX9.DECLFVF.INVALID: a texture coordinate set's refusal is the declaration's -/
theorem tex_refusal_refused (e : List (Nat × Nat)) (pi : Nat × Nat) (c : Nat) (hp : posPart e = .ok pi)
    (ht : texPart e 8 0 (flagPart e pi).2 (flagPart e pi).1 = .error c) : declFvf e = .error INVALIDCALL := by
  unfold declFvf
  simp [hp, ht, texPart_error e _ _ _ _ c ht, bind, Except.bind]

/-- D3DX9.DECLFVF.INVALID: the measured refusals: a second stream, an offset left open, the
normal before the position, a set out of turn, FLOAT1 indices behind a weight -/
theorem refused_examples :
    declFvf [(1, 2)] = .error INVALIDCALL ∧
    declFvf [elem 0 2 0 0, elem 16 2 3 0] = .error INVALIDCALL ∧
    declFvf [elem 0 2 3 0, elem 12 2 0 0] = .error INVALIDCALL ∧
    declFvf [elem 0 2 0 0, elem 12 1 5 1] = .error INVALIDCALL ∧
    declFvf [elem 0 2 0 0, elem 12 0 1 0, elem 16 0 2 0] = .error INVALIDCALL :=
  ⟨rfl, rfl, rfl, rfl, rfl⟩

end Kikizake.D3d9.Fvf
