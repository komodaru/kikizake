//! Checks that keep the 2026-11-20 review's mistakes from coming back (absolute: no baseline).
//!
//! * `model-uses-generators` (review 4.3): a model (src/model/) reads nothing of the generators
//!   (src/generators/): what both need lives in a neutral module (src/dxbc.rs, src/shaders.rs,
//!   src/resource_desc.rs), so the cases and the answers they are checked against do not share
//!   one implementation. The sites left are counted per file in `MODEL_USES_GENERATORS` and
//!   only go down.
//! * `io-error-as-empty` (review 2.2, 2.4): a file or directory that cannot be read is not an
//!   empty one: a statement that reads one (`fs::read_dir`, `fs::read_to_string`) must not turn
//!   the error into a default (`unwrap_or_default`, `unwrap_or`, `unwrap_or_else`, `.ok()`),
//!   unless a comment on it or the line before says `(optional`.
//! * `default-beside-new` (review 4.1): a type that derives Default and has a `fn new()` with no
//!   arguments makes `new` the default itself (`Self::default()`), so the two cannot disagree.
//! * `dib-without-flush` (review 3.4): an executor function that touches a DIB section's memory
//!   (`->bits`, `row(`) calls GdiFlush first (the function that creates the section excepted).
//! * `wait-unchecked` (review 3.5): a D3D12 queue signal, fence event or wait whose result is
//!   thrown away (the call is the whole statement).
//! * `confined` (review of 8480456, 1.2 and 1.3): what must go through one function is used only
//!   there (`CONFINED`): DirectDraw's pixels (`.px[`) only in Surf's accessors, so nothing reads
//!   a byte the model does not know (Px::Unknown) as a value; D3DX's half routines only in
//!   F16Array::half, so the array and the load codecs cannot pick different ones.
//! * `forbidden` (review of 8480456, 1.1): a construction Windows' answers refuted (`FORBIDDEN`):
//!   JPEG 4:4:0's chroma repeated instead of interpolated; device12 again dropping the swap
//!   chain (2.2); OptimizeRGB's index taken as 0 for a dot below 0 on x64 (4: Windows faults).
//! * `silent-cut` (review of 8480456, 3): an executor loop that copies only while a count is
//!   below a capacity (`for (...; ... && n < CAP; ...)`) drops the rest without a word, and the
//!   model reads it all: refuse past the capacity (`bad = 1`) instead. The line reader's MAXTOK
//!   (runner/main.c, mirrored by the model's rootsig::MAXTOK) is the one cut allowed.
//! * `kani-gates` (review of 8480456, 5.1): an API stable in the build's Rust but not in Kani's
//!   compiler (`KANI_GATES`) is used only with its feature turned on for Kani in src/lib.rs
//!   (`#![cfg_attr(kani, feature(..))]`), so the proofs compile whatever the generators use.
//! * `lean-sampler` (review of 84b05c8, 4.1): the Lean mirrors' samples (src/lean/) come from
//!   rng::Sample64, never the cases' XorShift32, whose 32-bit draws left all but the lowest 2^32
//!   values of a 64-bit domain out.
//! * `zip-unchecked` (review of 84b05c8, 4.3): tests/*.rs pair sequences with pairs::zip_eq, not
//!   `.zip(`, so an output missing its lines fails instead of comparing nothing.
//! * `header-unflushed` (review of 8480456, 4): runner/main.c writes a case's header out at once
//!   (`oflush()` right after it), so a stop inside a case names that case.
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::code::{code_only, line_of};

/// (file, what (a regular expression), the functions it may appear in): `confined`
pub const CONFINED: &[(&str, &str, &[&str])] = &[
    // (the current source and which decoder's frame it is change together, golden wmpframe-*:
    // a frame of JPEG XR's written again by JPEG XR's encoder keeps its pixels)
    ("src/model/wicimage.rs", r"self\.cur\s*=\s*|self\.from\s*=\s*", &["set_cur", "op"]),
    ("src/model/ddraw.rs", r"\.px\[", &["get", "pixel", "set", "put"]),
    ("src/model/d3dx9.rs", r"\bf16_(sse|one|x64|x64_tail)\s*\(", &["half"]),
    // (a stage argument's selector and modifiers are stage_arg's, which the Lean mirror d3d9.arg
    // holds to spec/lean/Kikizake/D3d9/Arg.lean)
    ("src/model/d3d9draw.rs", r"\[v\[3\];\s*4\]|D3D9_STAGE_NOTEX|&\s*0x20\s*!=\s*0", &["stage_arg"]),
    // (CheckDeviceFormat's answers are check_hr's and the formats taken supported's, which the
    // Lean mirrors d3d9.check, d3d9.supported hold to spec/lean/Kikizake/D3d9/Formats.lean)
    ("src/model/d3d9draw.rs", r"0x8876_086a|0x0876_086f|D3D9_FORMATS_CHECK|let\s+no_tex\s*=", &["check_hr", "supported"]),
    // (what stops a draw on x86 or without a palette, the filters a sampler's MAGFILTER and
    // MINFILTER make, palette texels and the two-sided stencil states are the pure draw_gate,
    // sample_filter, d3d10_setup, palette_texel and stencil_states the Lean mirrors d3d9.gate,
    // d3d9.filter, d3d9.palette, d3d9.stencil2 hold to spec/lean/Kikizake/D3d9/Tex.lean)
    ("src/model/d3d9draw.rs", r"\(1\.\.=5\)\.contains|D3D9_BLEND_OPX86|D3D9_TEX_ADDRESSX86|D3D9_TEX_PALETTE_NONE", &["draw_gate"]),
    ("src/model/d3d9draw.rs", r"TexFilter::of\(|D3D9_TEX_FILTEROTHER|D3D9_TEX_MAGONLY", &["sample_filter", "d3d10_setup"]),
    ("src/model/d3d9draw.rs", r"D3D9_TEX_PALETTE\)|D3D9_STENCIL_TWOSIDED|\b18[6-9]\b", &["palette_texel", "stencil_states"]),
    // (DXT texels, the channels a format lacks, how a format is sampled and a level's bytes are
    // dxt_*, to_rgba, texel_kind and level_bytes, which d3d9.dxt, d3d9.rgba, d3d9.level hold to
    // spec/lean/Kikizake/D3d9/Texel.lean)
    ("src/model/d3d9draw.rs", r"\b171\s*\*|\b85\s*\*\s*a\b|D3D9_TEX_DXT", &["dxt_mix", "dxt_block_texel"]),
    ("src/model/d3d9draw.rs", r"D3D9_TEX_CREATE|div_ceil\(\s*4\s*\)|D3D9_TEX_MISSING|D3D9_TEX_FORMAT", &["create_level", "level_bytes", "to_rgba", "texel_kind"]),
    // (the order the texture stages run in and the bump mapping arithmetic are stage_plan,
    // reads_texture, reads_current, bump_uv, bump_luminance, bump_scale and premodulate, which
    // d3d9.plan, d3d9.reads, d3d9.bump hold to spec/lean/Kikizake/D3d9/Chain.lean)
    ("src/model/d3d9draw.rs", r"D3D9_STAGE_(DISABLE|MULTI|BUMPCHAIN|ALPHADISABLE(_FIRST)?)\b|&\s*0xf\s*==\s*2|&\s*0xf,\s*0\s*\|\s*1\)", &["stage_plan", "reads_texture", "reads_current"]),
    ("src/model/d3d9draw.rs", r"round_ties_even\(\)|\bm\(\s*7\s*\)\s*\*|\(\w+\s*\*\s*scale\s*\+", &["bump_scale", "bump_uv", "bump_luminance"]),
    // (a linear filter's texel and weight, the level of a lod and a depth view's bounds are
    // split, level and past_depth, which the Lean mirrors d3d11.split, d3d11.level and
    // d3d11.pastdepth hold to spec/lean/Kikizake/D3d11/Filter.lean)
    ("src/model/d3draster.rs", r"as\s+f32\s*-\s*0\.5|\.fract\(\)\s*\*\s*256|\+\s*0\.5\)\.floor\(\)|index\(dw\)\s*<=", &["split", "level", "past_depth"]),
    // (a triangle's and a line's planes, a depth's clamp and a vertex's depth are plane_value,
    // line_value, clamp_range and vertex_depth, which d3d11.plane, d3d11.lineplane,
    // d3d11.clamprange and d3d11.vertexdepth hold to spec/lean/Kikizake/D3d11/Interp.lean)
    ("src/model/d3draster.rs", r"\.max\(lo\)\.min\(hi\)|\(1\.0\s*/\s*[\w.]+(\[3\])?\)\s*\*\s*\(maxz\s*-\s*minz\)|\(a\[0\]\s*-\s*gx\s*\*\s*x0\)|1\.0\s*/\s*\(b\s*-\s*a\)", &["plane_value", "line_value", "clamp_range", "vertex_depth"]),
    // (the f32 clippings, a triangle's coordinates not finite, what the clipping leaves with no
    // area and depths wholly out of range are clip_line, clip_line_z, clip_xy, clip_z,
    // not_finite, clipped_away and depth_away, which d3d11.clipline, d3d11.clipxy,
    // d3d11.notfinite, d3d11.clippedaway and d3d11.depthaway hold to
    // spec/lean/Kikizake/D3d11/Clip.lean)
    ("src/model/d3draster.rs", r"da\s*/\s*\(da\s*-\s*db\)|\*z\s*<\s*0\.0\)\s*\|\||!x\.is_finite\(\)|\.windows\(2\)\.skip\(1\)", &["clip_line", "clip_line_z", "clip_xy", "clip_z", "not_finite", "clipped_away", "depth_away"]),
    // (a line's major axis, the multisample and alpha lines' corners and the alpha coverage are
    // major_of, quad_corners, alpha_corners and alpha_coverage, which d3d11.linecovers,
    // d3d11.quadcorners, d3d11.alphacorners and d3d11.alphacover hold to
    // spec/lean/Kikizake/D3d11/Line.lean)
    ("src/model/d3draster.rs", r"QUAD_HALF\s*/|0x3EE6_6666|2\.0\s*/\s*len\b|\.y\s*-\s*\w+\.y\)\.abs\(\)\s*>", &["major_of", "quad_corners", "alpha_corners", "alpha_coverage"]),
    // (CreateInputLayout's verdict on the generator's programs is layout_verdict, which
    // d3d11.layout holds to spec/lean/Kikizake/D3d11/Layout.lean)
    ("src/model/d3draster.rs", r#""bad"\s*=>|"vsid"\s*=>\s*1"#, &["layout_verdict"]),
    // (a declaration's refusals are decl_ok's, which the Lean mirror d3d9.decl holds to
    // spec/lean/Kikizake/D3d9/Decl.lean)
    ("src/model/d3d9draw.rs", r"usage\s*==\s*8\s*&&|\(9,\s*0\)", &["decl_ok"]),
    // (the shading, clipping and snapping arithmetic is grad9, at9, plane10, clip_bounds and
    // snap256, which the Lean mirrors d3d9.grad, at, plane, clipbounds, snap hold bit for bit to
    // spec/lean/Kikizake/D3d9/Shade.lean and Clip.lean)
    ("src/model/d3d9draw.rs", r"\*\s*\(y\[2\]\s*-\s*y\[0\]\)|1\.0\s*/\s*area|\*\s*256\.0\)|D3D9_RASTER_CLIP8\b|D3D9_SHADE_(FLAT|GOURAUD)", &["grad9", "plane10", "snap256", "clip_bounds", "shaded"]),
    // (the primitives a draw assembles, the words each vertex is read from, the pixel a point
    // lights and a pixel's packing are assemble, vertex_words, point_pixel and pack, which the Lean
    // mirrors d3d9.assemble, vertices, pointpixel, pack hold to spec/lean/Kikizake/D3d9/Draw.lean)
    ("src/model/d3d9draw.rs", r"D3D9_DRAW_(ASSEMBLY|STRIDE)\b|-\s*0\.5\)\.ceil\(\)|\|\|\s*x_alpha\b|chunks\(stride\)", &["assemble", "vertex_words", "point_pixel", "pack"]),
    // (a line's cut, an untransformed vertex's transform and clipping and the perspective-correct
    // attributes are line_cut, transform_xyz, clip_xyz, xyz_vertex, rhw_attrs and rhw_value, which
    // the Lean mirrors d3d9.linecut, xyz, clipxyz, xyzvertex, rhw hold to
    // spec/lean/Kikizake/D3d9/Line.lean and Xyz.lean)
    ("src/model/d3d9draw.rs", r"D3D9_DRAW_(LINECLIP(_NEAR)?|XYZ_CLIP|RHW)\b|\*=?\s*at\[11\]|/\s*at32\[11\]|1\.0\s*/\s*[cq]\[3\]", &["line_cut", "transform_xyz", "clip_xyz", "xyz_vertex", "rhw_attrs", "rhw_value"]),
    // (blending on targets of other formats is blend_float and factor_f32, which d3d9.blendf holds
    // to spec/lean/Kikizake/D3d9/Blend.lean)
    ("src/model/d3d9draw.rs", r"D3D9_BLEND_FORMAT|\.min\(\s*1\.0\s*-|\|x\|\s*x\.clamp\(0\.0,\s*1\.0\)\s*as\s*f32", &["blend_float", "factor_f32"]),
    // (point and linear filtering's coordinates, weights and sums are point_at, bilinear_at,
    // bilinear_weights, bilinear_bytes, bilinear_unit and bilinear_float, which d3d9.point,
    // d3d9.bilinear, d3d9.bibytes and d3d9.bifloat hold to spec/lean/Kikizake/D3d9/Filter.lean)
    ("src/model/d3d9draw.rs", r"\*\s*64\.0\)\.round\(\)|\*\s*(32\.0|q\s+as\s+f64)\)\.floor\(\)|/\s*BILERP_ONE|\(q\s*-\s*fx\)|\(Q\s*-\s*fx\)", &["point_at", "bilinear_at", "bilinear_weights", "bilinear_unit", "bilinear_float"]),
    // (the sets of coordinates the stages read and a set's row are tci_slots and coord_row, which
    // d3d9.slots and d3d9.row hold to spec/lean/Kikizake/D3d9/Coord.lean)
    ("src/model/d3d9draw.rs", r"slots\.contains\(|Ordering::Equal\s*=>\s*1\.0|\[0\.0,\s*0\.0,\s*1\.0,\s*0\.0\]", &["tci_slots", "coord_row"]),
    // (a stage's texture matrix is tex_transform, which d3d9.ttf holds to
    // spec/lean/Kikizake/D3d9/Coord.lean)
    ("src/model/d3d9draw.rs", r"ttf\s*&\s*0x100|\[count\s*-\s*1\]|count\s*-\s*1\)", &["tex_transform"]),
];

/// (file, what (a regular expression), why): `forbidden`
pub const FORBIDDEN: &[(&str, &str, &str)] = &[
    ("src/model/wicimage.rs", r#"self\.from\s*==\s*Some\(\s*"wmp"\s*\)\s*&&\s*bag\b|if\s+rcw\s*&&\s*i\.fmt\s*==\s*"gray8""#, "a JPEG XR frame written again by JPEG XR's encoder keeps its pixels whatever its options (wmpframe-opt-*), and which turned files copy is turned()'s, of the width (wmpturn-*)"),
    ("src/model/wicimage.rs", r#"for\s+\w+\s+in\s+&?t\s*\[\s*first\s*\.\.\s*\]|\bmatch\s+option\s*\(\s*cont\s*,\s*o\s*\)|Decodes\s*\(\s*"wmp"\s*,\s*img\s*\)"#, "an encoder's options are Bag::write's, each name's last value as Initialize reads them (a name given twice), and a lossless JPEG XR file is turned by its BitmapTransform as a lossy one is (review of 84b05c8, 3.4)"),
    ("src/model/opengl_list.rs", r"1\s*<<\s*20|0x10_?0000|1_?048_?576", "glDeleteLists deletes every list of its range, however wide (list-more-delete-*), not the first 2^20 names (review of 84b05c8, 5)"),
    ("src/model/opengl_list.rs", r#""gltex"\s*\|\s*"glmat"|"glmat"\s*\|\s*"glfogv"\s*\|\s*"gldraw"\s*\|\s*"gltex"\s*=>\s*Kind::Compiled\s*,\s*\n\s*_\s*=>"#, "a glTexImage of a proxy target runs at once, outside the list (GL 1.1 3.8.3; list-more-proxy-*): its gltex arm comes before the compiled ones (review of 84b05c8, 3.5)"),
    ("src/model/jxr.rs", r">>=?\s*\(?\s*1\s*-\s*e1", "float2pixel's shift by 1 - e1 reaches 32 and more (float-shift-*): it is a wrapping_shr, x86's SAR count mod 32, not Rust's >>, which panics there in a debug build (review of 84b05c8, 5)"),
    ("src/model/d3d9draw.rs", r"draw\.(fvf|decl|decl_from)\s*=[^=]", "the FVF and the declaration change together through set_fvf and set_decl, which spec/tla/D3D9Fvf.tla's replay holds to fvf-decl-order-*; one set on its own leaves the other stale"),
    ("src/model/opengl.rs", r"0xd16\s*\|\s*0xd17\s*=>|0xd1[01]\s*=>\s*vec!\[B\(false\)\]|0xd1[23]\s*=>\s*i\(0\)|0xb06\s*=>\s*vec!\[F\(0\.0\)|0xd1e\s*=>\s*vec!\[F\(1\.0\)\]|0xb04\s*=>\s*c4\(\[1\.0;\s*4\]\)|0xb05\s*=>\s*vec!\[F\(1\.0\)\]", "glGet reads the state glPixelZoom, glPixelTransfer and glRasterPos set (the pixel mode and raster position the attribute stack gives back), not its initial value (review of 84b05c8, 3.3; the raster color: golden attrib-more-current-pushed-line)"),
    ("src/model/gdiplus.rs", r"\*\s*\(\s*256\s*-\s*wt\s*\)|\(\s*2\s*\*\s*\(\s*c0\b", "a hatch texel's mix and an antialiased pixel's share are the pure hatch_mix and coverage_share the Lean mirrors gp.hatch_mix, gp.coverage_share hold equal to spec/lean/Kikizake/GdiPlus/Blend.lean; a copy inlined in paint escapes them"),
    ("src/model/jpeg.rs", r"Chroma::H1V2\s*=>\s*repeat\s*\(", "4:4:0's chroma rows are interpolated 3:1 (opt-jpeg-subs-*-4), not repeated"),
    ("src/calls.rs", r"\[\s*(open|close)\s*\+|\[\s*\.\.\s*open\s*\]", "a call line is split (split_once, rsplit_once), not sliced at positions found apart: \") = (\" panicked (review of 8480456, 5.4)"),
    ("src/model/ntstr.rs", r"from_str_radix\s*\(\s*&\w+\s*\[", "hex bytes are read as the executor's hexbytes does, digit by digit of the bytes: a str slice through a character past ASCII panicked (review of 8480456, 5.4)"),
    ("tests/calls.rs", r"calls::check_dir\s*\(", "the whole set's condition is check_complete (every suite of spec/calls-suites.txt, current); check_dir checks only the recordings there are"),
    ("src/model/d3d12.rs", r#""device12"\s*=>\s*\{[^}]*self\.(chain|factory)\s*="#, "device12 again keeps the executor's device and what was made of it (dxgi device-again)"),
    ("src/model/d3d12.rs", r"\bat\s*\+\s*pitch\s*\*", "a subresource's bytes and its last row's end are 32-bit counts on Windows (Bytes32::of; golden d3d12-* fp-limit2-offsets-*, fp-limit2-total-*), not 64-bit sums"),
    ("src/model/d3d12.rs", r"kind\(\)\s*!=\s*Dim::Tex2D\s*&&\s*\(\s*d\.w\s*>", "GetCopyableFootprints checks a 2D texture's size as CreateCommittedResource does (size_max; golden d3d12-* fp-wide-*, fp-limit-*)"),
    ("src/lean/d3d12.rs", r"\bFP_SIZES\b|0x7fff_fff0", "the footprints mirror asks widths and heights up to 0xffffffff (SIZES): a narrower domain hid the model's 32-bit wrap"),
    ("src/model/d3draster.rs", r"d\.rast\[1\]\s*==\s*[23]\s*&&|p[xy]\s*>=\s*d\.scissor|\(s\.[xy]\s*-\s*1\)\.div_euclid|rem_euclid\(\s*2\s*\*\s*size|\(256\s*-\s*w[uv]?\)|2\s*=>\s*(a|reference)\s*<|let\s+top_left\s*=|Topo::TriStripAdj\s*=>\s*\(\[", "culling, the scissor test, a point's pixel, the address modes, the comparison functions, the 8-bit filters, the top-left rule and the topologies' vertices are the pure functions of d3drules.rs the Lean mirrors hold equal to spec/lean/Kikizake/D3d11/; a copy inlined in d3draster.rs escapes them"),
    ("src/model/d3d9draw.rs", r"if\s+self\.v8\s*\{\s*0xffff_ffff\s*\}|Self::blend_op\b|let\s+e\s*=\s*\(q\.0\s*-\s*p\.0\)", "the blend color, blend operation and top-left rule are the pure blend_color, blend_op and covers the Lean mirrors hold equal to spec/lean/Kikizake/D3d9/Raster.lean; a copy inlined elsewhere escapes them"),
    ("src/model/d3d9draw.rs", r"a1\[i\]\s*\*\s*a2\[i\]|a1\[i\]\s*\+\s*a2\[i\]", "the stage operations' formulas are stage_value's, which the Lean mirror d3d9.stage holds bit for bit to spec/lean/Kikizake/D3d9/Stage.lean; a copy inlined elsewhere escapes it"),
    ("src/model/d3dres.rs", r#""uav none"\.into\(\)\);\s*return\s+Ok\(\(\)\);\s*\}\s*if\s+!check\s*\{\s*self\.views\.empty"#, "the executor empties the view slot before it answers Direct3D 10's \"uav none\" (runner/d3dres.c op_view; spec/tla/D3DResCalls.tla, probe-uav-slot): answering first kept the old view"),
    ("src/model/d3dres.rs", r#""buf"\s*\|\s*"srv""#, "on a removed device a view of an empty slot is still the executor's nores (spec/tla/D3DResCalls.tla, probe-removed-calls): views have their own removed arm after the nores one, not the resources' arm"),
    ("src/model/d3d9draw.rs", r"\(-\(z\s*\*\s*density\)\)\.exp\(\)|\(1\.0\s*-\s*f\)\s*\*\s*fc\[", "the fog factor and blend are fog_factor, fog_exp_arg and fog_blend, which the Lean mirrors d3d9.fog* hold to spec/lean/Kikizake/D3d9/Fog.lean; a copy inlined elsewhere escapes them"),
];

/// (file, function, what (a regular expression), why): `forbidden` inside one function only
pub const FORBIDDEN_IN: &[(&str, &str, &str, &str)] = &[
    (
        "src/model/opengl.rs",
        "pop_attrib",
        r"\.\.\s*\}",
        "glPopAttrib names every field of Ctx with its group (or why it has none), so a new field does not compile until it is placed: a rest pattern (..) let the current texture coordinates, raster position, line stipple and list base go unrestored (review of 84b05c8, 3.3)",
    ),
    (
        "src/model/opengl_tex.rs",
        "pop",
        r"\.\.\s*\}",
        "Tex::pop names every field of Tex with its group (TEXTURE_BIT, CURRENT_BIT or none): a rest pattern (..) hides a new one (review of 84b05c8, 3.3)",
    ),
    (
        "src/model/opengl_list.rs",
        "pop",
        r"\.\.\s*\}",
        "Lists::pop names every field of Lists (the base LIST_BIT's): a rest pattern (..) hides a new one (review of 84b05c8, 3.3)",
    ),
    (
        "src/model/d3draster.rs",
        "shade",
        r"\btexel\s*\(\s*r\s*,\s*0\s*,|\br\.size\s*\(\s*0\s*\)|\bv\.words\b|\bv\.u\s*\(",
        "a view's levels and slices are view_reach's (Load, Sample and the binding hazards alike): reading the resource's level 0, or the view's union words apart, took a view's most detailed level for the resource's (review of 84b05c8, 3.2)",
    ),
    (
        "src/calls.rs",
        "check_dir",
        r"\brsplit_once\s*\(",
        "a recording's suite and architecture come from RecName::parse, of the file's name and the header's alike, which check_dir compares (review of 84b05c8, 4.2: a 32-bit recording under the 64-bit name passed)",
    ),
    (
        "src/model/gdiplus.rs",
        "fill_area",
        r"\bodd_end\b|rem_euclid\s*\(\s*4\s*\)",
        "a gamma corrected texture's dither place is the pure dither_column and dither_row, which the Lean mirror gp.dither_place holds equal to spec/lean/Kikizake/GdiPlus/Dither.lean; a run walk inlined in fill_area escapes them",
    ),
    (
        "src/model/gdiplus.rs",
        "mix",
        r"8192\.0|\bas\s+f32\b",
        "x86-64's gamma corrected word is the pure mix64, which the Lean mirror gp.gamma64 holds equal to spec/lean/Kikizake/GdiPlus/Matrix.lean; float arithmetic inlined in mix escapes it",
    ),
    (
        "src/model/gdiplus.rs",
        "new",
        r"\.sub\s*\(\s*1\.0",
        "a gamma corrected source's rest is the pure gamma_rest, which the Lean mirror gp.gamma64 holds equal to spec/lean/Kikizake/GdiPlus/Matrix.lean; a copy inlined in Src::new escapes it",
    ),
    (
        "src/model/dxtenc.rs",
        "optimize_rgb",
        r"\bdot\s*<=?\s*F::of\s*\(\s*0(\.0)?\s*\)",
        "neither build's OptimizeRGB tests the dot below 0: x64 faults on the index -1 (zero-extended, 16 GiB past the table), x86 reads below it (Below; loadtex-dxt-negindex)",
    ),
    (
        "src/model/dxtenc.rs",
        "optimize_rgb",
        r"F::BELOW|trunc_i\s*\(",
        "a point's step and the fault below 0 are step_index, which tests/laws_dxtenc.rs proves for every f32 dot; an index taken inline in optimize_rgb escapes it",
    ),
    (
        "src/model/dxgiconv.rs",
        "decode_block",
        r"171\s*\*|\*\s*257\b|\*\s*32767\b|\+\s*[23]\s*\)\s*/\s*[57]",
        "BC1's colours, BC3's alpha and BC4's steps are the pure bc1_codes, bc3_alpha, bc4_code and bc4_snorm_code that tests/laws_conv.rs proves over every endpoint pair; a copy inlined in decode_block escapes them",
    ),
    (
        "src/model/dxgiconv.rs",
        "encode",
        r"\.trunc\s*\(|0x7f80_0000",
        "a 32-bit float's store and an integer clear are the pure f32_store and int_clear that tests/laws_conv.rs proves for every float; a copy inlined in encode escapes them",
    ),
    (
        "src/model/d3d12.rs",
        "clear_depth",
        r"\bto_unorm\s*\(|\.clamp\s*\(",
        "a UNORM depth clear is unorm_depth, which tests/laws_conv.rs proves for every float; a conversion inlined in clear_depth escapes it",
    ),
    (
        "src/model/d3dx9.rs",
        "dxt_d3dx",
        r"1\.0 / 3\.0|1\.0 / 15\.0|1\.0 / 255\.0|1\.0 / 31",
        "a DXT texel's colour and alpha are dxt_colour, dxt3_alpha and dxt5_alpha, which tests/laws_d3dx9_dxt.rs proves over every block; a reading inlined in dxt_d3dx escapes them",
    ),
    (
        "src/model/d3dx9.rs",
        "yuy2_pair",
        r"65\.481|37\.797|93\.786",
        "YUY2's Y, U and V are yuy2_bytes, which tests/laws_d3dx9_yuv.rs proves for every colour; a formula inlined in yuy2_pair escapes it",
    ),
    (
        "src/model/d3dx9.rs",
        "yuv_d3dx",
        r"0\.00456621|wrapping_sub",
        "a YUV pixel's chroma and colour are chroma_tap and yuv_rgb, which tests/laws_d3dx9_yuv.rs proves over their domains; arithmetic inlined in yuv_d3dx escapes them",
    ),
    (
        "src/model/bptc.rs",
        "bc6h",
        r"BC6H_MODES\s*\.\s*iter\s*\(\s*\)\s*\.\s*position",
        "BC6H's mode is bc6h_mode, which tests/laws_conv.rs proves for every mode value; a lookup inlined in bc6h escapes it",
    ),
    (
        "src/model/bptc.rs",
        "bc7",
        r"\.find\s*\(\s*\|_\|\s*s\.take",
        "BC7's mode is bc7_mode, which tests/laws_conv.rs proves for every first byte; a scan inlined in bc7 escapes it",
    ),    (
        "src/model/d3dres_check.rs",
        "check_with",
        r"\b(group_support|uav_support|support|format)\s*\(",
        "check_with takes the format tables only through its Facts (facts gives them): the d3d11.desc mirror chooses the Facts, so a table read inlined in check_with escapes spec/lean/Kikizake/D3d11/Desc.lean",
    ),
    (
        "src/model/d3dres_check.rs",
        "check_view_with",
        r"\b(support|format|group|view_facts)\s*\(|dxgifmt::",
        "check_view_with takes the format tables only through its ViewFacts (view_facts gives them): the d3d11.view mirror chooses the facts, so a table read inlined in check_view_with escapes spec/lean/Kikizake/D3d11/View.lean",
    ),
];

/// the name of the function `at` lies in (the last `fn name` before it)
fn enclosing_fn(code: &str, at: usize) -> &str {
    let before = code.get(..at).unwrap_or_default();
    let Some(k) = before.rfind("fn ") else { return "" };
    let name = before.get(k.saturating_add(3)..).unwrap_or_default();
    name.get(..name.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(name.len())).unwrap_or_default()
}

/// `confined`: the sites of one file outside the functions allowed
fn confined(rel: &str, code: &str, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for &(file, pat, allowed) in CONFINED.iter().filter(|c| c.0 == rel) {
        let Some(re) = rx(pat) else {
            bad.push(format!("{file}: confined: {pat} does not compile"));
            continue;
        };
        for m in re.find_iter(code) {
            // (the function's own definition is no use of it)
            if code.get(..m.start()).is_some_and(|b| b.trim_end().ends_with("fn")) {
                continue;
            }
            let f = enclosing_fn(code, m.start());
            if !allowed.contains(&f) {
                bad.push(format!("{rel}:{}: confined: {} in {f}, only in {}", line_of(text, m.start()), m.as_str().trim(), allowed.join(", ")));
            }
        }
    }
    bad
}

/// (file, sites): src/model/ files still reading the generators
pub const MODEL_USES_GENERATORS: &[(&str, usize)] = &[("src/model/wicimage.rs", 3)];

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static GENERATORS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\b(crate|super)::generators\b|\buse\s+crate::generators"));
static FS_READ: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bfs::(read_dir|read_to_string)\s*\("));
static SWALLOW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\.(unwrap_or_default|unwrap_or|unwrap_or_else)\s*\(|\.ok\(\)|\.into_iter\(\)\s*\.flatten\(\)"));
/// (an `unwrap_or_else` that stops the program reports the error)
static STOPS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bexit\s*\(|\bpanic!|\breturn\b"));
static DERIVE_DEFAULT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*pub(?:\([^)]*\))?\s+struct\s+(\w+)|#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*struct\s+(\w+)"));
static NEW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"fn\s+new\s*\(\s*\)\s*->\s*(\w+)\s*\{"));
static IMPL: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"impl\s+(\w+)\s*\{"));
static C_FN: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^[A-Za-z_][\w \*]*\b(\w+)\s*\([^;{]*\)\s*\{"));
static DIB: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"->bits\b|\brow\s*\("));
/// (what only tests or sets the pointer: `!s->bits`, `s->bits ?`, `s->bits = ...`)
static DIB_POINTER: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"!\s*[\w>.-]*->bits\b|->bits\s*\?|->bits\s*=[^=]"));
static SILENT_CUT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"for\s*\([^;]*;[^;]*&&\s*\w+\s*<\s*([A-Z][A-Z0-9_]*)\s*;"));
/// (what the source writes, the feature Kani 0.68's compiler wants for it)
pub const KANI_GATES: &[(&str, &str)] = &[("Vec::<_>::from_fn", "vec_from_fn"), ("Vec::from_fn", "vec_from_fn"), (".trim_prefix(", "trim_prefix_suffix"), (".trim_suffix(", "trim_prefix_suffix")];

/// `kani-gates`: the features the sources need for Kani that src/lib.rs does not turn on
pub fn kani_gates(sources: &[(String, String)], lib: &str) -> Vec<String> {
    let lib = code_only(lib, false);
    let mut bad = Vec::new();
    for &(api, feature) in KANI_GATES {
        let Some((rel, _)) = sources.iter().find(|(_, t)| code_only(t, false).contains(api)) else { continue };
        // (one gate may turn on several: `feature(vec_from_fn, trim_prefix_suffix)`)
        let on = lib.lines().filter(|l| l.contains("#![cfg_attr(kani")).filter_map(|l| l.split_once("feature(")).filter_map(|(_, r)| r.split_once(')')).any(|(list, _)| list.split(',').any(|f| f.trim() == feature));
        if !on && !bad.iter().any(|b: &String| b.contains(feature)) {
            bad.push(format!("src/lib.rs: kani-gates: {rel} uses {api}, unstable in Kani's compiler: #![cfg_attr(kani, feature({feature}))]"));
        }
    }
    bad
}

/// (executor, its op table, model, its guard): the ops a device-less executor answers `nodev`
/// to are the model's: the C ops whose function begins with `if (!on12) { out("nodev\n"); ...`
/// and the names of the model's `... if !self.dev =>` arm (the review of 84b05c8, 5: rootsig12+
/// answered nodev in the model and kept its tokens in the executor)
pub const NODEV_GATES: &[(&str, &str, &str, &str)] = &[("runner/d3d12.c", "d3d12_ops", "src/model/d3d12.rs", "!self.dev")];

/// the ops of `table` in `c` whose function tests `gate` (as `if (gate) { out("nodev\n")`),
/// and the names of the arm of `rs` guarded `if <guard> =>`: the two differing, each name apart
pub fn nodev_gates(c_rel: &str, c: &str, table: &str, rs_rel: &str, rs: &str, guard: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let Some(body) = c.split_once(&format!("{table}[] = {{")).and_then(|(_, r)| r.split_once("};")).map(|(b, _)| b) else {
        return vec![format!("{c_rel}: nodev-gates: no table {table}")];
    };
    let mut gated = std::collections::BTreeSet::new();
    let mut ops = std::collections::BTreeSet::new();
    for entry in body.split('{').skip(1) {
        let Some((name, f)) = entry.split_once(',') else { continue };
        let (name, f) = (name.trim().trim_end_matches(['}', ' ']).trim_matches('"'), f.trim().trim_end_matches(['}', ',', ' ', '\n']).trim());
        if name == "0" || name.is_empty() {
            continue;
        }
        ops.insert(name.to_string());
        // (the op's function, to its first closing brace at the line's start)
        let Some(func) = c.split_once(&format!("static void {f}(void) {{")).and_then(|(_, r)| r.split_once("\n}")).map(|(b, _)| b) else {
            bad.push(format!("{c_rel}: nodev-gates: {name}'s function {f} not found"));
            continue;
        };
        if func.contains("if (!on12) { out(\"nodev\\n\"); return; }") {
            gated.insert(name.to_string());
        }
    }
    let arm = rs.lines().find(|l| l.trim_end().ends_with(&format!("if {guard} => {{")));
    let Some(arm) = arm else { return vec![format!("{rs_rel}: nodev-gates: no arm guarded {guard}")] };
    let model: std::collections::BTreeSet<String> = arm.split('"').skip(1).step_by(2).map(str::to_string).collect();
    for n in gated.difference(&model) {
        bad.push(format!("{rs_rel}: nodev-gates: the executor answers nodev to {n} without a device ({c_rel}); the model's `if {guard}` arm does not name it"));
    }
    for n in model.difference(&gated) {
        let why = if ops.contains(n) { "the executor's function runs it without a device" } else { "the executor has no such op" };
        bad.push(format!("{rs_rel}: nodev-gates: the model answers nodev to {n} without a device, but {why} ({c_rel})"));
    }
    bad
}

/// (file, capacity): the cuts allowed
pub const CUTS: &[(&str, &str)] = &[("runner/main.c", "MAXTOK")];
static WAIT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^\s*(ID3D12CommandQueue_Signal|ID3D12Fence_SetEventOnCompletion|WaitForSingleObject)\s*\("));

/// every pattern, compiled (None: one does not, an error of this file)
struct Pats {
    generators: &'static Regex,
    fs_read: &'static Regex,
    swallow: &'static Regex,
    stops: &'static Regex,
    derive_default: &'static Regex,
    new: &'static Regex,
    impls: &'static Regex,
    c_fn: &'static Regex,
    dib: &'static Regex,
    dib_pointer: &'static Regex,
    wait: &'static Regex,
    silent_cut: &'static Regex,
}

fn pats() -> Option<Pats> {
    Some(Pats { generators: GENERATORS.as_ref()?, fs_read: FS_READ.as_ref()?, swallow: SWALLOW.as_ref()?, stops: STOPS.as_ref()?, derive_default: DERIVE_DEFAULT.as_ref()?, new: NEW.as_ref()?, impls: IMPL.as_ref()?, c_fn: C_FN.as_ref()?, dib: DIB.as_ref()?, dib_pointer: DIB_POINTER.as_ref()?, wait: WAIT.as_ref()?, silent_cut: SILENT_CUT.as_ref()? })
}

const BROKEN: &str = "tools/lint/src/guards.rs: a pattern does not compile";

/// the statements of `code` (split at `;`, `{` and `}`) with their byte offsets
fn statements(code: &str) -> Vec<(usize, &str)> {
    let mut v = Vec::new();
    let mut from = 0;
    for (i, c) in code.char_indices() {
        if matches!(c, ';' | '{' | '}') {
            v.push((from, code.get(from..i).unwrap_or_default()));
            from = i.saturating_add(1);
        }
    }
    v.push((from, code.get(from..).unwrap_or_default()));
    v
}

/// the statement's only fallback is an `unwrap_or_else` whose closure stops the program
fn reports(p: &Pats, code: &str, at: usize, st: &str) -> bool {
    let Some(k) = st.find("unwrap_or_else(").filter(|_| p.swallow.find_iter(st).count() == 1) else { return false };
    let from = at.saturating_add(k).saturating_add("unwrap_or_else".len());
    let mut depth = 0usize;
    for (i, c) in code.get(from..).unwrap_or_default().char_indices() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return code.get(from..from.saturating_add(i)).is_some_and(|args| p.stops.is_match(args));
                }
            }
            _ => {}
        }
    }
    false
}

/// the problems of one Rust source at `rel`
pub fn rust(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = code_only(text, false);
    let mut bad = Vec::new();
    if rel.starts_with("src/model/") {
        let n = p.generators.find_iter(&code).count();
        let allowed = MODEL_USES_GENERATORS.iter().find(|(f, _)| *f == rel).map_or(0, |(_, n)| *n);
        if n > allowed {
            bad.push(format!("{rel}: model-uses-generators: {n} sites, at most {allowed}: move what both need to a neutral module"));
        }
    }
    bad.extend(confined(rel, &code, text));
    if rel.starts_with("src/lean/") {
        bad.extend(code.match_indices("XorShift32").map(|(at, _)| format!("{rel}:{}: lean-sampler: a mirror's samples come from rng::Sample64 (64 bits a draw), not the cases' 32-bit XorShift32", line_of(text, at))));
    }
    // (string literals kept: a match arm is known by its string)
    let with_strings = code_only(text, true);
    for &(file, pat, why) in FORBIDDEN.iter().filter(|f| f.0 == rel) {
        match rx(pat) {
            Some(re) => bad.extend(re.find_iter(&with_strings).map(|m| format!("{file}:{}: forbidden: {why}", line_of(text, m.start())))),
            None => bad.push(format!("{file}: forbidden: {pat} does not compile")),
        }
    }
    for &(file, func, pat, why) in FORBIDDEN_IN.iter().filter(|f| f.0 == rel) {
        match rx(pat) {
            Some(re) => bad.extend(re.find_iter(&with_strings).filter(|m| enclosing_fn(&with_strings, m.start()) == func).map(|m| format!("{file}:{}: forbidden in {func}: {why}", line_of(text, m.start())))),
            None => bad.push(format!("{file}: forbidden: {pat} does not compile")),
        }
    }
    let lines: Vec<&str> = text.split('\n').collect();
    for (at, st) in statements(&code) {
        if p.fs_read.is_match(st) && p.swallow.is_match(st) && !reports(&p, &code, at, st) {
            let (first, last) = (line_of(text, at), line_of(text, at.saturating_add(st.len())));
            let said = (first.saturating_sub(1)..=last).any(|l| l.checked_sub(1).and_then(|k| lines.get(k)).is_some_and(|t| t.contains("(optional")));
            if !said {
                bad.push(format!("{rel}:{}: io-error-as-empty: a read that fails is not an empty file; return the error (or say \"(optional\" why it may be missing)", line_of(text, at.saturating_add(st.len().saturating_sub(st.trim_start().len())))));
            }
        }
    }
    let defaults: Vec<&str> = p.derive_default.captures_iter(&code).filter_map(|c| c.get(1).or(c.get(2)).map(|m| m.as_str())).collect();
    for m in p.new.captures_iter(&code) {
        let (Some(all), Some(ret)) = (m.get(0), m.get(1)) else { continue };
        let ty = if ret.as_str() == "Self" {
            p.impls.captures_iter(code.get(..all.start()).unwrap_or_default()).last().and_then(|c| c.get(1)).map_or("", |t| t.as_str())
        } else {
            ret.as_str()
        };
        if !defaults.contains(&ty) {
            continue;
        }
        let body = code.get(all.end()..).unwrap_or_default();
        let body = body.get(..body.find('}').unwrap_or(body.len())).unwrap_or_default().trim();
        if body != "Self::default()" && body != format!("{ty}::default()") {
            bad.push(format!("{rel}:{}: default-beside-new: {ty} derives Default, so new() must be Self::default() (or drop the derive)", line_of(text, all.start())));
        }
    }
    bad
}

/// `zip-unchecked`: a test (tests/*.rs) pairs two sequences with pairs::zip_eq, which fails on
/// unequal lengths, never `.zip(`, which stops at the shorter and so compares nothing of an
/// output that lost its lines (the review of 84b05c8, 4.3)
pub fn test_rust(rel: &str, text: &str) -> Vec<String> {
    let code = code_only(text, false);
    code.match_indices(".zip(").map(|(at, _)| format!("{rel}:{}: zip-unchecked: pairs::zip_eq(a, b) (equal lengths or the test fails), not a.zip(b), which stops at the shorter", line_of(text, at))).collect()
}

/// C with its comments blanked (string literals kept), positions kept
fn c_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let next = b.get(i.saturating_add(1)).copied();
        if c == '/' && next == Some('*') {
            let mut j = i.saturating_add(2);
            while j < b.len() && !(b.get(j) == Some(&'*') && b.get(j.saturating_add(1)) == Some(&'/')) {
                j = j.saturating_add(1);
            }
            let end = j.saturating_add(2).min(b.len());
            for &x in b.get(i..end).unwrap_or_default() {
                out.push(if x == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else if c == '/' && next == Some('/') {
            while b.get(i).is_some_and(|&x| x != '\n') {
                out.push(' ');
                i = i.saturating_add(1);
            }
        } else {
            out.push(c);
            i = i.saturating_add(1);
        }
    }
    out
}

/// the problems of one executor source at `rel`
pub fn c(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = c_code(text);
    let mut bad = Vec::new();
    for c in p.silent_cut.captures_iter(&code) {
        let (Some(all), Some(cap)) = (c.get(0), c.get(1)) else { continue };
        if !CUTS.contains(&(rel, cap.as_str())) {
            bad.push(format!("{rel}:{}: silent-cut: a copy stops at {} without refusing the rest; set bad past it", line_of(&code, all.start()), cap.as_str()));
        }
    }
    if rel == "runner/main.c" && code.contains("out(\"case %s") && !rx(r#"out\("case %s\\n"[^;]*\);\s*oflush\(\);"#).is_some_and(|h| h.is_match(&code)) {
        bad.push(format!("{rel}: header-unflushed: oflush() right after a case's header, so a stop inside the case names it"));
    }
    for m in p.wait.find_iter(&code) {
        bad.push(format!("{rel}:{}: wait-unchecked: the result of {} decides whether the GPU finished; check it", line_of(&code, m.start()), m.as_str().trim().trim_end_matches('(').trim()));
    }
    if Path::new(rel).file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with("gdi")) {
        let starts: Vec<(usize, &str)> = p.c_fn.captures_iter(&code).filter_map(|c| Some((c.get(0)?.start(), c.get(1)?.as_str()))).collect();
        for (k, &(at, name)) in starts.iter().enumerate() {
            let end = starts.get(k.saturating_add(1)).map_or(code.len(), |n| n.0);
            let body = code.get(at..end).unwrap_or_default();
            // (the body past the signature: `row` itself, and a function that makes the section)
            let open = body.find('{').map_or(0, |o| o.saturating_add(1));
            let inner = body.get(open..).unwrap_or_default();
            if name == "row" || inner.contains("CreateDIBSection") {
                continue;
            }
            let inner = p.dib_pointer.replace_all(inner, |c: &regex::Captures| " ".repeat(c.get(0).map_or(0, |m| m.as_str().len())));
            if let Some(first) = p.dib.find(&inner) {
                let flushed = inner.get(..first.start()).is_some_and(|p| p.contains("GdiFlush()"));
                if !flushed {
                    bad.push(format!("{rel}:{}: dib-without-flush: {name} touches the DIB section's memory before GdiFlush()", line_of(&code, at.saturating_add(open).saturating_add(first.start()))));
                }
            }
        }
    }
    bad
}

/// every problem in the repository at `root`
pub fn check(root: &Path) -> Result<(), String> {
    let mut bad = Vec::new();
    let mut seen = Vec::new();
    let mut rust_sources: Vec<(String, String)> = Vec::new();
    for (dir, ext) in [("src", "rs"), ("runner", "c")] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
            for e in entries {
                let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some(ext) {
                    continue;
                }
                let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
                bad.extend(if ext == "rs" { rust(&rel, &text) } else { c(&rel, &text) });
                if ext == "rs" {
                    rust_sources.push((rel.clone(), text));
                }
                seen.push(rel);
            }
        }
    }
    for f in MODEL_USES_GENERATORS.iter().map(|g| g.0).chain(CONFINED.iter().map(|c| c.0)).chain(FORBIDDEN.iter().map(|f| f.0).filter(|f| f.starts_with("src/"))).chain(FORBIDDEN_IN.iter().map(|f| f.0)) {
        if !seen.iter().any(|s| s == f) {
            bad.push(format!("{f}: listed in MODEL_USES_GENERATORS but gone: take its line out"));
        }
    }
    for &(c_rel, table, rs_rel, guard) in NODEV_GATES {
        let read = |f: &str| std::fs::read_to_string(root.join(f)).map_err(|e| format!("{f}: {e}"));
        bad.extend(nodev_gates(c_rel, &read(c_rel)?, table, rs_rel, &read(rs_rel)?, guard));
    }
    // (files outside src/ that a forbidden construction names)
    for f in FORBIDDEN.iter().map(|f| f.0).filter(|f| !f.starts_with("src/")) {
        match std::fs::read_to_string(root.join(f)) {
            Ok(t) => {
                bad.extend(rust(f, &t).into_iter().filter(|b| b.contains(": forbidden: ")));
                seen.push(f.to_string());
            }
            Err(e) => bad.push(format!("{f}: {e}")),
        }
    }
    // (the tests' comparisons: tests/*.rs)
    let tests = root.join("tests");
    let mut nt = 0usize;
    for e in std::fs::read_dir(&tests).map_err(|e| format!("{}: {e}", tests.display()))? {
        let p = e.map_err(|e| format!("{}: {e}", tests.display()))?.path();
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
        bad.extend(test_rust(&rel, &text));
        nt = nt.saturating_add(1);
    }
    if nt == 0 {
        bad.push("tests/: no tests found".into());
    }
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).map_err(|e| format!("src/lib.rs: {e}"))?;
    bad.extend(kani_gates(&rust_sources, &lib));
    if seen.is_empty() {
        bad.push("no sources found".into());
    }
    bad.sort();
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "guards_tests.rs"]
mod tests;
