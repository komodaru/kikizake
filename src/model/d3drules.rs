//! The integer rules of drawing on Direct3D 10 and 11 (spec/d3dres.txt, D3DRASTER.*), each a
//! pure function d3draster.rs calls and spec/lean/Kikizake/D3d11/ proves its clauses of, the
//! mirrors (src/lean/d3d11.rs) holding the two equal: the top-left rule, culling, the scissor
//! rectangle, a point's pixel and samples, the sample patterns, which draws run, how the
//! topologies take their vertices, the vertex buffer's words, OMSetRenderTargets' binding, the
//! address modes, the comparison functions, 8-bit bilinear and mip filtering, the minimum and
//! maximum filters, the color write mask.

/// a position in 1/256 pixel (16.8 fixed point), x right and y down
pub type Sub = (i64, i64);

/// twice the signed area of a b c (positive: clockwise on the screen); the snapped coordinates
/// lie within 2^40 (d3draster's snap holds them to 2^31 pixels), so the products stay inside i128
pub fn area(a: Sub, b: Sub, c: Sub) -> i128 {
    let d = |p: i64, q: i64| i128::from(p).saturating_sub(i128::from(q));
    d(b.0, a.0).saturating_mul(d(c.1, a.1)).saturating_sub(d(b.1, a.1).saturating_mul(d(c.0, a.0)))
}

/// a top edge (running right, y the same) or a left one (going up)
fn top_left(a: Sub, b: Sub) -> bool {
    a.1 == b.1 && b.0 > a.0 || b.1 < a.1
}

/// D3DRASTER.TOPLEFT, DEGENERATE: c inside the triangle t by the top-left rule, whichever way
/// t winds; nothing for a triangle of no area
pub fn covers(t: [Sub; 3], c: Sub) -> bool {
    let [p, q, r] = t;
    let s = area(p, q, r);
    let t = if s == 0 {
        return false;
    } else if s > 0 {
        [(p, q), (q, r), (r, p)]
    } else {
        [(p, r), (r, q), (q, p)]
    };
    t.iter().all(|&(a, b)| {
        let w = area(a, b, c);
        w > 0 || w == 0 && top_left(a, b)
    })
}

/// D3DRASTER.CULL: whether D3D11_CULL_MODE (NONE 1, FRONT 2, BACK 3) culls a triangle, clockwise
/// on the screen or not, FrontCounterClockwise or not
pub fn culls(mode: u32, front_ccw: bool, cw: bool) -> bool {
    let front = cw != front_ccw;
    mode == 2 && front || mode == 3 && !front
}

/// D3DRASTER.SCISSOR: pixel p inside the scissor rectangle (left, top, right, bottom) when
/// ScissorEnable is on
pub fn scissor_holds(on: bool, r: [i64; 4], p: (i64, i64)) -> bool {
    let [l, t, rt, b] = r;
    !on || p.0 >= l && p.0 < rt && p.1 >= t && p.1 < b
}

/// D3DRASTER.POINT: the pixel a point snapped at s (one axis, 1/256) lights: ceil(s / 256) - 1
pub fn point_pixel(s: i64) -> i64 {
    s.saturating_sub(1).div_euclid(256)
}

/// a sample of pixel p, o/16 pixel off its center, in 1/256
pub fn sample_sub(p: i64, o: i64) -> i64 {
    p.saturating_mul(256).saturating_add(128).saturating_add(o.saturating_mul(16))
}

/// D3DRASTER.MS.POINTS: a sample (in 1/256) inside the pixel square about a point snapped at s:
/// its left and top edges in, its right and bottom ones out
pub fn point_covers(s: Sub, sample: Sub) -> bool {
    let inside = |c: i64, v: i64| c.saturating_sub(128) <= v && v < c.saturating_add(128);
    inside(s.0, sample.0) && inside(s.1, sample.1)
}

/// D3DRASTER.MS.SAMPLES: the samples of a multisampled target (quality 0) at
/// D3D11_STANDARD_MULTISAMPLE_PATTERN's positions, in 1/16 pixel from the pixel center
pub fn sample_pattern(n: u32) -> Option<&'static [(i64, i64)]> {
    match n {
        2 => Some(&[(4, 4), (-4, -4)]),
        4 => Some(&[(-2, -6), (6, -2), (-6, 2), (2, 6)]),
        8 => Some(&[(1, -3), (-1, 3), (5, 1), (-3, -5), (-5, 5), (-7, -1), (3, 7), (7, -7)]),
        // (MEASURED, WARP: probe-rt-ms16)
        16 => Some(&[(1, 1), (-1, -3), (-3, 2), (4, -1), (-5, -2), (2, 5), (5, 3), (3, -5), (-2, 6), (0, -7), (-4, -6), (-6, 4), (-8, 0), (7, -4), (6, 7), (-7, -8)]),
        _ => None,
    }
}

/// D3DRASTER.MS.SAMPLES: the samples of pixel p a triangle t covers (by the top-left rule)
pub fn sample_mask(t: [Sub; 3], p: (i64, i64), pattern: &[(i64, i64)]) -> Vec<bool> {
    pattern.iter().map(|&(ox, oy)| covers(t, (sample_sub(p.0, ox), sample_sub(p.1, oy)))).collect()
}

/// the primitive topologies drawn (D3D11_PRIMITIVE_TOPOLOGY 1 to 5, 10 to 13)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topo {
    Points,
    Lines,
    LineStrip,
    Tris,
    TriStrip,
    LinesAdj,
    LineStripAdj,
    TrisAdj,
    TriStripAdj,
}

impl Topo {
    pub fn of(top: u32) -> Option<Topo> {
        match top {
            1 => Some(Topo::Points),
            2 => Some(Topo::Lines),
            3 => Some(Topo::LineStrip),
            4 => Some(Topo::Tris),
            5 => Some(Topo::TriStrip),
            10 => Some(Topo::LinesAdj),
            11 => Some(Topo::LineStripAdj),
            12 => Some(Topo::TrisAdj),
            13 => Some(Topo::TriStripAdj),
            _ => None,
        }
    }

    pub fn lines(self) -> bool {
        matches!(self, Topo::Lines | Topo::LineStrip | Topo::LinesAdj | Topo::LineStripAdj)
    }
}

/// what a Draw does
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// (D3DRASTER.INCOMPLETE) no programs and input layout, or neither a target nor a depth
    /// view: nothing drawn
    Incomplete,
    /// (D3DRASTER.NOVIEWPORT) nothing drawn
    NoViewport,
    /// (D3DRASTER.PATCH) the executor ends
    Crash,
    /// (D3DRASTER.TOPOLOGY) the device is removed
    Removed,
    /// (D3DRASTER.TOPOLOGY) topology 6: nothing drawn
    Nothing,
    /// the primitives drawn
    Draw(Topo),
}

/// D3DRASTER.INCOMPLETE, NOVIEWPORT, TOPOLOGY, PATCH: what a draw does with programs and an
/// input layout bound or not, a target or depth view or not, a viewport or not, of topology top
pub fn draw_verdict(programs: bool, view: bool, viewport: bool, top: u32) -> Verdict {
    if !programs || !view {
        return Verdict::Incomplete;
    }
    if !viewport {
        return Verdict::NoViewport;
    }
    match Topo::of(top) {
        Some(t) => Verdict::Draw(t),
        None if (33..=64).contains(&top) => Verdict::Crash,
        None if top == 6 => Verdict::Nothing,
        None => Verdict::Removed,
    }
}

/// The view used for the draw's extent, with a color write only when a color target is bound.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetPlan<V> {
    None,
    Color(V),
    DepthOnly(V),
}

pub fn target_plan<V>(target: Option<V>, depth: Option<V>) -> TargetPlan<V> {
    match (target, depth) {
        (Some(view), _) => TargetPlan::Color(view),
        (None, Some(view)) => TargetPlan::DepthOnly(view),
        (None, None) => TargetPlan::None,
    }
}

/// The drawable rows and the starting element and slice of the view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TargetRows {
    pub width: u32,
    pub height: u32,
    pub first: u32,
    pub slice: u32,
}

pub fn target_rows(dim: u32, view: u32, (width, height, depth): (u32, u32, u32), (first, slice): (u32, u32)) -> Option<TargetRows> {
    let (height, first, slice) = match (dim, view) {
        (1, 2 | 3) => (1, 0, 0),
        (2, _) => (height, 0, 0),
        (3, 8) if slice < depth => (height, 0, slice),
        (4, 1) => (1, first, 0),
        _ => return None,
    };
    Some(TargetRows { width, height, first, slice })
}

/// a primitive's vertices (their indices in the draw) and its leading vertex
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prim {
    Point(u32),
    Line(u32, u32),
    Tri([u32; 3], u32),
}

/// D3DRASTER.STRIP, ADJ: primitive p's vertices: every other triangle of a strip with its first
/// two swapped; with adjacency (and no geometry program) the adjacent vertices skipped (None
/// past u32)
pub fn primitive(topo: Topo, p: u32) -> Option<Prim> {
    let at = |k: u32, a: u32| p.checked_mul(k)?.checked_add(a);
    let odd = p % 2 == 1;
    Some(match topo {
        Topo::Points => Prim::Point(p),
        Topo::Lines => Prim::Line(at(2, 0)?, at(2, 1)?),
        Topo::LineStrip => Prim::Line(p, at(1, 1)?),
        Topo::LinesAdj => Prim::Line(at(4, 1)?, at(4, 2)?),
        Topo::LineStripAdj => Prim::Line(at(1, 1)?, at(1, 2)?),
        Topo::Tris => Prim::Tri([at(3, 0)?, at(3, 1)?, at(3, 2)?], at(3, 0)?),
        Topo::TrisAdj => Prim::Tri([at(6, 0)?, at(6, 2)?, at(6, 4)?], at(6, 0)?),
        Topo::TriStrip if odd => Prim::Tri([at(1, 1)?, p, at(1, 2)?], p),
        Topo::TriStrip => Prim::Tri([p, at(1, 1)?, at(1, 2)?], p),
        Topo::TriStripAdj if odd => Prim::Tri([at(2, 2)?, at(2, 0)?, at(2, 4)?], at(2, 0)?),
        Topo::TriStripAdj => Prim::Tri([at(2, 0)?, at(2, 2)?, at(2, 4)?], at(2, 0)?),
    })
}

/// the primitives a draw of count vertices makes, at most those that reach the nv vertices the
/// buffer holds (past them every vertex reads zeros, D3DRASTER.VBPAST: nothing to draw)
pub fn prim_count(topo: Topo, count: u32, nv: u64) -> u64 {
    let c = u64::from(count);
    match topo {
        Topo::Points => c.min(nv),
        Topo::Lines => (c / 2).min(nv.div_ceil(2)),
        Topo::LineStrip => c.saturating_sub(1).min(nv),
        Topo::Tris => (c / 3).min(nv.div_ceil(3)),
        Topo::TriStrip => c.saturating_sub(2).min(nv),
        Topo::LinesAdj => (c / 4).min(nv.div_ceil(4)),
        Topo::LineStripAdj => c.saturating_sub(3).min(nv),
        Topo::TrisAdj => (c / 6).min(nv.div_ceil(6)),
        Topo::TriStripAdj => (c / 2).saturating_sub(2).min(nv.div_ceil(2)),
    }
}

/// D3DRASTER.VBPAST: word k of vertex v (stride bytes apart; the executor's 32-bit offset) of the
/// vertex buffer's words, 0 past them
pub fn vertex_word(vtx: &[u32], v: u32, stride: u32, k: u32) -> u32 {
    let i = (v.wrapping_mul(stride) / 4).wrapping_add(k);
    usize::try_from(i).ok().and_then(|i| vtx.get(i)).copied().unwrap_or(0)
}

/// a view OMSetRenderTargets is given: its resource's sample count, its first subresource's
/// width and height (None: no resource)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bound {
    pub samples: u32,
    pub size: Option<(u32, u32)>,
}

/// what OMSetRenderTargets does
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bind {
    /// the call changes nothing: the views bound before stay
    Keep,
    /// the target given is bound, with the depth view given or without one
    Set { depth: bool },
}

/// which Direct3D binds
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Api {
    D10,
    D101,
    D11,
}

/// D3DRASTER.MS.DEPTHCOUNT, DEPTH.SMALL: OMSetRenderTargets with a target and a depth view (or
/// either alone)
pub fn bind_targets(api: Api, rt: Option<Bound>, ds: Option<Bound>) -> Bind {
    let Some((a, b)) = rt.zip(ds) else {
        return Bind::Set { depth: ds.is_some() };
    };
    let smaller = matches!((b.size, a.size), (Some((dw, dh)), Some((w, h))) if dw < w || dh < h);
    match api {
        Api::D11 if a.samples != b.samples || smaller => Bind::Keep,
        Api::D10 if a.samples != b.samples => Bind::Set { depth: false },
        Api::D10 | Api::D101 | Api::D11 => Bind::Set { depth: true },
    }
}

/// D3DRASTER.ADDRESS, BORDER: a texel coordinate (in texels) of a texture size texels wide
/// through D3D11_TEXTURE_ADDRESS_MODE (WRAP 1, MIRROR 2, CLAMP 3, BORDER 4, MIRROR_ONCE 5);
/// None: outside the texture, the border color
pub fn address(mode: u32, i: i64, size: i64) -> Option<i64> {
    let last = size.checked_sub(1)?;
    Some(match mode {
        1 => i.checked_rem_euclid(size)?,
        2 => {
            let k = i.checked_rem_euclid(size.checked_mul(2)?)?;
            if k < size { k } else { last.checked_mul(2)?.checked_add(1)?.checked_sub(k)? }
        }
        3 => i.clamp(0, last.max(0)),
        4 => {
            if i < 0 || i >= size {
                return None;
            }
            i
        }
        _ => {
            let k = if i < 0 { i.checked_add(1)?.checked_neg()? } else { i };
            k.min(last)
        }
    })
}

/// D3DRASTER.DEPTH, SAMPLE.CMP: D3D11_COMPARISON_FUNC (NEVER 1, LESS 2, EQUAL 3, LESS_EQUAL 4,
/// GREATER 5, NOT_EQUAL 6, GREATER_EQUAL 7, ALWAYS 8) of a new value against a stored one
pub fn compare<T: PartialOrd>(func: u32, a: T, s: T) -> bool {
    match func {
        1 => false,
        2 => a < s,
        3 => a == s,
        4 => a <= s,
        5 => a > s,
        6 => a != s,
        7 => a >= s,
        _ => true,
    }
}

/// D3DRASTER.FILTER.LINEAR: 8-bit texels a (i, j), b (i + 1, j), c (i, j + 1), e (i + 1, j + 1)
/// filtered with the weights wu, wv (in 1/256): down the columns first, halved, then across, in
/// 1/128 of a byte
pub fn bilinear8(a: i64, b: i64, c: i64, e: i64, wu: i64, wv: i64) -> i64 {
    let mix = |x: i64, y: i64, w: i64| 256i64.saturating_sub(w).saturating_mul(x).saturating_add(y.saturating_mul(w));
    let l = mix(a, c, wv) >> 1;
    let r = mix(b, e, wv) >> 1;
    mix(l, r, wu) >> 8
}

/// D3DRASTER.FILTER.MIPLINEAR: two levels' 8-bit filtered values (in 1/128) by the next level's
/// weight f (in 1/256)
pub fn mip8(n0: i64, n1: i64, f: i64) -> i64 {
    256i64.saturating_sub(f).saturating_mul(n0).saturating_add(n1.saturating_mul(f)) >> 8
}

/// a minimum or maximum filter's reduction
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Least,
    Greatest,
}

/// D3DRASTER.FILTER.MINMAX: of the 2 x 2 texels a (i, j), b (i + 1, j), c (i, j + 1), e
/// (i + 1, j + 1) per component the least or greatest of those whose weight is not 0
pub fn reduce(pick: Pick, [a, b, c, e]: [[f32; 4]; 4], (wu, wv): (i64, i64)) -> [f32; 4] {
    let take: [(bool, [f32; 4]); 4] = [(true, a), (wu != 0, b), (wv != 0, c), (wu != 0 && wv != 0, e)];
    [0usize, 1, 2, 3].map(|k| {
        let got = |t: &[f32; 4]| t.get(k).copied().unwrap_or(0.0);
        take.iter().filter(|t| t.0).map(|t| got(&t.1)).fold(got(&a), |x, y| match pick {
            Pick::Least => x.min(y),
            Pick::Greatest => x.max(y),
        })
    })
}

/// D3DRASTER.WRITEMASK: an element written through the mask (1 red, 2 green, 4 blue, 8 alpha):
/// the bits of each component (comps: its bits and its channel, in the element's order from bit
/// 0) the mask names from new, the others kept from old
pub fn merge(comps: &[(u32, u32)], old: &[u8], new: &[u8], mask: u32) -> Vec<u8> {
    let mut out = old.to_vec();
    let mut at = 0u32;
    for &(bits, chan) in comps {
        let end = at.saturating_add(bits);
        if mask.checked_shr(chan).is_some_and(|m| m & 1 == 1) {
            for b in at..end {
                let (byte, bit) = (usize::try_from(b / 8).unwrap_or(usize::MAX), b % 8);
                let fresh = new.get(byte).copied().unwrap_or(0) & (1 << bit);
                if let Some(o) = out.get_mut(byte) {
                    *o = *o & !(1 << bit) | fresh;
                }
            }
        }
        at = end;
    }
    out
}
