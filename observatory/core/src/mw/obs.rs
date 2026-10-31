//! Observing a generated scene with one instrument.
//!
//! For every band: per-voxel emissivity j_ν and absorption α_ν from the
//! physical state, a jittered ray march through the volume with exact transfer
//! for piecewise-constant j and α,
//!
//! ```text
//! I ← I e^(−Δτ) + (j/α)(1 − e^(−Δτ))
//! ```
//!
//! plus point and compact sources (attenuated by the same α along their own
//! sightline), analytic cluster atmospheres, the CMB and the lensing field.
//! Then the instrument: its PSF or beam, its units and its noise.

use super::bands::{Band, Instrument, Kind, Spec};
use super::phys::*;
use super::*;
use crate::math::V3;
use crate::rng::Rng;
use crate::sensor::fft_convolve;
use std::f64::consts::PI;

// --------------------------------------------------------------- view ---
#[derive(Clone)]
pub struct View {
    pub w: usize,
    pub h: usize,
    /// Pixel size, radians.
    pub pix: f64,
    pub fwd: V3,
    pub right: V3,
    pub up: V3,
}

impl View {
    /// A gnomonic view `fov_x` radians wide, looking from the origin at `look`.
    pub fn new(w: usize, h: usize, fov_x: f64, look: [f64; 3]) -> View {
        let fwd = V3::new(look[0], look[1], look[2]).norm();
        let right = V3::new(0.0, 1.0, 0.0).cross(fwd).norm();
        let up = fwd.cross(right).norm();
        View { w, h, pix: 2.0 * (0.5 * fov_x).tan() / w as f64, fwd, right, up }
    }
    pub fn pix_arcsec(&self) -> f64 { self.pix / ARCSEC }
    pub fn omega_pix(&self) -> f64 { self.pix * self.pix }
    /// Direction through image point (x, y) in pixel units (y down).
    pub fn dir(&self, x: f64, y: f64) -> V3 {
        let tx = (x - 0.5 * self.w as f64) * self.pix;
        let ty = (0.5 * self.h as f64 - y) * self.pix;
        (self.fwd + self.right * tx + self.up * ty).norm()
    }
    /// Pixel coordinates of a world point, if it is in front of the camera.
    pub fn project(&self, p: [f64; 3]) -> Option<(f64, f64)> {
        let v = V3::new(p[0], p[1], p[2]);
        let z = v.dot(self.fwd);
        if z <= 0.0 { return None; }
        Some((v.dot(self.right) / z / self.pix + 0.5 * self.w as f64, 0.5 * self.h as f64 - v.dot(self.up) / z / self.pix))
    }
}

// ------------------------------------------------------- band emission ---
/// Per-band volume grids: emissivity in units of `jscale`, and optical depth
/// per cell, with the volume geometry needed to march them.
pub struct Grids {
    pub j: Vec<Vec<f32>>,
    pub tau: Vec<Vec<f32>>,
    pub jscale: Vec<f64>,
    pub geom: Geom,
    /// Per-band factor applied to the marched intensity: (1+z)⁻³ and the
    /// foreground screen (1 for SZ bands, which carry Compton y).
    pub factor: Vec<f64>,
}

#[derive(Clone, Copy)]
pub struct Geom {
    pub n: usize,
    pub dx: f64,
    pub center: [f64; 3],
}

impl Geom {
    pub fn half(&self) -> f64 { 0.5 * self.n as f64 * self.dx }
}

impl Grids {
    /// Serialise for the browser's render workers: a small header, then f32 planes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let nb = self.j.len();
        let mut b = Vec::new();
        b.extend_from_slice(b"MWGRID01");
        for v in [nb as f64, self.geom.n as f64, self.geom.dx, self.geom.center[0], self.geom.center[1], self.geom.center[2]] { b.extend_from_slice(&v.to_le_bytes()); }
        for k in 0..nb { b.extend_from_slice(&self.jscale[k].to_le_bytes()); b.extend_from_slice(&self.factor[k].to_le_bytes()); b.push(!self.j[k].is_empty() as u8); }
        for k in 0..nb { for v in self.j[k].iter().chain(self.tau[k].iter()) { b.extend_from_slice(&v.to_le_bytes()); } }
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Grids> {
        if b.len() < 56 || &b[..8] != b"MWGRID01" { return None; }
        let f = |o: usize| f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let nb = f(8) as usize;
        let n = f(16) as usize;
        let geom = Geom { n, dx: f(24), center: [f(32), f(40), f(48)] };
        let mut o = 56;
        let mut g = Grids { j: vec![], tau: vec![], jscale: vec![], geom, factor: vec![] };
        let mut has = vec![];
        for _ in 0..nb { g.jscale.push(f(o)); g.factor.push(f(o + 8)); has.push(b[o + 16] != 0); o += 17; }
        let n3 = n * n * n;
        let rd = |o: &mut usize| -> Vec<f32> { let v = (0..n3).map(|i| f32::from_le_bytes(b[*o + 4 * i..*o + 4 * i + 4].try_into().unwrap())).collect(); *o += 4 * n3; v };
        for k in 0..nb {
            if has[k] { g.j.push(rd(&mut o)); g.tau.push(rd(&mut o)); } else { g.j.push(vec![]); g.tau.push(vec![]); }
        }
        Some(g)
    }
}

const K_SAMPLES: usize = 6;

struct BandCtx {
    nus: Vec<f64>,
    lines: Vec<usize>,
    h2: bool,
    dnu: f64,
    xray: Option<(f64, f64)>,
    sz: bool,
    cl_lnu: Vec<f64>,
}

fn band_ctx(sc: &MwScene, b: &Band) -> BandCtx {
    let z = sc.vol.z;
    let (a, c) = b.spec.nu_range();
    let nus: Vec<f64> = (0..K_SAMPLES).map(|k| a * (c / a).powf((k as f64 + 0.5) / K_SAMPLES as f64) * (1.0 + z)).collect();
    let lines = LINES.iter().enumerate().filter(|(_, l)| matches!(b.spec, Spec::Wave { .. }) && b.spec.contains_um(l.1, z)).map(|(i, _)| i).collect();
    let h2 = matches!(b.spec, Spec::Wave { .. }) && b.spec.contains_um(2.1218, z);
    let xray = if let Spec::Energy { lo, hi } = b.spec { Some((lo * (1.0 + z), hi * (1.0 + z))) } else { None };
    let cl_lnu = match sc.cluster { Some((_, _, l, t)) => nus.iter().map(|&nu| star_lnu(nu, l, t)).collect(), None => vec![0.0; K_SAMPLES] };
    BandCtx { nus, lines, h2, dnu: (c - a) * (1.0 + z), xray, sz: matches!(b.spec, Spec::Sz { .. }), cl_lnu }
}

/// Emissivity (erg s⁻¹ cm⁻³ Hz⁻¹ sr⁻¹; for SZ bands dy/dl in cm⁻¹) and
/// absorption (cm⁻¹) of cell i in band b.
fn cell_band(sc: &MwScene, i: usize, bc: &BandCtx, pos: [f64; 3]) -> (f64, f64) {
    let f = &sc.vol.f;
    let cosmic = sc.kind == "supercluster";
    let nh = f[F_NH][i] as f64;
    let (nhot, thot) = (f[F_NHOT][i] as f64, f[F_THOT][i] as f64);
    if bc.sz {
        let ne = if cosmic { nh } else { f[F_NE][i] as f64 };
        let t = f[F_TGAS][i] as f64;
        return (dy_dl(ne, t) + dy_dl(nhot, thot), 0.0);
    }
    if let Some((e1, e2)) = bc.xray {
        let z = sc.vol.metallicity;
        let j = xray_band(e1, e2, nhot, nhot / 1.2, thot, z);
        if cosmic { return (j, 0.0); }
        let x = f[F_XION][i] as f64;
        let mut a = 0.0;
        for k in 0..K_SAMPLES {
            let e = H * bc.nus[k] / KEV;
            a += nh * (1.0 - 0.9 * x) * sigma_pe(e);
        }
        return (j, a / K_SAMPLES as f64);
    }
    if cosmic {
        // warm–hot gas: free–free only (negligible below X-rays, kept for completeness)
        let t = f[F_TGAS][i] as f64;
        let (mut j, mut a) = (0.0, 0.0);
        for &nu in &bc.nus { j += j_ff(nu, nh, nh / 1.2, t); a += alpha_ff(nu, nh, nh / 1.2, t); }
        return (j / K_SAMPLES as f64, a / K_SAMPLES as f64);
    }
    let x = f[F_XION][i] as f64;
    let t = f[F_TGAS][i] as f64;
    let u = f[F_U][i] as f64;
    let ne_w = f[F_NE][i] as f64;
    let fd = f[F_FDUST][i] as f64;
    let fp = f[F_FPAH][i] as f64;
    let nsrc = f[F_NSRC][i] as f64;
    let td = dust_temperature(u);
    let ne = ne_w;
    let np = ne_w / 1.1;
    let r2 = sc.cluster.map(|(c, ..)| (0..3).map(|k| (pos[k] - c[k]).powi(2)).sum::<f64>()).unwrap_or(1e60);
    let (mut j, mut a) = (0.0, 0.0);
    for k in 0..K_SAMPLES {
        let nu = bc.nus[k];
        let l_um = C / nu * 1e4;
        j += j_ff(nu, ne, np, t) + j_ff(nu, nhot, nhot / 1.2, thot);
        a += alpha_ff(nu, ne, np, t);
        if l_um < 3000.0 {
            let se = sigma_ext(l_um);
            a += nh * fd * se;
            j += nh * fd * sigma_abs(l_um) * planck_nu(nu, td);
            j += nh * fp * u * pah_per_h(l_um) + nh * fd * u * vsg_per_h(l_um);
            if l_um < 5.0 {
                let fnu = bc.cl_lnu[k] / (4.0 * PI * r2) * (-se * nsrc).exp();
                j += nh * fd * se * albedo(l_um) * fnu / (4.0 * PI);
            }
        }
    }
    j /= K_SAMPLES as f64;
    a /= K_SAMPLES as f64;
    if !bc.lines.is_empty() && ne > 0.0 {
        let le = line_emissivities(ne, np, t, f[F_LOGU][i] as f64, f[F_FRONT][i] as f64);
        for &li in &bc.lines { j += le[li] / bc.dnu; }
    }
    if bc.h2 && x < 0.99 {
        j += h2_emissivity(nh * (1.0 - x), f[F_USURF][i] as f64, f[F_COLSURF][i] as f64) / bc.dnu;
    }
    (j, a)
}

pub fn cell_center(v: &MwVolume, i: usize) -> [f64; 3] {
    let n = v.n;
    let h = 0.5 * (n as f64 - 1.0);
    [v.center[0] + ((i % n) as f64 - h) * v.dx, v.center[1] + (((i / n) % n) as f64 - h) * v.dx, v.center[2] + ((i / (n * n)) as f64 - h) * v.dx]
}

/// Build the emissivity and optical-depth grids for every band of `ins`.
pub fn band_grids(sc: &MwScene, ins: &Instrument) -> Grids {
    let n3 = sc.vol.n.pow(3);
    let path = 1.0 / (1.0 + sc.vol.z); // comoving cell → proper path for cosmic gas
    let zfac = 1.0 / (1.0 + sc.vol.z).powi(3);
    let factor = ins.bands.iter().map(|b| if matches!(b.spec, Spec::Sz { .. }) { 1.0 } else { zfac * foreground(b, sc.vol.fg_av) }).collect();
    let mut g = Grids { j: vec![], tau: vec![], jscale: vec![], geom: Geom { n: sc.vol.n, dx: sc.vol.dx, center: sc.vol.center }, factor };
    for b in &ins.bands {
        if matches!(b.spec, Spec::Kappa) { g.j.push(vec![]); g.tau.push(vec![]); g.jscale.push(0.0); continue; }
        let bc = band_ctx(sc, b);
        let ja: Vec<(f64, f64)> = par_map(n3, |i| cell_band(sc, i, &bc, cell_center(&sc.vol, i)));
        let jmax = ja.iter().map(|v| v.0).fold(0.0, f64::max).max(1e-300);
        g.j.push(ja.iter().map(|v| (v.0 / jmax) as f32).collect());
        g.tau.push(ja.iter().map(|v| (v.1 * sc.vol.dx * path) as f32).collect());
        g.jscale.push(jmax * sc.vol.dx * path);
    }
    g
}

#[inline]
fn trilinear(g: &Grids, n: usize, p: [f64; 3], jo: &mut [f64], to: &mut [f64]) {
    for v in jo.iter_mut() { *v = 0.0; }
    for v in to.iter_mut() { *v = 0.0; }
    let f = [p[0].floor(), p[1].floor(), p[2].floor()];
    let t = [p[0] - f[0], p[1] - f[1], p[2] - f[2]];
    let ni = n as i64;
    for c in 0..8 {
        let o = [(c & 1) as i64, ((c >> 1) & 1) as i64, ((c >> 2) & 1) as i64];
        // clamp to the edge cells: the box boundary is the volume's edge, not a ramp to zero
        let ix = [0, 1, 2].map(|k| (f[k] as i64 + o[k]).clamp(0, ni - 1));
        let w = (0..3).map(|k| if o[k] == 1 { t[k] } else { 1.0 - t[k] }).product::<f64>();
        let i = ((ix[2] * ni + ix[1]) * ni + ix[0]) as usize;
        for b in 0..jo.len() {
            if g.j[b].is_empty() { continue; }
            jo[b] += w * g.j[b][i] as f64;
            to[b] += w * g.tau[b][i] as f64;
        }
    }
}

/// Entry and exit of a ray (origin o, direction d) through the volume box, as distances.
fn box_hit(v: &Geom, o: [f64; 3], d: V3) -> Option<(f64, f64)> {
    let h = v.half();
    let dd = [d.x, d.y, d.z];
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for k in 0..3 {
        let (lo, hi) = (v.center[k] - h, v.center[k] + h);
        if dd[k].abs() < 1e-300 {
            if o[k] < lo || o[k] > hi { return None; }
            continue;
        }
        let (mut a, mut b) = ((lo - o[k]) / dd[k], (hi - o[k]) / dd[k]);
        if a > b { std::mem::swap(&mut a, &mut b); }
        t0 = t0.max(a);
        t1 = t1.min(b);
    }
    if t1 > t0 { Some((t0, t1)) } else { None }
}

/// March one ray from `o` along `d`, stopping at distance `tmax`; returns
/// per-band intensity (in units of jscale) and optical depth.
fn march(v: &Geom, g: &Grids, o: [f64; 3], d: V3, tmax: f64, jitter: f64, out_i: &mut [f64], out_tau: &mut [f64]) {
    let nb = out_i.len();
    for b in 0..nb { out_i[b] = 0.0; out_tau[b] = 0.0; }
    let Some((t0, t1)) = box_hit(v, o, d) else { return };
    let t1 = t1.min(tmax);
    if t1 <= t0 { return; }
    let ds = 0.5 * v.dx;
    let base = [v.center[0] - v.half(), v.center[1] - v.half(), v.center[2] - v.half()];
    let mut jb = vec![0.0; nb];
    let mut tb = vec![0.0; nb];
    // front-to-back accumulation with running transmission; the first step is
    // a jittered fraction of ds so sample positions decorrelate between rays
    let mut t = t0;
    let mut first = true;
    while t < t1 {
        let step = if first { (jitter.max(0.05) * ds).min(t1 - t) } else { ds.min(t1 - t) };
        first = false;
        let mid = t + 0.5 * step;
        let p = [o[0] + d.x * mid, o[1] + d.y * mid, o[2] + d.z * mid];
        let pv = [(p[0] - base[0]) / v.dx - 0.5, (p[1] - base[1]) / v.dx - 0.5, (p[2] - base[2]) / v.dx - 0.5];
        trilinear(g, v.n, pv, &mut jb, &mut tb);
        let cells = step / v.dx;
        for b in 0..nb {
            let dtau = tb[b] * cells;
            let trans = (-out_tau[b]).exp();
            let w = if dtau < 1e-4 { cells * (1.0 - 0.5 * dtau) } else { cells * (1.0 - (-dtau).exp()) / dtau };
            out_i[b] += jb[b] * w * trans;
            out_tau[b] += dtau;
        }
        t += step;
    }
}

// --------------------------------------------------------------- sources --
/// Band-averaged flux density at the observer (erg s⁻¹ cm⁻² Hz⁻¹), before
/// absorption by the volume, including Galactic foreground extinction.
pub fn sed_flux(s: &Source, b: &Band) -> f64 {
    let (a, c) = b.spec.nu_range();
    let d = if s.d_l > 0.0 { s.d_l } else { (s.pos[0] * s.pos[0] + s.pos[1] * s.pos[1] + s.pos[2] * s.pos[2]).sqrt() };
    let z = s.z;
    let fac = (1.0 + z) / (4.0 * PI * d * d);
    let nh_fg = 2.2e21 * s.a_v_fg;
    if let Spec::Energy { lo, hi } = b.spec {
        // coronal or star-forming X-rays: thermal (YSO) or Γ = 2 power law (galaxies)
        let (lx, shape): (f64, Box<dyn Fn(f64) -> f64>) = match s.sed {
            Sed::Yso { lx, kt_x, .. } => (lx, Box::new(move |e: f64| (-e / kt_x).exp() / ((-0.5 / kt_x).exp() - (-8.0 / kt_x).exp()) / kt_x)),
            Sed::Galaxy { sfr, .. } => (2.6e39 * sfr, Box::new(|e: f64| 1.0 / (e * e) / (1.0 / 0.5 - 1.0 / 8.0))),
            _ => return 0.0,
        };
        let n = 16;
        let mut f = 0.0;
        for k in 0..n {
            let e = lo + (hi - lo) * (k as f64 + 0.5) / n as f64;
            let er = e * (1.0 + z);
            // energy per keV × dE, absorbed by the foreground column
            f += lx * shape(er) * (1.0 + z) * (hi - lo) / n as f64 * (-nh_fg * sigma_pe(e)).exp();
        }
        let dnu = (hi - lo) * KEV / H;
        return f / (4.0 * PI * d * d) / dnu;
    }
    let mut sum = 0.0;
    for k in 0..K_SAMPLES {
        let nu = a * (c / a).powf((k as f64 + 0.5) / K_SAMPLES as f64);
        let nur = nu * (1.0 + z);
        let l_um = C / nu * 1e4;
        let lnu = match s.sed {
            Sed::Star { l, t } => star_lnu(nur, l, t),
            Sed::Yso { l, t, f_env, t_env, .. } => star_lnu(nur, l * (1.0 - f_env), t) + greybody_lnu(nur, l * f_env, t_env, 1.0),
            Sed::Galaxy { mstar, sfr, l14, alpha } => galaxy_lnu(nur, mstar, sfr, l14, alpha),
            Sed::Radio { l14, alpha } => l14 * (nur / 1.4e9).powf(-alpha),
        };
        let ext = if l_um < 3000.0 { 10f64.powf(-0.4 * s.a_v_fg * ext_ratio(l_um)) } else { 1.0 };
        sum += lnu * ext;
    }
    sum / K_SAMPLES as f64 * fac
}

/// Rest-frame L_ν of a galaxy: old stars (4000 K, M/L_r ≈ 3), young stars
/// (15 000 K, Kennicutt UV calibration, internal A_V = 0.8), dust heated by
/// star formation (grey body, 30 K, β = 1.8; L_IR = 5.8e9 L☉ per M☉ yr⁻¹),
/// PAHs and radio (*numerically approximated* templates).
pub fn galaxy_lnu(nu: f64, mstar: f64, sfr: f64, l14: f64, alpha: f64) -> f64 {
    let l_um = C / nu * 1e4;
    let nu_r = C / 0.62e-4;
    let old = 2.74e18 * mstar * planck_nu(nu, 4000.0) / planck_nu(nu_r, 4000.0);
    let nu_uv = C / 0.15e-4;
    let young = 7.1e27 * sfr * planck_nu(nu, 15000.0) / planck_nu(nu_uv, 15000.0) * 10f64.powf(-0.4 * 0.8 * ext_ratio(l_um.min(3000.0)));
    let lir = 5.8e9 * sfr;
    let dust = greybody_lnu(nu, 0.8 * lir, 30.0, 1.8);
    let pah = if l_um > 2.0 && l_um < 40.0 { 0.12 * lir * L_SUN * 4.0 * PI * pah_per_h(l_um) / P_PAH_ISRF } else { 0.0 };
    let radio = l14 * (nu / 1.4e9).powf(-alpha);
    old + young + dust + pah + radio
}

/// March from the observer to distance `d` along `dir` (for tests and tools).
pub fn march_public(geom: &Geom, g: &Grids, d: f64, dir: V3, out_i: &mut [f64], out_tau: &mut [f64]) {
    march(geom, g, [0.0; 3], dir, d, 0.5, out_i, out_tau);
}

/// Optical depth per band from a source to the observer through the volume.
fn source_tau(g: &Grids, s: &Source) -> Vec<f64> {
    let nb = g.j.len();
    let p = V3::new(s.pos[0], s.pos[1], s.pos[2]);
    let d = p.len();
    let dir = p * (1.0 / d);
    let mut i = vec![0.0; nb];
    let mut t = vec![0.0; nb];
    march(&g.geom, g, [0.0, 0.0, 0.0], dir, d, 0.5, &mut i, &mut t);
    t
}

/// Add a source's flux to a map in intensity units (flux / pixel solid angle).
fn splat(map: &mut [f64], view: &View, x: f64, y: f64, flux: f64, size_rad: f64) {
    let (w, h) = (view.w as i64, view.h as i64);
    let inten = flux / view.omega_pix();
    let sp = size_rad / view.pix;
    if sp < 0.4 {
        let (fx, fy) = ((x - 0.5).floor(), (y - 0.5).floor());
        let (tx, ty) = (x - 0.5 - fx, y - 0.5 - fy);
        for (dx, dy, wgt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let (px, py) = (fx as i64 + dx, fy as i64 + dy);
            if px >= 0 && py >= 0 && px < w && py < h { map[(py * w + px) as usize] += inten * wgt; }
        }
        return;
    }
    let r = (3.5 * sp).ceil() as i64;
    let (cx, cy) = (x.floor() as i64, y.floor() as i64);
    let mut norm = 0.0;
    let mut ws = Vec::new();
    for py in cy - r..=cy + r { for px in cx - r..=cx + r {
        let (dx, dy) = (px as f64 + 0.5 - x, py as f64 + 0.5 - y);
        let wgt = (-(dx * dx + dy * dy) / (2.0 * sp * sp)).exp();
        norm += wgt;
        ws.push((px, py, wgt));
    }}
    for (px, py, wgt) in ws {
        if px >= 0 && py >= 0 && px < w && py < h { map[(py * w + px) as usize] += inten * wgt / norm; }
    }
}

// ---------------------------------------------------------- cluster gas --
/// X-ray band intensity and Compton y of every halo, plus its radio halo and
/// relics, added in image space. `maps[b]` receives I_ν (or y for SZ bands).
fn add_halos(sc: &MwScene, view: &View, bands: &[Band], maps: &mut [Vec<f64>]) {
    for hl in &sc.halos {
        let Some((cx, cy)) = view.project(hl.pos) else { continue };
        let chi = (hl.pos[0].powi(2) + hl.pos[1].powi(2) + hl.pos[2].powi(2)).sqrt();
        let zc = hl.z;
        let d_a = chi / (1.0 + zc);
        let rmax = 3.0 * hl.r500;
        let rpx = (rmax / d_a / view.pix).ceil() as i64;
        if cx < -(rpx as f64) || cy < -(rpx as f64) || cx > (view.w as i64 + rpx) as f64 || cy > (view.h as i64 + rpx) as f64 { continue; }
        let hp = V3::new(hl.pos[0], hl.pos[1], hl.pos[2]).norm();
        let (cpa, spa) = (hl.pa.cos(), hl.pa.sin());
        // per-band unit emissivities (n_e = 1)
        let unit: Vec<(u8, f64)> = bands.iter().map(|b| match b.spec {
            Spec::Energy { lo, hi } => (1, xray_band(lo * (1.0 + zc), hi * (1.0 + zc), 1.0, 1.0 / 1.2, hl.t, 0.3)),
            Spec::Sz { .. } => (2, dy_dl(1.0, hl.t)),
            Spec::Freq { ghz, .. } if ghz < 20.0 => (3, ghz * 1e9),
            _ => (0, 0.0),
        }).collect();
        if unit.iter().all(|u| u.0 == 0) { continue; }
        let d_l = chi * (1.0 + zc);
        let ss = 2usize; // sub-pixel samples per axis
        let (x0, x1) = ((cx as i64 - rpx).max(0), (cx as i64 + rpx).min(view.w as i64 - 1));
        let (y0, y1) = ((cy as i64 - rpx).max(0), (cy as i64 + rpx).min(view.h as i64 - 1));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let mut acc = vec![0.0; bands.len()];
                for sy in 0..ss { for sx in 0..ss {
                    let d = view.dir(px as f64 + (sx as f64 + 0.5) / ss as f64, py as f64 + (sy as f64 + 0.5) / ss as f64);
                    let off = d - hp;
                    let (ox, oy) = (off.dot(view.right), off.dot(view.up));
                    let (u, v) = (cpa * ox + spa * oy, -spa * ox + cpa * oy);
                    let re = (u * u + v * v / (hl.q * hl.q)).sqrt() * d_a; // elliptical projected radius (cm)
                    let rr = (u * u + v * v).sqrt() * d_a;
                    // line-of-sight integrals of n_e² and n_e
                    let (mut em, mut col) = (0.0, 0.0);
                    if re < rmax {
                        let lmax = (rmax * rmax - re * re).sqrt();
                        let nq = 24;
                        for k in 0..nq {
                            let uu = (k as f64 + 0.5) / nq as f64;
                            let l = lmax * uu * uu;
                            let dl = 2.0 * lmax * uu / nq as f64;
                            let r2 = re * re + l * l;
                            let ne = hl.ne0 * (1.0 + r2 / (hl.rc * hl.rc)).powf(-1.5 * hl.beta);
                            em += 2.0 * ne * ne * dl;
                            col += 2.0 * ne * dl;
                        }
                    }
                    for (b, &(kind, val)) in unit.iter().enumerate() {
                        acc[b] += match kind {
                            1 => val * em / (1.0 + zc).powi(3),
                            2 => val * col,
                            3 => {
                                let nu_r = val * (1.0 + zc);
                                let mut sb = 0.0;
                                if hl.radio_halo > 0.0 {
                                    let rh = 0.3 * hl.r500;
                                    let lnu = hl.radio_halo * (nu_r / 1.4e9).powf(-1.3);
                                    let s = lnu * (1.0 + zc) / (4.0 * PI * d_l * d_l);
                                    // projected exponential, normalised over the sky: ∫ = 2π (r_h/d_A)²
                                    sb += s * (-rr / rh).exp() / (2.0 * PI * (rh / d_a).powi(2));
                                }
                                if let Some((_, dist, pw)) = hl.relics {
                                    let lnu = pw * (nu_r / 1.4e9).powf(-1.1);
                                    let s = lnu * (1.0 + zc) / (4.0 * PI * d_l * d_l);
                                    let (uu, vv) = (u * d_a, v * d_a);
                                    let sr = 0.06 * MPC;
                                    for sgn in [-1.0, 1.0] {
                                        // an arc of radius `dist` about the merger axis, ±35°, radial σ 60 kpc
                                        let phi = vv.atan2(uu * sgn);
                                        // compact angular window, zero beyond ±0.9 rad
                                        let win = if phi.abs() < 0.9 { (phi / 0.9 * PI / 2.0).cos().powi(2) } else { 0.0 };
                                        let prof = (-(rr - dist).powi(2) / (2.0 * sr * sr)).exp() * win;
                                        let norm = (2.0 * PI).sqrt() * sr * 0.9 * dist / (d_a * d_a);
                                        sb += s * prof / norm;
                                    }
                                }
                                sb
                            }
                            _ => 0.0,
                        };
                    }
                }}
                let i = py as usize * view.w + px as usize;
                for b in 0..bands.len() { maps[b][i] += acc[b] / (ss * ss) as f64; }
            }
        }
    }
}

// ------------------------------------------------------------ CMB field --
/// Primary CMB anisotropy over the view (µK), flat-sky Gaussian field with an
/// approximate ΛCDM D_ℓ: Sachs–Wolfe plateau, five acoustic peaks and damping
/// (*numerically approximated*: a fit, not a Boltzmann code).
pub fn d_ell(l: f64) -> f64 {
    let peaks = [(220.0, 4600.0, 85.0), (537.0, 2100.0, 70.0), (810.0, 2350.0, 85.0), (1120.0, 1150.0, 90.0), (1430.0, 820.0, 100.0)];
    let mut d = 1050.0 * (1.0 + (l / 120.0).powi(2)).powf(-0.25);
    for (c, a, w) in peaks { d += a * (-(l - c).powi(2) / (2.0 * w * w)).exp(); }
    let trough = 1.0 + 0.9 * (l / 400.0).min(1.0);
    (d * trough / 1.9) * (-(l / 1550.0).powf(1.6)).exp()
}

pub fn cmb_field(view: &View, seed: u64) -> Vec<f64> {
    let n = view.w.max(view.h).next_power_of_two();
    let mut rng = Rng::new(seed, 0xC3B);
    let mut re: Vec<f64> = (0..n * n).map(|_| rng.normal()).collect();
    let mut im = vec![0.0; n * n];
    fft2(&mut re, &mut im, n, false);
    let p = view.pix;
    for y in 0..n { for x in 0..n {
        let kx = if x <= n / 2 { x as f64 } else { x as f64 - n as f64 };
        let ky = if y <= n / 2 { y as f64 } else { y as f64 - n as f64 };
        let l = 2.0 * PI * (kx * kx + ky * ky).sqrt() / (n as f64 * p);
        let a = if l < 2.0 { 0.0 } else { (2.0 * PI * d_ell(l) / (l * (l + 1.0)) / (p * p)).sqrt() };
        re[y * n + x] *= a;
        im[y * n + x] *= a;
    }}
    fft2(&mut re, &mut im, n, true);
    let mut out = vec![0.0; view.w * view.h];
    for y in 0..view.h { for x in 0..view.w { out[y * view.w + x] = re[y * n + x]; } }
    out
}

fn fft2(re: &mut [f64], im: &mut [f64], n: usize, inverse: bool) {
    let mut r = vec![0.0; n];
    let mut i = vec![0.0; n];
    for y in 0..n { crate::sensor::fft(&mut re[y * n..(y + 1) * n], &mut im[y * n..(y + 1) * n], inverse); }
    for x in 0..n {
        for y in 0..n { r[y] = re[y * n + x]; i[y] = im[y * n + x]; }
        crate::sensor::fft(&mut r, &mut i, inverse);
        for y in 0..n { re[y * n + x] = r[y]; im[y * n + x] = i[y]; }
    }
}

// --------------------------------------------------------- weak lensing --
/// Convergence κ for sources at z = 1 in the Born approximation:
/// κ = (3/2)(H₀/c)² Ω_m ∫ dχ (1+z) χ (χ_s − χ)/χ_s δ, with δ from the
/// particles (each carrying volume L³/N) minus the mean along each ray, plus
/// NFW halo cores when `nfw` is set.
pub fn kappa_map(sc: &MwScene, view: &View, nfw: bool) -> Vec<f64> {
    let (h, om, lbox) = sc.cosmo.unwrap_or((0.7, 0.3, 120.0));
    let cos = super::cosmic::Cosmo { h, om };
    let chi_s = cos.chi(1.0);
    let ztab = cos.z_table();
    let z_at = |chi: f64| { let c = chi.clamp(0.0, 1999.999); let i = c as usize; ztab[i] + (ztab[i + 1] - ztab[i]) * (c - i as f64) };
    let hc = h / 2997.92458;
    let wk = |chi: f64| 1.5 * hc * hc * om * (1.0 + z_at(chi)) * chi * (chi_s - chi) / chi_s;
    let mut k = vec![0.0; view.w * view.h];
    if let Some(parts) = &sc.particles {
        let vp = lbox.powi(3) / parts.len() as f64;
        for q in parts {
            let p = [q[0] as f64 * MPC, q[1] as f64 * MPC, q[2] as f64 * MPC];
            let Some((x, y)) = view.project(p) else { continue };
            let chi = ((q[0] as f64).powi(2) + (q[1] as f64).powi(2) + (q[2] as f64).powi(2)).sqrt();
            let val = wk(chi) * vp / (chi * chi * view.omega_pix());
            let (fx, fy) = ((x - 0.5).floor(), (y - 0.5).floor());
            let (tx, ty) = (x - 0.5 - fx, y - 0.5 - fy);
            for (dx, dy, wg) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                let (px, py) = (fx as i64 + dx, fy as i64 + dy);
                if px >= 0 && py >= 0 && (px as usize) < view.w && (py as usize) < view.h { k[py as usize * view.w + px as usize] += val * wg; }
            }
        }
        // subtract the mean-density contribution along each ray through the box
        let v = Geom { n: sc.vol.n, dx: sc.vol.dx, center: sc.vol.center };
        for py in 0..view.h { for px in 0..view.w {
            let d = view.dir(px as f64 + 0.5, py as f64 + 0.5);
            if let Some((t0, t1)) = box_hit(&v, [0.0; 3], d) {
                let (c0, c1) = (t0 / MPC, t1 / MPC);
                let nq = 32;
                let s: f64 = (0..nq).map(|i| wk(c0 + (c1 - c0) * (i as f64 + 0.5) / nq as f64)).sum::<f64>() * (c1 - c0) / nq as f64;
                k[py * view.w + px] -= s;
            }
        }}
    }
    if nfw {
        let sig_c_const = 1.6625e18; // c²/4πG in M☉ Mpc⁻¹
        for hl in &sc.halos {
            let Some((cx, cy)) = view.project(hl.pos) else { continue };
            let chi = (hl.pos[0].powi(2) + hl.pos[1].powi(2) + hl.pos[2].powi(2)).sqrt() / MPC;
            let zl = z_at(chi);
            let (dl, ds, dls) = (chi / (1.0 + zl), chi_s / 2.0, (chi_s - chi) / 2.0);
            let sig_c = sig_c_const * ds / (dl * dls); // M☉ Mpc⁻²
            let c = hl.c200;
            let rs = hl.r200 / MPC / c;
            let rho_c = cos.rho_crit() * cos.e(zl).powi(2);
            let rhos = 200.0 / 3.0 * rho_c * c.powi(3) / ((1.0 + c).ln() - c / (1.0 + c));
            let rpx = (3.0 * hl.r200 / MPC / dl / view.pix).ceil() as i64;
            for py in (cy as i64 - rpx).max(0)..=(cy as i64 + rpx).min(view.h as i64 - 1) {
                for px in (cx as i64 - rpx).max(0)..=(cx as i64 + rpx).min(view.w as i64 - 1) {
                    let rr = ((px as f64 + 0.5 - cx).powi(2) + (py as f64 + 0.5 - cy).powi(2)).sqrt() * view.pix * dl;
                    let x = (rr / rs).max(1e-3);
                    let f = if (x - 1.0).abs() < 1e-3 { 1.0 / 3.0 } else if x < 1.0 {
                        (1.0 - 2.0 / (1.0 - x * x).sqrt() * (((1.0 - x) / (1.0 + x)).sqrt()).atanh()) / (x * x - 1.0)
                    } else {
                        (1.0 - 2.0 / (x * x - 1.0).sqrt() * (((x - 1.0) / (1.0 + x)).sqrt()).atan()) / (x * x - 1.0)
                    };
                    k[py as usize * view.w + px as usize] += 2.0 * rs * rhos * f / sig_c;
                }
            }
        }
    }
    k
}

// ------------------------------------------------------------ observing --
pub struct BandMap {
    pub name: String,
    pub traces: String,
    /// Observed map in the instrument's unit, with noise.
    pub data: Vec<f32>,
    /// The same without noise (after PSF/beam).
    pub ideal: Vec<f32>,
    /// Noise rms per pixel, in the map's unit.
    pub sigma: f64,
    pub fwhm_px: f64,
    pub lambda_um: f64,
}

pub struct Observation {
    pub ins: Instrument,
    pub w: usize,
    pub h: usize,
    pub pix_arcsec: f64,
    pub maps: Vec<BandMap>,
    pub log: Vec<String>,
}

pub struct ObsOpts {
    pub spp: u32,
    pub seed: u64,
    pub noise: bool,
}

/// One image row of the volume march: w × nb intensities (cgs, or y for SZ
/// bands), already scaled by each band's `jscale` and `factor`.
pub fn march_row(g: &Grids, view: &View, y: usize, spp: u32, seed: u64) -> Vec<f64> {
    let nb = g.j.len();
    let w = view.w;
    let spp = spp.max(1);
    let mut row = vec![0.0; w * nb];
    let mut ii = vec![0.0; nb];
    let mut tt = vec![0.0; nb];
    let mut rng = Rng::keyed(&[seed, 0x5A3, y as u64]);
    for x in 0..w {
        for _ in 0..spp {
            let (jx, jy) = if spp == 1 { (0.5, 0.5) } else { (rng.f64(), rng.f64()) };
            let d = view.dir(x as f64 + jx, y as f64 + jy);
            march(&g.geom, g, [0.0; 3], d, f64::INFINITY, rng.f64(), &mut ii, &mut tt);
            for b in 0..nb { row[x * nb + b] += ii[b] * g.jscale[b]; }
        }
    }
    for x in 0..w { for b in 0..nb { row[x * nb + b] *= g.factor[b] / spp as f64; } }
    row
}

/// Everything except the volume: cluster atmospheres, sources (attenuated by
/// the volume when it is a Galactic cloud) and, for lensing, κ.
pub fn extra_maps(sc: &MwScene, ins: &Instrument, view: &View, g: Option<&Grids>, log: &mut dyn FnMut(&str)) -> Vec<Vec<f64>> {
    let nb = ins.bands.len();
    let (w, h) = (view.w, view.h);
    let mut maps = vec![vec![0.0f64; w * h]; nb];
    if ins.kind == Kind::Lensing {
        maps[0] = kappa_map(sc, view, ins.smooth_arcmin < 2.0);
        return maps;
    }
    add_halos(sc, view, &ins.bands, &mut maps);
    log(&format!("{}: {} sources", ins.id, sc.sources.len()));
    let srcs: Vec<(f64, f64, Vec<f64>, f64)> = par_map(sc.sources.len(), |k| {
        let s = &sc.sources[k];
        let Some((x, y)) = view.project(s.pos) else { return (0.0, 0.0, vec![], 0.0) };
        if x < -20.0 || y < -20.0 || x > w as f64 + 20.0 || y > h as f64 + 20.0 { return (0.0, 0.0, vec![], 0.0); }
        let tau = match g { Some(g) if sc.kind == "pillars" => source_tau(g, s), _ => vec![0.0; nb] };
        let f: Vec<f64> = ins.bands.iter().enumerate().map(|(b, band)| sed_flux(s, band) * (-tau[b]).exp()).collect();
        (x, y, f, s.size)
    });
    for (x, y, f, size) in srcs {
        if f.is_empty() { continue; }
        for b in 0..nb {
            if f[b] <= 0.0 { continue; }
            // SZ bands carry Compton y until the instrument step multiplies by T_CMB f(ν):
            // store a source as the y that gives its ΔT = I_ν / (dB/dT)
            let val = if let Spec::Sz { ghz } = ins.bands[b].spec { f[b] / db_dt(ghz * 1e9) / (T_CMB * sz_dt_factor(ghz * 1e9)) } else { f[b] };
            splat(&mut maps[b], view, x, y, val, size);
        }
    }
    maps
}

/// Intensity maps (cgs, before the instrument) of every band: volume, sources and haloes.
pub fn sky_maps(sc: &MwScene, ins: &Instrument, view: &View, opts: &ObsOpts, log: &mut dyn FnMut(&str)) -> Vec<Vec<f64>> {
    let nb = ins.bands.len();
    let w = view.w;
    if ins.kind == Kind::Lensing { return extra_maps(sc, ins, view, None, log); }
    log(&format!("{}: emissivity grids", ins.id));
    let g = band_grids(sc, ins);
    log(&format!("{}: ray march ({} spp)", ins.id, opts.spp));
    let rows: Vec<Vec<f64>> = par_map(view.h, |y| march_row(&g, view, y, opts.spp, opts.seed));
    let mut maps = extra_maps(sc, ins, view, Some(&g), log);
    for (y, row) in rows.iter().enumerate() {
        for x in 0..w { for b in 0..nb { maps[b][y * w + x] += row[x * nb + b]; } }
    }
    maps
}

/// Band-averaged transmission of a Galactic foreground screen of visual
/// extinction a_v (dust at λ < 3 mm, photoelectric absorption by N_H = 2.2e21 A_V in X-rays).
pub fn foreground(b: &Band, a_v: f64) -> f64 {
    if a_v <= 0.0 { return 1.0; }
    let (a, c) = b.spec.nu_range();
    (0..K_SAMPLES).map(|k| {
        let nu = a * (c / a).powf((k as f64 + 0.5) / K_SAMPLES as f64);
        match b.spec {
            Spec::Energy { .. } => (-2.2e21 * a_v * sigma_pe(H * nu / KEV)).exp(),
            Spec::Kappa | Spec::Sz { .. } => 1.0,
            _ => { let l = C / nu * 1e4; if l < 3000.0 { 10f64.powf(-0.4 * a_v * ext_ratio(l)) } else { 1.0 } }
        }
    }).sum::<f64>() / K_SAMPLES as f64
}

/// dB_ν/dT at T_CMB, erg s⁻¹ cm⁻² Hz⁻¹ sr⁻¹ K⁻¹.
pub fn db_dt(nu: f64) -> f64 {
    let x = H * nu / (K * T_CMB);
    2.0 * K * nu * nu / (C * C) * x * x * x.exp() / (x.exp_m1()).powi(2)
}

fn gauss_kernel(sigma: f64) -> (Vec<f32>, i64) {
    let r = (4.0 * sigma).ceil().max(1.0) as i64;
    let n = (2 * r + 1) as usize;
    let mut k = vec![0f32; n * n];
    let mut s = 0.0;
    for y in 0..n { for x in 0..n {
        let (dx, dy) = (x as f64 - r as f64, y as f64 - r as f64);
        let v = (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp();
        k[y * n + x] = v as f32;
        s += v;
    }}
    for v in k.iter_mut() { *v /= s as f32; }
    (k, r)
}

pub fn convolve(img: &[f64], w: usize, h: usize, sigma_px: f64) -> Vec<f64> {
    if sigma_px < 0.3 { return img.to_vec(); }
    // keep precision: convolve a normalised copy in f32
    let m = img.iter().fold(0.0f64, |a, &v| a.max(v.abs())).max(1e-300);
    let f: Vec<f32> = img.iter().map(|&v| (v / m) as f32).collect();
    let (k, r) = gauss_kernel(sigma_px);
    fft_convolve(&f, w, h, &k, r).iter().map(|&v| v as f64 * m).collect()
}

/// Observe: sky maps → beam/PSF → units → noise.
pub fn observe(sc: &MwScene, ins: &Instrument, view: &View, opts: &ObsOpts, log: &mut dyn FnMut(&str)) -> Observation {
    let sky = sky_maps(sc, ins, view, opts, log);
    apply_instrument(ins, view, sky, opts)
}

/// The instrument: beam or PSF, units and noise, applied to sky maps.
pub fn apply_instrument(ins: &Instrument, view: &View, mut sky: Vec<Vec<f64>>, opts: &ObsOpts) -> Observation {
    let (w, h) = (view.w, view.h);
    let pa = view.pix_arcsec();
    let mut out = Vec::new();
    let mut cmb: Option<Vec<f64>> = None;
    for (b, band) in ins.bands.iter().enumerate() {
        let fwhm_px = band.fwhm / pa;
        let sig_px = fwhm_px / 2.3548;
        let mut rng = Rng::keyed(&[opts.seed, 0x0B5, b as u64]);
        let (ideal, data, sigma): (Vec<f64>, Vec<f64>, f64) = match ins.kind {
            Kind::Imager | Kind::Xray => {
                let (a, c) = band.spec.nu_range();
                let conv = view.omega_pix() * band.area * ins.exposure_s * (c / a).ln() / H;
                let img = convolve(&sky[b], w, h, sig_px);
                if ins.kind == Kind::Xray {
                    let bkg = band.bkg * pa * pa * ins.exposure_s;
                    let ideal: Vec<f64> = img.iter().map(|&v| v * conv + bkg).collect();
                    let data: Vec<f64> = ideal.iter().map(|&m| if opts.noise { rng.poisson(m) } else { m }).collect();
                    (ideal, data, bkg.sqrt())
                } else {
                    let bkg = band.bkg * 1e-17 * conv;
                    let rn = ins.read_noise * (pa / ins.native_px).max(1.0);
                    let to_mjy = 1e17 / conv;
                    let ideal: Vec<f64> = img.iter().map(|&v| v * 1e17).collect();
                    let data: Vec<f64> = img.iter().map(|&v| {
                        let m = v * conv;
                        if opts.noise { (rng.poisson(m + bkg) + rn * rng.normal() - bkg) * to_mjy } else { m * to_mjy }
                    }).collect();
                    (ideal, data, (bkg + rn * rn).sqrt() * to_mjy)
                }
            }
            Kind::Radio => {
                let beam_sr = PI / (4.0 * 2f64.ln()) * (band.fwhm * ARCSEC).powi(2);
                let img = convolve(&sky[b], w, h, sig_px);
                let ideal: Vec<f64> = img.iter().map(|&v| v * beam_sr * 1e26).collect();
                let noise = correlated_noise(w, h, sig_px, band.sigma, &mut rng);
                let data: Vec<f64> = ideal.iter().zip(&noise).map(|(&v, &n)| if opts.noise { v + n } else { v }).collect();
                (ideal, data, band.sigma)
            }
            Kind::Cmb => {
                let Spec::Sz { ghz } = band.spec else { unreachable!() };
                let nu = ghz * 1e9;
                if cmb.is_none() { cmb = Some(cmb_field(view, opts.seed ^ 0xC4B)); }
                // sky[b] holds y (volume + haloes) plus source ΔT in K (already divided by dB/dT)
                let fz = sz_dt_factor(nu);
                let prim = cmb.as_ref().unwrap();
                let total: Vec<f64> = (0..w * h).map(|i| (T_CMB * fz * sky[b][i]) * 1e6 + prim[i]).collect();
                let ideal = convolve(&total, w, h, sig_px);
                let sig_pix = band.sigma / (pa / 60.0);
                let data: Vec<f64> = ideal.iter().map(|&v| if opts.noise { v + sig_pix * rng.normal() } else { v }).collect();
                (ideal, data, sig_pix)
            }
            Kind::Lensing => {
                let pam = pa / 60.0;
                let sig_pix = band.sigma / (ins.n_gal * pam * pam).sqrt();
                let ss = ins.smooth_arcmin / pam;
                let ideal = convolve(&sky[b], w, h, ss);
                let noise: Vec<f64> = (0..w * h).map(|_| sig_pix * rng.normal()).collect();
                let nsm = convolve(&noise, w, h, ss);
                let data: Vec<f64> = ideal.iter().zip(&nsm).map(|(&v, &n)| if opts.noise { v + n } else { v }).collect();
                let s_after = sig_pix / (2.0 * PI.sqrt() * ss.max(0.5));
                (ideal, data, s_after)
            }
        };
        sky[b].clear();
        out.push(BandMap {
            name: band.name.clone(), traces: band.traces.clone(),
            data: data.iter().map(|&v| v as f32).collect(), ideal: ideal.iter().map(|&v| v as f32).collect(),
            sigma, fwhm_px, lambda_um: band.spec.lambda_um(),
        });
    }
    Observation { ins: ins.clone(), w, h, pix_arcsec: pa, maps: out, log: vec![] }
}

fn correlated_noise(w: usize, h: usize, sig_px: f64, sigma: f64, rng: &mut Rng) -> Vec<f64> {
    let white: Vec<f64> = (0..w * h).map(|_| rng.normal()).collect();
    let c = convolve(&white, w, h, sig_px);
    let sd = (c.iter().map(|v| v * v).sum::<f64>() / c.len() as f64).sqrt().max(1e-30);
    c.iter().map(|v| v / sd * sigma).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::Json;

    fn slab(n: usize, nh: f32, x: f32, t: f32) -> MwScene {
        let mut v = MwVolume::new(n, 0.1 * PC, [0.0, 0.0, 1000.0 * PC]);
        for i in 0..n * n * n {
            v.f[F_NH][i] = nh; v.f[F_XION][i] = x; v.f[F_NE][i] = 1.1 * nh * x; v.f[F_TGAS][i] = t; v.f[F_U][i] = 1.0;
            v.f[F_FDUST][i] = if x > 0.5 { 0.0 } else { 1.0 }; v.f[F_LOGU][i] = -3.0;
        }
        MwScene { kind: "pillars".into(), vol: v, sources: vec![], halos: vec![], particles: None, cosmo: None, cluster: None, meta: Json::obj(), seed: 1 }
    }

    #[test]
    fn radio_brightness_of_an_ionised_box_matches_free_free_theory() {
        let n = 16;
        let sc = slab(n, 500.0, 1.0, 8000.0);
        let ins = super::super::bands::instrument("radio_cm").unwrap();
        let view = View::new(9, 9, 0.6 * 1.6 * PC / (1000.0 * PC) * 0.3, [0.0, 0.0, 1.0]);
        let m = sky_maps(&sc, &ins, &view, &ObsOpts { spp: 1, seed: 1, noise: false }, &mut |_| {});
        let i_c = m[0][4 * 9 + 4];
        let len = n as f64 * 0.1 * PC;
        let (ne, np) = (1.1 * 500.0, 500.0);
        let nus: Vec<f64> = (0..K_SAMPLES).map(|k| 4e9 * (6.0f64 / 4.0).powf((k as f64 + 0.5) / K_SAMPLES as f64)).collect();
        let expect: f64 = nus.iter().map(|&nu| { let tau = alpha_ff(nu, ne, np, 8000.0) * len; planck_nu(nu, 8000.0) * (1.0 - (-tau).exp()) }).sum::<f64>() / K_SAMPLES as f64;
        assert!((i_c / expect - 1.0).abs() < 0.02, "{i_c:e} vs {expect:e}");
    }

    #[test]
    fn thick_dust_slab_radiates_as_a_blackbody() {
        // optically thick at 850 µm needs a huge column; test at 21 µm with U set for a known T_d
        let n = 16;
        let mut sc = slab(n, 1e8, 0.0, 20.0);
        for i in 0..n * n * n { sc.vol.f[F_U][i] = 1e4; sc.vol.f[F_FPAH][i] = 0.0; }
        // remove VSG contribution by zeroing U-dependent small grains: compare against B_ν(T_d) with VSG added
        let ins = super::super::bands::instrument("miri").unwrap();
        let view = View::new(5, 5, 0.6 * 1.6 / 1000.0 * 0.2, [0.0, 0.0, 1.0]);
        let m = sky_maps(&sc, &ins, &view, &ObsOpts { spp: 1, seed: 1, noise: false }, &mut |_| {});
        let b = &ins.bands[3];
        let (a, c) = b.spec.nu_range();
        let td = dust_temperature(1e4);
        // the renderer's source function: band-mean j over band-mean α
        let (mut js, mut al) = (0.0, 0.0);
        for k in 0..K_SAMPLES {
            let nu = a * (c / a).powf((k as f64 + 0.5) / K_SAMPLES as f64);
            let l = C / nu * 1e4;
            js += sigma_abs(l) * planck_nu(nu, td) + 1e4 * vsg_per_h(l);
            al += sigma_ext(l);
        }
        let s = js / al;
        let got = m[3][2 * 5 + 2];
        assert!((got / s - 1.0).abs() < 0.05, "{got:e} vs {s:e}");
    }

    #[test]
    fn cluster_sz_matches_the_analytic_beta_model() {
        // projected y at the centre of a β = 2/3 cluster, truncated at 3 R500
        let hl = Halo { pos: [0.0, 0.0, 200.0 * MPC], m500: 5e14, r500: 1.0 * MPC, rc: 0.2 * MPC, beta: 2.0 / 3.0, ne0: 3e-3, t: 6e7, q: 1.0, pa: 0.0, z: 0.0, radio_halo: 0.0, relics: None, c200: 4.0, m200: 7e14, r200: 1.5 * MPC };
        let mut sc = slab(8, 0.0, 1.0, 1e4);
        sc.vol.center = [0.0, 0.0, 1e40];
        sc.halos.push(hl.clone());
        let ins = super::super::bands::instrument("cmb").unwrap();
        let view = View::new(11, 11, 11.0 * 0.01 * MPC / (200.0 * MPC), [0.0, 0.0, 1.0]);
        let mut maps = vec![vec![0.0; 121]; 3];
        add_halos(&sc, &view, &ins.bands, &mut maps);
        let y = maps[1][5 * 11 + 5];
        // analytic: y(0) = σT kT/mec² ne0 ∫ (1 + l²/rc²)^-1 dl over |l| < 3 R500 = 2 rc atan(3R500/rc)
        let expect = SIGMA_T * K * 6e7 / ME_C2 * 3e-3 * 2.0 * 0.2 * MPC * (15.0f64).atan();
        assert!((y / expect - 1.0).abs() < 0.03, "{y:e} vs {expect:e}");
    }
}
