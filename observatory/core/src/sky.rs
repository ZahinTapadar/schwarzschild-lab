//! Statistically synthesized sky: stars, unresolved starlight, dust, galaxies.
//!
//! This is the same population model as the real-time renderer's "calibrated
//! synthetic sky" (thin/thick disc, bar, halo; Kroupa mass function; dust disc
//! with lognormal clouds), but generated once per scene into an explicit
//! catalogue instead of being re-drawn per pixel:
//!
//! * a **star catalogue** down to a magnitude limit, Poisson-sampled per
//!   cube-sphere cell from the line-of-sight integral (*statistically
//!   synthesized*);
//! * an all-sky **diffuse map** of the stars fainter than that limit, plus
//!   dust-scattered light (*numerically approximated*);
//! * a **galaxy catalogue** from Euclidean number counts modulated by a
//!   lognormal large-scale field (*statistically synthesized*; the field is a
//!   fast stand-in for large-scale structure, not an N-body result).
//!
//! It is one realisation of a model, not a map of the real sky.

use crate::math::V3;
use crate::rng::{hash, Rng};
use crate::spectrum::{bin_center, extinction_ratio, planck_photon, NB, V0_PHOTON_FLUX};

// ------------------------------------------------------- galactic frame --
// Fixed orientation of the model galaxy in world coordinates (same as the
// real-time renderer): north pole N, Sun→centre C, E = N × C.
fn nrm(v: V3) -> V3 { v.norm() }
pub fn gal_frame() -> (V3, V3, V3) {
    let n = nrm(V3::new(0.18, 1.0, -0.42));
    let c0 = V3::new(-0.9, 0.0, 0.35);
    let c = nrm(c0 - n * c0.dot(n));
    (n.cross(c), n, c)
}

// --------------------------------------------------------- model params --
const R0: f64 = 8.2;
const ZSUN: f64 = 0.02;
const HR_THIN: f64 = 2.6;
const HZ_THIN: f64 = 0.30;
const HOLE: f64 = 3.0;
const N_THICK: f64 = 0.12;
const HR_THICK: f64 = 2.0;
const HZ_THICK: f64 = 0.9;
const N_BULGE: f64 = 120.0;
const BAR: f64 = 0.47;
const BX: f64 = 1.2;
const BY: f64 = 0.45;
const BZ: f64 = 0.35;
const N_HALO: f64 = 0.0015;
const RC_HALO: f64 = 0.5;
const Q_HALO: f64 = 0.7;
const P_HALO: f64 = 2.5;
pub const N0: f64 = 4.2e7; // stars kpc⁻³ near the Sun (calibrated to N(<V))
const FG_Y: f64 = 0.003;
const FG_O: f64 = 0.006;
const HR_DUST: f64 = 3.2;
const HZ_DUST: f64 = 0.12;
const KAPPA_V: f64 = 0.74;
const NSHELL: usize = 12;
const SHELL_R0: f64 = 0.005;
fn shell_dln() -> f64 { (8000.0f64).ln() / NSHELL as f64 }

fn gal_pos(g: V3, r: f64) -> V3 { V3::new(R0 - r * g.z, r * g.x, ZSUN + r * g.y) }
fn rho_thin(p: V3) -> f64 {
    let rr = (p.x * p.x + p.y * p.y).sqrt();
    let hole = (1.0 - (-(rr * rr) / (HOLE * HOLE)).exp()) / (1.0 - (-(R0 * R0) / (HOLE * HOLE)).exp());
    (-(rr - R0) / HR_THIN - (p.z.abs() - ZSUN) / HZ_THIN).exp() * hole
}
fn rho_thick(p: V3) -> f64 {
    N_THICK * (-((p.x * p.x + p.y * p.y).sqrt() - R0) / HR_THICK - (p.z.abs() - ZSUN) / HZ_THICK).exp()
}
fn rho_bulge(p: V3) -> f64 {
    let (c, s) = (BAR.cos(), BAR.sin());
    let b = V3::new((p.x * c + p.y * s) / BX, (-p.x * s + p.y * c) / BY, p.z / BZ);
    N_BULGE * (-0.5 * b.dot(b)).exp()
}
fn rho_halo(p: V3) -> f64 {
    let rh2 = p.x * p.x + p.y * p.y + (p.z / Q_HALO).powi(2);
    N_HALO * ((rh2 + RC_HALO * RC_HALO) / (R0 * R0 + RC_HALO * RC_HALO)).powf(-0.5 * P_HALO)
}
fn rho_dust(p: V3) -> f64 {
    (-((p.x * p.x + p.y * p.y).sqrt() - R0) / HR_DUST - (p.z.abs() - ZSUN) / HZ_DUST).exp()
}

// Kroupa present-day mass functions (see the real-time README / devlog).
fn seg_n(a: f64, m1: f64, m2: f64) -> f64 { (m1.powf(1.0 - a) - m2.powf(1.0 - a)) / (a - 1.0) }
fn seg_l(a: f64, m1: f64, m2: f64) -> f64 { let e = 4.5 - a; (m2.powf(e) - m1.powf(e)) / e }
fn tot(young: bool) -> f64 {
    seg_n(1.3, 0.08, 0.5) + 0.5 * seg_n(2.3, 0.5, if young { 1.0 } else { 0.95 }) + if young { 0.5 * seg_n(4.8, 1.0, 60.0) } else { 0.0 }
}
fn pdmf_tail(mt: f64, young: bool) -> f64 {
    let mut t = 0.0;
    if mt < 0.5 { t += seg_n(1.3, mt.max(0.08), 0.5); }
    let top = if young { 1.0 } else { 0.95 };
    if mt < top { t += 0.5 * seg_n(2.3, mt.max(0.5), top); }
    if young && mt < 60.0 { t += 0.5 * seg_n(4.8, mt.max(1.0), 60.0); }
    t / tot(young)
}
fn mass_of_mv(mv: f64) -> f64 { 10f64.powf((4.83 - mv) / 8.75) }
/// Fraction of stars brighter than absolute magnitude mv.
fn lf_cum(mv: f64, young: bool) -> f64 {
    let fg = if young { FG_Y } else { FG_O };
    (1.0 - fg) * pdmf_tail(mass_of_mv(mv), young) + fg * ((mv + 1.5) / 3.0).clamp(0.0, 1.0)
}
/// Mean luminosity (L_V,⊙) per star from stars fainter than mv.
fn lf_lum_fainter(mv: f64, young: bool) -> f64 {
    let mt = mass_of_mv(mv);
    let mut l = seg_l(1.3, 0.08, mt.clamp(0.08, 0.5));
    let top = if young { 1.0 } else { 0.95 };
    if mt > 0.5 { l += 0.5 * seg_l(2.3, 0.5, mt.min(top)); }
    if young && mt > 1.0 { l += 0.5 * seg_l(4.8, 1.0, mt.min(60.0)); }
    let fg = if young { FG_Y } else { FG_O };
    let lo = mv.clamp(-1.5, 1.5);
    let lg = (10f64.powf(-0.4 * (lo - 4.83)) - 10f64.powf(-0.4 * (1.5 - 4.83))) / (0.921_034 * 3.0);
    (1.0 - fg) * l / tot(young) + fg * lg
}
fn teff_ms(m: f64) -> f64 { 5778.0 * if m < 1.0 { m.powf(0.35) } else { m.powf(0.6) } }

// ---------------------------------------------------------------- noise --
fn grad(seed: u64, c: [i64; 3]) -> V3 {
    let h = hash(&[seed, c[0] as u64, c[1] as u64, c[2] as u64]);
    let f = |s: u32| ((h >> s) & 0xFFFF) as f64 / 32767.5 - 1.0;
    V3::new(f(0), f(16), f(32))
}
fn gnoise(seed: u64, p: V3) -> f64 {
    let i = [p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64];
    let f = V3::new(p.x - i[0] as f64, p.y - i[1] as f64, p.z - i[2] as f64);
    let w = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (wx, wy, wz) = (w(f.x), w(f.y), w(f.z));
    let mut acc = [0.0; 8];
    for (k, a) in acc.iter_mut().enumerate() {
        let o = [(k & 1) as i64, ((k >> 1) & 1) as i64, ((k >> 2) & 1) as i64];
        let g = grad(seed, [i[0] + o[0], i[1] + o[1], i[2] + o[2]]);
        *a = g.dot(f - V3::new(o[0] as f64, o[1] as f64, o[2] as f64));
    }
    let lx = |a: f64, b: f64| a + (b - a) * wx;
    let y0 = lx(acc[0], acc[1]) + (lx(acc[2], acc[3]) - lx(acc[0], acc[1])) * wy;
    let y1 = lx(acc[4], acc[5]) + (lx(acc[6], acc[7]) - lx(acc[4], acc[5])) * wy;
    y0 + (y1 - y0) * wz
}
/// Approximately unit-variance, domain-warped fractal field. Used only to
/// perturb physical densities (dust clouds, large-scale structure).
pub fn gauss_field(seed: u64, g: V3, scale: f64, octaves: u32, ridge: f64) -> f64 {
    let mut q = g * scale;
    let warp = V3::new(gnoise(seed ^ 11, q * 0.5), gnoise(seed ^ 12, q * 0.5), gnoise(seed ^ 13, q * 0.5));
    q = q + warp * 0.9;
    let (mut sp, mut sr, mut a, mut an) = (0.0, 0.0, 1.0, 0.0);
    for i in 0..octaves {
        let n = gnoise(seed.wrapping_add(i as u64 * 7919), q);
        sp += a * n;
        sr += a * (0.2 - n.abs());
        an += a * a;
        q = V3::new(0.8 * q.y + 0.6 * q.z, -0.8 * q.x + 0.36 * q.y - 0.48 * q.z, -0.6 * q.x - 0.48 * q.y + 0.64 * q.z) * 2.03;
        a *= 0.55;
    }
    ((1.0 - ridge) * sp / 0.25 + ridge * sr / 0.15) / an.sqrt()
}

// ------------------------------------------------------ line of sight ---
pub struct Los {
    pub mu: [f64; NSHELL],
    pub a_v: [f64; NSHELL],
    pub wy: [f64; NSHELL],
    pub wo: [f64; NSHELL],
    pub r: [f64; NSHELL],
    pub tau_total: f64,
    pub cloud: f64,
}

/// Line-of-sight model in galactic-frame direction g (x = E, y = N, z = C).
pub fn los(seed: u64, g: V3, dust_scale: f64, density_scale: f64) -> Los {
    let sig = 0.35 + 0.65 * (-g.y.abs() / 0.15).exp();
    let cloud = (sig * gauss_field(seed ^ 100, g, 4.5, 4, 0.6) - 0.5 * sig * sig).exp().min(12.0);
    let mut l = Los { mu: [0.0; NSHELL], a_v: [0.0; NSHELL], wy: [0.0; NSHELL], wo: [0.0; NSHELL], r: [0.0; NSHELL], tau_total: 0.0, cloud };
    let dln = shell_dln();
    let mut tau = 0.0;
    for k in 0..NSHELL {
        let r = SHELL_R0 * ((k as f64 + 0.5) * dln).exp();
        let dr = r * dln;
        let p = gal_pos(g, r);
        let dtau = KAPPA_V * dust_scale * rho_dust(p) * cloud * dr;
        l.r[k] = r;
        l.mu[k] = 5.0 * r.log10() + 10.0;
        l.a_v[k] = 1.086 * (tau + 0.5 * dtau);
        tau += dtau;
        let w = N0 * density_scale * r * r * dr;
        l.wy[k] = w * rho_thin(p);
        l.wo[k] = w * (rho_thick(p) + rho_bulge(p) + rho_halo(p));
    }
    l.tau_total = tau;
    l
}

impl Los {
    /// Stars per steradian brighter than apparent magnitude m.
    pub fn count_brighter(&self, m: f64) -> f64 {
        (0..NSHELL)
            .map(|k| {
                let off = self.mu[k] + self.a_v[k];
                self.wy[k] * lf_cum(m - off, true) + self.wo[k] * lf_cum(m - off, false)
            })
            .sum()
    }
}

// ------------------------------------------------------------ cube sphere --
pub fn cube_coords(d: V3) -> (usize, f64, f64) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let (face, u, v) = if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z / ax, d.y / ax) } else { (1, d.z / ax, d.y / ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x / ay, -d.z / ay) } else { (3, d.x / ay, d.z / ay) }
    } else if d.z > 0.0 {
        (4, d.x / az, d.y / az)
    } else {
        (5, -d.x / az, d.y / az)
    };
    let k = 4.0 / std::f64::consts::PI;
    (face, u.atan() * k, v.atan() * k)
}
pub fn cube_dir(face: usize, a: f64, b: f64) -> V3 {
    let q = std::f64::consts::FRAC_PI_4;
    let (u, v) = ((a * q).tan(), (b * q).tan());
    let d = match face {
        0 => V3::new(1.0, v, -u),
        1 => V3::new(-1.0, v, u),
        2 => V3::new(u, 1.0, -v),
        3 => V3::new(u, -1.0, v),
        4 => V3::new(u, v, 1.0),
        _ => V3::new(-u, v, -1.0),
    };
    d.norm()
}
fn cell_solid_angle(a: f64, b: f64, n: f64) -> f64 {
    let q = std::f64::consts::FRAC_PI_4;
    let (u, v) = ((a * q).tan(), (b * q).tan());
    let s = 1.0 + u * u + v * v;
    0.616_850 * (1.0 + u * u) * (1.0 + v * v) / (s * s.sqrt()) * 4.0 / (n * n)
}
pub fn cell_index(d: V3, level: u32) -> usize {
    let n = 1usize << level;
    let (f, a, b) = cube_coords(d);
    let i = (((a * 0.5 + 0.5) * n as f64) as usize).min(n - 1);
    let j = (((b * 0.5 + 0.5) * n as f64) as usize).min(n - 1);
    (f * n + j) * n + i
}


/// Edges of the magnitude bins used to Poisson-sample stars.
const MAG_EDGES: [f64; 13] = [-3.0, 1.5, 4.0, 6.5, 8.5, 10.5, 12.5, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0];

/// Temperature and line-of-sight A_V of a star of apparent magnitude m drawn
/// from line of sight l: pick the shell and population in proportion to
/// their contribution at m, then a main-sequence or giant temperature.
fn draw_star(l: &Los, m: f64, rng: &mut Rng) -> (f64, f64, f64) {
    // which shell and population produced it
    let mut wts = [0.0; 2 * NSHELL];
    let mut sum = 0.0;
    for k in 0..NSHELL {
        let off = l.mu[k] + l.a_v[k];
        let dm = 0.05;
        wts[2 * k] = l.wy[k] * (lf_cum(m + dm - off, true) - lf_cum(m - dm - off, true)).max(0.0);
        wts[2 * k + 1] = l.wo[k] * (lf_cum(m + dm - off, false) - lf_cum(m - dm - off, false)).max(0.0);
        sum += wts[2 * k] + wts[2 * k + 1];
    }
    let mut pick = rng.f64() * sum;
    let mut sel = 0;
    for (q, w) in wts.iter().enumerate() {
        if pick < *w { sel = q; break; }
        pick -= w;
    }
    let k = sel / 2;
    let young = sel % 2 == 0;
    let mv = m - l.mu[k] - l.a_v[k];
    let fg = if young { FG_Y } else { FG_O };
    let p_ms = (1.0 - fg) * (pdmf_tail(mass_of_mv(mv - 0.05), young) - pdmf_tail(mass_of_mv(mv + 0.05), young)).max(0.0) / 0.1;
    let p_g = if mv > -1.5 && mv < 1.5 { fg / 3.0 } else { 0.0 };
    let t = if rng.f64() * (p_ms + p_g) < p_g { 3900.0 + 1200.0 * rng.f64() } else { teff_ms(mass_of_mv(mv)).min(45000.0) * if young { 1.0 } else { 1.03 } };
    (t, l.a_v[k], l.r[k])
}

/// Sample stars of magnitudes up to `limit` in a patch of solid angle `om`
/// with line of sight `l`; `pos` draws a uniform position in the patch.
fn sample_stars(l: &Los, om: f64, limit: f64, rng: &mut Rng, pos: &mut dyn FnMut(&mut Rng) -> V3, out: &mut Vec<Star>) {
    let mut prev = l.count_brighter(MAG_EDGES[0]);
    for w in MAG_EDGES.windows(2) {
        let (ma, mb) = (w[0], w[1].min(limit));
        if ma >= mb { break; }
        let nb = l.count_brighter(mb);
        let lam = (nb - prev) * om;
        prev = nb;
        let count = rng.poisson(lam) as usize;
        for _ in 0..count {
            let dir = pos(rng);
            // magnitude: dN/dm ∝ 10^(0.3 m) within the bin
            let bb = 0.3 * std::f64::consts::LN_10;
            let m = ma + (1.0 + rng.f64() * ((bb * (mb - ma)).exp() - 1.0)).ln() / bb;
            let (t, a_v, d) = draw_star(l, m, rng);
            out.push(Star { dir: [dir.x as f32, dir.y as f32, dir.z as f32], v: m as f32, t: t as f32, a_v: a_v as f32, d_kpc: d as f32 });
        }
    }
}

/// Galaxy number counts per steradian brighter than m: Euclidean,
/// 1500 sr⁻¹ 10^(0.6(m − 15)), flattening to slope 0.45 beyond m = 20
/// (*statistically synthesized*, close to observed B-band counts).
pub fn galaxies_brighter_per_sr(m: f64) -> f64 {
    if m <= 20.0 { 1500.0 * 10f64.powf(0.6 * (m - 15.0)) } else { 1500.0 * 10f64.powf(3.0 + 0.45 * (m - 20.0)) }
}

/// Draw a magnitude in [lo, hi] from the count law above.
fn draw_galaxy_mag(lo: f64, hi: f64, rng: &mut Rng) -> f64 {
    let (a, b) = (galaxies_brighter_per_sr(lo), galaxies_brighter_per_sr(hi));
    let target = a + rng.f64() * (b - a);
    // invert piecewise power law
    if target <= galaxies_brighter_per_sr(20.0) { 15.0 + (target / 1500.0).log10() / 0.6 } else { 20.0 + ((target / 1500.0).log10() - 3.0) / 0.45 }
}

/// A galaxy at direction dir and magnitude m (extinction added later).
fn draw_galaxy(dir: V3, m: f64, rng: &mut Rng) -> Galaxy {
    let mabs = -20.5 + 1.6 * (rng.f64() + rng.f64() - 1.0);
    let r_mpc = 10f64.powf((m - mabs - 25.0) / 5.0);
    let z = 70.0 * r_mpc / 299_792.0;
    let re_kpc = 4.0 * 10f64.powf(-0.2 * (mabs + 20.5)) * (0.6 + 0.8 * rng.f64());
    // angular size uses D_A = D_L / (1 + z)²
    let re = re_kpc / (r_mpc * 1000.0) * (1.0 + z).powi(2);
    let kind = if rng.f64() < 0.3 { 0 } else { 1 };
    let ci = rng.f64();
    let q = if kind == 0 { 1.0 - 0.5 * rng.f64() } else { (ci * ci * (1.0 - 0.0225) + 0.0225).sqrt() };
    Galaxy { dir: [dir.x as f32, dir.y as f32, dir.z as f32], mag: m as f32, re: re as f32, q: q as f32, pa: (rng.f64() * std::f64::consts::PI) as f32, kind, z: z as f32, seed: rng.next_u32() }
}

// ---------------------------------------------------------------- deep field --
/// A deeper catalogue inside a cone around the field centre, indexed on a
/// gnomonic (tangent-plane) grid fine enough for arcsecond pixels. Outside the
/// cone the all-sky catalogue applies; inside it the all-sky catalogue is
/// suppressed, so no star is counted twice.
pub struct DeepField {
    pub center: V3,
    pub t1: V3,
    pub t2: V3,
    pub radius: f64,
    pub star_limit: f64,
    pub gal_limit: f64,
    /// Half-width of the tangent-plane square (tan radius) and grid size.
    pub half: f64,
    pub n: usize,
    pub stars: Vec<Star>,
    pub star_cells: Vec<u32>,
    pub galaxies: Vec<Galaxy>,
    /// Galaxy indices per cell (a galaxy is listed in every cell its profile touches).
    pub gal_refs: Vec<u32>,
    pub gal_cells: Vec<u32>,
}

pub struct DeepParams {
    pub center: V3,
    /// Half-width of the covered square on the tangent plane, as an angle.
    pub radius: f64,
    pub star_limit: f64,
    pub gal_limit: f64,
    /// Target index cell size (rad).
    pub cell: f64,
}

impl DeepParams {
    /// Inside the covered square (gnomonic |x|, |y| ≤ tan radius)?
    pub fn covers(&self, d: V3) -> bool {
        let (t1, t2) = crate::math::basis(self.center);
        let c = d.dot(self.center);
        let h = self.radius.tan();
        c > 0.0 && (d.dot(t1) / c).abs() <= h && (d.dot(t2) / c).abs() <= h
    }
}

impl DeepField {
    /// Tangent-plane coordinates of d, if inside the cone.
    #[inline]
    pub fn project(&self, d: V3) -> Option<(f64, f64)> {
        let c = d.dot(self.center);
        if c <= 0.0 { return None; }
        let (x, y) = (d.dot(self.t1) / c, d.dot(self.t2) / c);
        (x.abs() <= self.half && y.abs() <= self.half).then_some((x, y))
    }

    /// For a direction outside the covered square: the same position modulo
    /// the square (tangent-plane tiling; the far hemisphere is mirrored
    /// through the centre). Returns the tile coordinates and the equivalent
    /// direction inside the square. Used only for faint sources, as a
    /// statistical stand-in for the deep sky beyond the generated patch.
    pub fn wrap(&self, d: V3) -> (f64, f64, V3) {
        let d = if d.dot(self.center) > 1e-6 { d } else { -d };
        let c = d.dot(self.center).max(1e-6);
        let w = |v: f64| { let p = 2.0 * self.half; ((v + self.half) % p + p) % p - self.half };
        let (x, y) = (w(d.dot(self.t1) / c), w(d.dot(self.t2) / c));
        (x, y, (self.center + self.t1 * x + self.t2 * y).norm())
    }
    #[inline]
    pub fn cell_of(&self, x: f64, y: f64) -> Option<usize> {
        let fx = (x + self.half) / (2.0 * self.half) * self.n as f64;
        let fy = (y + self.half) / (2.0 * self.half) * self.n as f64;
        if fx < 0.0 || fy < 0.0 || fx >= self.n as f64 || fy >= self.n as f64 { return None; }
        Some(fy as usize * self.n + fx as usize)
    }
}

// --------------------------------------------------------------- catalogue --
#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub dir: [f32; 3],
    /// Apparent V magnitude including extinction.
    pub v: f32,
    /// Effective temperature (K) and line-of-sight A_V (for reddening).
    pub t: f32,
    pub a_v: f32,
    /// Distance (kpc) of the line-of-sight shell the star was drawn from.
    pub d_kpc: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Galaxy {
    pub dir: [f32; 3],
    pub mag: f32,
    /// Half-light radius (rad), axis ratio, position angle, type (0 elliptical, 1 disc), redshift z.
    pub re: f32,
    pub q: f32,
    pub pa: f32,
    pub kind: u8,
    pub z: f32,
    pub seed: u32,
}

pub struct SkyParams {
    pub seed: u64,
    pub mag_limit: f64,
    pub dust: f64,
    pub density: f64,
    pub galaxy_mag_limit: f64,
    pub galaxy_density: f64,
    pub diffuse_w: usize,
    pub deep: Option<DeepParams>,
}

impl Default for SkyParams {
    fn default() -> Self {
        SkyParams { seed: 1977, mag_limit: 10.5, dust: 1.0, density: 1.0, galaxy_mag_limit: 14.0, galaxy_density: 1.0, diffuse_w: 512, deep: None }
    }
}

pub struct Sky {
    pub frame: (V3, V3, V3),
    pub stars: Vec<Star>,
    /// Stars sorted by cell at INDEX_LEVEL; offsets into `stars`.
    pub star_cells: Vec<u32>,
    pub galaxies: Vec<Galaxy>,
    pub gal_cells: Vec<u32>,
    /// Diffuse photon radiance (per bin) on an equirectangular grid in galactic
    /// longitude/latitude, w × w/2.
    pub diffuse: Vec<[f32; NB]>,
    pub dw: usize,
    pub av_map: Vec<f32>,
    pub params_mag_limit: f64,
    pub deep: Option<DeepField>,
    /// Cached pole means of the diffuse map (see `refresh_poles`).
    pub poles: [[f32; NB]; 2],
}

pub const INDEX_LEVEL: u32 = 7;
pub const GAL_LEVEL: u32 = 5;
const GEN_LEVEL: u32 = 6;

impl Sky {
    pub fn to_gal(&self, d: V3) -> V3 {
        let (e, n, c) = self.frame;
        V3::new(d.dot(e), d.dot(n), d.dot(c))
    }
    pub fn from_gal(&self, g: V3) -> V3 {
        let (e, n, c) = self.frame;
        e * g.x + n * g.y + c * g.z
    }

    pub fn generate(p: &SkyParams) -> Sky {
        let frame = gal_frame();
        let mut sky = Sky { frame, stars: Vec::new(), star_cells: Vec::new(), galaxies: Vec::new(), gal_cells: Vec::new(), diffuse: Vec::new(), dw: p.diffuse_w, av_map: Vec::new(), params_mag_limit: p.mag_limit, deep: None, poles: [[0.0; NB]; 2] };
        sky.gen_stars(p);
        // with a deep field, unresolved light everywhere is "fainter than the
        // deep limit", so demagnified images of directions outside the cone do
        // not show a smooth glow of stars the cone resolves into points
        let diffuse_limit = p.deep.as_ref().map_or(p.mag_limit, |d| d.star_limit.max(p.mag_limit));
        sky.gen_diffuse(p, diffuse_limit);
        sky.gen_galaxies(p);
        if let Some(dp) = &p.deep { sky.gen_deep(p, dp); }
        sky.refresh_poles();
        sky
    }

    /// Deep catalogue inside the field cone (see `DeepField`).
    fn gen_deep(&mut self, p: &SkyParams, dp: &DeepParams) {
        let (t1, t2) = crate::math::basis(dp.center);
        let half = dp.radius.tan();
        let n = ((2.0 * half / dp.cell).ceil() as usize).clamp(8, 1024);
        // generation patches: G × G over the tangent square
        let g = 24usize;
        let mut stars = Vec::new();
        let mut gals = Vec::new();
        let pd = |x: f64, y: f64| (dp.center + t1 * x + t2 * y).norm();
        for j in 0..g {
            for i in 0..g {
                let (x0, y0) = (-half + 2.0 * half * i as f64 / g as f64, -half + 2.0 * half * j as f64 / g as f64);
                let w = 2.0 * half / g as f64;
                let (xc, yc) = (x0 + 0.5 * w, y0 + 0.5 * w);
                // gnomonic Jacobian: dΩ = dx dy / (1 + x² + y²)^{3/2}
                let om = w * w / (1.0 + xc * xc + yc * yc).powf(1.5);
                let dc = pd(xc, yc);
                let l = los(p.seed, self.to_gal(dc), p.dust, p.density);
                let mut rng = Rng::keyed(&[p.seed, 0xDEE9, j as u64, i as u64]);
                let mut pos = |rng: &mut Rng| pd(x0 + w * rng.f64(), y0 + w * rng.f64());
                let mut patch = Vec::new();
                sample_stars(&l, om, dp.star_limit, &mut rng, &mut pos, &mut patch);
                stars.extend(patch.into_iter().filter(|s| dp.covers(V3::new(s.dir[0] as f64, s.dir[1] as f64, s.dir[2] as f64))));
                // galaxies: same law as the all-sky catalogue, deeper
                let gg = self.to_gal(dc);
                let dens = (0.75 * gauss_field(p.seed ^ 200, gg, 2.2, 3, 0.0) + 0.66 * gauss_field(p.seed ^ 210, gg, 9.0, 2, 0.3) - 0.5).exp();
                let av = 1.086 * l.tau_total;
                let hide = 10f64.powf(-0.6 * 1.32 * av);
                let lam = galaxies_brighter_per_sr(dp.gal_limit) * om * dens * hide * p.galaxy_density;
                let mut rg = Rng::keyed(&[p.seed, 0xDEE6, j as u64, i as u64]);
                let count = rg.poisson(lam) as usize;
                for _ in 0..count {
                    let dir = pd(x0 + w * rg.f64(), y0 + w * rg.f64());
                    let m = draw_galaxy_mag(6.0, dp.gal_limit, &mut rg);
                    let mut gx = draw_galaxy(dir, m, &mut rg);
                    gx.mag += av as f32;
                    if dp.covers(dir) { gals.push(gx); }
                }
            }
        }
        // star index on the n × n grid
        let cell_of = |d: V3| -> usize {
            let c = d.dot(dp.center);
            let (x, y) = (d.dot(t1) / c, d.dot(t2) / c);
            let fx = (((x + half) / (2.0 * half) * n as f64) as usize).min(n - 1);
            let fy = (((y + half) / (2.0 * half) * n as f64) as usize).min(n - 1);
            fy * n + fx
        };
        let key = |s: &Star| cell_of(V3::new(s.dir[0] as f64, s.dir[1] as f64, s.dir[2] as f64));
        stars.sort_by_key(key);
        let mut offs = vec![0u32; n * n + 1];
        for s in &stars { offs[key(s) + 1] += 1; }
        for i in 0..n * n { offs[i + 1] += offs[i]; }
        // galaxy references: every cell within 8 r_e (≥ one cell) of the centre
        let cellw = 2.0 * half / n as f64;
        let mut refs: Vec<Vec<u32>> = vec![Vec::new(); n * n];
        for (gi, gx) in gals.iter().enumerate() {
            let d = V3::new(gx.dir[0] as f64, gx.dir[1] as f64, gx.dir[2] as f64);
            let c = d.dot(dp.center);
            let (x, y) = (d.dot(t1) / c, d.dot(t2) / c);
            let ext = 8.0 * (gx.re as f64).max(dp.cell);
            let k = (ext / cellw).ceil() as i64;
            let (cx, cy) = (((x + half) / cellw) as i64, ((y + half) / cellw) as i64);
            for yy in (cy - k).max(0)..=(cy + k).min(n as i64 - 1) {
                for xx in (cx - k).max(0)..=(cx + k).min(n as i64 - 1) {
                    refs[yy as usize * n + xx as usize].push(gi as u32);
                }
            }
        }
        let mut gal_cells = vec![0u32; n * n + 1];
        let mut gal_refs = Vec::new();
        for (c, r) in refs.iter().enumerate() { gal_refs.extend_from_slice(r); gal_cells[c + 1] = gal_refs.len() as u32; }
        self.deep = Some(DeepField { center: dp.center, t1, t2, radius: dp.radius, star_limit: dp.star_limit, gal_limit: dp.gal_limit, half, n, stars, star_cells: offs, galaxies: gals, gal_refs, gal_cells });
    }

    fn gen_stars(&mut self, p: &SkyParams) {
        let n = 1usize << GEN_LEVEL;
        let mut stars = Vec::new();
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    let a = (i as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                    let b = (j as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                    let d = cube_dir(face, a, b);
                    let g = self.to_gal(d);
                    let l = los(p.seed, g, p.dust, p.density);
                    let om = cell_solid_angle(a, b, n as f64);
                    let mut rng = Rng::keyed(&[p.seed, 0x57A2, face as u64, j as u64, i as u64]);
                    // position uniform in the cell (equal-angle coordinates)
                    let mut pos = |rng: &mut Rng| {
                        let ua = a + (rng.f64() - 0.5) * 2.0 / n as f64;
                        let ub = b + (rng.f64() - 0.5) * 2.0 / n as f64;
                        cube_dir(face, ua, ub)
                    };
                    sample_stars(&l, om, p.mag_limit, &mut rng, &mut pos, &mut stars);
                }
            }
        }
        if let Some(dp) = &p.deep {
                stars.retain(|s| !dp.covers(V3::new(s.dir[0] as f64, s.dir[1] as f64, s.dir[2] as f64)));
        }
        // spatial index
        let nc = 6 << (2 * INDEX_LEVEL);
        let key = |s: &Star| cell_index(V3::new(s.dir[0] as f64, s.dir[1] as f64, s.dir[2] as f64), INDEX_LEVEL);
        stars.sort_by_key(key);
        let mut offs = vec![0u32; nc + 1];
        for s in &stars { offs[key(s) + 1] += 1; }
        for i in 0..nc { offs[i + 1] += offs[i]; }
        self.stars = stars;
        self.star_cells = offs;
    }

    fn gen_diffuse(&mut self, p: &SkyParams, mag_limit: f64) {
        let w = p.diffuse_w;
        let h = w / 2;
        // mean all-sky starlight (for scattered light), from the same model
        let mut j_mean = [0.0; NB];
        let ns = 2000;
        for i in 0..ns {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / ns as f64;
            let rr = (1.0 - y * y).sqrt();
            let ph = i as f64 * std::f64::consts::PI * (3.0 - 5f64.sqrt());
            let g = V3::new(rr * ph.cos(), y, rr * ph.sin());
            let s = starlight(&los(p.seed, g, p.dust, p.density), -10.0, false);
            for k in 0..NB { j_mean[k] += s[k] / ns as f64; }
        }
        let mut out = vec![[0f32; NB]; w * h];
        let mut av = vec![0f32; w * h];
        for y in 0..h {
            let b = std::f64::consts::PI * (0.5 - (y as f64 + 0.5) / h as f64);
            for x in 0..w {
                let l = 2.0 * std::f64::consts::PI * ((x as f64 + 0.5) / w as f64 - 0.5);
                let g = V3::new(b.cos() * l.sin(), b.sin(), b.cos() * l.cos());
                let los_ = los(p.seed, g, p.dust, p.density);
                let s = starlight(&los_, mag_limit, true);
                let dgl = 1.0 - (-los_.tau_total).exp();
                let cell = &mut out[y * w + x];
                for k in 0..NB {
                    // diffuse galactic light: single scattering of the mean
                    // starlight, albedo·phase ≈ 0.3, bluer by ~λ⁻¹
                    let sca = 0.3 * j_mean[k] * dgl * (550.0 / bin_center(k));
                    cell[k] = (s[k] + sca) as f32;
                }
                av[y * w + x] = (1.086 * los_.tau_total) as f32;
            }
        }
        self.diffuse = out;
        self.av_map = av;
    }

    fn gen_galaxies(&mut self, p: &SkyParams) {
        // N(<m) = 1500 sr⁻¹ 10^(0.6(m − 15)) all-sky, on GAL_LEVEL cells
        let n = 1usize << GAL_LEVEL;
        let mut gals = Vec::new();
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    let a = (i as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                    let b = (j as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                    let d = cube_dir(face, a, b);
                    let g = self.to_gal(d);
                    let dens = (0.75 * gauss_field(p.seed ^ 200, g, 2.2, 3, 0.0) + 0.66 * gauss_field(p.seed ^ 210, g, 9.0, 2, 0.3) - 0.5).exp();
                    let av = 1.086 * los(p.seed, g, p.dust, p.density).tau_total;
                    let hide = 10f64.powf(-0.6 * 1.32 * av);
                    let om = cell_solid_angle(a, b, n as f64);
                    let lam = galaxies_brighter_per_sr(p.galaxy_mag_limit) * om * dens * hide * p.galaxy_density;
                    let mut rng = Rng::keyed(&[p.seed, 0x6A1A, face as u64, j as u64, i as u64]);
                    let count = rng.poisson(lam) as usize;
                    for _ in 0..count {
                        let ua = a + (rng.f64() - 0.5) * 2.0 / n as f64;
                        let ub = b + (rng.f64() - 0.5) * 2.0 / n as f64;
                        let dir = cube_dir(face, ua, ub);
                        let m = draw_galaxy_mag(6.0, p.galaxy_mag_limit, &mut rng);
                        let mut gx = draw_galaxy(dir, m, &mut rng);
                        gx.mag += av as f32;
                        gals.push(gx);
                    }
                }
            }
        }
        if let Some(dp) = &p.deep {
                gals.retain(|g| !dp.covers(V3::new(g.dir[0] as f64, g.dir[1] as f64, g.dir[2] as f64)));
        }
        let nc = 6 << (2 * GAL_LEVEL);
        let key = |g: &Galaxy| cell_index(V3::new(g.dir[0] as f64, g.dir[1] as f64, g.dir[2] as f64), GAL_LEVEL);
        gals.sort_by_key(key);
        let mut offs = vec![0u32; nc + 1];
        for g in &gals { offs[key(g) + 1] += 1; }
        for i in 0..nc { offs[i + 1] += offs[i]; }
        self.galaxies = gals;
        self.gal_cells = offs;
    }

    /// Serialise the generated catalogue so it can be built once and shared
    /// (the browser generates it in one worker and hands it to the others).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        let u = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        let f = |b: &mut Vec<u8>, v: f32| b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(b"OBSSKY03");
        u(&mut b, self.stars.len() as u32);
        for s in &self.stars { for v in [s.dir[0], s.dir[1], s.dir[2], s.v, s.t, s.a_v, s.d_kpc] { f(&mut b, v); } }
        u(&mut b, self.star_cells.len() as u32);
        for v in &self.star_cells { u(&mut b, *v); }
        u(&mut b, self.galaxies.len() as u32);
        for g in &self.galaxies {
            for v in [g.dir[0], g.dir[1], g.dir[2], g.mag, g.re, g.q, g.pa, g.z] { f(&mut b, v); }
            u(&mut b, g.kind as u32);
            u(&mut b, g.seed);
        }
        u(&mut b, self.gal_cells.len() as u32);
        for v in &self.gal_cells { u(&mut b, *v); }
        u(&mut b, self.dw as u32);
        for c in &self.diffuse { for v in c { f(&mut b, *v); } }
        for v in &self.av_map { f(&mut b, *v); }
        b.extend_from_slice(&self.params_mag_limit.to_le_bytes());
        match &self.deep {
            None => u(&mut b, 0),
            Some(df) => {
                u(&mut b, 1);
                for v in [df.center.x, df.center.y, df.center.z, df.t1.x, df.t1.y, df.t1.z, df.t2.x, df.t2.y, df.t2.z, df.radius, df.star_limit, df.gal_limit, df.half] {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                u(&mut b, df.n as u32);
                u(&mut b, df.stars.len() as u32);
                for s in &df.stars { for v in [s.dir[0], s.dir[1], s.dir[2], s.v, s.t, s.a_v, s.d_kpc] { f(&mut b, v); } }
                for v in &df.star_cells { u(&mut b, *v); }
                u(&mut b, df.galaxies.len() as u32);
                for g in &df.galaxies {
                    for v in [g.dir[0], g.dir[1], g.dir[2], g.mag, g.re, g.q, g.pa, g.z] { f(&mut b, v); }
                    u(&mut b, g.kind as u32);
                    u(&mut b, g.seed);
                }
                u(&mut b, df.gal_refs.len() as u32);
                for v in &df.gal_refs { u(&mut b, *v); }
                for v in &df.gal_cells { u(&mut b, *v); }
            }
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Sky, String> {
        if b.len() < 12 || &b[..8] != b"OBSSKY03" { return Err("not a sky catalogue".into()); }
        let mut i = 8;
        let mut u = || { let v = u32::from_le_bytes(b[i..i + 4].try_into().unwrap()); i += 4; v };
        let mut take = |n: usize| -> Vec<u32> { (0..n).map(|_| u()).collect() };
        let ns = take(1)[0] as usize;
        let sv = take(ns * 7);
        let fl = |x: u32| f32::from_bits(x);
        let stars = sv.chunks(7).map(|c| Star { dir: [fl(c[0]), fl(c[1]), fl(c[2])], v: fl(c[3]), t: fl(c[4]), a_v: fl(c[5]), d_kpc: fl(c[6]) }).collect();
        let nsc = take(1)[0] as usize;
        let star_cells = take(nsc);
        let ng = take(1)[0] as usize;
        let gv = take(ng * 10);
        let galaxies = gv.chunks(10).map(|c| Galaxy { dir: [fl(c[0]), fl(c[1]), fl(c[2])], mag: fl(c[3]), re: fl(c[4]), q: fl(c[5]), pa: fl(c[6]), z: fl(c[7]), kind: c[8] as u8, seed: c[9] }).collect();
        let ngc = take(1)[0] as usize;
        let gal_cells = take(ngc);
        let dw = take(1)[0] as usize;
        let ncell = dw * (dw / 2);
        let dv = take(ncell * NB);
        let diffuse = dv.chunks(NB).map(|c| { let mut a = [0f32; NB]; for k in 0..NB { a[k] = fl(c[k]); } a }).collect();
        let av_map = take(ncell).into_iter().map(fl).collect();
        let lim = f64::from_le_bytes(b[i..i + 8].try_into().map_err(|_| "truncated")?);
        i += 8;
        let rd_u = |i: &mut usize| { let v = u32::from_le_bytes(b[*i..*i + 4].try_into().unwrap()); *i += 4; v };
        let deep = if rd_u(&mut i) == 0 { None } else {
            let mut f8 = [0f64; 13];
            for v in f8.iter_mut() { *v = f64::from_le_bytes(b[i..i + 8].try_into().unwrap()); i += 8; }
            let n = rd_u(&mut i) as usize;
            let ns = rd_u(&mut i) as usize;
            let sv: Vec<u32> = (0..ns * 7).map(|_| rd_u(&mut i)).collect();
            let stars = sv.chunks(7).map(|c| Star { dir: [fl(c[0]), fl(c[1]), fl(c[2])], v: fl(c[3]), t: fl(c[4]), a_v: fl(c[5]), d_kpc: fl(c[6]) }).collect();
            let star_cells = (0..n * n + 1).map(|_| rd_u(&mut i)).collect();
            let ng = rd_u(&mut i) as usize;
            let gv: Vec<u32> = (0..ng * 10).map(|_| rd_u(&mut i)).collect();
            let galaxies = gv.chunks(10).map(|c| Galaxy { dir: [fl(c[0]), fl(c[1]), fl(c[2])], mag: fl(c[3]), re: fl(c[4]), q: fl(c[5]), pa: fl(c[6]), z: fl(c[7]), kind: c[8] as u8, seed: c[9] }).collect();
            let nr = rd_u(&mut i) as usize;
            let gal_refs = (0..nr).map(|_| rd_u(&mut i)).collect();
            let gal_cells = (0..n * n + 1).map(|_| rd_u(&mut i)).collect();
            Some(DeepField {
                center: V3::new(f8[0], f8[1], f8[2]), t1: V3::new(f8[3], f8[4], f8[5]), t2: V3::new(f8[6], f8[7], f8[8]),
                radius: f8[9], star_limit: f8[10], gal_limit: f8[11], half: f8[12], n, stars, star_cells, galaxies, gal_refs, gal_cells,
            })
        };
        let mut sky = Sky { frame: gal_frame(), stars, star_cells, galaxies, gal_cells, diffuse, dw, av_map, params_mag_limit: lim, deep, poles: [[0.0; NB]; 2] };
        sky.refresh_poles();
        Ok(sky)
    }

    /// Diffuse photon radiance at world direction d (bilinear). One map for
    /// every direction (built to the deep limit when a deep field exists), so
    /// rays inside and outside the deep cone see the same unresolved light.
    pub fn diffuse_at(&self, d: V3, out: &mut [f32; NB]) {
        let g = self.to_gal(d);
        let w = self.dw;
        let h = w / 2;
        let l = g.x.atan2(g.z);
        let b = g.y.clamp(-1.0, 1.0).asin();
        let fx = (l / (2.0 * std::f64::consts::PI) + 0.5) * w as f64 - 0.5;
        let fy = (0.5 - b / std::f64::consts::PI) * h as f64 - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor().clamp(0.0, (h - 1) as f64);
        let tx = (fx - x0) as f32;
        let ty = (fy - y0).clamp(0.0, 1.0) as f32;
        let xi = |x: f64| (((x as i64) % w as i64 + w as i64) % w as i64) as usize;
        let (xa, xb) = (xi(x0), xi(x0 + 1.0));
        let (ya, yb) = (y0 as usize, (y0 as usize + 1).min(h - 1));
        let (c00, c10, c01, c11) = (&self.diffuse[ya * w + xa], &self.diffuse[ya * w + xb], &self.diffuse[yb * w + xa], &self.diffuse[yb * w + xb]);
        for k in 0..NB {
            out[k] = (c00[k] * (1.0 - tx) + c10[k] * tx) * (1.0 - ty) + (c01[k] * (1.0 - tx) + c11[k] * tx) * ty;
        }
        // polar caps (beyond the first/last row centre): blend to the pole's
        // mean, so the map has one value at each pole instead of one per longitude
        let cap = if fy < 0.0 { Some((0usize, (-fy / 0.5).min(1.0))) } else if fy > (h - 1) as f64 { Some((1usize, ((fy - (h - 1) as f64) / 0.5).min(1.0))) } else { None };
        if let Some((pi, t)) = cap {
            let pole = self.poles[pi];
            for k in 0..NB { out[k] = out[k] * (1.0 - t as f32) + pole[k] * t as f32; }
        }
    }

    /// Recompute the cached pole means (after the map changes).
    pub fn refresh_poles(&mut self) {
        self.poles = [self.pole_mean(0), self.pole_mean(1)];
    }

    /// Mean of the first (north, 0) or last (south, 1) row of the diffuse map.
    pub fn pole_mean(&self, which: usize) -> [f32; NB] {
        let w = self.dw;
        let row = if which == 0 { 0 } else { w / 2 - 1 };
        let mut m = [0f64; NB];
        for x in 0..w { for k in 0..NB { m[k] += self.diffuse[row * w + x][k] as f64; } }
        let mut o = [0f32; NB];
        for k in 0..NB { o[k] = (m[k] / w as f64) as f32; }
        o
    }
}

/// Integrated starlight per steradian (photons s⁻¹ m⁻² nm⁻¹ sr⁻¹) from stars
/// fainter than `mag_limit` along a line of sight, extinguished if `extinct`.
/// Unresolved light is given a two-temperature blackbody shape: old
/// populations ~4600 K, young disc ~6500 K (numerically approximated).
fn starlight(l: &Los, mag_limit: f64, extinct: bool) -> [f64; NB] {
    let zp = V0_PHOTON_FLUX * 10f64.powf(-0.4 * 14.83);
    let mut s = [0.0; NB];
    let shape_y: Vec<f64> = (0..NB).map(|k| planck_photon(bin_center(k), 6500.0) / planck_photon(550.0, 6500.0)).collect();
    let shape_o: Vec<f64> = (0..NB).map(|k| planck_photon(bin_center(k), 4600.0) / planck_photon(550.0, 4600.0)).collect();
    let dln = shell_dln();
    for k in 0..NSHELL {
        let mv = mag_limit - l.mu[k] - l.a_v[k];
        let dr = l.r[k] * dln;
        let ly = l.wy[k] / (l.r[k] * l.r[k] * dr) * dr * lf_lum_fainter(mv, true) * zp;
        let lo = l.wo[k] / (l.r[k] * l.r[k] * dr) * dr * lf_lum_fainter(mv, false) * zp;
        for b in 0..NB {
            let ext = if extinct { 10f64.powf(-0.4 * l.a_v[k] * extinction_ratio(bin_center(b))) } else { 1.0 };
            s[b] += (ly * shape_y[b] + lo * shape_o[b]) * ext;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_star_counts_match_real_sky() {
        // integrate N(<V) over the sky with the same LOS model; the real sky has
        // ~5,000 stars brighter than V = 6 and ~2.3 million brighter than V = 12.
        let ns = 6000;
        let (mut n6, mut n12) = (0.0, 0.0);
        for i in 0..ns {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / ns as f64;
            let rr = (1.0 - y * y).sqrt();
            let ph = i as f64 * std::f64::consts::PI * (3.0 - 5f64.sqrt());
            let g = V3::new(rr * ph.cos(), y, rr * ph.sin());
            let l = los(1977, g, 1.0, 1.0);
            n6 += l.count_brighter(6.0) * 4.0 * std::f64::consts::PI / ns as f64;
            n12 += l.count_brighter(12.0) * 4.0 * std::f64::consts::PI / ns as f64;
        }
        assert!((n6 / 5000.0 - 1.0).abs() < 0.3, "N(<6) = {n6}");
        assert!((n12 / 2.3e6 - 1.0).abs() < 0.3, "N(<12) = {n12}");
    }

    #[test]
    fn catalogue_is_seed_reproducible() {
        let p = SkyParams { mag_limit: 6.5, galaxy_mag_limit: 10.0, diffuse_w: 32, ..Default::default() };
        let a = Sky::generate(&p);
        let b = Sky::generate(&p);
        assert_eq!(a.stars.len(), b.stars.len());
        assert!(a.stars.iter().zip(&b.stars).all(|(x, y)| x.dir == y.dir && x.v == y.v));
        let c = Sky::generate(&SkyParams { seed: 42, ..p });
        assert_ne!(a.stars.len(), c.stars.len());
        // roughly the real number of naked-eye stars
        assert!(a.stars.len() > 4000 && a.stars.len() < 14000, "{}", a.stars.len());
    }
}
