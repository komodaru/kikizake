import Kikizake.Mirror
/-! The output merger's write mask on Direct3D 10 and 11 (spec/d3dres.txt D3DRASTER.WRITEMASK), as
src/model/d3drules.rs `merge` computes it (mirror `d3d11.merge`): an element is its bytes as one
little-endian number; a format's components are (their bits, their channel: 0 red, 1 green,
2 blue, 3 alpha) from bit 0 on. -/
namespace Kikizake.D3d11.Merge

/-- the bits of a component of `b` bits from bit `pos` -/
def field (pos b : Nat) : Nat := (2 ^ b - 1) <<< pos

/-- the mask names channel c -/
def names (mask c : Nat) : Bool := (mask >>> c) % 2 == 1

/-- the components from bit `pos` on merged into acc: a named one's bits from new -/
def go (mask new : Nat) : List (Nat × Nat) → Nat → Nat → Nat
  | [], _, acc => acc
  | (b, c) :: cs, pos, acc =>
    go mask new cs (pos + b) (if names mask c then (acc ^^^ (acc &&& field pos b)) ||| (new &&& field pos b) else acc)

/-- D3DRASTER.WRITEMASK: an element of `len` bytes written through the mask: the bits of each
component the mask names from new, the others kept from old -/
def merge (comps : List (Nat × Nat)) (len old new mask : Nat) : Nat :=
  go mask new comps 0 old % 2 ^ (8 * len)

private theorem go_keep (mask new : Nat) (cs : List (Nat × Nat)) (h : ∀ p ∈ cs, names mask p.2 = false) :
    ∀ pos acc, go mask new cs pos acc = acc := by
  induction cs with
  | nil => intro _ _; rfl
  | cons p cs ih =>
    intro pos acc
    obtain ⟨b, c⟩ := p
    have hc : names mask c = false := h (b, c) (List.mem_cons_self ..)
    simp only [go, hc, Bool.false_eq_true, ite_false]
    exact ih (fun q hq => h q (List.mem_cons_of_mem _ hq)) _ _

/-- D3DRASTER.WRITEMASK: a mask naming none of the format's channels writes nothing -/
theorem merge_none (comps : List (Nat × Nat)) (len old new mask : Nat) (ho : old < 2 ^ (8 * len))
    (h : ∀ p ∈ comps, names mask p.2 = false) : merge comps len old new mask = old := by
  simp [merge, go_keep mask new comps h, Nat.mod_eq_of_lt ho]

/-- D3DRASTER.WRITEMASK: the empty mask writes nothing -/
theorem merge_zero (comps : List (Nat × Nat)) (len old new : Nat) (ho : old < 2 ^ (8 * len)) :
    merge comps len old new 0 = old :=
  merge_none comps len old new 0 ho (fun _ _ => by simp [names])

/-- the bytes of a number, `len` of them, and back -/
def bytes (len x : Nat) : List Nat := (List.range len).map fun i => (x >>> (8 * i)) % 256
def ofBytes (bs : List Nat) : Nat := bs.foldr (fun b acc => b + 256 * acc) 0

/-- the mirror: [the components' count, (bits, channel) each, the bytes' count, old's bytes,
new's bytes, the mask] -/
def mirrors : List Mirror := [
  ⟨"d3d11.merge", fun a => do
    let n ← nat (← a.head?)
    let rest := a.drop 1
    let cs ← (List.range n).mapM fun i => do pure (← nat (← rest[2 * i]?), ← nat (← rest[2 * i + 1]?))
    let rest := rest.drop (2 * n)
    let len ← nat (← rest.head?)
    let rest := rest.drop 1
    let old ← (rest.take len).mapM nat
    let new ← ((rest.drop len).take len).mapM nat
    let mask ← nat (← rest[2 * len]?)
    if rest.length != 2 * len + 1 || old.length != len || new.length != len then none
    pure ((bytes len (merge cs len (ofBytes old) (ofBytes new) mask)).map fun x : Nat => (x : Int))⟩
]

end Kikizake.D3d11.Merge
