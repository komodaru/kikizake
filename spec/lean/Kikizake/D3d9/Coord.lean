import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's sets of texture coordinates (D3D9.STAGE.TCI) as src/model/d3d9draw.rs picks
them: `slots` `tci_slots`, `row` `coord_row`, and a stage's texture matrix (D3D9.STAGE.TEXXFORM)
`texTransform` `tex_transform` (mirrors `d3d9.slots`, `d3d9.row`, `d3d9.ttf`). -/
namespace Kikizake.D3d9.Coord

/-- what tells sets apart: the TEXCOORDINDEX, the TEXTURETRANSFORMFLAGS and the stage whose
texture matrix it is (both 0 where the flags are 0) -/
abbrev Key := Nat × Nat × Nat

/-- the slots with the key added where new -/
def addNew (acc : List Key) (e : Key × Nat) : List Key := if e.1 ∈ acc then acc else acc ++ [e.1]

/-- the stages after the first up to the first COLOROP DISABLE (1), with their COLOROPs -/
def enabled (keys : List Key) (cops : List Nat) : List (Key × Nat) :=
  ((keys.drop 1).zip (cops.drop 1)).takeWhile (fun e => e.2 != 1)

/-- the sets the stages read: the first stage's, then each enabled stage's where new; none past
three -/
def slots (keys : List Key) (cops : List Nat) : Option (List Key) :=
  match keys with
  | [] => none
  | k0 :: _ =>
    let s := (enabled keys cops).foldl addNew [k0]
    if s.length ≤ 3 then some s else none

private theorem add_props (l : List (Key × Nat)) : ∀ acc : List Key,
    (acc.Nodup → (l.foldl addNew acc).Nodup) ∧ (∀ x ∈ acc, x ∈ l.foldl addNew acc) ∧
      (∀ e ∈ l, e.1 ∈ l.foldl addNew acc) ∧ (∀ x ∈ l.foldl addNew acc, x ∈ acc ∨ ∃ e ∈ l, e.1 = x) := by
  induction l with
  | nil => intro acc; simp
  | cons e l ih =>
    intro acc
    have ⟨h1, h2, h3, h4⟩ := ih (addNew acc e)
    simp only [List.foldl_cons]
    have hin : e.1 ∈ addNew acc e := by unfold addNew; split <;> simp_all
    have hsub : ∀ x ∈ acc, x ∈ addNew acc e := by intro x hx; unfold addNew; split <;> simp_all
    refine ⟨fun hn => h1 ?_, fun x hx => h2 x (hsub x hx), ?_, ?_⟩
    · unfold addNew; split
      · exact hn
      · rename_i h
        rw [List.nodup_append]; refine ⟨hn, by simp, ?_⟩
        intro a ha b hb; simp at hb; subst hb; intro hab; subst hab; exact h ha
    · intro e' he'
      simp only [List.mem_cons] at he'
      rcases he' with rfl | he'
      · exact h2 _ hin
      · exact h3 e' he'
    · intro x hx
      rcases h4 x hx with hx | ⟨e', he', rfl⟩
      · unfold addNew at hx; split at hx
        · exact Or.inl hx
        · simp only [List.mem_append, List.mem_singleton] at hx
          rcases hx with hx | rfl
          · exact Or.inl hx
          · exact Or.inr ⟨e, by simp, rfl⟩
      · exact Or.inr ⟨e', by simp [he'], rfl⟩

/-- D3D9.STAGE.TCI: the first stage reads the first set; the sets are told apart (no two alike),
at most three; every enabled stage's set is one of them, and each is some stage's -/
theorem slots_sets (keys : List Key) (cops : List Nat) (s : List Key) (h : slots keys cops = some s) :
    s.head? = keys.head? ∧ s.Nodup ∧ s.length ≤ 3 ∧ (∀ e ∈ enabled keys cops, e.1 ∈ s) ∧
      (∀ x ∈ s, x ∈ keys.head? ∨ ∃ e ∈ enabled keys cops, e.1 = x) := by
  match keys, h with
  | k0 :: ks, h =>
    simp only [slots] at h
    split at h
    · rename_i hl
      simp only [Option.some.injEq] at h
      subst h
      have ⟨h1, h2, h3, h4⟩ := add_props (enabled (k0 :: ks) cops) [k0]
      refine ⟨?_, h1 (by simp), hl, h3, ?_⟩
      · have : ∀ (l : List (Key × Nat)) (t : List Key), (l.foldl addNew (k0 :: t)).head? = some k0 := by
          intro l; induction l with
          | nil => intro t; rfl
          | cons e l ih =>
            intro t; simp only [List.foldl_cons]; unfold addNew; split
            · exact ih t
            · exact ih (t ++ [e.1])
        exact this _ []
      · intro x hx
        rcases h4 x hx with hx | hx
        · left; simp at hx; simp [hx]
        · right; exact hx
    · simp at h

/-- the row of set `set` of the vertex's sets (tex: their words; sizes: each set's count): its
coordinates, then 1, then 0, past the sets (0, 0, 1, 0); and as passed through, (0, 0) for a
set past them or of one coordinate -/
def row (tex sizes : List Nat) (set : Nat) : Option (List Nat × List Nat) :=
  match sizes[set]? with
  | none => some ([0, 0, 0x3f800000, 0], [0, 0, 0x3f800000, 0])
  | some n => do
    let o := (sizes.take set).foldl (· + ·) 0
    let r ← [0, 1, 2, 3].mapM fun k => if k < n then tex[o + k]? else if k = n then some 0x3f800000 else some 0
    pure (r, if 2 ≤ n then r else [0, 0, r.getD 2 0, r.getD 3 0])

/-- D3D9.STAGE.TCI: a set past the vertex's is (0, 0, 1, 0) either way; a set of two read after
the sets before it is (u, v, 1, 0), of three (u, v, w, 1), of four as it is; a set of one is
(u, 1, 0, 0) to a texture matrix and passed through as (0, 0) -/
theorem row_sets (tex sizes : List Nat) (set a b u v w q : Nat) (h : sizes.length ≤ set) :
    row tex sizes set = some ([0, 0, 0x3f800000, 0], [0, 0, 0x3f800000, 0]) ∧
    row [a, b, u, v] [2, 2] 1 = some ([u, v, 0x3f800000, 0], [u, v, 0x3f800000, 0]) ∧
    row [a, u, v, w] [1, 3] 1 = some ([u, v, w, 0x3f800000], [u, v, w, 0x3f800000]) ∧
    row [u, v, w, q] [4] 0 = some ([u, v, w, q], [u, v, w, q]) ∧
    row [u] [1] 0 = some ([u, 0x3f800000, 0, 0], [0, 0, 0, 0]) := by
  refine ⟨?_, rfl, rfl, rfl, rfl⟩
  simp [row, List.getElem?_eq_none h]

def one64 : Nat := 0x3ff0000000000000

/-- column j of the row v times the matrix a (row by row), binary32: the sum from 0 in v's order -/
def times (v a : List Nat) (j : Nat) : Nat :=
  [0, 1, 2, 3].foldl (fun s k => F32.add s (F32.mul (v.getD k 0) (a.getD (4 * k + j) 0))) 0

/-- a stage's set v through its texture matrix a by TEXTURETRANSFORMFLAGS ttf, binary64: the
first two of v a and the count's last where PROJECTED divides by it, 1 where not (Direct3D 8's
COUNT2 not); COUNT1 v as it is (Direct3D 9) or (the first, 0) (Direct3D 8); none for another
count -/
def texTransform (ttf : Nat) (v8 : Bool) (v a : List Nat) : Option (List Nat) :=
  let t (j : Nat) := F64.ofF32 (times v a j)
  let c := ttf % 256
  if c = 1 then some (if v8 then [t 0, 0, one64] else [F64.ofF32 (v.getD 0 0), F64.ofF32 (v.getD 1 0), one64])
  else if c = 2 ∧ v8 then some [t 0, t 1, one64]
  else if 2 ≤ c ∧ c ≤ 4 then some [t 0, t 1, if ttf / 256 % 2 = 1 then t (c - 1) else one64]
  else none

/-- D3D9.STAGE.TEXXFORM: COUNT1 leaves the set as it is on Direct3D 9 (the matrix unread),
takes (the first, 0) on Direct3D 8; Direct3D 8's PROJECTED COUNT2 is COUNT2; PROJECTED COUNT3,
COUNT4 (and COUNT2 on Direct3D 9) divide by the count's last, without it by 1; the counts
are 1 .. 4 -/
theorem tex_transform (v a : List Nat) (b : Bool) :
    texTransform 1 false v a = some [F64.ofF32 (v.getD 0 0), F64.ofF32 (v.getD 1 0), one64] ∧
    texTransform 0x101 false v a = texTransform 1 false v a ∧
    texTransform 1 true v a = some [F64.ofF32 (times v a 0), 0, one64] ∧
    texTransform 0x102 true v a = texTransform 2 true v a ∧
    texTransform 0x102 false v a = some [F64.ofF32 (times v a 0), F64.ofF32 (times v a 1), F64.ofF32 (times v a 1)] ∧
    texTransform 0x103 b v a = some [F64.ofF32 (times v a 0), F64.ofF32 (times v a 1), F64.ofF32 (times v a 2)] ∧
    texTransform 0x104 b v a = some [F64.ofF32 (times v a 0), F64.ofF32 (times v a 1), F64.ofF32 (times v a 3)] ∧
    texTransform 3 b v a = some [F64.ofF32 (times v a 0), F64.ofF32 (times v a 1), one64] ∧
    texTransform 0 b v a = none ∧ texTransform 5 b v a = none := by
  cases b <;> simp [texTransform]

/-- D3D9.STAGE.TEXXFORM: the row (u, v, 1, 0) of (0.25, 0.5) through a scale by 2 and 3 is
(0.5, 1.5), and a translation's last row moves it: (0.25, 0.5) by (1, 2) is (1.25, 2.5) -/
theorem tex_transform_rows :
    texTransform 2 false [0x3e800000, 0x3f000000, 0x3f800000, 0]
      [0x40000000, 0, 0, 0, 0, 0x40400000, 0, 0, 0, 0, 0x3f800000, 0, 0, 0, 0, 0x3f800000] =
      some [0x3fe0000000000000, 0x3ff8000000000000, one64] ∧
    texTransform 2 false [0x3e800000, 0x3f000000, 0x3f800000, 0]
      [0x3f800000, 0, 0, 0, 0, 0x3f800000, 0, 0, 0x3f800000, 0x40000000, 0x3f800000, 0, 0, 0, 0, 0x3f800000] =
      some [0x3ff4000000000000, 0x4004000000000000, one64] := by
  decide +kernel

def key (l : List Int) (i : Nat) : Option Key := do
  pure (← nat (← l[3 * i]?), ← nat (← l[3 * i + 1]?), ← nat (← l[3 * i + 2]?))

def mirrors : List Mirror := [
  ⟨"d3d9.slots", fun l => do
    let keys ← [0, 1, 2, 3, 4, 5, 6, 7].mapM (key l)
    let cops ← [0, 1, 2, 3, 4, 5, 6, 7].mapM fun i => do nat (← l[24 + i]?)
    match slots keys cops with
    | some s => pure (s.flatMap fun (t, f, st) => [(t : Int), f, st])
    | none => pure []⟩,
  ⟨"d3d9.row", fun
    | n :: rest => do
      let n ← nat n
      let sizes ← (rest.take n).mapM nat
      let set ← nat (← rest[n]?)
      let tex ← (rest.drop (n + 1)).mapM nat
      match row tex sizes set with
      | some (r, p) => pure ((r ++ p).map Int.ofNat)
      | none => pure []
    | _ => none⟩,
  ⟨"d3d9.ttf", fun
    | ttf :: v8 :: rest => do
      let v ← (rest.take 4).mapM nat
      let a ← (rest.drop 4).mapM nat
      match texTransform (← nat ttf) (v8 != 0) v a with
      | some r => pure (r.flatMap fun x => F64.halves (F64.canon x))
      | none => pure []
    | _ => none⟩
]

end Kikizake.D3d9.Coord
