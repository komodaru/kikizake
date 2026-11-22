//! The other forms of the vertex calls (glColor, glNormal, glTexCoord, glVertex, glRasterPos,
//! glIndex, glRect, each type and vector form), the double matrix calls and glFogi, glFogiv as
//! the float calls the model follows (GL.VARIANTS; MEASURED, golden opengl-32 / 64 gap-forms-0, 1).

use super::array::{color_component, normal_component, DOUBLE};
use super::e::{BYTE, FLOAT, INT, SHORT, UNSIGNED_BYTE, UNSIGNED_INT, UNSIGNED_SHORT};
use super::A;

/// which call a form is of
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Color,
    Normal,
    TexCoord,
    Vertex,
    RasterPos,
    Index,
    Rect,
}

/// a form: its call, its count of values, their type and whether they come by a pointer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub family: Family,
    pub n: usize,
    pub ty: u32,
    pub vector: bool,
}

const FAMILIES: [(&str, Family); 7] = [
    ("glColor", Family::Color),
    ("glNormal", Family::Normal),
    ("glTexCoord", Family::TexCoord),
    ("glVertex", Family::Vertex),
    ("glRasterPos", Family::RasterPos),
    ("glIndex", Family::Index),
    ("glRect", Family::Rect),
];

const TYPES: [(&str, u32); 8] = [("ub", UNSIGNED_BYTE), ("us", UNSIGNED_SHORT), ("ui", UNSIGNED_INT), ("b", BYTE), ("s", SHORT), ("i", INT), ("f", FLOAT), ("d", DOUBLE)];

/// the forms the model follows as they are
const NATIVE: [&str; 20] = ["glRasterPos4f", "glIndexf", "glRectf", "glColor4ub", "glColor3ub", "glColor3f", "glColor4f", "glVertex2f", "glVertex3f", "glVertex4f", "glVertex2i", "glVertex3i", "glNormal3f", "glTexCoord1f", "glTexCoord2f", "glTexCoord3f", "glTexCoord4f", "glRasterPos2i", "glRasterPos2f", "glIndexi"];

/// the form a function name is of (glColor3b .. glRectsv), GL 1.1's list of them
pub fn parse(name: &str) -> Option<Form> {
    let (family, rest) = FAMILIES.iter().find_map(|(p, f)| name.strip_prefix(p).map(|r| (*f, r)))?;
    let (n, rest) = match family {
        Family::Index => (1, rest),
        Family::Rect => (4, rest),
        Family::Color | Family::Normal | Family::TexCoord | Family::Vertex | Family::RasterPos => {
            let d = rest.chars().next()?.to_digit(10)?;
            (usize::try_from(d).ok()?, rest.get(1..)?)
        }
    };
    let (vector, rest) = match rest.strip_suffix('v') {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let ty = TYPES.iter().find(|(s, _)| *s == rest).map(|(_, t)| *t)?;
    let signed = matches!(ty, SHORT | INT | FLOAT | DOUBLE);
    let ok = match family {
        Family::Color => matches!(n, 3 | 4),
        Family::Normal => n == 3 && (signed || ty == BYTE),
        Family::TexCoord => (1..=4).contains(&n) && signed,
        Family::Vertex | Family::RasterPos => (2..=4).contains(&n) && signed,
        Family::Index => signed || ty == UNSIGNED_BYTE,
        Family::Rect => signed,
    };
    ok.then_some(Form { family, n, ty, vector })
}

/// whether a gl line's function is one the model takes through its float form
pub fn routed(name: &str) -> bool {
    !NATIVE.contains(&name) && parse(name).is_some_and(|f| !f.vector)
}

/// a gl line's argument as a value of the form's type (the executor passes integers as C longs)
pub(super) fn raw(ty: u32, a: A) -> f64 {
    match a {
        A::F(x) => x,
        A::I(v) => {
            let [b0, b1, b2, b3, ..] = v.to_le_bytes();
            match ty {
                BYTE => f64::from(i8::from_le_bytes([b0])),
                UNSIGNED_BYTE => f64::from(b0),
                SHORT => f64::from(i16::from_le_bytes([b0, b1])),
                UNSIGNED_SHORT => f64::from(u16::from_le_bytes([b0, b1])),
                UNSIGNED_INT => f64::from(u32::from_le_bytes([b0, b1, b2, b3])),
                _ => f64::from(i32::from_le_bytes([b0, b1, b2, b3])),
            }
        }
    }
}

/// the float call a form's values stand for: colors and normals mapped as the color and normal
/// arrays map them (array::color_component: SHORT and UNSIGNED_SHORT times float32 1 / 65535,
/// INT and UNSIGNED_INT times 0x2f800004, the bytes (2v + 1) / 255 and v / 255), a three-form
/// color's alpha 1; the rest each value in float32
pub(super) fn canonical(f: Form, v: &[f64]) -> Option<(&'static str, Vec<A>)> {
    let fl = |x: f32| A::F(f64::from(x));
    let narrow = |k: usize| v.get(k).map(|x| super::tri::narrow(*x));
    let all = |n: usize| -> Option<Vec<A>> { (0..n).map(|k| narrow(k).map(fl)).collect() };
    Some(match f.family {
        Family::Color => {
            let c = |k: usize| v.get(k).map(|x| color_component(f.ty, *x));
            let alpha = if f.n == 4 { c(3)? } else { 1.0 };
            ("glColor4f", vec![fl(c(0)?), fl(c(1)?), fl(c(2)?), fl(alpha)])
        }
        Family::Normal => ("glNormal3f", (0..3).map(|k| v.get(k).map(|x| fl(normal_component(f.ty, *x)))).collect::<Option<Vec<A>>>()?),
        Family::TexCoord => (*["glTexCoord1f", "glTexCoord2f", "glTexCoord3f", "glTexCoord4f"].get(f.n.checked_sub(1)?)?, all(f.n)?),
        Family::Vertex => (*["glVertex2f", "glVertex3f", "glVertex4f"].get(f.n.checked_sub(2)?)?, all(f.n)?),
        Family::RasterPos => {
            let z = if f.n > 2 { narrow(2)? } else { 0.0 };
            let w = if f.n > 3 { narrow(3)? } else { 1.0 };
            ("glRasterPos4f", vec![fl(narrow(0)?), fl(narrow(1)?), fl(z), fl(w)])
        }
        Family::Index => ("glIndexf", all(1)?),
        Family::Rect => ("glRectf", all(4)?),
    })
}

/// the double matrix calls as their float forms, the arguments rounded (MEASURED, golden
/// opengl-32 / 64 gap-forms-1: glRotated, glScaled, glTranslated), glFogi as glFogf
pub(super) fn other(name: &str, a: &[A]) -> Option<(&'static str, Vec<A>)> {
    let f = |k: usize| a.get(k).map(|x| A::F(f64::from(super::tri::narrow(raw(FLOAT, *x)))));
    Some(match name {
        "glRotated" => ("glRotatef", vec![f(0)?, f(1)?, f(2)?, f(3)?]),
        "glScaled" => ("glScalef", vec![f(0)?, f(1)?, f(2)?]),
        "glTranslated" => ("glTranslatef", vec![f(0)?, f(1)?, f(2)?]),
        "glFogi" => ("glFogf", vec![*a.first()?, A::F(raw(INT, *a.get(1)?))]),
        _ => return None,
    })
}

/// glFogiv's values as floats: the fog color's mapped as an INT color array's
/// (array::color_component), held to [0, 1] (MEASURED, golden opengl-32 / 64 gap-forms-1: 0
/// gives 2^-32 and 4 ulps, read back 5 through glGet's channel round trip; 2^31 - 1 gives 1),
/// the others as they are
pub fn fog_ints(pname: u32, v: &[f64]) -> Vec<f32> {
    const FOG_COLOR: u32 = 0x0b66;
    v.iter()
        .map(|x| if pname == FOG_COLOR { color_component(INT, *x).clamp(0.0, 1.0) } else { super::tri::narrow(*x) })
        .collect()
}
