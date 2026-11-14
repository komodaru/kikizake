//! TrueType files for the DirectWrite suite: a writer (the generator makes small fonts whose
//! glyphs are rectangles, every table field chosen by the case) and a reader (the model reads
//! the font a case gives, as DirectWrite would). Source: the OpenType specification (Microsoft
//! Learn, "OpenType specification": the table directory, head, hhea, maxp, OS/2, post, name,
//! cmap formats 4 and 12, loca, glyf simple glyphs, vhea, vmtx).

/// one glyph: its advance and its outline, rectangles (x0, y0, x1, y1) in font units
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glyph {
    pub adv: u16,
    pub rects: Vec<(i16, i16, i16, i16)>,
    /// vmtx: (advance height, top side bearing)
    pub vert: (u16, i16),
}

/// everything the writer puts in a font
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub upm: u16,
    /// hhea ascender, descender, lineGap
    pub hhea: (i16, i16, i16),
    /// OS/2 version, or none
    pub os2: Option<u16>,
    pub typo: (i16, i16, i16),
    pub win: (u16, u16),
    pub fs_selection: u16,
    pub x_height: i16,
    pub cap_height: i16,
    /// yStrikeoutSize, yStrikeoutPosition
    pub strike: (i16, i16),
    /// post underlinePosition, underlineThickness
    pub underline: (i16, i16),
    /// ySubscript X size, Y size, X offset, Y offset, then the same for superscript
    pub script: [i16; 8],
    pub fixed_pitch: bool,
    /// a (3, 0) symbol cmap in place of (3, 1)
    pub symbol: bool,
    /// vhea and vmtx
    pub vertical: bool,
    pub family: String,
    pub glyphs: Vec<Glyph>,
    /// code point -> glyph, sorted by code point
    pub cmap: Vec<(u32, u16)>,
}

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn checksum(t: &[u8]) -> u32 {
    t.chunks(4).fold(0u32, |s, c| {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(b))
    })
}

impl Font {
    fn bbox(g: &Glyph) -> Option<(i16, i16, i16, i16)> {
        let mut it = g.rects.iter();
        let f = *it.next()?;
        Some(it.fold(f, |b, r| (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3))))
    }

    fn glyf(&self) -> (Vec<u8>, Vec<u32>) {
        let mut glyf = Vec::new();
        let mut loca = vec![0u32];
        for g in &self.glyphs {
            if let Some((x0, y0, x1, y1)) = Self::bbox(g) {
                be16(&mut glyf, g.rects.len() as u16);
                for v in [x0, y0, x1, y1] {
                    be16(&mut glyf, v as u16);
                }
                for i in 0..g.rects.len() {
                    be16(&mut glyf, (i * 4 + 3) as u16);
                }
                be16(&mut glyf, 0); // no instructions
                glyf.extend(std::iter::repeat(1u8).take(g.rects.len() * 4)); // on curve, 16-bit deltas
                // (each rectangle clockwise: up the left edge, along the top, down the right)
                let pts: Vec<(i16, i16)> = g.rects.iter().flat_map(|&(x0, y0, x1, y1)| [(x0, y0), (x0, y1), (x1, y1), (x1, y0)]).collect();
                let mut last = 0i16;
                for p in &pts {
                    be16(&mut glyf, p.0.wrapping_sub(last) as u16);
                    last = p.0;
                }
                last = 0;
                for p in &pts {
                    be16(&mut glyf, p.1.wrapping_sub(last) as u16);
                    last = p.1;
                }
                while glyf.len() % 4 != 0 {
                    glyf.push(0);
                }
            }
            loca.push(glyf.len() as u32);
        }
        (glyf, loca)
    }

    fn cmap_table(&self) -> Vec<u8> {
        let bmp: Vec<(u32, u16)> = self.cmap.iter().copied().filter(|c| c.0 < 0xffff).collect();
        let wide = self.cmap.iter().any(|c| c.0 > 0xffff);
        // format 4: a segment for each code point, then the final 0xFFFF one
        let mut f4 = Vec::new();
        let seg = bmp.len() + 1;
        let mut sr = 1;
        let mut es = 0;
        while sr * 2 <= seg {
            sr *= 2;
            es += 1;
        }
        be16(&mut f4, 4);
        be16(&mut f4, (16 + 8 * seg) as u16);
        be16(&mut f4, 0);
        be16(&mut f4, (seg * 2) as u16);
        be16(&mut f4, (sr * 2) as u16);
        be16(&mut f4, es);
        be16(&mut f4, (seg * 2 - sr * 2) as u16);
        for c in &bmp {
            be16(&mut f4, c.0 as u16);
        }
        be16(&mut f4, 0xffff);
        be16(&mut f4, 0);
        for c in &bmp {
            be16(&mut f4, c.0 as u16);
        }
        be16(&mut f4, 0xffff);
        for c in &bmp {
            be16(&mut f4, c.1.wrapping_sub(c.0 as u16));
        }
        be16(&mut f4, 1);
        for _ in 0..seg {
            be16(&mut f4, 0);
        }
        let mut f12 = Vec::new();
        if wide {
            be16(&mut f12, 12);
            be16(&mut f12, 0);
            be32(&mut f12, 16 + 12 * self.cmap.len() as u32);
            be32(&mut f12, 0);
            be32(&mut f12, self.cmap.len() as u32);
            for c in &self.cmap {
                be32(&mut f12, c.0);
                be32(&mut f12, c.0);
                be32(&mut f12, c.1 as u32);
            }
        }
        let n = if wide { 2 } else { 1 };
        let mut t = Vec::new();
        be16(&mut t, 0);
        be16(&mut t, n);
        be16(&mut t, 3);
        be16(&mut t, if self.symbol { 0 } else { 1 });
        be32(&mut t, 4 + 8 * n as u32);
        if wide {
            be16(&mut t, 3);
            be16(&mut t, 10);
            be32(&mut t, 4 + 16 + f4.len() as u32);
        }
        t.extend(f4);
        t.extend(f12);
        t
    }

    fn name_table(&self) -> Vec<u8> {
        let full = format!("{} Regular", self.family);
        let ps: String = self.family.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>() + "-Regular";
        let recs: [(u16, &str); 5] = [(1, &self.family), (2, "Regular"), (3, &full), (4, &full), (6, &ps)];
        let mut strings = Vec::new();
        let mut t = Vec::new();
        be16(&mut t, 0);
        be16(&mut t, recs.len() as u16);
        be16(&mut t, (6 + 12 * recs.len()) as u16);
        for (id, s) in recs {
            let off = strings.len();
            for u in s.encode_utf16() {
                be16(&mut strings, u);
            }
            for v in [3, 1, 0x409, id, (strings.len() - off) as u16, off as u16] {
                be16(&mut t, v);
            }
        }
        t.extend(strings);
        t
    }

    /// the font file
    pub fn bytes(&self) -> Vec<u8> {
        let n = self.glyphs.len() as u16;
        let (glyf, loca) = self.glyf();
        let boxes: Vec<_> = self.glyphs.iter().filter_map(Self::bbox).collect();
        let fold = |f: fn(&(i16, i16, i16, i16)) -> i16, min: bool| boxes.iter().map(f).reduce(|a, b| if min { a.min(b) } else { a.max(b) }).unwrap_or(0);
        let (xmin, ymin, xmax, ymax) = (fold(|b| b.0, true), fold(|b| b.1, true), fold(|b| b.2, false), fold(|b| b.3, false));
        let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
        // head
        let mut t = Vec::new();
        be32(&mut t, 0x0001_0000);
        be32(&mut t, 0x0001_0000);
        be32(&mut t, 0); // checkSumAdjustment, set below
        be32(&mut t, 0x5F0F_3CF5);
        be16(&mut t, 0x000B);
        be16(&mut t, self.upm);
        t.extend([0u8; 16]);
        for v in [xmin, ymin, xmax, ymax] {
            be16(&mut t, v as u16);
        }
        be16(&mut t, 0);
        be16(&mut t, 8);
        be16(&mut t, 2);
        be16(&mut t, 1);
        be16(&mut t, 0);
        tables.push((*b"head", t));
        // hhea
        let mut t = Vec::new();
        be32(&mut t, 0x0001_0000);
        for v in [self.hhea.0, self.hhea.1, self.hhea.2] {
            be16(&mut t, v as u16);
        }
        let advmax = self.glyphs.iter().map(|g| g.adv).max().unwrap_or(0);
        be16(&mut t, advmax);
        let lsb = |g: &Glyph| Self::bbox(g).map_or(0, |b| b.0);
        let rsb = |g: &Glyph| Self::bbox(g).map_or(0, |b| g.adv as i16 - b.2);
        be16(&mut t, self.glyphs.iter().map(lsb).min().unwrap_or(0) as u16);
        be16(&mut t, self.glyphs.iter().map(rsb).min().unwrap_or(0) as u16);
        be16(&mut t, xmax as u16);
        be16(&mut t, 1);
        be16(&mut t, 0);
        be16(&mut t, 0);
        t.extend([0u8; 8]);
        be16(&mut t, 0);
        be16(&mut t, n);
        tables.push((*b"hhea", t));
        // maxp 1.0
        let mut t = Vec::new();
        be32(&mut t, 0x0001_0000);
        be16(&mut t, n);
        let maxc = self.glyphs.iter().map(|g| g.rects.len()).max().unwrap_or(0) as u16;
        be16(&mut t, maxc * 4);
        be16(&mut t, maxc);
        for v in [0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
            be16(&mut t, v);
        }
        tables.push((*b"maxp", t));
        // OS/2
        if let Some(ver) = self.os2 {
            let mut t = Vec::new();
            be16(&mut t, ver);
            let avg = if n > 0 { self.glyphs.iter().map(|g| g.adv as u32).sum::<u32>() / n as u32 } else { 0 };
            be16(&mut t, avg as u16);
            be16(&mut t, 400);
            be16(&mut t, 5);
            be16(&mut t, 0);
            for v in &self.script {
                be16(&mut t, *v as u16);
            }
            be16(&mut t, self.strike.0 as u16);
            be16(&mut t, self.strike.1 as u16);
            be16(&mut t, 0);
            t.extend([2u8, 0, 5, 3, 0, 0, 0, 0, 0, 0]);
            be32(&mut t, 1);
            be32(&mut t, 0);
            be32(&mut t, 0);
            be32(&mut t, 0);
            t.extend(b"KZKE");
            be16(&mut t, self.fs_selection);
            let first = self.cmap.first().map_or(0, |c| c.0.min(0xffff)) as u16;
            let last = self.cmap.last().map_or(0, |c| c.0.min(0xffff)) as u16;
            be16(&mut t, first);
            be16(&mut t, last);
            for v in [self.typo.0, self.typo.1, self.typo.2] {
                be16(&mut t, v as u16);
            }
            be16(&mut t, self.win.0);
            be16(&mut t, self.win.1);
            if ver >= 1 {
                be32(&mut t, 1);
                be32(&mut t, 0);
            }
            if ver >= 2 {
                be16(&mut t, self.x_height as u16);
                be16(&mut t, self.cap_height as u16);
                be16(&mut t, 0);
                be16(&mut t, 32);
                be16(&mut t, 1);
            }
            if ver >= 5 {
                be16(&mut t, 0);
                be16(&mut t, 0xffff);
            }
            tables.push((*b"OS/2", t));
        }
        tables.push((*b"cmap", self.cmap_table()));
        let mut t = Vec::new();
        for g in &self.glyphs {
            be16(&mut t, g.adv);
            be16(&mut t, Self::bbox(g).map_or(0, |b| b.0) as u16);
        }
        tables.push((*b"hmtx", t));
        let mut t = Vec::new();
        for o in &loca {
            be32(&mut t, *o);
        }
        tables.push((*b"loca", t));
        tables.push((*b"glyf", glyf));
        tables.push((*b"name", self.name_table()));
        let mut t = Vec::new();
        be32(&mut t, 0x0003_0000);
        be32(&mut t, 0);
        be16(&mut t, self.underline.0 as u16);
        be16(&mut t, self.underline.1 as u16);
        be32(&mut t, self.fixed_pitch as u32);
        t.extend([0u8; 16]);
        tables.push((*b"post", t));
        if self.vertical {
            let mut t = Vec::new();
            be32(&mut t, 0x0001_1000);
            for v in [self.hhea.0, self.hhea.1, self.hhea.2] {
                be16(&mut t, v as u16);
            }
            be16(&mut t, self.glyphs.iter().map(|g| g.vert.0).max().unwrap_or(0));
            for _ in 0..3 {
                be16(&mut t, 0);
            }
            be16(&mut t, 0);
            be16(&mut t, 1);
            be16(&mut t, 0);
            t.extend([0u8; 8]);
            be16(&mut t, 0);
            be16(&mut t, n);
            tables.push((*b"vhea", t));
            let mut t = Vec::new();
            for g in &self.glyphs {
                be16(&mut t, g.vert.0);
                be16(&mut t, g.vert.1 as u16);
            }
            tables.push((*b"vmtx", t));
        }
        tables.sort_by(|a, b| a.0.cmp(&b.0));
        let nt = tables.len();
        let mut sr = 1;
        let mut es = 0u16;
        while sr * 2 <= nt {
            sr *= 2;
            es += 1;
        }
        let mut f = Vec::new();
        be32(&mut f, 0x0001_0000);
        be16(&mut f, nt as u16);
        be16(&mut f, (sr * 16) as u16);
        be16(&mut f, es);
        be16(&mut f, ((nt - sr) * 16) as u16);
        let mut off = 12 + 16 * nt;
        let mut body = Vec::new();
        for (tag, t) in &tables {
            f.extend(tag);
            be32(&mut f, checksum(t));
            be32(&mut f, off as u32);
            be32(&mut f, t.len() as u32);
            body.extend(t);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            off = 12 + 16 * nt + body.len();
        }
        f.extend(body);
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&f));
        let head = tables.iter().position(|t| &t.0 == b"head").unwrap();
        let ho = u32::from_be_bytes(f[12 + 16 * head + 8..12 + 16 * head + 12].try_into().unwrap()) as usize;
        f[ho + 8..ho + 12].copy_from_slice(&adj.to_be_bytes());
        f
    }
}

/// what the reader takes from a font file
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub upm: u16,
    /// head xMin, yMin, xMax, yMax
    pub head_box: (i16, i16, i16, i16),
    pub hhea: Option<(i16, i16, i16)>,
    pub num_glyphs: u16,
    pub os2: Option<Os2>,
    /// post underlinePosition, underlineThickness, isFixedPitch
    pub post: Option<(i16, i16, u32)>,
    /// hmtx (advance, lsb) of each glyph
    pub hmtx: Vec<(u16, i16)>,
    /// vhea present: vmtx (advance height, top side bearing) of each glyph
    pub vmtx: Option<Vec<(u16, i16)>>,
    /// glyf: each glyph's bounding box and its contours (on-curve points only)
    pub glyphs: Vec<Option<((i16, i16, i16, i16), Vec<Vec<(i16, i16)>>)>>,
    /// code point -> glyph from the (3, 10) or (3, 1) subtable, or (3, 0)
    pub cmap: Vec<(u32, u16)>,
    pub symbol: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Os2 {
    pub version: u16,
    pub script: [i16; 8],
    pub strike: (i16, i16),
    pub fs_selection: u16,
    pub typo: (i16, i16, i16),
    pub win: (u16, u16),
    pub x_height: i16,
    pub cap_height: i16,
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        Some(u16::from_be_bytes(self.0.get(o..o + 2)?.try_into().ok()?))
    }
    fn i16(&self, o: usize) -> Option<i16> {
        self.u16(o).map(|v| v as i16)
    }
    fn u32(&self, o: usize) -> Option<u32> {
        Some(u32::from_be_bytes(self.0.get(o..o + 4)?.try_into().ok()?))
    }
}

/// the tables of a font file: tag -> bytes
fn tables(f: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let r = R(f);
    if r.u32(0)? != 0x0001_0000 {
        return None;
    }
    let n = r.u16(4)? as usize;
    let mut v = Vec::new();
    for i in 0..n {
        let e = 12 + 16 * i;
        let tag: [u8; 4] = f.get(e..e + 4)?.try_into().ok()?;
        let (o, l) = (r.u32(e + 8)? as usize, r.u32(e + 12)? as usize);
        v.push((tag, f.get(o..o.checked_add(l)?)?));
    }
    Some(v)
}

/// a font as the writer makes them (anything else: None)
pub fn parse(f: &[u8]) -> Option<Parsed> {
    let t = tables(f)?;
    let get = |tag: &[u8; 4]| t.iter().find(|x| &x.0 == tag).map(|x| R(x.1));
    let mut p = Parsed::default();
    let head = get(b"head")?;
    if head.u32(12)? != 0x5F0F_3CF5 {
        return None;
    }
    p.upm = head.u16(18)?;
    p.head_box = (head.i16(36)?, head.i16(38)?, head.i16(40)?, head.i16(42)?);
    let long_loca = head.i16(50)? == 1;
    let maxp = get(b"maxp")?;
    p.num_glyphs = maxp.u16(4)?;
    let hhea = get(b"hhea")?;
    p.hhea = Some((hhea.i16(4)?, hhea.i16(6)?, hhea.i16(8)?));
    let nh = hhea.u16(34)? as usize;
    let hmtx = get(b"hmtx")?;
    let mut last = (0, 0);
    for g in 0..p.num_glyphs as usize {
        if g < nh {
            last = (hmtx.u16(4 * g)?, hmtx.i16(4 * g + 2)?);
            p.hmtx.push(last);
        } else {
            p.hmtx.push((last.0, hmtx.i16(4 * nh + 2 * (g - nh))?));
        }
    }
    if let Some(vhea) = get(b"vhea") {
        let nv = vhea.u16(34)? as usize;
        let vmtx = get(b"vmtx")?;
        let mut v = Vec::new();
        let mut last = (0, 0);
        for g in 0..p.num_glyphs as usize {
            if g < nv {
                last = (vmtx.u16(4 * g)?, vmtx.i16(4 * g + 2)?);
                v.push(last);
            } else {
                v.push((last.0, vmtx.i16(4 * nv + 2 * (g - nv))?));
            }
        }
        p.vmtx = Some(v);
    }
    if let Some(o) = get(b"OS/2") {
        let version = o.u16(0)?;
        let mut script = [0i16; 8];
        for (i, s) in script.iter_mut().enumerate() {
            *s = o.i16(10 + 2 * i)?;
        }
        let mut os2 = Os2 { version, script, strike: (o.i16(26)?, o.i16(28)?), fs_selection: o.u16(62)?, typo: (o.i16(68)?, o.i16(70)?, o.i16(72)?), win: (o.u16(74)?, o.u16(76)?), ..Default::default() };
        if version >= 2 {
            os2.x_height = o.i16(86)?;
            os2.cap_height = o.i16(88)?;
        }
        p.os2 = Some(os2);
    }
    if let Some(post) = get(b"post") {
        p.post = Some((post.i16(8)?, post.i16(10)?, post.u32(12)?));
    }
    let loca = get(b"loca")?;
    let glyf = get(b"glyf")?;
    for g in 0..p.num_glyphs as usize {
        let (a, b) = if long_loca { (loca.u32(4 * g)? as usize, loca.u32(4 * g + 4)? as usize) } else { (loca.u16(2 * g)? as usize * 2, loca.u16(2 * g + 2)? as usize * 2) };
        if a == b {
            p.glyphs.push(None);
            continue;
        }
        let d = R(glyf.0.get(a..b)?);
        let nc = d.i16(0)?;
        if nc < 0 {
            return None; // (composite glyphs: the writer makes none)
        }
        let bbox = (d.i16(2)?, d.i16(4)?, d.i16(6)?, d.i16(8)?);
        let mut ends = Vec::new();
        for i in 0..nc as usize {
            ends.push(d.u16(10 + 2 * i)? as usize);
        }
        let npts = ends.last().map_or(0, |e| e + 1);
        let mut o = 10 + 2 * nc as usize;
        o += 2 + d.u16(o)? as usize;
        // flags (the writer's: on curve, no repeat, 16-bit deltas)
        for i in 0..npts {
            if *d.0.get(o + i)? != 1 {
                return None;
            }
        }
        o += npts;
        let mut xs = Vec::new();
        let mut x = 0i16;
        for i in 0..npts {
            x = x.wrapping_add(d.i16(o + 2 * i)?);
            xs.push(x);
        }
        o += 2 * npts;
        let mut pts = Vec::new();
        let mut y = 0i16;
        for (i, x) in xs.iter().enumerate() {
            y = y.wrapping_add(d.i16(o + 2 * i)?);
            pts.push((*x, y));
        }
        let mut contours = Vec::new();
        let mut s = 0;
        for e in ends {
            contours.push(pts.get(s..=e)?.to_vec());
            s = e + 1;
        }
        p.glyphs.push(Some((bbox, contours)));
    }
    let cmap = get(b"cmap")?;
    let nsub = cmap.u16(2)? as usize;
    let mut subs = Vec::new();
    for i in 0..nsub {
        subs.push((cmap.u16(4 + 8 * i)?, cmap.u16(6 + 8 * i)?, cmap.u32(8 + 8 * i)? as usize));
    }
    let pick = subs.iter().find(|s| (s.0, s.1) == (3, 10)).or_else(|| subs.iter().find(|s| (s.0, s.1) == (3, 1))).or_else(|| subs.iter().find(|s| (s.0, s.1) == (3, 0)))?;
    p.symbol = pick.1 == 0;
    let o = pick.2;
    match cmap.u16(o)? {
        4 => {
            let seg = cmap.u16(o + 6)? as usize / 2;
            for s in 0..seg {
                let end = cmap.u16(o + 14 + 2 * s)?;
                let start = cmap.u16(o + 16 + 2 * seg + 2 * s)?;
                let delta = cmap.u16(o + 16 + 4 * seg + 2 * s)?;
                let ro = cmap.u16(o + 16 + 6 * seg + 2 * s)?;
                if ro != 0 || end == 0xffff {
                    continue;
                }
                for c in start..=end {
                    p.cmap.push((c as u32, c.wrapping_add(delta)));
                }
            }
        }
        12 => {
            let n = cmap.u32(o + 12)? as usize;
            for g in 0..n {
                let (a, b, gid) = (cmap.u32(o + 16 + 12 * g)?, cmap.u32(o + 20 + 12 * g)?, cmap.u32(o + 24 + 12 * g)?);
                for c in a..=b.min(a.saturating_add(0x10000)) {
                    p.cmap.push((c, (gid + c - a) as u16));
                }
            }
        }
        _ => return None,
    }
    p.cmap.sort();
    Some(p)
}

/// the glyph of a code point (0: none)
pub fn glyph_of(p: &Parsed, c: u32) -> u16 {
    p.cmap.binary_search_by_key(&c, |e| e.0).map_or(0, |i| p.cmap[i].1)
}
