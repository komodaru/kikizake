import Kikizake.Mirror
/-! The description checks of Direct3D 12 (spec/d3d12.txt D3D12.DESC, D3D12.CREATE,
D3D12.HEAP clauses), as src/model/d3d12.rs computes them: `validate` is `validate`
(GetCopyableFootprints' checks; the refusing clause as a code), `create` `create_verdict`
(CreateCommittedResource with ppvResource NULL), `support` `dxgi_format::support`, `texState`
`tex_state_ok` (mirrors `d3d12.validate`, `d3d12.create`, `d3d12.support`). -/
namespace Kikizake.D3d12.Create

/-- D3D12_RESOURCE_DESC: Dimension (1 buffer, 2 1D, 3 2D, 4 3D), Alignment, Width, Height,
DepthOrArraySize, MipLevels, Format, SampleDesc, Layout, Flags -/
structure Desc where
  dim : Nat
  align : Nat
  w : Nat
  h : Nat
  d : Nat
  mips : Nat
  fmt : Nat
  samples : Nat
  quality : Nat
  layout : Nat
  flags : Nat

def SFALSE : Nat := 1
def INVALIDARG : Nat := 0x80070057
def NOTIMPL : Nat := 0x80004001

/-- the refusing clause of `validate`: D3D12.DESC.DIM, BUFFER, SIZE, ALIGN, MSAA, LAYOUT -/
def DIM : Nat := 1
def BUFFER : Nat := 2
def SIZE : Nat := 3
def ALIGN : Nat := 4
def MSAA : Nat := 5
def LAYOUT : Nat := 6

/-- the support bits of a format: 1D, 2D, 3D textures, render target, depth stencil,
multisampled render target, the video formats' one level (arrays taken), one level -/
def T1 : Nat := 1
def T2 : Nat := 2
def T3 : Nat := 4
def RT : Nat := 8
def DS : Nat := 16
def MS : Nat := 32
def SINGLE : Nat := 64
def ONE_MIP : Nat := 128

/-- f in a..b -/
def r (f a b : Nat) : Bool := a ≤ f && f ≤ b

/-- D3D12.CREATE.FORMAT, FORMAT.DIMS: what CreateCommittedResource takes each DXGI format for on WARP -/
def support (f : Nat) : Nat :=
  let color := T1 ||| T2 ||| T3 ||| RT ||| MS
  if r f 1 18 || r f 23 38 || r f 48 52 || r f 60 65 || f == 67 || r f 85 88 || r f 90 93 || f == 115 || f == 191 then color
  else if f == 39 || r f 41 43 || f == 53 || f == 54 || r f 56 59 then color ||| DS
  else if f == 40 || f == 55 then T1 ||| T2 ||| RT ||| MS ||| DS
  else if r f 19 22 || r f 44 47 then T1 ||| T2 ||| DS ||| MS
  else if f == 68 || f == 69 then T1 ||| T2 ||| T3
  else if r f 70 84 || r f 94 99 then T2 ||| T3
  else if f == 89 then T2 ||| RT ||| ONE_MIP
  else if f == 100 then T2 ||| RT
  else if f == 101 || f == 113 || f == 114 then T2 ||| ONE_MIP
  else if f == 102 then T2
  else if r f 103 105 then T2 ||| RT ||| SINGLE
  else if r f 106 109 then T2 ||| SINGLE
  else 0

/-- a format without an element size (`dxgi_format::block` None): UNKNOWN, the planar depth
stencil and video formats, the opaque and unassigned values -/
def blockless (f : Nat) : Bool :=
  !(r f 1 18 || r f 23 43 || r f 48 65 || r f 67 102 || r f 107 109 || r f 111 115 || f == 191)

/-- a texture dimension's support bit; 0 for a buffer and the values outside -/
def texBit (dim : Nat) : Nat := if dim = 2 then T1 else if dim = 3 then T2 else if dim = 4 then T3 else 0

/-- D3D12.DESC.SIZE, FOOTPRINT.LIMITS: the largest width and height CreateCommittedResource and
GetCopyableFootprints take (`size_max`) -/
def lim (x : Desc) : Nat :=
  if x.dim = 4 then 2048 else if x.dim = 3 ∧ x.samples ≤ 1 ∧ x.mips ≤ 1 ∧ x.d = 1 then 2 ^ 23 else 16384

/-- D3D12.DESC.*, FOOTPRINT.LIMITS: why GetCopyableFootprints refuses a description (0: taken) -/
def validate (x : Desc) : Nat :=
  if ¬(1 ≤ x.dim ∧ x.dim ≤ 4) then DIM
  else if x.dim = 1 then
    if x.align ≠ 0 ∧ x.align ≠ 65536 then BUFFER
    else if x.h ≠ 1 ∨ x.d ≠ 1 ∨ x.mips ≠ 1 ∨ x.fmt ≠ 0 ∨ x.samples ≠ 1 ∨ x.quality ≠ 0 ∨ x.layout ≠ 1 ∨ x.w = 0 then BUFFER
    else 0
  else if x.w = 0 ∨ x.h = 0 ∨ x.d = 0 then SIZE
  else if x.d > 2048 ∨ (x.h > 1 ∧ x.dim = 2) then SIZE
  else if x.w > lim x ∨ x.h > lim x then SIZE
  else if ¬(x.align = 0 ∨ x.align = 4096 ∨ x.align = 65536 ∨ x.align = 4194304) then ALIGN
  else if x.samples = 0 then MSAA
  else if (x.samples > 1 ∨ x.quality > 0) ∧ (x.dim ≠ 3 ∨ x.flags &&& 3 = 0 ∨ x.mips ≠ 1 ∨ x.flags &&& 0x20 ≠ 0 ∨ x.layout = 3) then MSAA
  else if x.layout > 3 then LAYOUT
  else 0

/-- the states that write (D3D12_RESOURCE_STATES), taken alone only -/
def WRITES : Nat := 0x4 ||| 0x8 ||| 0x10 ||| 0x100 ||| 0x400 ||| 0x1000 ||| 0x20000 ||| 0x80000 ||| 0x800000
/-- the states any texture may start in -/
def ANY : Nat := 0x40 ||| 0x80 ||| 0x400 ||| 0x800 ||| 0x1000 ||| 0x2000 ||| 0xf0000 ||| 0x200000 ||| 0x800000 ||| 0x1000000

/-- D3D12.CREATE.STATE: whether a texture of these flags may start in this state -/
def texState (state flags : Nat) : Bool :=
  if flags &&& 0x20 ≠ 0 then state != 0x400000
  else if state = 0xac3 then true
  else
    let allowed := ANY ||| (if flags &&& 1 ≠ 0 then 0x4 else 0) ||| (if flags &&& 4 ≠ 0 then 0x8 else 0) ||| (if flags &&& 2 ≠ 0 then 0x30 else 0)
    state &&& (0xffffffff - allowed) == 0 && (state &&& WRITES == 0 || state &&& (state - 1) == 0)

/-- D3D12.DESC.ALIGN -/
def alignOk (x : Desc) (small : Bool) : Prop :=
  x.align = 0 ∨ x.align = 65536 ∨ (x.align = 4096 ∧ x.flags &&& 3 = 0 ∧ x.samples ≤ 1 ∧ small = true) ∨ (x.align = 4194304 ∧ x.samples > 1)

instance (x : Desc) (s : Bool) : Decidable (alignOk x s) := by unfold alignOk; infer_instance

/-- D3D12.DESC.MSAA -/
def msOk (x : Desc) : Prop :=
  x.dim = 3 ∧ x.flags &&& 3 ≠ 0 ∧ x.mips = 1 ∧ (x.samples = 2 ∨ x.samples = 4 ∨ x.samples = 8 ∨ x.samples = 16) ∧
    (x.quality = 0 ∨ x.quality = 0xffffffff) ∧ support x.fmt &&& MS ≠ 0 ∧ x.flags &&& 0x520 = 0 ∧ x.layout ≠ 3

instance (x : Desc) : Decidable (msOk x) := by unfold msOk; infer_instance

/-- D3D12.DESC.LAYOUT, CREATE.LAYOUT64K -/
def layoutOk (x : Desc) (blockless : Bool) : Prop :=
  x.layout = 0 ∨ ((x.layout = 2 ∨ x.layout = 3) ∧ x.dim ≠ 2 ∧ ¬(x.layout = 2 ∧ x.dim = 3 ∧ x.d > 1) ∧
    ¬(x.layout = 3 ∧ x.flags &&& 2 ≠ 0) ∧ ¬(x.layout = 2 ∧ blockless = true))

instance (x : Desc) (b : Bool) : Decidable (layoutOk x b) := by unfold layoutOk; infer_instance

/-- the flags CreateCommittedResource takes -/
def FLAGS : Nat := 1 ||| 2 ||| 4 ||| 8 ||| 0x20 ||| 0x100 ||| 0x400

/-- a buffer's checks (D3D12.DESC.BUFFER, CREATE.BUFFERFLAGS, HEAP.CPU, CREATE.BUFFERSTATE) -/
def buffer (heap state : Nat) (x : Desc) : Nat :=
  if validate x ≠ 0 ∨ x.flags &&& 0x22 ≠ 0 then INVALIDARG
  else if heap = 2 ∨ heap = 3 then
    if x.flags &&& 7 ≠ 0 ∨ state &&& (0xffffffff - (if heap = 2 then 0xac3 else 0x1400)) ≠ 0 then INVALIDARG else SFALSE
  else SFALSE

/-- D3D12.DESC.DIM, SIZE, ALIGN, MSAA, LAYOUT, CREATE.FORMAT, TARGETS, SINGLE, STATE: what a
texture CreateCommittedResource takes is, past its heap (each failing check E_INVALIDARG): 1D,
2D or 3D; at least 1 in each size, a 1D one 1 high, at most 2048 slices or deep, at most `lim`
wide and high; its format takes the dimension, or it is a 2D render target or depth stencil of
a format that is one; a render target's format is one, a depth stencil's one,
DENY_SHADER_RESOURCE only with DEPTH_STENCIL; the one-level formats one level, the video ones an
even width too (4:2:0 an even height); aligned as `alignOk`; a sample at least, several as
`msOk`; laid out as `layoutOk`; starting in a state `texState` takes -/
def textureOk (state : Nat) (x : Desc) (blockless small : Bool) : Prop :=
  texBit x.dim ≠ 0 ∧
  (1 ≤ x.w ∧ 1 ≤ x.h ∧ 1 ≤ x.d ∧ (x.dim = 2 → x.h = 1) ∧ x.d ≤ 2048) ∧
  (x.w ≤ lim x ∧ x.h ≤ lim x) ∧
  (support x.fmt &&& texBit x.dim ≠ 0 ∨ x.flags &&& 3 ≠ 0 ∧ support x.fmt &&& (RT ||| DS) ≠ 0 ∧ x.dim = 3) ∧
  ((x.flags &&& 1 ≠ 0 → support x.fmt &&& RT ≠ 0) ∧ (x.flags &&& 2 ≠ 0 → support x.fmt &&& DS ≠ 0) ∧
    (x.flags &&& 8 ≠ 0 → x.flags &&& 2 ≠ 0)) ∧
  (support x.fmt &&& ONE_MIP ≠ 0 → x.mips = 1) ∧
  (support x.fmt &&& SINGLE ≠ 0 → x.mips = 1 ∧ x.w % 2 = 0 ∧ (103 ≤ x.fmt ∧ x.fmt ≤ 106 → x.h % 2 = 0)) ∧
  alignOk x small ∧
  1 ≤ x.samples ∧ (x.samples ≤ 1 ∨ msOk x) ∧
  layoutOk x blockless ∧
  texState state x.flags = true

instance (state : Nat) (x : Desc) (b s : Bool) : Decidable (textureOk state x b s) := by unfold textureOk; infer_instance

/-- a texture's verdict past its heap -/
def texture (state : Nat) (x : Desc) (blockless small : Bool) : Nat :=
  if textureOk state x blockless small then SFALSE else INVALIDARG

/-- D3D12.CREATE: CreateCommittedResource's verdict on a heap type (1 default, 2 upload,
3 readback, 4 custom, 5 GPU upload), an initial state and a description; `blockless` a format
with no element size, `small` a texture of at most 16 tiles of 4KB with 4KB asked -/
def create (heap state : Nat) (x : Desc) (blockless small : Bool) : Nat :=
  if ¬(1 ≤ heap ∧ heap ≤ 3 ∨ heap = 5) then INVALIDARG
  else if x.dim ≠ 1 ∧ x.layout = 0xffffffff then NOTIMPL
  else if x.flags &&& (0xffffffff - FLAGS) ≠ 0 ∨ x.flags &&& 3 = 3 ∨ x.flags &&& 6 = 6 then INVALIDARG
  else if x.dim = 1 then buffer heap state x
  else if heap = 2 ∨ heap = 3 then INVALIDARG
  else texture state x blockless small

/-- D3D12.DESC.FLAGS: what CreateCommittedResource takes has no other flag, not RENDER_TARGET
with DEPTH_STENCIL, nor DEPTH_STENCIL with UNORDERED_ACCESS -/
theorem flags_taken (heap state : Nat) (x : Desc) (b s : Bool) (h : create heap state x b s = SFALSE) :
    x.flags &&& (0xffffffff - FLAGS) = 0 ∧ x.flags &&& 3 ≠ 3 ∧ x.flags &&& 6 ≠ 6 := by
  unfold create at h
  repeat' split at h
  all_goals first | (simp [INVALIDARG, NOTIMPL, SFALSE] at h; done) | (simp only [ne_eq, not_or, Decidable.not_not] at *; exact ⟨by omega, by omega, by omega⟩)


/-- D3D12.HEAP.TYPE: a heap type CreateCommittedResource does not know is refused -/
theorem heap_type (heap state : Nat) (x : Desc) (b s : Bool) (h : ¬(1 ≤ heap ∧ heap ≤ 3 ∨ heap = 5)) :
    create heap state x b s = INVALIDARG := by
  simp [create, h]

/-- D3D12.CREATE.LAYOUTNOTIMPL: a texture's layout 0xffffffff is E_NOTIMPL, before any other check
of the description -/
theorem layout_notimpl (heap state : Nat) (x : Desc) (b s : Bool) (hh : 1 ≤ heap ∧ heap ≤ 3 ∨ heap = 5)
    (hd : x.dim ≠ 1) (hl : x.layout = 0xffffffff) : create heap state x b s = NOTIMPL := by
  simp [create, hh, hd, hl]

/-- D3D12.HEAP.CPU: no texture on an upload or readback heap -/
theorem cpu_no_texture (state : Nat) (x : Desc) (b s : Bool) (heap : Nat) (hh : heap = 2 ∨ heap = 3) (hd : x.dim ≠ 1) :
    create heap state x b s ≠ SFALSE := by
  intro h
  unfold create at h
  repeat' split at h
  all_goals first | (simp [INVALIDARG, NOTIMPL, SFALSE] at h; done) | omega

/-- D3D12.HEAP.CPU, CREATE.BUFFERFLAGS, DESC.BUFFER: a buffer taken is a valid one without
SIMULTANEOUS_ACCESS or DEPTH_STENCIL; on an upload or readback heap without RENDER_TARGET,
DEPTH_STENCIL or UNORDERED_ACCESS, starting in states GENERIC_READ is made of (upload) or
COPY_DEST and RESOLVE_DEST (readback) -/
theorem buffer_taken (heap state : Nat) (x : Desc) (b s : Bool) (hd : x.dim = 1) (h : create heap state x b s = SFALSE) :
    validate x = 0 ∧ x.flags &&& 0x22 = 0 ∧
    (heap = 2 → x.flags &&& 7 = 0 ∧ state &&& (0xffffffff - 0xac3) = 0) ∧
    (heap = 3 → x.flags &&& 7 = 0 ∧ state &&& (0xffffffff - 0x1400) = 0) := by
  unfold create buffer at h
  repeat' split at h
  all_goals first | (simp [INVALIDARG, NOTIMPL, SFALSE] at h; done) | (simp_all; done)


/-- D3D12.CREATE: a texture CreateCommittedResource takes is one `textureOk` holds for, on the
default or GPU upload heap -/
theorem texture_taken (heap state : Nat) (x : Desc) (b s : Bool) (hd : x.dim ≠ 1) (h : create heap state x b s = SFALSE) :
    (heap = 1 ∨ heap = 5) ∧ textureOk state x b s := by
  unfold create at h
  split at h
  · exact absurd h (by decide)
  split at h
  · exact absurd h (by decide)
  split at h
  · exact absurd h (by decide)
  split at h
  · contradiction
  unfold texture at h
  split at h
  · constructor
    · omega
    · assumption
  · exact absurd h (by decide)

/-- two textures of the same dimension, layout and flags that `textureOk` holds for alike get
one verdict -/
private theorem create_congr (heap state : Nat) (x y : Desc) (b s : Bool) (hd : x.dim ≠ 1) (hdy : y.dim = x.dim)
    (hl : y.layout = x.layout) (hf : y.flags = x.flags) (ht : textureOk state y b s ↔ textureOk state x b s) :
    create heap state y b s = create heap state x b s := by
  have hy : y.dim ≠ 1 := by omega
  unfold create texture
  rw [hl, hf]
  by_cases t : textureOk state x b s
  · have t' := ht.mpr t
    simp [hd, hy, t, t']
  · have t' : ¬textureOk state y b s := fun a => t (ht.mp a)
    simp [hd, hy, t, t']

/-- D3D12.CREATE.QUALITY: with one sample a texture's quality is not looked at -/
theorem quality_unchecked (heap state q : Nat) (x : Desc) (b s : Bool) (hd : x.dim ≠ 1) (h1 : x.samples ≤ 1) :
    create heap state { x with quality := q } b s = create heap state x b s := by
  apply create_congr heap state x { x with quality := q } b s hd rfl rfl rfl
  simp [textureOk, lim, alignOk, layoutOk, h1]

/-- D3D12.CREATE.MIPS: nor its MipLevels against its size (single-sampled, not a one-level
format): any two counts past 1 are one -/
theorem mips_unchecked (heap state m : Nat) (x : Desc) (b s : Bool) (hd : x.dim ≠ 1) (h1 : x.samples ≤ 1)
    (ho : support x.fmt &&& ONE_MIP = 0) (hs : support x.fmt &&& SINGLE = 0) (hm : 2 ≤ x.mips) (hm' : 2 ≤ m) :
    create heap state { x with mips := m } b s = create heap state x b s := by
  have a : ¬x.mips ≤ 1 := by omega
  have a' : ¬m ≤ 1 := by omega
  apply create_congr heap state x { x with mips := m } b s hd rfl rfl rfl
  simp [textureOk, lim, alignOk, layoutOk, msOk, h1, ho, hs, a, a']

/-- D3D12.CREATE.GPUUPLOAD: a GPU upload heap takes textures as the default heap does, buffers
in any state -/
theorem gpu_upload (state : Nat) (x : Desc) (b s : Bool) (hd : x.dim ≠ 1) :
    create 5 state x b s = create 1 state x b s := by
  simp [create, hd]

/-- D3D12.CREATE.BUFFERSTATE: a buffer on the default or GPU upload heap starts in any state -/
theorem buffer_state (heap state state' : Nat) (x : Desc) (b s : Bool) (hd : x.dim = 1) (hh : heap = 1 ∨ heap = 5) :
    create heap state x b s = create heap state' x b s := by
  rcases hh with hh | hh <;> simp [create, buffer, hd, hh]

/-- D3D12.CREATE.STATE: a simultaneous-access texture starts in any state but
RAYTRACING_ACCELERATION_STRUCTURE alone; the others take GENERIC_READ -/
theorem sim_state (state flags : Nat) (h : flags &&& 0x20 ≠ 0) : texState state flags = (state != 0x400000) := by
  simp [texState, h]

theorem generic_read (flags : Nat) : texState 0xac3 flags = true := by
  unfold texState; split <;> simp

/-- the write states -/
def writes : List Nat := [0x4, 0x8, 0x10, 0x100, 0x400, 0x1000, 0x20000, 0x80000, 0x800000]

/-- D3D12.CREATE.STATE: a write state is taken alone only: with any other of the 32 bits it is
refused, whatever the flags (but SIMULTANEOUS_ACCESS) -/
theorem write_alone : ∀ f, f < 32 → ∀ w ∈ writes, ∀ k, k < 32 → 2 ^ k ≠ w → texState (w ||| 2 ^ k) f = false := by
  decide +kernel

/-- D3D12.CREATE.STATE: RENDER_TARGET, UNORDERED_ACCESS, DEPTH_WRITE and DEPTH_READ want their
flags; VERTEX_AND_CONSTANT_BUFFER (1), INDEX_BUFFER (2), STREAM_OUT (0x100) alone,
INDIRECT_ARGUMENT (0x200) and RAYTRACING_ACCELERATION_STRUCTURE are refused, whatever the flags
(but SIMULTANEOUS_ACCESS) -/
theorem state_flags : ∀ f, f < 32 →
    (texState 0x4 f = (f &&& 0x20 != 0 || f &&& 1 != 0)) ∧ (texState 0x8 f = (f &&& 0x20 != 0 || f &&& 4 != 0)) ∧
    (texState 0x10 f = (f &&& 0x20 != 0 || f &&& 2 != 0)) ∧ (texState 0x20 f = (f &&& 0x20 != 0 || f &&& 2 != 0)) ∧
    (f &&& 0x20 = 0 → texState 1 f = false ∧ texState 2 f = false ∧ texState 0x100 f = false ∧
      texState 0x200 f = false ∧ texState 0x400000 f = false) := by
  decide +kernel

/-- D3D12.CREATE.LAYOUT64K, DESC.LAYOUT: the two-plane formats D24S8 (45), D32S8 (20), NV12
(103) have no element; D32 (40) one -/
theorem two_planes : blockless 45 = true ∧ blockless 20 = true ∧ blockless 103 = true ∧ blockless 40 = false := by
  decide

/-- D3D12.DESC.SIZE: a single-sampled 2D texture of one slice and one level up to 2^23 by width
or height (65536 x 131072 R8 taken, 2^23 + 1 refused); with two slices 16384 -/
theorem single_2d :
    create 1 0 ⟨3, 0, 65536, 131072, 1, 1, 61, 1, 0, 0, 0⟩ false false = SFALSE ∧
    create 1 0 ⟨3, 0, 2 ^ 23, 1, 1, 1, 61, 1, 0, 0, 0⟩ false false = SFALSE ∧
    create 1 0 ⟨3, 0, 2 ^ 23 + 1, 1, 1, 1, 61, 1, 0, 0, 0⟩ false false = INVALIDARG ∧
    create 1 0 ⟨3, 0, 16385, 1, 2, 1, 61, 1, 0, 0, 0⟩ false false = INVALIDARG := by
  decide

/-- D3D12.FOOTPRINT.LIMITS, DESC.SIZE: GetCopyableFootprints takes a texture only within the
sizes CreateCommittedResource takes; within them a 2D or 3D texture's width and height are not
looked at -/
theorem footprint_limits (x : Desc) (hv : validate x = 0) (hd : x.dim ≠ 1) : x.w ≤ lim x ∧ x.h ≤ lim x := by
  unfold validate at hv
  repeat' split at hv
  all_goals first | exact absurd hv (by decide) | omega

theorem footprint_sizes (x : Desc) (w h : Nat) (hd : x.dim ≠ 1) (h1 : x.dim ≠ 2) (hw : 1 ≤ x.w ∧ x.w ≤ lim x) (hh : 1 ≤ x.h ∧ x.h ≤ lim x)
    (hw' : 1 ≤ w ∧ w ≤ lim x) (hh' : 1 ≤ h ∧ h ≤ lim x) : validate { x with w := w, h := h } = validate x := by
  have l : lim { x with w := w, h := h } = lim x := by simp [lim]
  have a : x.w ≠ 0 := by omega
  have b : x.h ≠ 0 := by omega
  have c : w ≠ 0 := by omega
  have e : h ≠ 0 := by omega
  have f : ¬(x.w > lim x ∨ x.h > lim x) := by omega
  have g : ¬(w > lim x ∨ h > lim x) := by omega
  simp only [validate, l]
  simp only [ne_eq] at hd h1
  simp only [hd, h1, a, b, c, e, f, g, ite_false, and_false, or_false, false_or]

def mirrors : List Mirror := [
  ⟨"d3d12.support", fun | [f] => (nat f).map fun f => [support f, if blockless f then 1 else 0] | _ => none⟩,
  ⟨"d3d12.validate", fun
    | [dim, align, w, h, d, mips, fmt, samples, quality, layout, flags] => do
      some [validate ⟨← nat dim, ← nat align, ← nat w, ← nat h, ← nat d, ← nat mips, ← nat fmt, ← nat samples, ← nat quality, ← nat layout, ← nat flags⟩]
    | _ => none⟩,
  ⟨"d3d12.create", fun
    | [heap, state, dim, align, w, h, d, mips, fmt, samples, quality, layout, flags, blockless, small] => do
      some [create (← nat heap) (← nat state) ⟨← nat dim, ← nat align, ← nat w, ← nat h, ← nat d, ← nat mips, ← nat fmt, ← nat samples, ← nat quality, ← nat layout, ← nat flags⟩ (blockless != 0) (small != 0)]
    | _ => none⟩
]

end Kikizake.D3d12.Create
