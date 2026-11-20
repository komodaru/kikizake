//! GDI's inverse color table for gradients into color tables (GDI.GRAD.INDEXED): an
//! incremental inverse colormap over 5 bit cells whose green and blue scans keep bounds that
//! are never set (0), so some cells get an entry that is not the nearest and some none.
//!
//! Source: the clean-room note on the runner's win32kfull (gradients into color tables), a
//! step by step description of the builder; confirmed by golden blend gradrect8-*,
//! gradrect-dither-*.

/// a 5 bit cell (red << 10 | green << 5 | blue)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell(pub u16);

impl Cell {
    pub fn of(r: u8, g: u8, b: u8) -> Cell {
        Cell(u16::from(r & 31) << 10 | u16::from(g & 31) << 5 | u16::from(b & 31))
    }
}

/// the table: each cell's entry, None for a cell the builder never writes (Windows reads
/// what the memory held before)
pub struct Table(Vec<Option<u8>>);

impl Table {
    pub fn get(&self, c: Cell) -> Option<u8> {
        self.0.get(usize::from(c.0)).copied().flatten()
    }
}

struct Build {
    dist: Vec<u32>,
    tab: Vec<Option<u8>>,
    entry: u8,
    // the scans' state, kept across the loops as GDI keeps it
    ghere: i64,
    ginc: i64,
    bhere: i64,
    binc: i64,
    rdist: i64,
    gdist: i64,
    gptr: i64,
    bptr: i64,
    // the entry's cell and increments
    gc: i64,
    bc: i64,
    cginc: i64,
    cbinc: i64,
}

const XX: i64 = 128;

impl Build {
    fn dist(&self, p: i64) -> u32 {
        usize::try_from(p).ok().and_then(|p| self.dist.get(p)).copied().unwrap_or(0)
    }
    fn set(&mut self, p: i64, d: i64) {
        if let Ok(p) = usize::try_from(p) && p < self.dist.len() {
            self.dist[p] = d as u32;
            self.tab[p] = Some(self.entry);
        }
    }
    /// whether the cell at p is farther than d (unsigned, as GDI compares)
    fn farther(&self, p: i64, d: i64) -> bool {
        self.dist(p) > d as u32
    }

    fn red(&mut self, rc: i64, cdist: i64, crinc: i64, cptr: i64) {
        self.rdist = cdist;
        let mut rxx = crinc;
        self.gptr = cptr;
        let mut detect = false;
        let mut first = true;
        for _ in rc..=31 {
            if self.green(first) {
                detect = true;
            } else if detect {
                break;
            }
            self.gptr += 1024;
            self.rdist += rxx;
            rxx += XX;
            first = false;
        }
        rxx = crinc - XX;
        self.rdist = cdist - rxx;
        self.gptr = cptr - 1024;
        first = true;
        for _ in (0..rc).rev() {
            if self.green(first) {
                detect = true;
            } else if detect {
                break;
            }
            self.gptr -= 1024;
            rxx -= XX;
            self.rdist -= rxx;
            first = false;
        }
    }

    fn green(&mut self, restart: bool) -> bool {
        // (the bounds GDI never sets)
        let (lo, mut hi) = if restart { (0, 31) } else { (0, 0) };
        if restart {
            self.ghere = self.gc;
            self.ginc = self.cginc;
        }
        let mut g = self.ghere;
        let mut detect = false;
        let mut gcd = self.rdist;
        self.gdist = self.rdist;
        let mut gxx = self.ginc;
        let mut first = true;
        self.bptr = self.gptr;
        let mut gp = self.gptr;
        while g <= hi {
            if self.blue(first) {
                if !detect {
                    if g > self.ghere {
                        self.ghere = g;
                        self.gptr = gp;
                        self.rdist = gcd;
                        self.ginc = gxx;
                    }
                    detect = true;
                }
            } else if detect {
                hi = g - 1;
                break;
            }
            gcd += gxx;
            g += 1;
            self.bptr += 32;
            gp += 32;
            self.gdist += gxx;
            gxx += XX;
            first = false;
        }
        let _ = hi;
        gxx = self.ginc - XX;
        g = self.ghere - 1;
        gcd = self.rdist - gxx;
        self.gdist = gcd;
        self.bptr = self.gptr - 32;
        gp = self.bptr;
        first = true;
        while g >= lo {
            if self.blue(first) {
                if !detect {
                    self.ghere = g;
                    self.gptr = gp;
                    self.rdist = gcd;
                    self.ginc = gxx;
                    detect = true;
                }
            } else if detect {
                break;
            }
            gxx -= XX;
            g -= 1;
            gcd -= gxx;
            self.bptr -= 32;
            gp -= 32;
            self.gdist -= gxx;
            first = false;
        }
        detect
    }

    fn blue(&mut self, restart: bool) -> bool {
        let (lo, hi) = if restart { (0, 31) } else { (0, 0) };
        if restart {
            self.bhere = self.bc;
            self.binc = self.cbinc;
        }
        let mut detect = false;
        let mut b = self.bhere;
        let h0 = self.bhere;
        let mut d = self.gdist;
        let mut bxx = self.binc;
        let mut p = self.bptr;
        while b <= hi && !self.farther(p, d) {
            b += 1;
            p += 1;
            d += bxx;
            bxx += XX;
        }
        if b <= hi {
            if b > h0 {
                self.bhere = b;
                self.bptr = p;
                self.gdist = d;
                self.binc = bxx;
            }
            detect = true;
            loop {
                if !self.farther(p, d) {
                    break;
                }
                self.set(p, d);
                b += 1;
                p += 1;
                d += bxx;
                bxx += XX;
                if b > hi {
                    break;
                }
            }
        }
        bxx = self.binc - XX;
        b = self.bhere - 1;
        d = self.gdist - bxx;
        p = self.bptr - 1;
        if !detect {
            while b >= lo {
                if self.farther(p, d) {
                    self.bhere = b;
                    self.bptr = p;
                    self.gdist = d;
                    self.binc = bxx;
                    detect = true;
                    break;
                }
                b -= 1;
                p -= 1;
                bxx -= XX;
                d -= bxx;
            }
            if !detect {
                return false;
            }
        }
        while b >= lo && self.farther(p, d) {
            self.set(p, d);
            b -= 1;
            p -= 1;
            bxx -= XX;
            d -= bxx;
        }
        detect
    }
}

/// the table GDI builds for a color table (0xRRGGBB entries, at most 256)
pub fn table(pal: &[u32]) -> Table {
    let mut s = Build { dist: vec![u32::MAX; 32768], tab: vec![None; 32768], entry: 0, ghere: 0, ginc: 0, bhere: 0, binc: 0, rdist: 0, gdist: 0, gptr: 0, bptr: 0, gc: 0, bc: 0, cginc: 0, cbinc: 0 };
    for (i, &e) in pal.iter().enumerate().take(256) {
        let (r, g, b) = (i64::from(e >> 16 & 0xff), i64::from(e >> 8 & 0xff), i64::from(e & 0xff));
        let (rc, gc, bc) = (r >> 3, g >> 3, b >> 3);
        let cdist = (r - 8 * rc - 4).pow(2) + (g - 8 * gc - 4).pow(2) + (b - 8 * bc - 4).pow(2);
        let inc = |c: i64, v: i64| 2 * ((c + 1) * 64 - 8 * v);
        s.entry = i as u8;
        (s.gc, s.bc, s.cginc, s.cbinc) = (gc, bc, inc(gc, g), inc(bc, b));
        s.red(rc, cdist, inc(rc, r), rc * 1024 + gc * 32 + bc);
    }
    Table(s.tab)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the note's results at the 27 cells of the 3 levels (0, 16, 31 a channel)
    #[test]
    fn the_notes_cells() {
        let p332: Vec<u32> = (0..256u32).map(|i| ((i >> 5 & 7) * 255 / 7) << 16 | ((i >> 2 & 7) * 255 / 7) << 8 | (i & 3) * 255 / 3).collect();
        let vga = [0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080, 0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff];
        let lv = [0u8, 16, 31];
        let show = |t: &Table| -> String {
            let mut v = Vec::new();
            for r in 0..3 {
                for g in 0..3 {
                    for b in 0..3 {
                        v.push(match t.get(Cell::of(lv[r], lv[g], lv[b])) {
                            Some(x) => format!("{r}{g}{b}:{x:x}"),
                            None => format!("{r}{g}{b}:--"),
                        });
                    }
                }
            }
            v.join(" ")
        };
        assert_eq!(show(&table(&p332)), "000:0 001:2 002:3 010:10 011:12 012:-- 020:1c 021:1e 022:1f 100:80 101:82 102:83 110:90 111:92 112:-- 120:-- 121:-- 122:-- 200:e0 201:e2 202:e3 210:f0 211:f2 212:-- 220:fc 221:fe 222:ff");
        assert_eq!(show(&table(&vga)), "000:0 001:4 002:c 010:2 011:6 012:6 020:a 021:e 022:e 100:1 101:5 102:d 110:3 111:8 112:8 120:3 121:-- 122:-- 200:9 201:d 202:d 210:b 211:7 212:-- 220:b 221:f 222:f");
        let mono = table(&[0, 0xffffff]);
        let c = |r, g, b| mono.get(Cell::of(r, g, b));
        assert_eq!([c(0, 0, 0), c(0, 0, 31), c(0, 31, 0), c(0, 31, 31), c(31, 0, 0), c(31, 0, 31), c(31, 31, 0), c(31, 31, 31)], [Some(0), Some(0), Some(0), None, Some(0), Some(0), Some(1), Some(1)]);
    }
}
