import Kikizake.GdiPlus.Chain

/-! GDI+ dash normalization and the dash walker from GP.PEN.DASH. The walker has explicit fuel
so the executable mirror reports a failure if a sampled path cannot be fully consumed. -/
namespace Kikizake.GdiPlus.Dash

open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Linear

private abbrev Point := Nat × Nat
private abbrev F := F32.fmt

def dashUnit (r : Ops) (width major minimum : Nat) : Nat :=
  if flt (r.mul major width) minimum then r.div minimum major else width

def patternAtUnit (r : Ops) (unit : Nat) (adjust : Bool) (pat : List Nat) : Array Nat × Nat :=
  let padded := if pat.length % 2 = 1 then pat ++ [0] else pat
  let inset := if adjust then r.mul unit 0x3f000000 else 0
  let gap := r.add inset inset
  let floor := r.mul unit 0x3a83126f
  let lengths := padded.zipIdx |>.map fun (v, i) =>
    let length := r.mul v unit
    if flt 0 gap then
      if i % 2 = 0 then
        let shortened := r.sub length gap
        if flt shortened floor then floor else shortened
      else r.add length gap
    else length
  (lengths.toArray, gap)

def scaledPattern (r : Ops) (width major : Nat) (adjust : Bool) (pat : List Nat) : Array Nat × Nat :=
  patternAtUnit r (dashUnit r width major one) adjust pat

def truncate (v : Nat) : Nat :=
  let (negative, a, b) := toRat F v
  let n := a / b
  if !finite v || n ≥ 2 ^ 31 then 0xcf000000
  else if negative then neg F (F32.ofNat n) else F32.ofNat n

def modulo (r : Ops) (x m : Nat) : Nat :=
  let part y := r.sub y (r.mul (truncate (r.div y m)) m)
  if flt 0 x then part x
  else if flt x 0 then
    let v := part (neg F x)
    if flt 0 v then r.sub m v else v
  else 0

def segmentLength (d : Bool) (r : Ops) (a b : Point) : Nat :=
  let (dx, dy) := (r.sub b.1 a.1, r.sub b.2 a.2)
  Stroke.sqrt32 d (r.add (r.mul dy dy) (r.mul dx dx))

def closingLength (d : Bool) (r : Ops) (a b : Point) : Nat :=
  let (dx, dy) := (F64.ofF32 (r.sub b.1 a.1), F64.ofF32 (r.sub b.2 a.2))
  let length := Chain.sqrt64 d (add64 d (mul64 d dx dx) (mul64 d dy dy))
  if F64.lt length (F64.ofF32 epsF) then 0 else to32 d length

def samePoint (a b : Point) : Bool :=
  let eq x y := x == y || (isZero x && isZero y)
  eq a.1 b.1 && eq a.2 b.2

def closePoints (a b : Point) : Bool :=
  flt (fabs (F32.sub a.1 b.1)) epsF && flt (fabs (F32.sub a.2 b.2)) epsF

structure PatternCursor where
  idx : Nat
  rem : Nat
  on : Bool

def mergeClosingGap (r : Ops) (lengths : Array Nat) (cursor : PatternCursor) : PatternCursor :=
  if isZero (lengths.getD cursor.idx 0) && cursor.on && cursor.idx + 1 == lengths.size then
    { cursor with idx := 1 % lengths.size, rem := r.add cursor.rem (lengths.getD 0 0) }
  else cursor

def startCursor (r : Ops) (lengths : Array Nat) (offset : Nat) : PatternCursor := Id.run do
  let period := lengths.foldl r.add 0
  let mut rem := modulo r offset period
  let mut idx := 0
  for length in lengths do
    if flt rem length then
      rem := r.sub length rem
      break
    rem := r.sub rem length
    idx := idx + 1
  return mergeClosingGap r lengths ⟨(idx + 1) % lengths.size, rem, idx % 2 == 0⟩

structure WalkState where
  cursor : PatternCursor
  prev : Point
  seg : Nat
  vertex : Nat
  extendDash : Bool
  out : Array (Array Point)

def pushSegment (s : WalkState) (q : Point) : Array (Array Point) :=
  if s.extendDash then s.out.modify (s.out.size - 1) (·.push q)
  else s.out.push #[s.prev, q]

def advance (r : Ops) (p : Array Point) (lengths segLengths : Array Nat) (s : WalkState) : WalkState :=
  let endPt := p.getD s.vertex (0, 0)
  if isZero s.seg || flt s.seg s.cursor.rem then
    let visible := s.cursor.on && !closePoints s.prev endPt
    let next := s.vertex + 1
    { cursor := { s.cursor with rem := r.sub s.cursor.rem s.seg }
      prev := endPt, seg := segLengths.getD (next % p.size) 0, vertex := next
      extendDash := if s.cursor.on then visible else s.extendDash
      out := if visible then pushSegment s endPt else s.out }
  else
    let t := r.div s.cursor.rem s.seg
    let q := (r.add (r.mul (r.sub endPt.1 s.prev.1) t) s.prev.1,
              r.add (r.mul (r.sub endPt.2 s.prev.2) t) s.prev.2)
    let idx := s.cursor.idx
    let cursor := mergeClosingGap r lengths ⟨(idx + 1) % lengths.size, lengths.getD idx 0, idx % 2 == 0⟩
    { s with cursor, prev := q, seg := r.sub s.seg s.cursor.rem, extendDash := false
             out := if s.cursor.on && !closePoints s.prev q then pushSegment s q else s.out }

def walkSteps (r : Ops) (p : Array Point) (lengths segLengths : Array Nat) :
    Nat → WalkState → Option (Array (Array Point))
  | 0, s => if s.vertex ≥ p.size then some s.out else none
  | fuel + 1, s =>
    if s.vertex ≥ p.size then some s.out
    else walkSteps r p lengths segLengths fuel (advance r p lengths segLengths s)

def capDistance (r : Ops) (a b : Point) : Nat :=
  let (dx, dy) := (r.sub a.1 b.1, r.sub a.2 b.2)
  r.add (r.mul dx dx) (r.mul dy dy)

structure Span where
  points : Array Point
  startCap : Nat
  endCap : Nat

def walk (fuel : Nat) (d closed : Bool) (p : Array Point) (lengths : Array Nat)
    (off width dashCap startCap endCap : Nat) : Option (List Span) := do
  if p.size < 2 then return []
  let r := ops d
  let first := p.getD 0 (0, 0)
  let last := p.getD (p.size - 1) first
  let segLengths := (List.range p.size).map fun i =>
    segmentLength d r (if i == 0 then last else p.getD (i-1) first) (p.getD i first)
  let p := if closed && samePoint first last then p.pop else p
  let (prev, seg, vertex) :=
    if closed then
      let prev := p.getD (p.size - 1) first
      (prev, closingLength d r prev first, 0)
    else (first, segLengths.getD 1 0, 1)
  let state := ⟨startCursor r lengths off, prev, seg, vertex, false, #[]⟩
  let spans ← walkSteps r p lengths segLengths.toArray fuel state
  let threshold := r.mul (r.mul width width) 0x3e800000
  return spans.toList.zipIdx.map fun (pts, i) =>
    let start := if !closed && i == 0 && flt (capDistance r first (pts.getD 0 first)) threshold then startCap else dashCap
    let end_ := if !closed && i + 1 == spans.size && flt (capDistance r last (pts.getD (pts.size-1) last)) threshold then endCap else dashCap
    ⟨pts, start, end_⟩

def dashSpans (fuel : Nat) (d closed adjust : Bool) (width major offset dashCap startCap endCap : Nat)
    (pat : List Nat) (p : Array Point) : Option (List Span) :=
  let r := ops d
  let (lengths, gap) := scaledPattern r width major (adjust && (dashCap == 2 || dashCap == 3)) pat
  let off := r.sub (r.mul width offset) (if closed then gap else 0)
  walk fuel d closed p lengths off width dashCap startCap endCap

/-- Encode each open dash with its selected caps and points for the dash mirror. -/
def dashes (fuel : Nat) (d closed adjust : Bool) (width major offset dashCap startCap endCap : Nat)
    (pat : List Nat) (p : Array Point) : Option (List Nat) :=
  (dashSpans fuel d closed adjust width major offset dashCap startCap endCap pat p).map
    (·.flatMap fun s => [s.startCap, s.endCap, s.points.size] ++
      s.points.toList.flatMap fun (x, y) => [F32.canon x, F32.canon y])

/-- Normalization leaves a whole number of dash-gap pairs, including for odd custom patterns. -/
theorem normalized_pattern_is_even (r : Ops) (width major : Nat) (adjust : Bool) (pat : List Nat) :
    (scaledPattern r width major adjust pat).1.size % 2 = 0 := by
  simp only [scaledPattern, patternAtUnit, List.size_toArray, List.length_map, List.length_zipIdx]
  split
  · simp only [List.length_append, List.length_cons, List.length_nil]
    omega
  · omega

/-- A path with fewer than two points produces no dash spans. -/
theorem short_path_has_no_dashes (fuel : Nat) (d closed adjust : Bool)
    (width major offset dashCap startCap endCap : Nat) (pat : List Nat) (p : Array Point)
    (h : p.size < 2) :
    dashes fuel d closed adjust width major offset dashCap startCap endCap pat p = some [] := by
  simp [dashes, dashSpans, walk, h]

/-- An open two-point path uses the line cap at the first endpoint and the dash cap at each cut. -/
theorem horizontal_caps :
    dashes 16 false false false one one 0 2 1 3 [0x40400000, one] #[(one, one), (0x41100000, one)] =
      some [1, 2, 2, one, one, 0x40800000, one, 2, 2, 2, 0x40a00000, one, 0x41000000, one] := by
  decide +kernel

/-- Compensating a round dash cap for a unit-width pen cuts a two-unit dash to one unit. -/
theorem adjusted_round_cap :
    dashes 16 false false true one one 0 2 1 3 [0x40000000, 0x40000000] #[(one, one), (0x40a00000, one)] =
      some [1, 2, 2, one, one, 0x40000000, one] := by
  decide +kernel

/-- A one-element pattern can reach a vertex with no segment or gap left. -/
theorem zero_gap_at_vertex :
    dashes 16 false false false one one 0 0 0 0 [one]
      #[(0, 0), (0x40000000, 0), (0x40400000, 0)] =
      some [0, 0, 2, 0, 0, 0x40000000, 0, 0, 0, 2, 0x40000000, 0, 0x40400000, 0] := by
  decide +kernel

def pointPairs : List Nat → Option (List Point)
  | [] => some []
  | x :: y :: rest => (pointPairs rest).map ((x, y) :: ·)
  | [_] => none

def mirrors : List Mirror := [
  ⟨"gp.dashes", fun
    | dd :: closed :: adjust :: width :: major :: offset :: cap :: start :: end_ :: count :: rest => do
      let count ← nat count
      let _ ← if count == 0 || rest.length < count then none else some ()
      let pat ← (rest.take count).mapM nat
      let points ← pointPairs (← (rest.drop count).mapM nat)
      let result ← dashes 8192 (dd != 0) (closed != 0) (adjust != 0) (← nat width) (← nat major) (← nat offset)
        (← nat cap) (← nat start) (← nat end_) pat points.toArray
      pure (result.map Int.ofNat)
    | _ => none⟩
]

end Kikizake.GdiPlus.Dash
