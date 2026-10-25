//! Legends for diagnostic images: a small built-in bitmap font and a legend
//! band appended below the image, so every diagnostic PNG says what it shows,
//! in which units and on which (fixed) scale.

/// 5 × 7 glyphs, one byte per row (low 5 bits, MSB on the left).
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        ' ' => [0; 7],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        ',' => [0, 0, 0, 0, 0x0C, 0x04, 0x08],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        '/' => [0, 0x01, 0x02, 0x04, 0x08, 0x10, 0],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        ';' => [0, 0x0C, 0x0C, 0, 0x0C, 0x04, 0x08],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04],
        '=' => [0, 0, 0x1F, 0, 0x1F, 0, 0],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '\'' => [0x0C, 0x04, 0x08, 0, 0, 0, 0],
        '"' => [0x0A, 0x0A, 0, 0, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0x1F],
        '^' => [0x04, 0x0A, 0x11, 0, 0, 0, 0],
        '*' => [0, 0x04, 0x15, 0x0E, 0x15, 0x04, 0],
        '|' => [0x04; 7],
        '~' => [0, 0, 0x08, 0x15, 0x02, 0, 0],
        _ => [0x0E, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
    }
}

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

impl Canvas {
    pub fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                let i = (yy * self.w + xx) * 4;
                self.rgba[i..i + 3].copy_from_slice(&c);
                self.rgba[i + 3] = 255;
            }
        }
    }
    /// Draw text; returns the x after the last glyph.
    pub fn text(&mut self, x: usize, y: usize, s: &str, c: [u8; 3], scale: usize) -> usize {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        self.rect(cx + col * scale, y + row * scale, scale, scale, c);
                    }
                }
            }
            cx += 6 * scale;
        }
        cx
    }
}

pub fn text_width(s: &str, scale: usize) -> usize { s.chars().count() * 6 * scale }

/// Greedy word wrap to at most `max` characters per line.
pub fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(w);
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

/// What a legend shows.
pub enum Key {
    /// Named categories with sRGB swatches.
    Categories(Vec<(String, [u8; 3])>),
    /// A colour ramp from `lo` to `hi` (sRGB stops), with tick labels.
    Ramp { stops: Vec<[u8; 3]>, ticks: Vec<(f64, String)> },
}

pub struct Legend {
    /// e.g. "DIAGNOSTIC: RAY FATE"
    pub title: String,
    pub key: Key,
    /// A short note (units, scale, approximation).
    pub note: String,
}

const BG: [u8; 3] = [11, 13, 16];
const FG: [u8; 3] = [216, 222, 230];
const DIM: [u8; 3] = [125, 136, 150];

/// Append a legend band below an RGBA image.
pub fn with_legend(rgba: &[u8], w: usize, h: usize, lg: &Legend) -> (Vec<u8>, usize, usize) {
    let s = if w >= 900 { 2 } else { 1 };
    let line = 11 * s;
    let pad = 6 * s;
    // lay the key out first to know the band height
    let mut items: Vec<(usize, usize)> = Vec::new(); // (x, row) per category
    let mut rows = 1;
    if let Key::Categories(cats) = &lg.key {
        let mut x = pad;
        for (name, _) in cats {
            let iw = 12 * s + text_width(name, s) + 10 * s;
            if x + iw > w - pad && x > pad { x = pad; rows += 1; }
            items.push((x, rows - 1));
            x += iw;
        }
    } else {
        rows = 2;
    }
    let maxc = ((w - 2 * pad) / (6 * s)).max(8);
    let title = wrap(&lg.title, maxc);
    let note = wrap(&lg.note, maxc);
    let band = pad + title.len() * line + 2 * s + rows * (line + 2 * s) + note.len() * line + pad;
    let mut c = Canvas { w, h: h + band, rgba: Vec::with_capacity(w * (h + band) * 4) };
    c.rgba.extend_from_slice(&rgba[..w * h * 4]);
    c.rgba.resize(w * (h + band) * 4, 255);
    c.rect(0, h, w, band, BG);
    for (k, t) in title.iter().enumerate() { c.text(pad, h + pad + k * line, t, FG, s); }
    let y0 = h + pad + title.len() * line + 2 * s;
    match &lg.key {
        Key::Categories(cats) => {
            for ((name, col), (x, row)) in cats.iter().zip(&items) {
                let y = y0 + row * (line + 2 * s);
                c.rect(*x, y, 9 * s, 7 * s, DIM);
                c.rect(x + s, y + s, 7 * s, 5 * s, *col);
                c.text(x + 12 * s, y, name, FG, s);
            }
        }
        Key::Ramp { stops, ticks } => {
            let (x0, x1) = (pad + 4 * s, w - pad - 4 * s);
            let bw = x1 - x0;
            for k in 0..bw {
                let t = k as f64 / (bw - 1).max(1) as f64;
                let f = t * (stops.len() - 1) as f64;
                let i = (f.floor() as usize).min(stops.len() - 2);
                let u = f - i as f64;
                let col = [0, 1, 2].map(|j| (stops[i][j] as f64 * (1.0 - u) + stops[i + 1][j] as f64 * u) as u8);
                c.rect(x0 + k, y0, 1, 7 * s, col);
            }
            for (t, label) in ticks {
                let x = x0 + (t.clamp(0.0, 1.0) * (bw - 1) as f64) as usize;
                c.rect(x, y0 + 7 * s, s, 2 * s, FG);
                let tw = text_width(label, s);
                let lx = x.saturating_sub(tw / 2).min(w - pad - tw).max(pad);
                c.text(lx, y0 + 10 * s, label, FG, s);
            }
        }
    }
    for (k, t) in note.iter().enumerate() { c.text(pad, h + band - pad - (note.len() - k) * line + 4 * s, t, DIM, s); }
    (c.rgba, w, h + band)
}

/// Sample an sRGB ramp at t ∈ [0, 1] (for building ramp stops from functions).
pub fn stops(f: &dyn Fn(f64) -> [f64; 3], n: usize) -> Vec<[u8; 3]> {
    (0..n).map(|i| { let c = f(i as f64 / (n - 1) as f64); [0, 1, 2].map(|k| (c[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8) }).collect()
}
