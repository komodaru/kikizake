import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Clip
/-! Direct3D 9's DrawPrimitiveUP before rasterization (spec/d3d9draw.txt D3D9.DRAW.ASSEMBLY,
DRAW.STRIDE, DRAW.POINTS) and the pixel a draw writes (DRAW.XZERO), as src/model/d3d9draw.rs
`assemble`, `vertex_words`, `point_pixel` and `pack` compute them (mirrors `d3d9.assemble`,
`d3d9.vertices`, `d3d9.pointpixel`, `d3d9.pack`). -/
namespace Kikizake.D3d9.Draw

/-- D3D9.DRAW.ASSEMBLY: lines (2 LINELIST, 3 LINESTRIP) as pairs, triangles (4 LIST, 5 STRIP, 6
FAN) as triples, each with its first (flat shading) vertex first -/
def assemble (ty nv : Nat) : Option (List (List Nat)) :=
  match ty with
  | 2 => some ((List.range (nv / 2)).map fun i => [2 * i, 2 * i + 1])
  | 3 => some ((List.range (nv - 1)).map fun i => [i, i + 1])
  | 4 => some ((List.range (nv / 3)).map fun i => [3 * i, 3 * i + 1, 3 * i + 2])
  | 5 => some ((List.range (nv - 2)).map fun i => if i % 2 = 0 then [i, i + 1, i + 2] else [i, i + 2, i + 1])
  | 6 => some ((List.range (nv - 2)).map fun i => [i + 1, i + 2, 0])
  | _ => none

/-- D3D9.DRAW.ASSEMBLY: a strip's triangles each take a vertex and the two after it, every odd one
with its last two swapped (so all keep the first's winding as the strip turns); a fan's each next
pair with the first; a list three vertices each; every vertex number below nv -/
theorem strip (nv i : Nat) (h : i < nv - 2) :
    ((assemble 5 nv).getD [])[i]? = some (if i % 2 = 0 then [i, i + 1, i + 2] else [i, i + 2, i + 1]) := by
  simp [assemble, h]

theorem fan (nv i : Nat) (h : i < nv - 2) : ((assemble 6 nv).getD [])[i]? = some [i + 1, i + 2, 0] := by
  simp [assemble, h]

theorem list (nv i : Nat) (h : i < nv / 3) : ((assemble 4 nv).getD [])[i]? = some [3 * i, 3 * i + 1, 3 * i + 2] := by
  simp [assemble, h]

theorem within (ty nv : Nat) (t : List Nat) (ht : t ∈ (assemble ty nv).getD []) : ∀ k ∈ t, k < nv := by
  intro k hk
  unfold assemble at ht
  split at ht <;> simp at ht <;> obtain ⟨i, hi, rfl⟩ := ht <;> (try simp at hk) <;> (try split at hk) <;> (try simp at hk) <;> omega

/-- D3D9.DRAW.STRIDE: each of nv vertices is read from its stride's start, the first size words
(the FVF's) of it; none for a stride short of the size -/
def vertices (words : List Nat) (nv size : Nat) : Option (List (List Nat)) :=
  if nv = 0 then none else
  let stride := words.length / nv
  if stride < size || stride = 0 || words.length % nv != 0 then none
  else some ((List.range nv).map fun i => (words.drop (i * stride)).take size)

theorem stride_read (words : List Nat) (nv size i : Nat) (vs : List (List Nat)) (h : vertices words nv size = some vs)
    (hi : i < nv) : vs[i]? = some ((words.drop (i * (words.length / nv))).take size) := by
  unfold vertices at h
  split at h
  · simp at h
  · simp only at h
    split at h
    · simp at h
    · simp only [Option.some.injEq] at h
      subst h
      simp [hi]

/-- D3D9.DRAW.POINTS: the pixel a point lights, ceil(x - 0.5) of the f32 difference -/
def ceilHalf (x : Nat) : Int :=
  let (s, n, d) := Float.toRat F32.fmt (F32.sub x Clip.half)
  if s then -((n / d : Nat) : Int) else (((n + d - 1) / d : Nat) : Int)

/-- Rust's `as i64` of it: NaN 0, past i64 (infinities among them) saturated -/
def pixel (x : Nat) : Int :=
  let v := F32.sub x Clip.half
  if F32.isNaN v then 0 else
  if Float.isInf F32.fmt v then (if Float.sign F32.fmt v then -2 ^ 63 else 2 ^ 63 - 1) else
  max (-2 ^ 63) (min (2 ^ 63 - 1) (ceilHalf x))

/-- D3D9.DRAW.POINTS: the nearest pixel center, halves down: c - 1 < x - 1/2 ≤ c for the value of
x - 1/2 (the f32 difference, exact for the positions drawn), before `as i64` saturates -/
private theorem nearest (x : Nat) (s : Bool) (n d : Nat)
    (hv : Float.toRat F32.fmt (F32.sub x Clip.half) = (s, n, d)) (hd : 0 < d) :
    let v : Int := if s then -(n : Int) else n
    (ceilHalf x - 1) * d < v ∧ v ≤ ceilHalf x * d := by
  simp only [ceilHalf, hv]
  have e := Nat.div_add_mod n d
  have hr := Nat.mod_lt n hd
  have e2 := Nat.div_add_mod (n + d - 1) d
  have hr2 := Nat.mod_lt (n + d - 1) hd
  generalize n / d = q at *
  generalize n % d = r at *
  generalize (n + d - 1) / d = q2 at *
  generalize (n + d - 1) % d = r2 at *
  have e' : (d : Int) * q + r = n := by exact_mod_cast e
  have e2' : (d : Int) * q2 + r2 = n + d - 1 := by
    have : n + d - 1 + 1 = n + d := by omega
    omega
  cases s <;> simp only [Bool.false_eq_true, ↓reduceIte] <;>
    simp only [Int.sub_mul, Int.neg_mul, Int.one_mul, Int.mul_comm (q : Int) d, Int.mul_comm (q2 : Int) d] <;> omega

/-- D3D9.DRAW.POINTS: the pixel lit is that nearest center, for a finite x - 1/2 within i64 -/
theorem nearest_pixel (x : Nat) (s : Bool) (n d : Nat) (hnan : F32.isNaN (F32.sub x Clip.half) = false)
    (hinf : Float.isInf F32.fmt (F32.sub x Clip.half) = false)
    (hlo : -2 ^ 63 ≤ ceilHalf x) (hhi : ceilHalf x ≤ 2 ^ 63 - 1)
    (hv : Float.toRat F32.fmt (F32.sub x Clip.half) = (s, n, d)) (hd : 0 < d) :
    (pixel x - 1) * d < (if s then -(n : Int) else n) ∧ (if s then -(n : Int) else n) ≤ pixel x * d := by
  have e : pixel x = ceilHalf x := by
    simp only [pixel, hnan, hinf, Bool.false_eq_true, ↓reduceIte]
    omega
  rw [e]
  exact nearest x s n d hv hd

/-- D3D9.DRAW.XZERO: a pixel's channels (letter, shift, raw value) put together; the X bits 0
unless `xa` (Clear writes the alpha there) -/
def pack (ch : List (Nat × Nat × Nat)) (xa : Bool) : Nat :=
  (ch.filter fun c => c.1 != 'x'.toNat || xa).foldl (fun w c => w ||| c.2.2 <<< c.2.1) 0

/-- D3D9.DRAW.XZERO: what a draw writes does not depend on the X channels' values -/
theorem xzero (ch : List (Nat × Nat × Nat)) : pack ch false = pack (ch.filter fun c => c.1 != 'x'.toNat) false := by
  simp [pack, List.filter_filter]

theorem xzero_any (ch ch' : List (Nat × Nat × Nat)) (h : ch.filter (fun c => c.1 != 'x'.toNat) = ch'.filter (fun c => c.1 != 'x'.toNat)) :
    pack ch false = pack ch' false := by
  rw [xzero ch, xzero ch', h]

def mirrors : List Mirror := [
  ⟨"d3d9.assemble", fun
    | [ty, nv] => do
      match assemble (← nat ty) (← nat nv) with
      | some r => pure (r.flatten.map fun k : Nat => (k : Int))
      | none => pure [-1]
    | _ => none⟩,
  ⟨"d3d9.vertices", fun
    | nv :: size :: ws => do
      match vertices (← ws.mapM nat) (← nat nv) (← nat size) with
      | some r => pure (r.flatten.map fun k : Nat => (k : Int))
      | none => pure [-1]
    | _ => none⟩,
  ⟨"d3d9.pointpixel", fun
    | [x] => do pure [pixel (← nat x)]
    | _ => none⟩,
  ⟨"d3d9.pack", fun
    | xa :: rest => do
      let ns ← rest.mapM nat
      let rec triples : List Nat → List (Nat × Nat × Nat)
        | a :: b :: c :: r => (a, b, c) :: triples r
        | _ => []
      let w := pack (triples ns) (xa != 0)
      pure ((List.range 4).map fun k => ((w / 2 ^ (32 * k) % 2 ^ 32 : Nat) : Int))
    | _ => none⟩
]

end Kikizake.D3d9.Draw
