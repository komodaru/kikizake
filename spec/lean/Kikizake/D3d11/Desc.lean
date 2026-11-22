import Kikizake.Mirror
/-! The resource descriptions Direct3D 10 and 11 accept (spec/d3dres.txt D3DRES.DESC.*), as
src/model/d3dres_check.rs computes them: `check` is `check_with` (CreateTexture1D/2D/3D and
CreateBuffer's checks of a description whose format has the facts given: the format tables'
part is data from Windows' answers), `createRefused` `create_refused` (mirrors `d3d11.desc`,
`d3d11.create`). A refused description is `none` (E_INVALIDARG). -/
namespace Kikizake.D3d11.Desc

/-- D3D11_TEXTURE1D/2D/3D_DESC and D3D11_BUFFER_DESC as the model keeps them: dim 1..3 textures,
4 a buffer (w its bytes, q its stride; for textures q the sample quality) -/
structure Desc where
  dim : Nat
  w : Nat
  h : Nat
  da : Nat
  mips : Nat
  fmt : Nat
  usage : Nat
  bind : Nat
  cpu : Nat
  misc : Nat
  count : Nat
  q : Nat
  deriving DecidableEq, Repr

/-- the format's part: known or not, its typeless group's support bits, the bits binding goes
by, the UAV support bits, its element's width and height, the quality levels of 2, 4, 8 and 16
samples -/
structure Facts where
  known : Bool
  s : Nat
  bs : Nat
  uav : Nat
  bw : Nat
  bh : Nat
  q2 : Nat
  q4 : Nat
  q8 : Nat
  q16 : Nat

/-- Direct3D 10, 10.1 at a feature level, 11 at a feature level -/
inductive Dev where
  | d10
  | d101 (level : Nat)
  | d11 (level : Nat)
  deriving DecidableEq

def Dev.d3d11 : Dev → Bool
  | .d11 _ => true
  | _ => false

def Dev.level : Dev → Nat
  | .d10 => 0xa000
  | .d101 l => l
  | .d11 l => l

/-- the largest 1D and 2D size, 3D size, array size of a feature level -/
def limits (l : Nat) : Nat × Nat × Nat :=
  if 0xb000 ≤ l then (16384, 2048, 2048)
  else if 0xa000 ≤ l then (8192, 2048, 512)
  else if l = 0x9300 then (4096, 256, 1)
  else (2048, 256, 1)

/-- the 32-bit complement -/
def notU (m : Nat) : Nat := 0xffffffff ^^^ m

/-- some bit of m in x -/
def has (x m : Nat) : Bool := x &&& m != 0

def INVALIDARG : Nat := 0x80070057

/-- the levels of a full chain: the bits of the largest side -/
def fullChain (dim w h da : Nat) : Nat :=
  let m := if dim = 1 then w else if dim = 2 then max w h else max (max w h) da
  Nat.log2 (max m 1) + 1

/-- D3D11_USAGE (DEFAULT 0, IMMUTABLE 1, DYNAMIC 2, STAGING 3; 0xffffffff passes as staging) with
the CPU access and bind flags it takes -/
def usageOk (d : Desc) (init : Bool) : Bool :=
  match d.usage with
  | 0 => !(d.cpu != 0 && has d.bind 0x40)
  | 1 => init && d.cpu == 0 && d.bind != 0 && !has d.bind 0xf0
  | 2 => d.cpu == 0x10000 && d.bind != 0 && !has d.bind 0xf0
  | 3 => d.bind == 0 && d.cpu != 0
  | 0xffffffff => d.bind == 0 && d.cpu != 0
  | _ => false

/-- D3DRES.DESC.BUF: a buffer's description -/
def bufOk (dev : Dev) (d : Desc) : Bool :=
  let d11 := dev.d3d11
  let binds := if d11 then 0x4bf else 0x43f
  let raw := has d.misc 0x20 && d11
  let st := has d.misc 0x40 && d11
  let miscs := if d11 then 0x172 else 6
  let cube := if d11 then 0 else 4
  d.w != 0 && !has d.bind (notU binds) && !has d.misc (notU miscs) &&
  !(has d.bind 4 && (d.w % 16 != 0 || d.bind != 4 || has d.misc 0x10)) &&
  !(d.usage == 0 && d.cpu != 0 && (!d11 || has d.bind 0x17 || !has d.bind 0x88)) &&
  !(d.usage == 2 && (has d.bind 0xb0 || has d.misc (notU (0x60 ||| cube)))) &&
  !(d.usage == 1 && has d.bind 0x80) &&
  !(d.usage == 3 && has d.misc (notU (0x102 ||| cube))) &&
  !(raw && (st || !has d.bind 0x88 || has d.bind (notU 0xab))) &&
  !(st && (has d.bind (notU 0x488) || d.q == 0 || d.q % 4 != 0 || d.q > 2048 || d.w % d.q != 0))

/-- the buffer's description GetDesc gives: the stride 0 unless structured -/
def bufOut (dev : Dev) (d : Desc) : Desc :=
  { d with q := if has d.misc 0x40 && dev.d3d11 then d.q else 0,
           misc := if dev.d3d11 then d.misc else d.misc &&& notU 4 }

/-- the height a texture has (1 for 1D) -/
def height (d : Desc) : Nat := if d.dim = 1 then 1 else d.h

/-- the levels it ends up with -/
def levels (d : Desc) : Nat :=
  if d.mips = 0 then fullChain d.dim d.w (height d) d.da else d.mips

/-- the misc flags it ends up with (Direct3D 10 drops TEXTURECUBE from 1D and 3D) -/
def miscOut (dev : Dev) (d : Desc) : Nat :=
  if !dev.d3d11 && d.dim != 2 then d.misc &&& notU 4 else d.misc

/-- D3DRES.DESC.SIZE, BIG: the sizes -/
def sizeOk (dev : Dev) (d : Desc) : Bool :=
  let l2 := (limits dev.level).1
  let l3 := (limits dev.level).2.1
  let la := (limits dev.level).2.2
  let h := height d
  let lim := if d.dim = 3 then l3 else l2
  d.w != 0 && h != 0 && d.da != 0 && (d.dim == 3 || d.da ≤ max la 1) &&
  d.w < 2 ^ 31 && h < 2 ^ 31 && !(d.dim == 3 && d.da ≥ 2 ^ 31) &&
  (d.dim == 2 || !(d.w > lim || h > lim || (d.dim == 3 && d.da > l3))) &&
  (d.dim != 2 || !(d.w > lim || h > lim) || (d.da == 1 && d.mips == 1 && d.w ≤ 2 ^ 23 && h ≤ 2 ^ 23))

/-- what a texture's checks ask of its usage and misc flags: default, immutable, dynamic,
staging (or 0xffffffff); GENERATE_MIPS, TEXTURECUBE (as kept), GDI_COMPATIBLE,
RESTRICTED_CONTENT, a flag staging refuses, a flag the API does not know, a flag other than
TEXTURECUBE -/
structure Flags where
  u0 : Bool
  u1 : Bool
  u2 : Bool
  u3 : Bool
  gen : Bool
  cube : Bool
  gdi : Bool
  restricted : Bool
  noStaging : Bool
  unknown : Bool
  notCube : Bool

def flags (dev : Dev) (d : Desc) : Flags :=
  ⟨d.usage == 0, d.usage == 1, d.usage == 2, d.usage == 3 || d.usage == 0xffffffff,
   has d.misc 1, has (miscOut dev d) 4, has d.misc 0x200, has d.misc 0x1000, has d.misc 0x1081,
   has d.misc (notU (if dev.d3d11 then 0x1387 else 7)), has d.misc (notU 4)⟩

/-- D3DRES.DESC.MISC: the misc flags -/
def miscOk (dev : Dev) (d : Desc) (f : Flags) : Bool :=
  !f.unknown &&
  !(f.restricted && d.dim != 2) &&
  !(f.cube && (d.dim != 2 || d.da % 6 != 0 || d.w != height d || (dev == .d10 && d.da != 6))) &&
  !(f.gen && d.bind &&& 0x28 != 0x28) &&
  !(f.gdi && !(d.fmt == 87 || d.fmt == 90 || d.fmt == 91)) &&
  !(f.u3 && f.noStaging) &&
  !(f.u1 && f.restricted)

/-- the multisampling of a 2D texture -/
def msOk (dev : Dev) (d : Desc) (f : Flags) (init : Bool) (fx : Facts) : Bool :=
  d.count != 0 &&
  if d.count > 1 then
    let q := if d.count = 2 then fx.q2 else if d.count = 4 then fx.q4 else if d.count = 8 then fx.q8
      else if d.count = 16 then fx.q16 else 0
    let d10 := dev == .d10 && (d.q ≥ 0xfffffffe || d.bind &&& 0x48 == 0x48)
    !(d10 || q == 0 || q == INVALIDARG || (d.q ≥ q && d.q < 0xfffffffe) || levels d != 1 || !f.u0 || init)
  else !(d.q != 0 && d.q < 0xfffffffe && dev.d3d11)

/-- the texture checks of a description (its usage and misc as the flags f, not read from d) -/
def texCore (dev : Dev) (d : Desc) (f : Flags) (init : Bool) (fx : Facts) : Bool :=
  let s := fx.s
  let dimbit := if d.dim = 1 then 0x10 else if d.dim = 2 then 0x20 else if d.dim = 3 then 0x40 else 0
  let gm := levels d
  fx.known && has s dimbit && sizeOk dev d &&
  d.w % max fx.bw 1 == 0 && height d % max fx.bh 1 == 0 &&
  d.mips ≤ fullChain d.dim d.w (height d) d.da &&
  !(gm > 1 && !has s 0x1000) && !(f.cube && !has s 0x80) && !(d.cpu != 0 && !has s 0x20000) &&
  !has d.bind (notU 0xe8) &&
  (!has d.bind 8 || has fx.bs 0x100) && (!has d.bind 0x20 || has fx.bs 0x4000) &&
  (!has d.bind 0x40 || has fx.bs 0x10000) && (!has d.bind 0x80 || has fx.uav 0x2000000) &&
  !(has d.bind 0x40 && (has d.bind 0xa0 || d.dim == 3)) &&
  miscOk dev d f &&
  !(d.dim == 3 && f.u0 && d.cpu != 0 && gm != 1) &&
  !(f.u0 && d.cpu != 0 && d.count > 1) &&
  !(f.u2 && (gm != 1 || (!dev.d3d11 && d.mips != 1) || (d.dim != 3 && d.da != 1) || f.notCube)) &&
  (d.dim != 2 || msOk dev d f init fx)

/-- D3DRES.DESC: a texture's description -/
def texOk (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) : Bool :=
  texCore dev { d with usage := 0, misc := 0 } (flags dev d) init fx

private theorem strip_size (dev : Dev) (d : Desc) (u m : Nat) : sizeOk dev { d with usage := u, misc := m } = sizeOk dev d := rfl
private theorem strip_height (d : Desc) (u m : Nat) : height { d with usage := u, misc := m } = height d := rfl
private theorem strip_levels (d : Desc) (u m : Nat) : levels { d with usage := u, misc := m } = levels d := rfl

/-- D3DRES.DESC: what CreateTexture* and CreateBuffer give back for a description (none:
E_INVALIDARG); `init` whether initial data comes with it -/
def check (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) : Option Desc :=
  if has d.cpu (notU 0x30000) || !usageOk d init then none
  else if d.dim = 4 then (if bufOk dev d then some (bufOut dev d) else none)
  else if texOk dev d init fx then some { d with mips := levels d, misc := miscOut dev d }
  else none

/-- D3DRES.DESC.CPURTV: Direct3D 11 refuses at creation a 2D default texture the CPU reads and
renders to of more than one subresource -/
def createRefused (dev : Dev) (d : Desc) : Bool :=
  dev.d3d11 && d.dim == 2 && d.usage == 0 && has d.cpu 0x20000 && has d.bind 0x20 && (d.mips != 1 || d.da != 1)

/-- D3DRES.DESC.SIZE: a texture's size of 0x80000000 or more is refused (a 3D one's depth too) -/
theorem desc_size (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hd : d.dim = 1 ∨ d.dim = 2 ∨ d.dim = 3)
    (hw : 2 ^ 31 ≤ d.w ∨ (d.dim ≠ 1 ∧ 2 ^ 31 ≤ d.h) ∨ (d.dim = 3 ∧ 2 ^ 31 ≤ d.da)) :
    check dev d init fx = none := by
  have h4 : d.dim ≠ 4 := by omega
  have : sizeOk dev d = false := by
    simp only [sizeOk, height]
    rcases hw with h | ⟨h1, h⟩ | ⟨h1, h⟩
    · have : ¬d.w < 2 ^ 31 := by omega
      simp [this]
    · have : ¬d.h < 2 ^ 31 := by omega
      simp [this, h1]
    · simp [h1, h]
  simp [check, h4, texOk, texCore, strip_size, this]

/-- D3DRES.DESC.BIG: 1D textures stop at the level's size (16384 on 11_0, 8192 on 10_x), 3D ones
at 2048 a side (on 10_0 and later); a 2D texture past the level's size passes only with one
level and one slice, and never past 2^23 texels a side -/
theorem desc_big (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) :
    (d.dim = 1 → (limits dev.level).1 < d.w → check dev d init fx = none) ∧
    (d.dim = 3 → 0xa000 ≤ dev.level → (2048 < d.w ∨ 2048 < d.h ∨ 2048 < d.da) → check dev d init fx = none) ∧
    (d.dim = 2 → ((limits dev.level).1 < d.w ∨ (limits dev.level).1 < d.h) → (d.da ≠ 1 ∨ d.mips ≠ 1) → check dev d init fx = none) ∧
    (d.dim = 2 → (2 ^ 23 < d.w ∨ 2 ^ 23 < d.h) → check dev d init fx = none) := by
  have hl : ∀ l, 2048 ≤ (limits l).1 ∧ (limits l).1 ≤ 16384 := by
    intro l; unfold limits; split <;> (try split) <;> (try split) <;> simp
  have l3 : ∀ l, 0xa000 ≤ l → (limits l).2.1 = 2048 := by
    intro l h; unfold limits; split <;> (try split) <;> simp <;> omega
  have none4 : ∀ x : Desc, x.dim ≠ 4 → (sizeOk dev x = true → False) → check dev x init fx = none := by
    intro x hx hs
    have : sizeOk dev x = false := by cases h : sizeOk dev x <;> simp_all
    simp [check, hx, texOk, texCore, strip_size, this]
  have := hl dev.level
  refine ⟨fun h1 hw => none4 d (by omega) ?_, fun h1 hlv hw => none4 d (by omega) ?_,
    fun h1 hw hm => none4 d (by omega) ?_, fun h1 hw => none4 d (by omega) ?_⟩
  · intro hs; simp [sizeOk, height, h1] at hs; omega
  · have := l3 _ hlv
    intro hs; simp [sizeOk, height, h1] at hs; omega
  · intro hs; simp [sizeOk, height, h1] at hs; omega
  · intro hs; simp [sizeOk, height, h1] at hs; omega

/-- D3DRES.DESC.USAGEFF: a texture of usage 0xffffffff, bound nowhere and with CPU access, is
checked as a staging one (GetDesc giving its usage back) -/
theorem usage_ff (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4) (hb : d.bind = 0) :
    check dev { d with usage := 0xffffffff } init fx = (check dev { d with usage := 3 } init fx).map ({ · with usage := 0xffffffff }) := by
  have ht : texOk dev { d with usage := 0xffffffff } init fx = texOk dev { d with usage := 3 } init fx := by
    simp [texOk, flags, miscOut]
  have hu : usageOk { d with usage := 0xffffffff } init = usageOk { d with usage := 3 } init := by
    simp [usageOk, hb]
  unfold check
  rw [ht, hu]
  simp only [hd, ↓reduceIte]
  split
  · rfl
  · split
    · simp [levels, miscOut, height, fullChain]
    · rfl

/-- D3DRES.DESC.CUBE10: Direct3D 10 (and 10.1) drops TEXTURECUBE from a 1D or 3D description and
makes it as the description without the flag -/
theorem cube10 (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hv : dev.d3d11 = false) (hd : d.dim = 1 ∨ d.dim = 3) :
    check dev d init fx = check dev { d with misc := d.misc &&& notU 4 } init fx := by
  have h2 : d.dim ≠ 2 := by omega
  have h4 : d.dim ≠ 4 := by omega
  have ha : ∀ c, has (d.misc &&& notU 4) c = has d.misc (notU 4 &&& c) := by
    intro c; simp only [has, Nat.and_assoc]
  have hm : d.misc &&& notU 4 &&& notU 4 = d.misc &&& notU 4 := by rw [Nat.and_assoc]; rfl
  have mo : miscOut dev { d with misc := d.misc &&& notU 4 } = miscOut dev d := by
    simp [miscOut, hv, h2, hm]
  have hf : flags dev { d with misc := d.misc &&& notU 4 } = flags dev d := by
    simp only [flags, mo, ha, hv]
    rfl
  have ht : texOk dev { d with misc := d.misc &&& notU 4 } init fx = texOk dev d init fx := by
    simp only [texOk, hf]
  unfold check
  rw [ht, mo]
  simp only [h4, ↓reduceIte]
  rfl

/-- D3DRES.DESC.BLOCKSIZE: a block format's top level is whole blocks -/
theorem blocksize (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4)
    (hb : d.w % max fx.bw 1 ≠ 0 ∨ height d % max fx.bh 1 ≠ 0) : check dev d init fx = none := by
  have : texOk dev d init fx = false := by
    rcases hb with h | h <;> simp [texOk, texCore, strip_height, h]
  simp [check, hd, this]

/-- an accepted texture's checks all hold -/
private theorem tex_holds (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4)
    (hok : check dev d init fx ≠ none) : texOk dev d init fx = true := by
  cases ht : texOk dev d init fx
  · exact absurd (by simp [check, hd, ht]) hok
  · rfl

/-- D3DRES.DESC.MISC: a texture takes only the flags its API knows (11: GENERATE_MIPS, SHARED,
TEXTURECUBE, RESOURCE_CLAMP, SHARED_KEYEDMUTEX, GDI_COMPATIBLE, RESTRICTED_CONTENT; 10: the
first three); RESTRICTED_CONTENT only on 2D textures, not immutable or staging; GDI_COMPATIBLE
only of the B8G8R8A8 formats; GENERATE_MIPS only bound as shader resource and target; a cube is
2D, square, of 6n slices (one cube on Direct3D 10.0) -/
theorem misc_flags (dev : Dev) (d : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4) (hok : check dev d init fx ≠ none) :
    has d.misc (notU (if dev.d3d11 then 0x1387 else 7)) = false ∧
    (has d.misc 0x1000 = true → d.dim = 2 ∧ d.usage ≠ 1 ∧ d.usage ≠ 3) ∧
    (has d.misc 0x200 = true → d.fmt = 87 ∨ d.fmt = 90 ∨ d.fmt = 91) ∧
    (has d.misc 1 = true → d.bind &&& 0x28 = 0x28) ∧
    (has (miscOut dev d) 4 = true → d.dim = 2 ∧ d.da % 6 = 0 ∧ d.w = d.h ∧ (dev = .d10 → d.da = 6)) := by
  have t := tex_holds dev d init fx hd hok
  simp only [texOk, texCore, miscOk, flags, Bool.and_eq_true] at t
  obtain ⟨⟨⟨⟨⟨_, m⟩, _⟩, _⟩, _⟩, _⟩ := t
  simp only [Bool.not_eq_eq_eq_not, Bool.not_true, Bool.and_eq_false_imp] at m
  obtain ⟨⟨⟨⟨⟨⟨m1, m2⟩, m3⟩, m4⟩, m5⟩, m6⟩, m7⟩ := m
  have sub : has d.misc 4096 = true → has d.misc 4225 = true := by
    simp only [has, bne_iff_ne, ne_eq]
    intro h1 h2
    apply h1
    have : d.misc &&& 4096 = (d.misc &&& 4225) &&& 4096 := by rw [Nat.and_assoc]; rfl
    rw [this, h2]
    rfl
  refine ⟨by simpa using m1, fun h => ⟨by simpa using m2 h, fun e => ?_, fun e => ?_⟩, fun h => by simpa [or_assoc] using m5 h,
    fun h => by simpa using m4 h, fun h => ?_⟩
  · have := m7 (by simp [e]); simp_all
  · have := m6 (by simp [e]); have := sub h; simp_all
  · have := m3 h
    simp only [Bool.or_eq_false_iff, bne_eq_false_iff_eq, beq_iff_eq, Bool.and_eq_false_imp] at this
    obtain ⟨⟨⟨a, b⟩, c⟩, e⟩ := this
    simp only [height, a] at c
    exact ⟨a, b, by simpa using c, fun h10 => by simpa using e (by simp [h10])⟩

/-- D3DRES.DESC.CPURTV: Direct3D 11 refuses at creation a 2D default texture the CPU reads and
renders to of more than one subresource; Direct3D 10 makes it, and every other description the
check passes is made -/
theorem cpu_rtv (d : Desc) (l : Nat) :
    createRefused (.d11 l) d = (d.dim == 2 && d.usage == 0 && has d.cpu 0x20000 && has d.bind 0x20 && (d.mips != 1 || d.da != 1)) ∧
    createRefused .d10 d = false ∧ createRefused (.d101 l) d = false := by
  simp [createRefused, Dev.d3d11]

private theorem buf_holds (dev : Dev) (d g : Desc) (init : Bool) (fx : Facts) (hd : d.dim = 4)
    (h : check dev d init fx = some g) : bufOk dev d = true ∧ g = bufOut dev d := by
  cases hb : bufOk dev d <;> cases hc : (has d.cpu (notU 0x30000) || !usageOk d init) <;> simp_all [check]

/-- D3DRES.DESC.BUF: a constant buffer is bound as nothing else and of 16-byte multiples; a
structured buffer's stride is a multiple of 4 up to 2048 that divides the size; the stride reads
back 0 unless the buffer is structured -/
theorem buffers (dev : Dev) (d g : Desc) (init : Bool) (fx : Facts) (hd : d.dim = 4) (h : check dev d init fx = some g) :
    (has d.bind 4 = true → d.bind = 4 ∧ d.w % 16 = 0) ∧
    (has d.misc 0x40 = true → dev.d3d11 = true → d.q ≠ 0 ∧ d.q % 4 = 0 ∧ d.q ≤ 2048 ∧ d.w % d.q = 0) ∧
    g.q = (if has d.misc 0x40 && dev.d3d11 then d.q else 0) ∧ d.w ≠ 0 := by
  obtain ⟨hb, rfl⟩ := buf_holds dev d g init fx hd h
  simp only [bufOk, Bool.and_eq_true, Bool.not_eq_eq_eq_not, Bool.not_true, Bool.and_eq_false_imp] at hb
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨⟨b1, _⟩, _⟩, b2⟩, _⟩, _⟩, _⟩, _⟩, _⟩, b8⟩ := hb
  refine ⟨fun hc => ?_, fun hs hv => ?_, by simp [bufOut], by simpa using b1⟩
  · have := b2 hc
    simp only [Bool.or_eq_false_iff, bne_eq_false_iff_eq] at this
    exact ⟨this.1.2, this.1.1⟩
  · have := b8 ⟨hs, hv⟩
    simp only [Bool.or_eq_false_iff, bne_eq_false_iff_eq, beq_eq_false_iff_ne, ne_eq, decide_eq_false_iff_not] at this
    omega

/-- an accepted texture is its description with its levels and misc flags as kept -/
private theorem tex_out (dev : Dev) (d g : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4)
    (h : check dev d init fx = some g) :
    texOk dev d init fx = true ∧ usageOk d init = true ∧ g = { d with mips := levels d, misc := miscOut dev d } := by
  cases ht : texOk dev d init fx <;> cases hu : usageOk d init <;> cases hc : has d.cpu (notU 0x30000) <;> simp_all [check]

/-- D3DRES.DESC: an accepted texture keeps its description but its levels (a full chain for 0,
never more than one) and Direct3D 10's dropped TEXTURECUBE; its usage takes its CPU access and
binds: immutable has initial data and no CPU access, dynamic CPU writes alone, staging is bound
nowhere; its format is known -/
theorem desc_out (dev : Dev) (d g : Desc) (init : Bool) (fx : Facts) (hd : d.dim ≠ 4) (h : check dev d init fx = some g) :
    g = { d with mips := levels d, misc := miscOut dev d } ∧ d.mips ≤ fullChain d.dim d.w (height d) d.da ∧
    (d.usage = 1 → init = true ∧ d.cpu = 0) ∧ (d.usage = 2 → d.cpu = 0x10000) ∧ (d.usage = 3 → d.bind = 0) ∧
    fx.known = true := by
  obtain ⟨ht, hu, rfl⟩ := tex_out dev d g init fx hd h
  simp only [texOk, texCore, Bool.and_eq_true, decide_eq_true_eq] at ht
  refine ⟨rfl, ?_, fun e => ?_, fun e => ?_, fun e => ?_, ?_⟩
  · simp_all [strip_height]
  · simp_all [usageOk]
  · simp_all [usageOk]
  · simp_all [usageOk]
  · simp_all

def mirrors : List Mirror := [
  ⟨"d3d11.desc", fun
    | [dk, lv, dim, w, h, da, mips, fmt, usage, bind, cpu, misc, count, q, init,
       known, s, bs, uav, bw, bh, q2, q4, q8, q16] => do
      let dev ← match dk with
        | 0 => some Dev.d10 | 1 => some (Dev.d101 (← nat lv)) | 2 => some (Dev.d11 (← nat lv)) | _ => none
      let d : Desc := ⟨← nat dim, ← nat w, ← nat h, ← nat da, ← nat mips, ← nat fmt, ← nat usage,
        ← nat bind, ← nat cpu, ← nat misc, ← nat count, ← nat q⟩
      let fx : Facts := ⟨known != 0, ← nat s, ← nat bs, ← nat uav, ← nat bw, ← nat bh, ← nat q2, ← nat q4, ← nat q8, ← nat q16⟩
      pure (match check dev d (init != 0) fx with
        | none => [0]
        | some g => [1, g.dim, g.w, g.h, g.da, g.mips, g.fmt, g.usage, g.bind, g.cpu, g.misc, g.count, g.q])
    | _ => none⟩,
  ⟨"d3d11.create", fun
    | [dk, lv, dim, w, h, da, mips, fmt, usage, bind, cpu, misc, count, q] => do
      let dev ← match dk with
        | 0 => some Dev.d10 | 1 => some (Dev.d101 (← nat lv)) | 2 => some (Dev.d11 (← nat lv)) | _ => none
      let d : Desc := ⟨← nat dim, ← nat w, ← nat h, ← nat da, ← nat mips, ← nat fmt, ← nat usage,
        ← nat bind, ← nat cpu, ← nat misc, ← nat count, ← nat q⟩
      pure [if createRefused dev d then 1 else 0]
    | _ => none⟩
]

end Kikizake.D3d11.Desc
