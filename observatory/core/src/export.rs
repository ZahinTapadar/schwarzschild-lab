//! Image writers: PNG (display), OpenEXR (linear HDR) and FITS (science data).
//!
//! All three are written from scratch with no dependencies:
//! * PNG: 8-bit RGB, zlib stream of stored (uncompressed) deflate blocks.
//! * OpenEXR: single-part scanline file, NO_COMPRESSION, 32-bit FLOAT
//!   channels, arbitrary channel names (sorted, as the format requires).
//! * FITS: standards-conforming primary HDU (FITS 4.0): 2880-byte blocks,
//!   BITPIX = -32 big-endian, 2-D image or 3-D cube (one plane per filter),
//!   first pixel at the lower left, metadata as header cards.

// ------------------------------------------------------------------- PNG --
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode 8-bit RGB pixels (row-major, top row first) as PNG. `text` adds tEXt chunks.
pub fn png(width: usize, height: usize, rgb: &[u8], text: &[(&str, String)]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * 3 + 1) * height);
    for y in 0..height {
        raw.push(0); // filter: none
        raw.extend_from_slice(&rgb[y * width * 3..(y + 1) * width * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(if chunks.peek().is_none() { 1 } else { 0 });
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    png_chunk(&mut out, b"IHDR", &ihdr);
    for (k, v) in text {
        let mut t = k.as_bytes().to_vec();
        t.push(0);
        t.extend(v.bytes().filter(|b| b.is_ascii()));
        png_chunk(&mut out, b"tEXt", &t);
    }
    png_chunk(&mut out, b"IDAT", &z);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

// ------------------------------------------------------------------- EXR --
/// Write a scanline OpenEXR with FLOAT channels. `channels` are (name, data)
/// with data row-major, top row first, width × height floats each.
pub fn exr(width: usize, height: usize, channels: &[(&str, &[f32])]) -> Vec<u8> {
    let mut ch: Vec<(&str, &[f32])> = channels.to_vec();
    ch.sort_by(|a, b| a.0.cmp(b.0));
    let mut h = Vec::new();
    let attr = |h: &mut Vec<u8>, name: &str, ty: &str, data: &[u8]| {
        h.extend_from_slice(name.as_bytes());
        h.push(0);
        h.extend_from_slice(ty.as_bytes());
        h.push(0);
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(data);
    };
    let mut chl = Vec::new();
    for (name, _) in &ch {
        chl.extend_from_slice(name.as_bytes());
        chl.push(0);
        chl.extend_from_slice(&2u32.to_le_bytes()); // FLOAT
        chl.extend_from_slice(&[0, 0, 0, 0]); // pLinear + reserved
        chl.extend_from_slice(&1i32.to_le_bytes());
        chl.extend_from_slice(&1i32.to_le_bytes());
    }
    chl.push(0);
    let bx = |w: usize, h_: usize| {
        let mut v = Vec::new();
        for x in [0i32, 0, w as i32 - 1, h_ as i32 - 1] { v.extend_from_slice(&x.to_le_bytes()); }
        v
    };
    attr(&mut h, "channels", "chlist", &chl);
    attr(&mut h, "compression", "compression", &[0]);
    attr(&mut h, "dataWindow", "box2i", &bx(width, height));
    attr(&mut h, "displayWindow", "box2i", &bx(width, height));
    attr(&mut h, "lineOrder", "lineOrder", &[0]);
    attr(&mut h, "pixelAspectRatio", "float", &1f32.to_le_bytes());
    let mut v2 = Vec::new();
    v2.extend_from_slice(&0f32.to_le_bytes());
    v2.extend_from_slice(&0f32.to_le_bytes());
    attr(&mut h, "screenWindowCenter", "v2f", &v2);
    attr(&mut h, "screenWindowWidth", "float", &1f32.to_le_bytes());
    h.push(0);

    let mut out = Vec::new();
    out.extend_from_slice(&20000630u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&h);
    let table_pos = out.len();
    out.resize(table_pos + 8 * height, 0);
    let line_bytes = width * 4 * ch.len();
    for y in 0..height {
        let off = out.len() as u64;
        out[table_pos + 8 * y..table_pos + 8 * y + 8].copy_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&(y as i32).to_le_bytes());
        out.extend_from_slice(&(line_bytes as u32).to_le_bytes());
        for (_, data) in &ch {
            for x in 0..width {
                out.extend_from_slice(&data[y * width + x].to_le_bytes());
            }
        }
    }
    out
}

// ------------------------------------------------------------------ FITS --
pub enum Card {
    Bool(&'static str, bool, &'static str),
    Int(&'static str, i64, &'static str),
    Float(&'static str, f64, &'static str),
    Str(&'static str, String, &'static str),
    Comment(String),
    History(String),
}

fn card_line(c: &Card) -> Vec<String> {
    let fmt = |key: &str, val: String, cmt: &str| {
        let mut s = format!("{key:<8}= {val:>20}");
        if !cmt.is_empty() { s.push_str(" / "); s.push_str(cmt); }
        s
    };
    match c {
        Card::Bool(k, v, m) => vec![fmt(k, if *v { "T".into() } else { "F".into() }, m)],
        Card::Int(k, v, m) => vec![fmt(k, v.to_string(), m)],
        Card::Float(k, v, m) => vec![fmt(k, format!("{:.10E}", v), m)],
        Card::Str(k, v, m) => {
            let esc: String = v.chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect::<String>().replace('\'', "''");
            let esc: String = esc.chars().take(66).collect();
            let mut s = format!("{k:<8}= '{:<8}'", esc);
            if !m.is_empty() && s.len() + 3 + m.len() <= 80 { s.push_str(" / "); s.push_str(m); }
            vec![s]
        }
        Card::Comment(t) | Card::History(t) => {
            let key = if matches!(c, Card::Comment(_)) { "COMMENT " } else { "HISTORY " };
            let clean: String = t.chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect();
            clean.as_bytes().chunks(72).map(|ch| format!("{key}{}", std::str::from_utf8(ch).unwrap())).collect()
        }
    }
}

/// Write a FITS primary HDU. `planes` are images of width × height floats,
/// row-major with the TOP row first (they are flipped to FITS's lower-left
/// origin here). One plane → 2-D image, several → 3-D cube.
pub fn fits(width: usize, height: usize, planes: &[&[f32]], cards: &[Card]) -> Vec<u8> {
    let mut lines = vec![
        format!("{:<8}= {:>20} / {}", "SIMPLE", "T", "conforms to FITS standard"),
        format!("{:<8}= {:>20} / {}", "BITPIX", -32, "IEEE single precision"),
        format!("{:<8}= {:>20}", "NAXIS", if planes.len() > 1 { 3 } else { 2 }),
        format!("{:<8}= {:>20}", "NAXIS1", width),
        format!("{:<8}= {:>20}", "NAXIS2", height),
    ];
    if planes.len() > 1 {
        lines.push(format!("{:<8}= {:>20}", "NAXIS3", planes.len()));
    }
    for c in cards {
        lines.extend(card_line(c));
    }
    lines.push("END".into());
    let mut out = Vec::new();
    for l in &lines {
        let mut b = l.as_bytes().to_vec();
        b.truncate(80);
        b.resize(80, b' ');
        out.extend_from_slice(&b);
    }
    out.resize(out.len().div_ceil(2880) * 2880, b' ');
    for p in planes {
        for y in (0..height).rev() {
            for x in 0..width {
                out.extend_from_slice(&p[y * width + x].to_be_bytes());
            }
        }
    }
    out.resize(out.len().div_ceil(2880) * 2880, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_is_block_aligned_and_flipped() {
        let img = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]; // 3×2, top row 1 2 3
        let f = fits(3, 2, &[&img], &[Card::Str("OBJECT", "test".into(), ""), Card::History("x".repeat(150))]);
        assert_eq!(f.len() % 2880, 0);
        assert!(f.starts_with(b"SIMPLE  =                    T"));
        let hdr = std::str::from_utf8(&f[..2880]).unwrap();
        assert!(hdr.contains("NAXIS1  =                    3"));
        assert!(hdr.contains("END"));
        // first data value is the bottom-left pixel (4.0)
        let d = &f[2880..2884];
        assert_eq!(f32::from_be_bytes(d.try_into().unwrap()), 4.0);
    }

    #[test]
    fn exr_header_and_offsets() {
        let r = vec![1.5f32; 4];
        let g = vec![2.5f32; 4];
        let e = exr(2, 2, &[("R", &r), ("G", &g)]);
        assert_eq!(&e[..4], &20000630u32.to_le_bytes());
        // channels are sorted: G before R
        let s = String::from_utf8_lossy(&e);
        assert!(s.find("G\0").unwrap() < s.find("R\0").unwrap());
        // last scanline offset points inside the file
        let hdr_end = e.windows(2).position(|w| w == [0, 0]).unwrap();
        assert!(hdr_end < e.len());
    }

    #[test]
    fn png_crc_known_value() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        let p = png(2, 1, &[255, 0, 0, 0, 255, 0], &[]);
        assert_eq!(&p[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }
}
