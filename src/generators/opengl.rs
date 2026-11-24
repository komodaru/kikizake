//! The opengl suite's case generator: WGL pixel formats and contexts on DIB sections, then
//! OpenGL 1.1 state, errors, matrices, the per-fragment operations and pixel transfers, read back
//! with glReadPixels and from the DIB itself.
use super::*;
use crate::model::opengl::{e::*, pname, CAPS, PNAMES};

const ORTHO: &str = "gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n";
/// the pixel formats asked for (PFD_DRAW_TO_BITMAP | PFD_SUPPORT_GDI | PFD_SUPPORT_OPENGL, RGBA, depth 32, stencil 8)
fn ctx(bpp: u32) -> String {
    format!("glctx {bpp} choose 0x38 0 {bpp} 0 0 32 8\n")
}

fn h(v: u32) -> String {
    if v < 10 { v.to_string() } else { format!("{v:#x}") }
}

/// a float the executor reads back exactly (short decimal of a dyadic value)
fn f(v: f64) -> String {
    format!("{v}")
}

fn gl(s: &mut String, name: &str, args: &[u32]) {
    s.push_str("gl ");
    s.push_str(name);
    for a in args {
        s.push(' ');
        s.push_str(&h(*a));
    }
    s.push('\n');
}

fn glf(s: &mut String, name: &str, args: &[f64]) {
    s.push_str("gl ");
    s.push_str(name);
    for a in args {
        s.push(' ');
        s.push_str(&f(*a));
    }
    s.push('\n');
}

/// a dyadic float in lo..hi with 1/256 steps, now and then an edge
fn rf(r: &mut XorShift32, lo: f64, hi: f64) -> f64 {
    if r.chance(6) {
        return *r.pick(&[lo, hi, 0.0, 1.0, 0.5, -1.0, 2.0]);
    }
    let steps = ((hi - lo) * 256.0) as i64;
    lo + r.range(0, steps) as f64 / 256.0
}

const BLEND_SRC: &[u32] = &[ZERO, ONE, DST_COLOR, ONE_MINUS_DST_COLOR, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DST_ALPHA, ONE_MINUS_DST_ALPHA, SRC_ALPHA_SATURATE];
const BLEND_DST: &[u32] = &[ZERO, ONE, SRC_COLOR, ONE_MINUS_SRC_COLOR, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DST_ALPHA, ONE_MINUS_DST_ALPHA];
const FUNCS: [u32; 8] = [0x200, 0x201, 0x202, 0x203, 0x204, 0x205, 0x206, 0x207];
const SOPS: &[u32] = &[KEEP, ZERO, REPLACE, INCR, DECR, INVERT];
pub(crate) const READ_FORMATS: &[u32] = &[RED, GREEN, BLUE, ALPHA, RGB, RGBA, LUMINANCE, LUMINANCE_ALPHA, BGR_EXT, BGRA_EXT, DEPTH_COMPONENT, STENCIL_INDEX, COLOR_INDEX];
pub(crate) const TYPES: &[u32] = &[UNSIGNED_BYTE, BYTE, UNSIGNED_SHORT, SHORT, UNSIGNED_INT, INT, FLOAT, BITMAP];

/// the destination: random RGBA bytes over the whole DIB, drawn with glDrawPixels
fn random_dst(s: &mut String, seed: u32) {
    writeln!(s, "gldraw 16 16 {} {} {seed} 1024", h(RGBA), h(UNSIGNED_BYTE));
}

fn read_all(s: &mut String) {
    writeln!(s, "glread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
}

pub fn opengl() -> String {
    let mut s = String::from("# gpu\n# kikizake suite opengl: WGL and OpenGL 1.1 on DIB sections (SPEC.md). Made by `kikizake gen opengl`.\n");
    let mut r = XorShift32::new(0x0960_1011);

    // every pixel format on DIBs of each depth
    for bpp in [1, 4, 8, 16, 24, 32] {
        writeln!(s, "case pf-{bpp}\nglpf {bpp}\nend");
    }
    // ChoosePixelFormat: each request field alone, then random requests
    let flags = [0x38, 0x28, 0x29, 0x2c, 0x3c, 0x39, 0x08, 0x20, 0x00, 0x18, 0x2d];
    for bpp in [8, 16, 24, 32] {
        writeln!(s, "case choose-fields-{bpp}");
        for fl in flags {
            writeln!(s, "glchoose {bpp} {} 0 {bpp} 0 0 32 8", h(fl));
        }
        for (ty, color, alpha, accum, depth, stencil) in [(1, bpp, 0, 0, 32, 8), (0, 0, 0, 0, 0, 0), (0, 8, 0, 0, 32, 8), (0, 16, 0, 0, 32, 8), (0, 24, 0, 0, 32, 8), (0, 32, 0, 0, 32, 8), (0, bpp, 8, 0, 32, 8), (0, bpp, 0, 64, 32, 8), (0, bpp, 0, 0, 16, 8), (0, bpp, 0, 0, 24, 8), (0, bpp, 0, 0, 0, 0), (0, bpp, 0, 0, 32, 0), (0, bpp, 0, 0, 16, 0), (0, bpp, 0, 32, 16, 4)] {
            writeln!(s, "glchoose {bpp} 0x38 {ty} {color} {alpha} {accum} {depth} {stencil}");
        }
        writeln!(s, "end");
    }
    for i in 0..12 {
        writeln!(s, "case choose-{i}");
        for _ in 0..16 {
            let bpp = *r.pick(&[8, 16, 24, 32]);
            let fl = *r.pick(&flags) | if r.chance(5) { 0x2 } else { 0 };
            let pick = |r: &mut XorShift32, v: &[u32]| *r.pick(v);
            writeln!(s, "glchoose {bpp} {} {} {} {} {} {} {}", h(fl), pick(&mut r, &[0, 0, 0, 1]), pick(&mut r, &[0, 4, 8, 15, 16, 24, 32, 48]), pick(&mut r, &[0, 0, 1, 8]), pick(&mut r, &[0, 0, 16, 32, 64]), pick(&mut r, &[0, 8, 16, 24, 32]), pick(&mut r, &[0, 0, 1, 4, 8, 16]));
        }
        writeln!(s, "end");
    }

    // contexts: the current context and DC, the strings, released
    for bpp in [8, 16, 24, 32] {
        writeln!(s, "case ctx-{bpp}\nglcur\n{}glcur", ctx(bpp));
        for n in [VENDOR, RENDERER, VERSION, EXTENSIONS, 0, 0x1f04] {
            writeln!(s, "glstr {}", h(n));
        }
        writeln!(s, "glerr\nglrelease\nglcur\nend");
    }
    // explicit formats: each of the first 24 on a 32-bit DIB
    for pf in 1..=24 {
        writeln!(s, "case ctx-pf-{pf}\nglctx 32 {pf}\nglget i {} 16\nglget i {} 16\nglget i {} 16\nglget i {} 16\nglget b {} 1\nend", h(pname("RED_BITS")), h(pname("DEPTH_BITS")), h(pname("STENCIL_BITS")), h(pname("ALPHA_BITS")), h(pname("DOUBLEBUFFER")));
    }

    // the initial state: every glGet parameter as integers, floats, booleans, doubles
    for (kind, bpps) in [("i", &[16u32, 24, 32][..]), ("f", &[32]), ("b", &[32]), ("d", &[32])] {
        for &bpp in bpps {
            writeln!(s, "case defaults-{kind}-{bpp}\n{}", ctx(bpp).trim_end());
            for p in PNAMES {
                writeln!(s, "glget {kind} {} 16 # {}", h(p.1), p.0);
            }
            writeln!(s, "glerr\nend");
        }
    }
    writeln!(s, "case defaults-caps\n{}", ctx(32).trim_end());
    for c in CAPS {
        writeln!(s, "glisen {}", h(*c));
    }
    writeln!(s, "glerr\nend");
    // names glGet and glIsEnabled do not know
    writeln!(s, "case defaults-unknown\n{}", ctx(32).trim_end());
    for p in [0, 1, 0xb0a, 0xb0f, 0xc03, 0xd06, 0xdff, 0x1000, 0xffff, 0x8000] {
        writeln!(s, "glget i {} 2\nglerr\nglisen {}\nglerr", h(p), h(p));
    }
    writeln!(s, "end");

    // errors: each call, then glGetError
    let errs: &[(&str, &[u32])] = &[
        ("glEnable", &[0]), ("glEnable", &[0xffff]), ("glDisable", &[0xb0a]), ("glMatrixMode", &[0x1703]), ("glMatrixMode", &[0]),
        ("glDepthFunc", &[0x1ff]), ("glDepthFunc", &[0x208]), ("glLogicOp", &[0x14ff]), ("glLogicOp", &[0x1510]),
        ("glBlendFunc", &[SRC_COLOR, ZERO]), ("glBlendFunc", &[ONE_MINUS_SRC_COLOR, ZERO]), ("glBlendFunc", &[ZERO, DST_COLOR]),
        ("glBlendFunc", &[ZERO, ONE_MINUS_DST_COLOR]), ("glBlendFunc", &[ZERO, SRC_ALPHA_SATURATE]), ("glBlendFunc", &[2, ZERO]),
        ("glStencilFunc", &[0x208, 0, 0]), ("glStencilOp", &[0x1e04, KEEP, KEEP]), ("glStencilOp", &[KEEP, KEEP, 0]),
        ("glShadeModel", &[0x1d02]), ("glCullFace", &[0x406]), ("glFrontFace", &[0x902]), ("glHint", &[0xc50, 0x1103]),
        ("glHint", &[0xc55, FASTEST]), ("glPolygonMode", &[FRONT, 0x1b03]), ("glPolygonMode", &[0x406, 0x1b02]),
        ("glPixelStorei", &[PACK_ALIGNMENT, 3]), ("glPixelStorei", &[PACK_ALIGNMENT, 0]), ("glPixelStorei", &[PACK_ALIGNMENT, 16]),
        ("glPixelStorei", &[UNPACK_ALIGNMENT, 5]), ("glPixelStorei", &[PACK_ROW_LENGTH, 0xffffffff]),
        ("glPixelStorei", &[PACK_SKIP_ROWS, 0xffffffff]), ("glPixelStorei", &[UNPACK_SKIP_PIXELS, 0xffffffff]), ("glPixelStorei", &[0xd06, 1]),
        ("glViewport", &[0, 0, 0xffffffff, 1]), ("glViewport", &[0, 0, 1, 0xffffffff]), ("glScissor", &[0, 0, 0xffffffff, 1]),
        ("glScissor", &[0, 0, 1, 0xfffffffe]), ("glClear", &[0x1]), ("glClear", &[0x8000]), ("glClear", &[0xffffffff]),
        ("glDrawBuffer", &[0x40c]), ("glDrawBuffer", &[0x401]), ("glDrawBuffer", &[0x409]), ("glReadBuffer", &[0x401]), ("glReadBuffer", &[0x409]),
        ("glReadBuffer", &[0x1234]), ("glPopMatrix", &[]), ("glPopAttrib", &[]), ("glEnd", &[]), ("glAccum", &[0x100, 0]),
        ("glBegin", &[10]), ("glPushAttrib", &[0]),
    ];
    writeln!(s, "case errors\n{}", ctx(32).trim_end());
    for (name, args) in errs {
        if *name == "glAccum" {
            writeln!(s, "gl glAccum 0x100 0.5");
        } else {
            gl(&mut s, name, args);
        }
        writeln!(s, "glerr");
    }
    // float arguments out of range
    for (name, v) in [("glLineWidth", 0.0), ("glLineWidth", -1.0), ("glPointSize", 0.0), ("glPointSize", -0.5), ("glLineWidth", 0.25), ("glPointSize", 1000.0)] {
        glf(&mut s, name, &[v]);
        writeln!(s, "glerr");
    }
    writeln!(s, "gl glAlphaFunc 0x208 0.5\nglerr\nend");
    // several errors before glGetError: which are kept
    writeln!(s, "case errors-queue\n{}gl glEnable 0\ngl glPixelStorei 0xd05 3\ngl glPopMatrix\nglerr\nglerr\nend", ctx(32));
    // inside glBegin / glEnd only some calls are allowed
    writeln!(s, "case errors-begin\n{}gl glBegin 7\ngl glEnable 0xbe2\ngl glEnd\nglerr\nglisen 0xbe2", ctx(32));
    writeln!(s, "gl glBegin 7\ngl glColor4ub 1 2 3 4\ngl glVertex2f 1 1\ngl glEnd\nglerr\nglget i 0xb00 4\nglerr\ngl glBegin 7\ngl glBegin 7\nglerr\ngl glEnd\nglerr\ngl glBegin 7\nglget i 0xb00 4\ngl glEnd\nglerr\nend");

    // set then get: enumerated state
    writeln!(s, "case set-blendfunc\n{}", ctx(32).trim_end());
    for src in BLEND_SRC.iter().chain(&[SRC_COLOR, ONE_MINUS_SRC_COLOR]) {
        for dst in BLEND_DST.iter().chain(&[DST_COLOR, SRC_ALPHA_SATURATE]) {
            gl(&mut s, "glBlendFunc", &[*src, *dst]);
            writeln!(s, "glget i 0xbe1 2\nglget i 0xbe0 2\nglerr");
        }
    }
    writeln!(s, "end");
    writeln!(s, "case set-enums\n{}", ctx(32).trim_end());
    for fu in FUNCS.iter().chain(&[0x1ff, 0x208]) {
        gl(&mut s, "glDepthFunc", &[*fu]);
        writeln!(s, "glget i 0xb74 1\nglerr");
        writeln!(s, "gl glAlphaFunc {} 0.25\nglget i 0xbc1 1\nglerr", h(*fu));
        gl(&mut s, "glStencilFunc", &[*fu, 3, 7]);
        writeln!(s, "glget i 0xb92 1\nglerr");
    }
    for op in 0x1500..=0x1510 {
        gl(&mut s, "glLogicOp", &[op]);
        writeln!(s, "glget i 0xbf0 1\nglerr");
    }
    for sop in SOPS.iter().chain(&[0x1e04, 0]) {
        gl(&mut s, "glStencilOp", &[*sop, KEEP, REPLACE]);
        gl(&mut s, "glStencilOp", &[KEEP, *sop, REPLACE]);
        writeln!(s, "glget i 0xb94 3\nglget i 0xb95 1\nglerr");
    }
    for (pn, p) in [(SMOOTH, 0xb54), (FLAT, 0xb54)] {
        gl(&mut s, "glShadeModel", &[pn]);
        writeln!(s, "glget i {} 1", h(p));
    }
    for m in [MODELVIEW, PROJECTION, TEXTURE] {
        gl(&mut s, "glMatrixMode", &[m]);
        writeln!(s, "glget i 0xba0 1");
    }
    for (t, v) in [(0xc50, NICEST), (0xc51, FASTEST), (0xc52, DONT_CARE), (0xc53, NICEST), (0xc54, FASTEST)] {
        gl(&mut s, "glHint", &[t, v]);
        writeln!(s, "glget i {} 1", h(t));
    }
    writeln!(s, "glerr\nend");
    // set then get: numbers, with the clamping each call does
    for i in 0..24 {
        writeln!(s, "case set-values-{i}\n{}", ctx(*r.pick(&[16, 24, 32])).trim_end());
        for _ in 0..6 {
            let kind = *r.pick(&["i", "f"]);
            match r.range(0, 11) {
                0 => {
                    let c: Vec<f64> = (0..4).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glClearColor", &c);
                    writeln!(s, "glget {kind} 0xc22 4");
                }
                1 => {
                    let c: Vec<f64> = (0..2).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glDepthRange", &c);
                    writeln!(s, "glget {kind} 0xb70 2");
                }
                2 => {
                    let v = rf(&mut r, -1.5, 2.5);
                    glf(&mut s, "glClearDepth", &[v]);
                    writeln!(s, "glget {kind} 0xb73 1");
                }
                3 => {
                    let v = rf(&mut r, -1.5, 2.5);
                    writeln!(s, "gl glAlphaFunc {} {}\nglget {kind} 0xbc2 1", h(*r.pick(&FUNCS)), f(v));
                }
                4 => {
                    let (rf_, m) = (*r.pick(&[0, 1, 255, 256, 257, 0xffff_ffff, 0x8000_0000, 1000]), *r.pick(&[0, 1, 0xff, 0x1ff, 0xffff_ffff]));
                    gl(&mut s, "glStencilFunc", &[*r.pick(&FUNCS), rf_, m]);
                    writeln!(s, "glget i 0xb97 1\nglget i 0xb93 1");
                }
                5 => {
                    let v = *r.pick(&[0, 1, 0xff, 0x100, 0xffff_ffff, 0x1234_5678]);
                    gl(&mut s, "glStencilMask", &[v]);
                    gl(&mut s, "glClearStencil", &[v]);
                    writeln!(s, "glget i 0xb98 1\nglget i 0xb91 1");
                }
                6 => {
                    let v: Vec<u32> = (0..4).map(|_| *r.pick(&[0, 1, 2, 0xff, 0x100])).collect();
                    gl(&mut s, "glColorMask", &v);
                    writeln!(s, "glget i 0xc23 4\nglget b 0xc23 4");
                }
                7 => {
                    let v: Vec<u32> = (0..4).map(|_| *r.pick(&[0, 1, 3, 16, 0xffff_fff0, 5000, 0x7fff_ffff])).collect();
                    gl(&mut s, if r.chance(2) { "glViewport" } else { "glScissor" }, &v);
                    writeln!(s, "glget i 0xba2 4\nglget i 0xc10 4");
                }
                8 => {
                    let v = rf(&mut r, -2.0, 300.0);
                    glf(&mut s, if r.chance(2) { "glLineWidth" } else { "glPointSize" }, &[v]);
                    writeln!(s, "glget f 0xb21 1\nglget f 0xb11 1");
                }
                9 => {
                    let p = *r.pick(&[PACK_ALIGNMENT, UNPACK_ALIGNMENT, PACK_ROW_LENGTH, PACK_SKIP_ROWS, UNPACK_SKIP_PIXELS, PACK_SWAP_BYTES, UNPACK_LSB_FIRST]);
                    let v = *r.pick(&[1, 2, 4, 8, 0, 3, 7, 100]);
                    gl(&mut s, "glPixelStorei", &[p, v]);
                    writeln!(s, "glget i {} 1\nglget b {} 1", h(p), h(p));
                }
                _ => {
                    let c: Vec<f64> = (0..4).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glColor4f", &c);
                    writeln!(s, "glget {kind} 0xb00 4");
                }
            }
            writeln!(s, "glerr");
        }
        writeln!(s, "end");
    }

    // matrices: each operation from identity, then random sequences; the stacks' depths
    let mat = |s: &mut String, r: &mut XorShift32| match r.range(0, 7) {
        0 => {
            let v: Vec<f64> = (0..3).map(|_| rf(r, -8.0, 8.0)).collect();
            glf(s, "glTranslatef", &v);
        }
        1 => {
            let v: Vec<f64> = (0..3).map(|_| rf(r, -4.0, 4.0)).collect();
            glf(s, "glScalef", &v);
        }
        2 => {
            let a = *r.pick(&[0.0, 90.0, 180.0, 270.0, -90.0, 45.0, 30.0, 360.0, 1.5, 720.0]);
            let mut ax: Vec<f64> = (0..3).map(|_| *r.pick(&[0.0, 0.0, 1.0, -1.0, 2.0, 0.5])).collect();
            if ax.iter().all(|v| *v == 0.0) {
                ax[2] = 1.0;
            }
            glf(s, "glRotatef", &[a, ax[0], ax[1], ax[2]]);
        }
        3 => {
            let (l, b, n) = (rf(r, -8.0, 8.0), rf(r, -8.0, 8.0), rf(r, -4.0, 4.0));
            let (w, ht, d) = (*r.pick(&[1.0, 2.0, 16.0, 0.5, 3.0]), *r.pick(&[1.0, 4.0, 16.0, 0.25]), *r.pick(&[1.0, 2.0, 8.0, 100.0]));
            glf(s, "glOrtho", &[l, l + w, b, b + ht, n, n + d]);
        }
        4 => {
            let (l, b) = (rf(r, -2.0, 0.0), rf(r, -2.0, 0.0));
            let (w, ht) = (*r.pick(&[1.0, 2.0, 4.0, 0.5]), *r.pick(&[1.0, 2.0, 0.25]));
            let n = *r.pick(&[0.5, 1.0, 2.0, 0.125]);
            glf(s, "glFrustum", &[l, l + w, b, b + ht, n, n + *r.pick(&[1.0, 10.0, 100.0])]);
        }
        5 => {
            let m: Vec<String> = (0..16).map(|_| f(rf(r, -4.0, 4.0))).collect();
            writeln!(s, "glmat {} {}", if r.chance(2) { "load" } else { "mult" }, m.join(" "));
        }
        _ => {
            writeln!(s, "gl glLoadIdentity");
        }
    };
    for i in 0..40 {
        writeln!(s, "case matrix-{i}\n{}", ctx(32).trim_end());
        let mode = *r.pick(&[MODELVIEW, PROJECTION, TEXTURE]);
        gl(&mut s, "glMatrixMode", &[mode]);
        for _ in 0..(if i < 14 { 1 } else { r.range(2, 5) }) {
            mat(&mut s, &mut r);
        }
        let p = match mode {
            MODELVIEW => MODELVIEW_MATRIX,
            PROJECTION => PROJECTION_MATRIX,
            _ => TEXTURE_MATRIX,
        };
        writeln!(s, "glget f {} 16\nglget d {} 16\nglerr\nend", h(p), h(p));
    }
    for (mode, depth) in [(MODELVIEW, MODELVIEW_STACK_DEPTH), (PROJECTION, PROJECTION_STACK_DEPTH), (TEXTURE, TEXTURE_STACK_DEPTH)] {
        writeln!(s, "case matrix-stack-{mode:x}\n{}", ctx(32).trim_end());
        gl(&mut s, "glMatrixMode", &[mode]);
        for k in 0..34 {
            writeln!(s, "gl glTranslatef {k} 0 0\ngl glPushMatrix\nglget i {} 1\nglerr", h(depth));
        }
        writeln!(s, "glget f {} 16", h(mode - 0x1700 + MODELVIEW_MATRIX));
        for _ in 0..35 {
            writeln!(s, "gl glPopMatrix\nglget i {} 1\nglerr", h(depth));
        }
        writeln!(s, "glget f {} 16\nend", h(mode - 0x1700 + MODELVIEW_MATRIX));
    }

    // the framebuffer: clear colors, then what each DIB depth holds
    for bpp in [16, 24, 32] {
        for i in 0..4 {
            writeln!(s, "case clear-{bpp}-{i}\n{}", ctx(bpp).trim_end());
            for d in [true, false] {
                if !d {
                    writeln!(s, "gl glDisable 0xbd0");
                }
                let c: Vec<f64> = (0..4).map(|_| rf(&mut r, 0.0, 1.0)).collect();
                glf(&mut s, "glClearColor", &c);
                writeln!(s, "gl glClear 0x4000\ngldib");
                read_all(&mut s);
            }
            writeln!(s, "glerr\nend");
        }
    }
    // clear levels: every 8-bit level of a channel (float k/255 and k/256) on 32 bpp
    for (name, div) in [("255", 255.0), ("256", 256.0)] {
        for part in 0..4 {
            writeln!(s, "case clear-levels-{name}-{part}\n{}gl glDisable 0xbd0", ctx(32));
            for k in (part * 64)..(part * 64 + 64) {
                let v = k as f64 / div;
                // (k/255 is not dyadic: printed with 9 digits, read back to the nearest float)
                let t = if div == 255.0 { format!("{:.9}", v as f32) } else { f(v) };
                writeln!(s, "gl glClearColor {t} {t} {t} {t}\ngl glClear 0x4000\nglread 0 0 1 1 {} {} 4", h(RGBA), h(UNSIGNED_BYTE));
            }
            writeln!(s, "end");
        }
    }

    // rectangles: which pixels a rectangle covers (integer, quarter and half coordinates)
    for i in 0..30 {
        writeln!(s, "case rect-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000", ctx(32));
        for _ in 0..4 {
            let q = if i < 10 { 1.0 } else if i < 20 { 4.0 } else { 2.0 };
            let c: Vec<f64> = (0..4).map(|_| r.range(-8, 24 * q as i64) as f64 / q).collect();
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            glf(&mut s, "glRectf", &c);
        }
        read_all(&mut s);
        writeln!(s, "glerr\nend");
    }
    // points at pixel centers and in between
    for i in 0..6 {
        writeln!(s, "case points-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glBegin 0", ctx(32));
        for _ in 0..24 {
            let q = if i < 3 { 2.0 } else { 8.0 };
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let (x, y) = (r.range(0, 16 * q as i64 - 1) as f64 / q, r.range(0, 16 * q as i64 - 1) as f64 / q);
            let (x, y) = if i < 3 { (x.floor() + 0.5, y.floor() + 0.5) } else { (x, y) };
            glf(&mut s, "glVertex2f", &[x, y]);
        }
        writeln!(s, "gl glEnd");
        read_all(&mut s);
        writeln!(s, "glerr\nend");
    }

    // per-fragment operations, over random destinations drawn with glDrawPixels
    let rect_all = "gl glRectf 0 0 16 16\n";
    // blending: every factor pair, a few source colors
    for (si, src) in BLEND_SRC.iter().enumerate() {
        for (di, dst) in BLEND_DST.iter().enumerate() {
            writeln!(s, "case blend-{si}-{di}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbe2", ctx(32));
            gl(&mut s, "glBlendFunc", &[*src, *dst]);
            for k in 0..2 {
                writeln!(s, "gl glDisable 0xbe2");
                random_dst(&mut s, 100 + (si * 16 + di) as u32 * 2 + k);
                writeln!(s, "gl glEnable 0xbe2");
                let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
                gl(&mut s, "glColor4ub", &col);
                s += rect_all;
                read_all(&mut s);
            }
            writeln!(s, "glerr\nend");
        }
    }
    // blending arithmetic: SRC_ALPHA / ONE_MINUS_SRC_ALPHA at every source alpha
    for part in 0..8 {
        writeln!(s, "case blend-alpha-{part}\n{}{ORTHO}gl glDisable 0xbd0\ngl glBlendFunc 0x302 0x303", ctx(32));
        for k in 0..32 {
            let a = part * 32 + k;
            writeln!(s, "gl glDisable 0xbe2");
            random_dst(&mut s, 5000 + a);
            writeln!(s, "gl glEnable 0xbe2");
            gl(&mut s, "glColor4ub", &[r.next() & 255, r.next() & 255, r.next() & 255, a]);
            s += rect_all;
            read_all(&mut s);
        }
        writeln!(s, "end");
    }
    // logic operations
    writeln!(s, "case logicop\n{}{ORTHO}gl glDisable 0xbd0", ctx(32));
    for op in 0x1500..=0x150f {
        writeln!(s, "gl glDisable 0xbf2");
        random_dst(&mut s, 300 + op);
        writeln!(s, "gl glEnable 0xbf2");
        gl(&mut s, "glLogicOp", &[op]);
        gl(&mut s, "glColor4ub", &[0x5a, 0xc3, 0x0f, 0x99]);
        s += rect_all;
        read_all(&mut s);
    }
    writeln!(s, "glerr\nend");
    // alpha test: every function, references around the source alpha
    for (fi, fu) in FUNCS.iter().enumerate() {
        writeln!(s, "case alphatest-{fi}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbc0", ctx(32));
        for k in 0..8 {
            let reference = *r.pick(&[0.0, 1.0, 0.5, 0.25, 0.75, 0.501_960_784, 0.498_039_216, 0.003_921_569]);
            writeln!(s, "gl glAlphaFunc {} {}", h(*fu), reference);
            writeln!(s, "gl glDisable 0xbc0");
            random_dst(&mut s, 400 + fi as u32 * 8 + k);
            writeln!(s, "gl glEnable 0xbc0");
            // (a column of 16 source alphas per rectangle row)
            for y in 0..16 {
                let a = *r.pick(&[0, 1, 63, 64, 127, 128, 129, 191, 192, 254, 255]);
                gl(&mut s, "glColor4ub", &[r.next() & 255, r.next() & 255, r.next() & 255, a]);
                writeln!(s, "gl glRecti 0 {y} 16 {}", y + 1);
            }
            read_all(&mut s);
        }
        writeln!(s, "glerr\nend");
    }
    // depth test: quads at depths, every function, depth mask
    for (fi, fu) in FUNCS.iter().enumerate() {
        writeln!(s, "case depth-{fi}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71", ctx(32));
        writeln!(s, "gl glClearDepth {}\ngl glClear 0x4100", f(*r.pick(&[0.5, 1.0, 0.25])));
        for k in 0..6 {
            gl(&mut s, "glDepthFunc", &[if k == 0 { ALWAYS } else { *fu }]);
            gl(&mut s, "glDepthMask", &[(k != 3) as u32]);
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let z = *r.pick(&[0.0, 0.5, -0.5, 0.25, -0.25, 0.75, -1.0, 1.0]);
            let (x0, y0) = (r.range(0, 8), r.range(0, 8));
            let (x1, y1) = (x0 + r.range(4, 12), y0 + r.range(4, 12));
            writeln!(s, "gl glBegin 7");
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                writeln!(s, "gl glVertex3f {x} {y} {}", f(z));
            }
            writeln!(s, "gl glEnd");
        }
        read_all(&mut s);
        writeln!(s, "glread 0 0 16 16 {} {} 1024\nglerr\nend", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
    }
    // stencil: functions, operations, masks
    for i in 0..16 {
        writeln!(s, "case stencil-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb90", ctx(32));
        writeln!(s, "gl glClearStencil {}\ngl glClear 0x4400", r.range(0, 255));
        for _ in 0..6 {
            gl(&mut s, "glStencilFunc", &[*r.pick(&FUNCS), *r.pick(&[0, 1, 2, 127, 128, 255, 256]), *r.pick(&[0xff, 0x0f, 0xf0, 0xffff_ffff, 1])]);
            gl(&mut s, "glStencilOp", &[*r.pick(SOPS), *r.pick(SOPS), *r.pick(SOPS)]);
            gl(&mut s, "glStencilMask", &[*r.pick(&[0xff, 0x0f, 0xf0, 0xffff_ffff, 0])]);
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let (x0, y0) = (r.range(0, 10), r.range(0, 10));
            writeln!(s, "gl glRecti {x0} {y0} {} {}", x0 + r.range(2, 10), y0 + r.range(2, 10));
        }
        read_all(&mut s);
        writeln!(s, "glread 0 0 16 16 {} {} 256\nglerr\nend", h(STENCIL_INDEX), h(UNSIGNED_BYTE));
    }
    // scissor and color mask
    for i in 0..8 {
        writeln!(s, "case scissor-mask-{i}\n{}{ORTHO}gl glDisable 0xbd0", ctx(*r.pick(&[16, 24, 32])));
        random_dst(&mut s, 600 + i);
        for _ in 0..3 {
            let (x, y) = (r.range(-4, 14), r.range(-4, 14));
            writeln!(s, "gl glEnable 0xc11\ngl glScissor {x} {y} {} {}", r.range(0, 12), r.range(0, 12));
            let m: Vec<u32> = (0..4).map(|_| r.range(0, 1) as u32).collect();
            gl(&mut s, "glColorMask", &m);
            let c: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            if r.chance(2) {
                gl(&mut s, "glColor4ub", &c);
                s += rect_all;
            } else {
                glf(&mut s, "glClearColor", &c.iter().map(|v| *v as f64 / 256.0).collect::<Vec<_>>());
                writeln!(s, "gl glClear 0x4000");
            }
        }
        read_all(&mut s);
        writeln!(s, "gldib\nglerr\nend");
    }
    // dithering on 16 bpp: a smooth gradient of colors with dithering on and off
    for (i, d) in [true, false].iter().enumerate() {
        writeln!(s, "case dither-{i}\n{}{ORTHO}gl {} 0xbd0", ctx(16), if *d { "glEnable" } else { "glDisable" });
        for y in 0..16 {
            for x in 0..16 {
                gl(&mut s, "glColor4ub", &[x * 16 + y, y * 16 + x, (x * y) & 255, 255]);
                writeln!(s, "gl glRecti {x} {y} {} {}", x + 1, y + 1);
            }
        }
        writeln!(s, "gldib");
        read_all(&mut s);
        writeln!(s, "end");
    }

    // glReadPixels: every format and type over a random picture, then the pack parameters
    // (the stencil and depth buffers cleared to values: before their first clear they hold what the
    // memory held, the stencil 0xbe on one executor and 1 on the other in this case's first answers)
    writeln!(s, "case read-formats\n{}gl glDisable 0xbd0\ngl glClearStencil 0xb5\ngl glClearDepth 0.75\ngl glClear 0x500", ctx(32));
    random_dst(&mut s, 700);
    for fm in READ_FORMATS {
        for ty in TYPES {
            writeln!(s, "glread 1 2 3 2 {} {} 128\nglerr", h(*fm), h(*ty));
        }
    }
    writeln!(s, "glread 0 0 16 16 {} {} 1024\nglread 0 0 16 16 {} {} 2048\nend", h(RGBA), h(FLOAT), h(RGB), h(UNSIGNED_SHORT));
    for i in 0..24 {
        writeln!(s, "case read-pack-{i}\n{}gl glDisable 0xbd0", ctx(32));
        random_dst(&mut s, 800 + i);
        for _ in 0..3 {
            for (p, vals) in [(PACK_ALIGNMENT, &[1u32, 2, 4, 8][..]), (PACK_ROW_LENGTH, &[0, 0, 5, 17]), (PACK_SKIP_ROWS, &[0, 0, 1, 3]), (PACK_SKIP_PIXELS, &[0, 0, 1, 2]), (PACK_SWAP_BYTES, &[0, 0, 1]), (PACK_LSB_FIRST, &[0, 0, 1])] {
                gl(&mut s, "glPixelStorei", &[p, *r.pick(vals)]);
            }
            let (x, y) = (r.range(0, 12), r.range(0, 12));
            let (w, ht) = (r.range(1, 5), r.range(1, 4));
            let fm = *r.pick(&[RGB, RGBA, LUMINANCE, ALPHA, BGRA_EXT, RED, LUMINANCE_ALPHA]);
            let ty = *r.pick(&[UNSIGNED_BYTE, BYTE, UNSIGNED_SHORT, SHORT, INT, FLOAT, BITMAP]);
            writeln!(s, "glread {x} {y} {w} {ht} {} {} 400\nglerr", h(fm), h(ty));
        }
        writeln!(s, "end");
    }
    // glDrawPixels: every format and type, then the unpack parameters and the pixel transfer
    let draw_formats = [RED, GREEN, BLUE, ALPHA, RGB, RGBA, LUMINANCE, LUMINANCE_ALPHA, BGR_EXT, BGRA_EXT];
    for (fi, fm) in draw_formats.iter().enumerate() {
        writeln!(s, "case draw-{fi}\n{}gl glDisable 0xbd0", ctx(32));
        for (ti, ty) in TYPES[..7].iter().enumerate() {
            writeln!(s, "gl glClear 0x4000\ngldraw 16 2 {} {} {} 2048\nglerr", h(*fm), h(*ty), 900 + fi * 8 + ti);
            writeln!(s, "glread 0 0 16 2 {} {} 128", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "end");
    }
    for i in 0..16 {
        writeln!(s, "case draw-unpack-{i}\n{}gl glDisable 0xbd0\ngl glClear 0x4000", ctx(32));
        for (p, vals) in [(UNPACK_ALIGNMENT, &[1u32, 2, 4, 8][..]), (UNPACK_ROW_LENGTH, &[0, 0, 7, 20]), (UNPACK_SKIP_ROWS, &[0, 0, 2]), (UNPACK_SKIP_PIXELS, &[0, 0, 1, 3]), (UNPACK_SWAP_BYTES, &[0, 0, 1])] {
            gl(&mut s, "glPixelStorei", &[p, *r.pick(vals)]);
        }
        if i >= 8 {
            for p in [RED_SCALE, GREEN_BIAS, BLUE_SCALE, ALPHA_BIAS] {
                writeln!(s, "gl glPixelTransferf {} {}", h(p), f(*r.pick(&[0.5, 2.0, -0.25, 0.25, 1.0])));
            }
        }
        let fm = *r.pick(&[RGB, RGBA, LUMINANCE, BGRA_EXT]);
        let ty = *r.pick(&[UNSIGNED_BYTE, UNSIGNED_SHORT, SHORT, FLOAT]);
        writeln!(s, "gldraw {} {} {} {} {} 4096\nglerr", r.range(1, 9), r.range(1, 6), h(fm), h(ty), 1000 + i);
        read_all(&mut s);
        writeln!(s, "end");
    }

    measure(&mut s);

    // edges: each case ends at one answer the model leaves open, or checks an error path
    let edges: &[(&str, &str)] = &[
        ("attrib", "gl glPushAttrib 0x4000\nglerr\nglget i 0xbb0 1\ngl glPopAttrib\nglerr\nglget i 0xbb0 1\nglerr"),
        ("attrib-overflow", "PUSH17glerr\nglget i 0xbb0 1\nglerr"),
        ("accum", "gl glAccum 0x100 0.5\nglerr\ngl glAccum 0x101 1\nglerr\ngl glAccum 0x103 0.5\nglerr\ngl glAccum 0x104 0.25\nglerr\ngl glAccum 0x105 1\nglerr\ngl glAccum 0xff 1\nglerr\ngl glClearAccum 0.5 0.25 0 1\ngl glClear 0x200\nglerr\ngl glAccum 0x102 1\nglerr\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("buffer", "gl glDrawBuffer 0x404\nglerr\ngl glReadBuffer 0x406\nglerr\nglget i 0xc01 1\nglget i 0xc02 1\ngl glDrawBuffer 0x400\ngl glReadBuffer 0x400\nglerr\nglget i 0xc01 1\nglget i 0xc02 1\ngl glDrawBuffer 0x408\nglerr\nglget i 0xc01 1\ngl glReadBuffer 0x408\nglerr\ngl glReadBuffer 0\nglerr\ngl glDrawBuffer 0x405\ngl glReadBuffer 0x405\nglerr\ngl glDrawBuffer 0\nglerr\nglget i 0xc01 1\ngl glClear 0x4000\nglread 0 0 4 1 0x1908 0x1401 16"),
        ("begin-query", "gl glBegin 7\nglisen 0xbe2\ngl glEnd\nglerr"),
        ("flush", "gl glClear 0x4000\ngl glFlush\ngl glFinish\nglerr"),
        ("pixels-errors", "glread 0 0 1 1 0x1234 0x1401 16\nglerr\nglread 0 0 1 1 0x1908 0x1234 16\nglerr\nglread 0 0 -1 1 0x1908 0x1401 16\nglerr\nglread 0 0 1 -1 0x1908 0x1401 16\nglerr\nglread 0 0 1 1 0x1900 0x1401 16\nglerr\ngldraw 1 1 0x1234 0x1401 1 16\nglerr\ngldraw -1 1 0x1908 0x1401 1 16\nglerr\ngldraw 1 1 0x1900 0x1401 1 16\nglerr\nglread 0 0 1 1 0x1900 0x1a00 16\nglerr"),
        ("draw-bitmap", "gldraw 8 1 0x1908 0x1a00 1 16\nglerr"),
        // (the depth buffer cleared first: before its first clear it holds what the memory held on
        // the x86-64 executor)
        ("draw-depth", "gl glClearDepth 0.25\ngl glClear 0x100\ngldraw 2 1 0x1902 0x1405 1 16\nglerr\nglread 0 0 2 1 0x1902 0x1405 8"),
        ("read-outside", "glread 14 14 4 4 0x1908 0x1401 64\nglerr"),
        ("blend-alpha-buffer", "BLEND"),
        ("depth-16", "DEPTH16"),
        ("dib-8", "DIB8"),
        ("transfer-map", "gl glPixelTransferf 0xd10 1\nglerr"),
        ("raster-clip", "ORTHOgl glBegin 7\ngl glVertex3f 1 1 2\ngl glVertex3f 5 1 2\ngl glVertex3f 5 5 2\ngl glVertex3f 1 5 2\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("raster-perspective", "gl glMatrixMode 0x1701\ngl glFrustum -1 1 -1 1 1 3\ngl glMatrixMode 0x1700\ngl glBegin 7\ngl glVertex3f -0.5 -0.5 -2\ngl glVertex3f 0.5 -0.5 -2\ngl glVertex3f 0.5 0.5 -2\ngl glVertex3f -0.5 0.5 -2\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("raster-interpolate", "ORTHOgl glBegin 7\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1 1\ngl glColor4ub 0 255 0 255\ngl glVertex2f 9 1\ngl glVertex2f 9 9\ngl glVertex2f 1 9\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("raster-polygon", "ORTHOgl glBegin 7\ngl glVertex2f 1 1\ngl glVertex2f 9 2\ngl glVertex2f 8 9\ngl glVertex2f 2 8\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("raster-triangles", "ORTHOgl glBegin 4\ngl glVertex2f 1 1\ngl glVertex2f 9 1\ngl glVertex2f 1 9\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024"),
        ("vertex-outside", "ORTHOgl glVertex2f 1 1\nglerr\nglread 0 0 4 4 0x1908 0x1401 64"),
        ("other-call", "gl glPixelZoom 2 2\nglerr"),
        ("pf-range", "PFRANGE"),
        ("bitmap-read-stencil", "gl glClearStencil 0xa5\ngl glClear 0x400\nglread 0 0 16 2 0x1901 0x1a00 8\nglerr\ngl glPixelStorei 0xd01 1\nglread 0 0 16 2 0x1901 0x1a00 8\nglerr"),
        ("bitmap-draw-stencil", "gldraw 16 2 0x1901 0x1a00 5 4\nglerr\nglread 0 0 16 2 0x1901 0x1401 32"),
        ("bitmap-draw-index", "gldraw 16 2 0x1900 0x1a00 5 4\nglerr\nglread 0 0 16 2 0x1908 0x1401 128"),
    ];
    for (name, body) in edges {
        let body = match *body {
            "PUSH17" => "gl glPushAttrib 0x4000\n".repeat(17),
            b if b.starts_with("ORTHO") => format!("{ORTHO}gl glDisable 0xbd0\n{}", &b[5..]),
            b => b.to_string(),
        };
        let body = body.replace("PUSH17", &"gl glPushAttrib 0x4000\n".repeat(17));
        let pre = match *name {
            // a format with alpha bits: blending reads the destination alpha
            "blend-alpha-buffer" => format!("glctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbe2\ngl glBlendFunc 0x304 0x305\ngl glColor4ub 200 100 50 128\ngl glRectf 0 0 8 8\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr"),
            // the second format: a 16-bit depth buffer
            "depth-16" => format!("glctx 32 2\nglget i 0xd56 1\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glRectf 0 0 8 8\nglread 0 0 16 16 0x1902 0x1405 1024\nglerr"),
            // an 8-bit DIB in RGB 3-3-2 (the format of index 25)
            // a pixel format past the table
            "pf-range" => "glctx 32 99\nglcur".to_string(),
            "dib-8" => "glctx 8 25\ngl glDisable 0xbd0\ngl glClearColor 1 0.5 0.25 1\ngl glClear 0x4000\ngldib\nglerr".to_string(),
            _ => String::new(),
        };
        if pre.is_empty() {
            writeln!(s, "case edge-{name}\n{}{body}\nend", ctx(32));
        } else {
            writeln!(s, "case edge-{name}\n{pre}\nend");
        }
    }
    let mut last = String::new();
    textures_more(&mut s, &mut last);
    arrays_more(&mut s);
    eye_more(&mut s);
    raster_rest(&mut s);
    attrib_more(&mut last);
    lists_more(&mut last);
    s + &last
}

/// display lists the review of 84b05c8 found handled wrong: a proxy texture image made while a
/// list compiles (run at once, not compiled; GL_COMPILE and GL_COMPILE_AND_EXECUTE), and
/// glDeleteLists over a range past 2^20 with one list at its end
fn lists_more(s: &mut String) {
    let q = "gltexq level 0x8064 0 0x1000 i 1";
    for (mode, name) in [(0x1300u32, "compile"), (0x1301, "compile-execute")] {
        writeln!(s, "case list-more-proxy-{name}\n{}{ORTHO}gl glNewList 1 {mode:#x}\ngltex 0x8064 0 4 8 8 0 0x1908 0x1401 21 256\n{q}\ngl glEndList\n{q}\ngltex 0x8064 0 4 4 4 0 0x1908 0x1401 21 64\n{q}\ngl glCallList 1\n{q}\nglerr\nend", ctx(32));
    }
    for (name, del) in [("far", "1 1048577"), ("near", "1048577 1"), ("short", "1 1048576")] {
        writeln!(s, "case list-more-delete-{name}\n{}gl glNewList 1048577 0x1300\ngl glColor4ub 1 2 3 4\ngl glEndList\nglr glIsList b i 1048577\ngl glDeleteLists {del}\nglr glIsList b i 1048577\nglerr\nend", ctx(32));
    }
}

/// glPushAttrib / glPopAttrib of each group the review of 84b05c8 (3.3) found left out, with the
/// state changed between: pushed with its group (back as it was) and with another (as changed)
fn attrib_more(s: &mut String) {
    let groups: [(&str, &str, &str, &str); 5] = [
        ("current", "gl glTexCoord4f 0.125 0.25 0.375 0.5\ngl glRasterPos2f 3 4\ngl glIndexf 2", "gl glTexCoord4f 0.5 0.625 0.75 0.875\ngl glColor4ub 10 20 30 40\ngl glRasterPos2f 7 9\ngl glIndexf 5", "glget f 0xb03 4\nglget f 0xb07 4\nglget f 0xb06 4\nglget f 0xb04 4\nglget i 0xb08 1\nglget f 0xb01 1"),
        ("line", "gl glLineStipple 2 0xaaaa\ngl glLineWidth 2", "gl glLineStipple 3 0x5555\ngl glLineWidth 3", "glget i 0xb25 1\nglget i 0xb26 1\nglget f 0xb21 1"),
        ("list", "gl glListBase 3", "gl glListBase 5", "glget i 0xb32 1"),
        ("color", "gl glClearIndex 3\ngl glIndexMask 5", "gl glClearIndex 6\ngl glIndexMask 9", "glget f 0xc20 1\nglget i 0xc21 1"),
        ("pixel", "gl glPixelZoom 2 3\ngl glPixelTransferi 0xd12 2\ngl glPixelTransferf 0xd1e 0.5", "gl glPixelZoom 4 5\ngl glPixelTransferi 0xd12 3\ngl glPixelTransferf 0xd1e 0.25", "glget f 0xd16 1\nglget f 0xd17 1\nglget i 0xd12 1\nglget f 0xd1e 1"),
    ];
    let bits = [("current", 0x1u32), ("line", 0x4), ("list", 0x20000), ("color", 0x4000), ("pixel", 0x20)];
    for (name, before, change, get) in groups {
        for (pushed, mask) in bits {
            if pushed != name && pushed != "line" && !(name == "line" && pushed == "list") {
                continue;
            }
            writeln!(s, "case attrib-more-{name}-pushed-{pushed}\n{}{ORTHO}gl glDisable 0xbd0\n{before}\ngl glPushAttrib {mask:#x}\n{change}\ngl glPopAttrib\n{get}\nglerr\nend", ctx(32));
        }
    }
}

/// what GL.RASTER.MODE still holds after raster-more-*: antialiased lines blended with the
/// source alpha and other destination factors, in 32 and 16 bits; a smooth polygon reaching
/// behind the eye with face culling; after every other case
fn raster_rest(s: &mut String) {
    let blends: [(&str, &str); 6] = [("one", "0x302 0x1"), ("zero", "0x302 0x0"), ("src-color", "0x302 0x300"), ("one-minus-dst", "0x302 0x307"), ("one-msa", "0x1 0x303"), ("dst-one", "0x306 0x1")];
    for bits in [32u32, 16] {
        for (name, f) in blends {
            writeln!(s, "case raster-rest-aa-line-{bits}-{name}\nglctx {bits} choose 0x38 0 {bits} 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc {f}\ngl glColor4ub 200 150 100 180\ngl glBegin 1\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glVertex2f 2.5 13.5\ngl glVertex2f 12.25 1.5\ngl glEnd");
            read_all(s);
            writeln!(s, "glerr\nend");
        }
    }
    for (name, cull) in [("back", "0x405"), ("front", "0x404")] {
        for (order, v) in [("ccw", ["-1 -1 -2", "1 -0.5 -2", "0 1 1"]), ("cw", ["1 -0.5 -2", "-1 -1 -2", "0 1 1"])] {
            writeln!(s, "case raster-rest-smooth-behind-{name}-{order}\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glFrustum -1 1 -1 1 1 10\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\ngl glEnable 0xb44\ngl glCullFace {cull}\ngl glBegin 4", ctx(32));
            for (k, p) in v.iter().enumerate() {
                let c = ["255 0 0", "0 255 0", "0 0 255"].get(k).copied().unwrap_or("0 0 0");
                writeln!(s, "gl glColor4ub {c} 255\ngl glVertex3f {p}");
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glerr\nend");
        }
    }
}

/// eye planes (GL.TEX.GEN) through modelviews past tex-open-more-eye-rot: the modelview as
/// glGetFloatv gives it, then the unit planes, each giving a row of its inverse; affine ones,
/// a projective one, one nearly singular; after every other case
fn eye_more(s: &mut String) {
    let sets: [(&str, &str); 6] = [
        ("rot", "gl glTranslatef 3 -1 0.5\ngl glRotatef 30 0.2 0.3 1\ngl glScalef 0.7 1.3 0.9"),
        ("rot-only", "gl glRotatef 47 1 -2 0.5"),
        ("scale", "gl glScalef 0.3 7 -2.5"),
        ("shear", "glmat mult 1 0.25 0 0 0.5 1 0.125 0 0 -0.75 1 0 2 3 -1 1"),
        ("projective", "glmat mult 1 0.25 0 0.125 0.5 1 0.125 -0.25 0 -0.75 1 0.5 2 3 -1 1"),
        ("near-singular", "glmat mult 1 2 3 0 2 4.0001 6 0 1 1 1 0 0 0 0 1"),
    ];
    for (name, m) in sets {
        writeln!(s, "case tex-eye-more-{name}\n{}{ORTHO}gl glDisable 0xbd0\n{m}\nglget f 0xba6 16", ctx(32));
        for k in 0..4u32 {
            let unit: Vec<&str> = (0..4u32).map(|j| if j == k { "1" } else { "0" }).collect();
            writeln!(s, "gl glTexGeni {} 9472 9216\ngltexv gen {} 9474 f {}", 8192u32.saturating_add(k), 8192u32.saturating_add(k), unit.join(" "));
        }
        for k in 0..4u32 {
            writeln!(s, "gltexq gen {} 9474 f 4", 8192u32.saturating_add(k));
        }
        writeln!(s, "glerr\nend");
    }
}

/// vertex arrays (GL.ARRAY) past gap-array-0, 1: every type, layout and error, lists, edge
/// flags; after every other case
fn arrays_more(s: &mut String) {
    let head = |s: &mut String, name: &str| {
        writeln!(s, "case array-more-{name}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000", ctx(32));
    };
    let end = |s: &mut String| {
        read_all(s);
        writeln!(s, "glget f 0xb00 4\nglerr\nend");
    };
    // points at (2 + 3k, 2 + 3j): the vertex array of 8 points, floats
    let points = "glp glVertexPointer iii 2 0x1406 0 f 2.5 2.5 5.5 2.5 8.5 2.5 11.5 2.5 2.5 5.5 5.5 5.5 8.5 5.5 11.5 5.5\ngl glEnableClientState 0x8074";
    // colors of each type (the extremes, a middle and negatives)
    let colors: [(&str, u32, &str); 8] = [
        ("b", 0x1400, "b 127 -128 0 127 64 -64 -1 127 1 2 3 127 -127 100 -100 127 0 0 0 127 50 60 70 127 -1 -2 -3 127 10 -10 20 127"),
        ("ub", 0x1401, "ub 255 0 128 255 1 2 3 255 254 127 129 255 10 20 30 255 200 100 50 255 0 0 0 255 77 88 99 255 255 255 255 0"),
        ("s", 0x1402, "s 32767 -32768 0 32767 16384 -16384 -1 32767 1 2 3 32767 -32767 1000 -1000 32767 0 0 0 32767 300 400 500 32767 -1 -2 -3 32767 100 -100 200 32767"),
        ("us", 0x1403, "us 65535 0 32768 65535 1 2 3 65535 65534 32767 32769 65535 100 200 300 65535 50000 25000 12500 65535 0 0 0 65535 7777 8888 9999 65535 65535 65535 65535 0"),
        ("i", 0x1404, "i 2147483647 -2147483648 0 2147483647 1073741824 -1073741824 -1 2147483647 1 2 3 2147483647 -2147483647 1000000 -1000000 2147483647 0 0 0 2147483647 300000000 400000000 500000000 2147483647 -1 -2 -3 2147483647 100 -100 200 2147483647"),
        ("ui", 0x1405, "ui 4294967295 0 2147483648 4294967295 1 2 3 4294967295 4294967294 2147483647 2147483649 4294967295 100 200 300 4294967295 3000000000 1500000000 750000000 4294967295 0 0 0 4294967295 7777 8888 9999 4294967295 4294967295 4294967295 4294967295 0"),
        ("f", 0x1406, "f 1 0 0.5 1 0.25 0.75 0.1 1 2 -1 0.33 1 0.001 0.999 0.5 1 0.6 0.3 0.2 1 0 0 0 1 0.4 0.5 0.6 1 1 1 1 0"),
        ("d", 0x140a, "d 1 0 0.5 1 0.25 0.75 0.1 1 2 -1 0.33 1 0.001 0.999 0.5 1 0.6 0.3 0.2 1 0 0 0 1 0.4 0.5 0.6 1 1 1 1 0"),
    ];
    for (name, ty, values) in colors {
        for size in [4, 3] {
            head(s, &format!("color-{name}-{size}"));
            writeln!(s, "{points}\nglp glColorPointer iii {size} {ty:#x} {} {values}\ngl glEnableClientState 0x8076\ngl glPointSize 2\ngl glDrawArrays 0 0 8", if size == 3 { type_bytes(ty).saturating_mul(4) } else { 0 });
            end(s);
        }
    }
    // vertices of each type and size
    let verts: [(&str, u32, &str); 4] = [
        ("s", 0x1402, "s 2 2 0 1 13 4 0 1 6 13 0 1"),
        ("i", 0x1404, "i 2 2 0 1 13 4 0 1 6 13 0 1"),
        ("f", 0x1406, "f 2.25 2.5 0.5 1 13.75 4.125 -0.5 1 6.5 13.25 0 1"),
        ("d", 0x140a, "d 2.25 2.5 0.5 1 13.75 4.125 -0.5 1 6.5 13.25 0 1"),
    ];
    for (name, ty, values) in verts {
        for size in [2, 3, 4] {
            head(s, &format!("vertex-{name}-{size}"));
            writeln!(s, "glp glVertexPointer iii {size} {ty:#x} {} {values}\ngl glEnableClientState 0x8074\ngl glColor4ub 200 150 100 255\ngl glDrawArrays 4 0 3", type_bytes(ty).saturating_mul(4));
            end(s);
        }
    }
    // a vertex of w 2 (size 4), strides, normals through lighting, texture coordinates, indices
    head(s, "vertex-w");
    writeln!(s, "glp glVertexPointer iii 4 0x1406 0 f 4 4 0 2 26 8 0 2 12 26 0 2\ngl glEnableClientState 0x8074\ngl glColor4ub 200 150 100 255\ngl glDrawArrays 4 0 3");
    end(s);
    head(s, "stride");
    writeln!(s, "glp glVertexPointer iii 2 0x1406 20 f 2 2 9 9 9 13 4 9 9 9 6 13 9 9 9\ngl glEnableClientState 0x8074\nglp glColorPointer iii 4 0x1401 8 ub 255 0 0 255 9 9 9 9 0 255 0 255 9 9 9 9 0 0 255 255 9 9 9 9\ngl glEnableClientState 0x8076\ngl glDrawArrays 4 0 3\nglget i 0x807c 1\nglget i 0x8083 1");
    end(s);
    for (name, ty, values) in [("b", 0x1400, "b 0 0 127 90 0 90 -128 0 0"), ("s", 0x1402, "s 0 0 32767 23170 0 23170 -32768 0 0"), ("i", 0x1404, "i 0 0 2147483647 1518500249 0 1518500249 -2147483648 0 0"), ("f", 0x1406, "f 0 0 1 0.7 0 0.7 -1 0 0"), ("d", 0x140a, "d 0 0 1 0.7 0 0.7 -1 0 0")] {
        head(s, &format!("normal-{name}"));
        writeln!(s, "glp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13\ngl glEnableClientState 0x8074\nglp glNormalPointer ii {ty:#x} 0 {values}\ngl glEnableClientState 0x8075\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glDrawArrays 4 0 3\nglget f 0xb02 3");
        end(s);
    }
    for (name, ty, size, values) in [("s-2", 0x1402, 2, "s 0 0 1 0 0 1"), ("i-1", 0x1404, 1, "i 0 1 2"), ("f-3", 0x1406, 3, "f 0 0 0 1 0 0 0.5 1 0"), ("d-4", 0x140a, 4, "d 0 0 0 1 2 0 0 2 0.5 1 0 1")] {
        head(s, &format!("tex-{name}"));
        writeln!(s, "gltex 3553 0 4 4 4 0 0x1908 0x1401 26 64\ngl glTexParameteri 3553 10241 9728\ngl glTexParameteri 3553 10240 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable 3553\nglp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13\ngl glEnableClientState 0x8074\nglp glTexCoordPointer iii {size} {ty:#x} 0 {values}\ngl glEnableClientState 0x8078\ngl glDrawArrays 4 0 3\nglget f 0xb03 4");
        end(s);
    }
    writeln!(s, "case array-more-index\nglctx 8 choose 0x38 1 8 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\nglp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13\ngl glEnableClientState 0x8074\nglp glIndexPointer ii 0x1401 0 ub 5 9 12\ngl glEnableClientState 0x8077\ngl glShadeModel 0x1d00\ngl glDrawArrays 4 0 3\nglread 0 0 16 16 0x1900 0x1401 256\nglget f 0xb01 1\nglerr\nend");
    // glDrawElements of each index type, glArrayElement outside glBegin, the vertex array off
    for (name, ty, values) in [("ub", 0x1401, "ub 7 0 5 2"), ("us", 0x1403, "us 7 0 5 2"), ("ui", 0x1405, "ui 7 0 5 2")] {
        head(s, &format!("elements-{name}"));
        writeln!(s, "{points}\ngl glPointSize 2\nglp glDrawElements iii 0 4 {ty:#x} {values}");
        end(s);
    }
    head(s, "outside");
    writeln!(s, "{points}\nglp glColorPointer iii 4 0x1401 0 ub 255 0 0 255 0 255 0 255 0 0 255 255 9 9 9 9\ngl glEnableClientState 0x8076\ngl glArrayElement 1\nglget f 0xb00 4\ngl glArrayElement 2");
    end(s);
    head(s, "no-vertex");
    writeln!(s, "glp glColorPointer iii 4 0x1401 0 ub 255 0 0 255 0 255 0 255 0 0 255 255\ngl glEnableClientState 0x8076\ngl glDrawArrays 4 0 3");
    end(s);
    head(s, "enable");
    writeln!(s, "glp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13\ngl glEnable 0x8074\nglerr\nglisen 0x8074\ngl glDrawArrays 4 0 3\ngl glDisable 0x8074\nglerr\nglisen 0x8074");
    end(s);
    // errors and their order
    head(s, "errors");
    for line in [
        "glp glVertexPointer iii 1 0x1406 0 f 0 0", "glp glVertexPointer iii 2 0x1401 0 f 0 0", "glp glVertexPointer iii 2 0x1406 -4 f 0 0", "glp glVertexPointer iii 1 0x1401 -4 f 0 0", "glp glVertexPointer iii 5 0x1406 0 f 0 0",
        "glp glColorPointer iii 2 0x1401 0 ub 0 0", "glp glColorPointer iii 4 0x1409 0 ub 0 0", "glp glNormalPointer ii 0x1401 0 ub 0 0", "glp glNormalPointer ii 0x1406 -1 f 0 0", "glp glIndexPointer ii 0x1400 0 b 0", "glp glIndexPointer ii 0x1401 0 ub 0",
        "glp glTexCoordPointer iii 0 0x1406 0 f 0", "glp glTexCoordPointer iii 4 0x1401 0 ub 0", "glp glEdgeFlagPointer i -1 ub 0", "glp glInterleavedArrays ii 0x2a1f 0 f 0", "glp glInterleavedArrays ii 0x2a20 -8 f 0",
        "gl glDrawArrays 10 0 -1", "gl glDrawArrays 4 0 -1", "glp glDrawElements iii 10 -1 0x1406 f 0", "glp glDrawElements iii 4 -1 0x1406 f 0", "glp glDrawElements iii 4 -1 0x1401 ub 0",
        "gl glEnableClientState 0xb50", "gl glDisableClientState 0x8070",
        "gl glBegin 4\ngl glDrawArrays 4 0 3\ngl glEnd", "gl glBegin 4\nglp glDrawElements iii 4 1 0x1401 ub 0\ngl glEnd", "gl glBegin 4\nglp glVertexPointer iii 2 0x1406 0 f 0 0\ngl glEnd", "gl glBegin 4\ngl glEnableClientState 0x8074\ngl glEnd", "gl glBegin 4\ngl glPushClientAttrib 2\ngl glEnd",
        "gl glPopClientAttrib",
    ] {
        writeln!(s, "{line}\nglerr");
    }
    writeln!(s, "glget i 0x807a 1\nglget i 0x807b 1\nglget i 0x807c 1\nglisen 0x8074");
    for _ in 0..17 {
        writeln!(s, "gl glPushClientAttrib 3");
    }
    writeln!(s, "glerr\nglget i 0xbb1 1\nglget i 0xd3b 1");
    end(s);
    // the client stack: pixel stores and arrays apart, an unknown bit
    head(s, "client-stack");
    writeln!(s, "gl glPixelStorei 0xcf2 7\nglp glVertexPointer iii 3 0x1402 6 s 0 0 0\ngl glEnableClientState 0x8074\ngl glPushClientAttrib 1\ngl glPixelStorei 0xcf2 3\ngl glDisableClientState 0x8074\nglp glVertexPointer iii 2 0x1406 0 f 0 0\ngl glPopClientAttrib\nglget i 0xcf2 1\nglisen 0x8074\nglget i 0x807a 1\ngl glPushClientAttrib 2\ngl glPixelStorei 0xcf2 5\nglp glVertexPointer iii 4 0x1404 0 i 0 0\ngl glPopClientAttrib\nglget i 0xcf2 1\nglget i 0x807a 1\nglget i 0x807b 1\ngl glPushClientAttrib 0xffffffff\ngl glPixelStorei 0xcf2 9\ngl glPopClientAttrib\nglget i 0xcf2 1\ngl glPushAttrib 0xfffff\ngl glEnableClientState 0x8076\ngl glEdgeFlag 0\ngl glPopAttrib\nglisen 0x8076\nglget i 0xb43 1");
    end(s);
    // pointers: each array's slot, a null one, the slots reused after 16 glp lines
    head(s, "pointers");
    writeln!(s, "glp glVertexPointer iii 2 0x1406 0 f 0 0\nglp glNormalPointer ii 0x1406 0 f 0 0 1\nglp glColorPointer iii 4 0x1401 0 ub 1 2 3 4\nglp glIndexPointer ii 0x1406 0 f 1\nglp glTexCoordPointer iii 2 0x1406 0 f 0 0\nglp glEdgeFlagPointer i 0 ub 1\nglp glColorPointer iii 4 0x1401 0 null");
    for k in 0x808eu32..=0x8093 {
        writeln!(s, "glq glGetPointerv i {k:#x} p 1");
    }
    writeln!(s, "glq glGetPointerv i 0x8094 p 1\nglerr");
    for _ in 0..12 {
        writeln!(s, "glp glTexCoordPointer iii 2 0x1406 0 f 0 0");
    }
    writeln!(s, "glq glGetPointerv i 0x8092 p 1\nglq glGetPointerv i 0x808e p 1");
    end(s);
    // an array read past its slot into the next; a list keeping the elements it was compiled with
    head(s, "past-slot");
    // (a stride of a whole slot: element 1 at the next slot's start, element 2 the one after)
    writeln!(s, "glp glVertexPointer iii 2 0x1406 4096 f 2 2\nglp glColorPointer iii 4 0x1401 0 f 13 4\nglp glColorPointer iii 4 0x1401 0 f 6 13\ngl glEnableClientState 0x8074\ngl glDrawArrays 4 0 3");
    end(s);
    head(s, "list");
    writeln!(s, "glp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13\ngl glEnableClientState 0x8074\nglp glColorPointer iii 4 0x1401 0 ub 255 0 0 255 0 255 0 255 0 0 255 255\ngl glEnableClientState 0x8076\ngl glNewList 1 0x1300\ngl glDrawArrays 4 0 3\nglp glDrawElements iii 0 1 0x1401 ub 2\ngl glBegin 0\ngl glArrayElement 1\ngl glEnd\ngl glEndList\nglp glColorPointer iii 4 0x1401 0 ub 9 9 9 255 9 9 9 255 9 9 9 255\nglp glVertexPointer iii 2 0x1406 0 f 1 1 1 1 1 1\ngl glDisableClientState 0x8076\ngl glCallList 1");
    end(s);
    // every interleaved layout, packed and with a stride
    let floats: String = (0..64).map(|k| format!(" {}", [0.25f32, 0.5, 0.75, 1.0, 2.0, 2.0, 0.0, 1.0, 13.0, 4.0, 0.5, 1.0, 6.0, 13.0, -0.5, 1.0].get(k % 16).copied().unwrap_or(0.0))).collect();
    for (k, format) in (0x2a20u32..=0x2a2d).enumerate() {
        for stride in [0, 64] {
            head(s, &format!("interleaved-{k}-{stride}"));
            writeln!(s, "gltex 3553 0 4 4 4 0 0x1908 0x1401 27 64\ngl glTexParameteri 3553 10241 9728\ngl glEnable 3553\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glEnable 0xb57\ngl glEnableClientState 0x8077\ngl glEnableClientState 0x8079\nglp glInterleavedArrays ii {format:#x} {stride} f{floats}\ngl glPointSize 3\ngl glDrawArrays 0 0 3");
            for c in 0x8074u32..=0x8079 {
                writeln!(s, "glisen {c:#x}");
            }
            writeln!(s, "glget i 0x807a 1\nglget i 0x807c 1\nglget i 0x8081 1\nglget i 0x8082 1\nglget i 0x8083 1\nglget i 0x8088 1\nglget i 0x808a 1\nglget i 0x807f 1");
            end(s);
        }
    }
    // edge flags: from an array, in GL_POINT mode, stippled across a skipped edge, a strip's
    for (name, setup) in [("edge-array", "gl glPolygonMode 0x408 0x1b01"), ("edge-points", "gl glPolygonMode 0x408 0x1b00\ngl glPointSize 2"), ("edge-stipple", "gl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb24\ngl glLineStipple 1 0x3333"), ("edge-back", "gl glPolygonMode 0x404 0x1b01\ngl glPolygonMode 0x405 0x1b00")] {
        head(s, name);
        writeln!(s, "{setup}\nglp glVertexPointer iii 2 0x1406 0 f 2 2 13 3 14 13 3 12 8 8\ngl glEnableClientState 0x8074\nglp glEdgeFlagPointer i 0 ub 1 0 1 1 0\ngl glEnableClientState 0x8079\ngl glDrawArrays 9 0 4\ngl glDrawArrays 4 2 3\nglget i 0xb43 1");
        end(s);
    }
    head(s, "edge-immediate");
    writeln!(s, "gl glPolygonMode 0x408 0x1b01\ngl glBegin 7\ngl glEdgeFlag 0\ngl glVertex2f 2 2\ngl glVertex2f 13 3\ngl glEdgeFlag 1\ngl glVertex2f 14 13\ngl glEdgeFlag 0\ngl glVertex2f 3 12\ngl glEnd\nglp glEdgeFlagv - ub 1\nglget i 0xb43 1\ngl glBegin 5\ngl glEdgeFlag 0\ngl glVertex2f 2 14\ngl glVertex2f 8 15\ngl glVertex2f 5 10\ngl glEnd");
    end(s);
    writeln!(s, "case gap-edge-0\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3\ngl glBegin 4\ngl glEdgeFlag 0\ngl glVertex2f -4 2\ngl glEdgeFlag 1\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
}

/// the bytes of an element of a GL type
fn type_bytes(ty: u32) -> u32 {
    match ty {
        0x1400 | 0x1401 => 1,
        0x1402 | 0x1403 => 2,
        0x140a => 8,
        _ => 4,
    }
}

/// what textured fragments do not settle yet (GL.RASTER.MODE), after every other case
/// (`last`: cases that go after every other, as cases are added at the end)
fn textures_more(s: &mut String, last: &mut String) {
    const T2: u32 = 0xde1;
    let head = |s: &mut String, name: &str, bpp: u32| {
        writeln!(s, "case {name}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
    };
    let q = |s: &mut String, kind: &str, args: &str, ty: &str, n: u32| {
        writeln!(s, "gltexq {kind} {args} {ty} {n}");
    };
    let quad = |s: &mut String, s0: f32, t0: f32, s1: f32, t1: f32| {
        writeln!(s, "gl glColor4ub 255 255 255 255\ngl glBegin 7\ngl glTexCoord2f {s0} {t0}\ngl glVertex2f 0 0\ngl glTexCoord2f {s1} {t0}\ngl glVertex2f 16 0\ngl glTexCoord2f {s1} {t1}\ngl glVertex2f 16 16\ngl glTexCoord2f {s0} {t1}\ngl glVertex2f 0 16\ngl glEnd");
    };
    let levels = [0x1000u32, 0x1001, 0x1003, 0x1005, 0x805c, 0x805d, 0x805e, 0x805f, 0x8060, 0x8061];
    // fast spans dithered, blended,
    // into alpha bits, depth tested on 16 and 32 bits and under GL_NICEST; a smooth polygon
    // fogged; outlines, wide and deep lines, smooth lines blended; an eye plane under another
    // modelview; a proxy image; a copy from outside the window; color index mode
    let fast = |s: &mut String, name: &str, glctx: &str, setup: &str| {
        writeln!(s, "case tex-open-more-{name}\n{glctx}{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 18 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexParameteri {T2} 10240 9728\ngl glEnable {T2}\ngl glClear 0x4100\n{setup}");
        writeln!(s, "gl glBegin 4\ngl glColor4ub 255 0 0 200\ngl glTexCoord2f 0 0\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 100\ngl glTexCoord2f 1 0\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glTexCoord2f 0.5 1\ngl glVertex3f 6 13 0\ngl glEnd");
        read_all(s);
        writeln!(s, "glread 0 0 16 16 0x1902 0x1405 1024\nglerr\nend");
    };
    let c32 = ctx(32);
    fast(s, "0", &c32, "gl glEnable 0xbd0\ngl glTexEnvi 8960 8704 7681");
    fast(s, "1", &c32, "gl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000");
    fast(s, "2", "glctx 32 choose 0x38 0 32 8 0 32 8\n", "gl glClearColor 0.25 0.5 0.75 0.5\ngl glClear 0x4000");
    fast(s, "3", "glctx 32 choose 0x38 0 32 0 0 16 8\n", "gl glEnable 0xb71");
    fast(s, "4", &c32, "gl glEnable 0xb71");
    fast(s, "5", &c32, "gl glEnable 0xb71\ngl glHint 0xc50 0x1102");
    fast(s, "6", &c32, "gl glEnable 0xb71\ngl glDepthFunc 0x207");
    fast(s, "7", &c32, "gl glEnable 0xb41\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1");
    fast(s, "8", &c32, "gl glPolygonMode 0x408 0x1b01");
    fast(s, "9", &c32, "gl glPolygonMode 0x408 0x1b00\ngl glPointSize 3");
    let line = |s: &mut String, name: &str, setup: &str| {
        writeln!(s, "case tex-open-more-{name}\n{}{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 19 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable {T2}\ngl glClear 0x4100\ngl glColor4ub 200 150 100 255\n{setup}", ctx(32));
        writeln!(s, "gl glBegin 1\ngl glTexCoord2f 0 0\ngl glVertex3f 1.5 2.25 0.5\ngl glTexCoord2f 1 1\ngl glVertex3f 14.5 11.75 -0.25\ngl glEnd");
        read_all(s);
        writeln!(s, "glerr\nend");
    };
    line(s, "line-wide", "gl glLineWidth 3");
    line(s, "line-deep", "gl glEnable 0xb71");
    line(s, "line-smooth-blend", "gl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303");
    line(s, "line-smooth-wide", "gl glEnable 0xb20\ngl glLineWidth 2.5");
    line(s, "line-smooth-fog", "gl glEnable 0xb20\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1");
    line(s, "line-smooth-depth", "gl glEnable 0xb20\ngl glEnable 0xb71");
    line(s, "line-smooth-flat", "gl glEnable 0xb20\ngl glShadeModel 0x1d00");
    line(s, "line-flat", "gl glShadeModel 0x1d00");
    // an eye plane given under a translated, scaled modelview
    head(s, "tex-open-more-eye", 32);
    writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 20 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glTranslatef 2 1 0\ngl glScalef 0.5 2 1\ngl glTexGeni 8192 9472 9216\ngl glTexGeni 8193 9472 9216\ngltexv gen 8192 9474 f 0.125 0.03125 0 0\ngltexv gen 8193 9474 f 0 0.125 0 0.25\ngl glLoadIdentity\ngl glEnable 0xc60\ngl glEnable 0xc61\ngl glClear 0x4000");
    quad(s, 0.0, 0.0, 1.0, 1.0);
    read_all(s);
    q(s, "gen", "8192 9474", "f", 4);
    q(s, "gen", "8193 9474", "f", 4);
    writeln!(s, "glerr\nend");
    // proxy images: their level parameters, too big and fitting
    head(s, "tex-open-more-proxy", 32);
    writeln!(s, "gltex 0x8064 0 4 8 8 0 0x1908 0x1401 21 256\nglerr");
    for l in levels {
        q(s, "level", &format!("0x8064 0 {l}"), "i", 1);
    }
    writeln!(s, "gltex 0x8064 0 4 2048 8 0 0x1908 0x1401 21 256\nglerr");
    q(s, "level", "0x8064 0 4096", "i", 1);
    writeln!(s, "gltex 0x8063 0 4 8 1 0 0x1908 0x1401 21 32\nglerr");
    q(s, "level", "0x8063 0 4096", "i", 1);
    q(s, "level", &format!("{T2} 0 4096"), "i", 1);
    writeln!(s, "glerr\nend");
    // copies reaching past the window
    head(s, "tex-open-more-copy", 32);
    writeln!(s, "gldraw 16 16 0x1908 0x1401 22 1024\ngl glCopyTexImage2D {T2} 0 0x8058 12 -2 8 8 0\nglerr\nglteximg {T2} 0 0x1908 0x1401 256\ngl glCopyTexSubImage2D {T2} 0 0 0 -3 14 4 4\nglerr\nglteximg {T2} 0 0x1908 0x1401 256\nglerr\nend");
    // color index mode: a texture enabled over an index polygon, line and point
    writeln!(s, "case tex-open-more-ci\nglctx 8 29\n{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 23 64\ngl glTexParameteri {T2} 10241 9728\ngl glEnable {T2}\ngl glClearIndex 3\ngl glClear 0x4000\ngl glIndexi 9\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex2f 2 2\ngl glTexCoord2f 1 0\ngl glVertex2f 13 4\ngl glTexCoord2f 0.5 1\ngl glVertex2f 6 13\ngl glEnd\nglread 0 0 16 16 0x1900 0x1405 1024\nglerr\nend");
    // (after them, so the earlier cases keep their places) the same in a color index format
    // asked for, textured antialiased lines' depths, a textured flat line's color end, an eye
    // plane under a rotation
    writeln!(s, "case tex-open-more-ci-choose\nglctx 8 choose 0x38 1 8 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 23 64\ngl glTexParameteri {T2} 10241 9728\ngl glEnable {T2}\ngl glClearIndex 3\ngl glClear 0x4000\ngl glIndexi 9\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex2f 2 2\ngl glTexCoord2f 1 0\ngl glVertex2f 13 4\ngl glTexCoord2f 0.5 1\ngl glVertex2f 6 13\ngl glEnd\ngl glBegin 1\ngl glTexCoord2f 0 0\ngl glVertex2f 1.5 14.5\ngl glTexCoord2f 1 1\ngl glVertex2f 14.5 9.5\ngl glEnd\nglread 0 0 16 16 0x1900 0x1405 1024\nglerr\nend");
    for (name, setup) in [("line-smooth-depth-read", "gl glEnable 0xb20\ngl glEnable 0xb71"), ("line-smooth-wide-depth-read", "gl glEnable 0xb20\ngl glEnable 0xb71\ngl glLineWidth 2.5")] {
        writeln!(s, "case tex-open-more-{name}\n{}{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 19 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable {T2}\ngl glClear 0x4100\ngl glColor4ub 200 150 100 255\n{setup}", ctx(32));
        writeln!(s, "gl glBegin 1\ngl glTexCoord2f 0 0\ngl glVertex3f 1.5 2.25 0.5\ngl glTexCoord2f 1 1\ngl glVertex3f 14.5 11.75 -0.25\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    for (name, setup) in [("line-flat-ends", "gl glShadeModel 0x1d00"), ("line-smooth-flat-ends", "gl glShadeModel 0x1d00\ngl glEnable 0xb20"), ("line-wide-flat-ends", "gl glShadeModel 0x1d00\ngl glLineWidth 3")] {
        writeln!(s, "case tex-open-more-{name}\n{}{ORTHO}gl glDisable 0xbd0\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 19 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable {T2}\ngl glClear 0x4100\n{setup}", ctx(32));
        writeln!(s, "gl glBegin 1\ngl glColor4ub 200 150 100 255\ngl glTexCoord2f 0 0\ngl glVertex2f 1.5 2.25\ngl glColor4ub 90 220 40 255\ngl glTexCoord2f 1 1\ngl glVertex2f 14.5 11.75\ngl glEnd");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    head(s, "tex-open-more-eye-rot", 32);
    writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 20 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glTranslatef 3 -1 0.5\ngl glRotatef 30 0.2 0.3 1\ngl glScalef 0.7 1.3 0.9\ngl glTexGeni 8192 9472 9216\ngl glTexGeni 8193 9472 9216\ngltexv gen 8192 9474 f 0.1 0.03 0.2 0.05\ngltexv gen 8193 9474 f -0.02 0.11 0.3 0.25\ngl glLoadIdentity\ngl glEnable 0xc60\ngl glEnable 0xc61\ngl glClear 0x4000");
    quad(s, 0.0, 0.0, 1.0, 1.0);
    read_all(s);
    q(s, "gen", "8192 9474", "f", 4);
    q(s, "gen", "8193 9474", "f", 4);
    writeln!(s, "glerr\nend");
    // alpha bits cleared to values between steps (0.5 gave 127 in tex-open-more-2)
    for (k, a) in [0.25f32, 0.3, 0.5, 0.75, 0.9, 0.002, 0.998].into_iter().enumerate() {
        writeln!(s, "case clear-alpha-more-{k}\nglctx 32 choose 0x38 0 32 8 0 32 8\ngl glClearColor {a} {a} {a} {a}\ngl glClear 0x4000\nglread 0 0 2 1 0x1908 0x1401 8\nglerr\nend");
    }
    // what GL.RASTER.MODE still holds: textured outlines cut or offset, offset outlines and
    // points of other shapes, antialiased lines blended otherwise into 16 bits, a smooth
    // polygon reaching behind the eye, an eye plane under a singular modelview, a paletted
    // image, images and bitmaps under a texture
    let tex = format!("gltex {T2} 0 4 4 4 0 0x1908 0x1401 19 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable {T2}\n");
    let tri3 = |s: &mut String, v: [[f32; 3]; 3]| {
        writeln!(s, "gl glColor4ub 200 150 100 255\ngl glBegin 4");
        for (k, [x, y, z]) in v.into_iter().enumerate() {
            let (ts, tt) = [(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)].get(k).copied().unwrap_or((0.0, 0.0));
            writeln!(s, "gl glTexCoord2f {ts} {tt}\ngl glVertex3f {x} {y} {z}");
        }
        writeln!(s, "gl glEnd");
    };
    let quad4 = |s: &mut String| {
        writeln!(s, "gl glColor4ub 90 220 40 255\ngl glBegin 7\ngl glVertex3f 3 3 0.5\ngl glVertex3f 12 4 -0.25\ngl glVertex3f 11 12 0\ngl glVertex3f 4 11 0.25\ngl glEnd");
    };
    let depth_read = |s: &mut String| {
        writeln!(s, "glread 0 0 16 16 0x1902 0x1405 1024");
    };
    let cut = [[-6.0, 3.0, 0.0], [13.0, 4.0, 0.0], [6.0, 13.0, 0.0]];
    let deep = [[2.0, 2.0, 0.5], [13.0, 4.0, -0.25], [6.0, 13.0, 0.0]];
    let offset = "gl glEnable 0xb71\ngl glClear 0x100\ngl glPolygonOffset 1 1";
    head(s, "tex-open-more-outline-cut", 32);
    writeln!(s, "{tex}gl glClear 0x4000\ngl glPolygonMode 0x408 0x1b01");
    tri3(s, cut);
    read_all(s);
    writeln!(s, "glerr\nend");
    head(s, "tex-open-more-outline-offset", 32);
    writeln!(s, "{tex}gl glClear 0x4000\n{offset}\ngl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01");
    tri3(s, deep);
    read_all(s);
    depth_read(s);
    writeln!(s, "glerr\nend");
    for (name, setup) in [("offset-point-quad", "gl glEnable 0x2a01\ngl glPolygonMode 0x408 0x1b00\ngl glPointSize 2"), ("offset-wide-quad", "gl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3"), ("offset-smooth-quad", "gl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb20")] {
        head(s, &format!("raster-more-{name}"), 32);
        writeln!(s, "gl glClear 0x4000\n{offset}\n{setup}");
        quad4(s);
        read_all(s);
        depth_read(s);
        writeln!(s, "glerr\nend");
    }
    for (name, setup) in [("offset-line-cut", "gl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01"), ("offset-wide-cut", "gl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3")] {
        head(s, &format!("raster-more-{name}"), 32);
        writeln!(s, "gl glClear 0x4000\n{offset}\n{setup}");
        tri3(s, [[-6.0, 3.0, 0.5], [13.0, 4.0, -0.25], [6.0, 13.0, 0.0]]);
        read_all(s);
        depth_read(s);
        writeln!(s, "glerr\nend");
    }
    for (k, (sf, df)) in [(1u32, 1u32), (0x306, 0x307), (0x302, 1)].into_iter().enumerate() {
        head(s, &format!("raster-more-aa-line-16-{k}"), 16);
        writeln!(s, "gl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc {sf:#x} {df:#x}\ngl glColor4ub 200 150 100 180\ngl glBegin 1\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glVertex2f 2.5 13.5\ngl glVertex2f 12.25 1.5\ngl glEnd");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    writeln!(s, "case raster-more-smooth-behind\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glFrustum -1 1 -1 1 1 10\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41", ctx(32));
    writeln!(s, "gl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f -1 -1 -2\ngl glColor4ub 0 255 0 255\ngl glVertex3f 1 -0.5 -2\ngl glColor4ub 0 0 255 255\ngl glVertex3f 0 1 1\ngl glEnd");
    read_all(s);
    writeln!(s, "glerr\nend");
    head(s, "tex-open-more-eye-singular", 32);
    writeln!(s, "gl glScalef 0 1 1\ngltexv gen 8192 9474 f 0.125 0.25 0.5 1\nglerr\ngl glLoadIdentity");
    q(s, "gen", "8192 9474", "f", 4);
    writeln!(s, "glerr\nend");
    head(s, "tex-open-more-paletted", 32);
    writeln!(s, "gltex {T2} 0 0x80e5 4 4 0 0x1900 0x1401 24 16\nglerr");
    for l in levels {
        q(s, "level", &format!("{T2} 0 {l}"), "i", 1);
    }
    writeln!(s, "glteximg {T2} 0 0x1908 0x1401 64\ngl glEnable {T2}\ngl glClear 0x4000");
    quad(s, 0.0, 0.0, 1.0, 1.0);
    read_all(s);
    writeln!(s, "glerr\nend");
    head(s, "tex-open-more-copypixels", 32);
    writeln!(s, "gldraw 16 16 0x1908 0x1401 25 1024\n{tex}gl glTexCoord2f 0.3 0.6\ngl glRasterPos2i 4 3\ngl glCopyPixels 1 1 6 5 0x1800");
    read_all(s);
    writeln!(s, "glerr\nend");
    head(s, "tex-open-more-bitmap", 32);
    writeln!(s, "{tex}gl glClear 0x4000\ngl glColor4ub 200 150 100 255\ngl glTexCoord2f 0.3 0.6\ngl glRasterPos2i 4 3\nglp glBitmap iiffff 8 4 1 1 5 2 ub 129 255 66 60");
    read_all(s);
    writeln!(s, "glerr\nend");
    // what GL.RASTER.MODE still holds of textured outlines (the review of 84b05c8, 1: no case
    // reached it): a cut outline flat-shaded, fogged, with generated coordinates, in GL_POINT
    // mode, offset under a depth test; an uncut one in GL_POINT mode offset under a depth test
    for (name, setup, corners) in [
        ("outline-cut-flat", "gl glShadeModel 0x1d00\ngl glPolygonMode 0x408 0x1b01", cut),
        ("outline-cut-fog", "gl glEnable 0xb60\ngl glFogi 0xb65 0x2601\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glPolygonMode 0x408 0x1b01", cut),
        ("outline-cut-gen", "gl glTexGeni 0x2000 0x2500 0x2401\ngl glEnable 0xc60\ngl glPolygonMode 0x408 0x1b01", cut),
        ("outline-cut-point", "gl glPolygonMode 0x408 0x1b00", cut),
        ("outline-cut-offset-depth", "gl glEnable 0xb71\ngl glClear 0x100\ngl glPolygonOffset 1 1\ngl glEnable 0x2a02\ngl glPolygonMode 0x408 0x1b01", [[-6.0, 3.0, 0.5], [13.0, 4.0, -0.25], [6.0, 13.0, 0.0]]),
        ("point-offset-depth", "gl glEnable 0xb71\ngl glClear 0x100\ngl glEnable 0x2a01\ngl glPolygonOffset 1 1\ngl glPolygonMode 0x408 0x1b00\ngl glPointSize 2", deep),
    ] {
        head(last, &format!("tex-open-more-{name}"), 32);
        writeln!(last, "{tex}gl glClear 0x4000\n{setup}");
        tri3(last, corners);
        read_all(last);
        writeln!(last, "glerr\nend");
    }
}

/// a vertex coordinate in 1/8 steps (three subpixel bits) in lo..hi
fn v8(q: &mut XorShift32, lo: i64, hi: i64) -> f64 {
    q.range(lo * 8, hi * 8) as f64 / 8.0
}

fn color(s: &mut String, q: &mut XorShift32) {
    let c: Vec<u32> = (0..3).map(|_| q.next() & 255).collect();
    gl(s, "glColor4ub", &[c[0], c[1], c[2], 255]);
}

/// the measurements of the rasterizer, shading, clipping, dithering, color index, accumulation,
/// attribute stack and 16-bit depth: one primitive, then the whole picture read back
fn measure(s: &mut String) {
    let mut q = XorShift32::new(0x0960_2001);
    let start = |s: &mut String, name: String, bpp: u32| {
        writeln!(s, "case {name}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
    };
    // horizontal edges through pixel centers: every row, as bottom and as top, each vertex order
    for k in 0..16 {
        start(s, format!("tie-y-{k}"), 32);
        let y = k as f64 + 0.5;
        for (far, order) in [(2.25, 0), (-1.75, 0), (3.0, 1), (-3.0, 1), (5.25, 2), (-0.25, 2), (1.5, 3), (-2.5, 3)] {
            let (x0, x1) = (v8(&mut q, 0, 6), v8(&mut q, 9, 16));
            let (ya, yb) = (y, y + far);
            let r = match order {
                0 => [x0, ya, x1, yb],
                1 => [x1, yb, x0, ya],
                2 => [x0, yb, x1, ya],
                _ => [x1, ya, x0, yb],
            };
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            glf(s, "glRectf", &r);
            read_all(s);
        }
        // the same edges from quads of both windings
        for cw in [false, true] {
            let (x0, x1) = (v8(&mut q, 0, 6), v8(&mut q, 9, 16));
            let mut v = vec![(x0, y), (x1, y), (x1, y + 2.5), (x0, y + 2.5)];
            if cw {
                v.reverse();
            }
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            writeln!(s, "gl glBegin 7");
            for (x, y) in v {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // vertical edges through pixel centers, against the horizontal ones
    for k in 0..4 {
        start(s, format!("tie-x-{k}"), 32);
        for _ in 0..8 {
            let x0 = q.range(-2, 14) as f64 + 0.5;
            let x1 = x0 + q.range(1, 6) as f64 + *q.pick(&[0.0, 0.25, 0.5]);
            let (y0, y1) = (v8(&mut q, -2, 8), v8(&mut q, 9, 18));
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            glf(s, "glRectf", &[x0, y0, x1, y1]);
            read_all(s);
        }
        writeln!(s, "end");
    }
    // flat triangles at subpixel positions, both windings
    for k in 0..16 {
        start(s, format!("tri-{k}"), 32);
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            writeln!(s, "gl glBegin 4");
            for _ in 0..3 {
                let (x, y) = if k < 4 { (q.range(-2, 18) as f64 + 0.5 * q.range(0, 1) as f64, q.range(-2, 18) as f64 + 0.5 * q.range(0, 1) as f64) } else { (v8(&mut q, -3, 19), v8(&mut q, -3, 19)) };
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // the other primitives: points of several sizes, lines, strips, loops, fans, quad strips, polygons
    for (k, (mode, n, extra)) in [(0u32, 12, "gl glPointSize 2\n"), (0, 12, "gl glPointSize 3\n"), (0, 12, "gl glPointSize 1.5\n"), (1, 8, ""), (1, 8, ""), (1, 8, "gl glLineWidth 2\n"), (1, 8, "gl glLineWidth 3\n"), (2, 6, ""), (3, 6, ""), (5, 6, ""), (6, 6, ""), (8, 8, ""), (9, 6, ""), (9, 5, ""), (1, 8, "gl glLineWidth 1.5\n"), (3, 8, "")]
        .iter()
        .enumerate()
    {
        start(s, format!("prim-{k}"), 32);
        s.push_str(extra);
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            writeln!(s, "gl glBegin {mode}");
            let c = (v8(&mut q, 4, 12), v8(&mut q, 4, 12));
            for i in 0..*n {
                let (x, y) = if *mode == 9 {
                    // a convex polygon around a center
                    let a = i as f64 * std::f64::consts::TAU / *n as f64;
                    (((c.0 + 6.0 * a.cos()) * 8.0).round() / 8.0, ((c.1 + 6.0 * a.sin()) * 8.0).round() / 8.0)
                } else {
                    (v8(&mut q, -2, 18), v8(&mut q, -2, 18))
                };
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // polygon modes and culling
    for (k, setup) in ["gl glPolygonMode 0x408 0x1b01", "gl glPolygonMode 0x408 0x1b00", "gl glEnable 0xb44", "gl glEnable 0xb44\ngl glCullFace 0x404", "gl glEnable 0xb44\ngl glFrontFace 0x900\ngl glCullFace 0x408", "gl glPolygonMode 0x404 0x1b01\ngl glPolygonMode 0x405 0x1b00"].iter().enumerate() {
        start(s, format!("polymode-{k}"), 32);
        writeln!(s, "{setup}");
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000");
            color(s, &mut q);
            writeln!(s, "gl glBegin 4");
            for _ in 0..3 {
                glf(s, "glVertex2f", &[v8(&mut q, -1, 17), v8(&mut q, -1, 17)]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // smooth shading and depth across triangles, lines and quads; flat shading takes the last vertex
    for k in 0..12 {
        let bpp = [32, 32, 32, 32, 32, 32, 24, 24, 16, 16, 32, 32][k];
        start(s, format!("smooth-{k}"), bpp);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glClear 0x4100");
        if k >= 10 {
            writeln!(s, "gl glShadeModel 0x1d00");
        }
        let mode = [4u32, 4, 4, 4, 7, 1, 4, 7, 4, 7, 4, 7][k];
        for _ in 0..3 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin {mode}");
            for _ in 0..[3, 3, 3, 3, 4, 2, 3, 4, 3, 4, 3, 4][k] {
                color(s, &mut q);
                glf(s, "glVertex3f", &[v8(&mut q, -2, 18), v8(&mut q, -2, 18), q.range(-32, 32) as f64 / 32.0]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "end");
    }
    // perspective: frustums, triangles in depth, colors and depth read back
    for k in 0..6 {
        writeln!(s, "case persp-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32));
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, [3.0, 10.0, 2.0, 100.0, 4.0, 8.0][k]]);
        writeln!(s, "gl glMatrixMode 0x1700");
        if k >= 4 {
            glf(s, "glRotatef", &[30.0, 0.0, 1.0, 0.0]);
        }
        for _ in 0..3 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k % 2 == 0 { 4 } else { 7 });
            for _ in 0..if k % 2 == 0 { 3 } else { 4 } {
                color(s, &mut q);
                glf(s, "glVertex3f", &[q.range(-32, 32) as f64 / 16.0, q.range(-32, 32) as f64 / 16.0, -(q.range(17, 40) as f64) / 16.0]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "end");
    }
    // clipping against the six planes: triangles past the window and through near and far
    for k in 0..6 {
        start(s, format!("clip-{k}"), 32);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x207");
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
            for _ in 0..3 {
                color(s, &mut q);
                glf(s, "glVertex3f", &[v8(&mut q, -24, 40), v8(&mut q, -24, 40), q.range(-64, 64) as f64 / 32.0]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "end");
    }
    // dithering: whole-window rectangles of one color on 16, 8 and 4-bit DIBs
    for (k, (bpp, pf)) in [(16u32, 0u32), (16, 0), (16, 0), (16, 0), (8, 25), (8, 25), (4, 31), (4, 31)].iter().enumerate() {
        if *pf == 0 {
            writeln!(s, "case dither-fill-{k}\n{}{ORTHO}gl glEnable 0xbd0", ctx(*bpp));
        } else {
            writeln!(s, "case dither-fill-{k}\nglctx {bpp} {pf}\n{ORTHO}gl glEnable 0xbd0");
        }
        for _ in 0..16 {
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib");
        }
        writeln!(s, "gl glDisable 0xbd0");
        for _ in 0..4 {
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib");
        }
        for _ in 0..4 {
            glf(s, "glClearColor", &[(q.next() & 255) as f64 / 256.0, (q.next() & 255) as f64 / 256.0, (q.next() & 255) as f64 / 256.0, 1.0]);
            writeln!(s, "gl glEnable 0xbd0\ngl glClear 0x4000\ngldib\ngl glDisable 0xbd0\ngl glClear 0x4000\ngldib");
        }
        writeln!(s, "end");
    }
    // color index: 8 and 4-bit formats, indices cleared, drawn, masked, read back, and the DIB
    for (k, (bpp, pf)) in [(8u32, 29u32), (8, 30), (4, 35), (8, 29)].iter().enumerate() {
        writeln!(s, "case ci-{k}\nglctx {bpp} {pf}\n{ORTHO}glget i 0xd51 1\nglget i 0xc20 1\nglget i 0xc21 1\nglget f 0xb00 4\nglget i 0xb01 1");
        for i in 0..6 {
            let v = q.range(0, 300);
            writeln!(s, "gl glClearIndex {v}\ngl glClear 0x4000\ngldib");
            writeln!(s, "gl glIndexi {}\ngl glRectf {} {} {} {}", q.range(0, 300), v8(&mut q, 0, 6), v8(&mut q, 0, 6), v8(&mut q, 9, 16), v8(&mut q, 9, 16));
            if i == 3 {
                writeln!(s, "gl glIndexMask 0xf0");
            }
            if i == 4 {
                writeln!(s, "gl glEnable 0xbf1\ngl glLogicOp 0x1506");
            }
            writeln!(s, "gldib\nglread 0 0 16 16 {} {} 256\nglread 0 0 16 16 {} {} 1024\nglerr", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "gldraw 16 2 {} {} 77 32\ngldib\nglerr\nend", h(COLOR_INDEX), h(UNSIGNED_BYTE));
    }
    // the accumulation buffer: load, accumulate, multiply, add, return
    for k in 0..6 {
        start(s, format!("accum-{k}"), [32, 32, 32, 16, 24, 32][k]);
        writeln!(s, "gl glClear 0x200");
        for _ in 0..4 {
            random_dst(s, q.next() & 0xffff);
            let op = *q.pick(&[0x100u32, 0x101]);
            writeln!(s, "gl glAccum {} {}", h(op), f(q.range(-8, 16) as f64 / 8.0));
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gl glAccum 0x100 {}\ngl glAccum {} {}", f(q.range(-8, 16) as f64 / 8.0), h(*q.pick(&[0x103u32, 0x104])), f(q.range(-8, 16) as f64 / 16.0));
            writeln!(s, "gl glAccum 0x102 {}", f(q.range(0, 32) as f64 / 8.0));
            read_all(s);
            writeln!(s, "glerr");
        }
        if k == 5 {
            writeln!(s, "gl glClearAccum 0.25 -0.5 1 0.75\ngl glClear 0x200\ngl glAccum 0x102 1");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // the attribute stack: state changed between push and pop, then read back
    for (k, mask) in [0x000fffffu32, 0x1, 0x8, 0x20, 0x100, 0x400, 0x1000, 0x4000, 0x2000, 0x800, 0x200, 0x8000].iter().enumerate() {
        start(s, format!("attrib-{k}"), 32);
        writeln!(s, "gl glPushAttrib {}", h(*mask));
        writeln!(s, "gl glColor4ub 1 2 3 4\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glDepthFunc 0x203\ngl glEnable 0xc11\ngl glScissor 1 2 3 4\ngl glViewport 1 1 8 8\ngl glClearColor 0.5 0.5 0.5 0.5\ngl glStencilFunc 0x202 1 3\ngl glLineWidth 3\ngl glPointSize 4\ngl glShadeModel 0x1d00\ngl glHint 0xc50 0x1101\ngl glLogicOp 0x1506\ngl glColorMask 1 0 1 0\ngl glDepthMask 0\ngl glMatrixMode 0x1701\ngl glPolygonMode 0x408 0x1b01\ngl glPixelTransferf 0xd14 0.5\ngl glAlphaFunc 0x204 0.5\ngl glEnable 0xbc0\ngl glDisable 0xbd0\ngl glEnable 0xb71\ngl glCullFace 0x404\ngl glPopAttrib\nglerr");
        for p in [0xb00u32, 0xbe2, 0xbe0, 0xbe1, 0xb74, 0xc11, 0xc10, 0xba2, 0xc22, 0xb92, 0xb21, 0xb11, 0xb54, 0xc50, 0xbf0, 0xc23, 0xb72, 0xba0, 0xb40, 0xd14, 0xbc1, 0xbc2, 0xbc0, 0xbd0, 0xb71, 0xb45, 0xbb0] {
            writeln!(s, "glget f {} 4", h(p));
        }
        writeln!(s, "glerr\nend");
    }
    // 16-bit depth buffers: tests at every function, clears, reads in every type
    for k in 0..4 {
        writeln!(s, "case depth16-{k}\nglctx 32 2\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71");
        glf(s, "glClearDepth", &[[1.0, 0.5, 0.25, 0.75][k]]);
        writeln!(s, "gl glClear 0x4100");
        for _ in 0..6 {
            writeln!(s, "gl glDepthFunc {}", h(*q.pick(&FUNCS[1..])));
            color(s, &mut q);
            let z = q.range(-64, 64) as f64 / 64.0;
            glf(s, "glRectf", &[v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17)]);
            let _ = z;
            writeln!(s, "gl glBegin 7");
            let (x0, y0, x1, y1) = (v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17));
            let z = q.range(-64, 64) as f64 / 64.0;
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                glf(s, "glVertex3f", &[x, y, z]);
            }
            writeln!(s, "gl glEnd");
        }
        read_all(s);
        for ty in [UNSIGNED_INT, UNSIGNED_SHORT, UNSIGNED_BYTE, FLOAT, INT] {
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(ty));
        }
        writeln!(s, "end");
    }
    // GL_NEVER: single rectangles on a clear picture
    for k in 0..4 {
        start(s, format!("never-{k}"), 32);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x200");
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100");
            color(s, &mut q);
            glf(s, "glRectf", &[v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17)]);
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "end");
    }
    // blending on 16 and 24-bit DIBs and with destination alpha
    for (k, (bpp, choose_alpha)) in [(16u32, 0u32), (24, 0), (32, 8), (16, 0), (24, 0), (32, 8)].iter().enumerate() {
        writeln!(s, "case blend-fmt-{k}\nglctx {bpp} choose 0x38 0 {bpp} {choose_alpha} 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbe2");
        for _ in 0..12 {
            let (sf, df) = (*q.pick(BLEND_SRC), *q.pick(BLEND_DST));
            gl(s, "glBlendFunc", &[sf, df]);
            writeln!(s, "gl glDisable 0xbe2");
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gl glEnable 0xbe2");
            let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
            gl(s, "glColor4ub", &c);
            writeln!(s, "gl glRectf 0 0 16 16");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // points on pixel boundaries
    for k in 0..2 {
        start(s, format!("point-edge-{k}"), 32);
        for _ in 0..8 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 0");
            for _ in 0..4 {
                color(s, &mut q);
                let (x, y) = (q.range(0, 32) as f64 / 2.0, q.range(0, 32) as f64 / 2.0);
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "end");
    }
    // each accumulation operation alone, between a load and a return, on every depth and with
    // alpha; then clears to values returned
    for (fmt, head) in [("32", ctx(32)), ("24", ctx(24)), ("16", ctx(16)), ("8", ctx(8)), ("4", ctx(4)), ("alpha", "glctx 32 choose 0x38 0 32 8 0 32 8\n".to_string())].iter() {
        for (ok, op) in ["load", "accum", "mult", "add"].iter().enumerate() {
            writeln!(s, "case accum-{op}-{fmt}\n{head}{ORTHO}gl glDisable 0xbd0");
            for _ in 0..6 {
                random_dst(s, q.next() & 0xffff);
                writeln!(s, "gl glAccum 0x101 {}", f(q.range(-8, 17) as f64 / 8.0));
                match ok {
                    1 => {
                        random_dst(s, q.next() & 0xffff);
                        writeln!(s, "gl glAccum 0x100 {}", f(q.range(-16, 17) as f64 / 8.0));
                    }
                    2 => writeln!(s, "gl glAccum 0x103 {}", f(q.range(-24, 25) as f64 / 16.0)),
                    3 => writeln!(s, "gl glAccum 0x104 {}", f(q.range(-16, 17) as f64 / 16.0 + q.range(0, 8) as f64 / 1024.0)),
                    _ => {}
                }
                writeln!(s, "gl glAccum 0x102 {}", f(q.range(0, 33) as f64 / 8.0));
                read_all(s);
            }
            writeln!(s, "glerr\nend");
        }
        writeln!(s, "case accum-clear-{fmt}\n{head}{ORTHO}gl glDisable 0xbd0");
        for _ in 0..6 {
            let c: Vec<String> = (0..4).map(|_| f(q.range(-4, 1025) as f64 / 1024.0)).collect();
            writeln!(s, "gl glClearAccum {}\ngl glClear 0x200\ngl glAccum 0x102 {}", c.join(" "), f(q.range(4, 17) as f64 / 8.0));
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // the tie rule under other viewports and object coordinates: a column (row) one pixel wide
    // whose edges both pass through pixel centers, at every column (row), read across
    for (k, (vp, o)) in [([0, 0, 16, 16], [0.0, 16.0]), ([0, 0, 16, 16], [-8.0, 8.0]), ([0, 0, 16, 16], [0.0, 32.0]), ([4, 4, 16, 16], [0.0, 16.0]), ([-4, -4, 16, 16], [0.0, 16.0]), ([0, 0, 32, 32], [0.0, 32.0]), ([0, 0, 8, 8], [0.0, 8.0]), ([-8, -8, 32, 32], [-1.0, 1.0])].iter().enumerate() {
        writeln!(s, "case tie-vp-{k}\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {} {} {} {} -1 1\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\ngl glViewport {} {} {} {}", ctx(32), f(o[0]), f(o[1]), f(o[0]), f(o[1]), vp[0], vp[1], vp[2], vp[3]);
        // window coordinate to object coordinate on axis `a`
        let obj = |w: f64, a: usize| o[0] + (w - vp[a] as f64) / vp[a + 2] as f64 * (o[1] - o[0]);
        for c in 0..16 {
            let (w0, w1) = (c as f64 + 0.5, c as f64 + 1.5);
            writeln!(s, "gl glClear 0x4000");
            glf(s, "glRectf", &[obj(w0, 0), obj(-1.0, 1), obj(w1, 0), obj(17.0, 1)]);
            writeln!(s, "glread 0 8 16 1 {} {} 64", h(RGBA), h(UNSIGNED_BYTE));
            writeln!(s, "gl glClear 0x4000");
            glf(s, "glRectf", &[obj(-1.0, 0), obj(w0, 1), obj(17.0, 0), obj(w1, 1)]);
            writeln!(s, "glread 8 0 1 16 {} {} 64", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // glGetString between glBegin and glEnd; glAccum without an accumulation buffer (color index)
    writeln!(s, "case begin-string\n{}gl glBegin 7\nglstr 0x1f00\nglstr 0x1f03\ngl glEnd\nglerr\nend", ctx(32));
    writeln!(s, "case accum-none\nglctx 32 9\ngl glAccum 0x101 1\nglerr\ngl glAccum 0x102 1\nglerr\ngl glClear 0x200\nglerr\nend");
    // depth reads in every type, 32 and 16 depth bits: clears and rectangles at several depths
    for (k, pf) in [(0, "glctx 32 choose 0x38 0 32 0 0 32 8"), (1, "glctx 32 2")] {
        writeln!(s, "case depth-types-{k}\n{pf}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207");
        for (j, cz) in [1.0, 0.0, 0.5, 0.25, 0.123456, 0.999].iter().enumerate() {
            glf(s, "glClearDepth", &[*cz]);
            writeln!(s, "gl glClear 0x100");
            for i in 0..4 {
                let z = [-1.0, 1.0, 0.0, 0.3, -0.7, 0.9999, -0.0001, 0.5][(i + j) % 8];
                writeln!(s, "gl glBegin 7");
                for (x, y) in [(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)] {
                    glf(s, "glVertex3f", &[x + (i % 2) as f64 * 8.0, y + (i / 2) as f64 * 8.0 - 4.0, z]);
                }
                writeln!(s, "gl glEnd");
            }
            for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
                let n = 256 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
                writeln!(s, "glread 0 0 16 16 {} {} {n}", h(DEPTH_COMPONENT), h(t));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // GL_NEVER again: which rectangles are drawn all the same (some clipped ones are), on 32
    // and 16 depth bits, with the depth mask off
    let mut q = XorShift32::new(0x0960_2002);
    for (k, (pf, extra)) in [("glctx 32 choose 0x38 0 32 0 0 32 8", ""), ("glctx 32 choose 0x38 0 32 0 0 32 8", "gl glDepthMask 0\n"), ("glctx 32 2", ""), ("glctx 32 choose 0x38 0 32 0 0 32 8", "gl glDepthFunc 0x201\n")].iter().enumerate() {
        write!(s, "case never-more-{k}\n{pf}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x200\n{extra}");
        for _ in 0..24 {
            writeln!(s, "gl glClear 0x4100");
            color(s, &mut q);
            glf(s, "glRectf", &[v8(&mut q, -2, 8), v8(&mut q, -2, 8), v8(&mut q, 8, 18), v8(&mut q, 8, 18)]);
            read_all(s);
        }
        writeln!(s, "end");
    }
    // clears of the accumulation buffer one pixel at a time: values everywhere in -1..1 and at
    // the ends, returned at several scales
    let mut q = XorShift32::new(0x0960_2003);
    for bpp in [32u32, 24, 16] {
        writeln!(s, "case accum-clear-fine-{bpp}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
        for i in 0..48 {
            let c: Vec<String> = (0..4).map(|k| match (i, k) {
                (0, _) => f(1.0),
                (1, _) => f(-1.0),
                (2, _) => f(0.5),
                _ => f((q.next() % 4097) as f64 / 4096.0),
            }).collect();
            let v = [1.0, 1.0, 1.0, 0.5, 2.0, 1.5][i % 6];
            writeln!(s, "gl glClearAccum {}\ngl glClear 0x200\ngl glAccum 0x102 {}\nglread 0 0 1 1 {} {} 4", c.join(" "), f(v), h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // GL_RETURN of an accumulation buffer never loaded or cleared, twice, on each depth
    for bpp in [32, 24, 16, 8, 4] {
        writeln!(s, "case accum-undefined-{bpp}\n{}{ORTHO}gl glDisable 0xbd0\ngl glAccum 0x102 1\nglread 0 0 16 16 {} {} 1024\ngl glAccum 0x103 2\ngl glAccum 0x102 1\nglread 0 0 16 16 {} {} 1024\nglerr\nend", ctx(bpp), h(RGBA), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE));
    }
    // many triangles, each read alone in red: vertices on the eighth grid (cases 0..5) and
    // anywhere (6..11), both windings, some past the window
    let mut q = XorShift32::new(0x0960_2004);
    for k in 0..12 {
        writeln!(s, "case tri-many-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255", ctx(32));
        for _ in 0..32 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            for _ in 0..3 {
                let (x, y) = if k < 6 { (v8(&mut q, -2, 18), v8(&mut q, -2, 18)) } else { (q.range(-2048, 18 * 1024) as f64 / 1024.0, q.range(-2048, 18 * 1024) as f64 / 1024.0) };
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "end");
    }
    // dithering of glDrawPixels and of fragments under a color mask, 16, 8 and 4 bits
    let mut q = XorShift32::new(0x0960_2005);
    for (k, (bpp, pf)) in [(16u32, 0u32), (8, 25), (4, 31)].iter().enumerate() {
        let head = if *pf == 0 { ctx(*bpp) } else { format!("glctx {bpp} {pf}\n") };
        writeln!(s, "case dither-draw-{k}\n{head}{ORTHO}gl glEnable 0xbd0");
        for i in 0..6 {
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gldib");
            gl(s, "glColorMask", &[(i & 1) as u32, 1, ((i >> 1) & 1) as u32, 1]);
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib\ngl glColorMask 1 1 1 1");
        }
        writeln!(s, "glerr\nend");
    }
    // color index elements of every type, drawn and read back, at 8 and 4 bits
    for (k, (bpp, pf)) in [(8u32, 29u32), (4, 35)].iter().enumerate() {
        write!(s, "case ci-types-{k}\nglctx {bpp} {pf}\n{ORTHO}");
        for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
            let n = 32 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
            writeln!(s, "gl glClear 0x4000\ngldraw 16 2 {} {} {} {n}\ngldib", h(COLOR_INDEX), h(t), 100 + t);
            for rt in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
                let m = 32 * [1, 1, 2, 2, 4, 4, 4][(rt - BYTE) as usize];
                writeln!(s, "glread 0 0 16 2 {} {} {m}", h(COLOR_INDEX), h(rt));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // blending with destination alpha: every source and destination factor pair, two random
    // pictures each under one random color
    let mut q = XorShift32::new(0x0960_2006);
    for (i, sf) in BLEND_SRC.iter().enumerate() {
        writeln!(s, "case blend-dstalpha-{i}\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0");
        for df in BLEND_DST {
            for _ in 0..2 {
                writeln!(s, "gl glDisable 0xbe2");
                random_dst(s, q.next() & 0xffff);
                gl(s, "glBlendFunc", &[*sf, *df]);
                writeln!(s, "gl glEnable 0xbe2");
                let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
                gl(s, "glColor4ub", &c);
                writeln!(s, "gl glRectf 0 0 16 16");
                read_all(s);
            }
        }
        writeln!(s, "end");
    }
    // glPixelZoom: an 8 x 8 image drawn at several zooms, down and up, whole and fractional
    let mut q = XorShift32::new(0x0960_2007);
    writeln!(s, "case pixel-zoom\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0");
    for z in ["2 2", "0.5 0.5", "3 1.5", "1.25 0.75", "0.3 2", "1.5 1", "1 2.5", "0.75 0.25"] {
        writeln!(s, "gl glClear 0x4000\ngl glPixelZoom {z}\ngldraw 8 8 {} {} {} 256", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff);
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // GL_MAP_COLOR with the initial maps (one entry each, 0): images drawn through them, and
    // read through them
    writeln!(s, "case transfer-map-color\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0");
    writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd10 1\ngldraw 16 16 {} {} {} 1024", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff);
    read_all(s);
    writeln!(s, "gl glPixelTransferi 0xd10 0\ngldraw 16 16 {} {} {} 1024", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff);
    read_all(s);
    writeln!(s, "gl glPixelTransferi 0xd10 1");
    read_all(s);
    writeln!(s, "glread 0 0 16 16 {} {} 1024", h(RGBA), h(FLOAT));
    writeln!(s, "glerr\nend");
    // GL_BITMAP images of color indices drawn in color index mode under the unpack parameters,
    // and indices and stencil values read back as GL_BITMAP under the pack ones (13 pixels a row:
    // a partial last byte)
    let mut q = XorShift32::new(0x0960_2008);
    write!(s, "case bitmap-index-0\nglctx 8 29\n{ORTHO}");
    // (store: lsb first, row length, skip rows, skip pixels, alignment)
    for (lsb, rl, sr, sp, al) in [(0, 0, 0, 0, 4), (1, 0, 0, 0, 1), (0, 20, 1, 3, 2), (1, 16, 0, 5, 8)] {
        writeln!(s, "gl glClear 0x4000");
        for (pn, v) in [(0xcf1, lsb), (0xcf2, rl), (0xcf3, sr), (0xcf4, sp), (0xcf5, al)] {
            writeln!(s, "gl glPixelStorei {} {v}", h(pn));
        }
        writeln!(s, "gldraw 13 3 {} {} {} 64\nglerr", h(COLOR_INDEX), h(BITMAP), q.next() & 0xffff);
        writeln!(s, "glread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE));
        for (pn, v) in [(0xd01, lsb), (0xd02, rl), (0xd03, sr), (0xd04, sp), (0xd05, al)] {
            writeln!(s, "gl glPixelStorei {} {v}", h(pn));
        }
        writeln!(s, "glread 0 0 13 3 {} {} 64\nglerr", h(COLOR_INDEX), h(BITMAP));
        for pn in [0xd01, 0xd02, 0xd03, 0xd04] {
            writeln!(s, "gl glPixelStorei {} 0", h(pn));
        }
        writeln!(s, "gl glPixelStorei 0xd05 4");
    }
    writeln!(s, "end");
    write!(s, "case bitmap-index-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}");
    for v in [0xa5u32, 0x5a, 0x01, 0xfe] {
        writeln!(s, "gl glClearStencil {}\ngl glClear 0x400\nglread 0 0 13 3 {} {} 64\ngl glPixelStorei 0xd01 1\nglread 2 1 13 3 {} {} 64\ngl glPixelStorei 0xd01 0", h(v), h(STENCIL_INDEX), h(BITMAP), h(STENCIL_INDEX), h(BITMAP));
    }
    writeln!(s, "glerr\nend");
    // depth and stencil images: every type, drawn through the depth test (ALWAYS, then LESS
    // over a depth already there) and under the stencil write mask, read back
    let mut q = XorShift32::new(0x0960_2009);
    write!(s, "case draw-depthstencil-0\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\n");
    for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
        let n = 128 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
        for func in [0x207u32, 0x201] {
            writeln!(s, "gl glDepthFunc {}", h(func));
            if func == 0x207 {
                writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x4100");
            }
            writeln!(s, "gl glColor4ub {} {} {} 255\ngldraw 16 8 {} {} {} {n}", q.next() & 255, q.next() & 255, q.next() & 255, h(DEPTH_COMPONENT), h(t), q.next() & 0xffff);
            writeln!(s, "glread 0 0 16 8 {} {} 512\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_INT), h(RGBA), h(UNSIGNED_BYTE));
        }
    }
    writeln!(s, "glerr\nend");
    write!(s, "case draw-depthstencil-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\n");
    for (t, mask) in [(BYTE, 0xffu32), (UNSIGNED_BYTE, 0xf0), (SHORT, 0xff), (UNSIGNED_SHORT, 0x3c), (INT, 0xff), (UNSIGNED_INT, 0x81), (FLOAT, 0xff)] {
        let n = 128 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
        writeln!(s, "gl glStencilMask 0xff\ngl glClearStencil {}\ngl glClear 0x4400\ngl glStencilMask {}", h(q.next() & 255), h(mask));
        writeln!(s, "gldraw 16 8 {} {} {} {n}\nglread 0 0 16 8 {} {} 128\nglread 0 0 16 8 {} {} 512", h(STENCIL_INDEX), h(t), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "gl glStencilMask 0xff\ngl glClear 0x400\ngl glEnable 0xc11\ngl glScissor 3 2 7 4\ngldraw 16 8 {} {} {} 128\nglread 0 0 16 8 {} {} 128", h(STENCIL_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE));
    writeln!(s, "gl glDisable 0xc11\ngl glClear 0x400\ngl glPixelStorei 0xcf5 1\ngldraw 13 3 {} {} {} 64\nglread 0 0 16 4 {} {} 64", h(STENCIL_INDEX), h(BITMAP), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE));
    writeln!(s, "glerr\nend");
    // the calls the model does not follow yet, each with a picture to answer it: raster
    // positions under glDrawPixels (and a negative zoom from one), polygon offset under a depth
    // test, linear fog over rectangles
    let mut q = XorShift32::new(0x0960_200a);
    write!(s, "case other-calls-0\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\n");
    for (pos, zoom) in [("glRasterPos2i 3 5", "1 1"), ("glRasterPos2f 2.5 6.75", "1 1"), ("glRasterPos2i 12 12", "-1 -1"), ("glRasterPos2f 9.25 2.5", "-1.5 2"), ("glRasterPos2i 14 -1", "1 1")] {
        writeln!(s, "gl glClear 0x4000\ngl glColor4ub 255 255 255 255\ngl {pos}\ngl glPixelZoom {zoom}\ngldraw 8 8 {} {} {} 256", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff);
        read_all(s);
    }
    writeln!(s, "gl glPixelZoom 1 1\nglerr\nend");
    write!(s, "case other-calls-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x201\n");
    for (fac, units) in [("0 0", "8 8"), ("1 1", "0 0"), ("0 -64", "0 0"), ("2 4", "-1 3")] {
        writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x4100\ngl glPolygonOffset {fac}\ngl glEnable 0x8037\ngl glColor4ub 255 0 0 255\ngl glRectf 2 2 14 14\ngl glPolygonOffset {units}\ngl glColor4ub 0 255 0 255\ngl glRectf 4 4 12 12\ngl glDisable 0x8037");
        read_all(s);
        writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
    }
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-2\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\n");
    for (mode, a, b) in [(0x2601u32, "-1", "1"), (0x800, "0.5", "0"), (0x801, "2", "0")] {
        writeln!(s, "gl glClear 0x4000\ngl glFogf 0xb65 {mode}\ngl glFogf 0xb63 {a}\ngl glFogf 0xb64 {b}\ngl glFogf 0xb62 {a}");
        for _ in 0..4 {
            let (x0, y0) = (q.next() % 12, q.next() % 12);
            writeln!(s, "gl glColor4ub {} {} {} 255\ngl glBegin 7\ngl glVertex3f {x0} {y0} -0.5\ngl glVertex3f {} {y0} 0\ngl glVertex3f {} {} 0.5\ngl glVertex3f {x0} {} 0.25\ngl glEnd", q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, x0 + 4, y0 + 4, y0 + 4);
        }
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // the pixel transfer the model does not follow yet: depth scale and bias over a depth image,
    // index shift and offset over a color index image
    write!(s, "case transfer-other\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\n");
    for (sc, bi) in [("0.5", "0.25"), ("2", "-0.5"), ("1", "0.125")] {
        writeln!(s, "gl glClearDepth 0\ngl glClear 0x100\ngl glPixelTransferf 0xd1e {sc}\ngl glPixelTransferf 0xd1f {bi}\ngldraw 16 8 {} {} {} 512\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_SHORT), q.next() & 0xffff, h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        writeln!(s, "gl glPixelTransferf 0xd1e 1\ngl glPixelTransferf 0xd1f 0\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
    }
    writeln!(s, "glerr\nend");
    write!(s, "case transfer-other-ci\nglctx 8 29\n{ORTHO}");
    for (sh, off) in [(1, 0), (-2, 3), (3, -5), (0, 200)] {
        writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd12 {}\ngl glPixelTransferi 0xd13 {}\ngldraw 16 4 {} {} {} 64\nglread 0 0 16 4 {} {} 64", sh, off, h(COLOR_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(COLOR_INDEX), h(UNSIGNED_BYTE));
        writeln!(s, "gl glPixelTransferi 0xd12 0\ngl glPixelTransferi 0xd13 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    // the maps on indices and stencil values (GL_MAP_COLOR in color index mode, GL_MAP_STENCIL),
    // and a name glPixelTransfer does not take
    write!(s, "case transfer-maps-ci\nglctx 8 29\n{ORTHO}");
    writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd10 1\ngldraw 16 4 {} {} 77 64\nglread 0 0 16 4 {} {} 64\ngl glPixelTransferi 0xd10 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(UNSIGNED_BYTE));
    writeln!(s, "glerr\nend");
    write!(s, "case transfer-maps-stencil\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}");
    writeln!(s, "gl glClearStencil 0x5a\ngl glClear 0x400\ngl glPixelTransferi 0xd11 1\ngldraw 16 4 {} {} 78 64\nglread 0 0 16 4 {} {} 64\ngl glPixelTransferi 0xd11 0\nglread 0 0 16 4 {} {} 64", h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(UNSIGNED_BYTE));
    writeln!(s, "glerr\ngl glPixelTransferi 0xd16 1\nglerr\ngl glPixelTransferf 0xd00 1\nglerr\nend");
    // the index and stencil maps on reads of values not 0, with the shift and offset, as
    // bytes, floats and bitmaps; a draw of indices with both
    write!(s, "case transfer-maps-read\nglctx 8 29\n{ORTHO}gl glClearIndex 85\ngl glClear 0x4000\n");
    for (m, sh) in [(1, 0), (1, 2), (0, 2)] {
        writeln!(s, "gl glPixelTransferi 0xd10 {m}\ngl glPixelTransferi 0xd12 {sh}\ngl glPixelTransferi 0xd13 {sh}\nglread 0 0 16 4 {} {} 64\nglread 0 0 16 4 {} {} 256\nglread 0 0 16 4 {} {} 8", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(FLOAT), h(COLOR_INDEX), h(BITMAP));
        writeln!(s, "gldraw 16 2 {} {} {} 32\ngl glPixelTransferi 0xd10 0\ngl glPixelTransferi 0xd12 0\ngl glPixelTransferi 0xd13 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(COLOR_INDEX), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    write!(s, "case transfer-maps-read-stencil\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glClearStencil 0xa7\ngl glClear 0x400\n");
    for (m, sh) in [(1, 0), (1, 1), (0, 1)] {
        writeln!(s, "gl glPixelTransferi 0xd11 {m}\ngl glPixelTransferi 0xd12 {sh}\ngl glPixelTransferi 0xd13 {sh}\nglread 0 0 16 4 {} {} 64\nglread 0 0 16 4 {} {} 256\nglread 0 0 16 4 {} {} 8", h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(FLOAT), h(STENCIL_INDEX), h(BITMAP));
    }
    writeln!(s, "glerr\nend");
    // glNormal3f, glVertex4f and glPixelStoref, and the capabilities that change what is
    // drawn: lighting with the initial lights, face culling, point and polygon antialiasing
    write!(s, "case other-calls-3\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\n", ctx(32));
    writeln!(s, "gl glNormal3f 0.25 -0.5 2\nglget f 0xb02 3\nglget i 0xb02 3\ngl glPushAttrib 0x1\ngl glNormal3f 1 0 0\ngl glPopAttrib\nglget f 0xb02 3");
    writeln!(s, "gl glColor4ub 200 100 50 255\ngl glBegin 7\ngl glVertex4f 1 1 0 1\ngl glVertex4f 7 1 0 1\ngl glVertex4f 7 7 0 1\ngl glVertex4f 1 7 0 1\ngl glEnd\ngl glBegin 0\ngl glVertex4f 12 4 0 1\ngl glEnd");
    read_all(s);
    for (p, v) in [(0xcf2, "3.4"), (0xcf3, "2.5"), (0xcf4, "1.6"), (0xcf5, "3.9"), (0xcf0, "0.25"), (0xcf1, "-0.5"), (0xd02, "-0.25"), (0xd05, "1.49"), (0xd05, "2.5")] {
        writeln!(s, "gl glPixelStoref {} {v}\nglerr\nglget i {} 1", h(p), h(p));
    }
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glPointSize 2\ngl glBegin 0\ngl glVertex4f 20 20 0 2\ngl glVertex4f 6 6 0.5 0.5\ngl glVertex4f -10 6 0 -2\ngl glEnd\n", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    for (k, cap) in [0xb50u32, 0xb44, 0xb10, 0xb41].into_iter().enumerate() {
        write!(s, "case render-caps-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable {}\ngl glPointSize 3\n", ctx(32), h(cap));
        writeln!(s, "gl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\ngl glColor4ub 50 200 100 128\ngl glRectf 15 1 9 7\ngl glRectf 9.25 9.25 14.5 14.75\ngl glColor4ub 10 20 250 255\ngl glBegin 0\ngl glVertex2f 3.5 12.5\ngl glVertex2f 5.3 9.7\ngl glEnd");
        read_all(s);
        writeln!(s, "gl glCullFace 0x404\ngl glFrontFace 0x900\ngl glColor4ub 250 250 0 255\ngl glRectf 2 10 6 14\ngl glRectf 1.75 14.25 0.25 9.75");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    // culling both faces; antialiased points of every size at random positions; lighting
    // with GL_COLOR_MATERIAL
    write!(s, "case render-caps-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb44\ngl glCullFace 0x408\ngl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\ngl glRectf 15 1 9 7\ngl glBegin 0\ngl glVertex2f 3.5 12.5\ngl glEnd\n", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    for k in 0..4 {
        write!(s, "case smooth-points-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb10\n", ctx(32));
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000\ngl glPointSize {}", (q.next() % 40) as f64 / 4.0 + 0.5);
            writeln!(s, "gl glColor4ub 255 255 255 255\ngl glBegin 0\ngl glVertex2f {} {}\ngl glEnd", (q.next() % 1000) as f64 / 1000.0 * 8.0 + 4.0, (q.next() % 1000) as f64 / 1000.0 * 8.0 + 4.0);
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // more antialiased points, four of a size apart in each image, at thousandths of a pixel
    for k in 4..12 {
        write!(s, "case smooth-points-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb10\n", ctx(32));
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4000\ngl glPointSize {}\ngl glColor4ub 255 255 255 255\ngl glBegin 0", (q.next() % 3500) as f64 / 1000.0 + 0.25);
            for (cx, cy) in [(4, 4), (12, 4), (4, 12), (12, 12)] {
                writeln!(s, "gl glVertex2f {} {}", cx as f64 + (q.next() % 2000) as f64 / 1000.0 - 1.0, cy as f64 + (q.next() % 2000) as f64 / 1000.0 - 1.0);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // what the model does not follow yet: fog on images, zoomed depth images, antialiasing
    // with alpha to show the coverage in, a light on, lighting in color index mode
    write!(s, "case other-calls-5\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glRasterPos2f 2.5 2.5\ngldraw 4 4 {} {} 90 64\n", ctx(32), h(RGBA), h(UNSIGNED_BYTE));
    read_all(s);
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-6\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glClear 0x4100\ngl glPixelZoom 2 3\ngl glRasterPos2f 1.5 1.5\ngldraw 4 4 {} {} 91 64\nglread 0 0 16 16 {} {} 1024\nglerr\nend\n", ctx(32), h(DEPTH_COMPONENT), h(FLOAT), h(DEPTH_COMPONENT), h(UNSIGNED_INT));
    write!(s, "case other-calls-7\nglctx 32 5\n{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\ngl glEnable 0xb10\ngl glPointSize 3.3\ngl glColor4ub 200 100 50 255\ngl glRectf 1.25 1.5 7.75 6.25\ngl glBegin 0\ngl glVertex2f 11.3 11.6\ngl glEnd\n");
    read_all(s);
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-10\nglctx 32 5\n{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\ngl glEnable 0xb10\ngl glPointSize 3.3\ngl glColor4ub 200 100 50 255\ngl glRectf 1.25 1.5 7.75 6.25\ngl glBegin 0\ngl glVertex2f 11.2 11.45\ngl glEnd\n");
    read_all(s);
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-8\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glNormal3f 0 0 1\ngl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\n", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    write!(s, "case other-calls-9\nglctx 8 29\n{ORTHO}gl glClear 0x4000\ngl glEnable 0xb50\ngl glIndexi 9\ngl glRectf 1 1 7 7\nglread 0 0 16 16 {} {} 256\nglerr\nend\n", h(COLOR_INDEX), h(UNSIGNED_BYTE));
    write!(s, "case lighting-material\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb50\ngl glEnable 0xb57\n", ctx(32));
    for i in 0..16 {
        let (x0, y0) = (i % 4 * 4, i / 4 * 4);
        writeln!(s, "gl glColor4ub {} {} {} 255\ngl glRectf {x0} {y0} {} {}", q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, y0 + 4);
    }
    read_all(s);
    writeln!(s, "glerr\nend");
    // 1-pixel lines at thousandths of a pixel (no diamond touched exactly), strips and loops,
    // some ends past the window, one in a smaller viewport
    let cr = |q: &mut XorShift32| (q.next() % 20000) as f64 / 1000.0 - 2.0;
    for k in 0..6 {
        write!(s, "case lines-{k}\n{}{ORTHO}gl glDisable 0xbd0\n", ctx(32));
        if k == 5 {
            writeln!(s, "gl glViewport 2 3 11 9");
        }
        for mode in [1, 3, 2] {
            writeln!(s, "gl glClear 0x4000\ngl glColor4ub {} {} {} 255\ngl glBegin {mode}", q.next() & 255, q.next() & 255, q.next() & 255);
            for _ in 0..6 {
                writeln!(s, "gl glVertex2f {} {}", cr(&mut q), cr(&mut q));
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // primitives wholly past the near or far plane, beside ones inside
    write!(s, "case clip-z\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glColor4ub 200 100 50 255\ngl glBegin 0\ngl glVertex3f 2.5 2.5 1.5\ngl glVertex3f 4.5 2.5 -1.5\ngl glVertex3f 6.5 2.5 0.5\ngl glEnd\ngl glBegin 1\ngl glVertex3f 0.3 6.2 -3\ngl glVertex3f 15.1 7.9 -1.25\ngl glVertex3f 0.3 9.2 0.25\ngl glVertex3f 15.1 10.9 0.25\ngl glEnd\ngl glBegin 7\ngl glVertex3f 1 12 1.01\ngl glVertex3f 5 12 1.01\ngl glVertex3f 5 15 1.01\ngl glVertex3f 1 15 1.01\ngl glVertex3f 9 12 0.99\ngl glVertex3f 13 12 0.99\ngl glVertex3f 13 15 0.99\ngl glVertex3f 9 15 0.99\ngl glEnd\n", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // fog over flat quads, one depth each, in every mode on the RGBA bit depths, a fog color,
    // the state read back; and color index fog and points
    let mut q = XorShift32::new(0x0960_200b);
    let z = |q: &mut XorShift32| format!("{}", (q.next() % 1999) as f64 / 1000.0 - 0.999);
    for (k, bpp) in [32, 16, 24, 32].into_iter().enumerate() {
        write!(s, "case fog-rects-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\n", ctx(bpp));
        for mode in [0x2601u32, 0x800, 0x801] {
            let (a, b) = (z(&mut q), z(&mut q));
            let dens = (q.next() % 4000) as f64 / 1000.0;
            writeln!(s, "gl glClear 0x4000\ngl glFogf 0xb65 {mode}\ngl glFogf 0xb63 {a}\ngl glFogf 0xb64 {b}\ngl glFogf 0xb62 {dens}");
            writeln!(s, "glfogv 0xb66 {} {} {} {}", (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0);
            for i in 0..16 {
                let (x0, y0) = (i % 4 * 4, i / 4 * 4);
                let zz = z(&mut q);
                writeln!(s, "gl glColor4ub {} {} {} {}\ngl glBegin 7\ngl glVertex3f {x0} {y0} {zz}\ngl glVertex3f {} {y0} {zz}\ngl glVertex3f {} {} {zz}\ngl glVertex3f {x0} {} {zz}\ngl glEnd", q.next() & 255, q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, x0 + 4, y0 + 4, y0 + 4);
            }
            read_all(s);
        }
        writeln!(s, "glget f 0xb66 4\nglget f 0xb62 1\nglget f 0xb63 1\nglget f 0xb64 1\nglget i 0xb65 1\nglerr");
        writeln!(s, "gl glFogf 0xb62 -1\nglerr\ngl glFogf 0xb65 4660\nglerr\ngl glFogf 0xb66 1\nglerr\nglfogv 0xb65 2049\nglerr\nglfogv 0x1234 1\nglerr\nglfogv 0xb66 2 -1 0.5 0.25\nglget f 0xb66 4\nglget i 0xb65 1\nglerr\nend");
    }
    write!(s, "case fog-points\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\ngl glClear 0x4000\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\nglfogv 0xb66 0.25 0.5 0.75 1\ngl glBegin 0\n", ctx(32));
    for i in 0..64 {
        writeln!(s, "gl glColor4ub {} {} {} 255\ngl glVertex3f {}.5 {}.5 {}", q.next() & 255, q.next() & 255, q.next() & 255, i % 16, i / 16 * 4, z(&mut q));
    }
    writeln!(s, "gl glEnd");
    read_all(s);
    writeln!(s, "glerr\nend");
    write!(s, "case fog-ci\nglctx 8 29\n{ORTHO}gl glEnable 0xb60\ngl glClear 0x4000\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glFogf 0xb61 {}\n", q.next() % 200);
    for i in 0..16 {
        let (x0, y0) = (i % 4 * 4, i / 4 * 4);
        let zz = z(&mut q);
        writeln!(s, "gl glIndexi {}\ngl glBegin 7\ngl glVertex3f {x0} {y0} {zz}\ngl glVertex3f {} {y0} {zz}\ngl glVertex3f {} {} {zz}\ngl glVertex3f {x0} {} {zz}\ngl glEnd", q.next() % 56, x0 + 4, x0 + 4, y0 + 4, y0 + 4);
    }
    writeln!(s, "glread 0 0 16 16 {} {} 256\nglerr\nend", h(COLOR_INDEX), h(UNSIGNED_BYTE));
    // polygon offset units on flat rectangles, on 32 and 16 depth bits, the state read back
    for (k, c) in ["glctx 32 choose 0x38 0 32 0 0 32 8", "glctx 32 2"].into_iter().enumerate() {
        write!(s, "case poly-offset-{k}\n{c}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glEnable 0x8037\n");
        for units in ["1", "-1", "2", "-3", "0.5", "-0.5", "-1000", "100000", "-100000000", "7.25"] {
            let zc = z(&mut q);
            writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x100\ngl glPolygonOffset {} {units}\ngl glBegin 7\ngl glVertex3f 0 0 {zc}\ngl glVertex3f 16 0 {zc}\ngl glVertex3f 16 8 {zc}\ngl glVertex3f 0 8 {zc}\ngl glEnd\ngl glRectf 0 8 16 16", (q.next() % 9) as f64 - 4.0);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "glget f 0x8038 1\nglget f 0x2a00 1\ngl glPushAttrib 0x8\ngl glPolygonOffset 3 4\ngl glPopAttrib\nglget f 0x8038 1\nglget f 0x2a00 1\nglerr\nend");
    }
    // the same triangle twice: under the window's own viewport, where the clip volume cuts the
    // ones past the window, and under a viewport reaching 16 pixels past every side, where
    // nothing is cut and only the window bounds the pixels; vertices on the eighth grid (0, 1)
    // and anywhere (2, 3)
    let mut q = XorShift32::new(0x0960_200c);
    for k in 0..4 {
        writeln!(s, "case tri-clip-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32));
        for _ in 0..32 {
            let v: Vec<(f64, f64)> = (0..3).map(|_| if k < 2 { (v8(&mut q, -2, 18), v8(&mut q, -2, 18)) } else { (q.range(-2048, 18 * 1024) as f64 / 1024.0, q.range(-2048, 18 * 1024) as f64 / 1024.0) }).collect();
            for (vp, o) in [("0 0 16 16", "0 16 0 16"), ("-16 -16 48 48", "-16 32 -16 32")] {
                writeln!(s, "gl glViewport {vp}\ngl glLoadIdentity\ngl glOrtho {o} -1 1\ngl glClear 0x4000\ngl glBegin 4");
                for (x, y) in &v {
                    glf(s, "glVertex2f", &[*x, *y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // blending with dithering on 16, 8 and 4-bit DIBs: random factor pairs, a random picture
    // drawn undithered under each, one random color over it dithered
    let mut q = XorShift32::new(0x0960_200d);
    for (k, (bpp, pf)) in [(16u32, 0u32), (8, 25), (4, 31)].iter().enumerate() {
        let head = if *pf == 0 { ctx(*bpp) } else { format!("glctx {bpp} {pf}\n") };
        writeln!(s, "case blend-dither-{k}\n{head}{ORTHO}");
        for _ in 0..24 {
            let (sf, df) = (*q.pick(BLEND_SRC), *q.pick(BLEND_DST));
            writeln!(s, "gl glDisable 0xbd0\ngl glDisable 0xbe2");
            random_dst(s, q.next() & 0xffff);
            gl(s, "glBlendFunc", &[sf, df]);
            writeln!(s, "gl glEnable 0xbd0\ngl glEnable 0xbe2");
            let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
            gl(s, "glColor4ub", &c);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib");
        }
        writeln!(s, "glerr\nend");
    }
    // triangles whose pixels Windows decides otherwise than the exact edges do (tri-many-*,
    // tri-clip-*: up to 0.006 pixels inward or outward), each under seven viewports, the object
    // coordinates kept as window coordinates: the window's own (the clip volume cuts the ones past
    // it), ones reaching 2, 16, 64 and 512 pixels past every side (nothing cut, the window
    // coordinates' viewport-relative values growing), a 64-pixel one from the corner and one
    // centred off the window; in 1/1024 pixels
    const PROBE: [[(i32, i32); 3]; 25] = [
        [(14080, 6144), (16512, 5248), (4736, 16640)],
        [(768, 17152), (3712, -1792), (768, 18304)],
        [(9600, -1280), (2560, 12032), (17408, 13696)],
        [(2304, 3712), (4224, 17408), (5248, 14080)],
        [(17664, -1664), (768, 16896), (2560, 7296)],
        [(12288, 3712), (1792, 10752), (-640, 1152)],
        [(9728, 3584), (3328, -1920), (13952, 4864)],
        [(4003, 1441), (-2015, 5913), (12488, 8749)],
        [(12820, 2569), (7157, -254), (888, 12289)],
        [(11830, 14545), (567, 1947), (10463, 9922)],
        [(2693, 11098), (-1703, -121), (-111, 15748)],
        [(-729, 2669), (17378, 6285), (4140, 14637)],
        [(18415, 2167), (11953, 12950), (15173, 11829)],
        [(9933, 1039), (1728, 5155), (17096, 6949)],
        [(14464, 18304), (16896, 12928), (6144, 0)],
        [(9984, 2816), (12800, 6784), (17024, 14976)],
        [(4992, 16384), (1792, 5248), (17152, 6656)],
        [(128, 8064), (4608, 9984), (9856, 17536)],
        [(896, 1664), (8832, 10368), (12032, -640)],
        [(5421, 10975), (-576, 4799), (-68, 16443)],
        [(14741, 12632), (14955, -655), (-423, 14514)],
        [(16209, 7038), (45, -161), (4635, -230)],
        [(15123, 12624), (17881, 4557), (4775, 12871)],
        [(12032, 14976), (8704, 9728), (8960, 12288)],
        [(17294, 15114), (12459, 14799), (7281, 6022)]
    ];
    for (k, part) in PROBE.chunks(5).enumerate() {
        writeln!(s, "case tri-probe-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701", ctx(32));
        for t in part {
            for (vp, o) in [("0 0 16 16", "0 16 0 16"), ("-2 -2 20 20", "-2 18 -2 18"), ("-16 -16 48 48", "-16 32 -16 32"), ("-64 -64 144 144", "-64 80 -64 80"), ("-512 -512 1040 1040", "-512 528 -512 528"), ("0 0 64 64", "0 64 0 64"), ("-48 -48 64 64", "-48 16 -48 16")] {
                writeln!(s, "gl glViewport {vp}\ngl glLoadIdentity\ngl glOrtho {o} -1 1\ngl glClear 0x4000\ngl glBegin 4");
                for (x, y) in t {
                    glf(s, "glVertex2f", &[f64::from(*x) / 1024.0, f64::from(*y) / 1024.0]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // one triangle edge measured alone: an edge through the window from y = -100 to y = 116
    // (slope s in x per y, through x0 at y = 8), the third vertex 400 pixels to its right (the
    // edge the left boundary) or left (the right one), under a viewport 512 pixels past every side
    // so that nothing is cut; x0 swept over -16/2048 .. 16/2048 around a point between pixel
    // centres, each row's pixel flipping where Windows puts the edge in that row
    const SLOPES: [f64; 10] = [0.0, 1.0 / 64.0, 0.125, 33.0 / 64.0, 1.0, -1.0, 1.2109375, -1.03125, 2.25, 0.4296875];
    for (k, sl) in SLOPES.iter().enumerate() {
        for (side, dx) in [("l", 400.0), ("r", -400.0)] {
            writeln!(s, "case tri-edge-{side}{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
            for j in -16..=16 {
                let x0 = 8.25 + f64::from(j) / 2048.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                glf(s, "glVertex2f", &[x0 - 108.0 * sl, -100.0]);
                glf(s, "glVertex2f", &[x0 + 108.0 * sl, 116.0]);
                glf(s, "glVertex2f", &[x0 + dx, 8.0]);
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
            writeln!(s, "glerr\nend");
        }
    }
    // the PROBE triangles swept sideways by -8/1024 .. 8/1024 pixel under a viewport that cuts
    // nothing: where each of a triangle's edges flips in each row, against the lone edges of
    // tri-edge-*, which Windows puts where they are
    for (k, part) in PROBE.chunks(5).enumerate() {
        writeln!(s, "case tri-sweep-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for t in part {
            for j in -8..=8 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                for (x, y) in t {
                    glf(s, "glVertex2f", &[f64::from(*x + j) / 1024.0, f64::from(*y) / 1024.0]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // a lone edge (vertical, and slope 1: every row's pixel centre on it, through x = 8.5 at
    // y = 8.5, the left boundary) made part of a
    // smaller triangle step by step: the third vertex nearer (400 .. 10 pixels to the right), the
    // lower vertex higher (y = -100 .. 3.8125, inside the window), both; each swept by -8/1024 ..
    // 8/1024 pixel under a viewport that cuts nothing
    const NEAR: [(f64, f64, f64); 11] = [(-100.0, 400.0, 8.0), (-100.0, 100.0, 8.0), (-100.0, 40.0, 8.0), (-100.0, 16.0, 8.0), (-100.0, 10.0, 8.0), (-20.0, 400.0, 8.0), (-2.6875, 400.0, 8.0), (1.3125, 400.0, 8.0), (3.8125, 400.0, 8.0), (1.3125, 10.0, 8.0), (1.3125, 10.0, 12.3125)];
    for (k, (ya, dx, yc, sl)) in NEAR.iter().flat_map(|&(a, b, c)| [(a, b, c, 0.0), (a, b, c, 1.0)]).enumerate() {
        writeln!(s, "case tri-near-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (yc - 8.0) + dx, yc]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // the same edges (vertical through x = 8.5, slope 1 through the pixel centres) with the lower
    // vertex at uneven heights inside the window and the third vertex 10 pixels to the right (left
    // boundary) or left (right boundary): whether the edge moves with where its vertices lie
    const LOW: [f64; 6] = [1.0009765625, 2.3330078125, 3.7509765625, 4.1259765625, 5.6669921875, 6.9990234375];
    for (k, (ya, sl, dx)) in LOW.iter().flat_map(|&a| [(a, 0.0, 10.0), (a, 1.0, 10.0), (a, 0.0, -10.0), (a, 1.0, -10.0)]).enumerate() {
        writeln!(s, "case tri-low-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + dx, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // edges a hair off the vertical and off slope 1 (non-dyadic slopes, every row's pixel centre
    // within 8/1024 of the edge), lower vertex far below or inside the window, as the left and the
    // right boundary: whether a slope that is no short binary fraction moves the edge row by row
    const HAIR: [f64; 4] = [1.0 / 3000.0, -1.0 / 3000.0, 1.0 + 1.0 / 3000.0, 1.0 - 1.0 / 2900.0];
    let mut cfg = Vec::new();
    for sl in HAIR {
        for ya in [-100.0, 1.3125] {
            for dx in [10.0, -10.0] {
                cfg.push((sl, ya, dx));
            }
        }
    }
    for (k, (sl, ya, dx)) in cfg.into_iter().enumerate() {
        writeln!(s, "case tri-hair-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + dx, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // the edge a hair off the vertical (slope 1/3000, the left boundary) with the lower vertex at
    // five heights and the upper one at y = 116 or 32: which rows Windows moves the edge in
    // (tri-hair-*: with the lower vertex inside the window the edge stays put for ten rows, then
    // moves in rows 12 and 14)
    let mut cfg = Vec::new();
    for ya in [0.3125, 1.3125, 1.8125, 3.3125, 5.3125] {
        for yb in [116.0, 32.0] {
            cfg.push((ya, yb));
        }
    }
    let sl = 1.0 / 3000.0;
    for (k, (ya, yb)) in cfg.into_iter().enumerate() {
        writeln!(s, "case tri-hair2-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * (yb - 8.0), yb]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + 10.0, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // single edges swept by 1/8192 pixel across the center (8.5, 8.5): vertical left and right
    // boundaries with far vertices, with the upper or the lower vertex inside the window, slopes
    // 1/64 and 1/3 through the center, and horizontal lower and upper boundaries: where Windows
    // rounds a vertex, where it starts an edge's walk, and how it steps it (tri-hair2-*: the
    // vertices look rounded to 1/256 and the edges walked from the upper vertex)
    let fine: [fn(f64) -> [(f64, f64); 3]; 9] = [
        |d| [(8.5 + d, -100.0), (8.5 + d, 116.0), (408.5 + d, 8.0)],
        |d| [(8.5 + d, -100.0), (8.5 + d, 116.0), (-391.5 + d, 8.0)],
        |d| [(8.5 + d, -100.0), (8.5 + d, 12.3125), (408.5 + d, 8.0)],
        |d| [(8.5 + d, 3.6875), (8.5 + d, 116.0), (408.5 + d, 8.0)],
        |d| [(8.5 - 108.5 / 64.0 + d, -100.0), (8.5 + 107.5 / 64.0 + d, 116.0), (408.5 + d, 8.0)],
        |d| [(8.5 - 108.5 / 3.0 + d, -100.0), (8.5 + 107.5 / 3.0 + d, 116.0), (408.5 + d, 8.0)],
        |d| [(8.5 - 108.5 / 3.0 + d, -100.0), (8.5 + 107.5 / 3.0 + d, 116.0), (-391.5 + d, 8.0)],
        |d| [(-400.0, 8.5 + d), (400.0, 8.5 + d), (0.0, 200.0)],
        |d| [(-400.0, 8.5 + d), (400.0, 8.5 + d), (0.0, -200.0)],
    ];
    for (k, tri) in fine.iter().enumerate() {
        writeln!(s, "case tri-fine-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for j in -24..=24 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            for (x, y) in tri(f64::from(j) / 8192.0) {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // a sloped left edge through (8.5 + p / 1024, 8.5) with far vertices, swept by 1/8192: where
    // Windows rounds vertices with other fractions of 1/256 (tri-fine-5: the slope-1/3 line
    // moves 2/8192 later than rounding half down would have it)
    let mut k = 0;
    for sl in [1.0 / 3.0, 1.0 / 5.0, 2.0 / 3.0, 3.0 / 7.0] {
        for p in 0..4 {
            writeln!(s, "case tri-snap-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
            for j in -24..=24 {
                let d = f64::from(p) / 1024.0 + f64::from(j) / 8192.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                for (x, y) in [(8.5 - 108.5 * sl + d, -100.0), (8.5 + 107.5 * sl + d, 116.0), (408.5 + d, 8.0)] {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
            writeln!(s, "glerr\nend");
            k += 1;
        }
    }
    // the same lines from y = -1 with the upper vertex at 116, 64, 32 or 18, and the last three
    // under narrower viewports (20, 48, 144 wide, centered on (8, 8)) that still hold them:
    // whether the slope-dependent offset of tri-snap-* grows with the rows walked, and whether
    // the viewport's size moves it
    let mut k = 0;
    for sl in [1.0 / 3.0, 1.0 / 5.0, 2.0 / 3.0, 3.0 / 7.0] {
        for (top, vw) in [(116.0, 1040), (64.0, 1040), (32.0, 1040), (18.0, 1040), (18.0, 20), (32.0, 48), (64.0, 144)] {
            let lo = 8 - vw / 2;
            writeln!(s, "case tri-len-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport {lo} {lo} {vw} {vw}\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", ctx(32), lo + vw, lo + vw);
            let bottom = -1.0;
            for j in -24..=24 {
                let d = f64::from(j) / 8192.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                for (x, y) in [(8.5 + (bottom - 8.5) * sl + d, bottom), (8.5 + (top - 8.5) * sl + d, top), (8.5 + (8.0 - 8.5) * sl + d + 4.0, 8.0)] {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
            writeln!(s, "glerr\nend");
            k += 1;
        }
    }
    // one triangle under viewports of ten sizes, all centered on (8, 8): a vertical left edge at
    // x = 8.5 + d, a flat lower edge at y = 8.5 + d, a 1/3 slope through (8.5, 8.5) + d; where
    // the coverage of the pixel next to the edge flips tells where each viewport snaps it
    let shapes: [fn(f64) -> [(f64, f64); 3]; 3] = [
        |d| [(8.5 + d, 1.0), (8.5 + d, 15.0), (15.0, 8.0)],
        |d| [(1.0, 8.5 + d), (15.0, 8.5 + d), (8.0, 15.0)],
        |d| [(8.5 - 7.5 / 3.0 + d, 1.0), (8.5 + 6.5 / 3.0 + d, 15.0), (15.0, 8.0)],
    ];
    let mut k = 0;
    for vw in [16, 20, 32, 48, 64, 144, 272, 528, 1040, 2064] {
        let lo = 8 - vw / 2;
        for shape in shapes {
            writeln!(s, "case tri-vp-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport {lo} {lo} {vw} {vw}\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", ctx(32), lo + vw, lo + vw);
            for j in -24..=24 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                for (x, y) in shape(f64::from(j) / 8192.0) {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
            writeln!(s, "glerr\nend");
            k += 1;
        }
    }
    // a vertex at NDC +-1/2 under an 18 wide viewport lands on an exact half of a 1/256 step
    // (8 + 1/2 (9 - 1/256) = 12.498046875) right beside a pixel center: which way a tie snaps
    writeln!(s, "case tri-tie\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255", ctx(32));
    for lo in [-1, -3] {
        let c = f64::from(lo + 9);
        writeln!(s, "gl glViewport {lo} {lo} 18 18\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", lo + 18, lo + 18);
        for tri in [
            [(c + 4.5, 1.0), (c + 4.5, 15.0), (c + 8.5, 8.0)],
            [(c - 4.5, 1.0), (c - 4.5, 15.0), (c - 8.5, 8.0)],
            [(1.0, c + 4.5), (15.0, c + 4.5), (8.0, c + 8.5)],
            [(1.0, c - 4.5), (15.0, c - 4.5), (8.0, c - 8.5)],
        ] {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            for (x, y) in tri {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
        }
    }
    writeln!(s, "glerr\nend");
    // long left edges whose first x lands near a half of a 1/256 step, where an exact start,
    // a float32 start, a start from the float32 slope and one from the 2^-22 slope round apart
    for (k, tri) in [
        [(26.375, -12.875), (8.60546875, 28.08984375)],
        [(-9.7734375, -1.76171875), (8.33203125, 19.5390625)],
        [(14.359375, -12.4375), (10.13671875, 19.1875)],
        [(9.22265625, -8.43359375), (4.74609375, 30.36328125)],
        [(-16.10546875, -21.6640625), (6.9375, 27.1328125)],
        [(16.640625, -27.0703125), (0.95703125, 39.1015625)],
    ]
    .into_iter()
    .enumerate()
    {
        writeln!(s, "case tri-start-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700\ngl glClear 0x4000\ngl glBegin 4", ctx(32));
        for (x, y) in [tri[0], tri[1], (60.0, 8.0)] {
            glf(s, "glVertex2f", &[x, y]);
        }
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256\nglerr\nend", h(RED), h(UNSIGNED_BYTE));
    }
    // smooth triangles over the whole window, inside a 1040 viewport (no clipping): corners
    // at (-56, -56), (200, -56), (-56, 200) so each channel's plane steps by (c1 - c0) / 256
    // a pixel along x and (c2 - c0) / 256 along y; colors from a sweep, each order of the
    // corners: how Windows interpolates
    let mut r = XorShift32::new(0x1f7e_4a0d);
    for k in 0..8 {
        writeln!(s, "case interp-{k}\n{}gl glDisable 0xbd0\ngl glShadeModel 0x1d01\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for _ in 0..4 {
            let cols: [[u32; 3]; 3] = std::array::from_fn(|_| std::array::from_fn(|_| r.next() % 256));
            let corners = [(-56.0, -56.0), (200.0, -56.0), (-56.0, 200.0)];
            for order in [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1]] {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
                for i in order {
                    let c = cols[i];
                    writeln!(s, "gl glColor4ub {} {} {} 255", c[0], c[1], c[2]);
                    glf(s, "glVertex2f", &[corners[i].0, corners[i].1]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // depth across one-colored triangles, read back as 32-bit integers: the window's corners
    // under big triangles in a 1040 viewport (planes of dyadic and other slopes) and small
    // ones inside a 16 viewport, each from every corner: how Windows steps depth
    let mut r = XorShift32::new(0x2e1d_7c53);
    for k in 0..8 {
        let big = k < 4;
        let view = if big { "gl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700" } else { "gl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700" };
        writeln!(s, "case zinterp-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\n{view}", ctx(32));
        for _ in 0..4 {
            let corners: [(f64, f64); 3] = if big {
                [(-56.0, -56.0), (200.0, -56.0), (-56.0, 200.0)]
            } else {
                std::array::from_fn(|_| (f64::from(r.next() % 113) / 8.0 + 1.0, f64::from(r.next() % 113) / 8.0 + 1.0))
            };
            let z: [f64; 3] = std::array::from_fn(|i| if k % 2 == 0 && i < 3 { f64::from(r.next() % 257) / 128.0 - 1.0 } else { f64::from(r.next() % 2001) / 1000.0 - 1.0 });
            for order in [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1]] {
                writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
                for i in order {
                    glf(s, "glVertex3f", &[corners[i].0, corners[i].1, z[i]]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // more small triangles of varying depth, corners on a 1/128 grid, tall and thin ones among
    // them: how Windows steps depth from row to row along an edge
    let mut r = XorShift32::new(0x5a1d_93e7);
    for k in 8..20 {
        // (16..20: a 16-bit depth buffer)
        let ctx = if k < 16 { ctx(32) } else { "glctx 32 2\n".to_string() };
        writeln!(s, "case zinterp-{k}\n{ctx}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700");
        for _ in 0..12 {
            let thin = k % 2 == 1;
            let corners: [(f64, f64); 3] = std::array::from_fn(|_| {
                let x = if thin { f64::from(r.next() % 513) / 128.0 + 6.0 } else { f64::from(r.next() % 1921) / 128.0 };
                (x, f64::from(r.next() % 1921) / 128.0)
            });
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
            for c in corners {
                glf(s, "glVertex3f", &[c.0, c.1, f64::from(r.next() % 2001) / 1000.0 - 1.0]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "glerr\nend");
    }
    // polygons that are not convex (a bow tie, an arrow, a pentagon with a notch, a pentagram),
    // each from every vertex both ways round, as GL_POLYGON and four-cornered ones as GL_QUADS:
    // how Windows splits them
    let shapes: [&[(f64, f64)]; 4] = [
        &[(2.3125, 2.1875), (13.6875, 13.5625), (13.5625, 2.4375), (2.1875, 13.8125)],
        &[(2.0625, 2.3125), (13.9375, 8.1875), (2.1875, 13.6875), (6.0625, 8.0625)],
        &[(1.1875, 1.3125), (14.8125, 1.4375), (14.6875, 14.5625), (8.0625, 5.1875), (1.3125, 14.6875)],
        &[(8.0625, 15.1875), (12.3125, 1.4375), (1.0625, 9.8125), (14.9375, 9.6875), (3.6875, 1.3125)],
    ];
    for (k, shape) in shapes.into_iter().enumerate() {
        writeln!(s, "case poly-nc-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        let n = shape.len();
        let modes: &[u32] = if n == 4 { &[9, 7] } else { &[9] };
        for &mode in modes {
            for back in [false, true] {
                for first in 0..n {
                    writeln!(s, "gl glClear 0x4000\ngl glBegin {mode}");
                    for i in 0..n {
                        let at = if back { (first + n - i) % n } else { (first + i) % n };
                        glf(s, "glVertex2f", &[shape[at].0, shape[at].1]);
                    }
                    writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
                }
            }
        }
        writeln!(s, "glerr\nend");
    }
    // perspective through a modelview that turns: small smooth triangles about the origin,
    // turned, then pushed back into the frustum (none cut): how Windows rounds turned vertices
    let mut r = XorShift32::new(0x7e57_0b1c);
    let turns: [(f64, [f64; 3]); 8] = [(10.0, [0.0, 1.0, 0.0]), (30.0, [1.0, 0.0, 0.0]), (45.0, [0.0, 1.0, 0.0]), (60.0, [1.0, 1.0, 0.0]), (90.0, [0.0, 0.0, 1.0]), (135.0, [0.0, 1.0, 1.0]), (20.0, [1.0, 2.0, 3.0]), (-75.0, [0.0, 1.0, 0.0])];
    for (k, (angle, axis)) in turns.into_iter().enumerate() {
        writeln!(s, "case persp-turn-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glShadeModel 0x1d01\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32));
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        writeln!(s, "gl glMatrixMode 0x1700\ngl glLoadIdentity");
        glf(s, "glTranslatef", &[0.0, 0.0, -3.0]);
        glf(s, "glRotatef", &[angle, axis[0], axis[1], axis[2]]);
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
            for _ in 0..3 {
                writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256);
                let c = |r: &mut XorShift32| f64::from(r.next() % 41) / 64.0 - 0.3125;
                let v = [c(&mut r), c(&mut r), c(&mut r)];
                glf(s, "glVertex3f", &v);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "glerr\nend");
    }
    // the polygons that are not convex, moved partly past the viewport (w = 1) and, under a
    // projection, partly past the sides and the near plane: how Windows cuts them
    let shifts: [(f64, f64); 4] = [(-6.0, 0.0), (6.0, 1.0), (1.0, 6.0), (-1.0, -6.0)];
    for (k, shape) in shapes.into_iter().enumerate() {
        writeln!(s, "case clip-nc-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32));
        for (dx, dy) in shifts {
            for first in 0..2 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 9");
                for i in 0..shape.len() {
                    let at = shape[(first + i) % shape.len()];
                    glf(s, "glVertex2f", &[at.0 + dx, at.1 + dy]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    for (k, shape) in shapes.into_iter().enumerate() {
        writeln!(s, "case persp-cut-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32));
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        writeln!(s, "gl glMatrixMode 0x1700");
        // (k even: past the sides; odd: through the near plane too)
        for (dx, dy) in shifts {
            for first in 0..2 {
                writeln!(s, "gl glClear 0x4100\ngl glBegin 9");
                for i in 0..shape.len() {
                    let at = shape[(first + i) % shape.len()];
                    let z = if k % 2 == 0 { -2.0 - at.0 / 16.0 } else { -0.5 - at.1 / 8.0 };
                    glf(s, "glVertex3f", &[(at.0 + dx - 8.0) / 8.0, (at.1 + dy - 8.0) / 8.0, z]);
                }
                writeln!(s, "gl glEnd");
                read_all(s);
                writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // 1-pixel lines with an end on a diamond's border: on each corner and edge middle of pixel
    // (8, 8), leaving or arriving from 16 directions (along the axes, the edges, and between):
    // which border parts a diamond holds
    let at: [(f64, f64); 8] = [(8.5, 8.0), (9.0, 8.5), (8.5, 9.0), (8.0, 8.5), (8.25, 8.25), (8.75, 8.25), (8.75, 8.75), (8.25, 8.75)];
    let dirs: [(f64, f64); 16] = [(2.25, 0.625), (0.625, 2.25), (-0.625, 2.25), (-2.25, 0.625), (-2.25, -0.625), (-0.625, -2.25), (0.625, -2.25), (2.25, -0.625), (2.0, 0.0), (0.0, 2.0), (-2.0, 0.0), (0.0, -2.0), (1.5, 1.5), (-1.5, 1.5), (-1.5, -1.5), (1.5, -1.5)];
    for (k, p) in at.into_iter().enumerate() {
        start(s, format!("line-tie-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        for d in dirs {
            for ends in [false, true] {
                let (a, b) = if ends { ((p.0 - d.0, p.1 - d.1), p) } else { (p, (p.0 + d.0, p.1 + d.1)) };
                writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
                glf(s, "glVertex2f", &[a.0, a.1]);
                glf(s, "glVertex2f", &[b.0, b.1]);
                writeln!(s, "gl glEnd\nglread 5 5 7 7 {} {} 196", h(RGBA), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // wide lines inside the window, one per image: widths 2, 3, 4 and 2.5, x- and y-major
    let mut r = XorShift32::new(0x31de_4a07);
    for (k, width) in [2.0, 2.0, 3.0, 3.0, 4.0, 2.5].into_iter().enumerate() {
        start(s, format!("line-wide-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        glf(s, "glLineWidth", &[width]);
        for _ in 0..16 {
            let mut v = || f64::from(r.next() % 97) / 8.0 + 2.0;
            let (a, b) = ((v(), v()), (v(), v()));
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // under a projection: lines the view volume cuts (sides, near plane, behind the eye), with
    // their depths; triangles of glVertex4f corners (w 2, 1/2, -1) at w = 1 projections; culled
    // triangles with a corner behind the eye
    let mut r = XorShift32::new(0x6c0f_2d95);
    for k in 0..8 {
        writeln!(s, "case persp-more-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32));
        match k {
            0..=2 => glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]),
            3..=5 => writeln!(s, "gl glOrtho 0 16 0 16 -1 1"),
            _ => {
                glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
                writeln!(s, "gl glEnable 0xb44\ngl glCullFace {}", if k == 6 { "0x405" } else { "0x404" });
            }
        }
        writeln!(s, "gl glMatrixMode 0x1700");
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4100");
            writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256);
            let mut c = |lo: i32, hi: i32| f64::from(lo + (r.next() % (hi - lo + 1) as u32) as i32) / 8.0;
            match k {
                0..=2 => {
                    writeln!(s, "gl glBegin 1");
                    for _ in 0..2 {
                        let z = if k == 2 { c(-24, 8) } else { c(-24, -4) };
                        let v = [c(-24, 24), c(-24, 24), z];
                        glf(s, "glVertex3f", &v);
                    }
                }
                3..=5 => {
                    writeln!(s, "gl glBegin 4");
                    for _ in 0..3 {
                        let w = [2.0, 0.5, -1.0, 1.0][(c(0, 3) * 8.0) as usize];
                        let v = [c(0, 128) * w / 8.0, c(0, 128) * w / 8.0, c(-6, 6) * w / 8.0, w];
                        glf(s, "glVertex4f", &v);
                    }
                }
                _ => {
                    writeln!(s, "gl glBegin 4");
                    for i in 0..3 {
                        let z = if i == 0 { c(1, 16) } else { c(-32, -10) };
                        let v = [c(-16, 16), c(-16, 16), z];
                        glf(s, "glVertex3f", &v);
                    }
                }
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "glerr\nend");
    }
    // long 1-pixel lines whose exact path meets pixel boundaries partway (slopes p/q with q
    // not a power of 2, ends on pixel centers): how Windows steps across, in fixed or float
    let slopes: [(i32, i32); 8] = [(1, 3), (2, 3), (1, 5), (3, 7), (1, 6), (5, 6), (2, 7), (4, 9)];
    for k in 0..4 {
        start(s, format!("line-dda-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        for (i, (p, q)) in slopes.into_iter().enumerate() {
            for flip in [false, true] {
                // (k: 0 x-major rising, 1 x-major falling, 2 y-major, 3 starting off center)
                let n = 14 / q * q;
                let (major0, minor0) = (0.5 + f64::from(i as i32 % 2), 1.0 + f64::from(i as i32) / 4.0);
                let off = if k == 3 { 0.25 } else { 0.0 };
                let (dmaj, dmin) = (f64::from(n), f64::from(n / q * p) * if k == 1 { -1.0 } else { 1.0 });
                let minor0 = if k == 1 { minor0 + 13.0 } else { minor0 };
                let (a, b) = ((major0 + off, minor0), (major0 + off + dmaj, minor0 + dmin));
                let (a, b) = if flip { (b, a) } else { (a, b) };
                let (a, b) = if k == 2 { ((a.1, a.0), (b.1, b.0)) } else { (a, b) };
                writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
                glf(s, "glVertex2f", &[a.0, a.1]);
                glf(s, "glVertex2f", &[b.0, b.1]);
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
            }
        }
        writeln!(s, "glerr\nend");
    }
    // wide lines whose ends sit off every diamond border (odd 64ths): widths 2, 3, 5
    let mut r = XorShift32::new(0x2b9d_61c3);
    for (k, width) in [2.0, 3.0, 5.0].into_iter().enumerate() {
        start(s, format!("line-wide-{}", k + 6), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        glf(s, "glLineWidth", &[width]);
        for _ in 0..12 {
            let mut v = || f64::from(r.next() % 49) / 4.0 + 2.0 + f64::from(2 * (r.next() % 8) + 1) / 64.0;
            let (a, b) = ((v(), v()), (v(), v()));
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // wide lines whose first end lies past the left plane (widths 2 and 3, x- and y-major,
    // up and down), the same lines drawn the other way, and first ends past the other planes
    let mut q = XorShift32::new(0x51d7_0e2b);
    for k in 0..4 {
        start(s, format!("line-wide-{}", k + 9), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        glf(s, "glLineWidth", &[if k % 2 == 0 { 2.0 } else { 3.0 }]);
        for i in 0..12 {
            let mut v = |lo: u32, hi: u32| f64::from(lo + q.next() % (hi - lo + 1)) / 8.0;
            // the end past a plane: left of x = 0, then for k = 3 below, right of and above
            let out = v(1, 24);
            let past = match (k, i % 3) {
                (3, 0) => (v(8, 120), -out),
                (3, 1) => (16.0 + out, v(8, 120)),
                (3, _) => (v(8, 120), 16.0 + out),
                _ => (-out, v(8, 120)),
            };
            let inside = (v(16, 120), v(8, 120));
            let (a, b) = if k == 2 { (inside, past) } else { (past, inside) };
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // wide lines whose first end a plane across their minor axis cuts: y-major past the left
    // and the right, x-major past the bottom and the top (widths 2, 3, 4), each up and down
    let mut q = XorShift32::new(0x7e21_93ad);
    for k in 0..4 {
        start(s, format!("line-wide-{}", k + 13), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        glf(s, "glLineWidth", &[[2.0, 3.0, 4.0, 3.0][k]]);
        for i in 0..16 {
            let mut v = |lo: u32, hi: u32| f64::from(lo + q.next() % (hi - lo + 1)) / 8.0;
            let (out, along, far) = (v(1, 20), v(16, 112), v(24, 104));
            // (the other end inside, across by less than along so the major axis stays)
            let across = v(0, 6) * if i % 4 < 2 { 1.0 } else { -1.0 };
            let side = if k == 3 { 1 - i % 2 } else { i % 2 };
            let (a, b) = if (i / 2) % 2 == 0 {
                // y-major, past the left (side 0) or the right
                let x = if side == 0 { -out } else { 16.0 + out };
                ((x, along), (x + (out + across.abs() * 0.5 + 1.0) * if side == 0 { 1.0 } else { -1.0 }, if along < 8.0 { along + far * 0.75 } else { along - far * 0.75 }))
            } else {
                // x-major, past the bottom (side 0) or the top
                let y = if side == 0 { -out } else { 16.0 + out };
                ((along, y), (if along < 8.0 { along + far * 0.75 } else { along - far * 0.75 }, y + (out + across.abs() * 0.5 + 1.0) * if side == 0 { 1.0 } else { -1.0 }))
            };
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased lines; stippled (the default pattern, all set), antialiased and offset polygons
    for (k, setup) in ["gl glEnable 0xb20", "gl glEnable 0xb42", "gl glEnable 0xb41", "gl glEnable 0x8037\ngl glPolygonOffset 1 4\ngl glEnable 0xb71\ngl glDepthFunc 0x207"].into_iter().enumerate() {
        start(s, format!("raster-mode-{k}"), 32);
        writeln!(s, "{setup}\ngl glColor4ub 255 255 255 255");
        for _ in 0..4 {
            let mut v = || f64::from(r.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k < 1 { 3 } else { 4 });
            for _ in 0..3 {
                let p = [v(), v()];
                glf(s, "glVertex2f", &p);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased lines and polygons whose coverage shows: blended over black by their alpha
    // (lines 1 pixel and 1.5, 2, 3 wide; polygons), and lines antialiased 2 and 3 wide
    let mut q = XorShift32::new(0x3c5e_a917);
    for k in 0..4 {
        start(s, format!("raster-mode-{}", k + 4), 32);
        let setup = match k {
            0 => "gl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303",
            1 => "gl glEnable 0xb41\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303",
            2 => "gl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303",
            _ => "gl glEnable 0xb20",
        };
        writeln!(s, "{setup}\ngl glColor4ub 255 255 255 255");
        for i in 0..8 {
            if k >= 2 {
                glf(s, "glLineWidth", &[[1.5, 2.0, 3.0, 2.5][i % 4]]);
            }
            let mut v = || f64::from(q.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k == 1 { 4 } else if i % 2 == 0 { 1 } else { 3 });
            for _ in 0..if k == 1 { 3 } else { 2 + i % 2 } {
                let p = [v(), v()];
                glf(s, "glVertex2f", &p);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased 1-pixel lines (x- and y-major) and polygons into a buffer with alpha, not
    // blended: their coverage read back as alpha
    let mut q = XorShift32::new(0x6a0d_3b71);
    for k in 0..2 {
        writeln!(s, "case raster-mode-{}\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable {}\ngl glColor4ub 255 255 255 255", 8 + k, if k == 0 { "0xb20" } else { "0xb41" });
        for i in 0..12 {
            let mut v = || f64::from(q.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k == 0 { 1 } else { 4 });
            if k == 0 {
                // (alternately x- and y-major)
                let (a, b) = ((v(), v()), (v(), v()));
                let swap = (b.0 - a.0).abs() > (b.1 - a.1).abs();
                let (a, b) = if swap == (i % 2 == 0) { (a, b) } else { ((a.1, a.0), (b.1, b.0)) };
                glf(s, "glVertex2f", &[a.0, a.1]);
                glf(s, "glVertex2f", &[b.0, b.1]);
            } else {
                for _ in 0..3 {
                    let p = [v(), v()];
                    glf(s, "glVertex2f", &p);
                }
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased rectangles into a buffer with alpha whose edges fall on samples (at 1/8 and
    // 3/8 of a pixel), near the viewport's center and away from it
    writeln!(s, "case raster-mode-10\nglctx 32 5\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glColor4ub 200 100 50 255");
    for r in [[8.125, 8.125, 8.875, 8.875], [7.375, 7.625, 9.625, 9.375], [5.125, 6.375, 7.875, 9.125], [2.125, 2.125, 13.875, 4.375], [9.375, 3.125, 14.625, 13.875], [6.0, 6.0, 10.0, 10.0]] {
        writeln!(s, "gl glClear 0x4000");
        glf(s, "glRectf", &r);
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // antialiased triangles into a buffer with alpha: vertical edges 1/256 right of, on and left
    // of a column of samples (left, then right sides), an edge of slope 2 that many rows pass
    // 1/256 right of a sample, raster-mode-5's sixth and raster-mode-9's seventh and eleventh
    // again and moved by a pixel (the corners chosen to snap to whole 1/256s)
    writeln!(s, "case raster-mode-11\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glColor4ub 255 255 255 255");
    let tris: [[[f64; 2]; 3]; 14] = [
        [[5.87890625, 1.12109375], [15.87890625, 8.0], [5.87890625, 14.87890625]],
        [[5.875, 1.12109375], [15.87890625, 8.0], [5.875, 14.87890625]],
        [[5.87109375, 1.12109375], [15.87890625, 8.0], [5.87109375, 14.87890625]],
        [[5.87890625, 1.12109375], [5.87890625, 14.87890625], [0.12109375, 8.0]],
        [[5.875, 1.12109375], [5.875, 14.87890625], [0.12109375, 8.0]],
        [[5.87109375, 1.12109375], [5.87109375, 14.87890625], [0.12109375, 8.0]],
        [[2.125, 8.25], [15.62890625, 8.25], [13.375, 13.875]],
        [[3.0, 6.125], [2.125, 8.25], [13.375, 13.875]],
        [[4.00390625, 6.125], [3.125, 8.25], [14.375, 13.875]],
        [[3.0, 5.125], [2.125, 7.25], [13.375, 12.875]],
        [[7.25, 4.875], [10.125, 13.75], [4.25, 13.0]],
        [[6.25, 4.875], [9.125, 13.75], [3.24609375, 13.0]],
        [[13.375, 2.5], [12.125, 11.75], [2.875, 12.125]],
        [[13.375, 3.5], [12.125, 12.75390625], [2.875, 13.125]],
    ];
    for t in tris {
        writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
        for p in t {
            glf(s, "glVertex2f", &p);
        }
        writeln!(s, "gl glEnd");
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // antialiased triangles into a buffer with alpha whose one edge passes 1/256 right of a
    // sample (5 + 7/8, 10 + 1/8), (6 + 7/8, 10 + 1/8) or (5 + 7/8, 9 + 1/8), at slopes dx/dy of
    // 2, -2, 1, -1, 1/2, -1/2, 4 and -4, as the left side and as the right side (the corners
    // chosen to snap to whole 1/256s)
    writeln!(s, "case raster-mode-12\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glColor4ub 255 255 255 255");
    let tris: [[[f64; 2]; 3]; 48] = [
        [[3.876953125, 9.125], [7.87890625, 11.126953125], [12.568359375, 10.125]],
        [[3.876953125, 9.125], [7.87890625, 11.126953125], [0.12109375, 10.125]],
        [[7.87890625, 9.125], [3.876953125, 11.126953125], [12.568359375, 10.125]],
        [[7.87890625, 9.125], [3.876953125, 11.126953125], [0.12109375, 10.125]],
        [[3.876953125, 8.125], [7.87890625, 12.126953125], [12.568359375, 10.125]],
        [[3.876953125, 8.125], [7.87890625, 12.126953125], [0.12109375, 10.125]],
        [[7.87890625, 8.125], [3.876953125, 12.126953125], [12.568359375, 10.125]],
        [[7.87890625, 8.125], [3.876953125, 12.126953125], [0.12109375, 10.125]],
        [[4.876953125, 8.125], [6.87890625, 12.126953125], [11.568359375, 10.125]],
        [[4.876953125, 8.125], [6.87890625, 12.126953125], [0.1875, 10.125]],
        [[6.87890625, 8.125], [4.876953125, 12.126953125], [11.568359375, 10.125]],
        [[6.87890625, 8.125], [4.876953125, 12.126953125], [0.1875, 10.125]],
        [[1.875, 9.125], [9.87890625, 11.126953125], [14.5703125, 10.125]],
        [[1.875, 9.125], [9.87890625, 11.126953125], [0.12109375, 10.125]],
        [[9.87890625, 9.125], [1.875, 11.126953125], [14.5703125, 10.125]],
        [[9.87890625, 9.125], [1.875, 11.126953125], [0.12109375, 10.125]],
        [[4.876953125, 9.125], [8.87890625, 11.126953125], [13.5703125, 10.125]],
        [[4.876953125, 9.125], [8.87890625, 11.126953125], [0.1875, 10.125]],
        [[8.87890625, 9.125], [4.876953125, 11.126953125], [13.5703125, 10.125]],
        [[8.87890625, 9.125], [4.876953125, 11.126953125], [0.1875, 10.125]],
        [[4.876953125, 8.125], [8.87890625, 12.126953125], [13.5703125, 10.125]],
        [[4.876953125, 8.125], [8.87890625, 12.126953125], [0.1875, 10.125]],
        [[8.87890625, 8.125], [4.876953125, 12.126953125], [13.5703125, 10.125]],
        [[8.87890625, 8.125], [4.876953125, 12.126953125], [0.1875, 10.125]],
        [[5.87890625, 8.125], [7.87890625, 12.126953125], [12.568359375, 10.125]],
        [[5.87890625, 8.125], [7.87890625, 12.126953125], [1.1875, 10.125]],
        [[7.87890625, 8.125], [5.87890625, 12.126953125], [12.568359375, 10.125]],
        [[7.87890625, 8.125], [5.87890625, 12.126953125], [1.1875, 10.125]],
        [[2.875, 9.125], [10.87890625, 11.126953125], [15.5703125, 10.125]],
        [[2.875, 9.125], [10.87890625, 11.126953125], [0.12109375, 10.125]],
        [[10.87890625, 9.125], [2.875, 11.126953125], [15.5703125, 10.125]],
        [[10.87890625, 9.125], [2.875, 11.126953125], [0.12109375, 10.125]],
        [[3.876953125, 8.125], [7.87890625, 10.125], [12.568359375, 9.125]],
        [[3.876953125, 8.125], [7.87890625, 10.125], [0.12109375, 9.125]],
        [[7.87890625, 8.125], [3.876953125, 10.125], [12.568359375, 9.125]],
        [[7.87890625, 8.125], [3.876953125, 10.125], [0.12109375, 9.125]],
        [[3.876953125, 7.125], [7.87890625, 11.126953125], [12.568359375, 9.125]],
        [[3.876953125, 7.125], [7.87890625, 11.126953125], [0.12109375, 9.125]],
        [[7.87890625, 7.125], [3.876953125, 11.126953125], [12.568359375, 9.125]],
        [[7.87890625, 7.125], [3.876953125, 11.126953125], [0.12109375, 9.125]],
        [[4.876953125, 7.125], [6.87890625, 11.126953125], [11.568359375, 9.125]],
        [[4.876953125, 7.125], [6.87890625, 11.126953125], [0.1875, 9.125]],
        [[6.87890625, 7.125], [4.876953125, 11.126953125], [11.568359375, 9.125]],
        [[6.87890625, 7.125], [4.876953125, 11.126953125], [0.1875, 9.125]],
        [[1.875, 8.125], [9.87890625, 10.125], [14.5703125, 9.125]],
        [[1.875, 8.125], [9.87890625, 10.125], [0.12109375, 9.125]],
        [[9.87890625, 8.125], [1.875, 10.125], [14.5703125, 9.125]],
        [[9.87890625, 8.125], [1.875, 10.125], [0.12109375, 9.125]],
    ];
    for t in tris {
        writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
        for p in t {
            glf(s, "glVertex2f", &p);
        }
        writeln!(s, "gl glEnd");
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // antialiased triangles into a buffer with alpha whose left side lies on one line of slope
    // dx/dy 2 passing 1/256 right of a sample in every row (x = 1505 + 2 (y - 2592) in 1/256),
    // its lower end moved along it by 4/256 at a time over a whole row of samples, then its
    // upper end by 8/256 (the corners chosen to snap to whole 1/256s)
    writeln!(s, "case raster-mode-13\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glColor4ub 255 255 255 255");
    let tris: [[[f64; 2]; 3]; 24] = [
        [[3.126953125, 8.75], [9.87890625, 12.126953125], [15.62890625, 8.75]],
        [[3.158203125, 8.765625], [9.87890625, 12.126953125], [15.62890625, 8.765625]],
        [[3.189453125, 8.78125], [9.87890625, 12.126953125], [15.62890625, 8.78125]],
        [[3.220703125, 8.796875], [9.87890625, 12.126953125], [15.62890625, 8.796875]],
        [[3.251953125, 8.8125], [9.87890625, 12.126953125], [15.62890625, 8.8125]],
        [[3.283203125, 8.828125], [9.87890625, 12.126953125], [15.62890625, 8.828125]],
        [[3.314453125, 8.84375], [9.87890625, 12.126953125], [15.62890625, 8.84375]],
        [[3.345703125, 8.859375], [9.87890625, 12.126953125], [15.62890625, 8.859375]],
        [[3.376953125, 8.875], [9.87890625, 12.126953125], [15.62890625, 8.875]],
        [[3.408203125, 8.890625], [9.87890625, 12.126953125], [15.62890625, 8.890625]],
        [[3.439453125, 8.90625], [9.87890625, 12.126953125], [15.62890625, 8.90625]],
        [[3.470703125, 8.921875], [9.87890625, 12.126953125], [15.62890625, 8.921875]],
        [[3.501953125, 8.9375], [9.87890625, 12.126953125], [15.62890625, 8.9375]],
        [[3.533203125, 8.953125], [9.87890625, 12.126953125], [15.62890625, 8.953125]],
        [[3.564453125, 8.96875], [9.87890625, 12.126953125], [15.62890625, 8.96875]],
        [[3.595703125, 8.984375], [9.87890625, 12.126953125], [15.62890625, 8.984375]],
        [[3.126953125, 8.75], [9.62890625, 12.001953125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [9.69140625, 12.033203125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [9.75390625, 12.064453125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [9.81640625, 12.095703125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [9.87890625, 12.126953125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [9.94140625, 12.158203125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [10.00390625, 12.189453125], [15.62890625, 8.75]],
        [[3.126953125, 8.75], [10.06640625, 12.220703125], [15.62890625, 8.75]],
    ];
    for t in tris {
        writeln!(s, "gl glClear 0x4100\ngl glBegin 4");
        for p in t {
            glf(s, "glVertex2f", &p);
        }
        writeln!(s, "gl glEnd");
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // 1-pixel lines with an end on a diamond's border, as line-tie-* but about pixels on the
    // other sides of the tie pivot (13, 9), (13, 2), (8, 2)
    for (q, (px, py, read)) in [(13.0, 9.0, "10 6 6 7"), (13.0, 2.0, "10 0 6 6"), (8.0, 2.0, "5 0 7 6")].into_iter().enumerate() {
        for (k, p) in at.into_iter().enumerate() {
            let p = (p.0 - 8.0 + px, p.1 - 8.0 + py);
            start(s, format!("line-tie-{}", 8 + 8 * q + k), 32);
            writeln!(s, "gl glColor4ub 255 255 255 255");
            for d in dirs {
                for ends in [false, true] {
                    let (a, b) = if ends { ((p.0 - d.0, p.1 - d.1), p) } else { (p, (p.0 + d.0, p.1 + d.1)) };
                    writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
                    glf(s, "glVertex2f", &[a.0, a.1]);
                    glf(s, "glVertex2f", &[b.0, b.1]);
                    writeln!(s, "gl glEnd\nglread {read} {} {} 196", h(RGBA), h(UNSIGNED_BYTE));
                }
            }
            writeln!(s, "glerr\nend");
        }
    }
    // lines of slope +-1/3 through pixel boundaries at centers, on every side of the tie pivot
    // and on it, x- and y-major, both ways
    for (k, cy) in [2.0, 4.0, 7.0, 12.0].into_iter().enumerate() {
        start(s, format!("line-dda-{}", 4 + k), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255");
        for (cx, back, ahead) in [(3.5, 3.0, 6.0), (8.5, 6.0, 6.0), (12.5, 6.0, 3.0)] {
            for sign in [1.0, -1.0] {
                for ymajor in [false, true] {
                    for flip in [false, true] {
                        let (a, b) = ((cx - back, cy - back / 3.0 * sign), (cx + ahead, cy + ahead / 3.0 * sign));
                        let (a, b) = if flip { (b, a) } else { (a, b) };
                        let (a, b) = if ymajor { ((a.1, a.0), (b.1, b.0)) } else { (a, b) };
                        writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
                        glf(s, "glVertex2f", &[a.0, a.1]);
                        glf(s, "glVertex2f", &[b.0, b.1]);
                        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
                    }
                }
            }
        }
        writeln!(s, "glerr\nend");
    }
    // under a projection: points inside and past the sides, the near plane and the eye (1 and
    // 3 pixels wide); raster positions images are drawn at; fog at glVertex4f corners
    let mut r = XorShift32::new(0x1f3a_9c55);
    for k in 0..3 {
        writeln!(s, "case persp-more-{}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", 8 + k, ctx(32));
        if k < 2 {
            glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        } else {
            writeln!(s, "gl glOrtho 0 16 0 16 -1 1\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 -0.5\ngl glFogf 0xb64 0.5\nglfogv 0xb66 0.25 0.5 0.75 1");
        }
        writeln!(s, "gl glMatrixMode 0x1700");
        for i in 0..6 {
            writeln!(s, "gl glClear 0x4100");
            writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256);
            let mut c = |lo: i32, hi: i32| f64::from(lo + (r.next() % (hi - lo + 1) as u32) as i32) / 8.0;
            match k {
                0 => {
                    glf(s, "glPointSize", &[if i % 2 == 0 { 1.0 } else { 3.0 }]);
                    writeln!(s, "gl glBegin 0");
                    for _ in 0..8 {
                        let v = [c(-24, 24), c(-24, 24), c(-24, 8)];
                        glf(s, "glVertex3f", &v);
                    }
                    writeln!(s, "gl glEnd");
                }
                1 => {
                    writeln!(s, "gl glLoadIdentity");
                    glf(s, "glTranslatef", &[0.0, 0.0, c(-24, 4)]);
                    glf(s, "glRasterPos2f", &[c(-16, 16), c(-16, 16)]);
                    writeln!(s, "gldraw 4 4 {} {} {} 64", h(RGBA), h(UNSIGNED_BYTE), 92 + i);
                }
                _ => {
                    writeln!(s, "gl glBegin 4");
                    for _ in 0..3 {
                        let w = [2.0, 0.5, 1.0, 0.25][(c(0, 31) * 8.0) as usize % 4];
                        let v = [c(8, 120) * w, c(8, 120) * w, c(-8, 8) * w / 8.0, w];
                        glf(s, "glVertex4f", &v);
                    }
                    writeln!(s, "gl glEnd");
                }
            }
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT));
        }
        writeln!(s, "glerr\nend");
    }
    // glVertex4f corners of w = 0 (points, a triangle's corner, a line's end) and of w = 2
    // outside glBegin
    writeln!(s, "case persp-more-11\n{}gl glDisable 0xbd0\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32));
    let draws: [(u32, &[[f64; 4]]); 6] = [
        (0, &[[1.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [16.0, 16.0, 0.0, 2.0]]),
        (4, &[[4.0, 4.0, 0.0, 1.0], [12.0, 4.0, 0.0, 1.0], [1.0, 1.0, 0.0, 0.0]]),
        (4, &[[4.0, 4.0, 0.0, 1.0], [12.0, 4.0, 0.0, 1.0], [-1.0, 2.0, 0.0, 0.0]]),
        (1, &[[4.0, 4.0, 0.0, 1.0], [1.0, 0.5, 0.0, 0.0]]),
        (1, &[[8.0, 8.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]]),
        (0, &[]),
    ];
    for (i, (mode, vs)) in draws.iter().enumerate() {
        writeln!(s, "gl glClear 0x4000\ngl glColor4ub {} 200 100 255", 40 * i + 20);
        if vs.is_empty() {
            glf(s, "glVertex4f", &[8.0, 8.0, 0.0, 2.0]);
        }
        writeln!(s, "gl glBegin {mode}");
        for v in *vs {
            glf(s, "glVertex4f", v);
        }
        if vs.is_empty() {
            glf(s, "glVertex2f", &[3.0, 3.0]);
        }
        writeln!(s, "gl glEnd");
        read_all(s);
    }
    writeln!(s, "glerr\nend");
    // smooth lines (colors changing along them) of every direction, some cut by the window's
    // sides, at 32, 24 and 16 bits; flat-shaded lines, strips and loops of a color per vertex
    let mut r = XorShift32::new(0x11e_c010);
    for k in 0..5 {
        let bpp = [32, 32, 24, 16, 32][k];
        writeln!(s, "case line-color-{k}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
        if k == 4 {
            writeln!(s, "gl glShadeModel 0x1d00");
        }
        for n in 0..12 {
            let (mode, count) = if k == 4 { [(1, 2), (3, 3), (2, 3)][n % 3] } else { (1, 2) };
            writeln!(s, "gl glClear 0x4000\ngl glBegin {mode}");
            for _ in 0..count {
                color(s, &mut r);
                let (lo, hi) = if k == 1 { (-6, 22) } else { (-1, 17) };
                glf(s, "glVertex2f", &[v8(&mut r, lo, hi), v8(&mut r, lo, hi)]);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // what smooth colors do with lighting, as polygon outlines, antialiased lines (with and
    // without an alpha buffer) and on a rectangle fogged across under a rotation
    let setups = [
        ("gl glEnable 0xb50\ngl glEnable 0x4000", 4, 3),
        ("gl glPolygonMode 0x408 0x1b01", 4, 3),
        ("gl glEnable 0xb20", 1, 2),
        ("gl glEnable 0xb20", 1, 2),
        ("gl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glTranslatef 8 8 0\ngl glRotatef 20 0 1 0", 0, 0),
    ];
    for (k, (setup, mode, count)) in setups.into_iter().enumerate() {
        let context = if k == 3 { "glctx 32 choose 0x38 0 32 8 0 32 8\n".to_string() } else { ctx(32) };
        writeln!(s, "case interp-open-{k}\n{context}{ORTHO}gl glDisable 0xbd0\n{setup}");
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4000");
            if count == 0 {
                color(s, &mut r);
                glf(s, "glRectf", &[v8(&mut r, -2, 0), v8(&mut r, -7, 0), v8(&mut r, 1, 3), v8(&mut r, 1, 7)]);
            } else {
                writeln!(s, "gl glBegin {mode}");
                for _ in 0..count {
                    color(s, &mut r);
                    if k == 0 {
                        glf(s, "glNormal3f", &[r.range(-8, 8) as f64 / 8.0, r.range(-8, 8) as f64 / 8.0, 1.0]);
                    }
                    glf(s, "glVertex2f", &[v8(&mut r, -1, 17), v8(&mut r, -1, 17)]);
                }
                writeln!(s, "gl glEnd");
            }
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // wide lines whose first end lies on one plane and past the one across it (prim-6's x-major
    // line from (-0.5, 16)): x-major on the top or bottom, past the left or right; y-major on
    // the left or right, past the bottom or top (widths 2, 3, 4)
    for k in 0..3 {
        writeln!(s, "case line-wide-{}\n{}{ORTHO}gl glDisable 0xbd0\ngl glColor4ub 255 255 255 255", k + 17, ctx(32));
        glf(s, "glLineWidth", &[[2.0, 3.0, 4.0][k]]);
        for i in 0..16u32 {
            let out = v8(&mut r, 1, 3);
            let (run, rise) = (v8(&mut r, 6, 15), v8(&mut r, 1, 5));
            let (on_hi, past_hi) = (i % 2 == 1, (i / 2) % 2 == 1);
            let (a, b) = if (i / 4) % 2 == 0 {
                // x-major: on the bottom or top, past the left or right
                let (x, y) = (if past_hi { 16.0 + out } else { -out }, if on_hi { 16.0 } else { 0.0 });
                ((x, y), (x + if past_hi { -run } else { run }, y + if on_hi { -rise } else { rise }))
            } else {
                // y-major: on the left or right, past the bottom or top
                let (x, y) = (if on_hi { 16.0 } else { 0.0 }, if past_hi { 16.0 + out } else { -out });
                ((x, y), (x + if on_hi { -rise } else { rise }, y + if past_hi { -run } else { run }))
            };
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            let (a, b) = if i >= 8 { (b, a) } else { (a, b) };
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased lines blended (SRC_ALPHA, ONE_MINUS_SRC_ALPHA) into a buffer without alpha,
    // white over black, one at a time: x-major drawn right, left; y-major up, down; then over
    // a gray and in a color
    for k in 0..6 {
        writeln!(s, "case line-smooth-blend-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303", ctx(32));
        if k == 4 {
            writeln!(s, "gl glClearColor 0.4 0.6 0.2 1");
        }
        for i in 0..12u32 {
            let (run, rise) = (v8(&mut r, 5, 14), v8(&mut r, -4, 4) * if i % 3 == 0 { 0.25 } else { 1.0 });
            let (x0, y0) = (v8(&mut r, 1, 15 - run as i64), v8(&mut r, 5, 11));
            let (a, b) = match k % 4 {
                0 => ((x0, y0), (x0 + run, y0 + rise)),
                1 => ((x0 + run, y0 + rise), (x0, y0)),
                2 => ((y0, x0), (y0 + rise, x0 + run)),
                _ => ((y0 + rise, x0 + run), (y0, x0)),
            };
            let (a, b) = if k >= 4 && i % 2 == 1 { (b, a) } else { (a, b) };
            writeln!(s, "gl glClear 0x4000");
            if k == 5 {
                color(s, &mut r);
            } else {
                writeln!(s, "gl glColor4ub 255 255 255 255");
            }
            writeln!(s, "gl glBegin 1");
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // flat-shaded lines (line-color-4 drew them a step apart from smooth ones): one geometry
    // of 12 single lines, all white, then each end its own color, then smooth and white
    let lines: Vec<[f64; 4]> = (0..12).map(|_| [v8(&mut r, 1, 15), v8(&mut r, 1, 15), v8(&mut r, 1, 15), v8(&mut r, 1, 15)]).collect();
    for k in 0..3 {
        writeln!(s, "case line-flat-{k}\n{}{ORTHO}gl glDisable 0xbd0", ctx(32));
        if k < 2 {
            writeln!(s, "gl glShadeModel 0x1d00");
        }
        for l in &lines {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
            for e in 0..2 {
                if k == 1 {
                    color(s, &mut r);
                } else {
                    writeln!(s, "gl glColor4ub 255 255 255 255");
                }
                glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // flat-shaded lines (GL.RASTER.LINE.FLAT measured on line-flat-0's lines): ends a little
    // either side of the pixel boundaries and centers (3); ends past the window (4); under a
    // viewport (5); on 16- and 24-bit DIBs (6, 7); depth tested (8); blended (9); scissored
    // (10); in strips and loops of colors (11); 2 wide (12); depths that differ (13)
    let near: Vec<[f64; 4]> = (0..24)
        .map(|_| {
            let mut v = || r.range(2, 28) as f64 / 2.0 + [-1.0 / 128.0, 0.0, 1.0 / 128.0, -1.0 / 256.0, 1.0 / 256.0][r.range(0, 4) as usize];
            [v(), v(), v(), v()]
        })
        .collect();
    let past: Vec<[f64; 4]> = (0..24).map(|_| [v8(&mut r, -6, 22), v8(&mut r, -6, 22), v8(&mut r, -6, 22), v8(&mut r, -6, 22)]).collect();
    for k in 3..14 {
        let bpp = match k {
            6 => 16,
            7 => 24,
            _ => 32,
        };
        writeln!(s, "case line-flat-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(bpp));
        match k {
            5 => writeln!(s, "gl glViewport 3 2 10 11"),
            8 => writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x201\ngl glClearDepth 1"),
            9 => writeln!(s, "gl glEnable 0xbe2\ngl glBlendFunc 1 0"),
            10 => writeln!(s, "gl glEnable 0xc11\ngl glScissor 0 0 16 16"),
            12 => writeln!(s, "gl glLineWidth 2"),
            _ => {}
        }
        let set: &[[f64; 4]] = match k {
            3 => &near,
            4 => &past,
            _ => &lines,
        };
        for (n, l) in set.iter().enumerate() {
            writeln!(s, "gl glClear {}", if k == 8 { 0x4100 } else { 0x4000 });
            if k == 11 {
                // a strip or a loop through this line's ends and the next one's
                let m = &set[(n + 1) % set.len()];
                writeln!(s, "gl glBegin {}", 2 + n % 2);
                for p in [[l[0], l[1]], [l[2], l[3]], [m[0], m[1]]] {
                    color(s, &mut r);
                    glf(s, "glVertex2f", &p);
                }
            } else {
                writeln!(s, "gl glBegin 1");
                for e in 0..2 {
                    color(s, &mut r);
                    if k == 13 {
                        glf(s, "glVertex3f", &[l[2 * e], l[2 * e + 1], [-0.75, 0.5][e]]);
                    } else {
                        glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
                    }
                }
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // flat-shaded lines whose ends lie exactly between two 16ths (GL.RASTER.LINE.FLAT's 28.4
    // rounding of a half: up, down, to even?), every one in x and y
    writeln!(s, "case line-flat-14\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(32));
    for _ in 0..24 {
        writeln!(s, "gl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255");
        let mut v = || (r.range(16, 239) as f64 * 2.0 + 1.0) / 32.0;
        let l = [v(), v(), v(), v()];
        glf(s, "glVertex2f", &[l[0], l[1]]);
        glf(s, "glVertex2f", &[l[2], l[3]]);
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    // flat-shaded wide lines past line-flat-12's width 2: widths 3, 4, 5, 7 (15); 2 wide past
    // the window (16); 2 wide depth tested (17)
    for k in 15..18 {
        writeln!(s, "case line-flat-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(32));
        if k == 17 {
            writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x201");
        }
        let set: &[[f64; 4]] = if k == 16 { &past } else { &lines };
        for (n, l) in set.iter().enumerate() {
            let w = if k == 15 { [3, 4, 5, 7][n % 4] } else { 2 };
            writeln!(s, "gl glLineWidth {w}\ngl glClear 0x4100\ngl glBegin 1");
            for e in 0..2 {
                color(s, &mut r);
                glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // antialiased wide lines past GL.RASTER.LINE.SMOOTH.WIDE (raster-mode-6, 7): widths under
    // 1.5 and past 3 (0); blended in colors over gray (1); into a buffer with alpha (2); in
    // colors that differ (3)
    let wides = [0.5, 1.25, 3.5, 4.0, 5.75, 7.0, 10.0, 12.5];
    for k in 0..4 {
        let alpha = if k == 2 { 8 } else { 0 };
        writeln!(s, "case line-aa-wide-{k}\nglctx 32 choose 0x38 0 32 {alpha} 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20");
        if k == 1 {
            writeln!(s, "gl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.4 0.5 0.6 1");
        }
        for n in 0..8 {
            let w = match k {
                0 => wides[n],
                _ => [1.5, 2.0, 2.5, 3.0][n % 4],
            };
            writeln!(s, "gl glLineWidth {}\ngl glClear 0x4000\ngl glBegin 1", f(w));
            let l = [v8(&mut r, 2, 14), v8(&mut r, 2, 14), v8(&mut r, 2, 14), v8(&mut r, 2, 14)];
            for e in 0..2 {
                // (one color a line but in 3)
                if k == 0 || k == 2 {
                    writeln!(s, "gl glColor4ub 255 255 255 255");
                } else if k == 3 || e == 0 {
                    color(s, &mut r);
                }
                glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // (appended last: each case takes from `r` after the ones before it)
    // flat-shaded lines with every end exactly where GDI's pixel changes, x = n + 15/32 and
    // 16 - y = n + 15/32 (18); wide ones blended and scissored (19)
    writeln!(s, "case line-flat-18\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(32));
    for _ in 0..32 {
        writeln!(s, "gl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255");
        let mut v = || r.range(1, 14) as f64 + 15.0 / 32.0;
        let l = [v(), 16.0 - v(), v(), 16.0 - v()];
        glf(s, "glVertex2f", &[l[0], l[1]]);
        glf(s, "glVertex2f", &[l[2], l[3]]);
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    writeln!(s, "case line-flat-19\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(32));
    for (n, l) in lines.iter().enumerate() {
        let on = if n % 2 == 0 { "gl glEnable 0xbe2\ngl glBlendFunc 1 0\ngl glDisable 0xc11" } else { "gl glDisable 0xbe2\ngl glEnable 0xc11\ngl glScissor 0 0 16 16" };
        writeln!(s, "{on}\ngl glLineWidth {}\ngl glClear 0x4000\ngl glBegin 1", [2, 3, 4][n % 3]);
        for e in 0..2 {
            color(s, &mut r);
            glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
        }
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    // antialiased wide lines into alpha, for which faint pixels Windows draws (line-aa-wide-4)
    writeln!(s, "case line-aa-wide-4\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glColor4ub 255 255 255 255");
    for n in 0..32 {
        writeln!(s, "gl glLineWidth {}\ngl glClear 0x4000\ngl glBegin 1", f([2.5, 3.5, 5.75, 2.0][n % 4]));
        let l = [v8(&mut r, 1, 15), v8(&mut r, 1, 15), v8(&mut r, 1, 15), v8(&mut r, 1, 15)];
        glf(s, "glVertex2f", &[l[0], l[1]]);
        glf(s, "glVertex2f", &[l[2], l[3]]);
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    // color indices drawn in RGBA mode into a buffer with alpha (GL.DRAW.INDEX): bytes, shorts
    // and floats, shifted and offset, over a cleared color: each through the initial one-entry
    // maps
    writeln!(s, "case draw-index-0\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glClearColor 0.2 0.4 0.6 0.8\ngl glClear 0x4000");
    for (k, (ty, size)) in [(0x1401, 1), (0x1403, 2), (0x1406, 4), (0x1400, 1)].into_iter().enumerate() {
        if k == 2 {
            writeln!(s, "gl glPixelTransferi 0xd12 3\ngl glPixelTransferi 0xd13 7");
        }
        writeln!(s, "gl glRasterPos2i 0 {}\ngldraw 16 3 0x1900 {} {} {}\nglerr", 4 * k, h(ty), 90 + k, 48 * size);
    }
    writeln!(s, "glread 0 0 16 16 {} {} 1024\nglerr\nend", h(RGBA), h(UNSIGNED_BYTE));
    // flat-shaded lines under the other per-fragment states (GL.RASTER.MODE): a stencil test,
    // an alpha test, a logic op, fog, a color mask, a depth test with blending, all passing
    // everything; and under a viewport smaller than the window, cut
    let states = [
        "gl glEnable 0xb90\ngl glStencilFunc 0x207 0 255",
        "gl glEnable 0xbc0\ngl glAlphaFunc 0x207 0",
        "gl glEnable 0xbf2\ngl glLogicOp 0x1503",
        "gl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 -1\ngl glFogf 0xb64 1",
        "gl glColorMask 1 1 0 1",
        "gl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glEnable 0xbe2\ngl glBlendFunc 1 0",
        "gl glViewport 2 3 11 9",
    ];
    for (k, on) in states.iter().enumerate() {
        writeln!(s, "case line-flat-{}\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00\n{on}", 20 + k, ctx(32));
        let set: &[[f64; 4]] = if k == 6 { &past } else { &lines };
        for l in set.iter().take(12) {
            writeln!(s, "gl glClear 0x4500\ngl glBegin 1");
            for e in 0..2 {
                color(s, &mut r);
                glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // stippled lines (GL.RASTER.MODE): the state read back (0); 1-pixel lines, segments and
    // strips, of patterns and factors, smooth (1), flat (2), wide (3)
    writeln!(s, "case line-stipple-0\n{}{ORTHO}", ctx(32));
    for (f, p) in [(3, 0xf0f3), (300, 0x12345), (0, 0x8001), (-5, 0xaaaa), (256, 0x0f0f)] {
        writeln!(s, "gl glLineStipple {f} {p}\nglget i 0xb25 1\nglget i 0xb26 1\nglerr");
    }
    writeln!(s, "end");
    for k in 1..4 {
        writeln!(s, "case line-stipple-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb24", ctx(32));
        if k == 2 {
            writeln!(s, "gl glShadeModel 0x1d00");
        }
        if k == 3 {
            writeln!(s, "gl glLineWidth 3");
        }
        for (n, l) in lines.iter().enumerate() {
            let (f, p) = [(1, 0xf0f3), (2, 0x3c5a), (3, 0x8421), (1, 0x5555)][n % 4];
            writeln!(s, "gl glLineStipple {f} {p}\ngl glClear 0x4000\ngl glBegin {}\ngl glColor4ub 255 255 255 255", [1, 3][n % 2]);
            let m = &lines[(n + 5) % lines.len()];
            for q in [[l[0], l[1]], [l[2], l[3]], [m[0], m[1]], [m[2], m[3]]] {
                glf(s, "glVertex2f", &q);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
        writeln!(s, "glerr\nend");
    }
    // stippled flat-shaded lines in colors: which end's (line-stipple-2 drew them white)
    writeln!(s, "case line-stipple-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb24\ngl glShadeModel 0x1d00\ngl glLineStipple 1 0xf3f7", ctx(32));
    for l in &lines {
        writeln!(s, "gl glClear 0x4000\ngl glBegin 1");
        for e in 0..2 {
            color(s, &mut r);
            glf(s, "glVertex2f", &[l[2 * e], l[2 * e + 1]]);
        }
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
    }
    writeln!(s, "glerr\nend");
    // flat lines from ends exactly where the pixel changes (window x or y = j + 15/32), one per
    // row or column, both ways along each axis, under viewports moved and scaled: which way
    // each tie goes, by the window coordinate or by where it is in the viewport
    writeln!(s, "case line-flat-27\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00", ctx(32));
    for (ox, oy, w) in [(0, 0, 16), (-6, -3, 16), (5, 6, 16), (0, 0, 32), (-16, -16, 32)] {
        writeln!(s, "gl glViewport {ox} {oy} {w} {w}");
        let obj = |v: f64, o: i64| (v - o as f64) * 16.0 / w as f64;
        for (axis, way) in [(0, 2.25), (0, -2.25), (1, 2.25), (1, -2.25)] {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255");
            for j in 1..15 {
                let (tie, at) = (j as f64 + 15.0 / 32.0, j as f64 + 0.25);
                for along in [tie, tie + way] {
                    let (x, y) = if axis == 0 { (along, at) } else { (at, along) };
                    glf(s, "glVertex2f", &[obj(x, ox), obj(y, oy)]);
                }
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
    }
    writeln!(s, "glerr\nend");
    // antialiased wide lines in colors (GL.RASTER.MODE, line-aa-wide-3): red from 0 to 250 over
    // a major length of 10, so each pixel's step shows, upright and leaning a little or nearly 45
    // degrees (19 in 20) either way, up and down, from ends 1/8 to 7/8 into a pixel
    writeln!(s, "case line-aa-wide-5\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20", ctx(32));
    for (n, lean) in [0.0f64, 0.25, -0.25, 1.0, -1.0].into_iter().enumerate() {
        for (k, from) in [2.125, 2.375, 2.625, 2.875].into_iter().enumerate() {
            let width = [2.0, 3.0, 2.5, 4.0][(n + k) % 4];
            let (x0, x1) = if lean.abs() < 1.0 { (7.5, 7.5 + 10.0 * lean) } else { (8.0 - 4.75 * lean, 8.0 + 4.75 * lean) };
            let (y0, y1) = if lean.abs() < 1.0 { (from, from + 10.0) } else { (from + 0.5, from + 10.5) };
            let ends = [[x0, y0], [x1, y1]];
            let ends = if k % 2 == 1 { [ends[1], ends[0]] } else { ends };
            writeln!(s, "gl glLineWidth {width}\ngl glClear 0x4000\ngl glBegin 1");
            for (e, at) in ends.iter().enumerate() {
                writeln!(s, "gl glColor4ub {} 128 {} 255", 250 * e, 250 - 250 * e);
                glf(s, "glVertex2f", at);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
        }
    }
    writeln!(s, "glerr\nend");
    // more of them (line-aa-wide-5): slopes 1/2, 3/4 and 19/20 across, widths 2 to 3, the
    // start moved across by sixteenths, up and down
    writeln!(s, "case line-aa-wide-6\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20", ctx(32));
    for (n, lean) in [0.5f64, 0.75, 0.95].into_iter().enumerate() {
        for (k, width) in [2.0, 2.5, 3.0].into_iter().enumerate() {
            for off in 0..4 {
                let x0 = 2.0 + f64::from(off) / 16.0 + (k as f64) / 4.0;
                let ends = [[x0, 2.625], [x0 + 10.0 * lean, 12.625]];
                let ends = if (n + k + off as usize) % 2 == 1 { [ends[1], ends[0]] } else { ends };
                writeln!(s, "gl glLineWidth {width}\ngl glClear 0x4000\ngl glBegin 1");
                for (e, at) in ends.iter().enumerate() {
                    writeln!(s, "gl glColor4ub {} 128 {} 255", 250 * e, 250 - 250 * e);
                    glf(s, "glVertex2f", at);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE));
            }
        }
    }
    writeln!(s, "glerr\nend");
    // antialiased rectangles of one color set outside glBegin (raster-mode-10's fourth and fifth)
    // drawn by glRectf, as a GL_QUADS quad, as its two triangles (v0 v1 v2, v2 v3 v0) and as a
    // quad with a red ramp across its corners: whether the colors come from planes, on the
    // executor's format 5 and on a chosen one with alpha
    for (n, ctx_line) in (14..).zip(["glctx 32 5", "glctx 32 choose 0x38 0 32 8 0 32 8"]) {
        writeln!(s, "case raster-mode-{n}\n{ctx_line}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41");
        for r in [[2.125f64, 2.125, 13.875, 4.375], [9.375, 3.125, 14.625, 13.875]] {
            let v = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]];
            writeln!(s, "gl glColor4ub 200 100 50 255\ngl glClear 0x4000");
            glf(s, "glRectf", &r);
            read_all(s);
            writeln!(s, "gl glClear 0x4000\ngl glBegin 7");
            for p in &v {
                glf(s, "glVertex2f", p);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4");
            for p in [v[0], v[1], v[2], v[2], v[3], v[0]] {
                glf(s, "glVertex2f", &p);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
            writeln!(s, "gl glClear 0x4000\ngl glBegin 7");
            for (i, p) in v.iter().enumerate() {
                writeln!(s, "gl glColor4ub {} 100 50 255", if i < 2 { 255 } else { 0 });
                glf(s, "glVertex2f", p);
            }
            writeln!(s, "gl glEnd");
            read_all(s);
        }
        writeln!(s, "glerr\nend");
    }
    // a polygon drawn as its edges whose corners' depths differ (GL.RASTER.INTERPOLATE), and a
    // stippled antialiased line (GL.RASTER.MODE): still open, measured to settle them
    writeln!(s, "case interp-open-5\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    writeln!(s, "case raster-open-1\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb24\ngl glLineStipple 1 0xf0f0\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a polygon drawn as its edges whose corners' depths differ under a depth test
    // (GL.RASTER.INTERPOLATE), and a stippled antialiased line strip, its counter carried on
    // (GL.RASTER.MODE): open, measured to settle them
    writeln!(s, "case interp-open-6\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonMode 0x408 0x1b01\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    writeln!(s, "case raster-open-2\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb24\ngl glLineStipple 2 0xc3a5\ngl glClear 0x4000\ngl glBegin 3\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 9.25 6.5\ngl glVertex2f 14.5 13.75\ngl glVertex2f 3.125 12.5\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a smooth triangle the near plane cuts, under a depth test (GL.RASTER.INTERPOLATE), and a
    // stippled antialiased line past the window's right edge (GL.RASTER.MODE): open
    writeln!(s, "case interp-open-7\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 1.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0.5\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    writeln!(s, "case raster-open-3\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb24\ngl glLineStipple 1 0x3c3c\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f -1.5 3.25\ngl glVertex2f 17.5 12.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a rectangle fogged across: tilted about x so its corners' eye depths differ, linear fog
    // (GL.RASTER.INTERPOLATE): open
    writeln!(s, "case interp-open-8\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 -2\ngl glFogf 0xb64 2\ngl glRotatef 40 1 0 0\ngl glClear 0x4000\ngl glColor4ub 255 255 255 255\ngl glRectf 3 2 13 9", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a lit triangle whose corners' colors differ (GL_COLOR_MATERIAL, GL_LIGHT0), smooth
    // shaded (GL.RASTER.INTERPOLATE): open
    writeln!(s, "case interp-open-9\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glEnable 0xb57\ngl glClear 0x4000\ngl glBegin 4\ngl glNormal3f 0 0 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 2 2\ngl glColor4ub 0 255 0 255\ngl glVertex2f 13 4\ngl glColor4ub 0 0 255 255\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a flat-shaded line the far plane cuts (w = 1, the second end past it), no depth test
    // (GL.RASTER.INTERPOLATE): open
    writeln!(s, "case interp-open-10\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 200 100 50 255\ngl glVertex3f 2 2 0\ngl glVertex3f 14 10 -3\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a rectangle fogged across at one window depth: tilted about x, glDepthRange 0 0
    // (GL.RASTER.INTERPOLATE): open
    writeln!(s, "case interp-open-11\n{}{ORTHO}gl glDisable 0xbd0\ngl glDepthRange 0 0\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glRotatef 5 1 0 0\ngl glClear 0x4000\ngl glColor4ub 255 255 255 255\ngl glRectf 3 2 13 9", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line flat-shaded, its ends' colors differing (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glShadeModel 0x1d00\ngl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a stippled polygon outline (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-5\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb24\ngl glLineStipple 1 0x3c3c\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 2 2\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle under GL_CULL_FACE, front facing (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-6\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb44\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 2 2\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a rectangle fogged across, tilted about x inside the view volume: drawn as a polygon
    // (GL.RASTER.INTERPOLATE.COLOR)
    writeln!(s, "case interp-open-12\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glRotatef 5 1 0 0\ngl glClear 0x4000\ngl glColor4ub 255 255 255 255\ngl glRectf 3 2 13 9", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line fogged (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-7\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex3f 1.5 2.25 -0.2\ngl glVertex3f 14.5 11.75 -0.7\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle under a depth test, its corners' depths differing (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-8\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 2 2 0.5\ngl glVertex3f 13 4 -0.25\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a stippled polygon outline under a depth test, its corners' depths differing (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-9\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb24\ngl glLineStipple 1 0x3c3c\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 2 2 0.5\ngl glVertex3f 13 4 -0.25\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // two stippled polygon outlines in one glBegin: the counter from 0 at each
    // (GL.RASTER.LINE.STIPPLE)
    writeln!(s, "case raster-open-10\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb24\ngl glLineStipple 2 0x5a5a\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1 1\ngl glVertex2f 7 2\ngl glVertex2f 3 7\ngl glVertex2f 8 8\ngl glVertex2f 15 9\ngl glVertex2f 10 15\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle under a depth test: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-11\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 2 2 0.5\ngl glVertex3f 13 4 -0.25\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased line under a depth test: its colors and the depths it writes
    // (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-12\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex3f 1.5 2.25 0.5\ngl glVertex3f 14.5 11.75 -0.25\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glread 0 0 16 16 0x1902 0x1405 1024\nglerr\nend");
    // an antialiased triangle fogged across (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-13\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 2 2 -0.1\ngl glVertex3f 13 4 -0.8\ngl glVertex3f 6 13 -0.4\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line under a scissor box (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-14\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xc11\ngl glScissor 4 3 7 6\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line under a stencil test (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-15\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb90\ngl glStencilFunc 0x207 0 255\ngl glStencilOp 0x1e00 0x1e00 0x1e02\ngl glClear 0x4400\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line under a color mask (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-16\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glColorMask 1 0 1 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle lit (GL_LIGHT0, GL_COLOR_MATERIAL) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-17\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glEnable 0xb57\ngl glClear 0x4000\ngl glBegin 4\ngl glNormal3f 0 0 1\ngl glColor4ub 200 0 0 255\ngl glVertex2f 2 2\ngl glColor4ub 0 200 0 255\ngl glVertex2f 13 4\ngl glColor4ub 0 0 200 255\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle alpha tested, into a buffer with alpha (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-18\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xbc0\ngl glAlphaFunc 0x204 0.5\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 2 2\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd");
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle the view volume cuts (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-19\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex2f -3 2\ngl glColor4ub 0 255 0 255\ngl glVertex2f 13 4\ngl glColor4ub 0 0 255 255\ngl glVertex2f 6 19\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line alpha tested, into a buffer with alpha (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-20\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xbc0\ngl glAlphaFunc 0x204 0.5\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd");
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line under a logic op (XOR over a gray clear) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-21\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xbf2\ngl glLogicOp 0x1506\ngl glClearColor 0.5 0.5 0.5 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line blended (ONE, ONE) over a gray clear (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-22\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 1 1\ngl glClearColor 0.25 0.25 0.25 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 255 255 255\ngl glVertex2f 1.5 2.25\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle with GL_TEXTURE_2D on and no image (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-23\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xde1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 2 2\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle under GL_POLYGON_STIPPLE (the pattern all set) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-24\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb42\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex2f 2 2\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased smooth triangle into a 16-bit DIB (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-25\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(16));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased triangle blended (SRC_ALPHA, ONE) over a gray clear (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-26\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 1\ngl glClearColor 0.25 0.25 0.25 1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a polygon outline under a perspective projection (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-27\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glFrustum -1 1 -1 1 1 10\ngl glMatrixMode 0x1700\ngl glLoadIdentity\ngl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f -0.8 -0.7 -2\ngl glVertex3f 1.5 -0.3 -3\ngl glVertex3f 0.2 1.2 -2.5\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a polygon outline fogged (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-28\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a filled polygon offset under a depth test, its depths differing (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-29\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased smooth triangle into a 16-bit DIB, dithered (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-30\n{}{ORTHO}gl glEnable 0xb41\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd", ctx(16));
    read_all(s);
    writeln!(s, "glerr\nend");
    // a filled polygon offset under a depth test, its depths differing: the depths it
    // writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-31\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased smooth line into a 16-bit DIB (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-32\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(16));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased line lit (GL_LIGHT0, GL_COLOR_MATERIAL) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-33\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glEnable 0xb57\ngl glClear 0x4000\ngl glNormal3f 0 0 1\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd", ctx(32));
    read_all(s);
    writeln!(s, "glerr\nend");
    // an antialiased smooth line into a 16-bit DIB, dithered (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-34\n{}{ORTHO}gl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // an antialiased smooth line into a 16-bit DIB, blended (SRC_ALPHA, ONE_MINUS_SRC_ALPHA) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-35\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // an antialiased smooth triangle under a 16-bit depth buffer, depth tested: its colors and depths (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-36\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb71\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", "glctx 32 choose 0x38 0 32 0 0 16 8\n");
    writeln!(s, "glerr\nend");
    // an antialiased smooth triangle offset (factor 1, units 2) under a depth test: its colors and depths (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-37\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor 1, units 0) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-38\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 1 0\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor 3, units 0) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-39\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 3 0\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor 0.5, units -4) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-40\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 0.5 -4\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor 2, units 1) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-41\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 2 1\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 3 1 -0.75\ngl glVertex3f 12 6 0.25\ngl glVertex3f 5 15 0.875\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor 1, units 2) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-42\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", "glctx 32 choose 0x38 0 32 0 0 16 8\n");
    writeln!(s, "glerr\nend");
    // a filled triangle offset (factor -1, units 0) under a depth test, its depths differing: the depths it writes (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-43\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glPolygonOffset -1 0\ngl glEnable 0x8037\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 255 255 255\ngl glVertex3f 3 1 -0.75\ngl glVertex3f 12 6 0.25\ngl glVertex3f 5 15 0.875\ngl glEnd\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased smooth line with GL_TEXTURE_2D on and no image (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-44\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glEnable 0xde1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased line in color index (indices 9 to 40) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-45\n{}{ORTHO}gl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 1\ngl glIndexi 9\ngl glVertex2f 1.5 2.25\ngl glIndexi 40\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1900 0x1401 256", "glctx 8 29\n");
    writeln!(s, "glerr\nend");
    // an antialiased triangle in color index (indices 9, 40, 25) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-46\n{}{ORTHO}gl glEnable 0xb41\ngl glClear 0x4000\ngl glBegin 4\ngl glIndexi 9\ngl glVertex2f 2 2\ngl glIndexi 40\ngl glVertex2f 13 4\ngl glIndexi 25\ngl glVertex2f 6 13\ngl glEnd\nglread 0 0 16 16 0x1900 0x1401 256", "glctx 8 29\n");
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) lit (GL_LIGHT0, GL_COLOR_MATERIAL) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-47\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glEnable 0xb57\ngl glClear 0x4000\ngl glNormal3f 0 0 1\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) 3 pixels wide (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-48\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) antialiased (GL_LINE_SMOOTH) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-49\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) offset (GL_POLYGON_OFFSET_LINE, factor 1, units 2) under a depth test: its colors and depths (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-50\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x2a02\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased line 2.5 pixels wide into a 16-bit DIB, undithered: the general path (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-51\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glLineWidth 2.5\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 255\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) 3 pixels wide, flat-shaded (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-52\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glShadeModel 0x1d00\ngl glLineWidth 3\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle outlined (GL_LINE) antialiased, cut by the view volume (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-53\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb20\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f -3 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle as its corners (GL_POINT) offset (GL_POLYGON_OFFSET_POINT, factor 1, units 2) under a depth test (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-54\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b00\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x2a01\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a quadrilateral (GL_POLYGON) outlined and offset (GL_POLYGON_OFFSET_LINE, factor 1, units 2) under a depth test (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-55\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x2a02\ngl glClear 0x4100\ngl glBegin 9\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 3 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 12 13 0\ngl glColor4ub 255 255 0 255\ngl glVertex3f 3 11 0.25\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // an antialiased triangle in color index, fogged (linear 0..1, fog index 3) (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-56\n{}{ORTHO}gl glEnable 0xb41\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glFogf 0xb61 3\ngl glClear 0x4000\ngl glBegin 4\ngl glIndexi 9\ngl glVertex3f 2 2 0.5\ngl glIndexi 40\ngl glVertex3f 13 4 -0.25\ngl glIndexi 25\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1900 0x1401 256", "glctx 8 29\n");
    writeln!(s, "glerr\nend");
    // an antialiased line 2.5 pixels wide into a 16-bit DIB, blended (SRC_ALPHA, ONE_MINUS_SRC_ALPHA), undithered (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-57\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb20\ngl glLineWidth 2.5\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glBegin 1\ngl glColor4ub 255 0 0 128\ngl glVertex2f 1.5 2.25\ngl glColor4ub 0 0 255 255\ngl glVertex2f 14.5 11.75\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // an antialiased triangle into a 16-bit DIB, blended (SRC_ALPHA, ONE_MINUS_SRC_ALPHA) and dithered (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-58\n{}{ORTHO}gl glEnable 0xb41\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // an antialiased triangle into a 16-bit DIB, blended (SRC_ALPHA, ONE_MINUS_SRC_ALPHA), undithered (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-59\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb41\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glClearColor 0.25 0.5 0.75 1\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(16));
    writeln!(s, "glerr\nend");
    // a triangle outlined 3 pixels wide and offset (GL_POLYGON_OFFSET_LINE) under a depth test (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-60\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3\ngl glEnable 0xb71\ngl glPolygonOffset 1 2\ngl glEnable 0x2a02\ngl glClear 0x4100\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f 2 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglread 0 0 16 16 0x1902 0x1405 1024", ctx(32));
    writeln!(s, "glerr\nend");
    // a triangle outlined 3 pixels wide, cut by the view volume (GL.RASTER.MODE): open
    writeln!(s, "case raster-open-61\n{}{ORTHO}gl glDisable 0xbd0\ngl glPolygonMode 0x408 0x1b01\ngl glLineWidth 3\ngl glClear 0x4000\ngl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f -3 2 0.5\ngl glColor4ub 0 255 0 255\ngl glVertex3f 13 4 -0.25\ngl glColor4ub 0 0 255 255\ngl glVertex3f 6 13 0\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024", ctx(32));
    writeln!(s, "glerr\nend");
    textures(s);
    gaps(s);
}

/// texturing (GL 1.1 3.8): objects, images and their formats, queries, environments, filters,
/// wrapping, mipmaps, sub-images and copies, coordinate generation, the texture matrix, and
/// textured primitives (GL.RASTER.MODE: open)
fn textures(s: &mut String) {
    const T2: u32 = 0xde1;
    let head = |s: &mut String, name: &str, bpp: u32| {
        writeln!(s, "case {name}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
    };
    let q = |s: &mut String, kind: &str, args: &str, ty: &str, n: u32| {
        writeln!(s, "gltexq {kind} {args} {ty} {n}");
    };
    // (a textured quad over the window, texture coordinates (s0, t0) .. (s1, t1))
    let quad = |s: &mut String, s0: f32, t0: f32, s1: f32, t1: f32| {
        writeln!(s, "gl glColor4ub 255 255 255 255\ngl glBegin 7\ngl glTexCoord2f {s0} {t0}\ngl glVertex2f 0 0\ngl glTexCoord2f {s1} {t0}\ngl glVertex2f 16 0\ngl glTexCoord2f {s1} {t1}\ngl glVertex2f 16 16\ngl glTexCoord2f {s0} {t1}\ngl glVertex2f 0 16\ngl glEnd");
    };
    let params = [0x2800u32, 0x2801, 0x2802, 0x2803, 0x8066, 0x8067];
    let levels = [0x1000u32, 0x1001, 0x1003, 0x1005, 0x805c, 0x805d, 0x805e, 0x805f, 0x8060, 0x8061];
    // objects: names, binding, residency, priorities, deletion, and the defaults
    head(s, "tex-open-0", 32);
    writeln!(s, "glgentex 3\nglistex 1\ngl glBindTexture {T2} 1\nglistex 1\nglerr");
    for p in params {
        q(s, "param", &format!("{T2} {p}"), "i", 1);
    }
    q(s, "param", &format!("{T2} 4100"), "f", 4);
    writeln!(s, "glresident 1 2 3\nglprio 0.25 1 2\nglresident 1 2\nglerr");
    q(s, "param", &format!("{T2} 32870"), "f", 1);
    writeln!(s, "gldeltex 1 7\nglistex 1\nglistex 2\nglgentex 2\ngl glBindTexture 0xde0 2\ngl glBindTexture {T2} 2\nglerr\nend");
    // images in every internal format, read back, and their level parameters
    let formats = [1u32, 2, 3, 4, 0x1906, 0x1909, 0x190a, 0x8049, 0x803b, 0x803c, 0x803f, 0x8040, 0x8041, 0x8042, 0x8045, 0x8048, 0x804b, 0x804c, 0x804f, 0x8050, 0x8051, 0x8052, 0x8053, 0x8054, 0x8055, 0x8056, 0x8057, 0x8058, 0x8059, 0x805a, 0x805b, 0x2a10, 0x80e5];
    for (k, f) in formats.iter().enumerate() {
        head(s, &format!("tex-open-format-{k}"), 32);
        writeln!(s, "gltex {T2} 0 {f} 4 2 0 0x1908 0x1401 {} 32\nglerr", k.wrapping_add(0x100));
        for l in levels {
            q(s, "level", &format!("{T2} 0 {l}"), "i", 1);
        }
        writeln!(s, "glteximg {T2} 0 0x1908 0x1401 32\nglteximg {T2} 0 0x1909 0x1406 32\nglerr\nend");
    }
    // source formats and types into RGBA8
    let sources = [(0x1908u32, 0x1401u32), (0x1907, 0x1401), (0x1909, 0x1401), (0x190a, 0x1401), (0x1906, 0x1401), (0x1903, 0x1401), (0x1904, 0x1401), (0x1905, 0x1401), (0x80e1, 0x1401), (0x80e0, 0x1401), (0x1908, 0x1400), (0x1908, 0x1402), (0x1908, 0x1403), (0x1908, 0x1404), (0x1908, 0x1405), (0x1908, 0x1406), (0x1900, 0x1401), (0x1908, 0x1a00)];
    for (k, (f, t)) in sources.iter().enumerate() {
        head(s, &format!("tex-open-source-{k}"), 32);
        writeln!(s, "gltex {T2} 0 0x8058 4 2 0 {f} {t} {} 256\nglerr\nglteximg {T2} 0 0x1908 0x1401 32\nglerr\nend", k.wrapping_add(0x200));
    }
    // errors: sizes not 2^n + 2 border, borders, levels, targets, formats, sub-images out of range
    head(s, "tex-open-errors", 32);
    for (tg, lv, ifmt, w, h, b, f, t) in [(T2, 0, 4, 3, 2, 0, 0x1908, 0x1401), (T2, 0, 4, 4, 2, 2, 0x1908, 0x1401), (T2, -1, 4, 4, 2, 0, 0x1908, 0x1401), (T2, 11, 4, 4, 2, 0, 0x1908, 0x1401), (0xde2, 0, 4, 4, 2, 0, 0x1908, 0x1401), (T2, 0, 5, 4, 2, 0, 0x1908, 0x1401), (T2, 0, 4, 4, 2, 0, 0x1902, 0x1401), (T2, 0, 4, 4, 2, 0, 0x1908, 0x1a00), (T2, 0, 4, 2048, 1, 0, 0x1908, 0x1401), (T2, 0, 4, 6, 6, 1, 0x1908, 0x1401), (T2, 0, 4, 0, 0, 0, 0x1908, 0x1401)] {
        writeln!(s, "gltex {tg} {lv} {ifmt} {w} {h} {b} {f} {t} 1 128\nglerr");
    }
    writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 1 64\ngltexsub {T2} 0 2 2 4 4 0x1908 0x1401 2 64\nglerr\ngltexsub {T2} 0 -1 0 1 1 0x1908 0x1401 2 4\nglerr\ngltexsub {T2} 1 0 0 1 1 0x1908 0x1401 2 4\nglerr");
    writeln!(s, "gl glTexParameteri {T2} 10241 7\nglerr\ngl glTexEnvi 8960 8704 7\nglerr\ngl glTexGeni 8192 9472 7\nglerr\ngl glBegin 7\ngltex {T2} 0 4 4 4 0 0x1908 0x1401 1 64\nglerr\ngl glEnd\nglerr\nend");
    // sub-images and copies from the color buffer, read back
    head(s, "tex-open-sub", 32);
    writeln!(s, "gltex {T2} 0 0x8058 8 8 0 0x1908 0x1401 3 256\ngltexsub {T2} 0 2 3 4 2 0x1908 0x1401 4 32\nglteximg {T2} 0 0x1908 0x1401 256\nglerr\nend");
    for (k, ifmt) in [0x8058u32, 0x1907, 0x1909, 0x8056].iter().enumerate() {
        head(s, &format!("tex-open-copy-{k}"), 32);
        writeln!(s, "gldraw 16 16 0x1908 0x1401 {} 1024\ngl glCopyTexImage2D {T2} 0 {ifmt} 1 2 8 4 0\nglerr\nglteximg {T2} 0 0x1908 0x1401 128", k.wrapping_add(0x300));
        writeln!(s, "gl glCopyTexSubImage2D {T2} 0 2 1 9 5 4 2\nglerr\nglteximg {T2} 0 0x1908 0x1401 128\ngl glCopyTexImage1D 0xde0 0 {ifmt} 3 4 8 0\nglerr\nglteximg 0xde0 0 0x1908 0x1401 32\ngl glCopyTexSubImage1D 0xde0 0 1 0 0 4\nglteximg 0xde0 0 0x1908 0x1401 32\nglerr\nend");
    }
    // the environments over a smooth background color, each base format
    for (k, (mode, ifmt)) in [(0x2100u32, 4u32), (0x2100, 3), (0x2100, 0x1906), (0x2100, 0x1909), (0x2100, 0x190a), (0x2100, 0x8049), (0x2101, 4), (0x2101, 3), (0x0be2, 4), (0x0be2, 3), (0x0be2, 0x1909), (0x0be2, 0x8049), (0x1e01, 4), (0x1e01, 3), (0x1e01, 0x1906), (0x1e01, 0x8049)].iter().enumerate() {
        head(s, &format!("tex-open-env-{k}"), 32);
        writeln!(s, "gltex {T2} 0 {ifmt} 4 4 0 0x1908 0x1401 {} 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexParameteri {T2} 10240 9728\ngl glTexEnvi 8960 8704 {mode}\ngltexv env 8960 8705 f 0.25 0.5 0.75 0.5\ngl glEnable {T2}\ngl glClear 0x4000", k.wrapping_add(0x400));
        writeln!(s, "gl glBegin 7\ngl glColor4ub 255 0 0 200\ngl glTexCoord2f 0 0\ngl glVertex2f 0 0\ngl glColor4ub 0 255 0 100\ngl glTexCoord2f 1 0\ngl glVertex2f 16 0\ngl glColor4ub 0 0 255 255\ngl glTexCoord2f 1 1\ngl glVertex2f 16 16\ngl glColor4ub 255 255 255 50\ngl glTexCoord2f 0 1\ngl glVertex2f 0 16\ngl glEnd");
        q(s, "env", "8960 8704", "i", 1);
        q(s, "env", "8960 8705", "f", 4);
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    // filters: magnified and minified, each min filter, with a full mipmap chain
    for (k, (min, mag, s1)) in [(9728u32, 9728u32, 1.0f32), (9729, 9729, 1.0), (9728, 9729, 0.37), (9729, 9728, 0.37), (9984, 9728, 4.0), (9985, 9728, 4.0), (9986, 9728, 4.0), (9987, 9728, 4.0), (9987, 9728, 2.5), (9729, 9728, 4.0), (9984, 9728, 1.0), (9987, 9729, 0.5)].iter().enumerate() {
        head(s, &format!("tex-open-filter-{k}"), 32);
        writeln!(s, "gltex {T2} 0 4 8 8 0 0x1908 0x1401 {} 256\ngltex {T2} 1 4 4 4 0 0x1908 0x1401 {} 64\ngltex {T2} 2 4 2 2 0 0x1908 0x1401 {} 16\ngltex {T2} 3 4 1 1 0 0x1908 0x1401 {} 4", k.wrapping_add(0x500), k.wrapping_add(0x510), k.wrapping_add(0x520), k.wrapping_add(0x530));
        writeln!(s, "gl glTexParameteri {T2} 10241 {min}\ngl glTexParameteri {T2} 10240 {mag}\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glClear 0x4000");
        quad(s, 0.0, 0.0, *s1, s1.mul_add(0.75, 0.0));
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    // an incomplete mipmap chain under a mipmap filter, and a quad over the mip levels' ranges
    head(s, "tex-open-incomplete", 32);
    writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 9 64\ngl glTexParameteri {T2} 10241 9984\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glClear 0x4000");
    quad(s, 0.0, 0.0, 1.0, 1.0);
    read_all(s);
    writeln!(s, "glerr\nend");
    // wrapping and the border: REPEAT and CLAMP with coordinates past 0..1, a bordered image,
    // the border color
    for (k, (ws, wt, b)) in [(10497u32, 10497u32, 0u32), (10496, 10496, 0), (10497, 10496, 0), (10496, 10496, 1), (10497, 10497, 1)].iter().enumerate() {
        for filter in [9728u32, 9729] {
            head(s, &format!("tex-open-wrap-{k}-{filter}"), 32);
            let side = b.wrapping_mul(2).wrapping_add(4);
            writeln!(s, "gltex {T2} 0 4 {side} {side} {b} 0x1908 0x1401 {} {}\ngl glTexParameteri {T2} 10242 {ws}\ngl glTexParameteri {T2} 10243 {wt}\ngltexv param {T2} 4100 f 1 0.5 0 1\ngl glTexParameteri {T2} 10241 {filter}\ngl glTexParameteri {T2} 10240 {filter}\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glClear 0x4000", k.wrapping_add(0x600), side.wrapping_mul(side).wrapping_mul(4));
            quad(s, -1.25, -0.75, 2.5, 2.0);
            read_all(s);
            writeln!(s, "glerr\nend");
        }
    }
    // one-dimensional textures
    head(s, "tex-open-1d", 32);
    writeln!(s, "gltex 0xde0 0 4 8 1 0 0x1908 0x1401 11 32\ngl glTexParameteri 0xde0 10241 9729\ngl glTexEnvi 8960 8704 7681\ngl glEnable 0xde0\ngl glClear 0x4000\ngl glBegin 7\ngl glTexCoord1f -0.2\ngl glVertex2f 0 0\ngl glTexCoord1f 1.3\ngl glVertex2f 16 0\ngl glTexCoord1f 1.3\ngl glVertex2f 16 16\ngl glTexCoord1f -0.2\ngl glVertex2f 0 16\ngl glEnd");
    read_all(s);
    writeln!(s, "gl glEnable {T2}\nglerr\nend");
    // perspective: a textured quad in a frustum under each perspective hint
    for (k, hint) in [0x1100u32, 0x1101, 0x1102].iter().enumerate() {
        writeln!(s, "case tex-open-persp-{k}\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glFrustum -1 1 -1 1 1 10\ngl glMatrixMode 0x1700\ngl glLoadIdentity\ngl glDisable 0xbd0\ngl glHint 0xc50 {hint}", ctx(32));
        writeln!(s, "gltex {T2} 0 4 8 8 0 0x1908 0x1401 12 256\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glClear 0x4000\ngl glBegin 7\ngl glTexCoord2f 0 0\ngl glVertex3f -1 -1 -1.2\ngl glTexCoord2f 1 0\ngl glVertex3f 1 -1 -1.2\ngl glTexCoord2f 1 1\ngl glVertex3f 1 1 -4\ngl glTexCoord2f 0 1\ngl glVertex3f -1 1 -4\ngl glEnd");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    // the texture matrix and texture coordinate generation (each mode), and their queries
    head(s, "tex-open-matrix", 32);
    writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 13 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glMatrixMode 0x1702\ngl glTranslatef 0.25 0.125 0\ngl glRotatef 30 0 0 1\ngl glMatrixMode 0x1700\ngl glClear 0x4000");
    quad(s, 0.0, 0.0, 1.0, 1.0);
    read_all(s);
    writeln!(s, "glget f 0xba8 16\nglerr\nend");
    for (k, mode) in [9217u32, 9216, 9218].iter().enumerate() {
        head(s, &format!("tex-open-gen-{k}"), 32);
        writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 14 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 7681\ngl glEnable {T2}\ngl glTexGeni 8192 9472 {mode}\ngl glTexGeni 8193 9472 {mode}\ngltexv gen 8192 9473 f 0.0625 0 0 0.1\ngltexv gen 8193 9473 f 0 0.0625 0 -0.2\ngltexv gen 8192 9474 f 0.125 0.03125 0 0\ngltexv gen 8193 9474 d 0 0.125 0 0.25\ngl glEnable 0xc60\ngl glEnable 0xc61\ngl glClear 0x4000\ngl glNormal3f 0.3 0.4 0.866");
        quad(s, 0.0, 0.0, 1.0, 1.0);
        read_all(s);
        q(s, "gen", "8192 9472", "i", 1);
        q(s, "gen", "8192 9473", "f", 4);
        q(s, "gen", "8193 9474", "d", 4);
        writeln!(s, "glerr\nend");
    }
    // textured lines, points, smooth triangles and lit, fogged and dithered textured polygons
    let prims = [
        ("line", 32, "gl glBegin 1\ngl glTexCoord2f 0 0\ngl glVertex2f 1.5 2.25\ngl glTexCoord2f 1 1\ngl glVertex2f 14.5 11.75\ngl glEnd"),
        ("line-smooth", 32, "gl glEnable 0xb20\ngl glBegin 1\ngl glTexCoord2f 0 0\ngl glVertex2f 1.5 2.25\ngl glTexCoord2f 1 1\ngl glVertex2f 14.5 11.75\ngl glEnd"),
        ("point", 32, "gl glPointSize 3\ngl glBegin 0\ngl glTexCoord2f 0.3 0.6\ngl glVertex2f 5.5 7.5\ngl glTexCoord2f 0.8 0.1\ngl glVertex2f 11.5 3.5\ngl glEnd"),
        ("tri-smooth", 32, "gl glEnable 0xb41\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex2f 2 2\ngl glTexCoord2f 1 0\ngl glVertex2f 13 4\ngl glTexCoord2f 0.5 1\ngl glVertex2f 6 13\ngl glEnd"),
        ("lit", 32, "gl glEnable 0xb50\ngl glEnable 0x4000\ngl glNormal3f 0 0 1\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex2f 2 2\ngl glTexCoord2f 1 0\ngl glVertex2f 13 4\ngl glTexCoord2f 0.5 1\ngl glVertex2f 6 13\ngl glEnd"),
        ("fog", 32, "gl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex3f 2 2 0.5\ngl glTexCoord2f 1 0\ngl glVertex3f 13 4 -0.25\ngl glTexCoord2f 0.5 1\ngl glVertex3f 6 13 0\ngl glEnd"),
        ("dither16", 16, "gl glEnable 0xbd0\ngl glBegin 4\ngl glTexCoord2f 0 0\ngl glVertex2f 2 2\ngl glTexCoord2f 1 0\ngl glVertex2f 13 4\ngl glTexCoord2f 0.5 1\ngl glVertex2f 6 13\ngl glEnd"),
        ("rect", 32, "gl glRectf 2 3 12 13"),
        ("drawpixels", 32, "gl glRasterPos2i 3 3\ngldraw 4 4 0x1908 0x1401 9 64"),
    ];
    for (name, bpp, body) in prims {
        head(s, &format!("tex-open-prim-{name}"), bpp);
        writeln!(s, "gltex {T2} 0 4 4 4 0 0x1908 0x1401 15 64\ngl glTexParameteri {T2} 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable {T2}\ngl glClear 0x4000\ngl glColor4ub 200 150 100 255\n{body}");
        read_all(s);
        writeln!(s, "glerr\nend");
    }
    // the texture bit of glPushAttrib, and texture objects' state across binds
    head(s, "tex-open-state", 32);
    writeln!(s, "glgentex 2\ngl glBindTexture {T2} 1\ngltex {T2} 0 4 2 2 0 0x1908 0x1401 16 16\ngl glTexParameteri {T2} 10241 9729\ngl glBindTexture {T2} 2\ngltex {T2} 0 3 4 4 0 0x1908 0x1401 17 64");
    q(s, "param", &format!("{T2} 10241"), "i", 1);
    q(s, "level", &format!("{T2} 0 4096"), "i", 1);
    writeln!(s, "gl glBindTexture {T2} 1");
    q(s, "param", &format!("{T2} 10241"), "i", 1);
    q(s, "level", &format!("{T2} 0 4096"), "i", 1);
    writeln!(s, "glget i 0x8069 1\ngl glPushAttrib 0x40000\ngl glBindTexture {T2} 2\ngl glTexEnvi 8960 8704 8449\ngl glEnable {T2}\ngl glPopAttrib\nglget i 0x8069 1\nglisen {T2}");
    q(s, "env", "8960 8704", "i", 1);
    q(s, "param", &format!("{T2} 10241"), "i", 1);
    writeln!(s, "gl glBindTexture {T2} 0");
    q(s, "level", &format!("{T2} 0 4096"), "i", 1);
    writeln!(s, "glerr\nend");
}

/// what the model does not follow yet, one open clause each (spec/opengl.txt GL.LIST .. WGL.OTHER):
/// display lists, vertex arrays, evaluators, selection and feedback, lights and materials, clip
/// planes, bitmaps, glCopyPixels, stipple patterns, pixel maps, the other forms of calls, WGL
fn gaps(s: &mut String) {
    let head = |s: &mut String, name: &str, bpp: u32| {
        writeln!(s, "case {name}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp));
    };
    let tri = "gl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex2f 2 2\ngl glColor4ub 0 255 0 255\ngl glVertex2f 13 4\ngl glColor4ub 0 0 255 255\ngl glVertex2f 6 13\ngl glEnd";
    // display lists: names, compiling, executing, nesting, glCallLists' types and base, errors (GL.LIST)
    head(s, "gap-list-0", 32);
    writeln!(s, "glr glGenLists u i 3\nglr glIsList b i 1\ngl glNewList 1 0x1300\ngl glColor4ub 10 200 30 255\ngl glRectf 1 1 6 6\ngl glEndList\nglr glIsList b i 1\nglget i 0xb33 1\ngl glClear 0x4000");
    read_all(s);
    writeln!(s, "gl glNewList 2 0x1301\ngl glTranslatef 4 4 0\ngl glCallList 1\ngl glEndList\nglget f 0xba6 16");
    read_all(s);
    writeln!(s, "gl glLoadIdentity\ngl glListBase 1\nglp glCallLists ii 2 0x1401 ub 0 1\nglget i 0xb32 1\nglerr\ngl glEndList\nglerr\ngl glNewList 0 0x1300\nglerr\ngl glNewList 3 0x1302\nglerr\ngl glNewList 3 0x1300\ngl glNewList 4 0x1300\nglerr\ngl glEndList\ngl glDeleteLists 1 2\nglr glIsList b i 1\nglr glIsList b i 3\nglr glGenLists u i 0\nglr glGenLists i i -1\nglerr\nend");
    // glCallLists of each type and the byte forms, nesting past the limit, what is compiled and
    // what runs at once, a list in glBegin / glEnd, errors in a list at its call (GL.LIST)
    head(s, "gap-list-1", 32);
    writeln!(s, "glr glGenLists u i 4\ngl glNewList 1 0x1300\ngl glTranslatef 1 0 0\ngl glEndList\ngl glNewList 2 0x1300\ngl glCallList 2\ngl glTranslatef 0 1 0\ngl glEndList\ngl glNewList 3 0x1300\ngl glColor4ub 255 255 0 255\ngl glPixelStorei 0xcf5 1\nglget i 0xcf5 1\ngl glVertex2f 3 3\ngl glEnable 0xbe2\ngl glMatrixMode 0x1705\ngl glEndList\nglerr\nglisen 0xbe2\nglget f 0xb00 4");
    for (ty, el, vals) in [("0x1400", "b", "-1 1"), ("0x1402", "s", "1 -2"), ("0x1403", "us", "1 65535"), ("0x1404", "i", "1 2"), ("0x1405", "ui", "1 2"), ("0x1406", "f", "1 2"), ("0x1407", "ub", "0 1 0 2"), ("0x1408", "ub", "0 0 1 0 0 3"), ("0x1409", "ub", "0 0 0 1 0 0 0 1"), ("0x140a", "ub", "1 1")] {
        writeln!(s, "gl glLoadIdentity\nglp glCallLists ii 2 {ty} {el} {vals}\nglerr\nglget f 0xba6 16");
    }
    writeln!(s, "gl glLoadIdentity\ngl glListBase 4294967295\nglp glCallLists ii 2 0x1401 ub 2 3\nglget f 0xba6 16\ngl glListBase 0\ngl glLoadIdentity\ngl glCallList 2\nglget f 0xba6 16\ngl glCallList 3\nglerr\nglget f 0xb00 4\nglisen 0xbe2\nglget i 0xcf5 1\nglp glCallLists ii -1 0x1401 ub 1\nglerr\ngl glBegin 1\ngl glCallList 1\nglerr\ngl glNewList 4 0x1300\nglerr\ngl glEnd\ngl glNewList 4 0x1300\ngl glBegin 1\ngl glVertex2f 1 1\ngl glEndList\nglerr\ngl glDeleteLists 1 -1\nglerr\nglr glIsList b i 0\nglr glGenLists u i 2\nglr glGenLists u i 1\nglerr\nend");
    head(s, "gap-list-2", 32);
    writeln!(s, "gl glNewList 5 0x1300\ngl glRasterPos2i 2 2\ngldraw 2 2 0x1908 0x1401 5 16\ngltex 0xde1 0 4 2 2 0 0x1908 0x1401 6 16\ngl glEndList\ngl glPixelStorei 0xcf2 4\ngl glClear 0x4000\ngl glCallList 5");
    read_all(s);
    writeln!(s, "gltexq level 0xde1 0 4096 i 1\nglteximg 0xde1 0 0x1908 0x1401 16\nglerr\nend");
    // vertex arrays: pointers, client state, glArrayElement, glDrawArrays, glDrawElements,
    // interleaved arrays, the client attribute stack, edge flags (GL.ARRAY)
    head(s, "gap-array-0", 32);
    writeln!(s, "glp glVertexPointer iii 2 0x1406 0 f 2 2 13 4 6 13 14 14\nglp glColorPointer iii 4 0x1401 0 ub 255 0 0 255 0 255 0 255 0 0 255 255 255 255 255 255\ngl glEnableClientState 0x8074\ngl glEnableClientState 0x8076\nglisen 0x8074\ngl glClear 0x4000\ngl glDrawArrays 4 0 3");
    read_all(s);
    writeln!(s, "gl glClear 0x4000\nglp glDrawElements iii 1 2 0x1403 us 0 3\ngl glBegin 0\ngl glArrayElement 3\ngl glEnd");
    read_all(s);
    writeln!(s, "glq glGetPointerv i 0x808e p 1\nglget i 0x807a 1\nglget i 0x807b 1\nglget i 0x807c 1\ngl glPushClientAttrib 2\ngl glDisableClientState 0x8074\ngl glPopClientAttrib\nglisen 0x8074\ngl glDrawArrays 4 0 -1\nglerr\ngl glDrawArrays 10 0 3\nglerr\nglp glDrawElements iii 4 3 0x1406 f 0 1 2\nglerr\nend");
    head(s, "gap-array-1", 32);
    writeln!(s, "glp glInterleavedArrays ii 0x2a22 0 ub 255 0 0 255 0 0 0 64 0 0 0 64 0 255 0 255 0 0 80 65 0 0 128 64 0 0 255 255 0 0 192 64 0 0 80 65\ngl glClear 0x4000\ngl glDrawArrays 4 0 3");
    read_all(s);
    writeln!(s, "glisen 0x8076\nglget i 0x8082 1\ngl glPolygonMode 0x408 0x1b01\ngl glClear 0x4000\ngl glBegin 4\ngl glEdgeFlag 0\ngl glVertex2f 2 2\ngl glEdgeFlag 1\ngl glVertex2f 13 4\ngl glVertex2f 6 13\ngl glEnd");
    read_all(s);
    writeln!(s, "glget i 0xb43 1\nglerr\nend");
    // evaluators: a 1D vertex and color map, grids and meshes, a 2D map, the queries (GL.EVAL)
    head(s, "gap-eval-0", 32);
    writeln!(s, "glp glMap1f iffii 0xd97 0 1 3 3 f 1 1 0 8 15 0 15 1 0\nglp glMap1f iffii 0xd90 0 1 4 2 f 1 0 0 1 0 0 1 1\ngl glEnable 0xd97\ngl glEnable 0xd90\ngl glClear 0x4000\ngl glBegin 3\ngl glEvalCoord1f 0\ngl glEvalCoord1f 0.25\ngl glEvalCoord1f 0.5\ngl glEvalCoord1d 0.75\ngl glEvalCoord1f 1\ngl glEnd");
    read_all(s);
    writeln!(s, "gl glMapGrid1f 8 0 1\ngl glClear 0x4000\ngl glEvalMesh1 0x1b00 0 8\ngl glEvalPoint1 4");
    read_all(s);
    writeln!(s, "glq glGetMapfv ii 0xd97 0xa00 f 9\nglq glGetMapiv ii 0xd97 0xa01 i 1\nglq glGetMapdv ii 0xd97 0xa02 d 2\nglget f 0xdd0 2\nglget i 0xdd1 1\nglerr\nglp glMap1f iffii 0xd97 1 1 3 3 f 0 0 0 0 0 0 0 0 0\nglerr\nglp glMap1f iffii 0xd97 0 1 3 40 f 0\nglerr");
    writeln!(s, "glp glMap2f iffiiffii 0xdb7 0 1 3 2 0 1 6 2 f 1 1 0 14 1 0 1 14 0 14 14 0\ngl glEnable 0xdb7\ngl glMapGrid2f 4 0 1 4 0 1\ngl glClear 0x4000\ngl glEvalMesh2 0x1b02 0 4 0 4");
    read_all(s);
    writeln!(s, "gl glClear 0x4000\ngl glEvalMesh2 0x1b01 0 4 0 4");
    read_all(s);
    writeln!(s, "glq glGetMapiv ii 0xdb7 0xa01 i 2\nglerr\nend");
    // selection: names, hits, overflow; feedback of each type, pass-through (GL.SELECT)
    head(s, "gap-select-0", 32);
    writeln!(s, "glp glSelectBuffer i 64 obuf\nglr glRenderMode i i 0x1c02\ngl glInitNames\ngl glPushName 7\ngl glRectf 1 1 4 4\ngl glLoadName 8\ngl glRectf 20 20 24 24\ngl glPushName 9\n{tri}\ngl glPopName\ngl glPopName\ngl glPopName\nglerr\nglr glRenderMode i i 0x1c00\nglobuf u 16\nglget i 0xc40 1\nglerr");
    writeln!(s, "glp glSelectBuffer i 3 obuf\nglr glRenderMode i i 0x1c02\ngl glPushName 1\ngl glPushName 2\ngl glRectf 1 1 4 4\nglr glRenderMode i i 0x1c00\nglobuf u 4\nglerr\ngl glPushName 1\nglerr\nend");
    for (k, ty) in [0x600u32, 0x601, 0x602, 0x603, 0x604].iter().enumerate() {
        head(s, &format!("gap-select-{}", k.wrapping_add(1)), 32);
        writeln!(s, "glp glFeedbackBuffer ii 128 {ty} obuf\nglr glRenderMode i i 0x1c01\ngl glPassThrough 3.5\n{tri}\ngl glBegin 1\ngl glVertex2f 1 15\ngl glVertex2f 15 1\ngl glEnd\ngl glBegin 0\ngl glVertex2f 8 8\ngl glEnd\ngl glRasterPos2f 3 3\ngldraw 1 1 0x1908 0x1401 1 4\nglr glRenderMode i i 0x1c00\nglobuf f 96\nglerr\nend");
    }
    // lights and materials: each parameter set and read back, a lit triangle (GL.LIGHT.STATE)
    head(s, "gap-light-0", 32);
    writeln!(s, "glq glGetLightfv ii 0x4001 0x1201 f 4\nglq glGetLightfv ii 0x4000 0x1203 f 4\nglq glGetLightiv ii 0x4000 0x1206 i 1\nglq glGetMaterialfv ii 0x404 0x1601 f 1\nglq glGetMaterialfv ii 0x404 0x1201 f 4");
    writeln!(s, "gl glTranslatef 1 2 0\nglp glLightfv ii 0x4000 0x1203 f 0 0 1 0\nglp glLightfv ii 0x4000 0x1201 f 0.5 0.25 1 1\nglp glLightfv ii 0x4001 0x1204 f 0 0 -1\ngl glLightf 0x4001 0x1206 30\ngl glLighti 0x4001 0x1205 5\ngl glLightf 0x4001 0x1207 2\nglq glGetLightfv ii 0x4000 0x1203 f 4\nglq glGetLightfv ii 0x4001 0x1204 f 3\nglq glGetLightiv ii 0x4000 0x1201 i 4\nglq glGetLightfv ii 0x4001 0x1206 f 1\nglq glGetLightfv ii 0x4001 0x1207 f 1");
    writeln!(s, "gl glLightf 0x4000 0x1206 100\nglerr\ngl glLightf 0x4000 0x1205 200\nglerr\ngl glLightModeli 0xb52 1\ngl glLightModelf 0xb51 1\nglp glLightModelfv i 0xb53 f 0.1 0.2 0.3 1\nglget f 0xb53 4\nglget i 0xb52 1\nglp glMaterialfv ii 0x408 0x1602 f 0.8 0.6 0.4 1\nglp glMaterialfv ii 0x404 0x1202 f 1 1 1 1\ngl glMaterialf 0x404 0x1601 20\ngl glMateriali 0x405 0x1601 200\nglerr\nglq glGetMaterialfv ii 0x405 0x1200 f 4\nglq glGetMaterialiv ii 0x404 0x1201 i 4\nglq glGetMaterialfv ii 0x404 0x1601 f 1");
    writeln!(s, "gl glLoadIdentity\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glNormal3f 0 0 1\ngl glClear 0x4000\n{tri}");
    read_all(s);
    writeln!(s, "gl glColorMaterial 0x404 0x1201\ngl glEnable 0xb57\ngl glClear 0x4000\n{tri}");
    read_all(s);
    writeln!(s, "glget i 0xb55 1\nglget i 0xb56 1\ngl glColorMaterial 0x404 0x1500\nglerr\nend");
    // user clip planes (GL.CLIPPLANE)
    head(s, "gap-clip-0", 32);
    writeln!(s, "gl glTranslatef 2 0 0\nglp glClipPlane i 0x3000 d 1 -1 0 0\ngl glLoadIdentity\nglq glGetClipPlane i 0x3000 d 4\ngl glEnable 0x3000\ngl glClear 0x4000\n{tri}");
    read_all(s);
    writeln!(s, "glp glClipPlane i 0x3001 d 0 1 0 -7.5\ngl glEnable 0x3001\ngl glClear 0x4000\ngl glRectf 0 0 16 16");
    read_all(s);
    writeln!(s, "glp glClipPlane i 0x3006 d 0 0 0 0\nglerr\nend");
    // bitmaps: set bits at the raster position, the moves, the raster color, no raster position (GL.BITMAP)
    head(s, "gap-bitmap-0", 32);
    writeln!(s, "gl glColor4ub 200 100 50 255\ngl glRasterPos2f 2.5 3.5\ngl glColor4ub 0 0 255 255\ngl glClear 0x4000\nglp glBitmap iiffff 8 4 1 1 5 2 ub 129 255 66 60\nglget f 0xb07 4\nglp glBitmap iiffff 8 4 0 0 5 2 ub 15 240 15 240\nglget f 0xb07 4\nglget f 0xb04 4");
    read_all(s);
    writeln!(s, "gl glPixelStorei 0xcf1 1\ngl glClear 0x4000\nglp glBitmap iiffff 8 2 0 0 0 0 ub 3 192\ngl glRasterPos2f -4 2\nglp glBitmap iiffff 0 0 0 0 6 0 null\nglp glBitmap iiffff 8 2 0 0 0 0 ub 255 255\nglget i 0xb08 1\nglget f 0xb07 4");
    read_all(s);
    writeln!(s, "glp glBitmap iiffff -1 2 0 0 0 0 ub 255 255\nglerr\nend");
    // glCopyPixels of color, depth and stencil, overlapping, zoomed, from outside (GL.COPYPIXELS)
    head(s, "gap-copy-0", 32);
    writeln!(s, "gldraw 16 16 0x1908 0x1401 21 1024\ngl glRasterPos2i 4 3\ngl glCopyPixels 1 1 6 5 0x1800");
    read_all(s);
    writeln!(s, "gl glRasterPos2i 2 2\ngl glPixelZoom 2 -1\ngl glCopyPixels 0 0 4 4 0x1800\ngl glPixelZoom 1 1\ngl glRasterPos2i 10 10\ngl glCopyPixels 12 12 8 8 0x1800");
    read_all(s);
    writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glClear 0x400\ngl glRectf 0 0 8 8\ngl glRasterPos2i 8 8\ngl glCopyPixels 0 0 8 8 0x1801\nglread 0 0 16 16 0x1902 0x1405 1024\ngl glCopyPixels 0 0 4 4 0x1802\nglread 0 0 16 16 0x1901 0x1401 256\ngl glCopyPixels 0 0 -1 4 0x1800\nglerr\ngl glCopyPixels 0 0 1 1 0x1900\nglerr\nend");
    // a polygon stipple pattern, set, read back, masking (GL.POLYGON.STIPPLE.PATTERN)
    head(s, "gap-stipple-0", 32);
    // (the 128 bytes as 32 little-endian words: the executor takes 80 tokens a line)
    let mut pat = String::new();
    for k in 0u32..32 {
        let byte = |j: u32| k.wrapping_mul(4).wrapping_add(j).wrapping_mul(37).wrapping_add(11) & 0xff;
        let v = byte(0) | byte(1) << 8 | byte(2) << 16 | byte(3) << 24;
        pat.push_str(&format!(" {v}"));
    }
    writeln!(s, "glq glGetPolygonStipple - ub 128\nglp glPolygonStipple - ui{pat}\nglq glGetPolygonStipple - ub 128\ngl glEnable 0xb42\ngl glClear 0x4000\n{tri}");
    read_all(s);
    writeln!(s, "gl glPixelStorei 0xcf1 1\nglp glPolygonStipple - ui{pat}\ngl glPixelStorei 0xd01 1\nglq glGetPolygonStipple - ub 128\ngl glClear 0x4000\ngl glRectf 0 0 16 16");
    read_all(s);
    writeln!(s, "glerr\nend");
    // pixel maps: sizes and values, drawn and read through them, the queries (GL.PIXELMAP)
    head(s, "gap-pixelmap-0", 32);
    writeln!(s, "glq glGetPixelMapfv i 0xc76 f 2\nglget i 0xcb6 1\nglp glPixelMapfv ii 0xc76 4 f 0 0.25 0.5 1\nglp glPixelMapuiv ii 0xc77 2 ui 0 4294967295\nglp glPixelMapusv ii 0xc78 3 us 65535 0 32768\nglp glPixelMapfv ii 0xc79 1 f 0.75\ngl glPixelTransferi 0xd10 1\ngl glRasterPos2i 0 0\ngldraw 16 4 0x1908 0x1401 22 256");
    read_all(s);
    writeln!(s, "glread 0 0 4 4 0x1908 0x1406 256\nglq glGetPixelMapfv i 0xc76 f 4\nglq glGetPixelMapuiv i 0xc77 ui 2\nglq glGetPixelMapusv i 0xc78 us 3\nglget i 0xcb6 1\nglget i 0xcb7 1\nglp glPixelMapfv ii 0xc76 3 f 0 0.5 1\nglerr\nglp glPixelMapfv ii 0xc70 2 f 1 2\nglerr\nglp glPixelMapfv ii 0xc76 0 f 0\nglerr\nend");
    // the other forms of calls: each type's conversion and the vector forms (GL.VARIANTS)
    head(s, "gap-forms-0", 32);
    for c in ["glColor3b 127 -1 64", "glColor4b -128 100 0 127", "glColor3d 0.25 0.5 2", "glColor3i 2147483647 -1 1073741824", "glColor3s 32767 -32768 16384", "glColor3ui 4294967295 0 2147483648", "glColor3us 65535 32768 1", "glColor4d 0.1 0.2 0.3 0.4", "glColor4i -2147483648 0 1 2147483647", "glColor4s 1 2 3 4", "glColor4ui 1 2 3 4", "glColor4us 1 2 3 4"] {
        writeln!(s, "gl {c}\nglget f 0xb00 4\nglget i 0xb00 4");
    }
    for c in ["glColor3fv - f 0.5 0.25 1", "glColor4ubv - ub 1 2 3 4", "glColor3bv - b 10 -20 30", "glColor4dv - d 1 0 0.5 0.25", "glColor3usv - us 1 2 65535", "glColor4iv - i 1 2 3 4"] {
        writeln!(s, "glp {c}\nglget f 0xb00 4");
    }
    for c in ["glNormal3b 127 -128 0", "glNormal3d 0.5 0.5 0.5", "glNormal3i 2147483647 0 -2147483648", "glNormal3s 32767 -1 0"] {
        writeln!(s, "gl {c}\nglget f 0xb02 3");
    }
    for c in ["glNormal3fv - f 1 2 3", "glNormal3bv - b 1 2 3", "glNormal3dv - d 0.1 0.2 0.3", "glNormal3iv - i 1 2 3", "glNormal3sv - s 1 2 3"] {
        writeln!(s, "glp {c}\nglget f 0xb02 3");
    }
    for c in ["glIndexd 3.75", "glIndexf -2.5", "glIndexs 300", "glIndexub 200"] {
        writeln!(s, "gl {c}\nglget f 0xb01 1");
    }
    for c in ["glIndexdv - d 7.5", "glIndexfv - f 1.25", "glIndexiv - i 9", "glIndexsv - s -4", "glIndexubv - ub 255"] {
        writeln!(s, "glp {c}\nglget f 0xb01 1");
    }
    for c in ["glTexCoord1d 0.5", "glTexCoord1i 3", "glTexCoord1s 2", "glTexCoord2d 0.25 0.75", "glTexCoord2i 1 2", "glTexCoord2s 3 4", "glTexCoord3d 1 2 3", "glTexCoord3i 1 2 3", "glTexCoord3s 1 2 3", "glTexCoord4d 1 2 3 4", "glTexCoord4i 1 2 3 4", "glTexCoord4s 1 2 3 4"] {
        writeln!(s, "gl {c}\nglget f 0xb03 4");
    }
    for c in ["glTexCoord2fv - f 0.5 0.5", "glTexCoord1dv - d 2", "glTexCoord3iv - i 1 2 3", "glTexCoord4sv - s 1 2 3 4", "glTexCoord2dv - d 1 2", "glTexCoord1fv - f 3", "glTexCoord1iv - i 4", "glTexCoord1sv - s 5", "glTexCoord2iv - i 6 7", "glTexCoord2sv - s 8 9", "glTexCoord3dv - d 1 2 3", "glTexCoord3fv - f 1 2 3", "glTexCoord3sv - s 1 2 3", "glTexCoord4dv - d 1 2 3 4", "glTexCoord4fv - f 1 2 3 4", "glTexCoord4iv - i 1 2 3 4"] {
        writeln!(s, "glp {c}\nglget f 0xb03 4");
    }
    for c in ["glRasterPos2d 3.5 4.5", "glRasterPos2s 5 6", "glRasterPos3d 1 2 0.5", "glRasterPos3f 7.25 8.75 -0.5", "glRasterPos3i 2 3 0", "glRasterPos3s 4 5 0", "glRasterPos4d 2 2 0 2", "glRasterPos4f 6 6 0 2", "glRasterPos4i 4 4 0 2", "glRasterPos4s 8 8 0 4"] {
        writeln!(s, "gl {c}\nglget f 0xb07 4\nglget i 0xb08 1");
    }
    for c in ["glRasterPos2fv - f 1.5 2.5", "glRasterPos2dv - d 3 4", "glRasterPos2iv - i 5 6", "glRasterPos2sv - s 7 8", "glRasterPos3dv - d 1 2 0", "glRasterPos3fv - f 1 2 0", "glRasterPos3iv - i 1 2 0", "glRasterPos3sv - s 1 2 0", "glRasterPos4dv - d 1 2 0 1", "glRasterPos4fv - f 1 2 0 1", "glRasterPos4iv - i 1 2 0 1", "glRasterPos4sv - s 1 2 0 1"] {
        writeln!(s, "glp {c}\nglget f 0xb07 4");
    }
    writeln!(s, "glerr\nend");
    head(s, "gap-forms-1", 32);
    writeln!(s, "gl glClear 0x4000\ngl glColor4ub 255 0 0 255\ngl glRectd 1.5 1.5 5.5 5.5\ngl glRects 6 6 9 9\nglp glRectfv - f 10 1 14 4\nglp glRectdv - d 1 10 4 14\nglp glRectiv - i 6 11 8 15\nglp glRectsv - s 11 11 15 15");
    read_all(s);
    writeln!(s, "gl glClear 0x4000\ngl glBegin 4\ngl glVertex2d 2 2\ngl glVertex2s 13 4\ngl glVertex3d 6 13 0\ngl glVertex3s 14 14 0\ngl glVertex4d 30 2 0 2\ngl glVertex4i 30 30 0 2\ngl glVertex4s 1 15 0 1\nglp glVertex2fv - f 15 8\nglp glVertex2dv - d 10 15\nglp glVertex2iv - i 3 8\nglp glVertex2sv - s 9 1\nglp glVertex3dv - d 15 1 0\nglp glVertex3fv - f 8 8 0\nglp glVertex3iv - i 1 1 0\nglp glVertex3sv - s 4 12 0\nglp glVertex4dv - d 8 2 0 1\nglp glVertex4fv - f 14 10 0 1\nglp glVertex4iv - i 2 14 0 1\nglp glVertex4sv - s 12 2 0 1\ngl glEnd");
    read_all(s);
    writeln!(s, "gl glFogi 0xb65 0x2601\nglp glFogiv i 0xb66 i 2147483647 0 1073741824 2147483647\nglget f 0xb66 4\nglget i 0xb65 1\ngl glRotated 30 0 0 1\ngl glScaled 2 0.5 1\ngl glTranslated 1 2 3\nglget f 0xba6 16\nglp glLoadMatrixd - d 1 0 0 0 0 1 0 0 0 0 1 0 0.1 0.2 0.3 1\nglp glMultMatrixd - d 2 0 0 0 0 2 0 0 0 0 2 0 0 0 0 1\nglget f 0xba6 16\nglp glEdgeFlagv - ub 0\nglget i 0xb43 1\nglerr\nend");
    // WGL: the other exports on the DIB's DC (WGL.OTHER)
    head(s, "gap-wgl-0", 32);
    writeln!(s, "wglx getpf\nwglx descpf 1\nwglx descpf 99\nwglx setpf 2\nwglx choose 0x38 0 32 0 0 32 8\nwglx swap\nwglx swaplayer 1\nwglx swapmultiple 1\nwglx proc glAddSwapHintRectWIN\nwglx proc glColorTableEXT\nwglx proc wglSwapIntervalEXT\nwglx proc glBegin\nwglx defproc glBegin\nwglx layer 1 1\nwglx layer 1 0\nwglx getlayerpal 1 0 4\nwglx setlayerpal 1 0 4\nwglx realize 1 1\nwglx layerctx 1\nwglx layerctx 0\nwglx share\nwglx copy 0x40000\nglerr\ngl glEnable -1\nglr glDebugEntry i ii 0 0\nglerr\ngl glBegin 0\nglr glDebugEntry i ii 1 1\ngl glEnd\nglerr\nend");
    head(s, "gap-wgl-1", 32);
    writeln!(s, "wglx fontbmp a 65 3 100\nglr glIsList b i 100\nglr glIsList b i 102\nglr glIsList b i 103\ngl glColor4ub 255 255 255 255\ngl glRasterPos2i 1 3\ngl glListBase 35\nglp glCallLists ii 3 0x1401 ub 65 66 67\nglget f 0xb07 4");
    read_all(s);
    writeln!(s, "wglx fontbmp w 0x3042 2 110\nwglx fontoutline a 65 2 120 0\nwglx fontoutline w 65 1 130 1\nglr glIsList b i 120\nglerr\nend");
}
/// Bitmap pack/unpack addressing for glPolygonStipple and glGetPolygonStipple.
pub fn glstipple() -> String {
    let mut s = String::from("# kikizake suite glstipple: polygon stipple pixel storage\n");
    let mut bytes = (0..=u8::MAX).map(|i| i.wrapping_mul(37).wrapping_add(11));
    let mut words = String::new();
    for _ in 0..64 {
        let value = u32::from_le_bytes(std::array::from_fn(|_| bytes.next().unwrap_or_default()));
        write!(words, " {value}");
    }
    for (lsb, align, row, rows, pixels) in [(0,4,0,0,0), (1,4,0,0,0), (0,8,40,1,3), (1,8,40,2,5), (0,1,16,0,0)] {
        writeln!(s, "case s-{lsb}-{align}-{row}-{rows}-{pixels}\nglctx 32 choose 0x38 0 32 0 0 32 8");
        for base in [0xcf0, 0xd00] {
            for (p, value) in (base..).zip([0, lsb, row, rows, pixels, align]) {
                writeln!(s, "gl glPixelStorei {p} {value}");
            }
            if base == 0xcf0 {
                writeln!(s, "glp glPolygonStipple - ui{words}\nglq glGetPolygonStipple - ub 128");
            }
        }
        writeln!(s, "glq glGetPolygonStipple - ub 320\nglerr\nend");
    }
    s
}
