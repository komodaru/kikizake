import Kikizake.Mirror
/-! Direct3D 9's per-pixel operations (spec/d3d9draw.txt), as src/model/d3d9draw.rs computes them:
`cmp` is `cmp` (D3DCMPFUNC: the alpha, stencil and depth tests), `stencilOp` `stencil_op`,
`stencilWrite` `stencil_write`, `address` `address` (D3DTEXTUREADDRESS), `blend8` `blend8` (the
8-bit blend of one channel) (mirrors `d3d9.cmp`, `d3d9.stencil`, `d3d9.address`, `d3d9.blend8`). -/
namespace Kikizake.D3d9.Pixel

/-- D3DCMPFUNC: NEVER, LESS, EQUAL, LESSEQUAL, GREATER, NOTEQUAL, GREATEREQUAL, ALWAYS (others
as ALWAYS) -/
def cmp (f a b : Nat) : Bool :=
  match f with
  | 1 => false
  | 2 => a < b
  | 3 => a == b
  | 4 => a ≤ b
  | 5 => a > b
  | 6 => a != b
  | 7 => a ≥ b
  | _ => true

/-- D3D9.ALPHATEST, STENCIL, DEPTH: the functions pair up as complements (LESS and GREATEREQUAL,
EQUAL and NOTEQUAL, LESSEQUAL and GREATER, NEVER and ALWAYS); exactly one of LESS, EQUAL,
GREATER holds; swapping the arguments turns LESS into GREATER, LESSEQUAL into GREATEREQUAL -/
theorem cmp_pairs (a b : Nat) :
    cmp 2 a b = !cmp 7 a b ∧ cmp 3 a b = !cmp 6 a b ∧ cmp 4 a b = !cmp 5 a b ∧ cmp 1 a b = !cmp 8 a b ∧
    ((cmp 2 a b && cmp 3 a b) = false ∧ (cmp 3 a b && cmp 5 a b) = false ∧ (cmp 2 a b && cmp 5 a b) = false) ∧
    (cmp 2 a b || cmp 3 a b || cmp 5 a b) = true ∧ cmp 2 a b = cmp 5 b a ∧ cmp 4 a b = cmp 7 b a := by
  simp only [cmp]
  rcases Nat.lt_trichotomy a b with h | h | h
  · have : ¬b < a := by omega
    have : a ≤ b := by omega
    have : ¬b ≤ a := by omega
    have : a ≠ b := by omega
    have : b ≠ a := by omega
    simp [*, bne]
  · subst h; simp
  · have : ¬a < b := by omega
    have : b ≤ a := by omega
    have : ¬a ≤ b := by omega
    have : a ≠ b := by omega
    have : b ≠ a := by omega
    simp [*, bne]

/-- D3DSTENCILOP on an 8-bit value: KEEP, ZERO, REPLACE, INCRSAT, DECRSAT, INVERT, INCR, DECR -/
def stencilOp (op s r : Nat) : Nat :=
  match op with
  | 2 => 0
  | 3 => r
  | 4 => min (s + 1) 255
  | 5 => s - 1
  | 6 => 255 - s
  | 7 => (s + 1) % 256
  | 8 => (s + 255) % 256
  | _ => s

/-- D3D9.STENCIL: every operation keeps an 8-bit value 8-bit; INCRSAT and DECRSAT clamp (never
past 255 or below 0, moving by one otherwise), INCR and DECR wrap and undo each other, INVERT
undoes itself -/
theorem stencil_ops (s r : Nat) (hs : s < 256) (hr : r < 256) :
    (∀ op, stencilOp op s r < 256) ∧ stencilOp 4 s r = (if s = 255 then 255 else s + 1) ∧
    stencilOp 5 s r = (if s = 0 then 0 else s - 1) ∧ stencilOp 8 (stencilOp 7 s r) r = s ∧
    stencilOp 7 (stencilOp 8 s r) r = s ∧ stencilOp 6 (stencilOp 6 s r) r = s ∧ stencilOp 7 255 r = 0 ∧
    stencilOp 8 0 r = 255 := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro op
    unfold stencilOp
    split <;> omega
  all_goals first | (simp only [stencilOp]; done) | (simp only [stencilOp]; split <;> omega) | (simp only [stencilOp]; omega)

/-- D3D9.STENCIL: the value stored through the write mask -/
def stencilWrite (s ns w : Nat) : Nat := (s &&& (255 - w)) ||| (ns &&& w)

private theorem not8 : ∀ w, w < 256 → 255 - w = 255 ^^^ w := by decide +kernel

/-- D3D9.STENCIL: through the write mask, each of the 8 bits is the new value's where the mask
has it, the old value's elsewhere -/
theorem stencil_write_bits (s ns w : Nat) (hw : w < 256) (i : Nat) (hi : i < 8) :
    (stencilWrite s ns w).testBit i = if w.testBit i then ns.testBit i else s.testBit i := by
  have ones : (255 : Nat).testBit i = true := by revert i; decide
  unfold stencilWrite
  rw [not8 w hw, Nat.testBit_or, Nat.testBit_and, Nat.testBit_and, Nat.testBit_xor, ones]
  cases w.testBit i <;> simp

/-- D3DTEXTUREADDRESS: a texel index into 0..n (none: the border) -/
def address (mode : Nat) (i n : Int) : Option Int :=
  match mode with
  | 1 => some (i % n)
  | 2 => let k := i % (2 * n); some (if k < n then k else 2 * n - 1 - k)
  | 3 => some (max 0 (min i (n - 1)))
  | 4 => if 0 ≤ i ∧ i < n then some i else none
  | _ => some (max 0 (min (if i < 0 then -i - 1 else i) (n - 1)))

/-- D3D9.TEX.ADDRESS, ADDRESS.BAD: every mode but BORDER gives a texel of the texture; BORDER
exactly inside it -/
theorem address_inside (mode : Nat) (i n : Int) (hn : 0 < n) :
    (∀ t, address mode i n = some t → 0 ≤ t ∧ t < n) ∧ (mode ≠ 4 → address mode i n ≠ none) ∧
    (address 4 i n = none ↔ i < 0 ∨ n ≤ i) := by
  have e1 := Int.emod_nonneg i (by omega : n ≠ 0)
  have e2 := Int.emod_lt_of_pos i hn
  have f1 := Int.emod_nonneg i (by omega : 2 * n ≠ 0)
  have f2 := Int.emod_lt_of_pos i (by omega : 0 < 2 * n)
  refine ⟨?_, ?_, ?_⟩
  · intro t h
    unfold address at h
    split at h <;> (try split at h) <;> simp_all <;> (try split at h) <;> omega
  · intro hm
    unfold address
    split <;> simp_all
  · unfold address
    split <;> simp_all <;> omega

/-- D3D9.TEX.ADDRESS: WRAP repeats every n texels; MIRROR every 2n, and reflects about the
texture's edge (texel -1 - i is texel i); CLAMP is the identity inside; MIRRORONCE reflects once
(as MIRROR between -n and n) then clamps -/
theorem address_modes (i n : Int) (hn : 0 < n) :
    address 1 (i + n) n = address 1 i n ∧ address 2 (i + 2 * n) n = address 2 i n ∧
    address 2 (-1 - i) n = address 2 i n ∧ (0 ≤ i → i < n → address 3 i n = some i) ∧
    (-n ≤ i → i < n → address 5 i n = address 2 i n) := by
  have reflect : (-1 - i) % (2 * n) = 2 * n - 1 - i % (2 * n) := by
    have q := Int.emod_add_mul_ediv i (2 * n)
    have f1 := Int.emod_nonneg i (by omega : 2 * n ≠ 0)
    have f2 := Int.emod_lt_of_pos i (by omega : 0 < 2 * n)
    have : -1 - i = (2 * n - 1 - i % (2 * n)) + (2 * n) * (-(i / (2 * n)) - 1) := by
      rw [Int.mul_sub, Int.mul_neg]; omega
    rw [this, Int.add_mul_emod_self_left, Int.emod_eq_of_lt (by omega) (by omega)]
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · simp [address, Int.add_emod_right]
  · simp [address, Int.add_emod_right]
  · simp only [address, reflect]
    have f1 := Int.emod_nonneg i (by omega : 2 * n ≠ 0)
    have f2 := Int.emod_lt_of_pos i (by omega : 0 < 2 * n)
    split <;> split <;> simp <;> omega
  · intro a b
    simp only [address]
    congr 1
    omega
  · intro a b
    simp only [address]
    by_cases neg : i < 0
    · have : i % (2 * n) = i + 2 * n := by
        rw [show i = (i + 2 * n) + (2 * n) * (-1) by omega, Int.add_mul_emod_self_left]
        rw [Int.emod_eq_of_lt (by omega) (by omega)]
        omega
      rw [this]
      simp only [neg, ↓reduceIte]
      congr 1
      split <;> omega
    · have : i % (2 * n) = i := Int.emod_eq_of_lt (by omega) (by omega)
      rw [this]
      simp only [neg, ↓reduceIte]
      congr 1
      split <;> omega

/-- the blend operations -/
inductive Op where
  | add
  | sub
  | revsub
  | min
  | max

/-- a product x * f of 8-bit numbers in 1/256ths (ONE: x exactly) -/
def prod (x : Int) (one : Bool) (f : Int) : Int := if one then x * 256 else x * f * 257 / 256

/-- D3D9.BLEND, BLEND.TIE: one 8-bit channel: the source and destination, each with whether its
factor is ONE and the factor -/
def blend8 (op : Op) (x : Int) (xone : Bool) (fs : Int) (y : Int) (yone : Bool) (fd : Int) (exact : Bool) : Int :=
  let a := prod x xone fs
  let b := prod y yone fd
  let r (v e : Int) : Int := if exact then Min.min ((2 * Max.max e 0 + 255) / 510) 255 else Min.min ((Max.max v 0 + 128) / 256) 255
  match op with
  | .add => r (a + b) (x * fs + y * fd)
  | .sub => r (a - b) (x * fs - y * fd)
  | .revsub => r (b - a) (y * fd - x * fs)
  | .min => Min.min x y
  | .max => Max.max x y

/-- D3D9.BLEND: a product in 1/256ths is a little under x f / 255: at most it, less than one
1/256th under for 8-bit numbers -/
theorem prod_under (x f : Int) (hx : 0 ≤ x) (hf : 0 ≤ f) (hx' : x ≤ 255) (hf' : f ≤ 255) :
    prod x false f * 255 ≤ x * f * 256 ∧ x * f * 256 < (prod x false f + 2) * 255 := by
  simp only [prod, Bool.false_eq_true, ↓reduceIte]
  have h0 : 0 ≤ x * f := Int.mul_nonneg hx hf
  have h1 : x * f ≤ 65025 := by
    have := Int.mul_le_mul hx' hf' hf (by omega)
    omega
  omega

/-- D3D9.BLEND: the result is an 8-bit value; drawing with ONE and ZERO writes the source as
it is (both roundings); MIN and MAX take the smaller and the larger -/
theorem blend8_holds (op : Op) (x y fs fd : Int) (xo yo e : Bool) (hx : 0 ≤ x) (hx' : x ≤ 255) (hy : 0 ≤ y) (hy' : y ≤ 255) :
    (0 ≤ blend8 op x xo fs y yo fd e ∧ blend8 op x xo fs y yo fd e ≤ 255) ∧
    blend8 .add x true 255 y false 0 e = x ∧ blend8 .min x xo fs y yo fd e = Min.min x y ∧
    blend8 .max x xo fs y yo fd e = Max.max x y := by
  refine ⟨?_, ?_, rfl, rfl⟩
  · unfold blend8
    cases op <;> simp only <;> (try split) <;> omega
  · unfold blend8 prod
    cases e <;> simp <;> omega

/-- D3D9.BLEND.TIE: the exact rounding is the nearest 8-bit value to the sum in 255ths, halves up -/
theorem exact_nearest (x fs y fd : Int) (xo yo : Bool) (h0 : 0 ≤ x * fs + y * fd) (h1 : x * fs + y * fd ≤ 65025) :
    2 * (x * fs + y * fd) - 255 < 510 * blend8 .add x xo fs y yo fd true ∧
    510 * blend8 .add x xo fs y yo fd true ≤ 2 * (x * fs + y * fd) + 255 := by
  simp only [blend8, ↓reduceIte]
  generalize x * fs + y * fd = n at h0 h1 ⊢
  omega

def mirrors : List Mirror := [
  ⟨"d3d9.cmp", fun | [f, a, b] => do pure [if cmp (← nat f) (← nat a) (← nat b) then 1 else 0] | _ => none⟩,
  ⟨"d3d9.stencil", fun
    | [op, s, r, w] => do
      let ns := stencilOp (← nat op) (← nat s) (← nat r)
      pure [ns, stencilWrite (← nat s) ns (← nat w)]
    | _ => none⟩,
  ⟨"d3d9.address", fun | [m, i, n] => do pure ((address (← nat m) i n).toList) | _ => none⟩,
  ⟨"d3d9.blend8", fun
    | [op, x, xo, fs, y, yo, fd, e] => do
      let o ← match op with | 1 => some Op.add | 2 => some .sub | 3 => some .revsub | 4 => some .min | 5 => some .max | _ => none
      pure [blend8 o x (xo != 0) fs y (yo != 0) fd (e != 0)]
    | _ => none⟩
]

end Kikizake.D3d9.Pixel
