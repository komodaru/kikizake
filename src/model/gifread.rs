//! How WIC's GIF decoder reads an image's data, row by row: its sub-block reader and its LZW
//! decoder, as they behave on any data, a file cut short or codes past the table included.
//!
//! FOUND clean-room (windowscodecs CGifCodec::GetLine, CLZWExpand<CLZWExpandGif>::Expand,
//! ::Create, CGifCodec::UpdateCurrentFrame) and confirmed by Windows (golden wicdecode gif-*,
//! cut-gif-*, cuta-gif-*): spec/wicimage.txt WICIMAGE.GIF.READ.
//!
//! - A row asks the LZW decoder for its pixels. When it holds fewer than 3 bytes of data (and
//!   the data has not ended), it first appends whole sub-blocks until it holds 2*(max(w,
//!   12000)/3) - 256 bytes or more. A size byte of 0, or none (the file ended), ends the data. A
//!   sub-block the file ends inside is not taken: the rest of that row is 0, and its size byte is
//!   gone, so the next refill takes the sub-block's first data byte as a size.
//! - The decoder stops when it lacks the bytes of a whole code, or at an end code (which also
//!   resets it, as a clear code); a code past the table is the next one (KwKwK), right after a
//!   clear code the code masked to the roots. A string the row has no room for is finished in the
//!   next row.
//! - A row whose decoder neither reads a byte nor writes a pixel: with 3 or more bytes held
//!   WINCODEC_ERR_BADIMAGE, else the rest of the row 0.

use super::wicimage::BAD_IMAGE;

/// the decoder's table: entries of (prefix << 20) | (length << 8) | last byte, one spare
/// slot past 4096 where the table stops growing
struct Lzw {
    min: u32,
    clear: u32,
    bits: u32,
    size: u32,
    threshold: u32,
    next: usize,
    pending: u32,
    tab: Vec<u32>,
}

fn byte(e: u32) -> u8 {
    (e & 0xff) as u8
}
fn prefix(e: u32) -> usize {
    (e >> 20) as usize
}

impl Lzw {
    /// Create: a minimum code size past 8 has no decoder; one below 2 is 2
    fn new(min: u8) -> Option<Lzw> {
        if min > 8 {
            return None;
        }
        let min = u32::from(min.max(2));
        let clear = 1u32 << min;
        let mut tab = vec![0u32; 4097];
        for (i, t) in tab.iter_mut().enumerate().take(clear as usize) {
            *t = i as u32;
        }
        Some(Lzw { min, clear, bits: 0, size: min + 1, threshold: 1 << (min + 1), next: clear as usize + 1, pending: 0, tab })
    }

    /// Expand: pixels into `out` from `input`; (bytes read, pixels written)
    fn expand(&mut self, out: &mut [u8], input: &[u8]) -> (usize, usize) {
        let avail = out.len();
        let mut room = avail;
        let mut o = 0usize;
        let mut inp = 0usize;
        if self.pending != 0 {
            // (the string the last row had no room for: its tail)
            let mut c = prefix(self.tab[self.next]);
            let mut r = self.pending as usize;
            let mut wrote = 0;
            while r != 0 {
                let e = self.tab[c];
                if r <= room {
                    out[r - 1] = byte(e);
                    wrote += 1;
                }
                c = prefix(e);
                r -= 1;
            }
            room -= wrote;
            o += wrote;
            self.pending -= wrote as u32;
        }
        while room != 0 {
            let left = input.len() - inp;
            let end = self.bits + self.size;
            let word = if left < 4 {
                let need = end.div_ceil(8) as usize;
                if left < need {
                    break;
                }
                input[inp..inp + need].iter().enumerate().fold(0u32, |a, (k, &v)| a | u32::from(v) << (8 * k))
            } else {
                u32::from_le_bytes([input[inp], input[inp + 1], input[inp + 2], input[inp + 3]])
            };
            let mut code = (word >> self.bits) & ((1 << self.size) - 1);
            self.bits = end & 7;
            inp += (end >> 3) as usize;
            if code & !1 == self.clear {
                self.next = self.clear as usize + 1;
                self.size = self.min + 1;
                self.threshold = 1 << self.size;
                if code != self.clear {
                    break;
                }
                continue;
            }
            if code as usize > self.next {
                code = if self.next == self.clear as usize + 1 { code & (self.clear - 1) } else { self.next as u32 };
            }
            let e = self.tab[code as usize];
            let len = ((e >> 8) & 0xfff) as usize;
            let mut first = byte(e);
            if len < room {
                let mut c = e;
                for k in (0..len).rev() {
                    c = self.tab[prefix(c)];
                    out[o + k] = byte(c);
                }
                if len != 0 {
                    first = byte(c);
                }
                self.tab[self.next] = (self.tab[self.next] & !0xff) | u32::from(first);
                out[o + len] = byte(self.tab[code as usize]);
                o += len + 1;
                room -= len + 1;
            } else {
                self.pending = (len - room + 1) as u32;
                let mut c = e;
                for r in (1..=len).rev() {
                    c = self.tab[prefix(c)];
                    if r <= room {
                        out[o + r - 1] = byte(c);
                    }
                }
                first = byte(c);
                self.tab[self.next] = (self.tab[self.next] & !0xff) | u32::from(first);
                o += room;
                room = 0;
            }
            let mut at = self.next;
            if at < 0x1000 {
                at += 1;
                self.next = at;
                if at as u32 == self.threshold && self.size < 12 {
                    self.threshold += 1 << self.size;
                    self.size += 1;
                }
            }
            self.tab[at] = ((len as u32 + 1) << 8) | (code << 20);
        }
        (inp, avail - room)
    }
}

/// one image's data as the decoder reads it: the file from after the LZW code size on
pub struct Rows<'a> {
    file: &'a [u8],
    pos: usize,
    ended: bool,
    held: Vec<u8>,
    fill: usize,
    lzw: Lzw,
}

impl<'a> Rows<'a> {
    /// None: no decoder for the code size (E_OUTOFMEMORY)
    pub fn new(data: &'a [u8], min: u8, w: u32) -> Option<Rows<'a>> {
        let cap = 2 * (w.max(12000) / 3) as usize;
        Some(Rows { file: data, pos: 0, ended: false, held: Vec::new(), fill: cap - 0x100, lzw: Lzw::new(min)? })
    }

    /// GetLine: the next row's pixels
    pub fn row(&mut self, row: &mut [u8]) -> Result<(), u32> {
        let mut o = 0;
        loop {
            if !self.ended && self.held.len() < 3 {
                while self.held.len() < self.fill {
                    let Some(&n) = self.file.get(self.pos) else {
                        self.ended = true;
                        break;
                    };
                    self.pos += 1;
                    if n == 0 {
                        self.ended = true;
                        break;
                    }
                    let Some(d) = self.file.get(self.pos..self.pos + usize::from(n)) else {
                        row[o..].fill(0);
                        return Ok(());
                    };
                    self.held.extend_from_slice(d);
                    self.pos += usize::from(n);
                }
            }
            if self.held.is_empty() {
                row[o..].fill(0);
                return Ok(());
            }
            let (read, wrote) = self.lzw.expand(&mut row[o..], &self.held);
            if read == 0 && wrote == 0 {
                if self.held.len() >= 3 {
                    return Err(BAD_IMAGE);
                }
                row[o..].fill(0);
                return Ok(());
            }
            self.held.drain(..read);
            o += wrote;
            if o == row.len() {
                return Ok(());
            }
        }
    }
}
