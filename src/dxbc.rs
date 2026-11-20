//! Shader model 4.0 programs as DXBC containers (input and output signatures, the token stream,
//! the container's checksum), written out here so that the executor needs no shader compiler.
//! Sources: Microsoft's d3d10TokenizedProgramFormat.hpp / d3d11TokenizedProgramFormat.hpp (token
//! layout, in the Windows SDK), the DXBC container (chunks ISGN, OSGN, SHDR; reflection data is
//! optional). The checksum is MD5 with the message length in the first and last words of the
//! final block (the same as vkd3d-shader's and Wine's); it was checked against 245 shaders
//! compiled by Microsoft's fxc.

/// a signature element: semantic, its index, system value (D3D_NAME), component type (1 uint,
/// 2 sint, 3 float), register, mask, and the mask the program reads (inputs) or 0
#[derive(Clone, Copy)]
pub struct Sig {
    pub name: &'static str,
    pub index: u32,
    pub sv: u32,
    pub ty: u32,
    pub reg: u32,
    pub mask: u32,
    pub used: u32,
}

pub const fn sig(name: &'static str, index: u32, sv: u32, ty: u32, reg: u32, mask: u32, used: u32) -> Sig {
    Sig { name, index, sv, ty, reg, mask, used }
}

fn signature(els: &[Sig]) -> Vec<u8> {
    let mut head = vec![els.len() as u32, 8];
    let mut names: Vec<u8> = Vec::new();
    let base = 8 + 24 * els.len();
    for e in els {
        head.extend([(base + names.len()) as u32, e.index, e.sv, e.ty, e.reg, e.mask | e.used << 8]);
        names.extend(e.name.as_bytes());
        names.push(0);
    }
    while names.len() % 4 != 0 {
        names.push(0xab);
    }
    let mut v: Vec<u8> = head.iter().flat_map(|w| w.to_le_bytes()).collect();
    v.extend(names);
    v
}

/// one MD5 block
fn md5_block(st: &mut [u32; 4], b: &[u8]) {
    const S: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
    let m: Vec<u32> = b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let [mut a, mut bb, mut c, mut d] = *st;
    for i in 0..64 {
        let k = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
        let (f, g) = match i / 16 {
            0 => ((bb & c) | (!bb & d), i),
            1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
            2 => (bb ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (bb | !d), (7 * i) % 16),
        };
        let f = f.wrapping_add(a).wrapping_add(k).wrapping_add(m[g]);
        a = d;
        d = c;
        c = bb;
        bb = bb.wrapping_add(f.rotate_left(S[i / 16 * 4 + i % 4]));
    }
    for (s, v) in st.iter_mut().zip([a, bb, c, d]) {
        *s = s.wrapping_add(v);
    }
}

/// the container checksum of everything after it (the bytes from offset 20)
pub fn checksum(data: &[u8]) -> [u32; 4] {
    let n = data.len();
    let mut st = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let full = n & !63;
    for b in data[..full].chunks(64) {
        md5_block(&mut st, b);
    }
    let left = &data[full..];
    let bits = (n as u32).wrapping_mul(8);
    let tail = |b: &mut Vec<u8>| {
        b.resize(60, 0);
        b.extend((bits >> 2 | 1).to_le_bytes());
    };
    if left.len() >= 56 {
        let mut b = left.to_vec();
        b.push(0x80);
        b.resize(64, 0);
        md5_block(&mut st, &b);
        let mut b = bits.to_le_bytes().to_vec();
        tail(&mut b);
        md5_block(&mut st, &b);
    } else {
        let mut b = bits.to_le_bytes().to_vec();
        b.extend(left);
        b.push(0x80);
        tail(&mut b);
        md5_block(&mut st, &b);
    }
    st
}

/// Whether this is a DXBC container with its declared size and checksum intact.
pub fn intact(bytes: &[u8]) -> bool {
    let (Some(size), Some(expected), Some(payload)) = (bytes.get(24..28), bytes.get(4..20), bytes.get(20..)) else {
        return false;
    };
    bytes.starts_with(b"DXBC") && u32::try_from(bytes.len()).is_ok_and(|n| size == n.to_le_bytes())
        && checksum(payload).iter().flat_map(|w| w.to_le_bytes()).eq(expected.iter().copied())
}

/// kind: 0 pixel, 1 vertex
pub fn blob(kind: u32, isgn: &[Sig], osgn: &[Sig], code: &[u32]) -> Vec<u8> {
    let mut shdr = vec![kind << 16 | 0x40, code.len() as u32 + 2];
    shdr.extend(code);
    let chunks: [(&[u8; 4], Vec<u8>); 3] = [(b"ISGN", signature(isgn)), (b"OSGN", signature(osgn)), (b"SHDR", shdr.iter().flat_map(|w| w.to_le_bytes()).collect())];
    let head = 32 + 4 * chunks.len();
    let total = head + chunks.iter().map(|c| 8 + c.1.len()).sum::<usize>();
    let mut v = b"DXBC".to_vec();
    v.extend([0u8; 16]);
    v.extend(1u32.to_le_bytes());
    v.extend((total as u32).to_le_bytes());
    v.extend((chunks.len() as u32).to_le_bytes());
    let mut at = head;
    for c in &chunks {
        v.extend((at as u32).to_le_bytes());
        at += 8 + c.1.len();
    }
    for (cc, d) in &chunks {
        v.extend(*cc);
        v.extend((d.len() as u32).to_le_bytes());
        v.extend(d);
    }
    let sum = checksum(&v[20..]);
    for (i, w) in sum.iter().enumerate() {
        v[4 + 4 * i..8 + 4 * i].copy_from_slice(&w.to_le_bytes());
    }
    v
}

// --- the token stream ---------------------------------------------------------------------

/// an instruction: opcode (with its control bits) and operand tokens, the length filled in
pub fn ins(opcode: u32, toks: &[u32]) -> Vec<u32> {
    let mut v = vec![opcode | (toks.len() as u32 + 1) << 24];
    v.extend(toks);
    v
}

pub const TEMP: u32 = 0;
pub const INPUT: u32 = 1;
pub const OUTPUT: u32 = 2;
pub const SAMPLER: u32 = 6;
pub const RESOURCE: u32 = 7;
pub const CB: u32 = 8;
pub const XYZW: u32 = 0xe4;

/// a destination register (4 components, write mask)
pub fn dst(ty: u32, reg: u32, mask: u32) -> [u32; 2] {
    [2 | mask << 4 | ty << 12 | 1 << 20, reg]
}
/// a source register, swizzled
pub fn src(ty: u32, reg: u32, swz: u32) -> [u32; 2] {
    [2 | 1 << 2 | swz << 4 | ty << 12 | 1 << 20, reg]
}
/// one component of a source register (a scalar operand)
pub fn src1(ty: u32, reg: u32, comp: u32) -> [u32; 2] {
    [2 | 2 << 2 | comp << 4 | ty << 12 | 1 << 20, reg]
}
/// cb<slot>[element], swizzled
pub fn cbsrc(slot: u32, el: u32, swz: u32) -> [u32; 3] {
    [2 | 1 << 2 | swz << 4 | CB << 12 | 2 << 20, slot, el]
}
/// l(a, b, c, d)
pub fn imm(v: [u32; 4]) -> [u32; 5] {
    [0x4002, v[0], v[1], v[2], v[3]]
}
/// a swizzle of components 0..3
pub fn swz(x: u32, y: u32, z: u32, w: u32) -> u32 {
    x | y << 2 | z << 4 | w << 6
}

pub const MOV: u32 = 0x36;
pub const ADD: u32 = 0x00;
pub const AND: u32 = 0x01;
pub const FTOU: u32 = 0x1c;
pub const UTOF: u32 = 0x56;
pub const LD: u32 = 0x2d;
pub const SAMPLE: u32 = 0x45;
pub const SAMPLE_C: u32 = 0x46;
pub const RET: u32 = 0x3e;
pub const DCL_INPUT: u32 = 0x5f;
pub const DCL_INPUT_SGV: u32 = 0x60;
pub const DCL_INPUT_PS: u32 = 0x62;
pub const DCL_INPUT_PS_SGV: u32 = 0x63;
pub const DCL_INPUT_PS_SIV: u32 = 0x64;
pub const DCL_OUTPUT: u32 = 0x65;
pub const DCL_OUTPUT_SIV: u32 = 0x67;
pub const DCL_TEMPS: u32 = 0x68;
pub const DCL_CB: u32 = 0x59;
pub const DCL_RESOURCE: u32 = 0x58;
pub const DCL_SAMPLER: u32 = 0x5a;
/// interpolation modes (dcl_input_ps bits 11..14)
pub const CONSTANT: u32 = 1 << 11;
pub const LINEAR: u32 = 2 << 11;
pub const NOPERSPECTIVE: u32 = 4 << 11;

pub fn cat(parts: &[Vec<u32>]) -> Vec<u32> {
    parts.concat()
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// a pixel shader written out here is byte for byte what fxc made of the same program (its
    /// checksum, 4 words of fxc's output, says so): float2 TEXCOORD0 in, SV_Target = (t.x, t.y, 1, 2)
    #[test]
    fn same_as_fxc() {
        let isgn = [sig("SV_Position", 0, 1, 3, 0, 0xf, 0), sig("TEXCOORD", 0, 0, 3, 1, 3, 3)];
        let osgn = [sig("SV_Target", 0, 0, 3, 0, 0xf, 0)];
        let code = cat(&[
            ins(DCL_INPUT_PS | LINEAR, &dst(INPUT, 1, 3)),
            ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
            ins(MOV, &[dst(OUTPUT, 0, 3), src(INPUT, 1, swz(0, 1, 0, 0))].concat()),
            ins(MOV, &[&dst(OUTPUT, 0, 0xc)[..], &imm([0, 0, 0x3f80_0000, 0x4000_0000])].concat()),
            ins(RET, &[]),
        ]);
        let b = blob(0, &isgn, &osgn, &code);
        assert_eq!(b.len(), 0x118);
        let sum: Vec<u32> = b[4..20].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        assert_eq!(sum, [0x7cc601b6, 0xc65b8bdb, 0x54d0f606, 0x9cc74d3d]);
    }
}
