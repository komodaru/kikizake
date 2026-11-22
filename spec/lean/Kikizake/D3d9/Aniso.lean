import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Line
/-! Direct3D 9's anisotropic filtering on WARP (spec/d3d9draw.txt D3D9.TEX.ANISO2, TEX.ANISON) in
binary32 (Kikizake.F32) as src/model/d3d9draw.rs `aniso_footprint`, `aniso_taps` and
`pair_average` compute it (mirrors `d3d9.footprint`, `d3d9.taps`, `d3d9.pairavg`): the footprint
of the coarse derivatives, where the taps fall, and how their sums are averaged. Each tap's sum is
the bilinear weighted sum (Kikizake.D3d9.Filter) truncated to a 64th of 1/255. -/
namespace Kikizake.D3d9.Aniso

open Kikizake.F32 (add sub mul div lt isNaN)
open Kikizake.D3d9.Clip (half mhalf)
open Kikizake.D3d9.Line (one abs)

def two : Nat := 0x40000000

/-- x > y, x <= y (false for NaN) -/
def gt (x y : Nat) : Bool := !isNaN x && !isNaN y && lt y x
def le (x y : Nat) : Bool := !isNaN x && !isNaN y && !lt y x
def lt' (x y : Nat) : Bool := !isNaN x && !isNaN y && lt x y

/-- Rust's `a.max(b)`: a NaN gives the other -/
def fmax (a b : Nat) : Nat := if isNaN a then b else if isNaN b then a else if lt a b then b else a

def nan0 (x : Nat) : Nat := if isNaN x then 0 else x

/-- MAXANISOTROPY taken down to a power of 2, 1 to 16 -/
def count (limit : Nat) : Nat :=
  if limit ≤ 1 then 1 else if limit ≤ 3 then 2 else if limit ≤ 7 then 4 else if limit ≤ 15 then 8 else 16

/-- D3D9.TEX.ANISO2: the footprint of derivatives a (ddx) and b (ddy) on a w x h texture: (r, the
major axis's direction, the tap count): X2 = a.a, Y2 = b.b, area = |a_u b_v - a_v b_u|, the major
axis ddx only where X2 > Y2, M = sqrt(M2), axis = derivative times 1/M; r = M2 / area, at most n
(then minor = M / r, else area / M); where minor < 1, r = max(1, r minor); NaN as 0 -/
def footprint (a0 a1 b0 b1 w h limit : Nat) : Nat × (Nat × Nat) × Nat :=
  let a0 := mul a0 w
  let a1 := mul a1 h
  let b0 := mul b0 w
  let b1 := mul b1 h
  let x2 := add (mul a0 a0) (mul a1 a1)
  let y2 := add (mul b0 b0) (mul b1 b1)
  let area := abs (sub (mul a0 b1) (mul a1 b0))
  let (m2, d0, d1) := if gt x2 y2 then (x2, a0, a1) else (y2, b0, b1)
  let m := F32.sqrt m2
  let inv := div one m
  let n := count limit
  let nf := F32.ofNat n
  let r0 := div m2 area
  let clamped := gt r0 nf
  let r1 := if clamped then nf else r0
  let minor := if clamped then div m r1 else div area m
  let r2 := if lt' minor one then fmax (mul r1 minor) one else r1
  (nan0 r2, (nan0 (mul d0 inv), nan0 (mul d1 inv)), n)

/-- D3D9.TEX.ANISO2: the tap count is MAXANISOTROPY taken down to 1, 2, 4, 8 or 16 -/
theorem count_pow2 (limit : Nat) : count limit ∈ [1, 2, 4, 8, 16] := by
  unfold count
  repeat' split
  all_goals simp

theorem footprint_count (a0 a1 b0 b1 w h limit : Nat) : (footprint a0 a1 b0 b1 w h limit).2.2 = count limit := rfl

private theorem nan0_ok (x : Nat) : isNaN (nan0 x) = false := by
  unfold nan0
  split
  · rfl
  · simp_all

/-- D3D9.TEX.ANISO2: r and the axis are never NaN -/
theorem footprint_not_nan (a0 a1 b0 b1 w h limit : Nat) :
    isNaN (footprint a0 a1 b0 b1 w h limit).1 = false ∧ isNaN (footprint a0 a1 b0 b1 w h limit).2.1.1 = false ∧
    isNaN (footprint a0 a1 b0 b1 w h limit).2.1.2 = false := by
  refine ⟨nan0_ok _, nan0_ok _, nan0_ok _⟩

def walk (s0 s1 : Nat) : Nat → Nat × Nat → List (Nat × Nat)
  | 0, _ => []
  | k + 1, (q0, q1) => (q0, q1) :: walk s0 s1 k (add q0 s0, add q1 s1)

theorem walk_length (s0 s1 k : Nat) (q : Nat × Nat) : (walk s0 s1 k q).length = k := by
  induction k generalizing q with
  | zero => rfl
  | succ k ih => simp [walk, ih]

/-- D3D9.TEX.ANISO2, ANISON: where the taps around (u, v) fall: r <= 1 none (bilinear); r <= 2 two
at the center plus and less 0.5 (r - 1) axis; past 2 n taps, step = f32(1/(n + 1)) r, the first at
the center plus (-0.5 r + step) axis, each next a step on -/
def taps (u v r : Nat) (ax : Nat × Nat) (n w h : Nat) : List (Nat × Nat) :=
  if !gt r one then [] else
  if le r two then
    let hh := mul half (sub r one)
    let o0 := div (mul hh ax.1) w
    let o1 := div (mul hh ax.2) h
    [(add u o0, add v o1), (sub u o0, sub v o1)]
  else
    let k := div one (F32.ofNat (n + 1))
    let step := mul k r
    let start := add (mul mhalf r) step
    let d0 := div ax.1 w
    let d1 := div ax.2 h
    walk (mul d0 step) (mul d1 step) n (add u (mul d0 start), add v (mul d1 start))

/-- D3D9.TEX.ANISO2: r <= 1 takes no taps: the bilinear filter -/
theorem taps_bilinear (u v r : Nat) (ax : Nat × Nat) (n w h : Nat) (hr : gt r one = false) : taps u v r ax n w h = [] := by
  simp [taps, hr]

/-- D3D9.TEX.ANISO2: r in (1, 2]: two taps, the center plus and less the same offset -/
theorem taps_two (u v r : Nat) (ax : Nat × Nat) (n w h : Nat) (h1 : gt r one = true) (h2 : le r two = true) :
    taps u v r ax n w h = [(add u (div (mul (mul half (sub r one)) ax.1) w), add v (div (mul (mul half (sub r one)) ax.2) h)),
      (sub u (div (mul (mul half (sub r one)) ax.1) w), sub v (div (mul (mul half (sub r one)) ax.2) h))] := by
  simp [taps, h1, h2]

/-- D3D9.TEX.ANISON: r past 2: always n taps -/
theorem taps_n (u v r : Nat) (ax : Nat × Nat) (n w h : Nat) (h1 : gt r one = true) (h2 : le r two = false) :
    (taps u v r ax n w h).length = n := by
  simp [taps, h1, h2, walk_length]

/-- one round of averaging neighbours in pairs, truncating; none for an odd count -/
def halve : List (List Nat) → Option (List (List Nat))
  | [] => some []
  | [_] => none
  | a :: b :: r => do
    let t ← halve r
    pure (((a.zip b).map fun (x, y) => (x + y) / 2) :: t)

def averageGo : Nat → List (List Nat) → Option (List Nat)
  | 0, vs => vs.head?
  | f + 1, vs => if vs.length ≤ 1 then vs.head? else do averageGo f (← halve vs)

/-- D3D9.TEX.ANISON: the taps' sums averaged in pairs of neighbours, truncating, until one is left -/
def pairAverage (vs : List (List Nat)) : Option (List Nat) := averageGo vs.length vs

private theorem zip_self (a : List Nat) : ((a.zip a).map fun (x, y) => (x + y) / 2) = a := by
  induction a with
  | nil => rfl
  | cons x r ih => simp only [List.zip_cons_cons, List.map_cons, ih]; congr 1; omega

private theorem halve_same (x : List Nat) (m : Nat) : halve (List.replicate (2 * m) x) = some (List.replicate m x) := by
  induction m with
  | zero => rfl
  | succ m ih =>
    have : 2 * (m + 1) = 2 * m + 1 + 1 := by omega
    rw [this, List.replicate_succ, List.replicate_succ]
    simp [halve, ih, zip_self, List.replicate_succ]

private theorem avg_rep (x : List Nat) (k : Nat) : ∀ f, k ≤ f → averageGo f (List.replicate (2 ^ k) x) = some x := by
  induction k with
  | zero => intro f _; cases f <;> simp [averageGo]
  | succ k ih =>
    intro f hf
    obtain ⟨g, rfl⟩ : ∃ g, f = g + 1 := ⟨f - 1, by omega⟩
    have hp : 2 ^ (k + 1) = 2 * 2 ^ k := by rw [Nat.pow_succ]; omega
    have hpos : 0 < 2 ^ k := Nat.two_pow_pos k
    rw [hp]
    have hl : ¬ (List.replicate (2 * 2 ^ k) x).length ≤ 1 := by simp; omega
    simp only [averageGo, hl, ↓reduceIte, halve_same, Option.bind_eq_bind, Option.bind_some]
    exact ih g (by omega)

/-- D3D9.TEX.ANISON: taps of one value average to it, whatever their count (a power of two) -/
theorem average_same (x : List Nat) (k : Nat) : pairAverage (List.replicate (2 ^ k) x) = some x := by
  unfold pairAverage
  rw [List.length_replicate]
  exact avg_rep x k _ (Nat.le_of_lt (Nat.lt_two_pow_self))

/-- D3D9.TEX.ANISON: two taps average to their halved sum, truncated -/
theorem average_two (a b : List Nat) : pairAverage [a, b] = some ((a.zip b).map fun (x, y) => (x + y) / 2) := by
  simp [pairAverage, averageGo, halve]

def mirrors : List Mirror := [
  ⟨"d3d9.footprint", fun
    | [a0, a1, b0, b1, w, h, limit] => do
      let (r, (x, y), n) := footprint (← nat a0) (← nat a1) (← nat b0) (← nat b1) (← nat w) (← nat h) (← nat limit)
      pure [F32.canon r, F32.canon x, F32.canon y, n]
    | _ => none⟩,
  ⟨"d3d9.taps", fun
    | [u, v, r, x, y, n, w, h] => do
      let t := taps (← nat u) (← nat v) (← nat r) (← nat x, ← nat y) (← nat n) (← nat w) (← nat h)
      pure (t.flatMap fun (a, b) => [(F32.canon a : Int), (F32.canon b : Int)])
    | _ => none⟩,
  ⟨"d3d9.pairavg", fun
    | ws => do
      let ns ← ws.mapM nat
      let rec quads : List Nat → List (List Nat)
        | a :: b :: c :: d :: r => [a, b, c, d] :: quads r
        | _ => []
      match pairAverage (quads ns) with
      | none => pure [-1]
      | some s => pure (s.map fun k : Nat => (k : Int))⟩
]

end Kikizake.D3d9.Aniso
