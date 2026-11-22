import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's texture stages as a draw runs them, as src/model/d3d9draw.rs `stage_plan`
orders them (`plan`; D3D9.STAGE.DISABLE, STAGE.MULTI, STAGE.BUMPCHAIN, STAGE.ALPHADISABLE.FIRST,
STAGE.ALPHADISABLE), with what it reads of the stages, `readsTexture` `reads_texture` and
`readsCurrent` `reads_current`; and the bump mapping arithmetic (D3D9.STAGE.BUMP) on binary64
and binary32 bits: `bumpUv` `bump_uv`, `bumpLuminance` `bump_luminance`, `bumpScale`
`bump_scale`, `premodulate` `premodulate` (mirrors `d3d9.plan`, `d3d9.reads`, `d3d9.bump`). -/
namespace Kikizake.D3d9.Chain

/-- what the plan reads of a stage: COLOROP, ALPHAOP, a texture, its operations reading it, its
color operation reading the diffuse color or CURRENT -/
structure Desc where
  cop : Nat
  aop : Nat
  hasTex : Bool
  readsTex : Bool
  readsCur : Bool

/-- a running stage's alpha: by an alpha operation, or under ALPHAOP DISABLE the alpha given, 1 -/
inductive Alpha where
  | op (n : Nat) | keep | one
  deriving DecidableEq, Repr

/-- a stage of the draw (moved: its texture coordinates moved by the bump mapping stage before) -/
inductive Step where
  | run (st : Nat) (moved : Bool) (alpha : Alpha) (premod : Bool)
  | defer (st : Nat) (moved : Bool)
  | bumpLast (st : Nat) (moved : Bool) (src : Option Nat)
  | off (st : Nat) (moved : Bool) (s : Nat)
  deriving DecidableEq, Repr

def Step.st : Step → Nat
  | .run st .. | .defer st .. | .bumpLast st .. | .off st .. => st

/-- BUMPENVMAP, BUMPENVMAPLUMINANCE -/
def isBump (c : Nat) : Bool := c == 22 || c == 23

/-- a stage reading the texture it lacks -/
def lacking (x : Desc) : Bool := !x.hasTex && x.readsTex

/-- the next stage is enabled, has a texture -/
def nextOn : List Desc → Bool | n :: _ => n.cop != 1 | [] => false
def nextTex : List Desc → Bool | n :: _ => n.hasTex | [] => false

/-- a bump mapping stage followed by an enabled one: the stages off from the first before it
lacking its texture, else it writes nothing -/
def chained (pre : List Desc) (moved : Bool) (tail : List Step) : List Step :=
  match pre.findIdx? lacking with
  | some s => [.off pre.length moved s]
  | none => .defer pre.length moved :: tail

/-- a running stage's alpha -/
def alphaOf (st : Nat) (x : Desc) : Alpha :=
  if x.aop != 1 then .op x.aop else if x.cop = 24 then .op 2
  else if 0 < st then .keep else if x.readsCur then .keep else .one

/-- the stages from the one after `pre` (the stages before, all enabled) -/
def go (pre : List Desc) (moved : Bool) : List Desc → List Step
  | [] => []
  | x :: rest =>
    if x.cop = 1 then []
    else if isBump x.cop then
      if nextOn rest then chained pre moved (go (pre ++ [x]) true rest)
      else .bumpLast pre.length moved (if pre.length = 0 then none else some (pre.length - 1)) :: go (pre ++ [x]) true rest
    else .run pre.length moved (alphaOf pre.length x) ((x.cop == 17 || x.aop == 17) && nextOn rest && nextTex rest) ::
      go (pre ++ [x]) false rest

/-- the stages of a draw, stage 0's first -/
def plan (ds : List Desc) : List Step := go [] false ds

/-- D3D9.STAGE.DISABLE: COLOROP DISABLE on stage 0 runs no stage, the diffuse color going through
untouched (CURRENT begins as it) -/
theorem disable (x : Desc) (rest : List Desc) (h : x.cop = 1) : plan (x :: rest) = [] := by
  simp [plan, go, h]

/-- a stage's step: none at a DISABLE, a last one turning the stages off, or one of its own number
followed by the next stage's -/
private theorem go_cons (pre : List Desc) (m : Bool) (x : Desc) (rest : List Desc) :
    (x.cop = 1 ∧ go pre m (x :: rest) = []) ∨
    (x.cop ≠ 1 ∧ ∃ b s, go pre m (x :: rest) = [.off pre.length b s]) ∨
    (x.cop ≠ 1 ∧ ∃ s m', s.st = pre.length ∧ (∀ a b c, s ≠ .off a b c) ∧ go pre m (x :: rest) = s :: go (pre ++ [x]) m' rest) := by
  by_cases h1 : x.cop = 1
  · exact .inl ⟨h1, by simp [go, h1]⟩
  · by_cases hb : isBump x.cop = true
    · by_cases hn : nextOn rest = true
      · cases hf : pre.findIdx? lacking with
        | some s => exact .inr (.inl ⟨h1, m, s, by simp [go, h1, hb, hn, chained, hf]⟩)
        | none => exact .inr (.inr ⟨h1, .defer pre.length m, true, rfl, by simp, by simp [go, h1, hb, hn, chained, hf]⟩)
      · exact .inr (.inr ⟨h1, .bumpLast pre.length m (if pre.length = 0 then none else some (pre.length - 1)), true, rfl,
          by simp, by simp [go, h1, hb, hn]⟩)
    · exact .inr (.inr ⟨h1, .run pre.length m (alphaOf pre.length x) ((x.cop == 17 || x.aop == 17) && nextOn rest && nextTex rest),
        false, rfl, by simp, by simp [go, h1, hb]⟩)

private theorem go_stages (ds pre : List Desc) (m : Bool) :
    ∀ i s, (go pre m ds)[i]? = some s → s.st = pre.length + i := by
  induction ds generalizing pre m with
  | nil => simp [go]
  | cons x rest ih =>
    intro i s hs
    rcases go_cons pre m x rest with ⟨_, h⟩ | ⟨_, b, c, h⟩ | ⟨_, t, m', ht, _, h⟩ <;> rw [h] at hs
    · simp at hs
    · cases i with
      | zero => simp at hs; subst hs; simp [Step.st]
      | succ j => simp at hs
    · cases i with
      | zero => simp at hs; subst hs; simp [ht]
      | succ j => have := ih _ _ j s (by simpa using hs); simp at this; omega

private theorem go_length (ds pre : List Desc) (m : Bool) :
    (go pre m ds).length ≤ (ds.takeWhile (·.cop != 1)).length ∧
    ((∀ s ∈ go pre m ds, ∀ a b c, s ≠ .off a b c) → (go pre m ds).length = (ds.takeWhile (·.cop != 1)).length) := by
  induction ds generalizing pre m with
  | nil => simp [go]
  | cons x rest ih =>
    rcases go_cons pre m x rest with ⟨h1, h⟩ | ⟨h1, b, c, h⟩ | ⟨h1, t, m', _, _, h⟩ <;> rw [h]
    · simp [h1]
    · have hx : (x.cop != 1) = true := by simpa using h1
      simp only [List.takeWhile_cons, hx, ↓reduceIte, List.length_cons]
      exact ⟨by simp, fun hn => absurd rfl (hn _ (by simp) pre.length b c)⟩
    · have hx : (x.cop != 1) = true := by simpa using h1
      simp only [List.takeWhile_cons, hx, ↓reduceIte, List.length_cons]
      obtain ⟨a1, a2⟩ := ih (pre ++ [x]) m'
      exact ⟨by omega, fun hn => by rw [a2 (fun s hs => hn s (by simp [hs]))]⟩

/-- D3D9.STAGE.MULTI: the stages run in turn, stage i the plan's i-th, none at or past the first
whose COLOROP is DISABLE, and (no bump mapping chain turning them off) every one before it -/
theorem multi (ds : List Desc) :
    (∀ i (s : Step), (plan ds)[i]? = some s → s.st = i) ∧ (plan ds).length ≤ (ds.takeWhile (·.cop != 1)).length ∧
    ((∀ s ∈ plan ds, ∀ a b c, s ≠ .off a b c) → (plan ds).length = (ds.takeWhile (·.cop != 1)).length) := by
  refine ⟨fun i s h => by simpa using go_stages ds [] false i s h, (go_length ds [] false).1, (go_length ds [] false).2⟩

/-- D3D9.STAGE.BUMPCHAIN: a bump mapping stage followed by an enabled stage writes no result
(the next stage's coordinates moved by it), but where a stage before it reads the texture it
lacks, the first such turns the stages off from it -/
theorem bump_chain (pre : List Desc) (m : Bool) (x n : Desc) (rest : List Desc)
    (hb : isBump x.cop = true) (hn : n.cop ≠ 1) :
    (pre.findIdx? lacking = none → go pre m (x :: n :: rest) = .defer pre.length m :: go (pre ++ [x]) true (n :: rest)) ∧
    (∀ s, pre.findIdx? lacking = some s → go pre m (x :: n :: rest) = [.off pre.length m s] ∧
      s < pre.length ∧ pre[s]?.map lacking = some true ∧ ∀ k < s, pre[k]?.map lacking = some false) := by
  have hx : x.cop ≠ 1 := by simp [isBump] at hb; omega
  have hn' : nextOn (n :: rest) = true := by simpa [nextOn] using hn
  refine ⟨fun h => by simp [go, hx, hb, hn', chained, h], fun s h => ⟨by simp [go, hx, hb, hn', chained, h], ?_⟩⟩
  have hs := List.findIdx?_eq_some_iff_getElem.1 h
  obtain ⟨hlt, hl, hk⟩ := hs
  refine ⟨hlt, ?_, fun k hk2 => ?_⟩
  · simp [List.getElem?_eq_getElem hlt, hl]
  · have := hk k hk2
    simp [List.getElem?_eq_getElem (Nat.lt_trans hk2 hlt), this]

/-- D3D9.STAGE.BUMP: a bump mapping stage that is the last gives the texture the stage before it
sampled (none on stage 0: 0), and the stage after a bump mapping stage has its coordinates moved -/
theorem bump_last (pre : List Desc) (m : Bool) (x : Desc) (rest : List Desc) (hb : isBump x.cop = true)
    (hn : ∀ n ∈ rest.head?, n.cop = 1) :
    go pre m (x :: rest) = .bumpLast pre.length m (if pre = [] then none else some (pre.length - 1)) :: go (pre ++ [x]) true rest := by
  have hx : x.cop ≠ 1 := by simp [isBump] at hb; omega
  have hn' : nextOn rest = false := by
    cases rest with
    | nil => rfl
    | cons n r => simp [nextOn, hn n (by simp)]
  simp [go, hx, hb, hn']

/-- D3D9.STAGE.ALPHADISABLE.FIRST: ALPHAOP DISABLE under an enabled COLOROP on stage 0 keeps the
alpha the stage is given where its color operation reads the diffuse color or CURRENT, else 1;
DOTPRODUCT3 writes its own (its operation's) -/
theorem alpha_disable_first (x : Desc) (rest : List Desc) (hc : x.cop ≠ 1) (hb : isBump x.cop = false) (ha : x.aop = 1) :
    ∃ p, plan (x :: rest) = .run 0 false (if x.cop = 24 then .op 2 else if x.readsCur then .keep else .one) p :: go [x] false rest := by
  refine ⟨(x.cop == 17 || x.aop == 17) && nextOn rest && nextTex rest, ?_⟩
  simp only [plan, go, hc, hb, ↓reduceIte, Bool.false_eq_true, List.length_nil, List.nil_append]
  by_cases h24 : x.cop = 24 <;> simp [alphaOf, ha, h24]

/-- D3D9.STAGE.ALPHADISABLE: ALPHAOP DISABLE under an enabled COLOROP on a later stage keeps
CURRENT's alpha (DOTPRODUCT3 writes its own) -/
theorem alpha_disable (pre : List Desc) (m : Bool) (x : Desc) (rest : List Desc) (hp : pre ≠ [])
    (hc : x.cop ≠ 1) (hb : isBump x.cop = false) (ha : x.aop = 1) (h24 : x.cop ≠ 24) :
    ∃ p, go pre m (x :: rest) = .run pre.length m .keep p :: go (pre ++ [x]) false rest := by
  have : 0 < pre.length := List.length_pos_iff.2 hp
  exact ⟨(x.cop == 17 || x.aop == 17) && nextOn rest && nextTex rest, by simp [go, hc, hb, alphaOf, ha, h24, this]⟩

/-- a stage's operations read its texture: an argument used of D3DTA_TEXTURE (COLORARG1, 2, 0,
ALPHAARG1, 2, 0), or BLENDTEXTUREALPHA(PM); the alpha operation not under ALPHAOP DISABLE -/
def readsTexture (cop aop c1 c2 c0 a1 a2 a0 : Nat) : Bool :=
  let r (op s1 s2 s0 : Nat) : Bool := (op != 3 && s1 % 16 == 2) || (op != 2 && s2 % 16 == 2) ||
    ((op == 25 || op == 26) && s0 % 16 == 2) || op == 13 || op == 14
  r cop c1 c2 c0 || (aop != 1 && r aop a1 a2 a0)

/-- a color operation reads the diffuse color or CURRENT: an argument used of either, or
BLENDDIFFUSEALPHA, BLENDCURRENTALPHA -/
def readsCurrent (cop c1 c2 c0 : Nat) : Bool :=
  let s (a : Nat) : Bool := a % 16 == 0 || a % 16 == 1
  (cop != 3 && s c1) || (cop != 2 && s c2) || ((cop == 25 || cop == 26) && s c0) || cop == 12 || cop == 16

/-- D3D9.STAGE.ALPHADISABLE.FIRST: SELECTARG1 reads only its first argument, SELECTARG2 its second;
BLENDDIFFUSEALPHA and BLENDCURRENTALPHA read the given alpha whatever their arguments -/
theorem reads_args (c1 c2 c0 : Nat) :
    readsCurrent 2 c1 c2 c0 = (c1 % 16 == 0 || c1 % 16 == 1) ∧ readsCurrent 3 c1 c2 c0 = (c2 % 16 == 0 || c2 % 16 == 1) ∧
    readsCurrent 12 c1 c2 c0 = true ∧ readsCurrent 16 c1 c2 c0 = true ∧
    readsTexture 2 1 c1 c2 c0 0 0 0 = (c1 % 16 == 2) ∧ readsTexture 13 1 0 0 0 0 0 0 = true := by
  simp [readsCurrent, readsTexture]

def one64 : Nat := 0x3ff0000000000000

/-- binary32 rounded to an integer, halves to even (Rust `round_ties_even`) -/
def rint (x : Nat) : Nat :=
  if F32.isNaN x || Float.isInf F32.fmt x then x else
  let (s, n, d) := Float.toRat F32.fmt x
  let q := n / d
  let r := n % d
  let m := if 2 * r > d ∨ (2 * r = d ∧ q % 2 = 1) then q + 1 else q
  Float.round F32.fmt s m 1

/-- a bump mapped stage's coordinates: (u + (M00 du + M10 dv), v + (M01 du + M11 dv)), binary64 -/
def bumpUv (u v m00 m01 m10 m11 du dv : Nat) : Nat × Nat :=
  (F64.add u (F64.add (F64.mul m00 du) (F64.mul m10 dv)), F64.add v (F64.add (F64.mul m01 du) (F64.mul m11 dv)))

/-- LUMINANCE's factor L LSCALE + LOFFSET clamped to 0..1, binary64 -/
def bumpLuminance (l s o : Nat) : Nat := F64.clamp (F64.add (F64.mul l s) o) 0 one64

/-- a color channel times the factor in binary32, rounded to the nearest of 255 (halves to even) -/
def bumpScale (c l : Nat) : Nat :=
  F64.ofF32 (F32.div (rint (F32.mul (F32.mul (F64.toF32 c) (F64.toF32 l)) 0x437f0000)) 0x437f0000)

/-- PREMODULATE's channel times the next stage's texture, binary32 -/
def premodulate (a t : Nat) : Nat := F64.ofF32 (F32.mul (F64.toF32 a) (F64.toF32 t))

/-- D3D9.STAGE.BUMP: the coordinates are moved by BUMPENVMAT times (du, dv), u by M00 du + M10 dv
and v by M01 du + M11 dv, each sum added last; LUMINANCE's factor is L LSCALE + LOFFSET clamped
to 0..1 (0 or 1 where outside, a NaN as it is); the luminance scale and PREMODULATE in binary32,
the scaled channel a whole number of 255ths -/
theorem bump (u v m00 m01 m10 m11 du dv l s o c f t : Nat) :
    bumpUv u v m00 m01 m10 m11 du dv = (F64.add u (F64.add (F64.mul m00 du) (F64.mul m10 dv)), F64.add v (F64.add (F64.mul m01 du) (F64.mul m11 dv))) ∧
    (bumpLuminance l s o = 0 ∨ bumpLuminance l s o = one64 ∨ bumpLuminance l s o = F64.add (F64.mul l s) o) ∧
    (F64.lt (F64.add (F64.mul l s) o) 0 = true → Float.isNaN F64.fmt (F64.add (F64.mul l s) o) = false → bumpLuminance l s o = 0) ∧
    bumpScale c f = F64.ofF32 (F32.div (rint (F32.mul (F32.mul (F64.toF32 c) (F64.toF32 f)) 0x437f0000)) 0x437f0000) ∧
    premodulate c t = F64.ofF32 (F32.mul (F64.toF32 c) (F64.toF32 t)) := by
  refine ⟨rfl, ?_, fun h hn => ?_, rfl, rfl⟩
  · simp only [bumpLuminance, F64.clamp, Float.clamp]
    split
    · exact .inr (.inr rfl)
    · split
      · exact .inl rfl
      · split
        · exact .inr (.inl rfl)
        · exact .inr (.inr rfl)
  · simp only [bumpLuminance, F64.clamp, Float.clamp]
    simp [hn, show F64.lt _ _ = true from h]

def desc (l : List Int) (i : Nat) : Option Desc := do
  pure ⟨← nat (← l[i]?), ← nat (← l[i + 1]?), (← l[i + 2]?) != 0, (← l[i + 3]?) != 0, (← l[i + 4]?) != 0⟩

def descs : Nat → List Int → Option (List Desc)
  | 0, _ => some []
  | k + 1, l => do pure ((← desc l 0) :: (← descs k (l.drop 5)))

def code : Step → List Int
  | .run st m a p => [0, st, if m then 1 else 0, (match a with | .op n => n | .keep => -1 | .one => -2), if p then 1 else 0]
  | .defer st m => [1, st, if m then 1 else 0, 0, 0]
  | .bumpLast st m f => [2, st, if m then 1 else 0, (match f with | some s => s | none => -1), 0]
  | .off st m s => [3, st, if m then 1 else 0, s, 0]

def mirrors : List Mirror := [
  ⟨"d3d9.plan", fun l => do
    let ds ← descs (l.length / 5) l
    pure ((plan ds).flatMap code)⟩,
  ⟨"d3d9.reads", fun
    | [cop, aop, c1, c2, c0, a1, a2, a0] => do
      let n := fun x => nat x
      pure [if readsTexture (← n cop) (← n aop) (← n c1) (← n c2) (← n c0) (← n a1) (← n a2) (← n a0) then 1 else 0,
        if readsCurrent (← n cop) (← n c1) (← n c2) (← n c0) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d9.bump", fun
    | [u1, u2, v1, v2, a1, a2, b1, b2, c1, c2, e1, e2, d1, d2, f1, f2, l1, l2, s1, s2, o1, o2, k1, k2, t1, t2] => do
      let j := fun (h w : Int) => do pure (F64.join (← nat h) (← nat w))
      let r := bumpUv (← j u1 u2) (← j v1 v2) (← j a1 a2) (← j b1 b2) (← j c1 c2) (← j e1 e2) (← j d1 d2) (← j f1 f2)
      let lum := bumpLuminance (← j l1 l2) (← j s1 s2) (← j o1 o2)
      let sc := bumpScale (← j k1 k2) lum
      let pm := premodulate (← j k1 k2) (← j t1 t2)
      pure ([r.1, r.2, lum, sc, pm].flatMap fun x => F64.halves (F64.canon x))
    | _ => none⟩
]

end Kikizake.D3d9.Chain
