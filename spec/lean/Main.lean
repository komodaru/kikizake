import Kikizake
/-! `mirror`: reads calls (a mirrored definition's name and its integer arguments, one per line)
from standard input and prints each one's answer on a line of its own (`kikizake lean`). -/
open Kikizake

def mirrors : List Mirror :=
  Gdi.Blend.mirrors ++ Gdi.Stretch.mirrors ++ Gdi.Xform.mirrors ++ Gdi.Gradient.mirrors ++
  Gdi.Hfd.mirrors ++
  Gdi.Glyph.mirrors ++
  Gdi.Arc.mirrors ++
  GdiPlus.Conv.mirrors ++ D3d12.Footprint.mirrors ++ D3d12.Alloc.mirrors ++ D3d12.Create.mirrors ++ D3d12.Copyable.mirrors ++ Dxgi.Args.mirrors ++
  D3d9.Pixel.mirrors ++ D3d9.Raster.mirrors ++ D3d9.Fvf.mirrors ++ D3d9.Stage.mirrors ++ D3d9.Fog.mirrors ++ D3d9.Arg.mirrors ++ D3d9.Formats.mirrors ++ D3d9.Decl.mirrors ++ D3d9.Shade.mirrors ++ D3d9.Clip.mirrors ++ D3d9.Draw.mirrors ++ D3d9.Line.mirrors ++ D3d9.Xyz.mirrors ++
  F32.mirrors ++
  D3d9.Tex.mirrors ++ D3d9.Texel.mirrors ++ D3d9.Chain.mirrors ++ D3d9.Blend.mirrors ++ D3d9.Filter.mirrors ++ D3d9.Coord.mirrors ++
  GdiPlus.Blend.mirrors ++
  GdiPlus.Matrix.mirrors ++
  GdiPlus.Flatten.mirrors ++
  GdiPlus.Dither.mirrors ++
  D3d11.Raster.mirrors ++ D3d11.Draw.mirrors ++ D3d11.Texture.mirrors ++
  D3d11.Desc.mirrors ++
  D3d11.View.mirrors ++
  D3d11.Filter.mirrors ++
  D3d11.Merge.mirrors ++
  D3d11.Interp.mirrors ++
  D3d11.Clip.mirrors ++
  DWrite.Advance.mirrors ++
  DWrite.Metrics.mirrors

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
