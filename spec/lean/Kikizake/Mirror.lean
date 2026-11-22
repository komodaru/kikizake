/-! The mirror's vocabulary: a definition the theorems are about, evaluated on the integers
`kikizake lean` sends (one call per line: its name, then its arguments) and answering with
integers, so the Rust model's function can be held equal to it on the same inputs. -/
namespace Kikizake

/-- one mirrored definition: its arguments and answers as integers; `none` for arguments
outside its domain (the comparison fails on it) -/
structure Mirror where
  name : String
  run : List Int → Option (List Int)

/-- a natural number argument (none if negative) -/
def nat (i : Int) : Option Nat := if i < 0 then none else some i.toNat

/-- a 64-bit natural number from its two 32-bit halves, the high one first (none outside them):
arguments are at most 64-bit signed on the Rust side, so a value of 64 bits comes in halves -/
def nat64 (hi lo : Int) : Option Nat := do
  let h ← nat hi
  let l ← nat lo
  if h < 2 ^ 32 ∧ l < 2 ^ 32 then some (h * 2 ^ 32 + l) else none

/-- an answer below 2^64 as its two 32-bit halves, the high one first -/
def halves (v : Nat) : List Int := [((v / 2 ^ 32 : Nat) : Int), ((v % 2 ^ 32 : Nat) : Int)]

end Kikizake
