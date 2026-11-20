use crate::dxbc::*;

/// a program the cases use
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prog {
    Vs,
    Vsid,
    Ps,
    Psflat,
    Pspos,
    Psprim,
    Psld,
    Psldu,
    Pssample,
    Pssamplec,
}

impl Prog {
    pub const ALL: [Prog; 10] = [Prog::Vs, Prog::Vsid, Prog::Ps, Prog::Psflat, Prog::Pspos, Prog::Psprim, Prog::Psld, Prog::Psldu, Prog::Pssample, Prog::Pssamplec];
    /// its name, as the cases' comments and the model's messages give it
    pub fn name(self) -> &'static str {
        match self {
            Prog::Vs => "vs",
            Prog::Vsid => "vsid",
            Prog::Ps => "ps",
            Prog::Psflat => "psflat",
            Prog::Pspos => "pspos",
            Prog::Psprim => "psprim",
            Prog::Psld => "psld",
            Prog::Psldu => "psldu",
            Prog::Pssample => "pssample",
            Prog::Pssamplec => "pssamplec",
        }
    }
    /// a vertex shader (the others are pixel shaders)
    pub fn vertex(self) -> bool {
        match self {
            Prog::Vs | Prog::Vsid => true,
            Prog::Ps | Prog::Psflat | Prog::Pspos | Prog::Psprim | Prog::Psld | Prog::Psldu | Prog::Pssample | Prog::Pssamplec => false,
        }
    }
}

/// every program's bytes
pub struct Programs {
    vs: Vec<u8>,
    vsid: Vec<u8>,
    ps: Vec<u8>,
    psflat: Vec<u8>,
    pspos: Vec<u8>,
    psprim: Vec<u8>,
    psld: Vec<u8>,
    psldu: Vec<u8>,
    pssample: Vec<u8>,
    pssamplec: Vec<u8>,
}

impl Programs {
    pub fn get(&self, p: Prog) -> &[u8] {
        match p {
            Prog::Vs => &self.vs,
            Prog::Vsid => &self.vsid,
            Prog::Ps => &self.ps,
            Prog::Psflat => &self.psflat,
            Prog::Pspos => &self.pspos,
            Prog::Psprim => &self.psprim,
            Prog::Psld => &self.psld,
            Prog::Psldu => &self.psldu,
            Prog::Pssample => &self.pssample,
            Prog::Pssamplec => &self.pssamplec,
        }
    }
    /// the program these bytes are (the model knows each one by its bytes)
    pub fn find(&self, b: &[u8]) -> Option<Prog> {
        Prog::ALL.into_iter().find(|p| self.get(*p) == b)
    }
}

/// the programs the cases use
pub fn shaders() -> Programs {
    let f4 = |name, idx, reg, used| sig(name, idx, 0, 3, reg, 0xf, used);
    let pos_out = sig("SV_Position", 0, 1, 3, 0, 0xf, 0);
    let target = [sig("SV_Target", 0, 0, 3, 0, 0xf, 0)];
    let target_u = [sig("SV_Target", 0, 0, 1, 0, 0xf, 0)];
    let ret = ins(RET, &[]);
    let mov = |d: [u32; 2], s: [u32; 2]| ins(MOV, &[d, s].concat());
    // vs: TEXCOORD0 position, TEXCOORD1 color, passed on
    let vs = blob(1, &[f4("TEXCOORD", 0, 0, 0xf), f4("TEXCOORD", 1, 1, 0xf)], &[pos_out, f4("TEXCOORD", 0, 1, 0)], &cat(&[
        ins(DCL_INPUT, &dst(INPUT, 0, 0xf)),
        ins(DCL_INPUT, &dst(INPUT, 1, 0xf)),
        ins(DCL_OUTPUT_SIV, &[&dst(OUTPUT, 0, 0xf)[..], &[1]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 1, 0xf)),
        mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)),
        mov(dst(OUTPUT, 1, 0xf), src(INPUT, 1, XYZW)),
        ret.clone(),
    ]));
    // vsid: TEXCOORD0 position; the color is (vertex id, instance id, 0, 1) as floats
    let vsid = blob(1, &[f4("TEXCOORD", 0, 0, 0xf), sig("SV_VertexID", 0, 6, 1, 1, 1, 1), sig("SV_InstanceID", 0, 8, 1, 2, 1, 1)], &[pos_out, f4("TEXCOORD", 0, 1, 0)], &cat(&[
        ins(DCL_INPUT, &dst(INPUT, 0, 0xf)),
        ins(DCL_INPUT_SGV, &[&dst(INPUT, 1, 1)[..], &[6]].concat()),
        ins(DCL_INPUT_SGV, &[&dst(INPUT, 2, 1)[..], &[8]].concat()),
        ins(DCL_OUTPUT_SIV, &[&dst(OUTPUT, 0, 0xf)[..], &[1]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 1, 0xf)),
        mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)),
        ins(UTOF, &[dst(OUTPUT, 1, 1), src(INPUT, 1, 0)].concat()),
        ins(UTOF, &[dst(OUTPUT, 1, 2), src(INPUT, 2, 0)].concat()),
        ins(MOV, &[&dst(OUTPUT, 1, 0xc)[..], &imm([0, 0, 0, 0x3f80_0000])].concat()),
        ret.clone(),
    ]));
    let ps_in = |interp: u32, used: u32| (interp, [sig("SV_Position", 0, 1, 3, 0, 0xf, used), f4("TEXCOORD", 0, 1, 0xf)]);
    // ps: the color, interpolated (linear) / flat (constant)
    let [ps, psflat] = [LINEAR, CONSTANT].map(|interp| {
        let (i, isgn) = ps_in(interp, 0);
        blob(0, &isgn, &target, &cat(&[ins(DCL_INPUT_PS | i, &dst(INPUT, 1, 0xf)), ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)), mov(dst(OUTPUT, 0, 0xf), src(INPUT, 1, XYZW)), ret.clone()]))
    });
    // pspos: SV_Position
    let (_, isgn) = ps_in(0, 0xf);
    let pspos = blob(0, &isgn, &target, &cat(&[ins(DCL_INPUT_PS_SIV | NOPERSPECTIVE, &[&dst(INPUT, 0, 0xf)[..], &[1]].concat()), ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)), mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)), ret.clone()]));
    // psprim: (SV_PrimitiveID, SV_IsFrontFace & 1.0, 0, 1)
    // (the system generated values after what the vertex shader gives, as fxc places them)
    let isgn = [sig("SV_Position", 0, 1, 3, 0, 0xf, 0), f4("TEXCOORD", 0, 1, 0), sig("SV_PrimitiveID", 0, 7, 1, 2, 1, 1), sig("SV_IsFrontFace", 0, 9, 1, 3, 1, 1)];
    let psprim = blob(0, &isgn, &target, &cat(&[
        ins(DCL_INPUT_PS_SGV | CONSTANT, &[&dst(INPUT, 2, 1)[..], &[7]].concat()),
        ins(DCL_INPUT_PS_SGV | CONSTANT, &[&dst(INPUT, 3, 1)[..], &[9]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(UTOF, &[dst(OUTPUT, 0, 1), src(INPUT, 2, 0)].concat()),
        ins(AND, &[&dst(OUTPUT, 0, 2)[..], &src(INPUT, 3, 0), &imm([0x3f80_0000; 4])].concat()),
        ins(MOV, &[&dst(OUTPUT, 0, 0xc)[..], &imm([0, 0, 0, 0x3f80_0000])].concat()),
        ret.clone(),
    ]));
    // psld / psldu: texture 0 loaded at the pixel (mip 0), to a float / uint target
    let (_, isgn) = ps_in(0, 3);
    let [psld, psldu] = [(&target, 0x5555), (&target_u, 0x4444)].map(|(osgn, ret_ty)| {
        blob(0, &isgn, osgn, &cat(&[
            ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, ret_ty]),
            ins(DCL_INPUT_PS_SIV | NOPERSPECTIVE, &[&dst(INPUT, 0, 3)[..], &[1]].concat()),
            ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
            ins(DCL_TEMPS, &[1]),
            ins(FTOU, &[dst(TEMP, 0, 3), src(INPUT, 0, swz(0, 1, 0, 0))].concat()),
            ins(MOV, &[&dst(TEMP, 0, 0xc)[..], &imm([0; 4])].concat()),
            ins(LD, &[dst(OUTPUT, 0, 0xf), src(TEMP, 0, XYZW), src(RESOURCE, 0, XYZW)].concat()),
            ret.clone(),
        ]))
    });
    // pssample: texture 0 sampled with sampler 0 at the color's xy
    let (_, isgn) = ps_in(0, 0);
    let isgn = [isgn[0], f4("TEXCOORD", 0, 1, 3)];
    let pssample = blob(0, &isgn, &target, &cat(&[
        ins(DCL_SAMPLER, &[SAMPLER << 12 | 1 << 20, 0]),
        ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, 0x5555]),
        ins(DCL_INPUT_PS | LINEAR, &dst(INPUT, 1, 3)),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(SAMPLE, &[&dst(OUTPUT, 0, 0xf)[..], &src(INPUT, 1, swz(0, 1, 0, 0)), &src(RESOURCE, 0, XYZW), &[SAMPLER << 12 | 1 << 20, 0]].concat()),
        ret.clone(),
    ]));
    // pssamplec: texture 0 compared with sampler 0 (comparison mode) at the color's xy against
    // its z, the result in every channel
    let isgn = [isgn[0], f4("TEXCOORD", 0, 1, 7)];
    let pssamplec = blob(0, &isgn, &target, &cat(&[
        ins(DCL_SAMPLER | 1 << 11, &[SAMPLER << 12 | 1 << 20, 0]),
        ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, 0x5555]),
        ins(DCL_INPUT_PS | LINEAR, &dst(INPUT, 1, 7)),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(DCL_TEMPS, &[1]),
        ins(SAMPLE_C, &[&dst(TEMP, 0, 1)[..], &src(INPUT, 1, swz(0, 1, 0, 0)), &src(RESOURCE, 0, XYZW), &[SAMPLER << 12 | 1 << 20, 0], &src1(INPUT, 1, 2)].concat()),
        mov(dst(OUTPUT, 0, 0xf), src(TEMP, 0, 0)),
        ret.clone(),
    ]));
    Programs { vs, vsid, ps, psflat, pspos, psprim, psld, psldu, pssample, pssamplec }
}
