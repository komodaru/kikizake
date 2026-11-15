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
    // (the stencil and depth buffers cleared to values: before their first clear they hold what the
    // memory held, the stencil 0xbe on one executor and 1 on the other in this case's first answers)
    writeln!(s, "case read-formats\n{}gl glDisable 0xbd0\ngl glClearStencil 0xb5\ngl glClearDepth 0.75\ngl glClear 0x500", ctx(32)).unwrap();
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
    // clears of the accumulation buffer one pixel at a time: values everywhere in -1..1 and at
    // the ends, returned at several scales
    let mut q = XorShift32::new(0x0960_2003);
    for fmt in ["32", "24", "16"] {
        writeln!(s, "case accum-clear-fine-{fmt}\n{}{ORTHO}gl glDisable 0xbd0", ctx(fmt.parse().unwrap())).unwrap();
        for i in 0..48 {
            let c: Vec<String> = (0..4).map(|k| match (i, k) {
                (0, _) => f(1.0),
                (1, _) => f(-1.0),
                (2, _) => f(0.5),
                _ => f((q.next() % 4097) as f64 / 4096.0),
            }).collect();
            let v = [1.0, 1.0, 1.0, 0.5, 2.0, 1.5][i % 6];
            writeln!(s, "gl glClearAccum {}\ngl glClear 0x200\ngl glAccum 0x102 {}\nglread 0 0 1 1 {} {} 4", c.join(" "), f(v), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // GL_RETURN of an accumulation buffer never loaded or cleared, twice, on each depth
    for bpp in [32, 24, 16, 8, 4] {
        writeln!(s, "case accum-undefined-{bpp}\n{}{ORTHO}gl glDisable 0xbd0\ngl glAccum 0x102 1\nglread 0 0 16 16 {} {} 1024\ngl glAccum 0x103 2\ngl glAccum 0x102 1\nglread 0 0 16 16 {} {} 1024\nglerr\nend", ctx(bpp), h(RGBA), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
    }
    // many triangles, each read alone in red: vertices on the eighth grid (cases 0..5) and
    // anywhere (6..11), both windings, some past the window
    let mut q = XorShift32::new(0x0960_2004);
    for k in 0..12 {
        writeln!(s, "case tri-many-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255", ctx(32)).unwrap();
        for _ in 0..32 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            for _ in 0..3 {
                let (x, y) = if k < 6 { (v8(&mut q, -2, 18), v8(&mut q, -2, 18)) } else { (q.range(-2048, 18 * 1024) as f64 / 1024.0, q.range(-2048, 18 * 1024) as f64 / 1024.0) };
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // dithering of glDrawPixels and of fragments under a color mask, 16, 8 and 4 bits
    let mut q = XorShift32::new(0x0960_2005);
    for (k, (bpp, pf)) in [(16u32, 0u32), (8, 25), (4, 31)].iter().enumerate() {
        let head = if *pf == 0 { ctx(*bpp) } else { format!("glctx {bpp} {pf}\n") };
        writeln!(s, "case dither-draw-{k}\n{head}{ORTHO}gl glEnable 0xbd0").unwrap();
        for i in 0..6 {
            random_dst(s, q.next() & 0xffff);
            writeln!(s, "gldib").unwrap();
            gl(s, "glColorMask", &[(i & 1) as u32, 1, ((i >> 1) & 1) as u32, 1]);
            color(s, &mut q);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib\ngl glColorMask 1 1 1 1").unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // color index elements of every type, drawn and read back, at 8 and 4 bits
    for (k, (bpp, pf)) in [(8u32, 29u32), (4, 35)].iter().enumerate() {
        write!(s, "case ci-types-{k}\nglctx {bpp} {pf}\n{ORTHO}").unwrap();
        for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
            let n = 32 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
            writeln!(s, "gl glClear 0x4000\ngldraw 16 2 {} {} {} {n}\ngldib", h(COLOR_INDEX), h(t), 100 + t).unwrap();
            for rt in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
                let m = 32 * [1, 1, 2, 2, 4, 4, 4][(rt - BYTE) as usize];
                writeln!(s, "glread 0 0 16 2 {} {} {m}", h(COLOR_INDEX), h(rt)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // blending with destination alpha: every source and destination factor pair, two random
    // pictures each under one random color
    let mut q = XorShift32::new(0x0960_2006);
    for (i, sf) in BLEND_SRC.iter().enumerate() {
        writeln!(s, "case blend-dstalpha-{i}\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0").unwrap();
        for df in BLEND_DST {
            for _ in 0..2 {
                writeln!(s, "gl glDisable 0xbe2").unwrap();
                random_dst(s, q.next() & 0xffff);
                gl(s, "glBlendFunc", &[*sf, *df]);
                writeln!(s, "gl glEnable 0xbe2").unwrap();
                let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
                gl(s, "glColor4ub", &c);
                writeln!(s, "gl glRectf 0 0 16 16").unwrap();
                read_all(s);
            }
        }
        writeln!(s, "end").unwrap();
    }
    // glPixelZoom: an 8 x 8 image drawn at several zooms, down and up, whole and fractional
    let mut q = XorShift32::new(0x0960_2007);
    writeln!(s, "case pixel-zoom\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0").unwrap();
    for z in ["2 2", "0.5 0.5", "3 1.5", "1.25 0.75", "0.3 2", "1.5 1", "1 2.5", "0.75 0.25"] {
        writeln!(s, "gl glClear 0x4000\ngl glPixelZoom {z}\ngldraw 8 8 {} {} {} 256", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff).unwrap();
        read_all(s);
    }
    writeln!(s, "glerr\nend").unwrap();
    // GL_MAP_COLOR with the initial maps (one entry each, 0): images drawn through them, and
    // read through them
    writeln!(s, "case transfer-map-color\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0").unwrap();
    writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd10 1\ngldraw 16 16 {} {} {} 1024", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff).unwrap();
    read_all(s);
    writeln!(s, "gl glPixelTransferi 0xd10 0\ngldraw 16 16 {} {} {} 1024", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff).unwrap();
    read_all(s);
    writeln!(s, "gl glPixelTransferi 0xd10 1").unwrap();
    read_all(s);
    writeln!(s, "glread 0 0 16 16 {} {} 1024", h(RGBA), h(FLOAT)).unwrap();
    writeln!(s, "glerr\nend").unwrap();
    // GL_BITMAP images of color indices drawn in color index mode under the unpack parameters,
    // and indices and stencil values read back as GL_BITMAP under the pack ones (13 pixels a row:
    // a partial last byte)
    let mut q = XorShift32::new(0x0960_2008);
    write!(s, "case bitmap-index-0\nglctx 8 29\n{ORTHO}").unwrap();
    // (store: lsb first, row length, skip rows, skip pixels, alignment)
    for (lsb, rl, sr, sp, al) in [(0, 0, 0, 0, 4), (1, 0, 0, 0, 1), (0, 20, 1, 3, 2), (1, 16, 0, 5, 8)] {
        writeln!(s, "gl glClear 0x4000").unwrap();
        for (pn, v) in [(0xcf1, lsb), (0xcf2, rl), (0xcf3, sr), (0xcf4, sp), (0xcf5, al)] {
            writeln!(s, "gl glPixelStorei {} {v}", h(pn)).unwrap();
        }
        writeln!(s, "gldraw 13 3 {} {} {} 64\nglerr", h(COLOR_INDEX), h(BITMAP), q.next() & 0xffff).unwrap();
        writeln!(s, "glread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
        for (pn, v) in [(0xd01, lsb), (0xd02, rl), (0xd03, sr), (0xd04, sp), (0xd05, al)] {
            writeln!(s, "gl glPixelStorei {} {v}", h(pn)).unwrap();
        }
        writeln!(s, "glread 0 0 13 3 {} {} 64\nglerr", h(COLOR_INDEX), h(BITMAP)).unwrap();
        for pn in [0xd01, 0xd02, 0xd03, 0xd04] {
            writeln!(s, "gl glPixelStorei {} 0", h(pn)).unwrap();
        }
        writeln!(s, "gl glPixelStorei 0xd05 4").unwrap();
    }
    writeln!(s, "end").unwrap();
    write!(s, "case bitmap-index-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}").unwrap();
    for v in [0xa5u32, 0x5a, 0x01, 0xfe] {
        writeln!(s, "gl glClearStencil {}\ngl glClear 0x400\nglread 0 0 13 3 {} {} 64\ngl glPixelStorei 0xd01 1\nglread 2 1 13 3 {} {} 64\ngl glPixelStorei 0xd01 0", h(v), h(STENCIL_INDEX), h(BITMAP), h(STENCIL_INDEX), h(BITMAP)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    // depth and stencil images: every type, drawn through the depth test (ALWAYS, then LESS
    // over a depth already there) and under the stencil write mask, read back
    let mut q = XorShift32::new(0x0960_2009);
    write!(s, "case draw-depthstencil-0\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\n").unwrap();
    for t in [BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT] {
        let n = 128 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
        for func in [0x207u32, 0x201] {
            writeln!(s, "gl glDepthFunc {}", h(func)).unwrap();
            if func == 0x207 {
                writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x4100").unwrap();
            }
            writeln!(s, "gl glColor4ub {} {} {} 255\ngldraw 16 8 {} {} {} {n}", q.next() & 255, q.next() & 255, q.next() & 255, h(DEPTH_COMPONENT), h(t), q.next() & 0xffff).unwrap();
            writeln!(s, "glread 0 0 16 8 {} {} 512\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_INT), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
    }
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case draw-depthstencil-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\n").unwrap();
    for (t, mask) in [(BYTE, 0xffu32), (UNSIGNED_BYTE, 0xf0), (SHORT, 0xff), (UNSIGNED_SHORT, 0x3c), (INT, 0xff), (UNSIGNED_INT, 0x81), (FLOAT, 0xff)] {
        let n = 128 * [1, 1, 2, 2, 4, 4, 4][(t - BYTE) as usize];
        writeln!(s, "gl glStencilMask 0xff\ngl glClearStencil {}\ngl glClear 0x4400\ngl glStencilMask {}", h(q.next() & 255), h(mask)).unwrap();
        writeln!(s, "gldraw 16 8 {} {} {} {n}\nglread 0 0 16 8 {} {} 128\nglread 0 0 16 8 {} {} 512", h(STENCIL_INDEX), h(t), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
    }
    writeln!(s, "gl glStencilMask 0xff\ngl glClear 0x400\ngl glEnable 0xc11\ngl glScissor 3 2 7 4\ngldraw 16 8 {} {} {} 128\nglread 0 0 16 8 {} {} 128", h(STENCIL_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE)).unwrap();
    writeln!(s, "gl glDisable 0xc11\ngl glClear 0x400\ngl glPixelStorei 0xcf5 1\ngldraw 13 3 {} {} {} 64\nglread 0 0 16 4 {} {} 64", h(STENCIL_INDEX), h(BITMAP), q.next() & 0xffff, h(STENCIL_INDEX), h(UNSIGNED_BYTE)).unwrap();
    writeln!(s, "glerr\nend").unwrap();
    // the calls the model does not follow yet, each with a picture to answer it: raster
    // positions under glDrawPixels (and a negative zoom from one), polygon offset under a depth
    // test, linear fog over rectangles
    let mut q = XorShift32::new(0x0960_200a);
    write!(s, "case other-calls-0\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\n").unwrap();
    for (pos, zoom) in [("glRasterPos2i 3 5", "1 1"), ("glRasterPos2f 2.5 6.75", "1 1"), ("glRasterPos2i 12 12", "-1 -1"), ("glRasterPos2f 9.25 2.5", "-1.5 2"), ("glRasterPos2i 14 -1", "1 1")] {
        writeln!(s, "gl glClear 0x4000\ngl glColor4ub 255 255 255 255\ngl {pos}\ngl glPixelZoom {zoom}\ngldraw 8 8 {} {} {} 256", h(RGBA), h(UNSIGNED_BYTE), q.next() & 0xffff).unwrap();
        read_all(s);
    }
    writeln!(s, "gl glPixelZoom 1 1\nglerr\nend").unwrap();
    write!(s, "case other-calls-1\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x201\n").unwrap();
    for (fac, units) in [("0 0", "8 8"), ("1 1", "0 0"), ("0 -64", "0 0"), ("2 4", "-1 3")] {
        writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x4100\ngl glPolygonOffset {fac}\ngl glEnable 0x8037\ngl glColor4ub 255 0 0 255\ngl glRectf 2 2 14 14\ngl glPolygonOffset {units}\ngl glColor4ub 0 255 0 255\ngl glRectf 4 4 12 12\ngl glDisable 0x8037").unwrap();
        read_all(s);
        writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-2\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\n").unwrap();
    for (mode, a, b) in [(0x2601u32, "-1", "1"), (0x800, "0.5", "0"), (0x801, "2", "0")] {
        writeln!(s, "gl glClear 0x4000\ngl glFogf 0xb65 {mode}\ngl glFogf 0xb63 {a}\ngl glFogf 0xb64 {b}\ngl glFogf 0xb62 {a}").unwrap();
        for _ in 0..4 {
            let (x0, y0) = (q.next() % 12, q.next() % 12);
            writeln!(s, "gl glColor4ub {} {} {} 255\ngl glBegin 7\ngl glVertex3f {x0} {y0} -0.5\ngl glVertex3f {} {y0} 0\ngl glVertex3f {} {} 0.5\ngl glVertex3f {x0} {} 0.25\ngl glEnd", q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, x0 + 4, y0 + 4, y0 + 4).unwrap();
        }
        read_all(s);
    }
    writeln!(s, "glerr\nend").unwrap();
    // the pixel transfer the model does not follow yet: depth scale and bias over a depth image,
    // index shift and offset over a color index image
    write!(s, "case transfer-other\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\n").unwrap();
    for (sc, bi) in [("0.5", "0.25"), ("2", "-0.5"), ("1", "0.125")] {
        writeln!(s, "gl glClearDepth 0\ngl glClear 0x100\ngl glPixelTransferf 0xd1e {sc}\ngl glPixelTransferf 0xd1f {bi}\ngldraw 16 8 {} {} {} 512\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_SHORT), q.next() & 0xffff, h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        writeln!(s, "gl glPixelTransferf 0xd1e 1\ngl glPixelTransferf 0xd1f 0\nglread 0 0 16 8 {} {} 512", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case transfer-other-ci\nglctx 8 29\n{ORTHO}").unwrap();
    for (sh, off) in [(1, 0), (-2, 3), (3, -5), (0, 200)] {
        writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd12 {}\ngl glPixelTransferi 0xd13 {}\ngldraw 16 4 {} {} {} 64\nglread 0 0 16 4 {} {} 64", sh, off, h(COLOR_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
        writeln!(s, "gl glPixelTransferi 0xd12 0\ngl glPixelTransferi 0xd13 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    // the maps on indices and stencil values (GL_MAP_COLOR in color index mode, GL_MAP_STENCIL),
    // and a name glPixelTransfer does not take
    write!(s, "case transfer-maps-ci\nglctx 8 29\n{ORTHO}").unwrap();
    writeln!(s, "gl glClear 0x4000\ngl glPixelTransferi 0xd10 1\ngldraw 16 4 {} {} 77 64\nglread 0 0 16 4 {} {} 64\ngl glPixelTransferi 0xd10 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case transfer-maps-stencil\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}").unwrap();
    writeln!(s, "gl glClearStencil 0x5a\ngl glClear 0x400\ngl glPixelTransferi 0xd11 1\ngldraw 16 4 {} {} 78 64\nglread 0 0 16 4 {} {} 64\ngl glPixelTransferi 0xd11 0\nglread 0 0 16 4 {} {} 64", h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(UNSIGNED_BYTE)).unwrap();
    writeln!(s, "glerr\ngl glPixelTransferi 0xd16 1\nglerr\ngl glPixelTransferf 0xd00 1\nglerr\nend").unwrap();
    // the index and stencil maps on reads of values not 0, with the shift and offset, as
    // bytes, floats and bitmaps; a draw of indices with both
    write!(s, "case transfer-maps-read\nglctx 8 29\n{ORTHO}gl glClearIndex 85\ngl glClear 0x4000\n").unwrap();
    for (m, sh) in [(1, 0), (1, 2), (0, 2)] {
        writeln!(s, "gl glPixelTransferi 0xd10 {m}\ngl glPixelTransferi 0xd12 {sh}\ngl glPixelTransferi 0xd13 {sh}\nglread 0 0 16 4 {} {} 64\nglread 0 0 16 4 {} {} 256\nglread 0 0 16 4 {} {} 8", h(COLOR_INDEX), h(UNSIGNED_BYTE), h(COLOR_INDEX), h(FLOAT), h(COLOR_INDEX), h(BITMAP)).unwrap();
        writeln!(s, "gldraw 16 2 {} {} {} 32\ngl glPixelTransferi 0xd10 0\ngl glPixelTransferi 0xd12 0\ngl glPixelTransferi 0xd13 0\nglread 0 0 16 4 {} {} 64", h(COLOR_INDEX), h(UNSIGNED_BYTE), q.next() & 0xffff, h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case transfer-maps-read-stencil\nglctx 32 choose 0x38 0 32 0 0 32 8\n{ORTHO}gl glClearStencil 0xa7\ngl glClear 0x400\n").unwrap();
    for (m, sh) in [(1, 0), (1, 1), (0, 1)] {
        writeln!(s, "gl glPixelTransferi 0xd11 {m}\ngl glPixelTransferi 0xd12 {sh}\ngl glPixelTransferi 0xd13 {sh}\nglread 0 0 16 4 {} {} 64\nglread 0 0 16 4 {} {} 256\nglread 0 0 16 4 {} {} 8", h(STENCIL_INDEX), h(UNSIGNED_BYTE), h(STENCIL_INDEX), h(FLOAT), h(STENCIL_INDEX), h(BITMAP)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    // glNormal3f, glVertex4f and glPixelStoref, and the capabilities that change what is
    // drawn: lighting with the initial lights, face culling, point and polygon antialiasing
    write!(s, "case other-calls-3\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\n", ctx(32)).unwrap();
    writeln!(s, "gl glNormal3f 0.25 -0.5 2\nglget f 0xb02 3\nglget i 0xb02 3\ngl glPushAttrib 0x1\ngl glNormal3f 1 0 0\ngl glPopAttrib\nglget f 0xb02 3").unwrap();
    writeln!(s, "gl glColor4ub 200 100 50 255\ngl glBegin 7\ngl glVertex4f 1 1 0 1\ngl glVertex4f 7 1 0 1\ngl glVertex4f 7 7 0 1\ngl glVertex4f 1 7 0 1\ngl glEnd\ngl glBegin 0\ngl glVertex4f 12 4 0 1\ngl glEnd").unwrap();
    read_all(s);
    for (p, v) in [(0xcf2, "3.4"), (0xcf3, "2.5"), (0xcf4, "1.6"), (0xcf5, "3.9"), (0xcf0, "0.25"), (0xcf1, "-0.5"), (0xd02, "-0.25"), (0xd05, "1.49"), (0xd05, "2.5")] {
        writeln!(s, "gl glPixelStoref {} {v}\nglerr\nglget i {} 1", h(p), h(p)).unwrap();
    }
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glPointSize 2\ngl glBegin 0\ngl glVertex4f 20 20 0 2\ngl glVertex4f 6 6 0.5 0.5\ngl glVertex4f -10 6 0 -2\ngl glEnd\n", ctx(32)).unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    for (k, cap) in [0xb50u32, 0xb44, 0xb10, 0xb41].into_iter().enumerate() {
        write!(s, "case render-caps-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable {}\ngl glPointSize 3\n", ctx(32), h(cap)).unwrap();
        writeln!(s, "gl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\ngl glColor4ub 50 200 100 128\ngl glRectf 15 1 9 7\ngl glRectf 9.25 9.25 14.5 14.75\ngl glColor4ub 10 20 250 255\ngl glBegin 0\ngl glVertex2f 3.5 12.5\ngl glVertex2f 5.3 9.7\ngl glEnd").unwrap();
        read_all(s);
        writeln!(s, "gl glCullFace 0x404\ngl glFrontFace 0x900\ngl glColor4ub 250 250 0 255\ngl glRectf 2 10 6 14\ngl glRectf 1.75 14.25 0.25 9.75").unwrap();
        read_all(s);
        writeln!(s, "glerr\nend").unwrap();
    }
    // culling both faces; antialiased points of every size at random positions; lighting
    // with GL_COLOR_MATERIAL
    write!(s, "case render-caps-4\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb44\ngl glCullFace 0x408\ngl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\ngl glRectf 15 1 9 7\ngl glBegin 0\ngl glVertex2f 3.5 12.5\ngl glEnd\n", ctx(32)).unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    for k in 0..4 {
        write!(s, "case smooth-points-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb10\n", ctx(32)).unwrap();
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4000\ngl glPointSize {}", (q.next() % 40) as f64 / 4.0 + 0.5).unwrap();
            writeln!(s, "gl glColor4ub 255 255 255 255\ngl glBegin 0\ngl glVertex2f {} {}\ngl glEnd", (q.next() % 1000) as f64 / 1000.0 * 8.0 + 4.0, (q.next() % 1000) as f64 / 1000.0 * 8.0 + 4.0).unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // more antialiased points, four of a size apart in each image, at thousandths of a pixel
    for k in 4..12 {
        write!(s, "case smooth-points-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb10\n", ctx(32)).unwrap();
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4000\ngl glPointSize {}\ngl glColor4ub 255 255 255 255\ngl glBegin 0", (q.next() % 3500) as f64 / 1000.0 + 0.25).unwrap();
            for (cx, cy) in [(4, 4), (12, 4), (4, 12), (12, 12)] {
                writeln!(s, "gl glVertex2f {} {}", cx as f64 + (q.next() % 2000) as f64 / 1000.0 - 1.0, cy as f64 + (q.next() % 2000) as f64 / 1000.0 - 1.0).unwrap();
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // what the model does not follow yet: fog on images, zoomed depth images, antialiasing
    // with alpha to show the coverage in, a light on, lighting in color index mode
    write!(s, "case other-calls-5\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glRasterPos2f 2.5 2.5\ngldraw 4 4 {} {} 90 64\n", ctx(32), h(RGBA), h(UNSIGNED_BYTE)).unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-6\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glClear 0x4100\ngl glPixelZoom 2 3\ngl glRasterPos2f 1.5 1.5\ngldraw 4 4 {} {} 91 64\nglread 0 0 16 16 {} {} 1024\nglerr\nend\n", ctx(32), h(DEPTH_COMPONENT), h(FLOAT), h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
    write!(s, "case other-calls-7\nglctx 32 5\n{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\ngl glEnable 0xb10\ngl glPointSize 3.3\ngl glColor4ub 200 100 50 255\ngl glRectf 1.25 1.5 7.75 6.25\ngl glBegin 0\ngl glVertex2f 11.3 11.6\ngl glEnd\n").unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-10\nglctx 32 5\n{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\ngl glEnable 0xb10\ngl glPointSize 3.3\ngl glColor4ub 200 100 50 255\ngl glRectf 1.25 1.5 7.75 6.25\ngl glBegin 0\ngl glVertex2f 11.2 11.45\ngl glEnd\n").unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-8\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb50\ngl glEnable 0x4000\ngl glNormal3f 0 0 1\ngl glColor4ub 200 100 50 255\ngl glRectf 1 1 7 7\n", ctx(32)).unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case other-calls-9\nglctx 8 29\n{ORTHO}gl glClear 0x4000\ngl glEnable 0xb50\ngl glIndexi 9\ngl glRectf 1 1 7 7\nglread 0 0 16 16 {} {} 256\nglerr\nend\n", h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    write!(s, "case lighting-material\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb50\ngl glEnable 0xb57\n", ctx(32)).unwrap();
    for i in 0..16 {
        let (x0, y0) = (i % 4 * 4, i / 4 * 4);
        writeln!(s, "gl glColor4ub {} {} {} 255\ngl glRectf {x0} {y0} {} {}", q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, y0 + 4).unwrap();
    }
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    // 1-pixel lines at thousandths of a pixel (no diamond touched exactly), strips and loops,
    // some ends past the window, one in a smaller viewport
    let cr = |q: &mut XorShift32| (q.next() % 20000) as f64 / 1000.0 - 2.0;
    for k in 0..6 {
        write!(s, "case lines-{k}\n{}{ORTHO}gl glDisable 0xbd0\n", ctx(32)).unwrap();
        if k == 5 {
            writeln!(s, "gl glViewport 2 3 11 9").unwrap();
        }
        for mode in [1, 3, 2] {
            writeln!(s, "gl glClear 0x4000\ngl glColor4ub {} {} {} 255\ngl glBegin {mode}", q.next() & 255, q.next() & 255, q.next() & 255).unwrap();
            for _ in 0..6 {
                writeln!(s, "gl glVertex2f {} {}", cr(&mut q), cr(&mut q)).unwrap();
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // primitives wholly past the near or far plane, beside ones inside
    write!(s, "case clip-z\n{}{ORTHO}gl glDisable 0xbd0\ngl glClear 0x4000\ngl glColor4ub 200 100 50 255\ngl glBegin 0\ngl glVertex3f 2.5 2.5 1.5\ngl glVertex3f 4.5 2.5 -1.5\ngl glVertex3f 6.5 2.5 0.5\ngl glEnd\ngl glBegin 1\ngl glVertex3f 0.3 6.2 -3\ngl glVertex3f 15.1 7.9 -1.25\ngl glVertex3f 0.3 9.2 0.25\ngl glVertex3f 15.1 10.9 0.25\ngl glEnd\ngl glBegin 7\ngl glVertex3f 1 12 1.01\ngl glVertex3f 5 12 1.01\ngl glVertex3f 5 15 1.01\ngl glVertex3f 1 15 1.01\ngl glVertex3f 9 12 0.99\ngl glVertex3f 13 12 0.99\ngl glVertex3f 13 15 0.99\ngl glVertex3f 9 15 0.99\ngl glEnd\n", ctx(32)).unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    // fog over flat quads, one depth each, in every mode on the RGBA bit depths, a fog color,
    // the state read back; and color index fog and points
    let mut q = XorShift32::new(0x0960_200b);
    let z = |q: &mut XorShift32| format!("{}", (q.next() % 1999) as f64 / 1000.0 - 0.999);
    for (k, bpp) in [32, 16, 24, 32].into_iter().enumerate() {
        write!(s, "case fog-rects-{k}\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\n", ctx(bpp)).unwrap();
        for mode in [0x2601u32, 0x800, 0x801] {
            let (a, b) = (z(&mut q), z(&mut q));
            let dens = (q.next() % 4000) as f64 / 1000.0;
            writeln!(s, "gl glClear 0x4000\ngl glFogf 0xb65 {mode}\ngl glFogf 0xb63 {a}\ngl glFogf 0xb64 {b}\ngl glFogf 0xb62 {dens}").unwrap();
            writeln!(s, "glfogv 0xb66 {} {} {} {}", (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0, (q.next() % 1001) as f64 / 1000.0).unwrap();
            for i in 0..16 {
                let (x0, y0) = (i % 4 * 4, i / 4 * 4);
                let zz = z(&mut q);
                writeln!(s, "gl glColor4ub {} {} {} {}\ngl glBegin 7\ngl glVertex3f {x0} {y0} {zz}\ngl glVertex3f {} {y0} {zz}\ngl glVertex3f {} {} {zz}\ngl glVertex3f {x0} {} {zz}\ngl glEnd", q.next() & 255, q.next() & 255, q.next() & 255, q.next() & 255, x0 + 4, x0 + 4, y0 + 4, y0 + 4).unwrap();
            }
            read_all(s);
        }
        writeln!(s, "glget f 0xb66 4\nglget f 0xb62 1\nglget f 0xb63 1\nglget f 0xb64 1\nglget i 0xb65 1\nglerr").unwrap();
        writeln!(s, "gl glFogf 0xb62 -1\nglerr\ngl glFogf 0xb65 4660\nglerr\ngl glFogf 0xb66 1\nglerr\nglfogv 0xb65 2049\nglerr\nglfogv 0x1234 1\nglerr\nglfogv 0xb66 2 -1 0.5 0.25\nglget f 0xb66 4\nglget i 0xb65 1\nglerr\nend").unwrap();
    }
    write!(s, "case fog-points\n{}{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb60\ngl glClear 0x4000\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\nglfogv 0xb66 0.25 0.5 0.75 1\ngl glBegin 0\n", ctx(32)).unwrap();
    for i in 0..64 {
        writeln!(s, "gl glColor4ub {} {} {} 255\ngl glVertex3f {}.5 {}.5 {}", q.next() & 255, q.next() & 255, q.next() & 255, i % 16, i / 16 * 4, z(&mut q)).unwrap();
    }
    writeln!(s, "gl glEnd").unwrap();
    read_all(s);
    writeln!(s, "glerr\nend").unwrap();
    write!(s, "case fog-ci\nglctx 8 29\n{ORTHO}gl glEnable 0xb60\ngl glClear 0x4000\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 0\ngl glFogf 0xb64 1\ngl glFogf 0xb61 {}\n", q.next() % 200).unwrap();
    for i in 0..16 {
        let (x0, y0) = (i % 4 * 4, i / 4 * 4);
        let zz = z(&mut q);
        writeln!(s, "gl glIndexi {}\ngl glBegin 7\ngl glVertex3f {x0} {y0} {zz}\ngl glVertex3f {} {y0} {zz}\ngl glVertex3f {} {} {zz}\ngl glVertex3f {x0} {} {zz}\ngl glEnd", q.next() % 56, x0 + 4, x0 + 4, y0 + 4, y0 + 4).unwrap();
    }
    writeln!(s, "glread 0 0 16 16 {} {} 256\nglerr\nend", h(COLOR_INDEX), h(UNSIGNED_BYTE)).unwrap();
    // polygon offset units on flat rectangles, on 32 and 16 depth bits, the state read back
    for (k, c) in ["glctx 32 choose 0x38 0 32 0 0 32 8", "glctx 32 2"].into_iter().enumerate() {
        write!(s, "case poly-offset-{k}\n{c}\n{ORTHO}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glEnable 0x8037\n").unwrap();
        for units in ["1", "-1", "2", "-3", "0.5", "-0.5", "-1000", "100000", "-100000000", "7.25"] {
            let zc = z(&mut q);
            writeln!(s, "gl glClearDepth 0.5\ngl glClear 0x100\ngl glPolygonOffset {} {units}\ngl glBegin 7\ngl glVertex3f 0 0 {zc}\ngl glVertex3f 16 0 {zc}\ngl glVertex3f 16 8 {zc}\ngl glVertex3f 0 8 {zc}\ngl glEnd\ngl glRectf 0 8 16 16", (q.next() % 9) as f64 - 4.0).unwrap();
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "glget f 0x8038 1\nglget f 0x2a00 1\ngl glPushAttrib 0x8\ngl glPolygonOffset 3 4\ngl glPopAttrib\nglget f 0x8038 1\nglget f 0x2a00 1\nglerr\nend").unwrap();
    }
    // the same triangle twice: under the window's own viewport, where the clip volume cuts the
    // ones past the window, and under a viewport reaching 16 pixels past every side, where
    // nothing is cut and only the window bounds the pixels; vertices on the eighth grid (0, 1)
    // and anywhere (2, 3)
    let mut q = XorShift32::new(0x0960_200c);
    for k in 0..4 {
        writeln!(s, "case tri-clip-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32)).unwrap();
        for _ in 0..32 {
            let v: Vec<(f64, f64)> = (0..3).map(|_| if k < 2 { (v8(&mut q, -2, 18), v8(&mut q, -2, 18)) } else { (q.range(-2048, 18 * 1024) as f64 / 1024.0, q.range(-2048, 18 * 1024) as f64 / 1024.0) }).collect();
            for (vp, o) in [("0 0 16 16", "0 16 0 16"), ("-16 -16 48 48", "-16 32 -16 32")] {
                writeln!(s, "gl glViewport {vp}\ngl glLoadIdentity\ngl glOrtho {o} -1 1\ngl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in &v {
                    glf(s, "glVertex2f", &[*x, *y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // blending with dithering on 16, 8 and 4-bit DIBs: random factor pairs, a random picture
    // drawn undithered under each, one random color over it dithered
    let mut q = XorShift32::new(0x0960_200d);
    for (k, (bpp, pf)) in [(16u32, 0u32), (8, 25), (4, 31)].iter().enumerate() {
        let head = if *pf == 0 { ctx(*bpp) } else { format!("glctx {bpp} {pf}\n") };
        writeln!(s, "case blend-dither-{k}\n{head}{ORTHO}").unwrap();
        for _ in 0..24 {
            let (sf, df) = (*q.pick(BLEND_SRC), *q.pick(BLEND_DST));
            writeln!(s, "gl glDisable 0xbd0\ngl glDisable 0xbe2").unwrap();
            random_dst(s, q.next() & 0xffff);
            gl(s, "glBlendFunc", &[sf, df]);
            writeln!(s, "gl glEnable 0xbd0\ngl glEnable 0xbe2").unwrap();
            let c: Vec<u32> = (0..4).map(|_| q.next() & 255).collect();
            gl(s, "glColor4ub", &c);
            writeln!(s, "gl glRectf 0 0 16 16\ngldib").unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case tri-probe-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701", ctx(32)).unwrap();
        for t in part {
            for (vp, o) in [("0 0 16 16", "0 16 0 16"), ("-2 -2 20 20", "-2 18 -2 18"), ("-16 -16 48 48", "-16 32 -16 32"), ("-64 -64 144 144", "-64 80 -64 80"), ("-512 -512 1040 1040", "-512 528 -512 528"), ("0 0 64 64", "0 64 0 64"), ("-48 -48 64 64", "-48 16 -48 16")] {
                writeln!(s, "gl glViewport {vp}\ngl glLoadIdentity\ngl glOrtho {o} -1 1\ngl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in t {
                    glf(s, "glVertex2f", &[f64::from(*x) / 1024.0, f64::from(*y) / 1024.0]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // one triangle edge measured alone: an edge through the window from y = -100 to y = 116
    // (slope s in x per y, through x0 at y = 8), the third vertex 400 pixels to its right (the
    // edge the left boundary) or left (the right one), under a viewport 512 pixels past every side
    // so that nothing is cut; x0 swept over -16/2048 .. 16/2048 around a point between pixel
    // centres, each row's pixel flipping where Windows puts the edge in that row
    const SLOPES: [f64; 10] = [0.0, 1.0 / 64.0, 0.125, 33.0 / 64.0, 1.0, -1.0, 1.2109375, -1.03125, 2.25, 0.4296875];
    for (k, sl) in SLOPES.iter().enumerate() {
        for (side, dx) in [("l", 400.0), ("r", -400.0)] {
            writeln!(s, "case tri-edge-{side}{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
            for j in -16..=16 {
                let x0 = 8.25 + f64::from(j) / 2048.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                glf(s, "glVertex2f", &[x0 - 108.0 * sl, -100.0]);
                glf(s, "glVertex2f", &[x0 + 108.0 * sl, 116.0]);
                glf(s, "glVertex2f", &[x0 + dx, 8.0]);
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
            writeln!(s, "glerr\nend").unwrap();
        }
    }
    // the PROBE triangles swept sideways by -8/1024 .. 8/1024 pixel under a viewport that cuts
    // nothing: where each of a triangle's edges flips in each row, against the lone edges of
    // tri-edge-*, which Windows puts where they are
    for (k, part) in PROBE.chunks(5).enumerate() {
        writeln!(s, "case tri-sweep-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for t in part {
            for j in -8..=8 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in t {
                    glf(s, "glVertex2f", &[f64::from(*x + j) / 1024.0, f64::from(*y) / 1024.0]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // a lone edge (vertical, and slope 1: every row's pixel centre on it, through x = 8.5 at
    // y = 8.5, the left boundary) made part of a
    // smaller triangle step by step: the third vertex nearer (400 .. 10 pixels to the right), the
    // lower vertex higher (y = -100 .. 3.8125, inside the window), both; each swept by -8/1024 ..
    // 8/1024 pixel under a viewport that cuts nothing
    const NEAR: [(f64, f64, f64); 11] = [(-100.0, 400.0, 8.0), (-100.0, 100.0, 8.0), (-100.0, 40.0, 8.0), (-100.0, 16.0, 8.0), (-100.0, 10.0, 8.0), (-20.0, 400.0, 8.0), (-2.6875, 400.0, 8.0), (1.3125, 400.0, 8.0), (3.8125, 400.0, 8.0), (1.3125, 10.0, 8.0), (1.3125, 10.0, 12.3125)];
    for (k, (ya, dx, yc, sl)) in NEAR.iter().flat_map(|&(a, b, c)| [(a, b, c, 0.0), (a, b, c, 1.0)]).enumerate() {
        writeln!(s, "case tri-near-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (yc - 8.0) + dx, yc]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // the same edges (vertical through x = 8.5, slope 1 through the pixel centres) with the lower
    // vertex at uneven heights inside the window and the third vertex 10 pixels to the right (left
    // boundary) or left (right boundary): whether the edge moves with where its vertices lie
    const LOW: [f64; 6] = [1.0009765625, 2.3330078125, 3.7509765625, 4.1259765625, 5.6669921875, 6.9990234375];
    for (k, (ya, sl, dx)) in LOW.iter().flat_map(|&a| [(a, 0.0, 10.0), (a, 1.0, 10.0), (a, 0.0, -10.0), (a, 1.0, -10.0)]).enumerate() {
        writeln!(s, "case tri-low-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + dx, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case tri-hair-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * 108.0, 116.0]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + dx, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case tri-hair2-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for j in -8..=8 {
            let x0 = 8.5 - sl * 0.5 + f64::from(j) / 1024.0;
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            glf(s, "glVertex2f", &[x0 + sl * (ya - 8.0), ya]);
            glf(s, "glVertex2f", &[x0 + sl * (yb - 8.0), yb]);
            glf(s, "glVertex2f", &[x0 + sl * (ya + 1.0 - 8.0) + 10.0, ya + 1.0]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case tri-fine-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for j in -24..=24 {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            for (x, y) in tri(f64::from(j) / 8192.0) {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // a sloped left edge through (8.5 + p / 1024, 8.5) with far vertices, swept by 1/8192: where
    // Windows rounds vertices with other fractions of 1/256 (tri-fine-5: the slope-1/3 line
    // moves 2/8192 later than rounding half down would have it)
    let mut k = 0;
    for sl in [1.0 / 3.0, 1.0 / 5.0, 2.0 / 3.0, 3.0 / 7.0] {
        for p in 0..4 {
            writeln!(s, "case tri-snap-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
            for j in -24..=24 {
                let d = f64::from(p) / 1024.0 + f64::from(j) / 8192.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in [(8.5 - 108.5 * sl + d, -100.0), (8.5 + 107.5 * sl + d, 116.0), (408.5 + d, 8.0)] {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
            writeln!(s, "glerr\nend").unwrap();
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
            writeln!(s, "case tri-len-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport {lo} {lo} {vw} {vw}\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", ctx(32), lo + vw, lo + vw).unwrap();
            let bottom = -1.0;
            for j in -24..=24 {
                let d = f64::from(j) / 8192.0;
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in [(8.5 + (bottom - 8.5) * sl + d, bottom), (8.5 + (top - 8.5) * sl + d, top), (8.5 + (8.0 - 8.5) * sl + d + 4.0, 8.0)] {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
            writeln!(s, "glerr\nend").unwrap();
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
            writeln!(s, "case tri-vp-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport {lo} {lo} {vw} {vw}\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", ctx(32), lo + vw, lo + vw).unwrap();
            for j in -24..=24 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                for (x, y) in shape(f64::from(j) / 8192.0) {
                    glf(s, "glVertex2f", &[x, y]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
            writeln!(s, "glerr\nend").unwrap();
            k += 1;
        }
    }
    // a vertex at NDC +-1/2 under an 18 wide viewport lands on an exact half of a 1/256 step
    // (8 + 1/2 (9 - 1/256) = 12.498046875) right beside a pixel center: which way a tie snaps
    writeln!(s, "case tri-tie\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255", ctx(32)).unwrap();
    for lo in [-1, -3] {
        let c = f64::from(lo + 9);
        writeln!(s, "gl glViewport {lo} {lo} 18 18\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho {lo} {} {lo} {} -1 1\ngl glMatrixMode 0x1700", lo + 18, lo + 18).unwrap();
        for tri in [
            [(c + 4.5, 1.0), (c + 4.5, 15.0), (c + 8.5, 8.0)],
            [(c - 4.5, 1.0), (c - 4.5, 15.0), (c - 8.5, 8.0)],
            [(1.0, c + 4.5), (15.0, c + 4.5), (8.0, c + 8.5)],
            [(1.0, c - 4.5), (15.0, c - 4.5), (8.0, c - 8.5)],
        ] {
            writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
            for (x, y) in tri {
                glf(s, "glVertex2f", &[x, y]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
        }
    }
    writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case tri-start-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700\ngl glClear 0x4000\ngl glBegin 4", ctx(32)).unwrap();
        for (x, y) in [tri[0], tri[1], (60.0, 8.0)] {
            glf(s, "glVertex2f", &[x, y]);
        }
        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256\nglerr\nend", h(RED), h(UNSIGNED_BYTE)).unwrap();
    }
    // smooth triangles over the whole window, inside a 1040 viewport (no clipping): corners
    // at (-56, -56), (200, -56), (-56, 200) so each channel's plane steps by (c1 - c0) / 256
    // a pixel along x and (c2 - c0) / 256 along y; colors from a sweep, each order of the
    // corners: how Windows interpolates
    let mut r = XorShift32::new(0x1f7e_4a0d);
    for k in 0..8 {
        writeln!(s, "case interp-{k}\n{}gl glDisable 0xbd0\ngl glShadeModel 0x1d01\ngl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for _ in 0..4 {
            let cols: [[u32; 3]; 3] = std::array::from_fn(|_| std::array::from_fn(|_| r.next() % 256));
            let corners = [(-56.0, -56.0), (200.0, -56.0), (-56.0, 200.0)];
            for order in [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1]] {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 4").unwrap();
                for i in order {
                    let c = cols[i];
                    writeln!(s, "gl glColor4ub {} {} {} 255", c[0], c[1], c[2]).unwrap();
                    glf(s, "glVertex2f", &[corners[i].0, corners[i].1]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // depth across one-colored triangles, read back as 32-bit integers: the window's corners
    // under big triangles in a 1040 viewport (planes of dyadic and other slopes) and small
    // ones inside a 16 viewport, each from every corner: how Windows steps depth
    let mut r = XorShift32::new(0x2e1d_7c53);
    for k in 0..8 {
        let big = k < 4;
        let view = if big { "gl glViewport -512 -512 1040 1040\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho -512 528 -512 528 -1 1\ngl glMatrixMode 0x1700" } else { "gl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700" };
        writeln!(s, "case zinterp-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\n{view}", ctx(32)).unwrap();
        for _ in 0..4 {
            let corners: [(f64, f64); 3] = if big {
                [(-56.0, -56.0), (200.0, -56.0), (-56.0, 200.0)]
            } else {
                std::array::from_fn(|_| (f64::from(r.next() % 113) / 8.0 + 1.0, f64::from(r.next() % 113) / 8.0 + 1.0))
            };
            let z: [f64; 3] = std::array::from_fn(|i| if k % 2 == 0 && i < 3 { f64::from(r.next() % 257) / 128.0 - 1.0 } else { f64::from(r.next() % 2001) / 1000.0 - 1.0 });
            for order in [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1]] {
                writeln!(s, "gl glClear 0x4100\ngl glBegin 4").unwrap();
                for i in order {
                    glf(s, "glVertex3f", &[corners[i].0, corners[i].1, z[i]]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // more small triangles of varying depth, corners on a 1/128 grid, tall and thin ones among
    // them: how Windows steps depth from row to row along an edge
    let mut r = XorShift32::new(0x5a1d_93e7);
    for k in 8..20 {
        // (16..20: a 16-bit depth buffer)
        let ctx = if k < 16 { ctx(32) } else { "glctx 32 2\n".to_string() };
        writeln!(s, "case zinterp-{k}\n{ctx}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700").unwrap();
        for _ in 0..12 {
            let thin = k % 2 == 1;
            let corners: [(f64, f64); 3] = std::array::from_fn(|_| {
                let x = if thin { f64::from(r.next() % 513) / 128.0 + 6.0 } else { f64::from(r.next() % 1921) / 128.0 };
                (x, f64::from(r.next() % 1921) / 128.0)
            });
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4").unwrap();
            for c in corners {
                glf(s, "glVertex3f", &[c.0, c.1, f64::from(r.next() % 2001) / 1000.0 - 1.0]);
            }
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "case poly-nc-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        let n = shape.len();
        let modes: &[u32] = if n == 4 { &[9, 7] } else { &[9] };
        for &mode in modes {
            for back in [false, true] {
                for first in 0..n {
                    writeln!(s, "gl glClear 0x4000\ngl glBegin {mode}").unwrap();
                    for i in 0..n {
                        let at = if back { (first + n - i) % n } else { (first + i) % n };
                        glf(s, "glVertex2f", &[shape[at].0, shape[at].1]);
                    }
                    writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
                }
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // perspective through a modelview that turns: small smooth triangles about the origin,
    // turned, then pushed back into the frustum (none cut): how Windows rounds turned vertices
    let mut r = XorShift32::new(0x7e57_0b1c);
    let turns: [(f64, [f64; 3]); 8] = [(10.0, [0.0, 1.0, 0.0]), (30.0, [1.0, 0.0, 0.0]), (45.0, [0.0, 1.0, 0.0]), (60.0, [1.0, 1.0, 0.0]), (90.0, [0.0, 0.0, 1.0]), (135.0, [0.0, 1.0, 1.0]), (20.0, [1.0, 2.0, 3.0]), (-75.0, [0.0, 1.0, 0.0])];
    for (k, (angle, axis)) in turns.into_iter().enumerate() {
        writeln!(s, "case persp-turn-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glShadeModel 0x1d01\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32)).unwrap();
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        writeln!(s, "gl glMatrixMode 0x1700\ngl glLoadIdentity").unwrap();
        glf(s, "glTranslatef", &[0.0, 0.0, -3.0]);
        glf(s, "glRotatef", &[angle, axis[0], axis[1], axis[2]]);
        for _ in 0..4 {
            writeln!(s, "gl glClear 0x4100\ngl glBegin 4").unwrap();
            for _ in 0..3 {
                writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256).unwrap();
                let c = |r: &mut XorShift32| f64::from(r.next() % 41) / 64.0 - 0.3125;
                let v = [c(&mut r), c(&mut r), c(&mut r)];
                glf(s, "glVertex3f", &v);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // the polygons that are not convex, moved partly past the viewport (w = 1) and, under a
    // projection, partly past the sides and the near plane: how Windows cuts them
    let shifts: [(f64, f64); 4] = [(-6.0, 0.0), (6.0, 1.0), (1.0, 6.0), (-1.0, -6.0)];
    for (k, shape) in shapes.into_iter().enumerate() {
        writeln!(s, "case clip-nc-{k}\n{}gl glDisable 0xbd0\ngl glColor4ub 255 0 0 255\ngl glViewport 0 0 16 16\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
        for (dx, dy) in shifts {
            for first in 0..2 {
                writeln!(s, "gl glClear 0x4000\ngl glBegin 9").unwrap();
                for i in 0..shape.len() {
                    let at = shape[(first + i) % shape.len()];
                    glf(s, "glVertex2f", &[at.0 + dx, at.1 + dy]);
                }
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 256", h(RED), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    for (k, shape) in shapes.into_iter().enumerate() {
        writeln!(s, "case persp-cut-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glColor4ub 255 0 0 255\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32)).unwrap();
        glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        writeln!(s, "gl glMatrixMode 0x1700").unwrap();
        // (k even: past the sides; odd: through the near plane too)
        for (dx, dy) in shifts {
            for first in 0..2 {
                writeln!(s, "gl glClear 0x4100\ngl glBegin 9").unwrap();
                for i in 0..shape.len() {
                    let at = shape[(first + i) % shape.len()];
                    let z = if k % 2 == 0 { -2.0 - at.0 / 16.0 } else { -0.5 - at.1 / 8.0 };
                    glf(s, "glVertex3f", &[(at.0 + dx - 8.0) / 8.0, (at.1 + dy - 8.0) / 8.0, z]);
                }
                writeln!(s, "gl glEnd").unwrap();
                read_all(s);
                writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // 1-pixel lines with an end on a diamond's border: on each corner and edge middle of pixel
    // (8, 8), leaving or arriving from 16 directions (along the axes, the edges, and between):
    // which border parts a diamond holds
    let at: [(f64, f64); 8] = [(8.5, 8.0), (9.0, 8.5), (8.5, 9.0), (8.0, 8.5), (8.25, 8.25), (8.75, 8.25), (8.75, 8.75), (8.25, 8.75)];
    let dirs: [(f64, f64); 16] = [(2.25, 0.625), (0.625, 2.25), (-0.625, 2.25), (-2.25, 0.625), (-2.25, -0.625), (-0.625, -2.25), (0.625, -2.25), (2.25, -0.625), (2.0, 0.0), (0.0, 2.0), (-2.0, 0.0), (0.0, -2.0), (1.5, 1.5), (-1.5, 1.5), (-1.5, -1.5), (1.5, -1.5)];
    for (k, p) in at.into_iter().enumerate() {
        start(s, format!("line-tie-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
        for d in dirs {
            for ends in [false, true] {
                let (a, b) = if ends { ((p.0 - d.0, p.1 - d.1), p) } else { (p, (p.0 + d.0, p.1 + d.1)) };
                writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
                glf(s, "glVertex2f", &[a.0, a.1]);
                glf(s, "glVertex2f", &[b.0, b.1]);
                writeln!(s, "gl glEnd\nglread 5 5 7 7 {} {} 196", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // wide lines inside the window, one per image: widths 2, 3, 4 and 2.5, x- and y-major
    let mut r = XorShift32::new(0x31de_4a07);
    for (k, width) in [2.0, 2.0, 3.0, 3.0, 4.0, 2.5].into_iter().enumerate() {
        start(s, format!("line-wide-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
        glf(s, "glLineWidth", &[width]);
        for _ in 0..16 {
            let mut v = || f64::from(r.next() % 97) / 8.0 + 2.0;
            let (a, b) = ((v(), v()), (v(), v()));
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // under a projection: lines the view volume cuts (sides, near plane, behind the eye), with
    // their depths; triangles of glVertex4f corners (w 2, 1/2, -1) at w = 1 projections; culled
    // triangles with a corner behind the eye
    let mut r = XorShift32::new(0x6c0f_2d95);
    for k in 0..8 {
        writeln!(s, "case persp-more-{k}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", ctx(32)).unwrap();
        match k {
            0..=2 => glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]),
            3..=5 => writeln!(s, "gl glOrtho 0 16 0 16 -1 1").unwrap(),
            _ => {
                glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
                writeln!(s, "gl glEnable 0xb44\ngl glCullFace {}", if k == 6 { "0x405" } else { "0x404" }).unwrap();
            }
        }
        writeln!(s, "gl glMatrixMode 0x1700").unwrap();
        for _ in 0..6 {
            writeln!(s, "gl glClear 0x4100").unwrap();
            writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256).unwrap();
            let mut c = |lo: i32, hi: i32| f64::from(lo + (r.next() % (hi - lo + 1) as u32) as i32) / 8.0;
            match k {
                0..=2 => {
                    writeln!(s, "gl glBegin 1").unwrap();
                    for _ in 0..2 {
                        let z = if k == 2 { c(-24, 8) } else { c(-24, -4) };
                        let v = [c(-24, 24), c(-24, 24), z];
                        glf(s, "glVertex3f", &v);
                    }
                }
                3..=5 => {
                    writeln!(s, "gl glBegin 4").unwrap();
                    for _ in 0..3 {
                        let w = [2.0, 0.5, -1.0, 1.0][(c(0, 3) * 8.0) as usize];
                        let v = [c(0, 128) * w / 8.0, c(0, 128) * w / 8.0, c(-6, 6) * w / 8.0, w];
                        glf(s, "glVertex4f", &v);
                    }
                }
                _ => {
                    writeln!(s, "gl glBegin 4").unwrap();
                    for i in 0..3 {
                        let z = if i == 0 { c(1, 16) } else { c(-32, -10) };
                        let v = [c(-16, 16), c(-16, 16), z];
                        glf(s, "glVertex3f", &v);
                    }
                }
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // long 1-pixel lines whose exact path meets pixel boundaries partway (slopes p/q with q
    // not a power of 2, ends on pixel centers): how Windows steps across, in fixed or float
    let slopes: [(i32, i32); 8] = [(1, 3), (2, 3), (1, 5), (3, 7), (1, 6), (5, 6), (2, 7), (4, 9)];
    for k in 0..4 {
        start(s, format!("line-dda-{k}"), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
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
                writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
                glf(s, "glVertex2f", &[a.0, a.1]);
                glf(s, "glVertex2f", &[b.0, b.1]);
                writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // wide lines whose ends sit off every diamond border (odd 64ths): widths 2, 3, 5
    let mut r = XorShift32::new(0x2b9d_61c3);
    for (k, width) in [2.0, 3.0, 5.0].into_iter().enumerate() {
        start(s, format!("line-wide-{}", k + 6), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
        glf(s, "glLineWidth", &[width]);
        for _ in 0..12 {
            let mut v = || f64::from(r.next() % 49) / 4.0 + 2.0 + f64::from(2 * (r.next() % 8) + 1) / 64.0;
            let (a, b) = ((v(), v()), (v(), v()));
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // wide lines whose first end lies past the left plane (widths 2 and 3, x- and y-major,
    // up and down), the same lines drawn the other way, and first ends past the other planes
    let mut q = XorShift32::new(0x51d7_0e2b);
    for k in 0..4 {
        start(s, format!("line-wide-{}", k + 9), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
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
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // wide lines whose first end a plane across their minor axis cuts: y-major past the left
    // and the right, x-major past the bottom and the top (widths 2, 3, 4), each up and down
    let mut q = XorShift32::new(0x7e21_93ad);
    for k in 0..4 {
        start(s, format!("line-wide-{}", k + 13), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
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
            writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
            glf(s, "glVertex2f", &[a.0, a.1]);
            glf(s, "glVertex2f", &[b.0, b.1]);
            writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // antialiased lines; stippled (the default pattern, all set), antialiased and offset polygons
    for (k, setup) in ["gl glEnable 0xb20", "gl glEnable 0xb42", "gl glEnable 0xb41", "gl glEnable 0x8037\ngl glPolygonOffset 1 4\ngl glEnable 0xb71\ngl glDepthFunc 0x207"].into_iter().enumerate() {
        start(s, format!("raster-mode-{k}"), 32);
        writeln!(s, "{setup}\ngl glColor4ub 255 255 255 255").unwrap();
        for _ in 0..4 {
            let mut v = || f64::from(r.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k < 1 { 3 } else { 4 }).unwrap();
            for _ in 0..3 {
                let p = [v(), v()];
                glf(s, "glVertex2f", &p);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
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
        writeln!(s, "{setup}\ngl glColor4ub 255 255 255 255").unwrap();
        for i in 0..8 {
            if k >= 2 {
                glf(s, "glLineWidth", &[[1.5, 2.0, 3.0, 2.5][i % 4]]);
            }
            let mut v = || f64::from(q.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k == 1 { 4 } else if i % 2 == 0 { 1 } else { 3 }).unwrap();
            for _ in 0..if k == 1 { 3 } else { 2 + i % 2 } {
                let p = [v(), v()];
                glf(s, "glVertex2f", &p);
            }
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // antialiased 1-pixel lines (x- and y-major) and polygons into a buffer with alpha, not
    // blended: their coverage read back as alpha
    let mut q = XorShift32::new(0x6a0d_3b71);
    for k in 0..2 {
        writeln!(s, "case raster-mode-{}\nglctx 32 choose 0x38 0 32 8 0 32 8\n{ORTHO}gl glDisable 0xbd0\ngl glEnable {}\ngl glColor4ub 255 255 255 255", 8 + k, if k == 0 { "0xb20" } else { "0xb41" }).unwrap();
        for i in 0..12 {
            let mut v = || f64::from(q.next() % 113) / 8.0 + 1.0;
            writeln!(s, "gl glClear 0x4100\ngl glBegin {}", if k == 0 { 1 } else { 4 }).unwrap();
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
            writeln!(s, "gl glEnd").unwrap();
            read_all(s);
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // 1-pixel lines with an end on a diamond's border, as line-tie-* but about pixels on the
    // other sides of the tie pivot (13, 9), (13, 2), (8, 2)
    for (q, (px, py, read)) in [(13.0, 9.0, "10 6 6 7"), (13.0, 2.0, "10 0 6 6"), (8.0, 2.0, "5 0 7 6")].into_iter().enumerate() {
        for (k, p) in at.into_iter().enumerate() {
            let p = (p.0 - 8.0 + px, p.1 - 8.0 + py);
            start(s, format!("line-tie-{}", 8 + 8 * q + k), 32);
            writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
            for d in dirs {
                for ends in [false, true] {
                    let (a, b) = if ends { ((p.0 - d.0, p.1 - d.1), p) } else { (p, (p.0 + d.0, p.1 + d.1)) };
                    writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
                    glf(s, "glVertex2f", &[a.0, a.1]);
                    glf(s, "glVertex2f", &[b.0, b.1]);
                    writeln!(s, "gl glEnd\nglread {read} {} {} 196", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
                }
            }
            writeln!(s, "glerr\nend").unwrap();
        }
    }
    // lines of slope +-1/3 through pixel boundaries at centers, on every side of the tie pivot
    // and on it, x- and y-major, both ways
    for (k, cy) in [2.0, 4.0, 7.0, 12.0].into_iter().enumerate() {
        start(s, format!("line-dda-{}", 4 + k), 32);
        writeln!(s, "gl glColor4ub 255 255 255 255").unwrap();
        for (cx, back, ahead) in [(3.5, 3.0, 6.0), (8.5, 6.0, 6.0), (12.5, 6.0, 3.0)] {
            for sign in [1.0, -1.0] {
                for ymajor in [false, true] {
                    for flip in [false, true] {
                        let (a, b) = ((cx - back, cy - back / 3.0 * sign), (cx + ahead, cy + ahead / 3.0 * sign));
                        let (a, b) = if flip { (b, a) } else { (a, b) };
                        let (a, b) = if ymajor { ((a.1, a.0), (b.1, b.0)) } else { (a, b) };
                        writeln!(s, "gl glClear 0x4000\ngl glBegin 1").unwrap();
                        glf(s, "glVertex2f", &[a.0, a.1]);
                        glf(s, "glVertex2f", &[b.0, b.1]);
                        writeln!(s, "gl glEnd\nglread 0 0 16 16 {} {} 1024", h(RGBA), h(UNSIGNED_BYTE)).unwrap();
                    }
                }
            }
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // under a projection: points inside and past the sides, the near plane and the eye (1 and
    // 3 pixels wide); raster positions images are drawn at; fog at glVertex4f corners
    let mut r = XorShift32::new(0x1f3a_9c55);
    for k in 0..3 {
        writeln!(s, "case persp-more-{}\n{}gl glDisable 0xbd0\ngl glEnable 0xb71\ngl glDepthFunc 0x207\ngl glMatrixMode 0x1701\ngl glLoadIdentity", 8 + k, ctx(32)).unwrap();
        if k < 2 {
            glf(s, "glFrustum", &[-1.0, 1.0, -1.0, 1.0, 1.0, 8.0]);
        } else {
            writeln!(s, "gl glOrtho 0 16 0 16 -1 1\ngl glEnable 0xb60\ngl glFogf 0xb65 9729\ngl glFogf 0xb63 -0.5\ngl glFogf 0xb64 0.5\nglfogv 0xb66 0.25 0.5 0.75 1").unwrap();
        }
        writeln!(s, "gl glMatrixMode 0x1700").unwrap();
        for i in 0..6 {
            writeln!(s, "gl glClear 0x4100").unwrap();
            writeln!(s, "gl glColor4ub {} {} {} 255", r.next() % 256, r.next() % 256, r.next() % 256).unwrap();
            let mut c = |lo: i32, hi: i32| f64::from(lo + (r.next() % (hi - lo + 1) as u32) as i32) / 8.0;
            match k {
                0 => {
                    glf(s, "glPointSize", &[if i % 2 == 0 { 1.0 } else { 3.0 }]);
                    writeln!(s, "gl glBegin 0").unwrap();
                    for _ in 0..8 {
                        let v = [c(-24, 24), c(-24, 24), c(-24, 8)];
                        glf(s, "glVertex3f", &v);
                    }
                    writeln!(s, "gl glEnd").unwrap();
                }
                1 => {
                    writeln!(s, "gl glLoadIdentity").unwrap();
                    glf(s, "glTranslatef", &[0.0, 0.0, c(-24, 4)]);
                    glf(s, "glRasterPos2f", &[c(-16, 16), c(-16, 16)]);
                    writeln!(s, "gldraw 4 4 {} {} {} 64", h(RGBA), h(UNSIGNED_BYTE), 92 + i).unwrap();
                }
                _ => {
                    writeln!(s, "gl glBegin 4").unwrap();
                    for _ in 0..3 {
                        let w = [2.0, 0.5, 1.0, 0.25][(c(0, 31) * 8.0) as usize % 4];
                        let v = [c(8, 120) * w, c(8, 120) * w, c(-8, 8) * w / 8.0, w];
                        glf(s, "glVertex4f", &v);
                    }
                    writeln!(s, "gl glEnd").unwrap();
                }
            }
            read_all(s);
            writeln!(s, "glread 0 0 16 16 {} {} 1024", h(DEPTH_COMPONENT), h(UNSIGNED_INT)).unwrap();
        }
        writeln!(s, "glerr\nend").unwrap();
    }
    // glVertex4f corners of w = 0 (points, a triangle's corner, a line's end) and of w = 2
    // outside glBegin
    writeln!(s, "case persp-more-11\n{}gl glDisable 0xbd0\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700", ctx(32)).unwrap();
    let draws: [(u32, &[[f64; 4]]); 6] = [
        (0, &[[1.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [16.0, 16.0, 0.0, 2.0]]),
        (4, &[[4.0, 4.0, 0.0, 1.0], [12.0, 4.0, 0.0, 1.0], [1.0, 1.0, 0.0, 0.0]]),
        (4, &[[4.0, 4.0, 0.0, 1.0], [12.0, 4.0, 0.0, 1.0], [-1.0, 2.0, 0.0, 0.0]]),
        (1, &[[4.0, 4.0, 0.0, 1.0], [1.0, 0.5, 0.0, 0.0]]),
        (1, &[[8.0, 8.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]]),
        (0, &[]),
    ];
    for (i, (mode, vs)) in draws.iter().enumerate() {
        writeln!(s, "gl glClear 0x4000\ngl glColor4ub {} 200 100 255", 40 * i + 20).unwrap();
        if vs.is_empty() {
            glf(s, "glVertex4f", &[8.0, 8.0, 0.0, 2.0]);
        }
        writeln!(s, "gl glBegin {mode}").unwrap();
        for v in *vs {
            glf(s, "glVertex4f", v);
        }
        if vs.is_empty() {
            glf(s, "glVertex2f", &[3.0, 3.0]);
        }
        writeln!(s, "gl glEnd").unwrap();
        read_all(s);
    }
    writeln!(s, "glerr\nend").unwrap();
}
