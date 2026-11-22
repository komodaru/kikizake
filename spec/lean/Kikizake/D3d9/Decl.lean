import Kikizake.Mirror
/-! Direct3D 9's CreateVertexDeclaration (spec/d3d9draw.txt D3D9.DECL.CREATE), as
src/model/d3d9draw.rs `decl_ok` decides it: an element is (stream and offset, type, method,
usage and usage index) as the two words the case language writes (mirror `d3d9.decl`). -/
namespace Kikizake.D3d9.Decl

/-- one element: the stream in the low 16 bits of the first word and the offset above them; the
type, method, usage, usage index the bytes of the second -/
abbrev El := Nat × Nat

def ty (x : El) : Nat := x.2 % 256
def method (x : El) : Nat := x.2 / 256 % 256
def usage (x : El) : Nat := x.2 / 65536 % 256
def index (x : El) : Nat := x.2 / 16777216
def stream (x : El) : Nat := x.1 % 65536
def key (x : El) : Nat × Nat := (usage x, index x)

/-- an element on its own: an offset a multiple of 4, a type to FLOAT16_4 (16), the DEFAULT
method, a usage to DEPTH (12), an index below 16, TESSFACTOR (8) only a FLOAT1 of index 0 -/
def elemOk (x : El) : Bool :=
  x.1 / 65536 % 4 == 0 && ty x ≤ 16 && method x == 0 && usage x ≤ 12 && index x < 16 &&
    !(usage x == 8 && (ty x != 0 || index x != 0))

/-- some key twice -/
def dup : List (Nat × Nat) → Bool
  | [] => false
  | k :: r => r.contains k || dup r

/-- a stream that comes back after another (seen: the streams before, prev the last) -/
def apart : List Nat → Nat → List Nat → Bool
  | [], _, _ => false
  | s :: r, prev, seen => (s != prev && seen.contains s) || apart r s (s :: seen)

def declOk (e : List El) : Bool :=
  let keys := e.map key
  let streams := e.map stream
  let oneStream := streams.all (· == streams.headD 0)
  e.all elemOk && !dup keys && !apart streams (streams.headD 0) [] &&
    !(keys.contains (9, 0) && (keys.contains (0, 0) || !oneStream))

/-- D3D9.DECL.CREATE: an element refused on its own refuses the declaration -/
theorem bad_element (e : List El) (x : El) (hx : x ∈ e) (h : elemOk x = false) : declOk e = false := by
  have : e.all elemOk = false := by
    rw [List.all_eq_false]; exact ⟨x, hx, by simp [h]⟩
  unfold declOk; dsimp only; rw [this]; simp

private theorem dup_mid (k : Nat × Nat) (l1 l2 l3 : List (Nat × Nat)) : dup (l1 ++ k :: (l2 ++ k :: l3)) = true := by
  induction l1 with
  | nil => simp [dup]
  | cons a r ih => simp [dup, ih]

/-- D3D9.DECL.CREATE: a usage and index twice refuses the declaration, wherever the two are -/
theorem twice (x y : El) (l1 l2 l3 : List El) (h : key x = key y) :
    declOk (l1 ++ x :: (l2 ++ y :: l3)) = false := by
  have : dup ((l1 ++ x :: (l2 ++ y :: l3)).map key) = true := by
    simp only [List.map_append, List.map_cons, h]; exact dup_mid _ _ _ _
  unfold declOk; dsimp only; rw [this]; simp

/-- D3D9.DECL.CREATE: a POSITIONT (9) of index 0 with a POSITION (0) of index 0 refuses -/
theorem positiont (e : List El) (x y : El) (hx : x ∈ e) (hy : y ∈ e) (h9 : key x = (9, 0)) (h0 : key y = (0, 0)) :
    declOk e = false := by
  have a : (e.map key).contains (9, 0) = true := by
    simp only [List.contains_iff_mem, List.mem_map]; exact ⟨x, hx, h9⟩
  have b : (e.map key).contains (0, 0) = true := by
    simp only [List.contains_iff_mem, List.mem_map]; exact ⟨y, hy, h0⟩
  unfold declOk; dsimp only; rw [a, b]; simp

private theorem apart_found (r : List Nat) : ∀ (prev : Nat) (seen : List Nat) (s : Nat),
    s ∈ seen → s ≠ prev → s ∈ r → apart r prev seen = true := by
  induction r with
  | nil => intro _ _ _ _ _ h; simp at h
  | cons t r ih =>
    intro prev seen s hs hp hr
    by_cases e : s = t
    · subst e; simp [apart, hs, hp]
    · have : s ∈ r := by simp [e] at hr; exact hr
      simp [apart, ih t (t :: seen) s (by simp [hs]) e this]

private theorem apart_append (p : List Nat) : ∀ (r : List Nat) (prev : Nat) (seen : List Nat),
    apart (p ++ r) prev seen = (apart p prev seen || apart r (p.getLastD prev) (p.reverse ++ seen)) := by
  induction p with
  | nil => intros; simp [apart]
  | cons t p ih =>
    intro r prev seen
    simp only [List.cons_append, apart, ih, List.reverse_cons, List.append_assoc,
      Bool.or_assoc]
    cases p <;> rfl

/-- D3D9.DECL.CREATE: a stream's elements apart (the stream back after another stream's) refuses -/
theorem stream_apart (x y z : El) (l1 l2 l3 : List El) (hxy : stream x ≠ stream y) (hz : stream z = stream x) :
    declOk (l1 ++ x :: y :: (l2 ++ z :: l3)) = false := by
  have : apart ((l1 ++ x :: y :: (l2 ++ z :: l3)).map stream) (((l1 ++ x :: y :: (l2 ++ z :: l3)).map stream).headD 0) [] = true := by
    have e : (l1 ++ x :: y :: (l2 ++ z :: l3)).map stream =
        (l1.map stream ++ [stream x, stream y]) ++ (l2.map stream ++ stream x :: l3.map stream) := by simp [hz]
    rw [e, apart_append]
    have : (l1.map stream ++ [stream x, stream y]).getLastD
        (((l1.map stream ++ [stream x, stream y]) ++ (l2.map stream ++ stream x :: l3.map stream)).headD 0) = stream y := by
      simp
    rw [this, apart_found _ (stream y) _ (stream x) (by simp) hxy (by simp), Bool.or_true]
  unfold declOk; dsimp only; rw [this]; simp

def mirrors : List Mirror := [
  ⟨"d3d9.decl", fun
    | ws => do
      if ws.length % 2 != 0 then none
      let ns ← ws.mapM nat
      let rec pairs : List Nat → List El
        | a :: b :: r => (a, b) :: pairs r
        | _ => []
      pure [if declOk (pairs ns) then 1 else 0]⟩
]

end Kikizake.D3d9.Decl
