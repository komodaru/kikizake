import Kikizake.Mirror
/-! GetGlyphOutline's bitmaps and TextOut's pixels of the suite's rectangle glyphs
(spec/gditext.txt GDI.TEXT.BITMAP, GDI.TEXT.GRAY, GDI.TEXT.ITALIC, GDI.TEXT.DRAW), as
src/model/gdifont.rs `glyph` and `text_pixels` compute them (mirrors `gdi.glyph`,
`gdi.text`): the outline on the rounded ppem snapped to 1/64 pixel, sampled at pixel centres
on a grid k times finer (k 1 for GGO_BITMAP, 2, 4, 8 for the gray formats), the dropout rule,
the synthesized italic's shear and bold, the rows top down, each a whole number of DWORDs.

The font is the suite's with OS/2 and a negative lfHeight: the ppem is -lfHeight, the scale
-lfHeight / upm, the ascent A and descent D usWinAscent and the signed usWinDescent. -/
namespace Kikizake.Gdi.Glyph

/-- n / d rounded half up, n / d rounded up (d > 0); Rust's truncating division -/
def rnd (n d : Int) : Int := (2 * n + d) / (2 * d)
def ceil (n d : Int) : Int := -((-n) / d)
def tdiv (n d : Int) : Int := n.tdiv d

/-- the request: units per em, ppem, ascent and descent (font units), bold, italic -/
structure Font where
  upm : Int
  ppem : Int
  a : Int
  d : Int
  bold : Bool
  italic : Bool

/-- font units to pixels at the TEXTMETRIC scale, halves up -/
def Font.px (f : Font) (v : Int) : Int := rnd (v * f.ppem) f.upm
/-- a font unit on the rounded ppem in 1/64 pixel -/
def Font.q (f : Font) (v : Int) : Int := rnd (v * f.ppem * 64) f.upm

/-- GDI.TEXT.ITALIC: the shear of a point y (1/64 pixel, up) along x: y 0x5700 / 2^16,
rounded half away from 0 -/
def shear (y : Int) : Int := (y * 0x5700 + 0x8000 - (if y * 0x5700 < 0 then 1 else 0)) / 65536

def Font.point (f : Font) (x y : Int) : Int × Int :=
  let (x, y) := (f.q x, f.q y)
  if f.italic then (x + shear y, y) else (x, y)

/-- the whole numbers from lo to hi -/
def span (lo hi : Int) : List Int := (List.range (hi - lo + 1).toNat).map fun (i : Nat) => lo + (i : Int)

/-- a rectangle (x0, y0, x1, y1) -/
abbrev Rect := Int × Int × Int × Int

/-- the samples a glyph lights on the k-finer grid: whether sample (i, j) is lit, and columns
i0 to i1 and rows j0 to j1 that hold every lit one -/
structure Lit where
  lit : Int → Int → Bool
  i0 : Int
  i1 : Int
  j0 : Int
  j1 : Int

/-- v in lo to hi; lo to hi empty -/
def inside (lh : Int × Int) (v : Int) : Bool := decide (lh.1 ≤ v ∧ v ≤ lh.2)
def empty (lh : Int × Int) : Bool := decide (lh.2 < lh.1)

/-- the sample columns (rows) from lo to hi in 1/64 of the grid: centres 64 i + 32 on or
between them -/
def centres (lo hi : Int) : Int × Int := (ceil (lo - 32) 64, (hi - 32) / 64)

/-- the columns (rows) from the least to the greatest of vs, and one past -/
def reach (v : Int) (vs : List Int) : Int × Int := ((vs.foldl min v - 32) / 64, (vs.foldl max v - 32) / 64 + 1)

/-- the samples of the upright rectangles (GDI.TEXT.BITMAP): each sample point (64 i + 32,
64 j + 32) of the k-finer grid inside a rectangle or on its edge is lit; a rectangle a scan
line crosses between two samples lights the dropout one of the pair: the left (lower) one, or
the right (upper) one where the left lies before the black box's left (bottom) edge rounded as
GetCharABCWidths rounds it, unless the other one of the pair is lit by the scan lines (rows'
crossings) or by those and the rows' dropouts (columns' crossings); none where a rectangle
lies between samples both ways -/
def upright (f : Font) (r : List Rect) (k : Int) : Option Lit :=
  let q := fun v => f.q v * k
  let r := r.map fun (a, b, c, d) => (q a, q b, q c, q d)
  -- (each rectangle with its sample columns and rows)
  let es := r.map fun e => (e, centres e.1 e.2.2.1, centres e.2.1 e.2.2.2)
  match r.map (·.1), r.map (·.2.1) with
  | x :: xs, y :: ys =>
    if es.any (fun (_, cs, rs) => empty cs && empty rs) then none else
    let bx := ceil (xs.foldl min x - 32) 64
    let by_ := ceil (ys.foldl min y - 32) 64
    -- (the dropout sample of a crossing just past edge lo, and the other one of its pair)
    let drop := fun (lo b : Int) => let k := (lo - 32) / 64; if k < b then (k + 1, k) else (k, k + 1)
    let scans := fun i j => es.any fun (_, cs, rs) => inside cs i && inside rs j
    let across := fun i j => es.any fun (e, cs, rs) => empty cs && inside rs j &&
      (let (p, o) := drop e.1 bx; p == i && !scans o j)
    let rowsLit := fun i j => scans i j || across i j
    let down := fun i j => es.any fun (e, cs, rs) => empty rs && inside cs i &&
      (let (p, o) := drop e.2.1 by_; p == j && !rowsLit i o)
    let (i0, i1) := reach x (xs ++ r.map (·.2.2.1))
    let (j0, j1) := reach y (ys ++ r.map (·.2.2.2))
    some ⟨fun i j => rowsLit i j || down i j, i0, i1, j0, j1⟩
  | _, _ => some ⟨fun _ _ => false, 0, -1, 0, -1⟩

/-- a rectangle's parallelogram on the k-finer grid: its left side from (a0, b0) to (a1, b1),
its right side from (c0, b0) to (c1, b1) -/
structure Slant where
  a0 : Int
  a1 : Int
  c0 : Int
  c1 : Int
  b0 : Int
  b1 : Int

def Slant.of (f : Font) (k : Int) (e : Rect) : Slant :=
  let (x0, y0, x1, y1) := e
  let ((a0, b0), (a1, b1)) := (f.point x0 y0, f.point x0 y1)
  let (c0, c1) := ((f.point x1 y0).1, (f.point x1 y1).1)
  ⟨a0 * k, a1 * k, c0 * k, c1 * k, b0 * k, b1 * k⟩

/-- the columns of its row j's samples: centres between the sides' x at the row's centre
(fractions over the height) -/
def Slant.row (p : Slant) (j : Int) : Int × Int :=
  let (dx, dy) := (p.a1 - p.a0, p.b1 - p.b0)
  let y := 64 * j + 32
  let (ln, rn) := (p.a0 * dy + dx * (y - p.b0), p.c0 * dy + dx * (y - p.b0))
  (ceil (ln - 32 * dy) (64 * dy), (rn - 32 * dy) / (64 * dy))

def Slant.rows (p : Slant) : Int × Int := centres p.b0 p.b1

/-- its sample (i, j) -/
def Slant.lit (p : Slant) (i j : Int) : Bool := inside p.rows j && inside (p.row j) i

/-- a row of it between samples (its dropout, not measured) -/
def Slant.rowGap (p : Slant) : Bool := (span p.rows.1 p.rows.2).any fun j => empty (p.row j)

/-- a column crossing it between rows where no sample of the glyph so far is lit in that
column (its dropout, not measured) -/
def Slant.colGap (p : Slant) (sofar : List Slant) : Bool :=
  let (dx, dy) := (p.a1 - p.a0, p.b1 - p.b0)
  (span (ceil (min p.a0 p.a1 - 32) 64) ((max p.c0 p.c1 - 32) / 64)).any fun i =>
    let x := 64 * i + 32
    let top := if dx = 0 then p.b1 else max (p.b0 + tdiv ((x - p.c0) * dy) dx) p.b0
    let hi := if dx = 0 then p.b1 else min (p.b0 + tdiv ((x - p.a0) * dy) dx) p.b1
    let (lo, hi) := (max top p.b0, min hi p.b1)
    decide (lo ≤ hi) && !((span (ceil (lo - 32) 64) ((hi - 32) / 64)).any fun j => sofar.any (·.lit i j))

/-- the samples of the synthesized italic's parallelograms (GDI.TEXT.ITALIC): each rectangle's
corners sheared, a sample inside or on an edge lit; none where a parallelogram has no height,
or a row or a column crosses one between samples (its dropout, not measured) -/
def slanted (f : Font) (r : List Rect) (k : Int) : Option Lit := do
  let ps := r.map (Slant.of f k)
  let _ ← ps.foldl (fun acc p => acc.bind fun sofar =>
    if decide (p.b1 = p.b0) || p.rowGap || p.colGap (sofar ++ [p]) then none else some (sofar ++ [p])) (some [])
  let xs := ps.flatMap fun p => [p.a0, p.a1, p.c0, p.c1]
  let ys := ps.flatMap fun p => [p.b0, p.b1]
  let (i0, i1) := reach 0 xs
  let (j0, j1) := reach 0 ys
  pure ⟨fun i j => ps.any (·.lit i j), i0, i1, j0, j1⟩

/-- the formats: GGO_METRICS 0, GGO_BITMAP 1, GGO_GRAY2 4, GRAY4 5, GRAY8 6: the grid's
fineness and a pixel's top value -/
def fineness (fmt : Int) : Option (Int × Int) :=
  match fmt with
  | 0 | 1 => some (1, 1)
  | 4 => some (2, 4)
  | 5 => some (4, 16)
  | 6 => some (8, 64)
  | _ => none

/-- a glyph's pixels: each one's value, and columns i0 to i1 and rows j0 to j1 that hold every
one not 0 -/
structure Raster where
  value : Int → Int → Int
  i0 : Int
  i1 : Int
  j0 : Int
  j1 : Int

/-- the pixels not 0, row by row -/
def Raster.list (p : Raster) : List ((Int × Int) × Int) :=
  (span p.j0 p.j1).flatMap fun j => (span p.i0 p.i1).filterMap fun i =>
    let n := p.value i j
    if n > 0 then some ((i, j), n) else none

/-- the pixels' values: the samples lit of each pixel's k x k; bold adds each pixel's value to
its right neighbour's, at most the top (GDI.TEXT.BITMAP, GDI.TEXT.GRAY) -/
def raster (f : Font) (r : List Rect) (fmt : Int) : Option Raster := do
  let (k, top) ← fineness fmt
  let s ← if f.italic then slanted f r k else upright f r k
  let count := fun i j => (span 0 (k - 1)).foldl (fun n a =>
    (span 0 (k - 1)).foldl (fun n b => if s.lit (k * i + a) (k * j + b) then n + 1 else n) n) (0 : Int)
  -- (the counts of the pixels that hold samples, row by row from column i0 - 1)
  let (i0, i1, j0) := (s.i0 / k, s.i1 / k + 1, s.j0 / k)
  let wide := i1 - i0 + 2
  let tbl := ((span j0 (s.j1 / k)).flatMap fun j => (span (i0 - 1) i1).map fun i => count i j).toArray
  let cnt := fun i j => if i0 - 1 ≤ i ∧ i ≤ i1 ∧ j0 ≤ j then tbl.getD ((j - j0) * wide + (i - i0 + 1)).toNat 0 else 0
  let value := fun i j => if f.bold then min top (cnt i j + cnt (i - 1) j) else cnt i j
  pure ⟨value, i0, i1, j0, s.j1 / k⟩

def pixels (f : Font) (r : List Rect) (fmt : Int) : Option (List ((Int × Int) × Int)) :=
  (raster f r fmt).map Raster.list

/-- GDI.TEXT.WIDTH: the advance scaled, the bold pixel where it is not 0 -/
def width (f : Font) (adv : Int) : Int := f.px adv + (if f.bold && adv > 0 then 1 else 0)

/-- the rows of the black box and of what TextOut draws: from the descent below the baseline
to the ascent -/
def clipRows (f : Font) : Int × Int := (-(f.px f.d), f.px f.a)

/-- a row's bytes: GGO_BITMAP's 1 bit a pixel, high bit first, the gray formats a byte a
pixel; each row a whole number of DWORDs -/
def stride (fmt w : Int) : Int := if fmt ≤ 1 then (w + 31) / 32 * 4 else (w + 3) / 4 * 4

/-- the bitmap, rows top down from row y1, columns from x0: GGO_BITMAP a bit for each pixel
not 0, the gray formats each pixel's value -/
def bytes (fmt : Int) (value : Int → Int → Int) (x0 y1 w h : Int) : List Int :=
  let s := stride fmt w
  (List.range (s * h).toNat).map fun (n : Nat) =>
    let (row, b) := ((n : Int) / s, (n : Int) % s)
    let j := y1 - row
    if fmt ≤ 1 then
      (((List.range 8).foldl (fun (acc : Nat) (t : Nat) =>
        if value (x0 + 8 * b + (t : Int)) j > 0 ∧ 8 * b + (t : Int) < w then acc ||| (0x80 >>> t) else acc) 0 : Nat) : Int)
    else if b < w then value (x0 + b) j else 0

/-- GetGlyphOutline (GDI.TEXT.BITMAP, GDI.TEXT.GRAY): the buffer's size, gmBlackBoxX and Y,
the origin, the advance and the buffer; an empty glyph a 1 by 1 box and no buffer; none where
every row lies outside the cell -/
def glyph (f : Font) (r : List Rect) (adv fmt : Int) : Option (List Int) := do
  let p ← raster f r fmt
  let v := p.list
  let inc := width f adv
  let is := v.map (·.1.1)
  let js := v.map (·.1.2)
  match is, js with
  | i :: is', j :: js' =>
    let (x0, x1) := (is'.foldl min i, is'.foldl max i)
    let (y0, y1) := (js'.foldl min j, js'.foldl max j)
    let (w, h) := (x1 - x0 + 1, y1 - y0 + 1)
    let b := bytes fmt p.value x0 y1 w h
    let (lo, hi) := clipRows f
    match js.filter (fun j => lo ≤ j ∧ j < hi) with
    | [] => none
    | j :: rest =>
      let (r0, r1) := (rest.foldl min j, rest.foldl max j)
      let size : Int := if fmt = 0 then 1 else b.length
      pure ([size, w, r1 - r0 + 1, x0, r1 + 1, inc] ++ (if fmt = 0 then [] else b))
  | _, _ => pure [if fmt = 0 then 1 else 0, 1, 1, 0, 0, inc]

/-- one character of TextOut: its GGO_BITMAP pixels within the black box's rows, the baseline
asc below y, at the pen -/
def charPixels (f : Font) (r : List Rect) (pen y : Int) : Option (List (Int × Int)) :=
  (pixels f r 1).map fun v =>
    (v.filter fun ((_, j), _) => (clipRows f).1 ≤ j ∧ j < (clipRows f).2).map fun ((i, j), _) => (pen + i, y + f.px f.a - j - 1)

/-- GDI.TEXT.DRAW: TextOut of characters (each its advance and rectangles) from pen x, top y:
each character's pixels at the pen, the pen moving by each width -/
def text (f : Font) (y : Int) : Int → List (Int × List (Int × Int × Int × Int)) → Option (List (Int × Int))
  | _, [] => some []
  | pen, (adv, r) :: rest => do
    let here ← charPixels f r pen y
    let more ← text f y (pen + width f adv) rest
    pure (here ++ more)

/-- GDI.TEXT.BITMAP, GDI.TEXT.GRAY: every row of the buffer is a whole number of DWORDs, wide
enough for the black box's pixels at the format's bits per pixel -/
theorem rows_dword_aligned (fmt w : Int) (_hw : 0 < w) :
    stride fmt w % 4 = 0 ∧ (if fmt ≤ 1 then w ≤ 8 * stride fmt w else w ≤ stride fmt w) := by
  unfold stride
  split <;> constructor <;> omega

/-- GDI.TEXT.BITMAP, GDI.TEXT.GRAY: the buffer holds the box's rows, a stride each -/
theorem bytes_length (fmt : Int) (v : Int → Int → Int) (x0 y1 w h : Int) :
    (bytes fmt v x0 y1 w h).length = (stride fmt w * h).toNat := by
  simp [bytes]

/-- GDI.TEXT.ITALIC: the shear is 0 on the baseline and odd: a point as far below the baseline
as another is above it moves as far the other way -/
theorem shear_zero : shear 0 = 0 := by decide

theorem shear_odd (y : Int) : shear (-y) = -shear y := by
  unfold shear
  have h1 : (-y) * 0x5700 = -(y * 0x5700) := by omega
  rw [h1]
  split <;> split <;> omega

/-- GDI.TEXT.DRAW: TextOut is each character's pixels at its pen, the next pen the width on -/
theorem text_cons (f : Font) (y pen adv : Int) (r : List Rect) (rest : List (Int × List (Int × Int × Int × Int))) :
    text f y pen ((adv, r) :: rest) = (do
      let here ← charPixels f r pen y
      let more ← text f y (pen + width f adv) rest
      pure (here ++ more)) := by
  rfl

/-- GDI.TEXT.DRAW: every pixel TextOut lights lies in the rows from the ascent to the descent
below the top y -/
theorem char_rows (f : Font) (r : List Rect) (pen y : Int) (ps : List (Int × Int))
    (h : charPixels f r pen y = some ps) (p : Int × Int) (hp : p ∈ ps) :
    y + f.px f.a - (clipRows f).2 ≤ p.2 ∧ p.2 < y + f.px f.a - (clipRows f).1 := by
  unfold charPixels at h
  cases hv : pixels f r 1 with
  | none => simp [hv] at h
  | some v =>
    simp only [hv, Option.map_some, Option.some.injEq] at h
    subst h
    simp only [List.mem_map, List.mem_filter, decide_eq_true_eq] at hp
    obtain ⟨⟨⟨i, j⟩, n⟩, ⟨_, hj⟩, rfl⟩ := hp
    dsimp only at hj ⊢
    omega

/-- the font the italic theorem runs over: ppem 13 on an em of 64 -/
def em64 (bold italic : Bool) : Font := ⟨64, 13, 52, 12, bold, italic⟩

set_option maxRecDepth 100000 in
/-- GDI.TEXT.GRAY: GetGlyphOutline's gray bytes are 0 to 4, 16 or 64 (the samples lit of a
pixel's k x k), bold too -/
theorem gray_levels : ([[(4, 0, 30, 40)], [(0, 0, 3, 50)], [(5, 5, 50, 20), (10, 20, 18, 48)]].all fun r => [false, true].all fun b => [(4, 4), (5, 16), (6, 64)].all fun (fmt, top) =>
    match glyph ⟨64, 3, 52, 12, b, false⟩ r 40 fmt with
    | some l => (l.drop 6).all fun x => decide (0 ≤ x ∧ x ≤ top)
    | none => false) = true := by decide +kernel

set_option maxRecDepth 100000 in
/-- GDI.TEXT.ITALIC: the synthesized italic keeps the upright glyph's black box rows, its
origin's row and its advance (the shear moves pixels along their rows) -/
theorem italic_rows : ([[(4, 0, 30, 40)], [(2, -10, 40, 3)], [(8, 1, 60, 30)], [(3, -12, 33, 20)]].all fun r =>
    match glyph (em64 false false) r 40 1, glyph (em64 false true) r 40 1 with
    | some (_ :: _ :: h :: _ :: oy :: inc :: _), some (_ :: _ :: h' :: _ :: oy' :: inc' :: _) => h == h' && oy == oy' && inc == inc'
    | _, _ => false) = true := by decide +kernel

/-- GDI.TEXT.BITMAP: a stroke thinner than a pixel still lights a pixel of each row (column)
it crosses, by the dropout rule: here a column of two pixels and a row of two at ppem 3, each
row of the buffer a DWORD, the high bit first; GGO_BITMAP's pixels are lit or not, bold too -/
theorem dropout_strokes :
    glyph ⟨64, 3, 52, 12, false, false⟩ [(0, 0, 3, 50)] 64 1 = some [8, 1, 2, 0, 2, 3, 128, 0, 0, 0, 128, 0, 0, 0] ∧
    glyph ⟨64, 3, 52, 12, false, false⟩ [(0, 0, 50, 3)] 64 1 = some [4, 2, 1, 0, 1, 3, 192, 0, 0, 0] ∧
    ([[(4, 0, 30, 40)], [(5, 5, 50, 20), (10, 20, 18, 48)]].all fun r => [false, true].all fun b =>
      match pixels ⟨64, 3, 52, 12, b, false⟩ r 1 with
      | some v => v.all (·.2 == 1)
      | none => false) = true := by
  decide +kernel

theorem italic_width (adv : Int) (b : Bool) (u : Font) :
    width { u with italic := b } adv = width u adv := rfl

/-- GDI.TEXT.ITALIC: the italic's parallelograms are the upright rectangles' corners sheared:
each corner moves along its row by the shear of its height, the upright one not at all -/
theorem corners_sheared (u : Font) (k : Int) (e : Rect) :
    Slant.of { u with italic := true } k e =
      ⟨(u.q e.1 + shear (u.q e.2.1)) * k, (u.q e.1 + shear (u.q e.2.2.2)) * k,
       (u.q e.2.2.1 + shear (u.q e.2.1)) * k, (u.q e.2.2.1 + shear (u.q e.2.2.2)) * k,
       u.q e.2.1 * k, u.q e.2.2.2 * k⟩ ∧
    { u with italic := false }.point e.1 e.2.1 = (u.q e.1, u.q e.2.1) := by
  constructor <;> rfl

/-- the arguments: upm, ppem, ascent, descent, bold, italic, then the request's -/
def font? (xs : List Int) : Option (Font × List Int) :=
  match xs with
  | upm :: ppem :: a :: d :: bold :: it :: rest =>
    if upm ≤ 0 then none else some (⟨upm, ppem, a, d, bold != 0, it != 0⟩, rest)
  | _ => none

/-- n rectangles of four numbers each, and what follows them -/
def rectsOf : Nat → List Int → Option (List (Int × Int × Int × Int) × List Int)
  | 0, xs => some ([], xs)
  | n + 1, x0 :: y0 :: x1 :: y1 :: rest => (rectsOf n rest).map fun (r, t) => ((x0, y0, x1, y1) :: r, t)
  | _, _ => none

/-- the characters: each its advance, its count of rectangles and those -/
def charsOf : Nat → List Int → Option (List (Int × List (Int × Int × Int × Int)))
  | _, [] => some []
  | 0, _ => none
  | fuel + 1, adv :: n :: rest => do
    let (r, rest) ← rectsOf n.toNat rest
    let more ← charsOf fuel rest
    pure ((adv, r) :: more)
  | _, _ => none

def sorted (l : List (Int × Int)) : List (Int × Int) :=
  l.mergeSort fun a b => decide (a.1 < b.1 ∨ (a.1 = b.1 ∧ a.2 ≤ b.2))

def mirrors : List Mirror := [
  ⟨"gdi.glyph", fun xs => do
    let (f, rest) ← font? xs
    match rest with
    | fmt :: adv :: n :: rs => do
      let (r, _) ← rectsOf n.toNat rs
      glyph f r adv fmt
    | _ => none⟩,
  ⟨"gdi.text", fun xs => do
    let (f, rest) ← font? xs
    match rest with
    | x :: y :: cs => do
      let p ← text f y x (← charsOf 64 cs)
      pure ((sorted p.eraseDups).flatMap fun q => [q.1, q.2])
    | _ => none⟩
]

end Kikizake.Gdi.Glyph
