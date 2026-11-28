import Kikizake.GdiPlus.Chain
import Kikizake.GdiPlus.Wide

/-! The circular trim of an open endpoint before placing an anchor. The mirror
compares the edited points, their marks, and the direction used to place the cap. -/
namespace Kikizake.GdiPlus.Anchor

open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Linear (add64 mul64 div64 to32)
open Kikizake.GdiPlus.Chain (sqrt64)
open Kikizake.GdiPlus.Wide (Pair)

private abbrev D := F64.fmt
private def eps : Nat := 0x34000000
private def eps64 : Nat := F64.ofF32 eps
private def le64 (x y : Nat) : Bool := !isNaN D x && !isNaN D y && !F64.lt y x
private def sq (r : Ops) (x : Nat) : Nat := r.mul x x

private def intersection (d : Bool) (c : Pair) (r2 : Nat) (a b : Pair) : Option Pair := do
  let r := ops d
  let (dx, dy) := (r.sub b.1 a.1, r.sub b.2 a.2)
  let ld := sqrt64 d (F64.ofF32 (r.add (sq r dy) (sq r dx)))
  if F64.lt ld eps64 then none else
  let inv := to32 d (div64 d 0x3ff0000000000000 ld)
  let (ex, ey) := (r.sub c.1 a.1, r.sub c.2 a.2)
  let (ux, uy) := (r.mul dx inv, r.mul dy inv)
  let cc := F64.ofF32 (r.add (sq r ey) (sq r ex))
  let p := F64.ofF32 (r.add (r.mul ey uy) (r.mul ex ux))
  let radius := F64.ofF32 r2
  if F64.lt p eps64 && le64 radius cc then none else
  let disc := add64 d (add64 d radius (neg D cc)) (mul64 d p p)
  if F64.lt disc eps64 then none else
  let root := sqrt64 d disc
  let near := add64 d p (neg D root)
  let far := add64 d root p
  let t ← if le64 radius cc && F64.lt eps64 near then some near
    else if F64.lt eps64 far then some far
    else none
  let tf := to32 d t
  some (r.add (r.mul tf ux) a.1, r.add (r.mul tf uy) a.2)

private def direction (d : Bool) (x y : Nat) : Pair :=
  let r := ops d
  let (x64, y64) := (F64.ofF32 x, F64.ofF32 y)
  let v := add64 d (mul64 d x64 x64) (mul64 d y64 y64)
  let len := sqrt64 d v
  if F64.lt v 0 || F64.lt len eps64 then (0, 0)
  else
    let l := to32 d len
    (r.div x l, r.div y l)

abbrev Marked := Pair × Bool

def trim (d fromEnd : Bool) (points : List Marked) (length base scale : Nat) :
    Option (Pair × List Marked) := do
  let r := ops d
  let path := if fromEnd then points.reverse else points
  let first ← path.head?
  let e := first.1
  let r2 := sq r (r.mul length scale)
  let inset := if F32.lt (length % 0x80000000) eps then 0 else r.div base length
  let distance (p : Marked) :=
    r.add (sq r (r.sub p.1.1 e.1)) (sq r (r.sub p.1.2 e.2))
  let inside := path.takeWhile fun p => !F32.lt r2 (distance p)
  let outside := path.drop inside.length
  let current ← inside.getLast?
  let q := current.1
  let a := (outside.head?.map Prod.fst).getD q
  let x := (intersection d e r2 a q).getD q
  let g := direction d (r.sub x.1 e.1) (r.sub x.2 e.2)
  let k := r.sub one inset
  let adjusted := (r.add (r.mul (r.sub e.1 x.1) k) x.1, r.add (r.mul (r.sub e.2 x.2) k) x.2)
  let edited := inside.dropLast.map (fun p => (p.1, true)) ++
    [(adjusted, outside.isEmpty || current.2)] ++ outside
  some (g, if fromEnd then edited.reverse else edited)

private def line : List Marked := [((0, 0), false), ((0x40800000, 0), false)]

set_option exponentiation.threshold 2048
set_option maxRecDepth 20000

theorem half_inset_on_horizontal_line :
    trim false false line one 0x3f000000 one =
      some ((one, 0), [((0x3f000000, 0), false), ((0x40800000, 0), false)]) := by
  decide +kernel

theorem short_line_is_marked :
    trim false false line one 0x3f000000 0x41000000 =
      some ((one, 0), [((0, 0), true), ((0x40000000, 0), true)]) := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.anchor-trim", fun args => do
    let d ← args[0]?
    let fromEnd ← args[1]?
    let l ← nat (← args[2]?)
    let base ← nat (← args[3]?)
    let s ← nat (← args[4]?)
    let raw := args.drop 5
    if raw.isEmpty || raw.length % 3 != 0 then none else
      let ps ← (List.range (raw.length / 3)).mapM fun i => do
        let x ← nat (← raw[3 * i]?)
        let y ← nat (← raw[3 * i + 1]?)
        let m ← raw[3 * i + 2]?
        pure ((x, y), m != 0)
      let (g, edited) ← trim (d != 0) (fromEnd != 0) ps l base s
      let result := [F32.canon g.1, F32.canon g.2] ++
        edited.flatMap fun ((x, y), marked) => [F32.canon x, F32.canon y, if marked then 1 else 0]
      pure (result.map Int.ofNat)⟩
]

end Kikizake.GdiPlus.Anchor
