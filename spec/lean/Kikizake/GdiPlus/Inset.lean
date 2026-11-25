import Kikizake.Mirror

/-! The pixel intersection and RegionToPath part of GDI+ pen inset. The input masks are
the aliased device pixels of a closed figure's centered double-width outline and of the
whole path. The output figures use the model's corner order and diagonalization rules. -/
namespace Kikizake.GdiPlus.Inset

abbrev Cell := Int × Int
abbrev Edge := Cell × Cell

structure Mask where
  origin : Cell
  width : Nat
  height : Nat
  bits : Nat

def Mask.has (m : Mask) (p : Cell) : Bool :=
  let x := p.1 - m.origin.1
  let y := p.2 - m.origin.2
  decide (0 ≤ x ∧ x < m.width ∧ 0 ≤ y ∧ y < m.height) &&
    m.bits.testBit (y.toNat * m.width + x.toNat)

def Mask.cells (m : Mask) : List Cell :=
  (List.range m.height).flatMap fun y =>
    (List.range m.width).filterMap fun x =>
      if m.bits.testBit (y * m.width + x) then
        some (m.origin.1 + x, m.origin.2 + y)
      else none

def intersection (own whole : Mask) : List Cell :=
  own.cells.filter whole.has

theorem intersection_contained (own whole : Mask) (point : Cell)
    (h : point ∈ intersection own whole) : point ∈ own.cells ∧ whole.has point = true := by
  simpa [intersection, List.mem_filter] using h

private def plus (a b : Cell) : Cell := (a.1 + b.1, a.2 + b.2)
private def minus (a b : Cell) : Cell := (a.1 - b.1, a.2 - b.2)
private def before (a b : Cell) : Bool :=
  a.1 < b.1 || (a.1 == b.1 && a.2 < b.2)

def boundary (cells : List Cell) : List Edge :=
  cells.flatMap fun (x, y) =>
    [((x - 1, y), ((x, y), (0, 1))),
     ((x, y + 1), ((x, y + 1), (1, 0))),
     ((x + 1, y), ((x + 1, y + 1), (0, -1))),
     ((x, y - 1), ((x + 1, y), (-1, 0)))].filterMap fun (neighbor, edge) =>
      if cells.contains neighbor then none else some edge

/-- Take the first outgoing edge in row insertion order on a tie of start coordinates. -/
def startEdge (edges : List Edge) : Option Edge :=
  edges.foldl (fun best edge =>
    match best with
    | none => some edge
    | some current => if before edge.1 current.1 then some edge else best) none

def nextEdge (edges : List Edge) (pos dir : Cell) : Option Edge :=
  let left := (pos, (dir.2, -dir.1))
  if edges.contains left then some left else edges.find? (fun e => e.1 == pos)

def traceLoop (start firstDir : Cell) : Nat → Cell → Cell → List Edge →
    List Cell → Option (List Cell × List Edge)
  | 0, _, _, _, _ => none
  | fuel + 1, pos, dir, edges, corners => do
    let next := plus pos dir
    let first := (start, firstDir)
    let edges := if next == start && !edges.contains first then edges ++ [first] else edges
    let e ← nextEdge edges next dir
    let edges := edges.erase e
    let corners := if e.2 != dir then next :: corners else corners
    if next == start && e.2 == firstDir then
      some (corners.reverse, edges)
    else
      traceLoop start firstDir fuel next e.2 edges corners

def loops : Nat → List Edge → Option (List (List Cell))
  | 0, edges => if edges.isEmpty then some [] else none
  | fuel + 1, edges =>
    match startEdge edges with
    | none => some []
    | some e => do
      let (corners, remaining) ← traceLoop e.1 e.2 (edges.length + 1) e.1 e.2 (edges.erase e) []
      pure (corners :: (← loops fuel remaining))

def topLeft (points : List Cell) : Cell :=
  let beforeRow (a b : Cell) := before (a.2, a.1) (b.2, b.1)
  match points with
  | [] => (0, 0)
  | p :: rest => rest.foldl (fun best q => if beforeRow q best then q else best) p

def signedArea (points : List Cell) : Int :=
  (points.zip (points.drop 1 ++ points.take 1)).foldl
    (fun area (a, b) => area + a.1 * b.2 - b.1 * a.2) 0

def bottomCorner (a q b : Cell) : Bool :=
  let vertical (x y : Cell) := x.1 == y.1 && x.2 != y.2
  let horizontal (x y : Cell) := x.2 == y.2 && x.1 != y.1
  if vertical a q && q.2 > a.2 && horizontal q b then
    q.2 - a.2 == 1 || b.1 - q.1 == -1
  else if vertical q b && q.2 > b.2 && horizontal a q then
    q.2 - b.2 == 1 || a.1 - q.1 == -1
  else false

def sameDirection (a b : Cell) : Bool :=
  a.1 * b.2 - b.1 * a.2 == 0 && a.1 * b.1 + a.2 * b.2 > 0

def diagonalize : Nat → Nat → Array Cell → Array Cell
  | 0, _, points => points
  | fuel + 1, i, points =>
    if i ≥ points.size || points.size ≤ 2 then points else
    let a := points.getD (i - 1) (0, 0)
    let q := points.getD i (0, 0)
    let b := points.getD ((i + 1) % points.size) (0, 0)
    if !bottomCorner a q b then diagonalize fuel (i + 1) points else
    let points := points.eraseIdxIfInBounds i
    let a0 := points.getD (i - 2) (0, 0)
    let a1 := points.getD (i - 1) (0, 0)
    let b := points.getD (i % points.size) (0, 0)
    if i ≥ 3 && sameDirection (minus a1 a0) (minus b a1) then
      diagonalize fuel (i - 1) (points.eraseIdxIfInBounds (i - 1))
    else diagonalize fuel i points

private def firstTouch (corners : List Cell) : Option (Nat × Nat) :=
  corners.zipIdx.findSome? fun (p, end_) =>
    let begin := (corners.take end_).idxOf p
    if begin < end_ then some (begin, end_) else none

private def splitTouchingHoles : Nat → List Cell → List (List Cell)
  | 0, corners => [corners]
  | n + 1, corners =>
    if signedArea corners ≤ 0 then [corners] else
    match firstTouch corners with
    | none => [corners]
    | some (begin, end_) =>
      splitTouchingHoles n (corners.take begin ++ corners.drop end_) ++
      splitTouchingHoles n ((corners.drop begin).take (end_ - begin))

private def contourBounds (corners : List Cell) : Cell × Cell :=
  let first := corners.head?.getD (0, 0)
  corners.foldl (fun (lo, hi) (x, y) =>
    ((min lo.1 x, min lo.2 y), (max hi.1 x, max hi.2 y))) (first, first)

private def oddDepth (own : Cell × Cell) (bounds : List (Cell × Cell)) : Bool :=
  let ((left, top), (right, bottom)) := own
  let count := (bounds.filter fun other =>
    let ((x0, y0), (x1, y1)) := other
    own != other && x0 ≤ left && y0 ≤ top && right ≤ x1 && bottom ≤ y1).length
  count % 2 == 1

def outline (corners : List Cell) (reverseOuter : Bool) : List Cell :=
  let hole := signedArea corners > 0
  let corners := if hole then corners.reverse else corners
  let begin := corners.idxOf (topLeft corners)
  let ordered := corners.drop begin ++ corners.take begin
  let path := (diagonalize (2 * ordered.length) 1 ordered.toArray).toList
  if hole || reverseOuter then path.reverse else path

def rowKey (path : List Cell) : Cell :=
  let (x, y) := topLeft path
  (y, x)

def insertByTopLeft (path : List Cell) : List (List Cell) → List (List Cell)
  | [] => [path]
  | head :: tail =>
    if before (rowKey path) (rowKey head) then path :: head :: tail
    else head :: insertByTopLeft path tail

def toPath (cells : List Cell) : Option (List (List Cell)) :=
  match cells with
  | [] => some [List.replicate 4 (0, 0)]
  | first :: rest => do
    let (x0, x1) := rest.foldl (fun (lo, hi) p => (min lo p.1, max hi p.1)) (first.1, first.1)
    let (y0, y1) := (first.2, (rest.getLast?.getD first).2)
    if (x1 - x0 + 1) * (y1 - y0 + 1) == cells.length then
      return [[(x0, y0), (x1 + 1, y0), (x1 + 1, y1 + 1), (x0, y1 + 1)]]
    let edges := boundary cells
    let paths ← loops edges.length edges
    let paths := paths.flatMap fun p => splitTouchingHoles p.length p
    let paths := paths.foldl (fun acc path => insertByTopLeft path acc) []
    let bounds := paths.map contourBounds
    pure ((paths.zip bounds).map fun (p, own) => outline p (oddDepth own bounds))

def insetRegion (own whole : Mask) : Option (List (List Cell)) :=
  toPath (intersection own whole)

theorem empty_region (whole : Mask) (x y : Int) (n : Nat) :
    insetRegion ⟨(x, y), 0, n, 0⟩ whole = some [[(0, 0), (0, 0), (0, 0), (0, 0)]] := by
  have h : (List.range n).flatMap (fun _ : Nat => ([] : List Cell)) = [] := by simp
  simp [insetRegion, intersection, Mask.cells, h, toPath]

/-- The common columns of two overlapping rectangles form one east-first boundary. -/
theorem rectangle_intersection :
    insetRegion ⟨(0, 0), 3, 2, 63⟩ ⟨(1, 0), 3, 2, 63⟩ =
      some [[(1, 0), (3, 0), (3, 2), (1, 2)]] := by
  decide +kernel

/-- Corner-touching inside pixels make separate contours. -/
theorem four_connected :
    insetRegion ⟨(0, 0), 2, 2, 9⟩ ⟨(0, 0), 2, 2, 15⟩ =
      some [[(0, 0), (1, 1), (1, 0)], [(1, 1), (2, 2), (2, 1)]] := by
  decide +kernel

/-- The central hole's contour winds oppositely to the outer contour. -/
theorem hole_reverses_winding :
    insetRegion ⟨(0, 0), 3, 3, 495⟩ ⟨(0, 0), 3, 3, 511⟩ =
      some [[(0, 0), (0, 3), (3, 3), (3, 0)], [(2, 1), (2, 2), (1, 1)]] ∧
      signedArea [(0, 0), (0, 3), (3, 3), (3, 0)] < 0 ∧
      signedArea [(2, 1), (2, 2), (1, 1)] > 0 := by
  decide +kernel

/-- A disjoint island inside another contour's bounds is emitted in reverse order. -/
theorem disjoint_island_inside_bounds :
    insetRegion ⟨(0, 0), 3, 3, 245⟩ ⟨(0, 0), 3, 3, 511⟩ =
      some [[(1, 0), (1, 1), (0, 0)], [(2, 0), (1, 1), (0, 2), (2, 3), (2, 2), (3, 0)]] := by
  decide +kernel

/-- A second enclosing box restores the island's original orientation. -/
theorem double_enclosure_keeps_orientation :
    insetRegion ⟨(0, 0), 5, 5, 33084991⟩ ⟨(0, 0), 5, 5, 33554431⟩ =
      some [[(0, 0), (0, 5), (5, 5), (5, 0)], [(4, 1), (4, 4), (1, 4), (1, 1)],
        [(2, 2), (3, 3), (3, 2)]] := by
  decide +kernel

theorem corner_touching_holes_are_separate :
    insetRegion ⟨(0, 0), 5, 5, 20441820⟩ ⟨(0, 0), 5, 5, 33554431⟩ =
      some [[(2, 0), (1, 1), (0, 3), (0, 5), (4, 3), (5, 0)],
        [(4, 1), (4, 2), (3, 1)], [(3, 2), (3, 3), (2, 2)], [(5, 4), (5, 5), (4, 4)]] := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.inset-region", fun
    | [ax, ay, aw, ah, a, bx, by_, bw, bh, b] => do
      let own : Mask := ⟨(ax, ay), ← nat aw, ← nat ah, ← nat a⟩
      let whole : Mask := ⟨(bx, by_), ← nat bw, ← nat bh, ← nat b⟩
      let figures ← insetRegion own whole
      pure (figures.flatMap fun pts => [Int.ofNat pts.length] ++ pts.flatMap fun (x, y) => [x, y])
    | _ => none⟩
]

end Kikizake.GdiPlus.Inset
