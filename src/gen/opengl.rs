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
    writeln!(s, "gldraw 16 16 {} {} {seed} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
}

fn read_all(s: &mut String) {
    writeln!(s, "glread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
}

pub fn opengl() -> String {
    let mut s = String::from("# gpu\n# kikizake suite opengl: WGL and OpenGL 1.1 on DIB sections (SPEC.md). Made by `kikizake gen opengl`.\n");
    let mut r = XorShift32::new(0x0960_1011);

    // every pixel format on DIBs of each depth
    for bpp in [1, 4, 8, 16, 24, 32] {
        writeln!(s, "case pf-{bpp}\nglpf {bpp}\nend").unwrap();
    }
    // ChoosePixelFormat: each request field alone, then random requests
    let flags = [0x38, 0x28, 0x29, 0x2c, 0x3c, 0x39, 0x08, 0x20, 0x00, 0x18, 0x2d];
    for bpp in [8, 16, 24, 32] {
        writeln!(s, "case choose-fields-{bpp}").unwrap();
        for fl in flags {
            writeln!(s, "glchoose {bpp} {} 0 {bpp} 0 0 32 8", h(fl)).unwrap();
        }
        for (ty, color, alpha, accum, depth, stencil) in [(1, bpp, 0, 0, 32, 8), (0, 0, 0, 0, 0, 0), (0, 8, 0, 0, 32, 8), (0, 16, 0, 0, 32, 8), (0, 24, 0, 0, 32, 8), (0, 32, 0, 0, 32, 8), (0, bpp, 8, 0, 32, 8), (0, bpp, 0, 64, 32, 8), (0, bpp, 0, 0, 16, 8), (0, bpp, 0, 0, 24, 8), (0, bpp, 0, 0, 0, 0), (0, bpp, 0, 0, 32, 0), (0, bpp, 0, 0, 16, 0), (0, bpp, 0, 32, 16, 4)] {
            writeln!(s, "glchoose {bpp} 0x38 {ty} {color} {alpha} {accum} {depth} {stencil}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for i in 0..12 {
        writeln!(s, "case choose-{i}").unwrap();
        for _ in 0..16 {
            let bpp = *r.pick(&[8, 16, 24, 32]);
            let fl = *r.pick(&flags) | if r.chance(5) { 0x2 } else { 0 };
            let pick = |r: &mut XorShift32, v: &[u32]| *r.pick(v);
            writeln!(s, "glchoose {bpp} {} {} {} {} {} {} {}", h(fl), pick(&mut r, &[0, 0, 0, 1]), pick(&mut r, &[0, 4, 8, 15, 16, 24, 32, 48]), pick(&mut r, &[0, 0, 1, 8]), pick(&mut r, &[0, 0, 16, 32, 64]), pick(&mut r, &[0, 8, 16, 24, 32]), pick(&mut r, &[0, 0, 1, 4, 8, 16])).unwrap();
        }
        writeln!(s, "end").unwrap();
    }

    // contexts: the current context and DC, the strings, released
    for bpp in [8, 16, 24, 32] {
        writeln!(s, "case ctx-{bpp}\nglcur\n{}glcur", ctx(bpp)).unwrap();
        for n in [VENDOR, RENDERER, VERSION, EXTENSIONS, 0, 0x1f04] {
            writeln!(s, "glstr {}", h(n)).unwrap();
        }
        writeln!(s, "glerr\nglrelease\nglcur\nend").unwrap();
    }
    // explicit formats: each of the first 24 on a 32-bit DIB
    for pf in 1..=24 {
        writeln!(s, "case ctx-pf-{pf}\nglctx 32 {pf}\nglget i {} 16\nglget i {} 16\nglget i {} 16\nglget i {} 16\nglget b {} 1\nend", h(pname("RED_BITS")), h(pname("DEPTH_BITS")), h(pname("STENCIL_BITS")), h(pname("ALPHA_BITS")), h(pname("DOUBLEBUFFER"))).unwrap();
    }

    // the initial state: every glGet parameter as integers, floats, booleans, doubles
    for (kind, bpps) in [("i", &[16u32, 24, 32][..]), ("f", &[32]), ("b", &[32]), ("d", &[32])] {
        for &bpp in bpps {
            writeln!(s, "case defaults-{kind}-{bpp}\n{}", ctx(bpp).trim_end()).unwrap();
            for p in PNAMES {
                writeln!(s, "glget {kind} {} 16 # {}", h(p.1), p.0).unwrap();
            }
            writeln!(s, "glerr\nend").unwrap();
        }
    }
    writeln!(s, "case defaults-caps\n{}", ctx(32).trim_end()).unwrap();
    for c in CAPS {
        writeln!(s, "glisen {}", h(*c)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    // names glGet and glIsEnabled do not know
    writeln!(s, "case defaults-unknown\n{}", ctx(32).trim_end()).unwrap();
    for p in [0, 1, 0xb0a, 0xb0f, 0xc03, 0xd06, 0xdff, 0x1000, 0xffff, 0x8000] {
        writeln!(s, "glget i {} 2\nglerr\nglisen {}\nglerr", h(p), h(p)).unwrap();
    }
    writeln!(s, "end").unwrap();

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
    writeln!(s, "case errors\n{}", ctx(32).trim_end()).unwrap();
    for (name, args) in errs {
        if *name == "glAccum" {
            writeln!(s, "gl glAccum 0x100 0.5").unwrap();
        } else {
            gl(&mut s, name, args);
        }
        writeln!(s, "glerr").unwrap();
    }
    // float arguments out of range
    for (name, v) in [("glLineWidth", 0.0), ("glLineWidth", -1.0), ("glPointSize", 0.0), ("glPointSize", -0.5), ("glLineWidth", 0.25), ("glPointSize", 1000.0)] {
        glf(&mut s, name, &[v]);
        writeln!(s, "glerr").unwrap();
    }
    writeln!(s, "gl glAlphaFunc 0x208 0.5\nglerr\nend").unwrap();
    // several errors before glGetError: which are kept
    writeln!(s, "case errors-queue\n{}gl glEnable 0\ngl glPixelStorei 0xd05 3\ngl glPopMatrix\nglerr\nglerr\nend", ctx(32)).unwrap();
    // inside glBegin / glEnd only some calls are allowed
    writeln!(s, "case errors-begin\n{}gl glBegin 7\ngl glEnable 0xbe2\ngl glEnd\nglerr\nglisen 0xbe2", ctx(32)).unwrap();
    writeln!(s, "gl glBegin 7\ngl glColor4ub 1 2 3 4\ngl glVertex2f 1 1\ngl glEnd\nglerr\nglget i 0xb00 4\nglerr\ngl glBegin 7\ngl glBegin 7\nglerr\ngl glEnd\nglerr\ngl glBegin 7\nglget i 0xb00 4\ngl glEnd\nglerr\nend").unwrap();

    // set then get: enumerated state
    writeln!(s, "case set-blendfunc\n{}", ctx(32).trim_end()).unwrap();
    for src in BLEND_SRC.iter().chain(&[SRC_COLOR, ONE_MINUS_SRC_COLOR]) {
        for dst in BLEND_DST.iter().chain(&[DST_COLOR, SRC_ALPHA_SATURATE]) {
            gl(&mut s, "glBlendFunc", &[*src, *dst]);
            writeln!(s, "glget i 0xbe1 2\nglget i 0xbe0 2\nglerr").unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    writeln!(s, "case set-enums\n{}", ctx(32).trim_end()).unwrap();
    for fu in FUNCS.iter().chain(&[0x1ff, 0x208]) {
        gl(&mut s, "glDepthFunc", &[*fu]);
        writeln!(s, "glget i 0xb74 1\nglerr").unwrap();
        writeln!(s, "gl glAlphaFunc {} 0.25\nglget i 0xbc1 1\nglerr", h(*fu)).unwrap();
        gl(&mut s, "glStencilFunc", &[*fu, 3, 7]);
        writeln!(s, "glget i 0xb92 1\nglerr").unwrap();
    }
    for op in 0x1500..=0x1510 {
        gl(&mut s, "glLogicOp", &[op]);
        writeln!(s, "glget i 0xbf0 1\nglerr").unwrap();
    }
    for sop in SOPS.iter().chain(&[0x1e04, 0]) {
        gl(&mut s, "glStencilOp", &[*sop, KEEP, REPLACE]);
        gl(&mut s, "glStencilOp", &[KEEP, *sop, REPLACE]);
        writeln!(s, "glget i 0xb94 3\nglget i 0xb95 1\nglerr").unwrap();
    }
    for (pn, p) in [(SMOOTH, 0xb54), (FLAT, 0xb54)] {
        gl(&mut s, "glShadeModel", &[pn]);
        writeln!(s, "glget i {} 1", h(p)).unwrap();
    }
    for m in [MODELVIEW, PROJECTION, TEXTURE] {
        gl(&mut s, "glMatrixMode", &[m]);
        writeln!(s, "glget i 0xba0 1").unwrap();
    }
    for (t, v) in [(0xc50, NICEST), (0xc51, FASTEST), (0xc52, DONT_CARE), (0xc53, NICEST), (0xc54, FASTEST)] {
        gl(&mut s, "glHint", &[t, v]);
        writeln!(s, "glget i {} 1", h(t)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    // set then get: numbers, with the clamping each call does
    for i in 0..24 {
        writeln!(s, "case set-values-{i}\n{}", ctx(*r.pick(&[16, 24, 32])).trim_end()).unwrap();
        for _ in 0..6 {
            let kind = *r.pick(&["i", "f"]);
            match r.range(0, 11) {
                0 => {
                    let c: Vec<f64> = (0..4).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glClearColor", &c);
                    writeln!(s, "glget {kind} 0xc22 4").unwrap();
                }
                1 => {
                    let c: Vec<f64> = (0..2).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glDepthRange", &c);
                    writeln!(s, "glget {kind} 0xb70 2").unwrap();
                }
                2 => {
                    let v = rf(&mut r, -1.5, 2.5);
                    glf(&mut s, "glClearDepth", &[v]);
                    writeln!(s, "glget {kind} 0xb73 1").unwrap();
                }
                3 => {
                    let v = rf(&mut r, -1.5, 2.5);
                    writeln!(s, "gl glAlphaFunc {} {}\nglget {kind} 0xbc2 1", h(*r.pick(&FUNCS)), f(v)).unwrap();
                }
                4 => {
                    let (rf_, m) = (*r.pick(&[0, 1, 255, 256, 257, 0xffff_ffff, 0x8000_0000, 1000]), *r.pick(&[0, 1, 0xff, 0x1ff, 0xffff_ffff]));
                    gl(&mut s, "glStencilFunc", &[*r.pick(&FUNCS), rf_, m]);
                    writeln!(s, "glget i 0xb97 1\nglget i 0xb93 1").unwrap();
                }
                5 => {
                    let v = *r.pick(&[0, 1, 0xff, 0x100, 0xffff_ffff, 0x1234_5678]);
                    gl(&mut s, "glStencilMask", &[v]);
                    gl(&mut s, "glClearStencil", &[v]);
                    writeln!(s, "glget i 0xb98 1\nglget i 0xb91 1").unwrap();
                }
                6 => {
                    let v: Vec<u32> = (0..4).map(|_| *r.pick(&[0, 1, 2, 0xff, 0x100])).collect();
                    gl(&mut s, "glColorMask", &v);
                    writeln!(s, "glget i 0xc23 4\nglget b 0xc23 4").unwrap();
                }
                7 => {
                    let v: Vec<u32> = (0..4).map(|_| *r.pick(&[0, 1, 3, 16, 0xffff_fff0, 5000, 0x7fff_ffff])).collect();
                    gl(&mut s, if r.chance(2) { "glViewport" } else { "glScissor" }, &v);
                    writeln!(s, "glget i 0xba2 4\nglget i 0xc10 4").unwrap();
                }
                8 => {
                    let v = rf(&mut r, -2.0, 300.0);
                    glf(&mut s, if r.chance(2) { "glLineWidth" } else { "glPointSize" }, &[v]);
                    writeln!(s, "glget f 0xb21 1\nglget f 0xb11 1").unwrap();
                }
                9 => {
                    let p = *r.pick(&[PACK_ALIGNMENT, UNPACK_ALIGNMENT, PACK_ROW_LENGTH, PACK_SKIP_ROWS, UNPACK_SKIP_PIXELS, PACK_SWAP_BYTES, UNPACK_LSB_FIRST]);
                    let v = *r.pick(&[1, 2, 4, 8, 0, 3, 7, 100]);
                    gl(&mut s, "glPixelStorei", &[p, v]);
                    writeln!(s, "glget i {} 1\nglget b {} 1", h(p), h(p)).unwrap();
                }
                _ => {
                    let c: Vec<f64> = (0..4).map(|_| rf(&mut r, -1.5, 2.5)).collect();
                    glf(&mut s, "glColor4f", &c);
                    writeln!(s, "glget {kind} 0xb00 4").unwrap();
                }
            }
            writeln!(s, "glerr").unwrap();
        }
        writeln!(s, "end").unwrap();
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
            writeln!(s, "glmat {} {}", if r.chance(2) { "load" } else { "mult" }, m.join(" ")).unwrap();
        }
        _ => {
            writeln!(s, "gl glLoadIdentity").unwrap();
        }
    };
    for i in 0..40 {
        writeln!(s, "case matrix-{i}\n{}", ctx(32).trim_end()).unwrap();
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
        writeln!(s, "glget f {} 16\nglget d {} 16\nglerr\nend", h(p), h(p)).unwrap();
    }
    for (mode, depth) in [(MODELVIEW, MODELVIEW_STACK_DEPTH), (PROJECTION, PROJECTION_STACK_DEPTH), (TEXTURE, TEXTURE_STACK_DEPTH)] {
        writeln!(s, "case matrix-stack-{mode:x}\n{}", ctx(32).trim_end()).unwrap();
        gl(&mut s, "glMatrixMode", &[mode]);
        for k in 0..34 {
            writeln!(s, "gl glTranslatef {k} 0 0\ngl glPushMatrix\nglget i {} 1\nglerr", h(depth)).unwrap();
        }
        writeln!(s, "glget f {} 16", h(mode - 0x1700 + MODELVIEW_MATRIX)).unwrap();
        for _ in 0..35 {
            writeln!(s, "gl glPopMatrix\nglget i {} 1\nglerr", h(depth)).unwrap();
        }
        writeln!(s, "glget f {} 16\nend", h(mode - 0x1700 + MODELVIEW_MATRIX)).unwrap();
    }

    // the framebuffer: clear colors, then what each DIB depth holds
    for bpp in [16, 24, 32] {
        for i in 0..4 {
            writeln!(s, "case clear-{bpp}-{i}\n{}", ctx(bpp).trim_end()).unwrap();
            for d in [true, false] {
                if !d {
                    writeln!(s, "gl glDisable 0xbd0").unwrap();
                }
                let c: Vec<f64> = (0..4).map(|_| rf(&mut r, 0.0, 1.0)).collect();
                glf(&mut s, "glClearColor", &c);
                writeln!(s, "gl glClear 0x4000\ngldib").unwrap();
                read_all(&mut s);
            }
            writeln!(s, "glerr\nend").unwrap();
        }
    }
    // clear levels: every 8-bit level of a channel (float k/255 and k/256) on 32 bpp
    for (name, div) in [("255", 255.0), ("256", 256.0)] {
        for part in 0..4 {
            writeln!(s, "case clear-levels-{name}-{part}\n{}gl glDisable 0xbd0", ctx(32)).unwrap();
            for k in (part * 64)..(part * 64 + 64) {
                let v = k as f64 / div;
                // (k/255 is not dyadic: printed with 9 digits, read back to the nearest float)
                let t = if div == 255.0 { format!("{:.9}", v as f32) } else { f(v) };
                writeln!(s, "gl glClearColor {t} {t} {t} {t}\ngl glClear 0x4000\nglread 0 0 1 1 {} {} 4", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }

    // rectangles: which pixels a rectangle covers (integer, quarter and half coordinates)
    for i in 0..30 {
        writeln!(s, "case rect-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000", ctx(32)).unwrap();
        for _ in 0..4 {
            let q = if i < 10 { 1.0 } else if i < 20 { 4.0 } else { 2.0 };
            let c: Vec<f64> = (0..4).map(|_| r.range(-8, 24 * q as i64) as f64 / q).collect();
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            glf(&mut s, "glRectf", &c);
        }
        read_all(&mut s);
        writeln!(s, "glerr\nend").unwrap();
    }
    // points at pixel centers and in between
    for i in 0..6 {
        writeln!(s, "case points-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glBegin 0", ctx(32)).unwrap();
        for _ in 0..24 {
            let q = if i < 3 { 2.0 } else { 8.0 };
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let (x, y) = (r.range(0, 16 * q as i64 - 1) as f64 / q, r.range(0, 16 * q as i64 - 1) as f64 / q);
            let (x, y) = if i < 3 { (x.floor() + 0.5, y.floor() + 0.5) } else { (x, y) };
            glf(&mut s, "glVertex2f", &[x, y]);
        }
        writeln!(s, "gl glEnd").unwrap();
        read_all(&mut s);
        writeln!(s, "glerr\nend").unwrap();
    }

    // per-fragment operations, over random destinations drawn with glDrawPixels
    let rect_all = "gl glRectf 0 0 16 16\n";
    // blending: every factor pair, a few source colors
    for (si, src) in BLEND_SRC.iter().enumerate() {
        for (di, dst) in BLEND_DST.iter().enumerate() {
            writeln!(s, "case blend-{si}-{di}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbe2", ctx(32)).unwrap();
            gl(&mut s, "glBlendFunc", &[*src, *dst]);
            for k in 0..2 {
                writeln!(s, "gl glDisable 0xbe2").unwrap();
                random_dst(&mut s, 100 + (si * 16 + di) as u32 * 2 + k);
                writeln!(s, "gl glEnable 0xbe2").unwrap();
                let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
                gl(&mut s, "glColor4ub", &col);
                s += rect_all;
                read_all(&mut s);
            }
            writeln!(s, "glerr\nend").unwrap();
        }
    }
    // blending arithmetic: SRC_ALPHA / ONE_MINUS_SRC_ALPHA at every source alpha
    for part in 0..8 {
        writeln!(s, "case blend-alpha-{part}\n{}{ORTHO}gl glDisable 0xbd0\ngl glBlendFunc 0x302 0x303", ctx(32)).unwrap();
        for k in 0..32 {
            let a = part * 32 + k;
            writeln!(s, "gl glDisable 0xbe2").unwrap();
            random_dst(&mut s, 5000 + a);
            writeln!(s, "gl glEnable 0xbe2").unwrap();
            gl(&mut s, "glColor4ub", &[r.next() & 255, r.next() & 255, r.next() & 255, a]);
            s += rect_all;
            read_all(&mut s);
        }
        writeln!(s, "end").unwrap();
    }
    // logic operations
    writeln!(s, "case logicop\n{}{ORTHO}gl glDisable 0xbd0", ctx(32)).unwrap();
    for op in 0x1500..=0x150f {
        writeln!(s, "gl glDisable 0xbf2").unwrap();
        random_dst(&mut s, 300 + op);
        writeln!(s, "gl glEnable 0xbf2").unwrap();
        gl(&mut s, "glLogicOp", &[op]);
        gl(&mut s, "glColor4ub", &[0x5a, 0xc3, 0x0f, 0x99]);
        s += rect_all;
        read_all(&mut s);
    }
    writeln!(s, "glerr\nend").unwrap();
    // alpha test: every function, references around the source alpha
    for (fi, fu) in FUNCS.iter().enumerate() {
        writeln!(s, "case alphatest-{fi}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbc0", ctx(32)).unwrap();
        for k in 0..8 {
            let reference = *r.pick(&[0.0, 1.0, 0.5, 0.25, 0.75, 0.501_960_784, 0.498_039_216, 0.003_921_569]);
            writeln!(s, "gl glAlphaFunc {} {}", h(*fu), reference).unwrap();
            writeln!(s, "gl glDisable 0xbc0").unwrap();
            random_dst(&mut s, 400 + fi as u32 * 8 + k);
            writeln!(s, "gl glEnable 0xbc0").unwrap();
            // (a column of 16 source alphas per rectangle row)
            for y in 0..16 {
                let a = *r.pick(&[0, 1, 63, 64, 127, 128, 129, 191, 192, 254, 255]);
                gl(&mut s, "glColor4ub", &[r.next() & 255, r.next() & 255, r.next() & 255, a]);
                writeln!(s, "gl glRecti 0 {y} 16 {}", y + 1).unwrap();
            }
            read_all(&mut s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // depth test: quads at depths, every function, depth mask
    for (fi, fu) in FUNCS.iter().enumerate() {
        writeln!(s, "case depth-{fi}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71", ctx(32)).unwrap();
        writeln!(s, "gl glClearDepth {}\ngl glClear 0x4100", f(*r.pick(&[0.5, 1.0, 0.25]))).unwrap();
        for k in 0..6 {
            gl(&mut s, "glDepthFunc", &[if k == 0 { ALWAYS } else { *fu }]);
            gl(&mut s, "glDepthMask", &[(k != 3) as u32]);
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let z = *r.pick(&[0.0, 0.5, -0.5, 0.25, -0.25, 0.75, -1.0, 1.0]);
            let (x0, y0) = (r.range(0, 8), r.range(0, 8));
            let (x1, y1) = (x0 + r.range(4, 12), y0 + r.range(4, 12));
            writeln!(s, "gl glBegin 7").unwrap();
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                writeln!(s, "gl glVertex3f {x} {y} {}", f(z)).unwrap();
            }
            writeln!(s, "gl glEnd").unwrap();
        }
        read_all(&mut s);
        writeln!(s, "glread 0 0 16 16 {} {} 1024\nglerr\nend", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
    }
    // stencil: functions, operations, masks
    for i in 0..16 {
        writeln!(s, "case stencil-{i}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb90", ctx(32)).unwrap();
        writeln!(s, "gl glClearStencil {}\ngl glClear 0x4400", r.range(0, 255)).unwrap();
        for _ in 0..6 {
            gl(&mut s, "glStencilFunc", &[*r.pick(&FUNCS), *r.pick(&[0, 1, 2, 127, 128, 255, 256]), *r.pick(&[0xff, 0x0f, 0xf0, 0xffff_ffff, 1])]);
            gl(&mut s, "glStencilOp", &[*r.pick(SOPS), *r.pick(SOPS), *r.pick(SOPS)]);
            gl(&mut s, "glStencilMask", &[*r.pick(&[0xff, 0x0f, 0xf0, 0xffff_ffff, 0])]);
            let col: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            gl(&mut s, "glColor4ub", &col);
            let (x0, y0) = (r.range(0, 10), r.range(0, 10));
            writeln!(s, "gl glRecti {x0} {y0} {} {}", x0 + r.range(2, 10), y0 + r.range(2, 10)).unwrap();
        }
        read_all(&mut s);
        writeln!(s, "glread 0 0 16 16 {} {} 256\nglerr\nend", h(STENCIL_INDEX), h(UNSIGNED_BYTE)).unwrap();
    }
    // scissor and color mask
    for i in 0..8 {
        writeln!(s, "case scissor-mask-{i}\n{}{ORTHO}gl glDisable 0xbd0", ctx(*r.pick(&[16, 24, 32]))).unwrap();
        random_dst(&mut s, 600 + i);
        for _ in 0..3 {
            let (x, y) = (r.range(-4, 14), r.range(-4, 14));
            writeln!(s, "gl glEnable 0xc11\ngl glScissor {x} {y} {} {}", r.range(0, 12), r.range(0, 12)).unwrap();
            let m: Vec<u32> = (0..4).map(|_| r.range(0, 1) as u32).collect();
            gl(&mut s, "glColorMask", &m);
            let c: Vec<u32> = (0..4).map(|_| r.next() & 255).collect();
            if r.chance(2) {
                gl(&mut s, "glColor4ub", &c);
                s += rect_all;
            } else {
                glf(&mut s, "glClearColor", &c.iter().map(|v| *v as f64 / 256.0).collect::<Vec<_>>());
                writeln!(s, "gl glClear 0x4000").unwrap();
            }
        }
        read_all(&mut s);
        writeln!(s, "gldib\nglerr\nend").unwrap();
    }
    // dithering on 16 bpp: a smooth gradient of colors with dithering on and off
    for (i, d) in [true, false].iter().enumerate() {
        writeln!(s, "case dither-{i}\n{}{ORTHO}gl {} 0xbd0", ctx(16), if *d { "glEnable" } else { "glDisable" }).unwrap();
        for y in 0..16 {
            for x in 0..16 {
                gl(&mut s, "glColor4ub", &[x * 16 + y, y * 16 + x, (x * y) & 255, 255]);
                writeln!(s, "gl glRecti {x} {y} {} {}", x + 1, y + 1).unwrap();
            }
        }
        writeln!(s, "gldib").unwrap();
        read_all(&mut s);
        writeln!(s, "end").unwrap();
    }

    // glReadPixels: every format and type over a random picture, then the pack parameters
    writeln!(s, "case read-formats\n{}gl glDisable 0xbd0", ctx(32)).unwrap();
    random_dst(&mut s, 700);
    for fm in READ_FORMATS {
        for ty in TYPES {
            writeln!(s, "glread 1 2 3 2 {} {} 128\nglerr", h(*fm), h(*ty)).unwrap();
        }
    }
    writeln!(s, "glread 0 0 16 16 {} {} 1024\nglread 0 0 16 16 {} {} 2048\nend", h(RGBA), h(FLOAT), h(RGB), h(UNSIGNED_SHORT)).unwrap();
    for i in 0..24 {
        writeln!(s, "case read-pack-{i}\n{}gl glDisable 0xbd0", ctx(32)).unwrap();
        random_dst(&mut s, 800 + i);
        for _ in 0..3 {
            for (p, vals) in [(PACK_ALIGNMENT, &[1u32, 2, 4, 8][..]), (PACK_ROW_LENGTH, &[0, 0, 5, 17]), (PACK_SKIP_ROWS, &[0, 0, 1, 3]), (PACK_SKIP_PIXELS, &[0, 0, 1, 2]), (PACK_SWAP_BYTES, &[0, 0, 1]), (PACK_LSB_FIRST, &[0, 0, 1])] {
                gl(&mut s, "glPixelStorei", &[p, *r.pick(vals)]);
            }
            let (x, y) = (r.range(0, 12), r.range(0, 12));
            let (w, ht) = (r.range(1, 5), r.range(1, 4));
            let fm = *r.pick(&[RGB, RGBA, LUMINANCE, ALPHA, BGRA_EXT, RED, LUMINANCE_ALPHA]);
            let ty = *r.pick(&[UNSIGNED_BYTE, BYTE, UNSIGNED_SHORT, SHORT, INT, FLOAT, BITMAP]);
            writeln!(s, "glread {x} {y} {w} {ht} {} {} 400\nglerr", h(fm), h(ty)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // glDrawPixels: every format and type, then the unpack parameters and the pixel transfer
    let draw_formats = [RED, GREEN, BLUE, ALPHA, RGB, RGBA, LUMINANCE, LUMINANCE_ALPHA, BGR_EXT, BGRA_EXT];
    for (fi, fm) in draw_formats.iter().enumerate() {
        writeln!(s, "case draw-{fi}\n{}gl glDisable 0xbd0", ctx(32)).unwrap();
        for (ti, ty) in TYPES[..7].iter().enumerate() {
            writeln!(s, "gl glClear 0x4000\ngldraw 16 2 {} {} {} 2048\nglerr", h(*fm), h(*ty), 900 + fi * 8 + ti).unwrap();
            writeln!(s, "glread 0 0 16 2 {} {} 128", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for i in 0..16 {
        writeln!(s, "case draw-unpack-{i}\n{}gl glDisable 0xbd0\ngl glClear 0x4000", ctx(32)).unwrap();
        for (p, vals) in [(UNPACK_ALIGNMENT, &[1u32, 2, 4, 8][..]), (UNPACK_ROW_LENGTH, &[0, 0, 7, 20]), (UNPACK_SKIP_ROWS, &[0, 0, 2]), (UNPACK_SKIP_PIXELS, &[0, 0, 1, 3]), (UNPACK_SWAP_BYTES, &[0, 0, 1])] {
            gl(&mut s, "glPixelStorei", &[p, *r.pick(vals)]);
        }
        if i >= 8 {
            for p in [RED_SCALE, GREEN_BIAS, BLUE_SCALE, ALPHA_BIAS] {
                writeln!(s, "gl glPixelTransferf {} {}", h(p), f(*r.pick(&[0.5, 2.0, -0.25, 0.25, 1.0]))).unwrap();
            }
        }
        let fm = *r.pick(&[RGB, RGBA, LUMINANCE, BGRA_EXT]);
        let ty = *r.pick(&[UNSIGNED_BYTE, UNSIGNED_SHORT, SHORT, FLOAT]);
        writeln!(s, "gldraw {} {} {} {} {} 4096\nglerr", r.range(1, 9), r.range(1, 6), h(fm), h(ty), 1000 + i).unwrap();
        read_all(&mut s);
        writeln!(s, "end").unwrap();
    }
    s
}
