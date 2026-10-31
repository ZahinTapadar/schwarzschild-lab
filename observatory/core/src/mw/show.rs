//! Display of multiwavelength observations: single bands with their units,
//! false-colour composites, montages and generator diagnostics.
//!
//! Every curve here is a presentation choice and is written into the image's
//! legend: the percentiles, the asinh softening and, for composites, which band
//! went to which colour. Composites follow chromatic order (shortest
//! wavelength bluest), as most observatory releases do, and are always
//! labelled FALSE COLOUR.

use super::obs::Observation;
use super::bands::Kind;
use super::*;
use crate::annotate::{stops, with_legend, Canvas, Key, Legend};
use crate::sensor::{coolwarm, viridis};

#[derive(Clone, Copy)]
pub struct Stretch {
    pub lo_pct: f64,
    pub hi_pct: f64,
    pub soft: f64,
    /// Display-only Gaussian smoothing σ in pixels (stated in the legend).
    pub smooth: f64,
    /// Composites: one black and white point for every band, so band ratios survive.
    pub shared: bool,
}

impl Default for Stretch {
    fn default() -> Self { Stretch { lo_pct: 0.5, hi_pct: 99.8, soft: 0.05, smooth: 0.0, shared: false } }
}

fn smoothed(d: &[f32], w: usize, h: usize, s: f64) -> Vec<f32> {
    if s <= 0.0 { return d.to_vec(); }
    let v: Vec<f64> = d.iter().map(|&x| x as f64).collect();
    super::obs::convolve(&v, w, h, s).iter().map(|&x| x as f32).collect()
}

pub fn percentile(v: &[f32], p: f64) -> f64 {
    let mut s: Vec<f32> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() { return 0.0; }
    let k = ((p / 100.0) * (s.len() - 1) as f64).round() as usize;
    let k = k.min(s.len() - 1);
    s.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).unwrap());
    s[k] as f64
}

/// asinh stretch of v between black b and white w.
#[inline]
pub fn asinh01(v: f64, b: f64, w: f64, soft: f64) -> f64 {
    let x = (v - b) / (w - b).max(1e-300);
    ((x / soft).asinh() / (1.0 / soft).asinh()).clamp(0.0, 1.0)
}

fn inv_asinh01(t: f64, b: f64, w: f64, soft: f64) -> f64 { b + (w - b) * soft * (t * (1.0 / soft).asinh()).sinh() }

fn fmt_val(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 { "0".into() } else if a >= 1e4 || a < 1e-2 { format!("{v:.1e}") } else if a >= 100.0 { format!("{v:.0}") } else if a >= 1.0 { format!("{v:.1}") } else { format!("{v:.3}") }
}

fn to_rgba(rgb: &[[f64; 3]]) -> Vec<u8> {
    let mut o = Vec::with_capacity(rgb.len() * 4);
    for c in rgb { for k in 0..3 { o.push((c[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8); } o.push(255); }
    o
}

pub fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> { rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect() }

/// Does this band's map need a diverging scale (signed signal)?
fn signed(obs: &Observation) -> bool { matches!(obs.ins.kind, Kind::Cmb | Kind::Lensing) }

/// One band as a picture with its legend. Returns (rgba, w, h).
pub fn band_image(obs: &Observation, b: usize, st: Stretch) -> (Vec<u8>, usize, usize) {
    let m0 = &obs.maps[b];
    let (w, h) = (obs.w, obs.h);
    let unit = obs.ins.unit();
    let sm = smoothed(&m0.data, w, h, st.smooth);
    struct M<'a> { data: &'a [f32] }
    let m = M { data: &sm };
    let (rgb, ticks, ramp): (Vec<[f64; 3]>, Vec<(f64, String)>, Vec<[u8; 3]>) = if signed(obs) {
        let a = percentile(&m.data.iter().map(|v| v.abs()).collect::<Vec<_>>(), st.hi_pct).max(1e-30);
        let rgb = m.data.iter().map(|&v| coolwarm(0.5 + 0.5 * (v as f64 / a))).collect();
        (rgb, vec![(0.0, fmt_val(-a)), (0.5, "0".into()), (1.0, fmt_val(a))], stops(&coolwarm, 16))
    } else if obs.ins.kind == Kind::Xray {
        let hi = percentile(&m.data, st.hi_pct).max(1.0);
        let lo = percentile(&m.data, st.lo_pct).max(0.0);
        let f = |v: f64| (((v.max(0.0) + 1.0).ln() - (lo + 1.0).ln()) / ((hi + 1.0).ln() - (lo + 1.0).ln())).clamp(0.0, 1.0);
        let rgb = m.data.iter().map(|&v| viridis(f(v as f64))).collect();
        let tk = (0..5).map(|k| { let t = k as f64 / 4.0; (t, fmt_val(((lo + 1.0).ln() + t * ((hi + 1.0).ln() - (lo + 1.0).ln())).exp() - 1.0)) }).collect();
        (rgb, tk, stops(&viridis, 16))
    } else {
        let lo = percentile(&m.data, st.lo_pct);
        let hi = percentile(&m.data, st.hi_pct).max(lo + 1e-30);
        let rgb = m.data.iter().map(|&v| { let t = asinh01(v as f64, lo, hi, st.soft); [t, t, t] }).collect();
        let tk = [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|&t| (t, fmt_val(inv_asinh01(t, lo, hi, st.soft)))).collect();
        (rgb, tk, stops(&|t: f64| [t, t, t], 8))
    };
    let curve = match obs.ins.kind {
        Kind::Xray => "log(1 + counts)".to_string(),
        Kind::Cmb | Kind::Lensing => format!("linear, symmetric at the {}th percentile of |value|", st.hi_pct),
        _ => format!("asinh, softening {}, black/white at percentiles {}/{}", st.soft, st.lo_pct, st.hi_pct),
    };
    let curve = if st.smooth > 0.0 { format!("{curve}, smoothed with a {:.1} px Gaussian for display", st.smooth) } else { curve };
    let m = m0;
    let lg = Legend {
        title: format!("{}: {} ({})", obs.ins.name.to_uppercase(), m.name, unit),
        key: Key::Ramp { stops: ramp, ticks },
        note: format!(
            "Traces {}. Modelled on: {}. Beam/PSF FWHM {:.2} px, noise {} {} per pixel, {:.2}\"/px. Display: {}.",
            m.traces, obs.ins.analog, m.fwhm_px, fmt_val(m.sigma), unit, obs.pix_arcsec, curve
        ),
    };
    with_legend(&to_rgba(&rgb), w, h, &lg)
}

/// Hue for position k of n in chromatic order (blue → red), as sRGB.
pub fn chromatic(k: usize, n: usize) -> [f64; 3] {
    if n <= 1 { return [1.0, 1.0, 1.0]; }
    let hue = 240.0 * (1.0 - k as f64 / (n - 1) as f64);
    hsv(hue, 1.0, 1.0)
}

pub fn hsv(h: f64, s: f64, v: f64) -> [f64; 3] {
    let c = v * s;
    let hp = (h / 60.0) % 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
    let m = v - c;
    [r + m, g + m, b + m]
}

/// A layer of a composite: a stretched map and the colour it is drawn in.
pub struct Layer<'a> {
    pub data: &'a [f32],
    pub colour: [f64; 3],
    pub label: String,
    pub stretch: Stretch,
    /// Log stretch (X-ray counts) instead of asinh.
    pub log: bool,
    /// Flip the sign first (an SZ decrement shown as positive).
    pub negate: bool,
    /// Subtract a Gaussian-smoothed copy of this σ (px) first: a high-pass
    /// filter that removes structure larger than the clusters (0 = off).
    pub highpass: f64,
}

/// Additive false-colour composite. With `balance`, colours are normalised per
/// channel so that equal stretched values in every layer give white (used for
/// one instrument's bands); otherwise layers add as given (cross-instrument).
pub fn composite(layers: &[Layer], w: usize, h: usize, title: &str, note: &str, balance: bool) -> (Vec<u8>, usize, usize) {
    let mut norm = [0.0f64; 3];
    for l in layers { for k in 0..3 { norm[k] += l.colour[k]; } }
    let norm = if balance { norm.map(|v| v.max(1e-9)) } else { [1.0; 3] };
    let mut rgb = vec![[0.0f64; 3]; w * h];
    let datas: Vec<Vec<f32>> = layers.iter().map(|l| {
        let mut d: Vec<f32> = l.data.iter().map(|&v| if l.negate { -v } else { v }).collect();
        if l.highpass > 0.0 {
            let lo = smoothed(&d, w, h, l.highpass);
            for (a, b) in d.iter_mut().zip(&lo) { *a -= *b; }
        }
        smoothed(&d, w, h, l.stretch.smooth)
    }).collect();
    let shared = layers.iter().all(|l| l.stretch.shared);
    let (slo, shi) = if shared {
        (layers.iter().zip(&datas).map(|(l, d)| percentile(d, l.stretch.lo_pct)).fold(f64::MAX, f64::min),
         layers.iter().zip(&datas).map(|(l, d)| percentile(d, l.stretch.hi_pct)).fold(f64::MIN, f64::max))
    } else { (0.0, 0.0) };
    for (l, data) in layers.iter().zip(&datas) {
        let (lo, hi) = if shared { (slo, shi) } else { (percentile(data, l.stretch.lo_pct), percentile(data, l.stretch.hi_pct)) };
        let hi = hi.max(lo + 1e-30);
        for (i, &v) in data.iter().enumerate() {
            let t = if l.log {
                let f = |x: f64| (x.max(0.0) + 1.0).ln();
                ((f(v as f64) - f(lo)) / (f(hi) - f(lo)).max(1e-30)).clamp(0.0, 1.0)
            } else { asinh01(v as f64, lo, hi, l.stretch.soft) };
            for k in 0..3 { rgb[i][k] += t * l.colour[k] / norm[k]; }
        }
    }
    // soft highlight shoulder so saturated cores keep their hue
    for c in rgb.iter_mut() {
        let m = c[0].max(c[1]).max(c[2]);
        if m > 1.0 { for k in 0..3 { c[k] /= m; } }
    }
    let cats = layers.iter().map(|l| (l.label.clone(), l.colour.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8))).collect();
    let lg = Legend { title: title.to_string(), key: Key::Categories(cats), note: note.to_string() };
    with_legend(&to_rgba(&rgb), w, h, &lg)
}

/// Composite of all bands of one observation in chromatic order.
pub fn instrument_composite(obs: &Observation, st: Stretch) -> (Vec<u8>, usize, usize) {
    let n = obs.maps.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| obs.maps[a].lambda_um.partial_cmp(&obs.maps[b].lambda_um).unwrap());
    let layers: Vec<Layer> = idx.iter().enumerate().map(|(k, &b)| Layer {
        data: &obs.maps[b].data, colour: chromatic(k, n), label: obs.maps[b].name.clone(), stretch: st, log: obs.ins.kind == Kind::Xray, negate: false, highpass: 0.0,
    }).collect();
    composite(
        &layers, obs.w, obs.h,
        &format!("FALSE COLOUR: {} ({} bands, chromatic order)", obs.ins.name.to_uppercase(), n),
        &format!("Modelled on: {}. {} ({}), then added in the colour shown. {:.2}\"/px.",
            obs.ins.analog, if st.shared { "One black and white point for every band, so line ratios survive" } else { "Each band stretched on its own" }, if obs.ins.kind == Kind::Xray { "log counts".to_string() } else { format!("asinh, softening {}, percentiles {}/{}", st.soft, st.lo_pct, st.hi_pct) }, obs.pix_arcsec),
        true,
    )
}

/// Downsample an RGBA image by an integer box filter.
pub fn shrink(rgba: &[u8], w: usize, h: usize, f: usize) -> (Vec<u8>, usize, usize) {
    if f <= 1 { return (rgba.to_vec(), w, h); }
    let (nw, nh) = (w / f, h / f);
    let mut o = vec![255u8; nw * nh * 4];
    for y in 0..nh { for x in 0..nw {
        let mut s = [0u32; 3];
        for dy in 0..f { for dx in 0..f { let i = ((y * f + dy) * w + x * f + dx) * 4; for k in 0..3 { s[k] += rgba[i + k] as u32; } } }
        for k in 0..3 { o[(y * nw + x) * 4 + k] = (s[k] / (f * f) as u32) as u8; }
    }}
    (o, nw, nh)
}

/// Grid montage of labelled tiles (all the same size).
pub fn montage(tiles: &[(Vec<u8>, usize, usize, String)], cols: usize, title: &str, note: &str) -> (Vec<u8>, usize, usize) {
    let (tw, th) = (tiles[0].1, tiles[0].2);
    let rows = tiles.len().div_ceil(cols);
    let s = if tw >= 400 { 2 } else { 1 };
    let lab = 11 * s + 4;
    let gap = 4;
    let w = cols * tw + (cols + 1) * gap;
    let h = rows * (th + lab) + (rows + 1) * gap;
    let mut c = Canvas { w, h, rgba: vec![0u8; w * h * 4] };
    c.rect(0, 0, w, h, [11, 13, 16]);
    for (k, (img, iw, ih, label)) in tiles.iter().enumerate() {
        let (x0, y0) = (gap + (k % cols) * (tw + gap), gap + (k / cols) * (th + lab + gap));
        for y in 0..(*ih).min(th) { for x in 0..(*iw).min(tw) {
            let si = (y * iw + x) * 4;
            let di = ((y0 + y) * w + x0 + x) * 4;
            c.rgba[di..di + 4].copy_from_slice(&img[si..si + 4]);
        }}
        c.rect(x0, y0 + th, tw, lab, [11, 13, 16]);
        let lw: String = label.chars().take(tw / (6 * s)).collect();
        c.text(x0 + 2, y0 + th + 3, &lw, [216, 222, 230], s);
    }
    with_legend(&c.rgba, w, h, &Legend { title: title.into(), key: Key::Categories(vec![]), note: note.into() })
}

/// Generator diagnostics: mid-plane slices (x–y through the box centre) of
/// the physical state.
pub fn state_slices(sc: &MwScene) -> (Vec<u8>, usize, usize) {
    let v = &sc.vol;
    let n = v.n;
    let z = n / 2;
    let fields: Vec<(usize, &str, bool, f64, f64)> = if sc.kind == "pillars" {
        vec![(F_NH, "log n_H (cm^-3)", true, 0.0, 6.0), (F_XION, "ionised fraction", false, 0.0, 1.0), (F_U, "log U (ISRF)", true, 0.0, 5.0), (F_NHOT, "hot plasma n_e (cm^-3)", false, 0.0, 0.3)]
    } else {
        vec![(F_NH, "log n_e (cm^-3)", true, -7.5, -4.0), (F_TGAS, "log T (K)", true, 4.0, 7.0)]
    };
    let mut tiles = Vec::new();
    for &(f, name, lg, lo, hi) in &fields {
        let mut rgb = vec![[0.0; 3]; n * n];
        for y in 0..n { for x in 0..n {
            let val = v.f[f][(z * n + (n - 1 - y)) * n + x] as f64;
            let t = if lg { (val.max(1e-30).log10() - lo) / (hi - lo) } else { (val - lo) / (hi - lo) };
            rgb[y * n + x] = viridis(t);
        }}
        let up = (256 / n).max(1);
        let mut big = vec![[0.0; 3]; n * up * n * up];
        for y in 0..n * up { for x in 0..n * up { big[y * n * up + x] = rgb[(y / up) * n + x / up]; } }
        tiles.push((to_rgba(&big), n * up, n * up, format!("{name} [{lo}..{hi}]")));
    }
    montage(&tiles, tiles.len(), &format!("DIAGNOSTIC: GENERATED STATE ({}), SLICE THROUGH THE BOX CENTRE", sc.kind.to_uppercase()),
        "Viridis colour scale, fixed ranges in brackets. These are the inputs to every band, not observations.")
}

/// Redshift-space wedge of the galaxy catalogue (a 'CfA slice'): galaxies
/// within ±`half_dec` of the equator of the view, plotted by angle across the
/// view and by recession velocity cz. Left: real space; right: redshift space,
/// where cluster velocity dispersions stretch clusters into fingers of god.
pub fn redshift_wedge(sc: &MwScene, half_dec: f64, fov_x: f64, mag_limit_mstar: f64) -> (Vec<u8>, usize, usize) {
    let (w, h) = (1024usize, 640usize);
    let mut c = Canvas { w, h, rgba: vec![0u8; w * h * 4] };
    c.rect(0, 0, w, h, [5, 7, 10]);
    let cz_max = 21000.0;
    let (h0, _om, _) = sc.cosmo.unwrap_or((0.7, 0.3, 120.0));
    let panels = [(0usize, "REAL SPACE (distance x H0)"), (1, "REDSHIFT SPACE (cz)")];
    let pw = w / 2;
    for &(p, name) in &panels {
        let cx = (p * pw + pw / 2) as f64;
        let cy = h as f64 - 30.0;
        let rmax = h as f64 - 70.0;
        // wedge outline
        for k in 0..400 {
            let t = k as f64 / 399.0;
            for &edge in &[-0.5, 0.5] {
                let a = edge * fov_x;
                let (x, y) = (cx + rmax * t * a.sin(), cy - rmax * t * a.cos());
                c.rect(x as usize, y as usize, 1, 1, [60, 70, 85]);
            }
            for frac in [0.25, 0.5, 0.75, 1.0] {
                let a = (t - 0.5) * fov_x;
                let (x, y) = (cx + rmax * frac * a.sin(), cy - rmax * frac * a.cos());
                c.rect(x as usize, y as usize, 1, 1, [40, 48, 60]);
            }
        }
        for s in &sc.sources {
            let Sed::Galaxy { mstar, .. } = s.sed else { continue };
            if mstar < mag_limit_mstar { continue; }
            let r = (s.pos[0].powi(2) + s.pos[1].powi(2) + s.pos[2].powi(2)).sqrt();
            let dec = (s.pos[1] / r).asin();
            if dec.abs() > half_dec { continue; }
            let ra = s.pos[0].atan2(s.pos[2]);
            if ra.abs() > 0.5 * fov_x { continue; }
            let vel = if p == 0 { 100.0 * h0 * r / super::phys::MPC } else { 299_792.458 * s.z };
            if vel > cz_max { continue; }
            let rr = vel / cz_max * rmax;
            let (x, y) = (cx + rr * ra.sin(), cy - rr * ra.cos());
            let col = if s.flags & 2 != 0 { [255, 120, 90] } else { [120, 180, 255] };
            if x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h { c.rect(x as usize, y as usize, 2, 2, col); }
        }
        c.text(p * pw + 12, 10, name, [216, 222, 230], 2);
        for frac in [0.25, 0.5, 0.75, 1.0] {
            let v = frac * cz_max;
            c.text((cx + 4.0) as usize, (cy - rmax * frac - 14.0) as usize, &format!("{:.0} km/s", v), [125, 136, 150], 1);
        }
    }
    let title = "DIAGNOSTIC: REDSHIFT SURVEY WEDGE, GALAXY CATALOGUE";
    let note = format!(
        "Galaxies within +/-{:.1} deg of the view's equator, stellar mass > {:.0e} Msun. Red: quenched, blue: star-forming. Same galaxies in both panels; peculiar velocities from the Zel'dovich flow and cluster dispersions move them along the line of sight.",
        half_dec.to_degrees(), mag_limit_mstar
    );
    with_legend(&c.rgba, w, h, &Legend { title: title.into(), key: Key::Categories(vec![("quenched".into(), [255, 120, 90]), ("star-forming".into(), [120, 180, 255])]), note })
}
