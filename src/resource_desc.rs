/// a D3D12_RESOURCE_DESC as the executor's 11 tokens
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desc {
    pub dim: u32,
    pub align: u32,
    pub w: u32,
    pub h: u32,
    pub d: u32,
    pub mips: u32,
    pub fmt: u32,
    pub samples: u32,
    pub quality: u32,
    pub layout: u32,
    pub flags: u32,
}

impl Desc {
    pub fn words(&self) -> [u32; 11] {
        [self.dim, self.align, self.w, self.h, self.d, self.mips, self.fmt, self.samples, self.quality, self.layout, self.flags]
    }
    pub fn from_words(words: &[u32; 11]) -> Desc {
        let [dim, align, w, h, d, mips, fmt, samples, quality, layout, flags] = *words;
        Desc { dim, align, w, h, d, mips, fmt, samples, quality, layout, flags }
    }
    /// Values actually placed in the two UINT16 members of D3D12_RESOURCE_DESC.
    pub fn from_abi_words(words: &[u32]) -> Option<Desc> {
        let mut d = Self::from_words(words.try_into().ok()?);
        d.d &= 0xffff;
        d.mips &= 0xffff;
        Some(d)
    }
    pub fn buffer(w: u32) -> Desc {
        Desc { dim: 1, align: 0, w, h: 1, d: 1, mips: 1, fmt: 0, samples: 1, quality: 0, layout: 1, flags: 0 }
    }
    pub fn tex(dim: u32, w: u32, h: u32, d: u32, mips: u32, fmt: u32) -> Desc {
        Desc { dim, align: 0, w, h, d, mips, fmt, samples: 1, quality: 0, layout: 0, flags: 0 }
    }
}

