//! Synthetic sensor: turns the renderer's converged radiance into the data a
//! camera would record, then into display images and diagnostics.
//!
//! Readout, per filter (all in electrons):
//! 1. rate = Σ_channels L̄ · (filter × QE × bin width) · A_eff · Ω_pix
//!    (*physically derived* from the photon radiance and the optics);
//! 2. PSF: a Gaussian core with FWHM² = FWHM_optics² + (1.03 λ_eff/D)² (the
//!    Airy core width is physically derived, the optics term is a parameter),
//!    plus a Moffat (β = 2.5) halo holding `halo_fraction` of the light and,
//!    if the telescope has spider vanes, diffraction spikes holding the
//!    fraction of the pupil area the vanes cover (*numerically approximated*;
//!    the wide part is applied by FFT convolution, flux is conserved);
//! 3. N̄ = rate · t + dark · t, then a **Poisson** realisation, Gaussian read
//!    noise, optional cosmic rays, full-well saturation, and gain to ADU.
//!
//! The noise field is *frozen*: each pixel keeps its own random numbers, and
//! the Poisson draw is an inversion of the CDF at that fixed uniform, so as the
//! exposure grows the noise shrinks smoothly (√N) instead of flickering. Each
//! readout is still an exact Poisson (small N) or normal-approximation
//! (N ≥ 30) draw.
//!
//! The simulated exposure time grows with the render: t = exposure_s ·
//! min(1, spp / target_spp). The shutter is notionally open while the renderer
//! accumulates. Renderer (Monte-Carlo) noise and sensor (shot/read) noise are
//! therefore separate quantities, reported and viewable separately.

use crate::json::Json;
use crate::render::*;
use crate::rng::Rng;
use crate::spectrum::{true_color_weights, NCH};

/// What the sensor and the display need from the renderer, per pixel. The
/// browser's workers send exactly this (not the full spectral sums).
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub spp: u32,
    /// e⁻ s⁻¹ per filter, before the PSF.
    pub rates: Vec<Vec<f32>>,
    /// Linear sRGB "true colour" from the spectral means (presentation only).
    pub truecol: Vec<[f32; 3]>,
    /// Mean and relative standard error of the primary-filter rate.
    pub lmean: Vec<f32>,
    pub rel_err: Vec<f32>,
    /// Diagnostics summed over samples (same layout as Accum::diag).
    pub diag: Vec<f32>,
}

/// The parts of a scene the sensor needs (no sky, no geodesics).
pub struct Instrument {
    pub scene: crate::scene::Scene,
    pub filters: Vec<FilterInfo>,
    pub g_sky: f64,
}

impl Instrument {
    pub fn new(scene: crate::scene::Scene) -> Instrument {
        let g_sky = 1.0 / (1.0 - 2.0 * scene.mass / scene.cam_r).sqrt();
        let filters = make_filters(&scene, g_sky);
        Instrument { scene, filters, g_sky }
    }
    pub fn from_ctx(ctx: &Ctx) -> Instrument {
        Instrument { scene: ctx.scene.clone(), filters: make_filters(&ctx.scene, ctx.g_sky), g_sky: ctx.g_sky }
    }
}

/// Reduce accumulators for rows [y0, y1) to a frame slice (rates, colour, error, diagnostics).
pub fn frame_rows(ins: &Instrument, w: usize, y0: usize, y1: usize, spp: u32, sum: &[f32], lsum: &[f64], lsq: &[f64], diag: &[f32]) -> Frame {
    let npx = w * (y1 - y0);
    let n = spp.max(1) as f64;
    let cw = true_color_weights(ins.g_sky);
    let mut fr = Frame { w, h: y1 - y0, spp, rates: vec![vec![0.0; npx]; ins.filters.len()], truecol: vec![[0.0; 3]; npx], lmean: vec![0.0; npx], rel_err: vec![0.0; npx], diag: diag[..npx * ND].to_vec() };
    for p in 0..npx {
        let sp = &sum[p * NCH..(p + 1) * NCH];
        for (fi, f) in ins.filters.iter().enumerate() {
            let mut r = 0.0;
            for c in 0..NCH { r += sp[c] as f64 * f.w[c]; }
            fr.rates[fi][p] = (r / n) as f32;
        }
        let mut c3 = [0f64; 3];
        for ch in 0..NCH { for k in 0..3 { c3[k] += sp[ch] as f64 / n * cw[ch][k]; } }
        fr.truecol[p] = [c3[0] as f32, c3[1] as f32, c3[2] as f32];
        let m = lsum[p] / n;
        let var = (lsq[p] / n - m * m).max(0.0);
        fr.lmean[p] = m as f32;
        fr.rel_err[p] = if m > 0.0 && n > 1.0 { ((var / (n - 1.0)).sqrt() / m) as f32 } else { 0.0 };
    }
    fr
}

pub fn frame(ins: &Instrument, acc: &Accum) -> Frame {
    frame_rows(ins, acc.w, 0, acc.h, acc.spp, &acc.sum, &acc.lsum, &acc.lsq, &acc.diag)
}

pub struct Readout {
    pub w: usize,
    pub h: usize,
    pub t: f64,
    pub filters: Vec<String>,
    /// Noise-free electrons (after PSF), per filter.
    pub expected: Vec<Vec<f32>>,
    /// Realised electrons (after noise and saturation), per filter. For a
    /// composite or stack: the calibrated merged rate × `g_ref`.
    pub electrons: Vec<Vec<f32>>,
    pub saturated: f64,
    pub cosmic_rays: usize,
    /// Calibrated rate (e⁻ s⁻¹ before the ND filter, dark subtracted), per filter.
    pub rate: Vec<Vec<f32>>,
    /// More than one frame was merged.
    pub composite: bool,
    /// Effective integration Σ count × t × 10^−ND (s) the electrons refer to.
    pub g_ref: f64,
    /// The frames actually taken at this render progress: (t, ND, count).
    pub frames: Vec<(f64, f64, u32)>,
}

/// Inverse standard-normal CDF (Acklam's rational approximation, |ε| < 1.2e-9).
pub fn inv_norm(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e1, 2.209460984245205e2, -2.759285104469687e2, 1.383577518672690e2, -3.066479806614716e1, 2.506628277459239];
    const B: [f64; 5] = [-5.447609879822406e1, 1.615858368580409e2, -1.556989798598866e2, 6.680131188771972e1, -1.328068155288572e1];
    const C: [f64; 6] = [-7.784894002430293e-3, -3.223964580411365e-1, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    const D: [f64; 4] = [7.784695709041462e-3, 3.224671290700398e-1, 2.445134137142996, 3.754408661907416];
    let p = p.clamp(1e-12, 1.0 - 1e-12);
    if p < 0.02425 {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p > 1.0 - 0.02425 {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

/// Poisson draw by CDF inversion at a fixed uniform u.
pub fn poisson_inv(mean: f64, u: f64) -> f64 {
    if mean <= 0.0 {
        return 0.0;
    }
    if mean < 30.0 {
        let mut p = (-mean).exp();
        let mut c = p;
        let mut k = 0.0;
        while c < u && k < 200.0 {
            k += 1.0;
            p *= mean / k;
            c += p;
        }
        k
    } else {
        (mean + mean.sqrt() * inv_norm(u) + 0.5).floor().max(0.0)
    }
}

fn gauss_blur(img: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    if sigma < 0.05 {
        return img.to_vec();
    }
    let r = (3.5 * sigma).ceil() as i64;
    // pixel-integrated Gaussian weights
    let erf = |x: f64| {
        let t = 1.0 / (1.0 + 0.3275911 * x.abs());
        let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
        if x >= 0.0 { y } else { -y }
    };
    let k: Vec<f32> = (-r..=r).map(|i| (0.5 * (erf((i as f64 + 0.5) / (sigma * 2f64.sqrt())) - erf((i as f64 - 0.5) / (sigma * 2f64.sqrt())))) as f32).collect();
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (j, kk) in k.iter().enumerate() {
                let xx = x as i64 + j as i64 - r;
                if xx >= 0 && (xx as usize) < w { s += img[y * w + xx as usize] * kk; }
            }
            tmp[y * w + x] = s;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (j, kk) in k.iter().enumerate() {
                let yy = y as i64 + j as i64 - r;
                if yy >= 0 && (yy as usize) < h { s += tmp[yy as usize * w + x] * kk; }
            }
            out[y * w + x] = s;
        }
    }
    out
}

// ------------------------------------------------------------- FFT --
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 { j ^= bit; bit >>= 1; }
        j |= bit;
        if i < j { re.swap(i, j); im.swap(i, j); }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * std::f64::consts::PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let (xr, xi) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - xr; im[b] = im[a] - xi;
                re[a] += xr; im[a] += xi;
                let t = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = t;
            }
        }
        len <<= 1;
    }
    if inverse { for i in 0..n { re[i] /= n as f64; im[i] /= n as f64; } }
}

fn fft2(re: &mut [f64], im: &mut [f64], w: usize, h: usize, inverse: bool) {
    let mut rr = vec![0.0; h];
    let mut ii = vec![0.0; h];
    for y in 0..h { fft(&mut re[y * w..(y + 1) * w], &mut im[y * w..(y + 1) * w], inverse); }
    for x in 0..w {
        for y in 0..h { rr[y] = re[y * w + x]; ii[y] = im[y * w + x]; }
        fft(&mut rr, &mut ii, inverse);
        for y in 0..h { re[y * w + x] = rr[y]; im[y * w + x] = ii[y]; }
    }
}

/// Linear (zero-padded) convolution of img with a centred kernel of radius r, via FFT.
fn fft_convolve(img: &[f32], w: usize, h: usize, k: &[f32], r: i64) -> Vec<f32> {
    let pw = (w + 2 * r as usize).next_power_of_two();
    let ph = (h + 2 * r as usize).next_power_of_two();
    let n = (2 * r + 1) as usize;
    let mut ar = vec![0.0; pw * ph];
    let mut ai = vec![0.0; pw * ph];
    for y in 0..h { for x in 0..w { ar[y * pw + x] = img[y * w + x] as f64; } }
    let mut br = vec![0.0; pw * ph];
    let mut bi = vec![0.0; pw * ph];
    for ky in 0..n { for kx in 0..n {
        let (dx, dy) = (kx as i64 - r, ky as i64 - r);
        let (x, y) = (dx.rem_euclid(pw as i64) as usize, dy.rem_euclid(ph as i64) as usize);
        br[y * pw + x] = k[ky * n + kx] as f64;
    }}
    fft2(&mut ar, &mut ai, pw, ph, false);
    fft2(&mut br, &mut bi, pw, ph, false);
    for i in 0..pw * ph {
        let (re, im) = (ar[i] * br[i] - ai[i] * bi[i], ar[i] * bi[i] + ai[i] * br[i]);
        ar[i] = re; ai[i] = im;
    }
    fft2(&mut ar, &mut ai, pw, ph, true);
    let mut out = vec![0f32; w * h];
    for y in 0..h { for x in 0..w { out[y * w + x] = ar[y * pw + x] as f32; } }
    out
}

/// Halo + spike kernel (sums to 1), radius R.
fn wide_kernel(core_fwhm: f64, vanes: u32, spike_frac: f64, halo_frac: f64) -> (Vec<f32>, i64) {
    // circular support with a smooth taper over its outer fifth, so the halo of
    // a very bright source never shows a square or polygonal edge
    let r = ((24.0 * core_fwhm).ceil() as i64).clamp(40, 96);
    let n = (2 * r + 1) as usize;
    let mut k = vec![0f64; n * n];
    // Moffat halo, FWHM = 6 × core
    let beta = 2.5;
    let fw = (6.0 * core_fwhm).max(4.0);
    let alpha = fw / (2.0 * (2f64.powf(1.0 / beta) - 1.0).sqrt());
    let taper = |rr: f64| { let u = ((rr / r as f64 - 0.8) / 0.2).clamp(0.0, 1.0); 0.5 * (1.0 + (std::f64::consts::PI * u).cos()) };
    let mut hs = 0.0;
    for y in -r..=r { for x in -r..=r {
        let rr = ((x * x + y * y) as f64).sqrt();
        let v = (1.0 + ((x * x + y * y) as f64) / (alpha * alpha)).powf(-beta) * taper(rr);
        k[((y + r) as usize) * n + (x + r) as usize] = v;
        hs += v;
    }}
    for v in k.iter_mut() { *v *= halo_frac / hs; }
    if vanes > 0 && spike_frac > 0.0 {
        let mut sk = vec![0f64; n * n];
        let mut ss = 0.0;
        let arms = if vanes == 3 { 6 } else { 2 * vanes.max(2) / 2 * 2 };
        for a in 0..arms {
            let th = std::f64::consts::PI * 2.0 * a as f64 / arms as f64 + std::f64::consts::FRAC_PI_4;
            for t in 2..=r {
                let (x, y) = ((th.cos() * t as f64).round() as i64, (th.sin() * t as f64).round() as i64);
                if x.abs() > r || y.abs() > r { continue; }
                let v = taper(t as f64) / (t as f64).powi(2);
                sk[((y + r) as usize) * n + (x + r) as usize] += v;
                ss += v;
            }
        }
        for (a, b) in k.iter_mut().zip(&sk) { *a += b * spike_frac / ss; }
    }
    (k.iter().map(|&v| v as f32).collect(), r)
}

pub fn readout(ins: &Instrument, fr: &Frame) -> Readout {
    let s = &ins.scene;
    let (w, h) = (fr.w, fr.h);
    let prog = (fr.spp as f64 / s.target_spp.max(1) as f64).min(1.0);
    let exps = s.exposure_list();
    let composite = exps.len() > 1 || exps[0].count > 1;
    let t = exps[0].exposure_s * prog;
    let g_ref: f64 = if composite {
        exps.iter().map(|e| e.count as f64 * e.exposure_s * prog * 10f64.powf(-e.neutral_density)).sum()
    } else {
        t * 10f64.powf(-exps[0].neutral_density)
    };
    let mut rates = Vec::new();
    let d = s.aperture();
    let mut expected = Vec::new();
    let mut electrons = Vec::new();
    let mut saturated = 0usize;
    let mut cr_total = 0usize;
    for fi in 0..ins.filters.len() {
        let rate = fr.rates[fi].clone();
        let img = if s.psf {
            let core = psf_core_fwhm_px(ins, fi);
            let spike = if s.spider_vanes > 0 { (s.spider_vanes as f64 * 0.002 * d / 2.0) / (std::f64::consts::PI * d * d / 4.0) * 2.0 } else { 0.0 };
            let wide = (s.halo_fraction + spike).min(0.5);
            // core and wide parts both conserve flux; the wide (halo + spike)
            // kernel is applied to every pixel by FFT convolution
            let core_in: Vec<f32> = rate.iter().map(|&r| r * (1.0 - wide as f32)).collect();
            let mut out = gauss_blur(&core_in, w, h, core / 2.3548);
            if wide > 0.0 {
                let (k, r) = wide_kernel(core, s.spider_vanes, spike, s.halo_fraction);
                let ksum: f32 = k.iter().sum();
                let kn: Vec<f32> = k.iter().map(|v| v * wide as f32 / ksum.max(1e-30)).collect();
                let halo = fft_convolve(&rate, w, h, &kn, r);
                for (o, hh) in out.iter_mut().zip(&halo) { *o += hh.max(0.0); }
            }
            out
        } else {
            rate
        };
        let shutter = if s.shutter_open { 1.0f64 } else { 0.0 };
        let mut el = vec![0f32; w * h];
        let mut exp = vec![0f32; w * h];
        let mut rt = vec![0f32; w * h];
        if !composite {
            let e0 = &exps[0];
            let g = t * 10f64.powf(-e0.neutral_density);
            for p in 0..w * h { exp[p] = (img[p] as f64 * g * shutter + s.dark_e_per_s * t) as f32; }
            for p in 0..w * h {
                let n = exp[p] as f64;
                el[p] = if s.sensor_noise {
                    let mut rng = Rng::keyed(&[s.seed, 0x5E45, fi as u64, p as u64]);
                    let u = rng.f64();
                    let z = rng.normal();
                    (poisson_inv(n, u) + s.read_noise_e * z) as f32
                } else {
                    n as f32
                };
            }
            // cosmic rays: count ∝ t over the sensor area; positions frozen by index
            if s.sensor_noise && s.cosmic_rays_per_cm2_s > 0.0 {
                let area_cm2 = (w as f64 * s.pixel_um * 1e-4) * (h as f64 * s.pixel_um * 1e-4);
                let mut rng = Rng::keyed(&[s.seed, 0xC0517, fi as u64]);
                let count = poisson_inv(s.cosmic_rays_per_cm2_s * area_cm2 * t, rng.f64()) as usize;
                for _ in 0..count {
                    let (x, y) = (rng.f64() * w as f64, rng.f64() * h as f64);
                    let ang = rng.f64() * std::f64::consts::PI;
                    let len = 1.0 + rng.f64() * 6.0;
                    let e = 400.0 + rng.f64() * 2500.0;
                    for k in 0..=(len as usize * 2) {
                        let tt = k as f64 / 2.0;
                        let (xx, yy) = ((x + ang.cos() * tt) as usize, (y + ang.sin() * tt) as usize);
                        if xx < w && yy < h { el[yy * w + xx] += (e / (len * 2.0 + 1.0)) as f32; }
                    }
                }
                cr_total += count;
            }
            for v in el.iter_mut() {
                if *v >= s.full_well_e as f32 { *v = s.full_well_e as f32; saturated += 1; }
                *v = v.max(0.0);
            }
            let gd = g.max(1e-300);
            for p in 0..w * h { rt[p] = ((el[p] as f64 - s.dark_e_per_s * t) / gd) as f32; }
        } else {
            // Registered frames: each group of `count` frames of length t_k through
            // ND_k. The sum of n Poisson(μ) frames is exactly Poisson(nμ), and the
            // read noise adds in quadrature, so a group is drawn once. Groups whose
            // single-frame expectation reaches 95% of full well are saturated and
            // dropped; the rest give the maximum-likelihood rate
            //   R = Σ (S_k − n_k dark t_k) / Σ n_k g_k,   g_k = t_k 10^−ND_k.
            for p in 0..w * h {
                let (mut num, mut den) = (0.0f64, 0.0f64);
                let mut least: Option<(f64, f64)> = None; // (g, frame value) of the least exposed group
                for (k, e) in exps.iter().enumerate() {
                    let tk = e.exposure_s * prog;
                    let gk = tk * 10f64.powf(-e.neutral_density);
                    let n = e.count as f64;
                    let mu = img[p] as f64 * gk * shutter + s.dark_e_per_s * tk;
                    let sum = if s.sensor_noise {
                        let mut rng = Rng::keyed(&[s.seed, 0x5E45, fi as u64, p as u64, k as u64 + 1]);
                        let u = rng.f64();
                        let z = rng.normal();
                        poisson_inv(n * mu, u) + s.read_noise_e * n.sqrt() * z
                    } else {
                        n * mu
                    };
                    if least.map_or(true, |(g0, _)| gk < g0) { least = Some((gk, s.full_well_e.min(mu))); }
                    if mu < 0.95 * s.full_well_e {
                        num += sum - n * s.dark_e_per_s * tk;
                        den += n * gk;
                    }
                }
                rt[p] = if den > 0.0 {
                    (num / den) as f32
                } else {
                    saturated += 1;
                    let (g0, v) = least.unwrap();
                    (v / g0.max(1e-300)) as f32
                };
                exp[p] = (img[p] as f64 * g_ref) as f32;
                el[p] = (rt[p] as f64 * g_ref).max(0.0) as f32;
            }
        }
        expected.push(exp);
        electrons.push(el);
        rates.push(rt);
    }
    Readout {
        w,
        h,
        t: if composite { exps.iter().map(|e| e.count as f64 * e.exposure_s * prog).sum() } else { t },
        filters: ins.filters.iter().map(|f| f.name.clone()).collect(),
        expected,
        electrons,
        saturated: saturated as f64 / (w * h * ins.filters.len().max(1)) as f64,
        cosmic_rays: cr_total,
        rate: rates,
        composite,
        g_ref,
        frames: exps.iter().map(|e| (e.exposure_s * prog, e.neutral_density, e.count)).collect(),
    }
}

// ------------------------------------------------------------- stats --
#[derive(Clone, Debug, Default)]
pub struct Convergence {
    /// Median relative standard error of the pixel means (renderer MC noise),
    /// over the brightest tenth of the pixels with signal.
    pub render_noise: f64,
    /// Median shot+read+dark noise relative to signal, over pixels with signal.
    pub sensor_noise: f64,
    pub failed_fraction: f64,
    pub mean_steps: f64,
    pub mean_vol_steps: f64,
    pub disk_fraction: f64,
    pub capture_fraction: f64,
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() { return 0.0; }
    let k = v.len() / 2;
    v.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).unwrap());
    v[k]
}

pub fn convergence(ins: &Instrument, fr: &Frame, ro: Option<&Readout>) -> Convergence {
    let n = fr.spp.max(1) as f64;
    let npx = fr.w * fr.h;
    let mut rel = Vec::with_capacity(npx);
    // Renderer noise is reported where the image has structure: the median
    // relative error over the brightest tenth of the pixels with signal
    // (smooth, faint sky would otherwise dominate the median and hide the
    // noise on the disk, nebula or stars)
    let lfloor = percentile(&fr.lmean, 0.99) * 1e-3;
    let sig: Vec<f32> = fr.lmean.iter().cloned().filter(|&v| v as f64 > lfloor && v > 0.0).collect();
    let bright = if sig.is_empty() { f64::INFINITY } else { percentile(&sig, 0.9) };
    let (mut fail, mut steps, mut vs, mut vr, mut disk, mut capt) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in 0..npx {
        if fr.lmean[p] as f64 >= bright && fr.spp > 1 {
            rel.push(fr.rel_err[p] as f64);
        }
        let d = &fr.diag[p * ND..(p + 1) * ND];
        fail += d[D_FAIL] as f64;
        steps += d[D_STEPS] as f64;
        vs += d[D_VSTEPS] as f64;
        if d[D_VSTEPS] > 0.0 { vr += 1.0; }
        disk += d[D_GN] as f64 + d[D_UNRES] as f64;
        capt += d[D_CAPT] as f64;
    }
    let total = n * npx as f64;
    let sensor = ro.and_then(|r| r.expected.first().map(|e| {
        let rd = ins.scene.read_noise_e;
        let floor = percentile(e, 0.99) * 1e-3;
        median(e.iter().filter(|&&x| x as f64 > floor && x > 0.0).map(|&x| ((x as f64) + rd * rd).sqrt() / x as f64).collect())
    })).unwrap_or(0.0);
    Convergence {
        render_noise: median(rel),
        sensor_noise: sensor,
        failed_fraction: fail / total,
        mean_steps: steps / total,
        mean_vol_steps: if vr > 0.0 { vs / (vr * n) } else { 0.0 },
        disk_fraction: disk / total,
        capture_fraction: capt / total,
    }
}

// ----------------------------------------------------------- display --
pub const VIEWS: &[&str] = &[
    "sensor", "radiance", "band", "fate", "winding", "redshift", "invariant", "steps", "tau", "nebula", "variance", "noise", "correspondence",
];

// Fixed diagnostic scales (so images from different renders compare directly).
pub const STEPS_MAX: f64 = 200.0;
pub const WIND_MAX: f64 = 2.5;
pub const G_LO: f64 = 0.4;
pub const G_HI: f64 = 1.6;
pub const TAU_LO: f64 = -2.0;
pub const TAU_HI: f64 = 1.0;
pub const NEB_LO: f64 = 8.0;
pub const NEB_HI: f64 = 12.0;
pub const INV_LO: f64 = -16.0;
pub const INV_HI: f64 = -6.0;

/// The legend of a diagnostic view (None for presentation views).
pub fn legend(view: &str, ins: &Instrument) -> Option<crate::annotate::Legend> {
    use crate::annotate::{stops, Key, Legend};
    let ramp = |lo: f64, hi: f64, ticks: &[f64], fmt: &dyn Fn(f64) -> String, f: &dyn Fn(f64) -> [f64; 3]| Key::Ramp {
        stops: stops(f, 64),
        ticks: ticks.iter().map(|&v| ((v - lo) / (hi - lo), fmt(v))).collect(),
    };
    let dec = |v: f64| format!("1E{}", v as i64);
    let lg = match view {
        "fate" => Legend {
            title: "DIAGNOSTIC: RAY FATE (MAJORITY OF SAMPLES PER PIXEL)".into(),
            key: Key::Categories(FATE_CLASSES.iter().map(|(n, c)| (n.to_string(), *c)).chain(std::iter::once(("hatched: < 95% majority".to_string(), [70, 70, 74]))).collect()),
            note: format!("NOT AN IMAGE OF LIGHT. {} MODE, R0 = {:.4E} M, RTOL {:.0E}, MAX {} WINDINGS", ins.scene.mode.to_uppercase(), ins.scene.cam_r, ins.scene.rtol, ins.scene.max_windings),
        },
        "winding" => Legend { title: "DIAGNOSTIC: ORBITAL ANGLE SWEPT, PHI / 2PI (TURNS)".into(), key: ramp(0.0, WIND_MAX, &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5], &|v| format!("{v:.1}"), &viridis), note: "MEAN OVER SAMPLES; FIXED SCALE 0-2.5 TURNS".into() },
        "redshift" => Legend { title: "DIAGNOSTIC: DISK FREQUENCY RATIO G = NU_OBS / NU_EMIT".into(), key: ramp(G_LO, G_HI, &[0.4, 0.7, 1.0, 1.3, 1.6], &|v| format!("{v:.1}"), &|t| coolwarm(1.0 - t)), note: "MEAN OVER DISK HITS; G > 1 BLUESHIFT (BLUE), G < 1 REDSHIFT (RED); GREY: NO DISK HIT".into() },
        "invariant" => Legend { title: "DIAGNOSTIC: FIRST-INTEGRAL DRIFT |B^2 ((DU/DPHI)^2 + U^2 (1-2U)) - 1|".into(), key: ramp(INV_LO, INV_HI, &[-16.0, -14.0, -12.0, -10.0, -8.0, -6.0], &dec, &viridis), note: "LOG MEAN OVER SAMPLES".into() },
        "steps" => Legend { title: "DIAGNOSTIC: GEODESIC STEPS PER SAMPLE (DORMAND-PRINCE 5(4))".into(), key: ramp(0.0, STEPS_MAX, &[0.0, 50.0, 100.0, 150.0, 200.0], &|v| format!("{v:.0}"), &viridis), note: "ACCEPTED + REJECTED STEPS".into() },
        "tau" => Legend { title: "DIAGNOSTIC: NEBULA DUST OPTICAL DEPTH TAU_V ALONG THE RAY".into(), key: ramp(TAU_LO, TAU_HI, &[-2.0, -1.0, 0.0, 1.0], &|v| format!("{}", 10f64.powf(v)), &viridis), note: "FALSE COLOUR, LOG SCALE; BLACK: RAY MISSES THE VOLUME".into() },
        "nebula" => Legend { title: "DIAGNOSTIC: NEBULA LINE RADIANCE (PHOTONS S-1 M-2 SR-1, ALL LINES)".into(), key: ramp(NEB_LO, NEB_HI, &[8.0, 9.0, 10.0, 11.0, 12.0], &dec, &viridis), note: "FALSE COLOUR, LOG SCALE, AFTER INTERNAL AND FOREGROUND EXTINCTION".into() },
        "variance" => Legend { title: "DIAGNOSTIC: RENDERER (MONTE CARLO) RELATIVE STANDARD ERROR".into(), key: ramp(-4.0, 0.0, &[-4.0, -3.0, -2.0, -1.0, 0.0], &dec, &viridis), note: "PRIMARY FILTER, PER PIXEL; INDEPENDENT OF SENSOR NOISE".into() },
        "correspondence" => Legend {
            title: "DEBUG: SOURCE-FEATURE CORRESPONDENCE (VOLUME LANDMARKS)".into(),
            key: Key::Categories(LANDMARK_COLOURS.iter().enumerate().map(|(i, c)| (format!("landmark {}", i + 1), *c)).collect()),
            note: "PIXELS WHOSE RAYS PASS WITHIN 0.5 PC OF A BRIGHT H-ALPHA KNOT. NEVER FOR PRESENTATION".into(),
        },
        "noise" => Legend { title: "DIAGNOSTIC: SENSOR NOISE (REALISED - EXPECTED) / SIGMA".into(), key: ramp(-3.0, 3.0, &[-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0], &|v| format!("{v:+.0}"), &|t| [t, t, t]), note: "SHOT + READ + DARK, FIRST FILTER; FROZEN PER PIXEL".into() },
        _ => return None,
    };
    Some(lg)
}

fn asinh_stretch(v: f64, black: f64, white: f64, soft: f64) -> f64 {
    let x = ((v - black) / (white - black).max(1e-30)).max(0.0);
    ((x / soft).asinh() / (1.0 / soft).asinh()).min(1.0)
}

fn percentile(v: &[f32], q: f64) -> f64 {
    let mut s: Vec<f32> = v.iter().cloned().filter(|x| x.is_finite()).collect();
    if s.is_empty() { return 0.0; }
    let k = ((s.len() - 1) as f64 * q) as usize;
    s.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).unwrap());
    s[k] as f64
}

/// sRGB → linear (display colours below are specified in sRGB).
fn lin(c: [f64; 3]) -> [f64; 3] {
    c.map(|x| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) })
}
fn lin8(c: [u8; 3]) -> [f64; 3] { lin(c.map(|x| x as f64 / 255.0)) }

/// Perceptually uniform sequential map (viridis, polynomial fit), sRGB.
pub fn viridis(t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    const C: [[f64; 3]; 7] = [
        [0.277_727_33, 0.005_407_34, 0.334_099_81],
        [0.105_093_04, 1.404_613_53, 1.384_590_16],
        [-0.330_861_83, 0.214_847_56, 0.095_095_16],
        [-4.634_230_50, -5.799_100_97, -19.332_440_96],
        [6.228_269_94, 14.179_933_37, 56.690_552_60],
        [4.776_385_00, -13.745_145_38, -65.353_032_63],
        [-5.435_455_86, 4.645_852_61, 26.312_435_25],
    ];
    [0, 1, 2].map(|k| (C[0][k] + t * (C[1][k] + t * (C[2][k] + t * (C[3][k] + t * (C[4][k] + t * (C[5][k] + t * C[6][k])))))).clamp(0.0, 1.0))
}

/// Diverging blue–white–red (Moreland's cool–warm), t ∈ [0, 1], sRGB.
pub fn coolwarm(t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    let (a, m, b) = ([59.0, 76.0, 192.0], [221.0, 221.0, 221.0], [180.0, 4.0, 38.0]);
    if t < 0.5 { let u = t * 2.0; [0, 1, 2].map(|k| (a[k] * (1.0 - u) + m[k] * u) / 255.0) } else { let u = (t - 0.5) * 2.0; [0, 1, 2].map(|k| (m[k] * (1.0 - u) + b[k] * u) / 255.0) }
}

/// Debug landmark colours (sRGB).
pub const LANDMARK_COLOURS: [[u8; 3]; 5] = [[255, 64, 64], [64, 220, 90], [70, 140, 255], [255, 210, 40], [220, 80, 255]];

/// Ray-fate categories (sRGB), in diagnostic order.
pub const FATE_CLASSES: [(&str, [u8; 3]); 7] = [
    ("escaped (sky)", [154, 166, 184]),
    ("captured (r < 2M)", [18, 18, 20]),
    ("disk, direct image", [255, 176, 48]),
    ("disk, secondary image", [235, 104, 28]),
    ("disk, order >= 3", [255, 236, 150]),
    ("disk, unresolved order (dark)", [130, 80, 200]),
    ("integration failure", [255, 0, 200]),
];

/// Majority fate class of a pixel and the fraction of samples in it.
pub fn fate_majority(d: &[f32]) -> (usize, f64) {
    let c = [d[D_ESC], d[D_CAPT], d[D_DISK1], d[D_DISK2], d[D_DISK3], d[D_UNRES], d[D_FAIL]];
    let tot: f32 = c.iter().sum();
    let (mut k, mut best) = (0, -1.0f32);
    for (i, &v) in c.iter().enumerate() { if v > best { best = v; k = i; } }
    (k, if tot > 0.0 { (best / tot) as f64 } else { 1.0 })
}

/// Parity map image and legend.
pub fn parity_image(par: &[i8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    use crate::annotate::{with_legend, Key, Legend};
    let cols = [[230u8, 120, 40], [18, 18, 20], [70, 130, 200]];
    let mut rgba = vec![255u8; w * h * 4];
    for (q, &p) in par.iter().enumerate() {
        rgba[q * 4..q * 4 + 3].copy_from_slice(&cols[(p + 1) as usize]);
    }
    with_legend(&rgba, w, h, &Legend {
        title: "DIAGNOSTIC: IMAGE PARITY OF THE SCREEN-TO-SKY MAPPING".into(),
        key: Key::Categories(vec![("+1 sky as unlensed".into(), cols[2]), ("-1 sky mirrored".into(), cols[0]), ("not escaped (disk / captured)".into(), cols[1])]),
        note: "SIGN OF THE JACOBIAN FROM THREE RAYS PER PIXEL CENTRE".into(),
    })
}

/// Output class of a view: "diagnostic" or "presentation". Linear physical
/// data lives in the EXR and FITS files, never in a PNG.
pub fn view_class(view: &str) -> &'static str {
    match view {
        "sensor" | "electrons" | "radiance" | "band" => "presentation",
        _ => "diagnostic",
    }
}

/// Assign filters to display colours: by name for standard sets, else by
/// effective wavelength (longest → red).
pub fn palette(ins: &Instrument) -> Vec<[f64; 3]> {
    let names: Vec<String> = ins.filters.iter().map(|f| f.name.to_uppercase()).collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if names.len() == 1 {
        return vec![[1.0, 1.0, 1.0]];
    }
    if has("HA") && has("OIII") && has("SII") {
        // SHO "Hubble palette": SII red, Hα green, OIII blue
        return names.iter().map(|n| match n.as_str() { "SII" => [1.0, 0.0, 0.0], "HA" => [0.0, 1.0, 0.0], "OIII" => [0.0, 0.0, 1.0], _ => [0.3, 0.3, 0.3] }).collect();
    }
    if has("HA") && has("OIII") && names.len() == 2 {
        return names.iter().map(|n| if n == "HA" { [1.0, 0.0, 0.0] } else { [0.0, 0.85, 1.0] }).collect();
    }
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by(|&a, &b| ins.filters[b].lambda_eff.partial_cmp(&ins.filters[a].lambda_eff).unwrap());
    let mut pal = vec![[0.0; 3]; names.len()];
    let k = order.len();
    for (rank, &i) in order.iter().enumerate() {
        if names[i] == "L" { pal[i] = [0.0; 3]; continue; }
        let t = if k > 1 { rank as f64 / (k - 1) as f64 } else { 0.5 };
        pal[i] = [(1.0 - 2.0 * t).max(0.0), 1.0 - (2.0 * t - 1.0).abs(), (2.0 * t - 1.0).max(0.0)];
    }
    pal
}

/// Per-filter factors that make a 5800 K blackbody give equal signal in every
/// filter (white balance on a G2V star).
pub fn white_balance(ins: &Instrument) -> Vec<f64> {
    let r: Vec<f64> = ins.filters.iter().map(|f| {
        (0..crate::spectrum::NB).map(|c| f.w[c] * crate::spectrum::planck_photon(crate::spectrum::bin_center(c), 5800.0)).sum::<f64>()
    }).collect();
    let mx = r.iter().cloned().fold(0.0, f64::max).max(1e-300);
    r.iter().map(|v| if *v > 0.0 { mx / v } else { 0.0 }).collect()
}

/// Human-readable filter → display-colour mapping, flagging false colour.
pub fn palette_description(ins: &Instrument) -> (String, Vec<(String, [u8; 3])>) {
    let pal = palette(ins);
    let names: Vec<String> = ins.filters.iter().map(|f| f.name.to_uppercase()).collect();
    let narrow = names.iter().any(|n| n == "HA" || n == "OIII" || n == "SII");
    let kind = if names.len() == 1 {
        format!("MONOCHROME, {} FILTER", names[0])
    } else if narrow && names.len() == 3 && names.iter().all(|n| n == "HA" || n == "OIII" || n == "SII") {
        "FALSE COLOUR (SHO / 'HUBBLE' PALETTE): EMISSION LINES MAPPED TO R, G, B".into()
    } else if narrow {
        "FALSE COLOUR NARROWBAND COMPOSITE".into()
    } else if names.iter().any(|n| n == "L") {
        "LRGB: LUMINANCE FROM L, CHROMA FROM R, G, B (G2V WHITE BALANCE)".into()
    } else {
        "BROADBAND RGB, G2V WHITE BALANCE, COLOUR-PRESERVING STRETCH".into()
    };
    let keys = ins.filters.iter().zip(&pal).map(|(f, c)| {
        let c8 = c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
        (format!("{} -> {}", f.name, if names.len() == 1 { "GREY".into() } else { match (c[0] > 0.5, c[1] > 0.5, c[2] > 0.5) { (true, false, false) => "RED".to_string(), (false, true, false) => "GREEN".into(), (false, false, true) => "BLUE".into(), (false, true, true) => "CYAN".into(), _ => "LUMINANCE".into() } }), if c8 == [0, 0, 0] { [200, 200, 200] } else { c8 })
    }).collect();
    (kind, keys)
}

#[derive(Clone)]
pub struct DisplayParams {
    pub view: String,
    pub band: usize,
    /// Stretch: white point as a percentile of the image, and softness.
    pub white_pct: f64,
    pub black_pct: f64,
    pub soft: f64,
    pub gain: f64,
    /// linear | asinh | filmic | narrowband_sho | hdr_log
    pub transform: String,
    /// Shoulder start as a fraction of white (0 = hard clip).
    pub rolloff: f64,
    /// Per-channel black points (neutral background).
    pub neutralize: bool,
    pub hdr_decades: f64,
    /// hdr_log only, filled from the image: the empty intensity range
    /// (fractions of white) that the curve removes, and the black floor.
    pub gap: Option<(f64, f64)>,
    pub floor: f64,
}

impl Default for DisplayParams {
    fn default() -> Self {
        DisplayParams { view: "sensor".into(), band: 0, white_pct: 0.99995, black_pct: 0.02, soft: 0.08, gain: 1.0, transform: "asinh".into(), rolloff: 0.0, neutralize: false, hdr_decades: 8.0, gap: None, floor: 0.0 }
    }
}

impl DisplayParams {
    /// The scene's recorded presentation parameters.
    pub fn from_scene(s: &crate::scene::Scene) -> DisplayParams {
        DisplayParams {
            white_pct: s.white_point_percentile,
            black_pct: s.black_point_percentile,
            soft: s.display_softness,
            transform: s.display_transform.clone(),
            rolloff: s.highlight_rolloff,
            neutralize: s.background_neutralization,
            hdr_decades: s.hdr_decades,
            ..Default::default()
        }
    }

    /// One-line description for PNG metadata and captions.
    pub fn describe(&self) -> String {
        format!(
            "{} transform; white point at the {} quantile; black point at the {} quantile{}; softness {}; highlight shoulder {}{}",
            self.transform, self.white_pct, self.black_pct, if self.neutralize { " per channel (background neutralized)" } else { "" },
            self.soft, if self.rolloff > 0.0 { format!("from {}", self.rolloff) } else { "off".into() },
            if self.transform == "hdr_log" { format!("; log segment {} decades below the knee", self.hdr_decades) } else { String::new() }
        )
    }
}

/// Map x = value / white (≥ 0) to display [0, ~1] with the chosen transform.
fn tone(x: f64, p: &DisplayParams) -> f64 {
    let x = x.max(0.0);
    match p.transform.as_str() {
        "linear" => x,
        "filmic" => {
            // ACES-style filmic curve (Narkowicz fit), scaled so x = 1 maps near 0.8
            let y = 2.2 * x;
            (y * (2.51 * y + 0.03)) / (y * (2.43 * y + 0.59) + 0.14)
        }
        "hdr_log" => {
            // One global, monotone curve for registered HDR composites: a
            // logarithmic segment from the floor to the bottom of the largest
            // empty range of intensities (stars, sky), that empty range squeezed
            // to a sliver, and a linear segment from its top to white, so the
            // brightest structure (the disk) keeps its gradients.
            let (glo, ghi) = p.gap.unwrap_or((0.1, 0.1));
            let floor = p.floor.max(glo * 10f64.powf(-p.hdr_decades)).min(glo * 0.999);
            // the disk segment is linear in *displayed* brightness (sRGB), so
            // its radial and Doppler gradients read as such
            let disp = |y: f64| y.max(0.0).powf(2.2);
            if x >= ghi {
                disp(0.43 + 0.57 * ((x - ghi) / (1.0 - ghi).max(1e-12)).min(1.2))
            } else if x > glo {
                disp(0.40 + 0.03 * (x / glo).ln() / (ghi / glo).ln())
            } else {
                // stars and sky: logarithmic, lifted at the faint end (power 0.35)
                disp(0.40 * ((x.max(floor) / floor).ln() / (glo / floor).ln()).powf(0.35))
            }
        }
        _ => (x / p.soft).asinh() / (1.0 / p.soft).asinh(),
    }
}

/// FWHM of the PSF core in pixels for filter `fi`: optics/seeing blur given on
/// the sky (arcsec) plus the Airy core 1.03 λ/D, both at this render's scale.
pub fn psf_core_fwhm_px(ins: &Instrument, fi: usize) -> f64 {
    let s = &ins.scene;
    let px_rad = s.pixel_scale();
    let airy = 1.03 * ins.filters[fi].lambda_eff * 1e-9 / s.aperture() / px_rad;
    let optics_px = s.optics_fwhm_arcsec / (px_rad.to_degrees() * 3600.0);
    (optics_px.powi(2) + airy * airy).sqrt()
}

/// For hdr_log: find the widest empty range of log intensity (0.1-decade bins
/// holding < 10⁻⁵ of the pixels, at least one decade wide) and the black floor.
fn hdr_params(p: &DisplayParams, inten: &[f32], white: f64) -> DisplayParams {
    let mut q = p.clone();
    if p.transform != "hdr_log" { return q; }
    let pos: Vec<f32> = inten.iter().cloned().filter(|&v| v > 0.0).collect();
    if pos.is_empty() { return q; }
    q.floor = percentile(&pos, p.black_pct.max(0.5)) / white;
    let lo = (q.floor.max(1e-30)).log10();
    let nb = (((0.0 - lo) / 0.1).ceil() as usize).clamp(1, 400);
    let mut hist = vec![0usize; nb];
    for &v in &pos {
        let l = (v as f64 / white).max(1e-300).log10();
        if l >= lo && l < 0.0 { hist[(((l - lo) / 0.1) as usize).min(nb - 1)] += 1; }
    }
    let thresh = (pos.len() as f64 * 1e-5).max(0.5);
    let (mut best, mut run_start) = ((0usize, 0usize), None);
    for i in 0..=nb {
        let empty = i < nb && (hist[i] as f64) < thresh;
        match (empty, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(a)) => { if i - a > best.1 - best.0 { best = (a, i); } run_start = None; }
            _ => {}
        }
    }
    if best.1 - best.0 >= 10 {
        q.gap = Some((10f64.powf(lo + 0.1 * best.0 as f64), 10f64.powf(lo + 0.1 * best.1 as f64)));
    } else {
        q.gap = Some((0.1, 0.1));
    }
    q
}

/// Smooth shoulder: identity below k, tanh roll-off to 1 above it.
fn shoulder(v: f64, k: f64) -> f64 {
    if k <= 0.0 || v <= k { v.min(if k <= 0.0 { 1.0 } else { f64::INFINITY }) } else { k + (1.0 - k) * ((v - k) / (1.0 - k)).tanh() }
}

/// Render a view to RGBA8.
pub fn display(ins: &Instrument, fr: &Frame, ro: &Readout, p: &DisplayParams) -> Vec<u8> {
    let (w, h) = (fr.w, fr.h);
    let npx = w * h;
    let n = fr.spp.max(1) as f64;
    let mut rgb = vec![[0f64; 3]; npx];
    let diag = |px: usize, k: usize| fr.diag[px * ND + k] as f64;
    match p.view.as_str() {
        "sensor" | "electrons" if !ro.electrons.is_empty() => {
            let pal = palette(ins);
            let names: Vec<String> = ins.filters.iter().map(|f| f.name.to_uppercase()).collect();
            let narrow = names.iter().any(|n| n == "HA" || n == "OIII" || n == "SII");
            if ro.electrons.len() == 1 {
                let e = &ro.electrons[0];
                let black = percentile(e, p.black_pct);
                let white = (percentile(e, p.white_pct) - black).max(1.0) / p.gain;
                for q in 0..npx { let v = shoulder(tone((e[q] as f64 - black) / white, p), p.rolloff); rgb[q] = [v, v, v]; }
            } else {
                // Colour-preserving stretch (Lupton et al. 2004): the intensity is
                // tone-mapped and every channel keeps its ratio to it. Broadband
                // channels are white-balanced on a G2V star; narrowband (SHO)
                // channels are each normalised to their own white point — the false
                // colour convention — but share one stretch, so hue follows the
                // line ratios instead of saturating channel by channel.
                let wb = if narrow { vec![1.0; names.len()] } else { white_balance(ins) };
                let lum_idx = names.iter().position(|n| n == "L");
                let chans: Vec<usize> = (0..names.len()).filter(|&i| Some(i) != lum_idx).collect();
                let common_black = {
                    let mean: Vec<f32> = (0..npx).map(|q| (chans.iter().map(|&c| ro.electrons[c][q] as f64 * wb[c]).sum::<f64>() / chans.len() as f64) as f32).collect();
                    percentile(&mean, p.black_pct)
                };
                let bal: Vec<Vec<f64>> = ro.electrons.iter().enumerate().map(|(fi, e)| {
                    let bl = if p.neutralize || narrow { percentile(e, p.black_pct) * wb[fi] } else { common_black };
                    let mut v: Vec<f64> = e.iter().map(|&x| (x as f64 * wb[fi] - bl).max(0.0)).collect();
                    if narrow {
                        let f: Vec<f32> = v.iter().map(|&x| x as f32).collect();
                        let wn = percentile(&f, p.white_pct).max(1e-12);
                        for x in v.iter_mut() { *x /= wn; }
                    }
                    v
                }).collect();
                let inten: Vec<f32> = (0..npx).map(|q| match lum_idx {
                    Some(li) => bal[li][q] as f32,
                    None => (chans.iter().map(|&c| bal[c][q]).sum::<f64>() / chans.len() as f64) as f32,
                }).collect();
                let white = percentile(&inten, p.white_pct).max(1e-12) / p.gain;
                let p = &hdr_params(p, &inten, white);
                let desat = if narrow { 0.55 } else { 1.0 };
                for q in 0..npx {
                    let i = inten[q] as f64;
                    if i <= 0.0 { continue; }
                    let y = shoulder(tone(i / white, p), p.rolloff);
                    let mean = chans.iter().map(|&c| bal[c][q]).sum::<f64>() / chans.len() as f64;
                    let mut c3 = [0.0; 3];
                    for &fi in &chans {
                        let ratio = if mean > 0.0 { bal[fi][q] / mean } else { 1.0 };
                        let r = desat * ratio + (1.0 - desat);
                        for c in 0..3 { c3[c] += r * pal[fi][c]; }
                    }
                    // normalise so the channel mean equals the tone-mapped intensity
                    let m = (c3[0] + c3[1] + c3[2]) / 3.0;
                    for c in 0..3 { rgb[q][c] = if m > 0.0 { c3[c] / m * y } else { y }; }
                }
                // keep hue when a channel exceeds 1
                for q in 0..npx { let m = rgb[q][0].max(rgb[q][1]).max(rgb[q][2]); if m > 1.0 { for c in 0..3 { rgb[q][c] /= m; } } }
            }
        }
        "radiance" => {
            let mut lum = vec![0f32; npx];
            for q in 0..npx {
                let c3 = [fr.truecol[q][0] as f64, fr.truecol[q][1] as f64, fr.truecol[q][2] as f64];
                rgb[q] = c3;
                lum[q] = (0.2126 * c3[0] + 0.7152 * c3[1] + 0.0722 * c3[2]) as f32;
            }
            let white = percentile(&lum, p.white_pct).max(1e-30) / p.gain;
            let black = if p.neutralize { percentile(&lum, p.black_pct) } else { 0.0 };
            for q in 0..npx {
                let l = 0.2126 * rgb[q][0] + 0.7152 * rgb[q][1] + 0.0722 * rgb[q][2];
                let s = if l > black { shoulder(tone((l - black) / (white - black).max(1e-30), p), p.rolloff) / l } else { 0.0 };
                for c in 0..3 { rgb[q][c] = (rgb[q][c] * s).max(0.0); }
            }
        }
        "band" => {
            let fi = p.band.min(ro.expected.len().saturating_sub(1));
            if let Some(e) = ro.expected.get(fi) {
                let white = percentile(e, p.white_pct).max(1e-30) / p.gain;
                for q in 0..npx { let v = asinh_stretch(e[q] as f64, 0.0, white, p.soft); rgb[q] = [v, v, v]; }
            }
        }
        "variance" => {
            for q in 0..npx {
                let rel = fr.rel_err[q] as f64;
                rgb[q] = if rel > 0.0 { lin(viridis((rel.log10() + 4.0) / 4.0)) } else { [0.0; 3] };
            }
        }
        "steps" => { for q in 0..npx { rgb[q] = lin(viridis(diag(q, D_STEPS) / n / STEPS_MAX)); } }
        "invariant" => {
            for q in 0..npx {
                let k = diag(q, D_ESC) + diag(q, D_CAPT) + diag(q, D_GN) + diag(q, D_UNRES) + diag(q, D_FAIL);
                let v = diag(q, D_INV) / n;
                rgb[q] = if k > 0.0 { lin(viridis((v - INV_LO) / (INV_HI - INV_LO))) } else { [0.0; 3] };
            }
        }
        "tau" => {
            for q in 0..npx {
                let t = diag(q, D_TAU) / n;
                rgb[q] = if t > 0.0 { lin(viridis((t.log10() - TAU_LO) / (TAU_HI - TAU_LO))) } else { [0.0; 3] };
            }
        }
        "nebula" => {
            for q in 0..npx {
                let v = diag(q, D_NEB) / n;
                rgb[q] = if v > 0.0 { lin(viridis((v.log10() - NEB_LO) / (NEB_HI - NEB_LO))) } else { [0.0; 3] };
            }
        }
        "winding" => { for q in 0..npx { rgb[q] = lin(viridis(diag(q, D_WIND) / n / WIND_MAX)); } }
        "redshift" => {
            for q in 0..npx {
                let k = diag(q, D_GN);
                // blueshift (g > 1) blue, redshift red
                rgb[q] = if k > 0.0 { lin(coolwarm(1.0 - (diag(q, D_G) / k - G_LO) / (G_HI - G_LO))) } else { lin8([40, 40, 44]) };
            }
        }
        "fate" => {
            for q in 0..npx {
                let (k, conf) = fate_majority(&fr.diag[q * ND..(q + 1) * ND]);
                let mut c = lin8(FATE_CLASSES[k].1);
                // boundary-confidence overlay: hatch pixels without a ≥ 95% majority
                let (x, y) = (q % w, q / w);
                if conf < 0.95 && (x + y) % 3 == 0 { c = c.map(|v| v * 0.25 + 0.2); }
                rgb[q] = c;
            }
        }
        "correspondence" => {
            // debug: grey visible-light image with the volume landmarks each
            // pixel's rays pass (majority id) painted in their colours
            let lum: Vec<f32> = fr.truecol.iter().map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).collect();
            let white = percentile(&lum, 0.999).max(1e-30);
            for q in 0..npx {
                let g = asinh_stretch(lum[q] as f64, 0.0, white, 0.1) * 0.6;
                rgb[q] = lin([g, g, g]);
                let (s_, c_) = (diag(q, D_LMK), diag(q, D_LMKN));
                if c_ > 0.15 * n {
                    let id = ((s_ / c_).round() as usize).clamp(1, LANDMARK_COLOURS.len()) - 1;
                    rgb[q] = lin8(LANDMARK_COLOURS[id]);
                }
            }
        }
        "noise" => {
            // realised − expected electrons in units of the expected σ
            if let (Some(e), Some(x)) = (ro.electrons.first(), ro.expected.first()) {
                let rn2 = (ins.scene.read_noise_e * ins.scene.read_noise_e) as f32;
                for q in 0..npx {
                    let z = (e[q] - x[q]) / (x[q] + rn2).max(1e-6).sqrt();
                    let v = (0.5 + z as f64 / 6.0).clamp(0.0, 1.0);
                    rgb[q] = lin([v, v, v]);
                }
            }
        }
        _ => {}
    }
    let mut out = vec![0u8; npx * 4];
    for q in 0..npx {
        for c in 0..3 {
            let v = rgb[q][c].clamp(0.0, 1.0);
            let srgb = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
            out[q * 4 + c] = (srgb * 255.0 + 0.5) as u8;
        }
        out[q * 4 + 3] = 255;
    }
    out
}

// --------------------------------------------------------- metadata --
pub fn metadata(ins: &Instrument, fr: &Frame, ro: &Readout, conv: &Convergence, elapsed_s: f64, volume_id: &str) -> Json {
    let s = &ins.scene;
    let mut j = Json::obj();
    j.set("project", "Schwarzschild Observatory")
        .set("renderer_version", crate::VERSION)
        .set("synthetic", "Synthetic instrument simulation of a statistically synthesized scene; not a real observation.")
        .set("seed", s.seed)
        .set("width", fr.w).set("height", fr.h)
        .set("samples_per_pixel", fr.spp)
        .set("exposure_s", ro.t)
        .set("filters", ro.filters.iter().map(|f| Json::from(f.as_str())).collect::<Vec<_>>())
        .set("render_wall_time_s", elapsed_s)
        .set("volume_cache", volume_id)
        .set("rtol", s.rtol).set("max_windings", s.max_windings)
        .set("pixel_scale_arcsec", s.pixel_scale().to_degrees() * 3600.0)
        .set("focal_length_mm", s.focal_length_mm())
        .set("aperture_m", s.aperture())
        .set("g_sky", ins.g_sky)
        .set("render_noise_median_rel", conv.render_noise)
        .set("sensor_noise_median_rel", conv.sensor_noise)
        .set("failed_fraction", conv.failed_fraction)
        .set("mean_geodesic_steps", conv.mean_steps)
        .set("mean_volume_steps", conv.mean_vol_steps)
        .set("saturated_fraction", ro.saturated)
        .set("scene", s.to_json());
    j
}

pub fn fits_cards(ins: &Instrument, fr: &Frame, ro: &Readout, conv: &Convergence, date: &str, volume_id: &str) -> Vec<crate::export::Card> {
    use crate::export::Card::*;
    let s = &ins.scene;
    let mut c = vec![
        Str("BUNIT", "electron".into(), "detected photo-electrons"),
        Str("ORIGIN", "Schwarzschild Observatory".into(), "synthetic instrument"),
        Str("TELESCOP", "synthetic".into(), ""),
        Str("INSTRUME", "Schwarzschild Observatory sensor".into(), ""),
        Str("OBJECT", s.name.clone(), ""),
        Str("SWVERSN", crate::VERSION.into(), "renderer version"),
        Str("DATE", date.into(), "render date (UTC)"),
        Int("SEED", s.seed as i64, "scene seed"),
        Int("NSAMPLE", fr.spp as i64, "Monte-Carlo samples per pixel"),
        Float("EXPTIME", ro.t, "[s] simulated exposure"),
        Float("APERTURE", s.aperture(), "[m] aperture diameter"),
        Float("FOCALLEN", s.focal_length_mm(), "[mm] focal length"),
        Float("PIXSCALE", s.pixel_scale().to_degrees() * 3600.0, "[arcsec/pixel]"),
        Float("XPIXSZ", s.pixel_um, "[um] pixel pitch"),
        Float("EGAIN", s.gain_e_per_adu, "[e-/ADU]"),
        Float("RDNOISE", s.read_noise_e, "[e-] read noise"),
        Float("DARKCUR", s.dark_e_per_s, "[e-/s] dark current"),
        Float("FULLWELL", s.full_well_e, "[e-] full well"),
        Str("SCNMODE", s.mode.clone(), "telescope (distant) or local observer"),
        Str("OUTCLASS", s.output_class.clone(), "fixture/validation/presentation/high_quality"),
        Str("PRESMODE", s.presentation_mode.clone(), "single exposure, HDR composite or stack"),
        Bool("COMPOSIT", ro.composite, "T: merged registered frames, not one frame"),
        Int("NFRAMES", ro.frames.iter().map(|f| f.2 as i64).sum(), "registered frames merged"),
        Str("BUNIT", if ro.composite { "electron/s".into() } else { "electron".into() }, "pixel unit (composite: calibrated rate)"),
        Float("BH_MASS", s.bh_mass_msun, "[Msun] black-hole mass"),
        Float("BH_DIST", s.observer_distance_m() / crate::scene::PC_M, "[pc] observer-hole distance"),
        Float("CAM_R", s.cam_r, "[M] observer radius (static observer)"),
        Float("SHADOW", s.shadow_radius_rad().to_degrees() * 3600.0, "[arcsec] shadow angular radius"),
        Bool("LENSING", s.lensing, "geodesics on (F: straight-ray comparison)"),
        Float("CAM_INCL", s.cam_incl_deg, "[deg] camera inclination"),
        Float("FOV", s.fov_deg, "[deg] horizontal field of view"),
        Bool("DISK", s.disk, "accretion disk on"),
        Float("DISK_RIN", s.disk_r_in, "[M]"),
        Float("DISK_ROU", s.disk_r_out, "[M]"),
        Float("DISK_EDD", s.disk_eddington_ratio, "accretion rate L/L_Edd"),
        Float("DISK_FCO", s.disk_color_correction, "colour-correction factor"),
        Float("DISK_TAU", s.disk_tau, "vertical optical depth (<=0: thick)"),
        Float("DISK_SCL", s.disk_brightness_scale, "artistic radiance multiplier"),
        Float("ND", s.neutral_density, "neutral-density filter optical density"),
        Int("DISKORD", s.disk_max_order as i64, "highest photon order shaded"),
        Float("RTOL", s.rtol, "geodesic relative tolerance"),
        Float("WINDMAX", s.max_windings, "max photon windings"),
        Float("MCNOISE", conv.render_noise, "median rel. Monte-Carlo error"),
        Float("FAILFRAC", conv.failed_fraction, "geodesic failure fraction"),
        Str("VOLCACHE", volume_id.into(), "volume cache identity"),
    ];
    for (i, f) in ro.filters.iter().enumerate() {
        let k: &'static str = ["FILTER1", "FILTER2", "FILTER3", "FILTER4", "FILTER5", "FILTER6"].get(i).copied().unwrap_or("FILTERX");
        c.push(Str(k, f.clone(), "plane filter"));
    }
    c.push(Comment("Synthetic instrument simulation of a statistically synthesized scene.".into()));
    c.push(Comment("Not a real observation. See the JSON sidecar for the full scene.".into()));
    c
}

// -------------------------------------------------------- checkpoint --
pub fn checkpoint_bytes(scene_json: &Json, acc: &Accum) -> Vec<u8> {
    let sj = scene_json.dump();
    let mut b = Vec::with_capacity(32 + sj.len() + acc.sum.len() * 4 + acc.lsum.len() * 16 + acc.diag.len() * 4);
    b.extend_from_slice(b"OBSCKPT2");
    b.extend_from_slice(&(sj.len() as u32).to_le_bytes());
    b.extend_from_slice(sj.as_bytes());
    for v in [acc.w as u32, acc.h as u32, acc.spp] { b.extend_from_slice(&v.to_le_bytes()); }
    for v in &acc.sum { b.extend_from_slice(&v.to_le_bytes()); }
    for v in &acc.lsum { b.extend_from_slice(&v.to_le_bytes()); }
    for v in &acc.lsq { b.extend_from_slice(&v.to_le_bytes()); }
    for v in &acc.diag { b.extend_from_slice(&v.to_le_bytes()); }
    b
}

pub fn checkpoint_load(b: &[u8]) -> Result<(Json, Accum), String> {
    if b.len() < 12 || &b[..8] != b"OBSCKPT2" { return Err("not an OBSCKPT2 checkpoint".into()); }
    let mut i = 8;
    let u32_at = |i: &mut usize| { let v = u32::from_le_bytes(b[*i..*i + 4].try_into().unwrap()); *i += 4; v };
    let n = u32_at(&mut i) as usize;
    let scene = Json::parse(std::str::from_utf8(&b[i..i + n]).map_err(|_| "utf8")?)?;
    i += n;
    let (w, h, spp) = (u32_at(&mut i) as usize, u32_at(&mut i) as usize, u32_at(&mut i));
    let mut acc = Accum::new(w, h);
    acc.spp = spp;
    let need = acc.sum.len() * 4 + acc.lsum.len() * 16 + acc.diag.len() * 4;
    if b.len() < i + need { return Err("truncated checkpoint".into()); }
    for v in acc.sum.iter_mut() { *v = f32::from_le_bytes(b[i..i + 4].try_into().unwrap()); i += 4; }
    for v in acc.lsum.iter_mut() { *v = f64::from_le_bytes(b[i..i + 8].try_into().unwrap()); i += 8; }
    for v in acc.lsq.iter_mut() { *v = f64::from_le_bytes(b[i..i + 8].try_into().unwrap()); i += 8; }
    for v in acc.diag.iter_mut() { *v = f32::from_le_bytes(b[i..i + 4].try_into().unwrap()); i += 4; }
    Ok((scene, acc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_inversion_statistics() {
        for &m in &[3.0, 50.0, 4000.0] {
            let n = 100_000;
            let (mut s, mut s2) = (0.0, 0.0);
            let mut r = Rng::new(5, 1);
            for _ in 0..n {
                let k = poisson_inv(m, r.f64());
                s += k;
                s2 += k * k;
            }
            let mean = s / n as f64;
            let var = s2 / n as f64 - mean * mean;
            assert!((mean / m - 1.0).abs() < 0.01, "{m}: mean {mean}");
            assert!((var / m - 1.0).abs() < 0.03, "{m}: var {var}"); // σ ∝ √N
        }
    }

    #[test]
    fn frozen_noise_is_monotone_in_exposure() {
        // the same uniform gives a non-decreasing count as the mean grows
        let u = 0.37;
        let mut prev = -1.0;
        for k in 1..400 {
            let v = poisson_inv(k as f64 * 0.5, u);
            assert!(v >= prev);
            prev = v;
        }
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::render::{render_rows, Accum, Ctx};
    use crate::scene::Scene;

    fn tiny(filters: &[&str]) -> Scene {
        Scene {
            width: 24, height: 16, fov_deg: 40.0, star_mag_limit: 6.0, galaxies: false, stars: true, diffuse: true,
            filters: filters.iter().map(|s| s.to_string()).collect(), rtol: 1e-7, target_spp: 64, ..Default::default()
        }
    }

    fn render(ctx: &Ctx, acc: &mut Accum, add: u32) {
        let (w, h) = (acc.w, acc.h);
        render_rows(ctx, 0, h, acc.spp, add, &mut acc.sum, &mut acc.lsum, &mut acc.lsq, &mut acc.diag[..w * h * ND]);
        acc.spp += add;
    }

    #[test]
    fn accumulation_is_deterministic_resumable_and_converges() {
        // a dimmed disk: at physical brightness the disk outshines the sky by
        // 10¹⁵, and sky pixels touched by one disk sample stay in the rare-event
        // regime where the error does not yet fall as 1/√N
        let ctx = Ctx::new(Scene { disk_brightness_scale: 1e-15, ..tiny(&["L"]) }, None, &mut |_| {});
        let ins = Instrument::from_ctx(&ctx);
        let mut a = Accum::new(24, 16);
        render(&ctx, &mut a, 8);
        render(&ctx, &mut a, 8);
        let mut b = Accum::new(24, 16);
        render(&ctx, &mut b, 16);
        assert_eq!(a.sum, b.sum, "8+8 samples must equal 16 in one pass");
        // checkpoint round trip, then resume
        let (_, mut c) = checkpoint_load(&checkpoint_bytes(&Json::obj(), &a)).unwrap();
        assert_eq!((c.spp, &c.sum, &c.diag), (a.spp, &a.sum, &a.diag));
        render(&ctx, &mut c, 48);
        render(&ctx, &mut b, 48);
        assert_eq!(c.sum, b.sum, "resumed render must equal an uninterrupted one");
        // MC error falls roughly as 1/√N: 16 → 64 samples halves it
        let e16 = { let mut x = Accum::new(24, 16); render(&ctx, &mut x, 16); convergence(&ins, &frame(&ins, &x), None).render_noise };
        let e64 = convergence(&ins, &frame(&ins, &b), None).render_noise;
        assert!(e64 < e16 * 0.7 && e64 > e16 * 0.3, "{e16} → {e64}");
        // reset clears everything
        b.reset();
        assert!(b.spp == 0 && b.sum.iter().all(|&v| v == 0.0) && b.lsum.iter().all(|&v| v == 0.0) && b.diag.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn psf_conserves_flux_and_noise_free_readout_is_finite() {
        // a point source in the middle of a 96² frame: PSF core + halo + spikes
        // must redistribute, not create or destroy, electrons
        let mut sc = tiny(&["V", "Ha"]);
        sc.width = 96; sc.height = 96; sc.sensor_noise = false; sc.dark_e_per_s = 0.0;
        sc.halo_fraction = 0.05; sc.spider_vanes = 4; sc.target_spp = 1;
        let ins = Instrument::new(sc.clone());
        let n = 96 * 96;
        let mut fr = Frame { w: 96, h: 96, spp: 1, rates: vec![vec![0.0; n]; 2], truecol: vec![[0.0; 3]; n], lmean: vec![0.0; n], rel_err: vec![0.0; n], diag: vec![0.0; n * ND] };
        for r in fr.rates.iter_mut() { r[48 * 96 + 48] = 1000.0; }
        let ro = readout(&ins, &fr);
        for e in &ro.expected {
            let tot: f64 = e.iter().map(|&v| v as f64).sum();
            assert!((tot / (1000.0 * ro.t) - 1.0).abs() < 2e-3, "PSF total {tot}");
            assert!(e.iter().all(|v| v.is_finite() && *v >= 0.0));
            assert!(e[48 * 96 + 48] < 1000.0 * ro.t as f32, "the PSF spreads the point");
        }
    }

    #[test]
    fn sensor_is_linear_in_exposure_and_dark_frames_work() {
        let mut sc = tiny(&["L"]);
        sc.sensor_noise = false;
        sc.psf = false;
        sc.dark_e_per_s = 0.0;
        sc.full_well_e = 1e12;
        let ctx = Ctx::new(sc.clone(), None, &mut |_| {});
        let mut acc = Accum::new(24, 16);
        render(&ctx, &mut acc, 16);
        let mut s1 = sc.clone(); s1.exposure_s = 10.0; s1.target_spp = 16;
        let mut s2 = sc.clone(); s2.exposure_s = 40.0; s2.target_spp = 16;
        let i1 = Instrument::new(s1.clone());
        let i2 = Instrument::new(s2.clone());
        let r1 = readout(&i1, &frame(&i1, &acc));
        let r2 = readout(&i2, &frame(&i2, &acc));
        for (a, b) in r1.expected[0].iter().zip(&r2.expected[0]) {
            assert!((b - 4.0 * a).abs() <= 1e-4 * b.abs() + 1e-6);
        }
        // shutter closed: only dark current (+ read noise when enabled)
        let mut d = s2.clone(); d.shutter_open = false; d.dark_e_per_s = 0.5;
        let idk = Instrument::new(d);
        let rd = readout(&idk, &frame(&idk, &acc));
        assert!(rd.expected[0].iter().all(|&v| (v - 20.0).abs() < 1e-3)); // 0.5 e⁻/s × 40 s
    }

    #[test]
    fn filters_act_on_the_spectrum_before_colour() {
        let ctx = Ctx::new(tiny(&["Blue", "Red", "Ha"]), None, &mut |_| {});
        let ins = Instrument::from_ctx(&ctx);
        let mut acc = Accum::new(24, 16);
        render(&ctx, &mut acc, 8);
        let fr = frame(&ins, &acc);
        let tot = |k: usize| fr.rates[k].iter().map(|&v| v as f64).sum::<f64>();
        let (b, r, ha) = (tot(0), tot(1), tot(2));
        assert!(b > 0.0 && r > 0.0 && ha > 0.0);
        assert!((b / r - 1.0).abs() > 0.05, "broadbands must see different light");
        assert!(ha < r * 0.15, "a 5 nm filter passes a small fraction of the red continuum");
    }

    #[test]
    fn exr_preserves_linear_floats() {
        let w = 3usize;
        let h = 2usize;
        let vals: Vec<f32> = vec![1.0e-12, 3.5, 6.0e9, 0.0, -2.25, 1.0 / 3.0];
        let e = crate::export::exr(w, h, &[("Y", &vals)]);
        // after the header + offset table, each scanline is: y (i32), size (u32), floats
        let mut found = Vec::new();
        let tbl = e.len() - h * (8 + w * 4) - 8 * h;
        for y in 0..h {
            let off = u64::from_le_bytes(e[tbl + 8 * y..tbl + 8 * y + 8].try_into().unwrap()) as usize;
            for x in 0..w {
                let p = off + 8 + 4 * x;
                found.push(f32::from_le_bytes(e[p..p + 4].try_into().unwrap()));
            }
        }
        assert_eq!(found, vals);
    }

    fn telescope_disk(w: usize, h: usize) -> Scene {
        let mut sc = Scene {
            mode: "telescope".into(), bh_mass_msun: 4.3e6, width: w, height: h, fov_deg: 0.8, cam_incl_deg: 75.0,
            disk: true, disk_r_out: 30.0, sky_mode: "none".into(), filters: vec!["V".into()], rtol: 1e-8,
            aperture_m: 0.1, pixel_um: 9.0, exposure_s: 2e-5, neutral_density: 6.0, sensor_noise: false, target_spp: 2, ..Default::default()
        };
        sc.bh_distance_pc = 2e4 * sc.r_g_m() / crate::scene::PC_M;
        sc.finalize();
        sc
    }

    #[test]
    fn resolution_preserves_angles_flux_and_psf_and_reveals_detail() {
        // the same field at 640×400, 1024×640 and 1600×1000
        let mut prev: Option<(f64, f64, f64, f64)> = None;
        for (w, h) in [(640, 400), (1024, 640), (1600, 1000)] {
            let ctx = Ctx::new(telescope_disk(w, h), None, &mut |_| {});
            let ins = Instrument::from_ctx(&ctx);
            let mut acc = Accum::new(w, h);
            crate::render::render_pass(&ctx, &mut acc, 2, 8);
            let fr = frame(&ins, &acc);
            let ro = readout(&ins, &fr);
            let omega_px = ctx.scene.pixel_scale().powi(2);
            let n = (w * h) as f64;
            // shadow solid angle from the captured fraction
            let capt: f64 = (0..w * h).map(|p| fr.diag[p * ND + D_CAPT] as f64).sum::<f64>() / 2.0;
            let shadow_sr = capt * omega_px;
            // integrated detector electrons (PSF on): independent of pixel size
            let flux: f64 = ro.expected[0].iter().map(|&v| v as f64).sum();
            // PSF core in arcsec
            let psf = psf_core_fwhm_px(&ins, 0) * ctx.scene.pixel_scale().to_degrees() * 3600.0;
            // angular area of mixed-fate boundary pixels: shrinks with the pixel
            let mixed = (0..w * h).filter(|&p| fate_majority(&fr.diag[p * ND..(p + 1) * ND]).1 < 0.95).count() as f64 * omega_px;
            let _ = n;
            if let Some((s0, f0, p0, m0)) = prev {
                assert!((shadow_sr / s0 - 1.0).abs() < 0.03, "{w}: shadow {shadow_sr} vs {s0}");
                assert!((flux / f0 - 1.0).abs() < 0.01, "{w}: flux {flux} vs {f0}");
                assert!((psf / p0 - 1.0).abs() < 1e-9, "{w}: psf {psf}″ vs {p0}″");
                assert!(mixed < 0.85 * m0, "{w}: boundary area {mixed} did not shrink from {m0}");
            }
            prev = Some((shadow_sr, flux, psf, mixed));
        }
    }

    #[test]
    fn hdr_composite_recovers_rates_over_ten_decades() {
        let mut sc = tiny(&["L"]);
        sc.width = 64; sc.height = 1; sc.psf = false; sc.target_spp = 1; sc.full_well_e = 5e4; sc.read_noise_e = 3.0; sc.dark_e_per_s = 0.01;
        sc.exposures = vec![
            crate::scene::Exposure { exposure_s: 1e-5, neutral_density: 0.0, count: 1 },
            crate::scene::Exposure { exposure_s: 0.1, neutral_density: 0.0, count: 1 },
            crate::scene::Exposure { exposure_s: 300.0, neutral_density: 0.0, count: 1 },
        ];
        let ins = Instrument::new(sc);
        let n = 64;
        let truth: Vec<f32> = (0..n).map(|i| 10f32.powf(-1.0 + 10.0 * i as f32 / (n - 1) as f32)).collect();
        let fr = Frame { w: n, h: 1, spp: 1, rates: vec![truth.clone()], truecol: vec![[0.0; 3]; n], lmean: vec![0.0; n], rel_err: vec![0.0; n], diag: vec![0.0; n * ND] };
        let ro = readout(&ins, &fr);
        assert!(ro.composite && ro.saturated == 0.0);
        for (r, t) in ro.rate[0].iter().zip(&truth) {
            // error bound: shot + read noise of the longest unsaturated frame
            let t = *t as f64;
            let g = if t * 300.0 < 4.7e4 { 300.0 } else if t * 0.1 < 4.7e4 { 0.1 } else { 1e-5 };
            let sigma = (t * g + 9.0 + 3.0).sqrt() / g;
            assert!((*r as f64 - t).abs() < 6.0 * sigma + 1e-3, "rate {t}: got {r} (σ {sigma})");
        }
    }

    #[test]
    fn stacking_reduces_noise_as_root_n() {
        let mk = |count: u32| {
            let mut sc = tiny(&["L"]);
            sc.width = 4096; sc.height = 1; sc.psf = false; sc.target_spp = 1; sc.read_noise_e = 5.0;
            sc.exposures = vec![crate::scene::Exposure { exposure_s: 60.0, neutral_density: 0.0, count }];
            let ins = Instrument::new(sc);
            let fr = Frame { w: 4096, h: 1, spp: 1, rates: vec![vec![2.0; 4096]], truecol: vec![[0.0; 3]; 4096], lmean: vec![0.0; 4096], rel_err: vec![0.0; 4096], diag: vec![0.0; 4096 * ND] };
            let ro = readout(&ins, &fr);
            let m = ro.rate[0].iter().map(|&v| v as f64).sum::<f64>() / 4096.0;
            let var = ro.rate[0].iter().map(|&v| (v as f64 - m).powi(2)).sum::<f64>() / 4095.0;
            (m, var.sqrt())
        };
        let ((m2, s2), (m32, s32)) = (mk(2), mk(32));
        assert!((m2 / 2.0 - 1.0).abs() < 0.02 && (m32 / 2.0 - 1.0).abs() < 0.01, "means {m2} {m32}");
        assert!((s2 / s32 / 4.0 - 1.0).abs() < 0.1, "σ ratio {}", s2 / s32);
    }
}
