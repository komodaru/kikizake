import Kikizake.GdiPlus.Flatten
/-! GDI+'s scan converter (spec/gdiplus.txt GP.RASTER.FILL) as src/model/gppath.rs `fill` computes
it: `device` the world to device matrix (a half pixel offset in its translation) times 16, the
figures through it ceiled to 28.4 and their Beziers flattened (GdiPlus.Flatten `enumFig`), each
figure closed into edges (`Edge.new`: the first row ceiled, x ceiled and stepped by an exact
integer DDA), scanned row by row with Alternate or Winding, aliased or on a lattice of 8 x n
subsamples (`rasterize`); mirror `gp.fill`. -/
namespace Kikizake.GdiPlus.Raster
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten

private abbrev F := F32.fmt

/-- an edge: its x and error at the current row, the steps, its rows start .. end (exclusive),
its winding -/
structure Edge where
  x : Int
  dx : Int
  err : Int
  up : Int
  down : Int
  start : Int
  stop : Int
  wind : Int

/-- the DDA's first steps to row sy's next 1/16: the error and x after k of them -/
def lead : Nat → Int → Int → Int → Int → Int → Int × Int
  | 0, err, x, _, _, _ => (err, x)
  | k + 1, err, x, q, rest, dy =>
    let err := err + rest
    if err ≥ 0 then lead k (err - dy) (x + q + 1) q rest dy else lead k err (x + q) q rest dy

/-- Edge::new: an edge from p0 to p1 (lattice units of 1/16 of a row), none when it crosses no
row -/
def Edge.new (p0 p1 : Int × Int) : Option Edge :=
  let (x0, y0) := p0
  let (x1, y1) := p1
  let (wind, sx, sy, dx, dy, ys, ye) :=
    if y1 - y0 ≥ 0 then ((1 : Int), x0, y0, x1 - x0, y1 - y0, (y0 + 15) / 16, (y1 + 15) / 16)
    else (-1, x1, y1, x0 - x1, y0 - y1, (y1 + 15) / 16, (y0 + 15) / 16)
  if ye ≤ ys then none else
  let q := dx / dy
  let rest := dx % dy
  let f := sy % 16
  let (err, x) := if f ≠ 0 then lead (16 - f).toNat (-1) sx q rest dy else (-1, sx)
  let f := x % 16
  let (err, x) := if f ≠ 0 then (err - (16 - f) * dy, x + 15) else (err, x)
  some ⟨x / 16, q, err / 16, rest, dy, ys, ye, wind⟩

/-- k rows down at once -/
def Edge.skip (e : Edge) (k : Int) : Edge :=
  let v := e.err + e.up * k
  let carries := (v + e.down) / e.down
  { e with x := e.x + e.dx * k + carries, err := v - carries * e.down }

/-- one row down -/
def Edge.step (e : Edge) : Edge :=
  let x := e.x + e.dx
  let err := e.err + e.up
  if err ≥ 0 then { e with x := x + 1, err := err - e.down } else { e with x := x, err := err }

/-- the spans of a row's active edges (sorted by x): Alternate between pairs, Winding where the sum
is nonzero -/
def spans (winding : Bool) (act : List Edge) : List (Int × Int) :=
  if !winding then
    let rec pairs : List Edge → List (Int × Int)
      | a :: b :: rest => (if b.x > a.x then [(a.x, b.x)] else []) ++ pairs rest
      | _ => []
    pairs act
  else
    (act.foldl (fun (acc : Int × Int × List (Int × Int)) e =>
      let (sum, frm, out) := acc
      let was := sum
      let sum := sum + e.wind
      if was = 0 ∧ sum ≠ 0 then (sum, e.x, out)
      else if was ≠ 0 ∧ sum = 0 ∧ e.x > frm then (sum, frm, out ++ [(frm, e.x)])
      else (sum, frm, out)) (0, 0, [])).2.2

/-- a stable sort by key (equal keys in their order), as Rust's sort_by_key -/
def insertBy (key : Edge → Int) (x : Edge) : List Edge → List Edge
  | [] => [x]
  | y :: ys => if key y ≤ key x then y :: insertBy key x ys else x :: y :: ys
def sortBy (key : Edge → Int) (l : List Edge) : List Edge := l.foldl (fun acc x => insertBy key x acc) []

/-- RasterizePath: each pixel's count of covered samples (1 a pixel aliased, 8 x n antialiased) -/
def rasterize (figs : List (List (Int × Int))) (winding : Bool) (rows : Option Nat) (w h : Nat) : Array Nat :=
  let lattice (p : Int × Int) : Int × Int := match rows with
    | none => p
    | some n => (p.1 * 8 + 0x40, (p.2 + 8) * 2 ^ (Nat.log2 n))
  let edges := figs.flatMap fun f =>
    let n := f.length
    (List.range n).filterMap fun i => Edge.new (lattice (f.getD i (0, 0))) (lattice (f.getD ((i + 1) % n) (0, 0)))
  let (sx, sy) : Nat × Nat := match rows with | none => (1, 1) | some n => (8, n)
  let cols : Int := w * sx
  let lines : Int := h * sy
  let count0 : Array Nat := Array.replicate (w * h) 0
  match edges.map (·.start) |>.min? with
  | none => count0
  | some first =>
    let first := max first 0
    let last := min ((edges.map (·.stop)).max?.getD first) lines
    let pending := sortBy (·.start) edges
    let rec scan (fuel : Nat) (y : Int) (pending act : List Edge) (count : Array Nat) : Array Nat :=
      match fuel with
      | 0 => count
      | fuel + 1 =>
        if y ≥ last then count else
        let now := pending.takeWhile (·.start ≤ y)
        let pending := pending.dropWhile (·.start ≤ y)
        let act := act ++ (now.filter (·.stop > y)).map (fun e => e.skip (y - e.start))
        let act := sortBy (·.x) act
        let count := (spans winding act).foldl (fun c (a, b) =>
          let (a, b) := (max a 0, min b cols)
          (List.range (b - a).toNat).foldl (fun c (k : Nat) =>
            let x := a + (k : Int)
            let i := (y / sy * w + x / sx).toNat
            c.modify i (· + 1)) c) count
        let act := (act.filter (·.stop > y + 1)).map Edge.step
        scan fuel (y + 1) pending act count
    scan ((last - first).toNat + 1) first pending [] count0

/-- the device matrix: the world transform (or the identity), a half pixel offset its translation
less 1/2, times 16 -/
def device (r : Ops) (world : Option (List Nat)) (halfOffset : Bool) : List Nat :=
  let m := world.getD [one, 0, 0, one, 0, 0]
  let m := if halfOffset then m.set 4 (r.add (m.getD 4 0) 0xbf000000) |>.set 5 (r.add (m.getD 5 0) 0xbf000000) else m
  m.map (r.mul · k16)

/-- GP.RASTER.FILL: a path filled; none for a Bezier cut short -/
def fill (r : Ops) (ps : List Pt) (world : Option (List Nat)) (winding : Bool) (rows : Option Nat) (halfOffset : Bool) (w h : Nat) : Option (Array Nat) := do
  let m := device r world halfOffset
  let figs ← (figures ps).mapM (enumFig r m)
  pure (rasterize figs winding rows w h)

/-- GP.RASTER.FILL: the square (1, 1) - (3, 3) aliased covers its 4 pixels; antialiased 8 x 4, the
lattice half a pixel over, the square (0.5, 0.5) - (1.5, 1.5) covers all 32 samples of pixel (1, 1); a square and the
same square reversed cancel under Winding and pair off under Alternate -/
theorem squares :
    rasterize [[(16, 16), (48, 16), (48, 48), (16, 48)]] false none 4 4 =
      #[0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0] ∧
    rasterize [[(8, 8), (24, 8), (24, 24), (8, 24)]] true (some 4) 2 2 = #[0, 0, 0, 32] ∧
    rasterize [[(0, 0), (32, 0), (32, 32), (0, 32)], [(0, 0), (0, 32), (32, 32), (32, 0)]] true none 2 2 = #[0, 0, 0, 0] ∧
    rasterize [[(0, 0), (32, 0), (32, 32), (0, 32)], [(0, 0), (0, 32), (32, 32), (32, 0)]] false none 2 2 = #[0, 0, 0, 0] := by
  decide +kernel

private def pts : List Int → Option (List Pt)
  | [] => some []
  | x :: y :: t :: rest => do pure (⟨← nat x, ← nat y, (← nat t) % 128, (← nat t) ≥ 128⟩ :: (← pts rest))
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.fill", fun
    | d :: half :: hasM :: a :: b :: c :: e :: f :: g :: wind :: rows :: w :: h :: rest => do
      let m ← if hasM = 0 then pure none else some <$> [a, b, c, e, f, g].mapM nat
      let rows ← if rows = 0 then pure none else some <$> nat rows
      match fill (ops (d != 0)) (← pts rest) m (wind != 0) rows (half != 0) (← nat w) (← nat h) with
      | none => pure [-1]
      | some cov => pure (cov.toList.map fun (v : Nat) => (v : Int))
    | _ => none⟩
]

end Kikizake.GdiPlus.Raster
