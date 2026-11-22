import Kikizake
/-! `mirror`: reads calls (a mirrored definition's name and its integer arguments, one per line)
from standard input and prints each one's answer on a line of its own (`kikizake lean`). -/
open Kikizake

def mirrors : List Mirror := Gdi.Blend.mirrors

def answer (line : String) : String :=
  match ((line.replace "\n" "").replace "\r" "").splitOn " " with
  | name :: args =>
    match mirrors.find? (·.name == name), args.mapM String.toInt? with
    | some m, some xs => match m.run xs with
      | some r => " ".intercalate (r.map toString)
      | none => "outside"
    | none, _ => "unknown " ++ name
    | _, none => "malformed"
  | [] => "malformed"

partial def loop (i : IO.FS.Stream) (o : IO.FS.Stream) : IO Unit := do
  let line ← i.getLine
  if line.isEmpty then return
  o.putStrLn (answer line)
  loop i o

def main : IO Unit := do
  loop (← IO.getStdin) (← IO.getStdout)
