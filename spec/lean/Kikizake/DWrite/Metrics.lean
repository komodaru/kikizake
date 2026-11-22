import Kikizake.Mirror
/-! DirectWrite's design metrics of a face (spec/dwrite.txt DW.METRICS.*) and of a glyph
(DW.GLYPH.*), from the font's tables as src/model/dwrite.rs `metrics` and `glyph_metrics`
compute them, in the types DWRITE_FONT_METRICS and DWRITE_GLYPH_METRICS keep (mirrors
`dw.metrics`, `dw.glyph`). -/
namespace Kikizake.DWrite.Metrics

/-- an integer as a UINT16 keeps it (modulo 2^16) -/
def u16 (v : Int) : Int := v % 65536

/-- an integer as an INT16 keeps it (two's complement) -/
def i16 (v : Int) : Int := if v % 65536 ≥ 32768 then v % 65536 - 65536 else v % 65536

/-- an integer as a UINT32 keeps it (modulo 2^32) -/
def u32 (v : Int) : Int := v % 4294967296

/-- the tables the metrics come from: head's units per em and box, hhea's ascender, descender
and line gap where there is hhea, OS/2's fields where there is OS/2, post's underline where
there is post -/
structure Font where
  upm : Int
  box : Int × Int × Int × Int
  hhea : Option (Int × Int × Int)
  /-- version, ySubscript/ySuperscript fields (8), strikeout size and position, fsSelection,
  typo ascender, descender, line gap, win ascent, descent, sxHeight, sCapHeight -/
  os2 : Option (Int × List Int × (Int × Int) × Int × (Int × Int × Int) × (Int × Int) × Int × Int)
  post : Option (Int × Int)

/-- DWRITE_FONT_METRICS1's fields -/
structure Out where
  upm : Int
  asc : Int
  desc : Int
  gap : Int
  cap : Int
  x : Int
  ulpos : Int
  ulthick : Int
  stpos : Int
  stthick : Int
  bbox : List Int
  sub : List Int
  sup : List Int
  typo : Bool

/-- the metrics before the defaults: head's, then OS/2's (USE_TYPO_METRICS, fsSelection bit 7,
over the win ones) or hhea's, then post's -/
def raw (f : Font) : Out :=
  let (x0, y0, x1, y1) := f.box
  let base : Out := ⟨f.upm, 0, 0, 0, 0, 0, 0, 0, 0, 0, [x0, y1, x1, y0], [0, 0, 0, 0], [0, 0, 0, 0], false⟩
  let m := match f.os2 with
    | some (ver, s, (sz, sp), fs, (ta, td, tg), (wa, wd), xh, ch) =>
      let asc := wa
      let desc := u16 (i16 wd).natAbs
      let gap := match f.hhea with
        | some (a, d, g) => i16 (max (a + d.natAbs + max g 0 - asc - desc) 0)
        | none => 0
      let sub := [s.getD 2 0, i16 (-(s.getD 3 0)), s.getD 0 0, s.getD 1 0]
      let sup := [s.getD 6 0, s.getD 7 0, s.getD 4 0, s.getD 5 0]
      let (cap, x) := if ver ≥ 2 then (u16 ch, u16 xh) else (0, 0)
      if fs / 128 % 2 = 1 then
        { base with asc := u16 ta, desc := u16 (max (-td) 0), gap := tg, cap, x, stpos := sp, stthick := u16 sz, sub, sup, typo := true }
      else
        { base with asc, desc, gap, cap, x, stpos := sp, stthick := u16 sz, sub, sup }
    | none =>
      let stpos := f.upm / 3
      match f.hhea with
      | some (a, d, g) => { base with asc := u16 a, desc := u16 d.natAbs, gap := max g 0, stpos }
      | none => { base with stpos }
  match f.post with
  | some (pos, thick) => { m with ulpos := pos, ulthick := u16 thick }
  | none => { m with ulpos := -(f.upm / 10) }

/-- the metrics: the raw ones with the defaults where a value is 0 -/
def metrics (f : Font) : Out :=
  let m := raw f
  let ulthick := if m.ulthick = 0 then (f.upm + 7) / 14 else m.ulthick
  { m with
    ulthick
    stthick := if m.stthick = 0 then ulthick else m.stthick
    x := if m.x = 0 then f.upm / 2 else m.x
    cap := if m.cap = 0 then f.upm * 7 / 10 else m.cap }

/-- DW.METRICS.HEAD: units per em and the box (xMin, yMax, xMax, yMin) are head's -/
theorem head (f : Font) : (metrics f).upm = f.upm ∧ (metrics f).bbox = [f.box.1, f.box.2.2.2, f.box.2.2.1, f.box.2.1] := by
  obtain ⟨u, ⟨x0, y0, x1, y1⟩, h, o, p⟩ := f
  simp only [metrics, raw]
  repeat' split
  all_goals simp_all

/-- DW.METRICS.WIN: with OS/2 and fsSelection bit 7 clear, ascent and descent are the win
ones (the descent's magnitude), the line gap hhea's ascender + |descender| + max(0, lineGap)
less them, at least 0 (0 without hhea); strikeout and the sub- and superscript boxes OS/2's -/
theorem win (u : Int) (b : Int × Int × Int × Int) (h : Option (Int × Int × Int)) (p : Option (Int × Int))
    (ver : Int) (s : List Int) (sz sp fs ta td tg wa wd xh ch : Int) (hfs : fs / 128 % 2 ≠ 1) :
    let m := metrics ⟨u, b, h, some (ver, s, (sz, sp), fs, (ta, td, tg), (wa, wd), xh, ch), p⟩
    m.asc = wa ∧ m.desc = u16 (i16 wd).natAbs ∧
    m.gap = (match h with
      | some (a, d, g) => i16 (max (a + d.natAbs + max g 0 - wa - u16 (i16 wd).natAbs) 0)
      | none => 0) ∧
    m.stpos = sp ∧ (m.stthick = u16 sz ∨ u16 sz = 0) ∧
    m.sub = [s.getD 2 0, i16 (-(s.getD 3 0)), s.getD 0 0, s.getD 1 0] ∧ m.sup = [s.getD 6 0, s.getD 7 0, s.getD 4 0, s.getD 5 0] ∧
    m.typo = false := by
  obtain ⟨x0, y0, x1, y1⟩ := b
  simp only [metrics, raw, hfs, ↓reduceIte]
  cases h <;> cases p <;> simp <;> by_cases hz : u16 sz = 0 <;> simp [hz]

/-- DW.METRICS.TYPO: fsSelection bit 7 set: ascent sTypoAscender, descent -sTypoDescender
(0 where that is negative), line gap sTypoLineGap, the typo flag set -/
theorem typo (u : Int) (b : Int × Int × Int × Int) (h : Option (Int × Int × Int)) (p : Option (Int × Int))
    (ver : Int) (s : List Int) (sz sp fs ta td tg wa wd xh ch : Int) (hfs : fs / 128 % 2 = 1) :
    let m := metrics ⟨u, b, h, some (ver, s, (sz, sp), fs, (ta, td, tg), (wa, wd), xh, ch), p⟩
    m.asc = u16 ta ∧ m.desc = u16 (max (-td) 0) ∧ m.gap = tg ∧ m.typo = true := by
  obtain ⟨x0, y0, x1, y1⟩ := b
  simp only [metrics, raw, hfs, ↓reduceIte]
  cases h <;> cases p <;> simp

/-- DW.METRICS.OS2V2: OS/2 version 2 and later: capHeight and xHeight are sCapHeight and
sxHeight, where they are not 0 -/
theorem os2v2 (u : Int) (b : Int × Int × Int × Int) (h : Option (Int × Int × Int)) (p : Option (Int × Int))
    (ver : Int) (s : List Int) (sz sp fs ta td tg wa wd xh ch : Int) (hv : ver ≥ 2) (hc : u16 ch ≠ 0) (hx : u16 xh ≠ 0) :
    let m := metrics ⟨u, b, h, some (ver, s, (sz, sp), fs, (ta, td, tg), (wa, wd), xh, ch), p⟩
    m.cap = u16 ch ∧ m.x = u16 xh := by
  obtain ⟨x0, y0, x1, y1⟩ := b
  simp only [metrics, raw, hv, ↓reduceIte]
  repeat' split
  all_goals simp_all

/-- DW.METRICS.NOOS2: without OS/2, hhea's ascender, |descender| and max(0, lineGap) (0
without hhea), the strikeout at upm / 3 -/
theorem noos2 (u : Int) (b : Int × Int × Int × Int) (h : Option (Int × Int × Int)) (p : Option (Int × Int)) :
    let m := metrics ⟨u, b, h, none, p⟩
    m.stpos = u / 3 ∧ m.typo = false ∧
    (match h with
      | some (a, d, g) => m.asc = u16 a ∧ m.desc = u16 d.natAbs ∧ m.gap = max g 0
      | none => m.asc = 0 ∧ m.desc = 0 ∧ m.gap = 0) := by
  obtain ⟨x0, y0, x1, y1⟩ := b
  simp only [metrics, raw]
  cases h <;> cases p <;> simp

/-- DW.METRICS.DEFAULTS: a zero underline thickness is (upm + 7) / 14, a zero strikeout
thickness the underline's, a zero xHeight upm / 2, a zero capHeight upm 7 / 10; without post
the underline a tenth of the em below -/
theorem defaults (f : Font) :
    let r := raw f; let m := metrics f
    (r.ulthick = 0 → m.ulthick = (f.upm + 7) / 14) ∧ (r.stthick = 0 → m.stthick = m.ulthick) ∧
    (r.x = 0 → m.x = f.upm / 2) ∧ (r.cap = 0 → m.cap = f.upm * 7 / 10) ∧
    (f.post = none → m.ulpos = -(f.upm / 10)) := by
  refine ⟨fun h => by simp [metrics, h], fun h => by simp [metrics, h], fun h => by simp [metrics, h], fun h => by simp [metrics, h], fun h => ?_⟩
  simp only [metrics, raw, h]

/-- a glyph's tables: hmtx's advance and left side bearing, glyf's box where it has an outline,
vmtx's advance height and top side bearing where there is vmtx; and the face's OS/2 typo
ascender and descender where there is OS/2, hhea's descender where there is hhea, the face's
ascent -/
structure Glyph where
  adv : Int
  lsb : Int
  box : Option (Int × Int × Int × Int)
  vmtx : Option (Int × Int)
  typo : Option (Int × Int)
  hd : Option Int
  asc : Int

/-- DWRITE_GLYPH_METRICS: advance, lsb, rsb, tsb, advance height, bsb, vertical origin y (the
advance height a UINT32) -/
def glyph (g : Glyph) : List Int :=
  let (x0, y0, x1, y1) := g.box.getD (g.lsb, 0, g.lsb, 0)
  let (advh, voy) := match g.vmtx, g.typo with
    | some (a, t), _ => (a, t + y1)
    | none, some (ta, td) => (ta + td.natAbs, ta)
    | none, none => (g.asc + max (g.hd.getD 0) 0, g.asc)
  [g.adv, x0, g.adv - x1, voy - y1, u32 advh, advh - voy + y0, voy]

/-- DW.GLYPH.H: the advance is hmtx's; the side bearings the box's left and the advance less
its right (an empty glyph's box a point at hmtx's lsb) -/
theorem horizontal (g : Glyph) (x0 y0 x1 y1 : Int) (hb : g.box = some (x0, y0, x1, y1)) :
    (glyph g).take 3 = [g.adv, x0, g.adv - x1] := by
  simp only [glyph, hb, Option.getD_some]
  split <;> simp

/-- DW.GLYPH.VMTX: with vmtx, the advance height and top side bearing are vmtx's, the vertical
origin the top side bearing plus yMax -/
theorem vmtx (g : Glyph) (a t : Int) (hv : g.vmtx = some (a, t)) (ha : 0 ≤ a) (ha' : a < 4294967296) :
    let (_, _, _, y1) := g.box.getD (g.lsb, 0, g.lsb, 0)
    (glyph g)[3]? = some t ∧ (glyph g)[4]? = some a ∧ (glyph g)[6]? = some (t + y1) := by
  simp only [glyph, hv, u32, Int.emod_eq_of_lt ha ha']
  simp

/-- DW.GLYPH.NOVMTX: without vmtx, with OS/2: the advance height sTypoAscender +
|sTypoDescender|, the vertical origin sTypoAscender, whatever USE_TYPO_METRICS says (it is
not an input) -/
theorem novmtx (g : Glyph) (ta td : Int) (hv : g.vmtx = none) (ht : g.typo = some (ta, td)) (h0 : 0 ≤ ta + td.natAbs) (h1 : ta + td.natAbs < 4294967296) :
    (glyph g)[4]? = some (ta + td.natAbs) ∧ (glyph g)[6]? = some ta := by
  simp only [glyph, hv, ht, u32, Int.emod_eq_of_lt h0 h1]
  simp

/-- DW.GLYPH.NOVMTX.NOOS2: without vmtx and OS/2: the advance height the ascent plus hhea's
descender where it is positive, the vertical origin the ascent -/
theorem novmtx_noos2 (g : Glyph) (hv : g.vmtx = none) (ht : g.typo = none) (h0 : 0 ≤ g.asc + max (g.hd.getD 0) 0)
    (h1 : g.asc + max (g.hd.getD 0) 0 < 4294967296) :
    (glyph g)[4]? = some (g.asc + max (g.hd.getD 0) 0) ∧ (glyph g)[6]? = some g.asc := by
  simp only [glyph, hv, ht, u32, Int.emod_eq_of_lt h0 h1]
  simp

/-- an option from a flag and its value -/
def opt {α : Type} (flag : Int) (v : α) : Option α := if flag = 0 then none else some v

def font : List Int → Option Font
  | [u, x0, y0, x1, y1, hh, a, d, g, ho, ver, s0, s1, s2, s3, s4, s5, s6, s7, sz, sp, fs, ta, td, tg, wa, wd, xh, ch, hp, pos, thick] =>
    some ⟨u, (x0, y0, x1, y1), opt hh (a, d, g), opt ho (ver, [s0, s1, s2, s3, s4, s5, s6, s7], (sz, sp), fs, (ta, td, tg), (wa, wd), xh, ch), opt hp (pos, thick)⟩
  | _ => none

def out (m : Out) : List Int :=
  [m.upm, m.asc, m.desc, m.gap, m.cap, m.x, m.ulpos, m.ulthick, m.stpos, m.stthick] ++ m.bbox ++ m.sub ++ m.sup ++ [if m.typo then 1 else 0]

def mirrors : List Mirror := [
  ⟨"dw.metrics", fun xs => (font xs).map (out ∘ metrics)⟩,
  ⟨"dw.glyph", fun
    | [adv, lsb, hb, x0, y0, x1, y1, hv, va, vt, ho, ta, td, hh, hd, asc] =>
      some (glyph ⟨adv, lsb, opt hb (x0, y0, x1, y1), opt hv (va, vt), opt ho (ta, td), opt hh hd, asc⟩)
    | _ => none⟩
]

end Kikizake.DWrite.Metrics
