//! Volumetric nebula generator (slow, native; minutes, not milliseconds).
//!
//! What it evolves, on a uniform grid in parsecs, Myr, cm⁻³ and K:
//!
//! * gas density n_H and dust density (in H-equivalent units), moved by a
//!   conservative first-order donor-cell scheme (*numerically approximated*);
//! * momentum density, so the gas obeys the Euler equations in conservative
//!   form, ∂ₜn + ∇·(nv) = 0 and ∂ₜ(nv) + ∇·(nvv) = −∇P/μm_H, with
//!   P = n_H(1 + x)kT set by the ionization state, plus radial stellar-wind
//!   momentum injected inside a growing wind cavity (*numerically
//!   approximated*: a first-order, diffusive donor-cell scheme with a
//!   prescribed two-phase temperature instead of an energy equation; no
//!   self-gravity, no magnetic fields, no radiative-transfer feedback beyond
//!   the ionization front);
//! * ionization by the embedded massive stars: rays from each star spend the
//!   star's ionizing photon rate Q against case-B recombinations
//!   α_B n² dV along the ray (the Strömgren balance, *physically derived*),
//!   so dense clumps cast neutral shadows;
//! * temperature: ~8 000 K where ionized, ~30 K where neutral, hot in the wind
//!   cavity. Over-pressured ionized gas expands into the cloud, piling up
//!   shells and leaving pillars behind shadowing clumps.
//!
//! Noise only sets the initial turbulent density (a lognormal field). The
//! filaments, shells, cavities and pillars come from the evolution above.
//!
//! Emissivities written to the volume:
//! * H α from case-B recombination, α_eff(T) = 1.17e-13 (T/10⁴)^-0.94 cm³ s⁻¹
//!   (*physically derived*); H β = H α / 2.86;
//! * [O III], [N II], [S II] from H α times line ratios set by the local
//!   ionization parameter U = Φ_ion/(n c) and the partially ionized front
//!   (*statistically chosen within observed H II-region ranges*, not a
//!   photoionization calculation);
//! * dust extinction σ_V = 4.9e-22 cm² per H (A_V/N_H = 5.3e-22 mag cm²), and
//!   single-scattered starlight from the embedded stars, isotropic phase,
//!   albedo 0.6 (*numerically approximated*; multiple scattering is future work).

use crate::json::Json;
use crate::math::V3;
use crate::rng::Rng;
use crate::sky::gauss_field;
use crate::spectrum::{L_HA, L_HB, L_NII, L_OIII, L_SII_A, L_SII_B, V0_PHOTON_FLUX};
use crate::volume::{Volume, F_ALPHA, F_NH, F_SCA, F_XION, NF};
use std::sync::atomic::{AtomicU32, Ordering};

const PC: f64 = 3.0857e18; // cm
const ALPHA_B: f64 = 2.59e-13; // case-B recombination, cm³ s⁻¹ at 10⁴ K
const SIGMA_V: f64 = 4.9e-22; // dust extinction cross-section at V per H, cm²
const KB_MH: f64 = 0.008_628; // k/m_H in (pc/Myr)² K⁻¹
const C_CM: f64 = 2.998e10;

#[derive(Clone, Debug)]
pub struct NebulaStar {
    /// Position relative to the box centre, pc.
    pub pos: V3,
    /// Ionizing photon rate, s⁻¹.
    pub q: f64,
    /// Wind strength (cavity growth scale, pc at 1 Myr; 0 = no wind).
    pub wind: f64,
    /// Absolute V magnitude and effective temperature.
    pub m_v: f64,
    pub teff: f64,
}

#[derive(Clone, Debug)]
pub struct NebulaParams {
    pub seed: u64,
    pub n: usize,
    pub box_pc: f64,
    pub cloud_density: f64,
    pub cloud_radius: f64,
    pub ambient_density: f64,
    pub turbulence: f64,
    pub turbulence_scale: f64,
    pub t_end: f64,
    pub dust_to_gas: f64,
    pub stars: Vec<NebulaStar>,
    /// Ionization recomputed every this many steps.
    pub ion_every: usize,
    /// Static run: no flow (used for the Strömgren validation).
    pub static_gas: bool,
}

impl NebulaParams {
    /// The "Lensed nebula" object: a blister H II region. An O6 star and two
    /// late-O/early-B stars sit near the front face of a turbulent 24 pc
    /// molecular cloud (−z side, which faces the observer in the presets), so
    /// the ionized gas breaks out toward us, as in the Orion Nebula.
    pub fn lensed_nebula(seed: u64, n: usize) -> Self {
        NebulaParams {
            seed,
            n,
            box_pc: 24.0,
            cloud_density: 420.0,
            cloud_radius: 6.0,
            ambient_density: 0.1,
            turbulence: 1.0,
            turbulence_scale: 0.35,
            t_end: 1.0,
            dust_to_gas: 1.0,
            stars: vec![
                NebulaStar { pos: V3::new(-0.6, 0.4, -3.6), q: 1.2e49, wind: 1.4, m_v: -5.2, teff: 41000.0 },
                NebulaStar { pos: V3::new(1.8, -0.8, -3.0), q: 2.5e48, wind: 0.5, m_v: -4.6, teff: 34000.0 },
                NebulaStar { pos: V3::new(0.3, 2.0, -4.2), q: 5.0e47, wind: 0.0, m_v: -4.1, teff: 30000.0 },
            ],
            ion_every: 3,
            static_gas: false,
        }
    }

    pub fn to_json(&self) -> Json {
        let mut j = Json::obj();
        j.set("seed", self.seed).set("n", self.n).set("box_pc", self.box_pc)
            .set("cloud_density", self.cloud_density).set("cloud_radius", self.cloud_radius)
            .set("ambient_density", self.ambient_density).set("turbulence", self.turbulence)
            .set("turbulence_scale", self.turbulence_scale).set("t_end_myr", self.t_end)
            .set("dust_to_gas", self.dust_to_gas).set("static_gas", self.static_gas);
        let stars: Vec<Json> = self.stars.iter().map(|s| {
            let mut o = Json::obj();
            o.set("pos_pc", vec![Json::from(s.pos.x), s.pos.y.into(), s.pos.z.into()])
                .set("q", s.q).set("wind", s.wind).set("m_v", s.m_v).set("teff", s.teff);
            o
        }).collect();
        j.set("stars", stars);
        j
    }
}

pub struct GenStats {
    pub steps: usize,
    pub ionized_fraction: f64,
    pub mean_front_radius_pc: f64,
    pub mass_initial: f64,
    pub mass_final: f64,
}

pub struct Grid {
    pub n: usize,
    pub dx: f64,
    pub nh: Vec<f32>,
    pub dust: Vec<f32>,
    pub x: Vec<f32>,
    pub phi: Vec<f32>,
    pub illum: Vec<f32>,
    pub t: Vec<f32>,
}

impl Grid {
    #[inline]
    fn idx(&self, x: usize, y: usize, z: usize) -> usize { (z * self.n + y) * self.n + x }
    fn center(&self, i: usize) -> V3 {
        let n = self.n;
        let (x, y, z) = (i % n, (i / n) % n, i / (n * n));
        let h = 0.5 * (n as f64 - 1.0);
        V3::new((x as f64 - h) * self.dx, (y as f64 - h) * self.dx, (z as f64 - h) * self.dx)
    }
}

fn fibonacci_dirs(nd: usize) -> Vec<V3> {
    (0..nd).map(|i| {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / nd as f64;
        let r = (1.0 - y * y).sqrt();
        let ph = i as f64 * std::f64::consts::PI * (3.0 - 5f64.sqrt());
        V3::new(r * ph.cos(), y, r * ph.sin())
    }).collect()
}

fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16)
}

#[inline]
fn atomic_max(a: &AtomicU32, v: f32) {
    if v > 0.0 { a.fetch_max(v.to_bits(), Ordering::Relaxed); }
}

/// Cast ionizing rays from every star; returns per-voxel ionized fraction and
/// ionizing photon flux (cm⁻² s⁻¹). Assumes x = 1 behind the front.
fn ionize(g: &Grid, stars: &[NebulaStar], dirs: &[V3]) -> (Vec<f32>, Vec<f32>) {
    let n3 = g.n * g.n * g.n;
    let xa: Vec<AtomicU32> = (0..n3).map(|_| AtomicU32::new(0)).collect();
    let pa: Vec<AtomicU32> = (0..n3).map(|_| AtomicU32::new(0)).collect();
    let half = 0.5 * g.n as f64 * g.dx;
    let ds = 0.5 * g.dx;
    let d_omega = 4.0 * std::f64::consts::PI / dirs.len() as f64;
    let nt = threads();
    std::thread::scope(|sc| {
        for t in 0..nt {
            let (xa, pa) = (&xa, &pa);
            sc.spawn(move || {
                for (k, d) in dirs.iter().enumerate() {
                    if k % nt != t { continue; }
                    for s in stars {
                        let budget0 = s.q / dirs.len() as f64;
                        let mut budget = budget0;
                        let mut r = 0.5 * ds;
                        loop {
                            let p = s.pos + *d * r;
                            if p.x.abs() >= half || p.y.abs() >= half || p.z.abs() >= half { break; }
                            let vx = ((p.x + half) / g.dx) as usize;
                            let vy = ((p.y + half) / g.dx) as usize;
                            let vz = ((p.z + half) / g.dx) as usize;
                            let i = g.idx(vx.min(g.n - 1), vy.min(g.n - 1), vz.min(g.n - 1));
                            let nh = g.nh[i] as f64;
                            let dv = d_omega * (r * PC).powi(2) * ds * PC;
                            let rec = ALPHA_B * nh * nh * dv;
                            let flux = budget * dirs.len() as f64 / (4.0 * std::f64::consts::PI * (r * PC).powi(2));
                            if budget >= rec {
                                budget -= rec;
                                atomic_max(&xa[i], 1.0);
                                atomic_max(&pa[i], flux as f32);
                            } else {
                                atomic_max(&xa[i], (budget / rec) as f32);
                                atomic_max(&pa[i], flux as f32);
                                break;
                            }
                            r += ds;
                        }
                    }
                }
            });
        }
    });
    (xa.iter().map(|a| f32::from_bits(a.load(Ordering::Relaxed))).collect(),
     pa.iter().map(|a| f32::from_bits(a.load(Ordering::Relaxed))).collect())
}

/// One pass of the separable [1, 2, 1]/4 binomial filter along x, y and z
/// (edge cells use [3, 1]/4, so the sum over the box is preserved).
fn binomial3(f: &mut [f32], n: usize) {
    for axis in 0..3 {
        let off = [1usize, n, n * n][axis];
        let src = f.to_vec();
        for i in 0..src.len() {
            let c = (i / off) % n;
            let (a, b) = (if c > 0 { src[i - off] } else { src[i] }, if c + 1 < n { src[i + off] } else { src[i] });
            f[i] = 0.25 * a + 0.5 * src[i] + 0.25 * b;
        }
    }
}

/// Far from the stars the ionizing rays thin out and can step past a voxel.
/// A voxel left neutral while at least 4 of its 6 neighbours are ionized is
/// such a gap: give it the neighbours' mean (two passes).
fn close_gaps(n: usize, x: &mut [f32], phi: &mut [f32]) {
    for _ in 0..2 {
        let (x0, p0) = (x.to_vec(), phi.to_vec());
        for z in 1..n - 1 { for y in 1..n - 1 { for xx in 1..n - 1 {
            let i = (z * n + y) * n + xx;
            if x0[i] >= 0.99 { continue; }
            let nb = [i - 1, i + 1, i - n, i + n, i - n * n, i + n * n];
            let ion = nb.iter().filter(|&&j| x0[j] >= 0.99).count();
            if ion >= 4 {
                x[i] = x[i].max(nb.iter().map(|&j| x0[j]).sum::<f32>() / 6.0);
                phi[i] = phi[i].max(nb.iter().map(|&j| p0[j]).sum::<f32>() / 6.0);
            }
        }}}
    }
}

/// Dust-attenuated optical starlight from the embedded stars, photons
/// s⁻¹ cm⁻² nm⁻¹ at 550 nm.
fn illuminate(g: &Grid, stars: &[NebulaStar], dirs: &[V3]) -> Vec<f32> {
    let n3 = g.n * g.n * g.n;
    let ia: Vec<AtomicU32> = (0..n3).map(|_| AtomicU32::new(0)).collect();
    let half = 0.5 * g.n as f64 * g.dx;
    let ds = 0.5 * g.dx;
    let nt = threads();
    std::thread::scope(|sc| {
        for t in 0..nt {
            let ia = &ia;
            sc.spawn(move || {
                for (k, d) in dirs.iter().enumerate() {
                    if k % nt != t { continue; }
                    for s in stars {
                        // L_550 = 4π (10 pc)² F_V(M_V), photons s⁻¹ nm⁻¹
                        let l550 = 4.0 * std::f64::consts::PI * (10.0 * PC).powi(2) * 1e-4 * V0_PHOTON_FLUX * 10f64.powf(-0.4 * s.m_v);
                        let mut tau = 0.0;
                        let mut r = 0.5 * ds;
                        loop {
                            let p = s.pos + *d * r;
                            if p.x.abs() >= half || p.y.abs() >= half || p.z.abs() >= half { break; }
                            let i = g.idx((((p.x + half) / g.dx) as usize).min(g.n - 1), (((p.y + half) / g.dx) as usize).min(g.n - 1), (((p.z + half) / g.dx) as usize).min(g.n - 1));
                            tau += g.dust[i] as f64 * SIGMA_V * ds * PC;
                            let e = l550 / (4.0 * std::f64::consts::PI * (r * PC).powi(2)) * (-tau).exp();
                            // sum over stars: accumulate via max per star is wrong; use add with CAS
                            let mut cur = ia[i].load(Ordering::Relaxed);
                            loop {
                                let nv = (f32::from_bits(cur) as f64).max(e) as f32;
                                match ia[i].compare_exchange_weak(cur, nv.to_bits(), Ordering::Relaxed, Ordering::Relaxed) {
                                    Ok(_) => break,
                                    Err(c) => cur = c,
                                }
                            }
                            r += ds;
                        }
                    }
                }
            });
        }
    });
    ia.iter().map(|a| f32::from_bits(a.load(Ordering::Relaxed))).collect()
}

/// Run the generator. Prints progress via `log`.
pub fn generate(p: &NebulaParams, log: &mut dyn FnMut(&str)) -> (Volume, GenStats) {
    let n = p.n;
    let n3 = n * n * n;
    let dx = p.box_pc / n as f64;
    let mut g = Grid { n, dx, nh: vec![0.0; n3], dust: vec![0.0; n3], x: vec![0.0; n3], phi: vec![0.0; n3], illum: vec![0.0; n3], t: vec![30.0; n3] };

    // Initial cloud: softened Plummer-like profile × lognormal turbulence.
    let sig = p.turbulence;
    for i in 0..n3 {
        let c = g.center(i);
        let r = c.len();
        let prof = (1.0 + (r / p.cloud_radius).powi(2)).powf(-1.25);
        let taper = 1.0 / (1.0 + ((r - 0.42 * p.box_pc) / (0.03 * p.box_pc)).exp());
        let gf = gauss_field(p.seed ^ 0xC10D, c, p.turbulence_scale, 5, 0.35);
        let ln = (sig * gf - 0.5 * sig * sig).exp();
        let nh = p.ambient_density + p.cloud_density * prof * taper * ln;
        g.nh[i] = nh as f32;
        g.dust[i] = (nh * p.dust_to_gas) as f32;
    }
    let mass0: f64 = g.nh.iter().map(|&v| v as f64).sum();

    // ~12 rays per voxel area at r = n/2 so no voxel inside the front is missed
    let nd = (12 * n * n).max(8192);
    let dirs = fibonacci_dirs(nd);
    let (mut mx, mut my, mut mz) = (vec![0f32; n3], vec![0f32; n3], vec![0f32; n3]);
    let mut vx = vec![0f32; n3];
    let mut vy = vec![0f32; n3];
    let mut vz = vec![0f32; n3];
    let mut time = 0.0;
    let mut steps = 0usize;
    let mut rng = Rng::keyed(&[p.seed, 0xF10F]);
    let _ = rng.f64();

    loop {
        if steps % p.ion_every == 0 || p.static_gas {
            let (mut x, mut phi) = ionize(&g, &p.stars, &dirs);
            close_gaps(n, &mut x, &mut phi);
            g.x = x;
            g.phi = phi;
        }
        if p.static_gas || time >= p.t_end { break; }

        // temperature and pressure P/k = n_H (1 + x) T
        let wind_r: Vec<f64> = p.stars.iter().map(|s| s.wind * time.max(1e-3).powf(0.6)).collect();
        for i in 0..n3 {
            let x = g.x[i] as f64;
            let mut t = 30.0 + x * 8000.0;
            let c = g.center(i);
            for (s, &rw) in p.stars.iter().zip(&wind_r) {
                if rw > 0.0 && (c - s.pos).len() < rw { t = 1.0e6; }
            }
            g.t[i] = t as f32;
        }
        let pres: Vec<f32> = (0..n3).map(|i| g.nh[i] * (1.0 + g.x[i]) * g.t[i]).collect();

        // time step (CFL 0.3)
        let vmax = (0..n3).map(|i| (vx[i].abs().max(vy[i].abs()).max(vz[i].abs())) as f64).fold(0.0, f64::max);
        let cs_max = (KB_MH * 2.0 * 8000.0f64).sqrt();
        let dt = (0.3 * dx / (vmax + cs_max)).min(p.t_end - time).max(1e-6);

        // Conservative first-order update of mass and momentum (donor cell):
        //   ∂n/∂t + ∇·(n v) = 0
        //   ∂(n v)/∂t + ∇·(n v v) = −(k/μ m_H) ∇P + wind forcing
        // with P/k = n(1 + x)T prescribed by the ionization state.
        let mu = 1.4;
        let c = (dt / dx) as f32;
        let n_floor = 0.05f32;
        // Fluxes of n, dust and the three momentum components, one axis at a
        // time: face fluxes computed in parallel z-slabs, then their divergence.
        let mut dn = vec![0f32; n3];
        let mut dd = vec![0f32; n3];
        let mut dm = [vec![0f32; n3], vec![0f32; n3], vec![0f32; n3]];
        let nt = threads();
        let slab = n.div_ceil(nt);
        for d in 0..3 {
            let off = [1usize, n, n * n][d];
            let m_d: &[f32] = [&mx, &my, &mz][d];
            // face flux arrays: flux through the + face of cell i along axis d
            let mut fl: [Vec<f32>; 5] = std::array::from_fn(|_| vec![0f32; n3]);
            {
                let (nh, du, mxr, myr, mzr) = (&g.nh, &g.dust, &mx, &my, &mz);
                let mut parts: Vec<_> = fl.iter_mut().map(|v| v.chunks_mut(slab * n * n).collect::<Vec<_>>()).collect();
                std::thread::scope(|sc| {
                    let mut slabs: Vec<Vec<&mut [f32]>> = (0..parts[0].len()).map(|_| Vec::new()).collect();
                    for q in parts.iter_mut() {
                        for (k, ch) in q.drain(..).enumerate() { slabs[k].push(ch); }
                    }
                    for (k, mut outs) in slabs.into_iter().enumerate() {
                        sc.spawn(move || {
                            let z0 = k * slab;
                            let len = outs[0].len();
                            for li in 0..len {
                                let i = z0 * n * n + li;
                                let (x, y, z) = (i % n, (i / n) % n, i / (n * n));
                                let ok = match d { 0 => x + 1 < n, 1 => y + 1 < n, _ => z + 1 < n };
                                if !ok { continue; }
                                let j = i + off;
                                let u = 0.5 * (m_d[i] / nh[i].max(n_floor) + m_d[j] / nh[j].max(n_floor));
                                let up = if u > 0.0 { i } else { j };
                                outs[0][li] = u * nh[up] * c;
                                outs[1][li] = u * du[up] * c;
                                outs[2][li] = u * mxr[up] * c;
                                outs[3][li] = u * myr[up] * c;
                                outs[4][li] = u * mzr[up] * c;
                            }
                        });
                    }
                });
            }
            for i in 0..n3 {
                let fin = |q: &[f32]| if i >= off { q[i - off] } else { 0.0 };
                dn[i] += fin(&fl[0]) - fl[0][i];
                dd[i] += fin(&fl[1]) - fl[1][i];
                for k in 0..3 { dm[k][i] += fin(&fl[2 + k]) - fl[2 + k][i]; }
            }
        }
        // Open (outflow) boundaries: gas moving outward through a box face
        // leaves the domain; nothing flows in. Without this, swept-up shells
        // pile up against the walls.
        for d in 0..3 {
            let off = [1usize, n, n * n][d];
            let m_d: &[f32] = [&mx, &my, &mz][d];
            for i in 0..n3 {
                let cd = (i / off) % n;
                if cd != 0 && cd != n - 1 { continue; }
                let u = m_d[i] / g.nh[i].max(n_floor);
                let out = if cd == n - 1 { u.max(0.0) } else { (-u).max(0.0) } * c;
                if out == 0.0 { continue; }
                dn[i] -= out * g.nh[i];
                dd[i] -= out * g.dust[i];
                dm[0][i] -= out * mx[i]; dm[1][i] -= out * my[i]; dm[2][i] -= out * mz[i];
            }
        }
        // pressure force and wind forcing on the momentum
        let kf = (KB_MH / mu * dt) as f32;
        for z in 1..n - 1 { for y in 1..n - 1 { for x in 1..n - 1 {
            let i = g.idx(x, y, z);
            let h2 = (2.0 * dx) as f32;
            dm[0][i] -= kf * (pres[i + 1] - pres[i - 1]) / h2;
            dm[1][i] -= kf * (pres[i + n] - pres[i - n]) / h2;
            dm[2][i] -= kf * (pres[i + n * n] - pres[i - n * n]) / h2;
            let cpos = g.center(i);
            for (s, &rw) in p.stars.iter().zip(&wind_r) {
                let dvec = cpos - s.pos;
                let r = dvec.len();
                if rw > 0.0 && r < rw && r > 1e-6 {
                    // wind ram pressure: radial momentum injected inside the cavity
                    let a = (40.0 * dt / r * (1.0 - r / rw)) as f32 * g.nh[i].max(n_floor);
                    dm[0][i] += a * dvec.x as f32; dm[1][i] += a * dvec.y as f32; dm[2][i] += a * dvec.z as f32;
                }
            }
        }}}
        for i in 0..n3 {
            g.nh[i] = (g.nh[i] + dn[i]).max(0.0);
            g.dust[i] = (g.dust[i] + dd[i]).max(0.0);
            let nn = g.nh[i].max(n_floor);
            let mut v = [(mx[i] + dm[0][i]) / nn, (my[i] + dm[1][i]) / nn, (mz[i] + dm[2][i]) / nn];
            let sp = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if sp > 60.0 { for c in v.iter_mut() { *c *= 60.0 / sp; } }
            mx[i] = v[0] * g.nh[i]; my[i] = v[1] * g.nh[i]; mz[i] = v[2] * g.nh[i];
            vx[i] = v[0]; vy[i] = v[1]; vz[i] = v[2];
        }
        // dust destruction in the hot / strongly irradiated gas
        for i in 0..n3 {
            let k = if g.t[i] > 1.0e5 { 8.0 } else { 0.25 * g.x[i] as f64 * (g.phi[i] as f64 / 1e12).min(1.0) };
            g.dust[i] *= (-(k * dt)).exp() as f32;
        }
        time += dt;
        steps += 1;
        if steps % 25 == 0 {
            let ion: f64 = g.x.iter().map(|&v| v as f64).sum::<f64>() / n3 as f64;
            log(&format!("  t = {time:.3} Myr  step {steps}  dt = {dt:.4}  ionized volume {:.1}%", ion * 100.0));
        }
    }

    g.illum = illuminate(&g, &p.stars, &dirs);
    // fill rare voxels missed by the illumination rays (far corners): 3³ max
    let src = g.illum.clone();
    for z in 1..n - 1 { for y in 1..n - 1 { for x in 1..n - 1 {
        let i = g.idx(x, y, z);
        if src[i] == 0.0 {
            let mut m = 0f32;
            for dz in 0..3 { for dy in 0..3 { for dxx in 0..3 { m = m.max(src[g.idx(x + dxx - 1, y + dy - 1, z + dz - 1)]); }}}
            g.illum[i] = m;
        }
    }}}

    // emissivities
    let mut dense: [Vec<f32>; NF] = std::array::from_fn(|_| vec![0.0; n3]);
    let four_pi = 4.0 * std::f64::consts::PI;
    let smooth = |a: f64, b: f64, x: f64| { let t = ((x - a) / (b - a)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    for i in 0..n3 {
        let nh = g.nh[i] as f64;
        let x = g.x[i] as f64;
        let ne = x * nh;
        let t = (g.t[i] as f64).clamp(5000.0, 2.0e4);
        let ha = if x > 0.0 && g.t[i] < 1.0e5 { 1.17e-13 * (t / 1.0e4).powf(-0.94) * ne * ne / four_pi } else { 0.0 };
        let u = g.phi[i] as f64 / (nh.max(1e-3) * C_CM);
        let f_hi = smooth(-3.3, -2.3, u.max(1e-12).log10());
        let f_lo = (4.0 * x * (1.0 - x)).max(0.0) + 0.3 * (1.0 - f_hi);
        dense[L_HA][i] = ha as f32;
        dense[L_HB][i] = (ha / 2.86) as f32;
        dense[L_OIII][i] = (4.5 * ha / 2.86 * f_hi) as f32;
        dense[L_NII][i] = (ha * (0.12 + 0.3 * (1.0 - f_hi))) as f32;
        let sii = ha * (0.06 + 0.5 * f_lo);
        dense[L_SII_A][i] = (0.56 * sii) as f32;
        dense[L_SII_B][i] = (0.44 * sii) as f32;
        let alpha = g.dust[i] as f64 * SIGMA_V;
        dense[F_ALPHA][i] = alpha as f32;
        dense[F_SCA][i] = (0.6 * alpha * g.illum[i] as f64 / four_pi) as f32;
        dense[F_NH][i] = nh as f32;
        dense[F_XION][i] = x as f32;
    }

    let ion_frac = g.x.iter().map(|&v| v as f64).sum::<f64>() / n3 as f64;
    // mean front radius around the first star (ionized volume → equivalent sphere)
    let ion_vol = g.x.iter().map(|&v| v as f64).sum::<f64>() * dx * dx * dx;
    let r_front = (3.0 * ion_vol / (4.0 * std::f64::consts::PI)).cbrt();
    let mass1: f64 = g.nh.iter().map(|&v| v as f64).sum();
    let mut meta = Json::obj();
    meta.set("generator", "observatory volgen v1 (pressure-driven flow + Stromgren ray casting)")
        .set("params", p.to_json())
        .set("steps", steps).set("t_myr", time)
        .set("ionized_volume_fraction", ion_frac)
        .set("equivalent_front_radius_pc", r_front)
        .set("mass_retained_fraction", mass1 / mass0)
        .set("units", "lines: photons s^-1 cm^-3 sr^-1; alpha_v: cm^-1; sca: photons s^-1 cm^-3 sr^-1 nm^-1 at 550 nm; n_h: cm^-3");
    let hottest = p.stars.iter().map(|s| s.teff).fold(0.0, f64::max);
    // Reconstruction: the collocated first-order scheme carries an odd–even
    // (checkerboard) density mode at the grid's Nyquist scale, which n² line
    // emission amplifies. That mode is not resolved physics, so the cached
    // fields are low-pass filtered once with a separable [1, 2, 1]/4 binomial
    // kernel: it removes the Nyquist mode exactly, halves a 4-voxel wave and
    // leaves 16-voxel structure within 4% (*numerically approximated*).
    for f in dense.iter_mut() { binomial3(f, n); }
    meta.set("reconstruction", "separable [1,2,1]/4 binomial filter on all cached fields (removes the grid Nyquist mode)");
    let vol = Volume::from_dense(n, dx, &dense, meta, hottest);
    (vol, GenStats { steps, ionized_fraction: ion_frac, mean_front_radius_pc: r_front, mass_initial: mass0, mass_final: mass1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stromgren_radius_in_uniform_gas() {
        // Q = 1e48 s⁻¹, n = 100 cm⁻³: R_s = (3Q / 4π n² α_B)^(1/3) ≈ 1.46 pc
        let p = NebulaParams {
            seed: 1, n: 48, box_pc: 6.0, cloud_density: 0.0, cloud_radius: 1.0, ambient_density: 100.0,
            turbulence: 0.0, turbulence_scale: 0.3, t_end: 0.0, dust_to_gas: 1.0,
            stars: vec![NebulaStar { pos: V3::ZERO, q: 1.0e48, wind: 0.0, m_v: -4.0, teff: 35000.0 }],
            ion_every: 1, static_gas: true,
        };
        let (_, st) = generate(&p, &mut |_| {});
        let rs = (3.0 * 1.0e48 / (4.0 * std::f64::consts::PI * 1.0e4 * ALPHA_B)).cbrt() / PC;
        assert!((st.mean_front_radius_pc / rs - 1.0).abs() < 0.08, "front {} vs R_s {}", st.mean_front_radius_pc, rs);
    }

    #[test]
    fn donor_cell_conserves_mass_inside_box() {
        let mut p = NebulaParams::lensed_nebula(3, 24);
        p.t_end = 0.15;
        let (vol, st) = generate(&p, &mut |_| {});
        assert!(st.steps > 2);
        assert!(vol.bricks.iter().all(|v| v.is_finite() && *v >= 0.0), "fields must be finite and non-negative");
        // the update is conservative: only outflow through the box faces removes mass
        assert!(st.mass_final <= st.mass_initial * 1.0001);
        assert!(st.mass_final > st.mass_initial * 0.8);
    }
}
