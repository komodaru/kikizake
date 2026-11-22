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

end Kikizake
