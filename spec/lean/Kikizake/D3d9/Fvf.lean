import Kikizake.Mirror
/-! The flexible vertex formats (spec/d3dx9.txt, spec/d3d9draw.txt), as src/model/d3dx9.rs and
src/model/d3d9draw.rs compute them: `fvfDecl` is `fvf_decl` (D3DXDeclaratorFromFVF), `declFvf`
`decl_fvf` (D3DXFVFFromDeclarator), `fvfSize` `fvf_size` (D3DXGetFVFVertexSize) and
`runtimeDecl` `fvf_runtime_decl` (the declaration SetFVF makes) (mirrors `d3dx9.fvfdecl`,
`d3dx9.declfvf`, `d3dx9.fvfsize`, `d3d9.fvfdecl`). An element is the executor's pair (Stream |
Offset << 16, Type | Method << 8 | Usage << 16 | UsageIndex << 24). -/
namespace Kikizake.D3d9.Fvf

def INVALIDCALL : Nat := 0x8876086c

/-- x with the bits of m cleared -/
def clr (x m : Nat) : Nat := x ^^^ (x &&& m)

/-- D3DDECLTYPE's size in bytes; none past UNUSED -/
def declTypeSize : Nat → Option Nat
  | 0 | 4 | 5 | 6 | 8 | 9 | 11 | 13 | 14 | 15 => some 4
  | 1 | 7 | 10 | 12 | 16 => some 8
  | 2 => some 12
  | 3 => some 16
  | 17 => some 0
  | _ => none

def elem (off ty usage index : Nat) : Nat × Nat := (off <<< 16, ty ||| usage <<< 16 ||| index <<< 24)

/-- D3DFVF_TEXCOORDSIZEn: the floats of texture coordinate set i (2 past the 32 bits) -/
def texFloats (fvf i : Nat) : Nat := [2, 3, 4, 1].getD ((fvf >>> (16 + 2 * i)) % 4) 0

/-- the texture coordinate sets an FVF names -/
def sets (fvf : Nat) : Nat := (fvf >>> 8) % 16

/-- the position bits and whether they carry blend weights -/
def pos (fvf : Nat) : Nat := fvf &&& 0x400e
def blend (fvf : Nat) : Bool := decide (6 ≤ pos fvf &&& 0xe ∧ pos fvf &&& 0xe ≤ 0xe ∧ pos fvf &&& 0x4000 = 0)
/-- the blend weights left after a LASTBETA index -/
def weights (fvf : Nat) : Nat :=
  if blend fvf then 1 + ((pos fvf - 6) >>> 1) - (if fvf &&& 0x9000 ≠ 0 then 1 else 0) else 0

/-- the elements' (type, usage, index), in FVF order -/
def items (fvf : Nat) : List (Nat × Nat × Nat) :=
  (if pos fvf = 0 then [] else
    [if pos fvf = 4 then (3, 9, 0) else (2, 0, 0)] ++
    (if blend fvf then
      (if weights fvf > 0 then [(weights fvf - 1, 1, 0)] else []) ++
      (if fvf &&& 0x1000 ≠ 0 then [(5, 2, 0)] else if fvf &&& 0x8000 ≠ 0 then [(4, 2, 0)] else [])
    else [])) ++
  ([(0x10, 2, 3, 0), (0x20, 0, 4, 0), (0x40, 4, 10, 0), (0x80, 4, 10, 1)].filterMap
    fun (bit, it) => if fvf &&& bit ≠ 0 then some it else none) ++
  (List.range (sets fvf)).map fun i => (texFloats fvf i - 1, 5, i)

/-- the elements packed from offset off -/
def pack : List (Nat × Nat × Nat) → Nat → List (Nat × Nat)
  | [], _ => []
  | (t, u, i) :: r, off => elem off t u i :: pack r (off + (declTypeSize t).getD 0)

/-- D3D9.FVFDECL, FVFDECL.RESERVED, TEXCOUNT, POSITION: D3DXDeclaratorFromFVF -/
def fvfDecl (fvf : Nat) : Except Nat (List (Nat × Nat)) :=
  if fvf &&& 0x6001 ≠ 0 then .error INVALIDCALL
  else if sets fvf > 8 then .error INVALIDCALL
  else if weights fvf > 4 then .error INVALIDCALL
  else .ok (pack (items fvf) 0)

/-- D3DXGetFVFVertexSize -/
def fvfSize (fvf : Nat) : Nat :=
  [(0x10, 12), (0x20, 4), (0x40, 4), (0x80, 4)].foldl (fun n (bit, s) => if fvf &&& bit ≠ 0 then n + s else n) 0 +
  (match pos fvf with
    | 0x002 => 12 | 0x004 | 0x006 => 16 | 0x008 => 20 | 0x00a => 24 | 0x00c => 28 | 0x00e => 32 | _ => 0) +
  ((List.range (sets fvf)).map fun i => 4 * texFloats fvf i).sum

/-- element i, and D3DDECL_END past the last -/
def elemAt (e : List (Nat × Nat)) (i : Nat) : Nat × Nat := e.getD i (0xff, 17)
def isEnd (e : List (Nat × Nat)) (i : Nat) : Bool := (elemAt e i).1 % 65536 == 0xff
def is (e : List (Nat × Nat)) (i ty usage : Nat) (index : Option Nat) : Bool :=
  !isEnd e i && (elemAt e i).2 % 256 == ty && ((elemAt e i).2 >>> 16) % 256 == usage &&
    (match index with | none => true | some x => (elemAt e i).2 >>> 24 == x)

/-- the position, blend weights and indices: the FVF's bits and the next element -/
def posPart (e : List (Nat × Nat)) : Except Nat (Nat × Nat) :=
  if is e 0 2 0 none then
    let (p, i) := match [(3, 4), (2, 3), (1, 2), (0, 1)].find? (fun (ty, _) => is e 1 ty 1 none) with
      | some (_, n) => (0x006 + 2 * (n - 1), 2)
      | none => (0x002, 1)
    if is e i 0 2 (some 0) && i == 2 then .error INVALIDCALL
    else match [(5, 0x1000), (4, 0x8000)].find? (fun (ty, _) => is e i ty 2 (some 0)) with
      | some (_, beta) => .ok ((if i == 1 then 0x006 else p + 2) ||| beta, i + 1)
      | none => .ok (p, i)
  else if is e 0 3 9 none then .ok (0x004, 1)
  else .ok (0, 0)

/-- the normal, point size and colors -/
def flagPart (e : List (Nat × Nat)) (fi : Nat × Nat) : Nat × Nat :=
  [(0x10, 2, 3, none), (0x20, 0, 4, some 0), (0x40, 4, 10, some 0), (0x80, 4, 10, some 1)].foldl
    (fun (f, i) (bit, ty, us, ix) => if is e i ty us ix then (f ||| bit, i + 1) else (f, i)) fi

/-- the texture coordinate sets, to the end or the 8th: (sets, FVF bits, next element) -/
def texPart (e : List (Nat × Nat)) : Nat → Nat → Nat → Nat → Except Nat (Nat × Nat × Nat)
  | 0, n, i, f => .ok (n, f, i)
  | fuel + 1, n, i, f =>
    if isEnd e i || n ≥ 8 then .ok (n, f, i)
    else match (List.range 4).find? (fun k => is e i k 5 (some n)) with
      | none => .error INVALIDCALL
      | some k => texPart e fuel (n + 1) (i + 1) (f ||| ([3, 0, 1, 2].getD k 0 <<< (16 + 2 * n)))

/-- every element in stream 0 at the offset packing gives it -/
def packed : List (Nat × Nat) → Nat → Bool
  | [], _ => true
  | x :: r, off => x.1 % 65536 == 0xff || (x.1 >>> 16 == off && x.1 % 65536 == 0 && packed r (off + (declTypeSize (x.2 % 256)).getD 0))

/-- D3D9.DECLFVF, DECLFVF.INVALID: D3DXFVFFromDeclarator -/
def declFvf (e : List (Nat × Nat)) : Except Nat Nat := do
  let (p, i) ← posPart e
  let (f, i) := flagPart e (p, i)
  let (n, f, _) ← texPart e 8 0 i f
  if packed e 0 then pure (f ||| n <<< 8) else .error INVALIDCALL

/-- D3D9.FVF.DECL, TEXCOUNT, BLEND5: the declaration SetFVF makes (none for FVF 0; an error
where the model has no answer) -/
def runtimeDecl (fvf : Nat) : Except Unit (Option (List (Nat × Nat))) :=
  if fvf = 0 then .ok none else
  let s := sets fvf
  let xyzw := fvf &&& 0x400e == 0x4002
  let base := clr (clr fvf 0x6001) (if fvf &&& 0x4000 ≠ 0 then 0x400e else 0) ||| (if xyzw then 0x002 else 0)
  let base := if s > 8 then clr base 0xf00 ||| 8 <<< 8 else base
  let b5 := base &&& 0x400e == 0xe && base &&& 0x9000 == 0
  match fvfDecl (if b5 then base ||| 0x1000 else base) with
  | .error _ => .error ()
  | .ok e =>
    let e := if b5 then e.modify 2 (fun x => (x.1, clr x.2 255)) else e
    let e := if s > 8 then
        (List.range (s - 8)).foldl (fun e k => match e.getLast? with
          | some l => e ++ [elem ((l.1 >>> 16) + (declTypeSize (l.2 % 256)).getD 0) 1 5 (8 + k)]
          | none => e) e
      else e
    let e := if xyzw then (match e with | [] => [] | x :: r => (x.1, clr x.2 255 ||| 3) :: r.map fun y => (y.1 + (4 <<< 16), y.2)) else e
    let e := if fvf &&& 0x2000 ≠ 0 then
        let cut := (e.takeWhile fun x => (x.2 >>> 16) % 256 != 5).length
        let off := match e[cut]?, e.getLast? with
          | some x, _ => x.1 >>> 16
          | none, some l => (l.1 >>> 16) + (declTypeSize (l.2 % 256)).getD 0
          | none, none => 0
        let moved := e.take cut ++ (e.drop cut).map fun y => (y.1 + (4 <<< 16), y.2)
        moved.take cut ++ [elem off 0 11 0] ++ moved.drop cut
      else e
    .ok (some e)

/-- D3D9.FVFDECL.RESERVED, TEXCOUNT, POSITION: D3DXDeclaratorFromFVF refuses the reserved bits,
more than eight texture coordinate sets and more than four blend weights -/
theorem fvf_decl_refuses (fvf : Nat) :
    (fvf &&& 0x6001 ≠ 0 → fvfDecl fvf = .error INVALIDCALL) ∧
    (sets fvf > 8 → fvfDecl fvf = .error INVALIDCALL) ∧
    (weights fvf > 4 → fvfDecl fvf = .error INVALIDCALL) := by
  refine ⟨fun h => by simp [fvfDecl, h], fun h => ?_, fun h => ?_⟩
  · unfold fvfDecl; split <;> simp_all
  · unfold fvfDecl; split
    · rfl
    · split
      · rfl
      · simp

/-- an FVF in the shape the theorems below run over: the position bits, the LASTBETA flags,
the normal, point size and colors, and n sets of texture coordinates, the first 8 (the 32 bits
hold) of size code t -/
def shaped (p beta flags n t : Nat) : Nat :=
  [0, 0x002, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e].getD p 0 |||
  [0, 0x1000, 0x8000, 0x9000].getD beta 0 ||| flags <<< 4 ||| n <<< 8 |||
  (List.range (Min.min n 8)).foldl (fun f i => f ||| t <<< (16 + 2 * i)) 0

/-- the declaration's bytes -/
def declSize (e : List (Nat × Nat)) : Nat := (e.map fun x => (declTypeSize (x.2 % 256)).getD 0).sum

/-- D3DXFVFFromDeclarator takes back a declaration D3DXDeclaratorFromFVF made, to an FVF that
makes the same declaration -/
def roundTrips (fvf : Nat) : Bool :=
  match fvfDecl fvf with
  | .error _ => true
  | .ok e => match declFvf e with
    | .ok g => match fvfDecl g with
      | .ok e' => e' == e
      | .error _ => false
    | .error _ => false

/-- D3DXGetFVFVertexSize is the size of the declaration -/
def sizeAgrees (fvf : Nat) : Bool :=
  match fvfDecl fvf with
  | .ok e => declSize e == fvfSize fvf
  | .error _ => true

/-- P holds for every shaped FVF of position kind p -/
def overShaped (P : Nat → Bool) (p : Nat) : Bool :=
  (List.range 4).all fun b => (List.range 16).all fun fl =>
    (List.range 9).all fun n => P (shaped p b fl n (n % 4))

private theorem of_overShaped (P : Nat → Bool) (h : ∀ p < 8, overShaped P p = true) :
    ∀ p < 8, ∀ b < 4, ∀ fl < 16, ∀ n < 9, P (shaped p b fl n (n % 4)) = true := by
  intro p hp b hb fl hfl n hn
  have h := h p hp
  simp only [overShaped, List.all_eq_true, List.mem_range] at h
  exact h b hb fl hfl n hn

private theorem trip0 : overShaped (fun f => roundTrips f && sizeAgrees f) 0 = true := by decide +kernel
private theorem trip1 : overShaped (fun f => roundTrips f && sizeAgrees f) 1 = true := by decide +kernel
private theorem trip2 : overShaped (fun f => roundTrips f && sizeAgrees f) 2 = true := by decide +kernel
private theorem trip3 : overShaped (fun f => roundTrips f && sizeAgrees f) 3 = true := by decide +kernel
private theorem trip4 : overShaped (fun f => roundTrips f && sizeAgrees f) 4 = true := by decide +kernel
private theorem trip5 : overShaped (fun f => roundTrips f && sizeAgrees f) 5 = true := by decide +kernel
private theorem trip6 : overShaped (fun f => roundTrips f && sizeAgrees f) 6 = true := by decide +kernel
private theorem trip7 : overShaped (fun f => roundTrips f && sizeAgrees f) 7 = true := by decide +kernel

/-- D3D9.FVFDECL, DECLFVF, FVFSIZE: over every FVF of one position kind, LASTBETA flags, the
normal, point size and colors and up to 8 texture coordinate sets: the declaration
goes back to an FVF of the same declaration, and its size is D3DXGetFVFVertexSize's (the sets'
size code n mod 4: every size, mixed sizes in the mirrors) -/
theorem fvf_round_trip : ∀ p < 8, ∀ b < 4, ∀ fl < 16, ∀ n < 9,
    (roundTrips (shaped p b fl n (n % 4)) && sizeAgrees (shaped p b fl n (n % 4))) = true :=
  of_overShaped (fun f => roundTrips f && sizeAgrees f) fun p hp => by
    match p, hp with
    | 0, _ => exact trip0
    | 1, _ => exact trip1
    | 2, _ => exact trip2
    | 3, _ => exact trip3
    | 4, _ => exact trip4
    | 5, _ => exact trip5
    | 6, _ => exact trip6
    | 7, _ => exact trip7

/-- the same answer -/
def eqR : Except Unit (Option (List (Nat × Nat))) → Except Unit (Option (List (Nat × Nat))) → Bool
  | .ok a, .ok b => a == b
  | .error _, .error _ => true
  | _, _ => false

/-- D3D9.FVF.DECL, TEXCOUNT, BLEND5 of one FVF: FVF 0 none; the reserved bit 0x1 changes nothing;
past 8 sets, the declaration of 8 sets and then FLOAT2 TEXCOORDs of the next indices packed after it;
five weights and no indices (D3DX refuses) the position, a FLOAT4 BLENDWEIGHT and a FLOAT1
BLENDINDICES after it; otherwise D3DXDeclaratorFromFVF's declaration -/
def runtimeOk (f : Nat) : Bool :=
  let r := runtimeDecl f
  if f = 0 then eqR r (.ok none) else
  eqR (runtimeDecl (f ||| 1)) r &&
  if sets f > 8 then
    match runtimeDecl (clr f 0xf00 ||| 0x800) with
    | .ok (some d) => eqR r (.ok (some (d ++ (List.range (sets f - 8)).map fun k => elem (declSize d + 8 * k) 1 5 (8 + k))))
    | _ => false
  else if weights f > 4 then
    (match fvfDecl f with | .error _ => true | .ok _ => false) &&
    (match r with
      | .ok (some e) => e[0]? == some (elem 0 2 0 0) && e[1]? == some (elem 12 3 1 0) && e[2]? == some (elem 28 0 2 0)
      | _ => false)
  else eqR r (.ok (fvfDecl f).toOption)

/-- runtimeOk over the shaped FVFs of position kind p, to 15 sets, the colors and normal in four
mixes -/
def overRuntime (p : Nat) : Bool :=
  (List.range 4).all fun b => [0, 5, 10, 15].all fun fl =>
    (List.range 16).all fun n => runtimeOk (shaped p b fl n (n % 4))

private theorem rt0 : overRuntime 0 = true := by decide +kernel
private theorem rt1 : overRuntime 1 = true := by decide +kernel
private theorem rt2 : overRuntime 2 = true := by decide +kernel
private theorem rt3 : overRuntime 3 = true := by decide +kernel
private theorem rt4 : overRuntime 4 = true := by decide +kernel
private theorem rt5 : overRuntime 5 = true := by decide +kernel
private theorem rt6 : overRuntime 6 = true := by decide +kernel
private theorem rt7 : overRuntime 7 = true := by decide +kernel

/-- D3D9.FVF.DECL, TEXCOUNT, BLEND5: the declaration SetFVF makes is D3DXDeclaratorFromFVF's
where D3DX makes one; past 8 sets FLOAT2 TEXCOORDs of the next indices follow; five weights and
no indices are a FLOAT4 BLENDWEIGHT and a FLOAT1 BLENDINDICES; the reserved bit 0x1 is
ignored (every position kind, LASTBETA flag and count of sets, the colors and normal in four
mixes) -/
theorem runtime_decl : ∀ p < 8, ∀ b < 4, ∀ fl ∈ [0, 5, 10, 15], ∀ n < 16, runtimeOk (shaped p b fl n (n % 4)) = true := by
  intro p hp b hb fl hfl n hn
  have h : overRuntime p = true := by
    match p, hp with
    | 0, _ => exact rt0
    | 1, _ => exact rt1
    | 2, _ => exact rt2
    | 3, _ => exact rt3
    | 4, _ => exact rt4
    | 5, _ => exact rt5
    | 6, _ => exact rt6
    | 7, _ => exact rt7
  simp only [overRuntime, List.all_eq_true, List.mem_range] at h
  exact h b hb fl hfl n hn

/-- the pairs flattened -/
def flat (e : List (Nat × Nat)) : List Int := e.flatMap fun x => [(x.1 : Int), x.2]

/-- the arguments as pairs -/
def pairs : List Int → Option (List (Nat × Nat))
  | [] => some []
  | a :: b :: r => do pure ((← nat a, ← nat b) :: (← pairs r))
  | [_] => none

def mirrors : List Mirror := [
  ⟨"d3dx9.fvfdecl", fun
    | [f] => do
      match fvfDecl (← nat f) with
      | .ok e => pure (0 :: flat e)
      | .error c => pure [1, c]
    | _ => none⟩,
  ⟨"d3dx9.declfvf", fun
    | args => do
      match declFvf (← pairs args) with
      | .ok f => pure [0, f]
      | .error c => pure [1, c]⟩,
  ⟨"d3dx9.fvfsize", fun | [f] => do pure [fvfSize (← nat f)] | _ => none⟩,
  ⟨"d3d9.fvfdecl", fun
    | [f] => do
      match runtimeDecl (← nat f) with
      | .ok (some e) => pure (0 :: flat e)
      | .ok none => pure []
      | .error () => pure [-1]
    | _ => none⟩
]

end Kikizake.D3d9.Fvf
