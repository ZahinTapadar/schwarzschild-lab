//! Sonification: every survey image has an audio twin.
//!
//! A sonification is a *data-to-sound mapping*, not a recording: space is
//! silent at these wavelengths, and these scenes are synthetic. The mapping
//! follows the conventions of NASA's Chandra/Hubble/Webb sonifications so
//! that a listener who knows those knows these:
//!
//! * **Scan.** A line sweeps the image left to right (`Sweep`, as for the
//!   Pillars of Creation, the Bullet Cluster and the Crab), or a radar arm
//!   turns clockwise from 12 o'clock around the centre (`Radar`, as for the
//!   Cat's Eye or NGC 2392). Time is position.
//! * **Pitch.** Sweep: height in the image (top is higher). Radar: distance
//!   from the centre (farther is higher). Diffuse light uses a continuous
//!   log-frequency scale; compact sources play notes of a major pentatonic
//!   scale.
//! * **Volume.** Brightness after the same stretch the picture uses, gated
//!   just above the background so detector noise does not hiss.
//! * **Stereo.** Light on the left is heard on the left (RS Puppis), so a
//!   sweep pans from left to right and a radar arm pans with its angle.
//! * **Layers.** Each instrument plays in its own register and timbre, from
//!   radio and mass (lowest, a sine hum) through infrared and optical
//!   (woodwind- and string-like) to X-rays (highest, brassy), the order NASA
//!   uses for M87, the Crab and the Bullet Cluster. Within one instrument the
//!   bands split its register in chromatic order: bluer is higher.
//! * **Compact sources** (stars, galaxies, point X-ray sources) are plucked or
//!   bell-like notes when the scan reaches them; diffuse light is a sustained
//!   drone, as in the Westerlund 2 and Milky Way Centre pieces.

use super::bands::Kind;
use super::obs::Observation;
use super::show::{asinh01, percentile, Stretch};
use crate::json::Json;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scan { Sweep, Radar }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Timbre {
    /// Pure tone: radio, lensing mass (a hum).
    Hum,
    /// Soft pad with gentle overtones: sub-millimetre, CMB.
    Pad,
    /// Odd harmonics: infrared.
    Woodwind,
    /// Sawtooth-like strings: optical.
    Strings,
    /// Rich, harsh spectrum: X-rays.
    Brass,
}

impl Timbre {
    fn name(self) -> &'static str {
        match self { Timbre::Hum => "hum (sine)", Timbre::Pad => "pad", Timbre::Woodwind => "woodwind-like (odd harmonics)", Timbre::Strings => "strings-like (sawtooth)", Timbre::Brass => "brass-like (bright)" }
    }
    /// Harmonic amplitudes (fundamental first).
    fn harmonics(self) -> Vec<f64> {
        match self {
            Timbre::Hum => vec![1.0],
            Timbre::Pad => vec![1.0, 0.35, 0.15, 0.05],
            Timbre::Woodwind => (1..=9).map(|n| if n % 2 == 1 { 1.0 / (n * n) as f64 } else { 0.0 }).collect(),
            Timbre::Strings => (1..=10).map(|n| 0.8 / n as f64).collect(),
            Timbre::Brass => (1..=12).map(|n| (n as f64).powf(-0.75)).collect(),
        }
    }
}

/// One layer of a sonification.
pub struct Voice<'a> {
    pub data: &'a [f32],
    pub w: usize,
    pub h: usize,
    pub label: String,
    pub lo_hz: f64,
    pub hi_hz: f64,
    pub timbre: Timbre,
    pub stretch: Stretch,
    pub log: bool,
    pub gain: f64,
    /// Play compact sources as notes.
    pub notes: bool,
    /// Noise rms per pixel in the data's units (0 = unknown); compact
    /// sources must stand 5σ above the background in a 3 × 3 box.
    pub sigma: f64,
    /// Data are photon counts: also require ≥ 6 counts in the 3 × 3 box.
    pub counts: bool,
}

pub struct Sound {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub rate: u32,
    pub meta: Json,
}

const TABLE: usize = 4096;
const BINS: usize = 48;
const PENTA: [f64; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];

fn wavetable(t: Timbre) -> Vec<f32> {
    let hs = t.harmonics();
    let mut tab: Vec<f64> = (0..TABLE).map(|i| {
        let ph = 2.0 * PI * i as f64 / TABLE as f64;
        hs.iter().enumerate().map(|(n, a)| a * ((n + 1) as f64 * ph).sin()).sum()
    }).collect();
    let m = tab.iter().fold(0.0f64, |a, &v| a.max(v.abs())).max(1e-9);
    for v in tab.iter_mut() { *v /= m; }
    tab.into_iter().map(|v| v as f32).collect()
}

/// Brightness → [0, 1] loudness with the picture's stretch, then a gate just
/// above the background (the stretched median plus a margin).
fn loudness(v: &Voice) -> Vec<f32> {
    let lo = percentile(v.data, v.stretch.lo_pct.max(1.0));
    let hi = percentile(v.data, v.stretch.hi_pct).max(lo + 1e-30);
    let f = |x: f64| -> f64 {
        if v.log { ((x.max(0.0) + 1.0).ln() - (lo.max(0.0) + 1.0).ln()) / ((hi + 1.0).ln() - (lo.max(0.0) + 1.0).ln()) } else { asinh01(x, lo, hi, v.stretch.soft) }
    };
    let s: Vec<f64> = v.data.iter().map(|&x| f(x as f64).clamp(0.0, 1.0)).collect();
    let mut sorted: Vec<f32> = s.iter().map(|&x| x as f32).collect();
    let k = sorted.len() / 2;
    sorted.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).unwrap());
    let gate = (sorted[k] as f64 + 0.08).min(0.6);
    s.iter().map(|&t| (((t - gate) / (1.0 - gate)).max(0.0)).powf(1.4) as f32).collect()
}

/// Map an image point to (time 0–1, pitch position 0–1, pan −1–1).
fn place(scan: Scan, x: f64, y: f64, w: usize, h: usize) -> (f64, f64, f64) {
    match scan {
        Scan::Sweep => (x / w as f64, 1.0 - y / h as f64, 2.0 * x / w as f64 - 1.0),
        Scan::Radar => {
            let (cx, cy) = (0.5 * w as f64, 0.5 * h as f64);
            let (dx, dy) = (x - cx, cy - y);
            let ang = dx.atan2(dy).rem_euclid(2.0 * PI); // clockwise from 12 o'clock
            let rmax = (cx * cx + cy * cy).sqrt();
            (ang / (2.0 * PI), (dx * dx + dy * dy).sqrt() / rmax, ang.sin())
        }
    }
}

/// Envelopes of the diffuse drone: amplitude per pitch bin per frame, and the pan per frame.
fn envelopes(v: &Voice, loud: &[f32], scan: Scan, frames: usize) -> (Vec<Vec<f32>>, Vec<f32>) {
    let (w, h) = (v.w, v.h);
    let mut env = vec![vec![0f32; frames]; BINS];
    let mut pan = vec![0f32; frames];
    match scan {
        Scan::Sweep => {
            for f in 0..frames {
                let x0 = f * w / frames;
                let x1 = ((f + 1) * w / frames).max(x0 + 1).min(w);
                pan[f] = (2.0 * (x0 + x1) as f64 / 2.0 / w as f64 - 1.0) as f32;
                for b in 0..BINS {
                    // bin 0 is the bottom of the image (lowest pitch)
                    let y1 = h - b * h / BINS;
                    let y0 = h - (b + 1) * h / BINS;
                    let mut s = 0.0;
                    let mut c = 0usize;
                    for y in y0..y1 { for x in x0..x1 { s += loud[y * w + x] as f64; c += 1; } }
                    env[b][f] = (s / c.max(1) as f64) as f32;
                }
            }
        }
        Scan::Radar => {
            let (cx, cy) = (0.5 * w as f64, 0.5 * h as f64);
            let rmax = (cx * cx + cy * cy).sqrt();
            for f in 0..frames {
                let ang = 2.0 * PI * (f as f64 + 0.5) / frames as f64;
                pan[f] = ang.sin() as f32;
                for b in 0..BINS {
                    let mut s = 0.0;
                    let mut c = 0usize;
                    for sub in 0..6 {
                        let r = rmax * (b as f64 + (sub as f64 + 0.5) / 6.0) / BINS as f64;
                        for da in [-0.5, 0.0, 0.5] {
                            let a = ang + da * 2.0 * PI / frames as f64;
                            let (x, y) = (cx + r * a.sin(), cy - r * a.cos());
                            if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 { continue; }
                            s += loud[y as usize * w + x as usize] as f64;
                            c += 1;
                        }
                    }
                    env[b][f] = if c > 0 { (s / c as f64) as f32 } else { 0.0 };
                }
            }
        }
    }
    (env, pan)
}

/// Compact sources: local maxima in a 5 × 5 box that stand out from their
/// 9 × 9 surroundings; at most `max`, brightest first.
fn compact_sources(voice: &Voice, loud: &[f32], max: usize) -> Vec<(usize, usize, f32)> {
    let (w, h) = (voice.w, voice.h);
    let bkg = percentile(voice.data, 50.0);
    let box3 = |x: usize, y: usize| -> f64 { let mut s = 0.0; for dy in 0..3 { for dx in 0..3 { s += voice.data[(y + dy - 1) * w + x + dx - 1] as f64; } } s };
    let mut found = Vec::new();
    for y in 4..h.saturating_sub(4) {
        for x in 4..w.saturating_sub(4) {
            let v = loud[y * w + x];
            if v < 0.35 { continue; }
            let mut is_max = true;
            let mut ring = 0.0f32;
            let mut nr = 0;
            for dy in -4i64..=4 { for dx in -4i64..=4 {
                let u = loud[((y as i64 + dy) as usize) * w + (x as i64 + dx) as usize];
                if dx.abs() <= 2 && dy.abs() <= 2 && (dx, dy) != (0, 0) && u >= v { is_max = false; }
                if dx.abs() == 4 || dy.abs() == 4 { ring += u; nr += 1; }
            }}
            if !(is_max && v > 2.0 * ring / nr as f32 + 0.15) { continue; }
            let s3 = box3(x, y);
            // local background: the mean of the raw data on the 9 × 9 ring
            let mut rb = 0.0;
            for d in -4i64..=4 {
                for (xx, yy) in [(x as i64 + d, y as i64 - 4), (x as i64 + d, y as i64 + 4), (x as i64 - 4, y as i64 + d), (x as i64 + 4, y as i64 + d)] {
                    rb += voice.data[yy as usize * w + xx as usize] as f64;
                }
            }
            let bkg = (rb / 36.0).max(if voice.counts { 0.0 } else { bkg.min(rb / 36.0) });
            let excess = s3 - 9.0 * bkg;
            let sig_ok = if voice.sigma > 0.0 { excess > 5.0 * 3.0 * voice.sigma } else { true };
            let counts_ok = !voice.counts || (s3 >= 6.0 && excess > 5.0 * (9.0 * bkg + 1.0).sqrt());
            if sig_ok && counts_ok { found.push((x, y, v)); }
        }
    }
    found.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    found.truncate(max);
    found
}

#[inline]
fn pan_gains(p: f64) -> (f64, f64) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos(), a.sin())
}

/// Render the voices to stereo audio of `seconds` at `rate` Hz.
pub fn sonify(voices: &[Voice], scan: Scan, seconds: f64, rate: u32, title: &str) -> Sound {
    let n = (seconds * rate as f64) as usize;
    let mut left = vec![0f64; n];
    let mut right = vec![0f64; n];
    let mut layers = Vec::new();
    for v in voices {
        let loud = loudness(v);
        let frames = match scan { Scan::Sweep => v.w.min(512), Scan::Radar => 720 };
        let (env, pan) = envelopes(v, &loud, scan, frames);
        let tab = wavetable(v.timbre);
        let mut vl = vec![0f64; n];
        let mut vr = vec![0f64; n];
        // drone: one oscillator per pitch bin, phase-continuous, amplitude
        // interpolated linearly between frames
        let spf = n as f64 / frames as f64;
        for b in 0..BINS {
            if env[b].iter().all(|&a| a == 0.0) { continue; }
            let f = v.lo_hz * (v.hi_hz / v.lo_hz).powf(b as f64 / (BINS - 1) as f64);
            let inc = f * TABLE as f64 / rate as f64;
            // two slightly detuned copies give the drone some width
            for (det, side) in [(0.9983, -0.35), (1.0017, 0.35)] {
                let mut ph = (b as f64 * 977.0) % TABLE as f64;
                let step = inc * det;
                // frame by frame: amplitude ramps linearly from this frame's
                // value to the next; silent stretches only advance the phase
                for f in 0..frames {
                    let i0 = ((f as f64 + 0.5) * spf) as usize;
                    let i1 = (((f as f64 + 1.5) * spf) as usize).min(n);
                    let i0 = if f == 0 { 0 } else { i0.min(n) };
                    let i1 = if f + 1 == frames { n } else { i1 };
                    if i1 <= i0 { continue; }
                    let (a0, a1) = (env[b][f] as f64, env[b][(f + 1).min(frames - 1)] as f64);
                    if a0 == 0.0 && a1 == 0.0 {
                        ph = (ph + step * (i1 - i0) as f64) % TABLE as f64;
                        continue;
                    }
                    let p = pan[f] as f64 * 0.85 + side * 0.15;
                    let (gl, gr) = pan_gains(p);
                    let da = (a1 - a0) / (i1 - i0) as f64;
                    let mut a = a0;
                    for i in i0..i1 {
                        ph += step;
                        if ph >= TABLE as f64 { ph -= TABLE as f64; }
                        let s = tab[ph as usize] as f64 * a;
                        vl[i] += s * gl;
                        vr[i] += s * gr;
                        a += da;
                    }
                }
            }
        }
        // notes for compact sources
        let mut notes = 0usize;
        if v.notes {
            let src = compact_sources(v, &loud, 150);
            let octaves = (v.hi_hz / v.lo_hz).log2();
            for &(x, y, a) in &src {
                let (t, pos, p) = place(scan, x as f64, y as f64, v.w, v.h);
                // pentatonic note in the voice's register (an octave above the drone's base)
                let semis = pos * octaves * 12.0;
                let oct = (semis / 12.0).floor();
                let within = semis - 12.0 * oct;
                let step = PENTA.iter().copied().fold(0.0, |best, s| if (s - within).abs() < (best - within).abs() { s } else { best });
                let freq = v.lo_hz * 2f64.powf((12.0 * oct + step + 12.0) / 12.0);
                let start = (t * n as f64) as usize;
                let dur = (1.2 * rate as f64) as usize;
                let (gl, gr) = pan_gains(p);
                for k in 0..dur.min(n.saturating_sub(start)) {
                    let tt = k as f64 / rate as f64;
                    let envn = (tt / 0.004).min(1.0) * (-tt / 0.35).exp();
                    // bell-like partials (inharmonic), as chimes for stars
                    let s = (2.0 * PI * freq * tt).sin() + 0.4 * (2.0 * PI * freq * 2.76 * tt).sin() * (-tt / 0.15).exp() + 0.2 * (2.0 * PI * freq * 5.4 * tt).sin() * (-tt / 0.08).exp();
                    let val = 0.5 * a as f64 * envn * s;
                    vl[start + k] += val * gl;
                    vr[start + k] += val * gr;
                }
                notes += 1;
            }
        }
        // each voice normalised to the same RMS, then weighted by its gain
        let rms = ((vl.iter().chain(vr.iter()).map(|s| s * s).sum::<f64>()) / (2 * n) as f64).sqrt().max(1e-12);
        for i in 0..n { left[i] += vl[i] / rms * v.gain; right[i] += vr[i] / rms * v.gain; }
        let mut lj = Json::obj();
        lj.set("layer", v.label.as_str()).set("register_hz", vec![v.lo_hz.into(), v.hi_hz.into()]).set("timbre", v.timbre.name())
            .set("notes", notes).set("gain", v.gain).set("brightness", if v.log { "log(1 + counts)" } else { "asinh stretch of the picture" });
        layers.push(lj);
    }
    // fades and a master gain to −1 dBFS
    let fade = (0.05 * rate as f64) as usize;
    for i in 0..fade.min(n) {
        let g = i as f64 / fade as f64;
        left[i] *= g; right[i] *= g;
        left[n - 1 - i] *= g; right[n - 1 - i] *= g;
    }
    let peak = left.iter().chain(right.iter()).fold(0.0f64, |a, &s| a.max(s.abs())).max(1e-12);
    let master = 0.891 / peak;
    let mut meta = Json::obj();
    meta.set("title", title).set("kind", "sonification").set("statement", "A data-to-sound mapping of a synthetic observation. These are not sounds from space.")
        .set("scan", match scan { Scan::Sweep => "left to right; pitch rises with height in the image", Scan::Radar => "clockwise from 12 o'clock around the centre; pitch rises with distance from the centre" })
        .set("volume", "brightness after the picture's stretch, gated above the background")
        .set("stereo", "light on the left is heard on the left")
        .set("compact_sources", "pentatonic bell notes when the scan reaches them")
        .set("seconds", seconds).set("rate", rate).set("layers", layers);
    Sound { left: left.iter().map(|&s| (s * master) as f32).collect(), right: right.iter().map(|&s| (s * master) as f32).collect(), rate, meta }
}

/// 16-bit PCM stereo WAV.
pub fn wav(s: &Sound) -> Vec<u8> {
    let n = s.left.len();
    let data = n * 4;
    let mut b = Vec::with_capacity(44 + data);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&((36 + data) as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&s.rate.to_le_bytes());
    b.extend_from_slice(&(s.rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data as u32).to_le_bytes());
    for i in 0..n {
        for v in [s.left[i], s.right[i]] {
            b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
    }
    b
}

/// Register and timbre of an instrument, lowest (radio, mass) to highest (X-ray).
pub fn register(kind: Kind, lambda_um: f64) -> (f64, f64, Timbre) {
    match kind {
        Kind::Lensing => (41.2, 164.8, Timbre::Hum),
        Kind::Radio if lambda_um > 1e4 => (55.0, 220.0, Timbre::Hum),
        Kind::Radio | Kind::Cmb => (82.4, 329.6, Timbre::Pad),
        Kind::Xray => (440.0, 1760.0, Timbre::Brass),
        Kind::Imager if lambda_um > 5.0 => (110.0, 440.0, Timbre::Woodwind),
        Kind::Imager if lambda_um > 1.0 => (146.8, 587.3, Timbre::Woodwind),
        Kind::Imager => (220.0, 880.0, Timbre::Strings),
    }
}

/// The audio twin of one instrument's images: its bands as voices that split
/// the instrument's register in chromatic order (longest wavelength lowest).
pub fn instrument_sound(obs: &Observation, st: Stretch, scan: Scan, seconds: f64, rate: u32) -> Sound {
    let nb = obs.maps.len();
    let mut idx: Vec<usize> = (0..nb).collect();
    // longest wavelength first (lowest), shortest last (highest); CMB bands by frequency
    idx.sort_by(|&a, &b| obs.maps[b].lambda_um.partial_cmp(&obs.maps[a].lambda_um).unwrap());
    let lam = obs.maps.iter().map(|m| m.lambda_um).fold(0.0, f64::max);
    let (lo, hi, timbre) = register(obs.ins.kind, lam);
    let span = (hi / lo).log2();
    let signed = matches!(obs.ins.kind, Kind::Cmb | Kind::Lensing);
    let flipped: Vec<Vec<f32>> = obs.maps.iter().map(|m| if signed { m.data.iter().map(|&v| -v).collect() } else { vec![] }).collect();
    let voices: Vec<Voice> = idx.iter().enumerate().map(|(k, &b)| {
        // each band gets an overlapping slice of the register, a third of an octave apart
        let off = if nb > 1 { span * 0.5 * k as f64 / (nb - 1) as f64 } else { 0.0 };
        let sub_lo = lo * 2f64.powf(off);
        let sub_hi = sub_lo * 2f64.powf(span * 0.5 + if nb > 1 { 0.0 } else { span * 0.5 });
        let data: &[f32] = if signed { &flipped[b] } else { &obs.maps[b].data };
        let mut s = st;
        if signed { s.lo_pct = 50.0; }
        Voice {
            data, w: obs.w, h: obs.h, label: format!("{} {}", obs.ins.name, obs.maps[b].name), lo_hz: sub_lo, hi_hz: sub_hi,
            timbre, stretch: s, log: obs.ins.kind == Kind::Xray, gain: 1.0, notes: !signed,
            sigma: obs.maps[b].sigma, counts: obs.ins.kind == Kind::Xray,
        }
    }).collect();
    let what = if signed { " (sign flipped: an SZ decrement or a mass peak is loud)" } else { "" };
    sonify(&voices, scan, seconds, rate, &format!("{}{}", obs.ins.name, what))
}

/// The audio twin of a multiwavelength montage: one voice per instrument (its
/// bands averaged after stretching), each in its own register and timbre.
pub fn survey_sound(obs: &[Observation], stretch: &dyn Fn(&str) -> Stretch, scan: Scan, seconds: f64, rate: u32, title: &str) -> Sound {
    let mut mixes: Vec<(Vec<f32>, usize)> = Vec::new();
    for (k, o) in obs.iter().enumerate() {
        let st = stretch(&o.ins.id);
        let signed = matches!(o.ins.kind, Kind::Cmb | Kind::Lensing);
        // CMB: the middle band only; others: the mean of per-band stretched maps
        let picks: Vec<usize> = if o.ins.kind == Kind::Cmb { vec![o.maps.len().min(2) - 1] } else { (0..o.maps.len()).collect() };
        let mut acc = vec![0f32; o.w * o.h];
        for &b in &picks {
            let d: Vec<f32> = o.maps[b].data.iter().map(|&v| if signed { -v } else { v }).collect();
            let lo = percentile(&d, if signed { 50.0 } else { st.lo_pct });
            let hi = percentile(&d, st.hi_pct).max(lo + 1e-30);
            for (a, &v) in acc.iter_mut().zip(&d) {
                let t = if o.ins.kind == Kind::Xray { (((v as f64).max(0.0) + 1.0).ln() - (lo.max(0.0) + 1.0).ln()) / ((hi + 1.0).ln() - (lo.max(0.0) + 1.0).ln()) } else { asinh01(v as f64, lo, hi, st.soft) };
                *a += t.clamp(0.0, 1.0) as f32 / picks.len() as f32;
            }
        }
        mixes.push((acc, k));
    }
    let voices: Vec<Voice> = mixes.iter().map(|(d, k)| {
        let o = &obs[*k];
        let lam = o.maps.iter().map(|m| m.lambda_um).fold(0.0, f64::max);
        let (lo, hi, timbre) = register(o.ins.kind, lam);
        // already in [0, 1]: a linear stretch with the default gate
        Voice { data: d, w: o.w, h: o.h, label: o.ins.name.clone(), lo_hz: lo, hi_hz: hi, timbre, stretch: Stretch { lo_pct: 0.0, hi_pct: 100.0, soft: 10.0, smooth: 0.0, shared: false }, log: false, gain: 1.0, notes: !matches!(o.ins.kind, Kind::Cmb | Kind::Lensing | Kind::Xray), sigma: 0.0, counts: false }
    }).collect();
    sonify(&voices, scan, seconds, rate, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        (0..w * h).map(|i| f(i % w, i / w)).collect()
    }

    fn voice<'a>(d: &'a [f32], w: usize, h: usize) -> Voice<'a> {
        Voice { data: d, w, h, label: "test".into(), lo_hz: 220.0, hi_hz: 880.0, timbre: Timbre::Hum, stretch: Stretch { lo_pct: 1.0, hi_pct: 99.9, soft: 0.5, smooth: 0.0, shared: false }, log: false, gain: 1.0, notes: false, sigma: 0.0, counts: false }
    }

    fn band_energy(s: &[f32], rate: u32, t0: f64, t1: f64) -> f64 {
        let (a, b) = ((t0 * rate as f64) as usize, (t1 * rate as f64) as usize);
        s[a..b].iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / (b - a) as f64
    }

    #[test]
    fn a_bright_column_sounds_when_the_sweep_reaches_it() {
        let (w, h) = (100, 40);
        let d = img(w, h, |x, _| if (60..64).contains(&x) { 1.0 } else { 0.0 });
        let s = sonify(&[voice(&d, w, h)], Scan::Sweep, 2.0, 8000, "t");
        let on = band_energy(&s.right, 8000, 1.22, 1.26);
        let off = band_energy(&s.right, 8000, 0.4, 0.8);
        assert!(on > 1e-3 && off < on * 1e-6, "on {on:e} off {off:e}");
        // and it is panned right of centre
        assert!(band_energy(&s.right, 8000, 1.22, 1.26) > band_energy(&s.left, 8000, 1.22, 1.26));
    }

    #[test]
    fn higher_in_the_image_is_higher_in_pitch() {
        let (w, h) = (64, 48);
        let pitch = |row: usize| {
            let d = img(w, h, |_, y| if y == row { 1.0 } else { 0.0 });
            let s = sonify(&[voice(&d, w, h)], Scan::Sweep, 1.0, 16000, "t");
            // zero crossings per second ≈ 2 f
            let zc = s.left.windows(2).skip(2000).take(12000).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count();
            zc as f64 / (12000.0 / 16000.0) / 2.0
        };
        let (top, bottom) = (pitch(2), pitch(44));
        assert!(top > 1.8 * bottom, "top {top} Hz, bottom {bottom} Hz");
        assert!(bottom > 200.0 && top < 900.0);
    }

    #[test]
    fn brighter_is_louder_and_nothing_clips() {
        let (w, h) = (80, 40);
        let d = img(w, h, |x, _| if x < 40 { 0.25 } else { 1.0 });
        let s = sonify(&[voice(&d, w, h)], Scan::Sweep, 2.0, 8000, "t");
        let (dim, bright) = (band_energy(&s.left, 8000, 0.3, 0.7) + band_energy(&s.right, 8000, 0.3, 0.7), band_energy(&s.left, 8000, 1.3, 1.7) + band_energy(&s.right, 8000, 1.3, 1.7));
        assert!(bright > 2.0 * dim, "{bright:e} vs {dim:e}");
        assert!(s.left.iter().chain(s.right.iter()).all(|v| v.abs() <= 0.9));
        let b = wav(&s);
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(b.len(), 44 + 4 * s.left.len());
    }

    #[test]
    fn a_point_source_becomes_one_note_at_its_time() {
        let (w, h) = (100, 50);
        let mut d = img(w, h, |_, _| 0.0);
        d[25 * w + 30] = 1.0;
        let mut v = voice(&d, w, h);
        v.notes = true;
        let s = sonify(&[v], Scan::Sweep, 2.0, 8000, "t");
        let notes = s.meta.arr("layers").unwrap()[0].num("notes").unwrap();
        assert_eq!(notes, 1.0);
        let before = band_energy(&s.left, 8000, 0.3, 0.55);
        let at = band_energy(&s.left, 8000, 0.6, 0.7);
        assert!(at > 100.0 * before.max(1e-12), "{at:e} {before:e}");
    }

    #[test]
    fn radar_is_clockwise_from_twelve() {
        let (w, h) = (100, 100);
        // a bright spot at 3 o'clock is heard a quarter of the way through, on the right
        let d = img(w, h, |x, y| if (80..86).contains(&x) && (47..53).contains(&y) { 1.0 } else { 0.0 });
        let s = sonify(&[voice(&d, w, h)], Scan::Radar, 4.0, 8000, "t");
        let at = band_energy(&s.right, 8000, 0.95, 1.05);
        let other = band_energy(&s.right, 8000, 2.5, 3.5);
        assert!(at > 1e-3 && other < at * 1e-4, "{at:e} {other:e}");
        assert!(band_energy(&s.right, 8000, 0.95, 1.05) > 3.0 * band_energy(&s.left, 8000, 0.95, 1.05));
    }
}
