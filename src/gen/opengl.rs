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
        ("draw-depth", "gldraw 2 1 0x1902 0x1405 1 16\nglerr\nglread 0 0 2 1 0x1902 0x1405 8"),
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
            writeln!(s, "case edge-{name}\n{}{body}\nend", ctx(32)).unwrap();
        } else {
            writeln!(s, "case edge-{name}\n{pre}\nend").unwrap();
        }
    }
    s
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
        writeln!(s, "case {name}\n{}{ORTHO}gl glDisable 0xbd0", ctx(bpp)).unwrap();
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
            writeln!(s, "gl glClear 0x4000").unwrap();
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
            writeln!(s, "gl glClear 0x4000").unwrap();
            color(s, &mut q);
            writeln!(s, "gl glBegin 7").unwrap();
            for (x, y) in v {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // vertical edges through pixel centers, against the horizontal ones
    for k in 0..4 {
        start(s, format!("tie-x-{k}"), 32);
        for _ in 0..8 {
            let x0 = q.range(-2, 14) as f64 + 0.5;
            let x1 = x0 + q.range(1, 6) as f64 + *q.pick(&[0.0, 0.25, 0.5]);
            let (y0, y1) = (v8(&mut q, -2, 8), v8(&mut q, 9, 18));
            writeln!(s, "gl glClear 0x4000").unwrap();
            color(s, &mut q);
            glf(s, "glRectf", &[x0, y0, x1, y1]);
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // flat triangles at subpixel positions, both windings
    for k in 0..16 {
        start(s, format!("tri-{k}"), 32);
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4000").unwrap();
            color(s, &mut q);
            writeln!(s, "gl glBegin 4").unwrap();
            for _ in 0..3 {
                let (x, y) = if k < 4 { (q.range(-2, 18) as f64 + 0.5 * q.range(0, 1) as f64, q.range(-2, 18) as f64 + 0.5 * q.range(0, 1) as f64) } else { (v8(&mut q, -3, 19), v8(&mut q, -3, 19)) };
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // the other primitives: points of several sizes, lines, strips, loops, fans, quad strips, polygons
    for (k, (mode, n, extra)) in [(0u32, 12, "gl glPointSize 2\n"), (0, 12, "gl glPointSize 3\n"), (0, 12, "gl glPointSize 1.5\n"), (1, 8, ""), (1, 8, ""), (1, 8, "gl glLineWidth 2\n"), (1, 8, "gl glLineWidth 3\n"), (2, 6, ""), (3, 6, ""), (5, 6, ""), (6, 6, ""), (8, 8, ""), (9, 6, ""), (9, 5, ""), (1, 8, "gl glLineWidth 1.5\n"), (3, 8, "")]
        .iter()
        .enumerate()
    {
        start(s, format!("prim-{k}"), 32);
        s.push_str(extra);
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000").unwrap();
            color(s, &mut q);
            writeln!(s, "gl glBegin {mode}").unwrap();
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
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // polygon modes and culling
    for (k, setup) in ["gl glPolygonMode 0x408 0x1b01", "gl glPolygonMode 0x408 0x1b00", "gl glEnable 0xb44", "gl glEnable 0xb44\ngl glCullFace 0x404", "gl glEnable 0xb44\ngl glFrontFace 0x900\ngl glCullFace 0x408", "gl glPolygonMode 0x404 0x1b01\ngl glPolygonMode 0x405 0x1b00"].iter().enumerate() {
        start(s, format!("polymode-{k}"), 32);
        writeln!(s, "{setup}").unwrap();
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000").unwrap();
            color(s, &mut q);
            writeln!(s, "gl glBegin 4").unwrap();
            for _ in 0..3 {
                glf(s, "glVertex2f", &[v8(&mut q, -1, 17), v8(&mut q, -1, 17)]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // smooth shading and depth across triangles, lines and quads; flat shading takes the last vertex
    for k in 0..12 {
        let bpp = [32, 32, 32, 32, 32, 32, 24, 24, 16, 16, 32, 32][k];
        start(s, format!("smooth-{k}"), bpp);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glClear 0x4100").unwrap();
        if k >= 10 {
            writeln!(s, "gl glShadeModel 0x1d00").unwrap();
        }
        let mode = [4u32, 4, 4, 4, 7, 1, 4, 7, 4, 7, 4, 7][k];
        for _ in 0..3 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin {mode}").unwrap();
            for _ in 0..[3, 3, 3, 3, 4, 2, 3, 4, 3, 4, 3, 4][k] {
                color(s, &mut q);
                glf(s, "glVertex3f", &[v8(&mut q, -2, 18), v8(&mut q, -2, 18), q.range(-32, 32) as f64 / 32.0]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // perspective: frustums, triangles in depth, colors and depth read back
    for k in 0..6 {
        writeln!(s, "case persp-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32)).unwrap();
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, [3.0, 10.0, 2.0, 100.0, 4.0, 8.0][k]]);
        writeln!(s, "gl glMatrixMode 0x1700").unwrap();
        if k >= 4 {
            glf(s, "glRotatef", &[30.0, 0.0, 1.0, 0.0]);
        }
        for _ in 0..3 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k % 2 == 0 { 4 } else { 7 }).unwrap();
            for _ in 0..if k % 2 == 0 { 3 } else { 4 } {
                color(s, &mut q);
                glf(s, "glVertex3f", &[q.range(-32, 32) as f64 / 16.0, q.range(-32, 32) as f64 / 16.0, -(q.range(17, 40) as f64) / 16.0]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // clipping against the six planes: triangles past the window and through near and far
    for k in 0..6 {
        start(s, format!("clip-{k}"), 32);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x207").unwrap();
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4").unwrap();
            for _ in 0..3 {
                color(s, &mut q);
                glf(s, "glVertex3f", &[v8(&mut q, -24, 40), v8(&mut q, -24, 40), q.range(-64, 64) as f64 / 32.0]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // dithering: whole-window rectangles of one color on 16, 8 and 4-bit DIBs
    for (k, (bpp, pf)) in [(16u32, 0u32), (16, 0), (16, 0), (16, 0), (8, 25), (8, 25), (4, 31), (4, 31)].iter().enumerate() {
        if *pf == 0 {
            writeln!(s, "case dither-fill-{k}\n{}{ORTHO}gl glEnable 0xbd0", ctx(*bpp)).unwrap();
        } else {
            writeln!(s, "case dither-fill-{k}\nglctx {bpp} {pf}\n{ORTHO}gl glEnable 0xbd0").unwrap();
        }
        for _ in 0..16 {
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib").unwrap();
        }
        writeln!(s, "gl glDisable 0xbd0").unwrap();
        for _ in 0..4 {
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib").unwrap();
        }
        for _ in 0..4 {
            glf(s, "glClearColor", &[(q.next() & 255) as f64 / 256.0, (q.next() & 255) as f64 / 256.0, (q.next() & 255) as f64 / 256.0, 1.0]);
            writeln!(s, "gl glEnable 0xbd0\ngl glClear 0x4000\ngldib\ngl glDisable 0xbd0\ngl glClear 0x4000\ngldib").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // color index: 8 and 4-bit formats, indices cleared, drawn, masked, read back, and the DIB
    for (k, (bpp, pf)) in [(8u32, 29u32), (8, 30), (4, 35), (8, 29)].iter().enumerate() {
        writeln!(s, "case ci-{k}\nglctx {bpp} {pf}\n{ORTHO}glget i 0xd51 1\nglget i 0xc20 1\nglget i 0xc21 1\nglget f 0xb00 4\nglget i 0xb01 1").unwrap();
        for i in 0..6 {
            let v = q.range(0, 300);
            writeln!(s, "gl glClearIndex {v}\ngl glClear 0x4000\ngldib").unwrap();
            writeln!(s, "gl glIndexi {}\ngl glRectf {} {} {} {}", q.range(0, 300), v8(&mut q, 0, 6), v8(&mut q, 0, 6), v8(&mut q, 9, 16), v8(&mut q, 9, 16)).unwrap();
            if i == 3 {
                writeln!(s, "gl glIndexMask 0xf0").unwrap();
            }
            if i == 4 {
                writeln!(s, "gl glEnable 0xbf1\ngl glLogicOp 0x1506").unwrap();
            }
            writeln!(s, "gldib\nglread 0 0 16 16 {} {} 256\nglread 0 0 16 16 {} {} 1024\nglerr", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "gldraw 16 2 {} {} 77 32\ngldib\nglerr\nend", h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    }
    // the accumulation buffer: load, accumulate, multiply, add, return
    for k in 0..6 {
        start(s, format!("accum-{k}"), [32, 32, 32, 16, 24, 32][k]);
        writeln!(s, "gl glClear 0x200").unwrap();
        for _ in 0..4 {
            random_dst(s, q.next() & 0xffff);
            let op = *q.pick(&[0x100u32, 0x101]);
            writeln!(s, "gl glAccum {} {}", h(op), f(q.range(-8, 16) as f64 / 8.0)).unwrap();
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gl glAccum 0x100 {}\ngl glAccum {} {}", f(q.range(-8, 16) as f64 / 8.0), h(*q.pick(&[0x103u32, 0x104])), f(q.range(-8, 16) as f64 / 16.0)).unwrap();
            writeln!(s, "gl glAccum 0x102 {}", f(q.range(0, 32) as f64 / 8.0)).unwrap();
            read_all(s);
            writeln!(s, "glerr").unwrap();
        }
        if k == 5 {
            writeln!(s, "gl glClearAccum 0.25 -0.5 1 0.75\ngl glClear 0x200\ngl glAccum 0x102 1").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // the attribute stack: state changed between push and pop, then read back
    for (k, mask) in [0x000fffffu32, 0x1, 0x8, 0x20, 0x100, 0x400, 0x1000, 0x4000, 0x2000, 0x800, 0x200, 0x8000].iter().enumerate() {
        start(s, format!("attrib-{k}"), 32);
        writeln!(s, "gl glPushAttrib {}", h(*mask)).unwrap();
        writeln!(s, "gl glColor4ub 1 2 3 4\ngl glEnable 0xbe2\ngl glBlendFunc 0x302 0x303\ngl glDepthFunc 0x203\ngl glEnable 0xc11\ngl glScissor 1 2 3 4\ngl glViewport 1 1 8 8\ngl glClearColor 0.5 0.5 0.5 0.5\ngl glStencilFunc 0x202 1 3\ngl glLineWidth 3\ngl glPointSize 4\ngl glShadeModel 0x1d00\ngl glHint 0xc50 0x1101\ngl glLogicOp 0x1506\ngl glColorMask 1 0 1 0\ngl glDepthMask 0\ngl glMatrixMode 0x1701\ngl glPolygonMode 0x408 0x1b01\ngl glPixelTransferf 0xd14 0.5\ngl glAlphaFunc 0x204 0.5\ngl glEnable 0xbc0\ngl glDisable 0xbd0\ngl glEnable 0xb71\ngl glCullFace 0x404\ngl glPopAttrib\nglerr").unwrap();
        for p in [0xb00u32, 0xbe2, 0xbe0, 0xbe1, 0xb74, 0xc11, 0xc10, 0xba2, 0xc22, 0xb92, 0xb21, 0xb11, 0xb54, 0xc50, 0xbf0, 0xc23, 0xb72, 0xba0, 0xb40, 0xd14, 0xbc1, 0xbc2, 0xbc0, 0xbd0, 0xb71, 0xb45, 0xbb0] {
            writeln!(s, "glget f {} 4", h(p)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // 16-bit depth buffers: tests at every function, clears, reads in every type
    for k in 0..4 {
        writeln!(s, "case depth16-{k}\nglctx 32 2\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71").unwrap();
        glf(s, "glClearDepth", &[[1.0, 0.5, 0.25, 0.75][k]]);
        writeln!(s, "gl glClear 0x4100").unwrap();
        for _ in 0..6 {
            writeln!(s, "gl glDepthFunc {}", h(*q.pick(&FUNCS[1..]))).unwrap();
            color(s, &mut q);
            let z = q.range(-64, 64) as f64 / 64.0;
            glf(s, "glRectf", &[v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17)]);
            let _ = z;
            writeln!(s, "gl glBegin 7").unwrap();
            let (x0, y0, x1, y1) = (v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17));
            let z = q.range(-64, 64) as f64 / 64.0;
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                glf(s, "glVertex3f", &[x, y, z]);
            }
            writeln!(s, "gl glEnd").unwrap();
        }
        read_all(s);
        for ty in [UNSIGNED_INT, UNSIGNED_SHORT, UNSIGNED_BYTE, FLOAT, INT] {
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(ty)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // GL_NEVER: single rectangles on a clear picture
    for k in 0..4 {
        start(s, format!("never-{k}"), 32);
        writeln!(s, "gl glEnable 0xb71\ngl glDepthFunc 0x200").unwrap();
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100").unwrap();
            color(s, &mut q);
            glf(s, "glRectf", &[v8(&mut q, -1, 8), v8(&mut q, -1, 8), v8(&mut q, 8, 17), v8(&mut q, 8, 17)]);
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // blending on 16 and 24-bit DIBs and with destination alpha
    for (k, (bpp, choose_alpha)) in [(16u32, 0u32), (24, 0), (32, 8), (16, 0), (24, 0), (32, 8)].iter().enumerate() {
        writeln!(s, "case blend-fmt-{k}\nglctx {bpp} choose 0x38 0 {bpp} {choose_alpha} 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xbe2").unwrap();
        for _ in 0..12 {
            let (sf, df) = (*q.pick(BLEND_SRC), *q.pick(BLEND_DST));
            gl(s, "glBlendFunc", &[sf, df]);
            writeln!(s, "gl glDisable 0xbe2").unwrap();
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gl glEnable 0xbe2").unwrap();
            let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
            gl(s, "glColor4ub", &c);
            writeln!(s, "gl glRectf 0 0 16 16").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // points on pixel boundaries
    for k in 0..2 {
        start(s, format!("point-edge-{k}"), 32);
        for _ in 0..8 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 0").unwrap();
            for _ in 0..4 {
                color(s, &mut q);
                let (x, y) = (q.range(0, 32) as f64 / 2.0, q.range(0, 32) as f64 / 2.0);
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
    // each accumulation operation alone, between a load and a return, on every depth and with
    // alpha; then clears to values returned
    for fmt in ["32", "24", "16", "8", "4", "alpha"].iter() {
        let head = if *fmt == "alpha" { "glctx 32 choose 0x38 0 32 8 0 32 8\n".to_string() } else { ctx(fmt.parse().unwrap()) };
        for (ok, op) in ["load", "accum", "mult", "add"].iter().enumerate() {
            writeln!(s, "case accum-{op}-{fmt}\n{head}{ORTHO}gl glDisable 0xbd0").unwrap();
            for _ in 0..6 {
                random_dst(s, q.next() & 0xffff);
                writeln!(s, "gl glAccum 0x101 {}", f(q.range(-8, 17) as f64 / 8.0)).unwrap();
                match ok {
                    1 => {
                        random_dst(s, q.next() & 0xffff);
                        writeln!(s, "gl glAccum 0x100 {}", f(q.range(-16, 17) as f64 / 8.0)).unwrap();
                    }
                    2 => writeln!(s, "gl glAccum 0x103 {}", f(q.range(-24, 25) as f64 / 16.0)).unwrap(),
                    3 => writeln!(s, "gl glAccum 0x104 {}", f(q.range(-16, 17) as f64 / 16.0 + q.range(0, 8) as f64 / 1024.0)).unwrap(),
                    _ => {}
                }
                writeln!(s, "gl glAccum 0x102 {}", f(q.range(0, 33) as f64 / 8.0)).unwrap();
                read_all(s);
            }
            writeln!(s, "glerr\nend").unwrap();
        }
        writeln!(s, "case accum-clear-{fmt}\n{head}{ORTHO}gl glDisable 0xbd0").unwrap();
        for _ in 0..6 {
            let c: Vec<String> = (0..4).map(|_| f(q.range(-4, 1025) as f64 / 1024.0)).collect();
            writeln!(s, "gl glClearAccum {}\ngl glClear 0x200\ngl glAccum 0x102 {}", c.join(" "), f(q.range(4, 17) as f64 / 8.0)).unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // the tie rule under other viewports and object coordinates: a column (row) one pixel wide
    // whose edges both pass through pixel centers, at every column (row), read across
    for (k, (vp, o)) in [([0, 0, 16, 16], [0.0, 16.0]), ([0, 0, 16, 16], [-8.0, 8.0]), ([0, 0, 16, 16], [0.0, 32.0]), ([4, 4, 16, 16], [0.0, 16.0]), ([-4, -4, 16, 16], [0.0, 16.0]), ([0, 0, 32, 32], [0.0, 32.0]), ([0, 0, 8, 8], [0.0, 8.0]), ([-8, -8, 32, 32], [-1.0, 1.0])].iter().enumerate() {
        writeln!(s, "case tie-vp-{k}\n{}gl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {} {} {} {} -1 1\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\ngl glViewport {} {} {} {}", ctx(32), f(o[0]), f(o[1]), f(o[0]), f(o[1]), vp[0], vp[1], vp[2], vp[3]).unwrap();
        // window coordinate to object coordinate on axis `a`
        let obj = |w: f64, a: usize| o[0] + (w - vp[a] as f64) / vp[a + 2] as f64 * (o[1] - o[0]);
        for c in 0..16 {
            let (w0, w1) = (c as f64 + 0.5, c as f64 + 1.5);
            writeln!(s, "gl glClear 0x4000").unwrap();
            glf(s, "glRectf", &[obj(w0, 0), obj(-1.0, 1), obj(w1, 0), obj(17.0, 1)]);
            writeln!(s, "glread 0 8 16 1 {} {} 64", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
            writeln!(s, "gl glClear 0x4000").unwrap();
            glf(s, "glRectf", &[obj(-1.0, 0), obj(w0, 1), obj(17.0, 0), obj(w1, 1)]);
            writeln!(s, "glread 8 0 1 16 {} {} 64", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // glGetString between glBegin and glEnd; glAccum without an accumulation buffer (color index)
    writeln!(s, "case begin-string\n{}gl glBegin 7\nglstr 0x1f00\nglstr 0x1f03\ngl glEnd\nglerr\nend", ctx(32)).unwrap();
    writeln!(s, "case accum-none\nglctx 32 9\ngl glAccum 0x101 1\nglerr\ngl glAccum 0x102 1\nglerr\ngl glClear 0x200\nglerr\nend").unwrap();
    // depth reads in every type, 32 and 16 depth bits: clears and rectangles at several depths
    for (k, pf) in [(0, "glctx 32 choose 0x38 0 32 0 0 32 8"), (1, "glctx 32 2")] {
        writeln!(s, "case depth-types-{k}\n{pf}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207").unwrap();
        for (j, cz) in [1.0, 0.0, 0.5, 0.25, 0.123456, 0.999].iter().enumerate() {
            glf(s, "glClearDepth", &[*cz]);
            writeln!(s, "gl glClear 0x100").unwrap();
            for i in 0..4 {
                let z = [-1.0, 1.0, 0.0, 0.3, -0.7, 0.9999, -0.0001, 0.5][(i + j) % 8];
                writeln!(s, "gl glBegin 7").unwrap();
                for (x, y) in [(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)] {
                    glf(s, "glVertex3f", &[x + (i % 2) as f64 * 8.0, y + (i / 2) as f64 * 8.0 - 4.0, z]);
                }
                writeln!(s, "gl glEnd").unwrap();
            }
            for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
                let n = 256 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
                writeln!(s, "glread 0 0 16 16 {} {} {n}", h(DEPTH_COMPONENT), h(t)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // GL_NEVER again: which rectangles are drawn all the same (some clipped ones are), on 32
    // and 16 depth bits, with the depth mask off
    let mut q = XorShift32::new(0x0960_2002);
    for (k, (pf, extra)) in [("glctx 32 choose 0x38 0 32 0 0 32 8", ""), ("glctx 32 choose 0x38 0 32 0 0 32 8", "gl glDepthMask 0\n"), ("glctx 32 2", ""), ("glctx 32 choose 0x38 0 32 0 0 32 8", "gl glDepthFunc 0x201\n")].iter().enumerate() {
        write!(s, "case never-more-{k}\n{pf}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x200\n{extra}").unwrap();
        for _ in 0..24 {
            writeln!(s, "gl glClear 0x4100").unwrap();
            color(s, &mut q);
            glf(s, "glRectf", &[v8(&mut q, -2, 8), v8(&mut q, -2, 8), v8(&mut q, 8, 18), v8(&mut q, 8, 18)]);
            read_all(s);
        }
        writeln!(s, "end").unwrap();
    }
}
