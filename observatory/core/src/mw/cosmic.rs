//! Supercluster: a constrained ΛCDM realisation with clusters, galaxies,
//! hot gas and radio sources.
//!
//! What it computes (comoving Mpc, h = 0.7 written out, never hidden):
//!
//! 1. **Linear density field.** A Gaussian random field with the BBKS
//!    transfer function (Sugiyama's shape parameter), n_s = 0.965 and σ₈ = 0.81
//!    (*numerically approximated*: a fitting function, not a Boltzmann code).
//!    One peak of height ν σ_R at the box centre is imposed with the
//!    Hoffman–Ribak construction, so the box contains a supercluster without
//!    otherwise changing the statistics (*physically derived*).
//! 2. **Structure.** Particles moved by the truncated Zel'dovich
//!    approximation, x = q + ψ(q), with the linear field smoothed at the
//!    non-linear scale so particles do not overshoot pancakes (Coles, Melott &
//!    Shandarin 1993; *numerically approximated*: no gravity after the first
//!    step). Velocities v = f H₀ ψ with f = Ω_m^0.55.
//! 3. **Haloes.** A peak-patch-like excursion set on the linear field: from
//!    large to small top-hat scales R, every Lagrangian peak with δ_R > 1.686
//!    not already inside a larger halo collapses into a halo of mass
//!    (4π/3) R³ ρ̄_m, moved to where ψ takes it (Bond & Myers 1996, simplified
//!    to spherical collapse).
//! 4. **Cluster gas.** β-model atmospheres (β = 2/3, r_c = 0.2 R₅₀₀) holding
//!    12% of M₅₀₀ in gas inside R₅₀₀, at the Arnaud et al. (2005) M–T relation
//!    temperature, with a random projected ellipticity (*empirical* scalings).
//! 5. **Warm–hot gas.** The baryons outside clusters follow the smoothed
//!    matter density at T ≈ 2 × 10⁵ (ρ/ρ̄)^0.7 K (*empirical* scaling).
//! 6. **Galaxies.** Centrals from the Moster et al. (2013) stellar-to-halo mass
//!    relation, satellites on NFW orbits, field galaxies on random particles;
//!    star formation from the main sequence, quenched more often in clusters;
//!    radio AGN in massive ellipticals; a radio halo and two relics in the
//!    merging central cluster (*statistically synthesized*).

use super::field::{fft3, kint, white_k, cic};
use super::phys::*;
use super::*;
use crate::json::Json;
use crate::rng::Rng;
use std::f64::consts::PI;

pub struct CosmicParams {
    pub seed: u64,
    pub n: usize,
    pub box_mpc: f64,
    pub distance_mpc: f64,
    pub h: f64,
    pub om: f64,
    pub ob: f64,
    pub sigma8: f64,
    pub ns: f64,
    pub peak_nu: f64,
    pub peak_r: f64,
    pub tza: f64,
    pub gal_density: f64,
    pub mstar_min: f64,
    pub a_v_fg: f64,
}

impl CosmicParams {
    pub fn from_json(j: &Json) -> CosmicParams {
        let g = |k: &str, d: f64| j.num(k).unwrap_or(d);
        CosmicParams {
            seed: g("seed", 12.0) as u64,
            n: (g("grid", 128.0) as usize).next_power_of_two(),
            box_mpc: g("box_mpc", 120.0),
            distance_mpc: g("distance_mpc", 200.0),
            h: g("h", 0.7),
            om: g("omega_m", 0.3),
            ob: g("omega_b", 0.048),
            sigma8: g("sigma8", 0.81),
            ns: g("n_s", 0.965),
            peak_nu: g("peak_nu", 3.2),
            peak_r: g("peak_r_mpc", 10.0),
            tza: g("tza_factor", 1.0),
            gal_density: g("galaxy_density_mpc3", 0.015),
            mstar_min: g("mstar_min", 5.0e9),
            a_v_fg: g("foreground_av", 0.08),
        }
    }
    pub fn to_json(&self) -> Json {
        let mut j = Json::obj();
        j.set("generator", "supercluster").set("seed", self.seed).set("grid", self.n).set("box_mpc", self.box_mpc)
            .set("distance_mpc", self.distance_mpc).set("h", self.h).set("omega_m", self.om).set("omega_b", self.ob)
            .set("sigma8", self.sigma8).set("n_s", self.ns).set("peak_nu", self.peak_nu).set("peak_r_mpc", self.peak_r);
        j
    }
}

/// Flat ΛCDM background.
#[derive(Clone, Copy)]
pub struct Cosmo { pub h: f64, pub om: f64 }

impl Cosmo {
    pub fn hubble_mpc(&self) -> f64 { 2997.92458 / self.h } // c/H₀ in Mpc
    pub fn e(&self, z: f64) -> f64 { (self.om * (1.0 + z).powi(3) + 1.0 - self.om).sqrt() }
    /// Comoving distance to redshift z, Mpc.
    pub fn chi(&self, z: f64) -> f64 {
        let n = 200;
        (0..n).map(|i| { let zz = z * (i as f64 + 0.5) / n as f64; 1.0 / self.e(zz) }).sum::<f64>() * z / n as f64 * self.hubble_mpc()
    }
    /// Redshift at comoving distance χ (Mpc), by bisection.
    pub fn z_of_chi(&self, chi: f64) -> f64 {
        let (mut a, mut b) = (0.0, 5.0);
        for _ in 0..60 { let m = 0.5 * (a + b); if self.chi(m) < chi { a = m } else { b = m } }
        0.5 * (a + b)
    }
    /// A lookup table z(χ) on 0–2000 Mpc in 1 Mpc steps.
    pub fn z_table(&self) -> Vec<f64> { (0..=2000).map(|c| self.z_of_chi(c as f64)).collect() }
    /// Critical density today, M☉ Mpc⁻³.
    pub fn rho_crit(&self) -> f64 { 2.775e11 * self.h * self.h }
}

/// BBKS transfer function with the Sugiyama (1995) baryon correction; k in Mpc⁻¹.
pub fn transfer(k: f64, p: &CosmicParams) -> f64 {
    let gamma = p.om * p.h * (-p.ob * (1.0 + (2.0 * p.h).sqrt() / p.om)).exp();
    let q = (k / p.h) / gamma;
    if q < 1e-8 { return 1.0; }
    (1.0 + 2.34 * q).ln() / (2.34 * q) * (1.0 + 3.89 * q + (16.1 * q).powi(2) + (5.46 * q).powi(3) + (6.71 * q).powi(4)).powf(-0.25)
}

pub fn w_tophat(x: f64) -> f64 { if x < 1e-4 { 1.0 } else { 3.0 * (x.sin() - x * x.cos()) / (x * x * x) } }

/// Linear matter power spectrum at z = 0 (Mpc³), normalised to σ₈.
pub fn power(p: &CosmicParams) -> impl Fn(f64) -> f64 {
    let raw = |k: f64| k.powf(p.ns) * transfer(k, p).powi(2);
    let r8 = 8.0 / p.h;
    let s2 = sigma2(&raw, r8);
    let a = p.sigma8 * p.sigma8 / s2;
    let (ns, h, om, ob) = (p.ns, p.h, p.om, p.ob);
    move |k: f64| {
        let pp = CosmicParams { seed: 0, n: 0, box_mpc: 0.0, distance_mpc: 0.0, h, om, ob, sigma8: 0.0, ns, peak_nu: 0.0, peak_r: 0.0, tza: 0.0, gal_density: 0.0, mstar_min: 0.0, a_v_fg: 0.0 };
        a * k.powf(ns) * transfer(k, &pp).powi(2)
    }
}

/// σ²(R) for a top-hat of radius R: (1/2π²) ∫ k² P W² dk.
pub fn sigma2(p: &dyn Fn(f64) -> f64, r: f64) -> f64 {
    let n = 4000;
    let (l0, l1) = ((1e-4f64).ln(), (50.0f64).ln());
    (0..n).map(|i| {
        let k = (l0 + (l1 - l0) * (i as f64 + 0.5) / n as f64).exp();
        k * k * k * p(k) * w_tophat(k * r).powi(2)
    }).sum::<f64>() * (l1 - l0) / n as f64 / (2.0 * PI * PI)
}

pub struct CosmicStats {
    pub haloes: usize,
    pub m_max: f64,
    pub galaxies: usize,
    pub delta_peak: f64,
    pub k_nl: f64,
}

pub fn generate(p: &CosmicParams, log: &mut dyn FnMut(&str)) -> (MwScene, CosmicStats) {
    let n = p.n;
    let n3 = n * n * n;
    let l = p.box_mpc;
    let dxm = l / n as f64;
    let cosmo = Cosmo { h: p.h, om: p.om };
    let pk = power(p);
    let kf = 2.0 * PI / l;
    let kvec = |i: usize| -> [f64; 3] { [kf * kint(i % n, n), kf * kint((i / n) % n, n), kf * kint(i / (n * n), n)] };
    let kmag = |i: usize| -> f64 { let k = kvec(i); (k[0] * k[0] + k[1] * k[1] + k[2] * k[2]).sqrt() };

    // 1. linear field in Fourier space ------------------------------------
    log("supercluster: linear ΛCDM field");
    let (mut dr, mut di) = white_k(n, p.seed, 11);
    let dx3 = dxm.powi(3);
    let amp: Vec<f64> = (0..n3).map(|i| { let k = kmag(i); if k == 0.0 { 0.0 } else { (pk(k) / dx3).sqrt() } }).collect();
    for i in 0..n3 { dr[i] *= amp[i]; di[i] *= amp[i]; }
    // Hoffman–Ribak: impose δ_R(x₀) = ν σ_R at the centre (Gaussian filter R)
    let wg = |k: f64| (-0.5 * (k * p.peak_r).powi(2)).exp();
    let l3 = l.powi(3);
    let sig_r2: f64 = (0..n3).map(|i| { let k = kmag(i); if k == 0.0 { 0.0 } else { pk(k) * wg(k).powi(2) / l3 } }).sum();
    let sign = |i: usize| if ((i % n) + (i / n) % n + i / (n * n)) % 2 == 0 { 1.0 } else { -1.0 };
    let s_rand: f64 = (0..n3).map(|i| dr[i] * wg(kmag(i)) * sign(i)).sum::<f64>() / n3 as f64;
    let target = p.peak_nu * sig_r2.sqrt();
    let coef = (target - s_rand) / sig_r2 * n3 as f64 / l3;
    for i in 0..n3 {
        let k = kmag(i);
        if k == 0.0 { continue; }
        dr[i] += coef * pk(k) * wg(k) * sign(i);
    }
    // non-linear scale: ∫₀^k_nl k² P dk / 2π² = 1
    let mut acc = 0.0;
    let mut k_nl = 1.0;
    let mut k = 1e-3;
    while k < 10.0 { let dk = k * 0.01; acc += k * k * pk(k) * dk / (2.0 * PI * PI); if acc >= 1.0 { k_nl = k; break; } k += dk; }

    // 2. truncated Zel'dovich displacements ---------------------------------
    log("supercluster: truncated Zel'dovich displacements");
    let kg = p.tza * k_nl;
    let mut psi: Vec<Vec<f64>> = Vec::new();
    for ax in 0..3 {
        let mut re = vec![0.0; n3];
        let mut im = vec![0.0; n3];
        for i in 0..n3 {
            let kv = kvec(i);
            let k2 = kv[0] * kv[0] + kv[1] * kv[1] + kv[2] * kv[2];
            if k2 == 0.0 { continue; }
            let f = (-0.5 * k2 / (kg * kg)).exp() * kv[ax] / k2;
            // ψ_k = i k δ_k / k²
            re[i] = -di[i] * f;
            im[i] = dr[i] * f;
        }
        fft3(&mut re, &mut im, n, true);
        psi.push(re);
    }
    let fgrow = p.om.powf(0.55);
    let h0 = 100.0 * p.h; // km s⁻¹ Mpc⁻¹
    let mut parts: Vec<[f32; 3]> = Vec::with_capacity(n3);
    for i in 0..n3 {
        let q = [(i % n) as f64 + 0.5, ((i / n) % n) as f64 + 0.5, (i / (n * n)) as f64 + 0.5];
        parts.push([0, 1, 2].map(|a| ((q[a] + psi[a][i] / dxm).rem_euclid(n as f64)) as f32));
    }
    let dens = cic(n, &parts);

    // 3. haloes: peak-patch-like excursion set -------------------------------
    log("supercluster: haloes from the excursion set");
    let rho_m = p.om * cosmo.rho_crit();
    let mut used = vec![false; n3];
    let mut halos_l: Vec<([f64; 3], f64)> = Vec::new(); // Lagrangian cell position, mass
    let mut r = 20.0;
    while r > 3.4 {
        let mut re = vec![0.0; n3];
        let mut im = vec![0.0; n3];
        for i in 0..n3 { let w = w_tophat(kmag(i) * r); re[i] = dr[i] * w; im[i] = di[i] * w; }
        fft3(&mut re, &mut im, n, true);
        let rc = (r / dxm).ceil() as i64;
        let ni = n as i64;
        let at = |x: i64, y: i64, z: i64| re[((z.rem_euclid(ni) * ni + y.rem_euclid(ni)) * ni + x.rem_euclid(ni)) as usize];
        let mut peaks: Vec<(f64, usize)> = Vec::new();
        for i in 0..n3 {
            if re[i] < 1.686 || used[i] { continue; }
            let (x, y, z) = ((i % n) as i64, ((i / n) % n) as i64, (i / (n * n)) as i64);
            let mut is_max = true;
            'o: for dz in -1..=1 { for dy in -1..=1 { for dxx in -1..=1 {
                if (dxx, dy, dz) != (0, 0, 0) && at(x + dxx, y + dy, z + dz) > re[i] { is_max = false; break 'o; }
            }}}
            if is_max { peaks.push((re[i], i)); }
        }
        peaks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (_, i) in peaks {
            if used[i] { continue; }
            let (x, y, z) = ((i % n) as i64, ((i / n) % n) as i64, (i / (n * n)) as i64);
            let m = 4.0 / 3.0 * PI * r.powi(3) * rho_m;
            halos_l.push(([x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5], m));
            for dz in -rc..=rc { for dy in -rc..=rc { for dxx in -rc..=rc {
                if ((dxx * dxx + dy * dy + dz * dz) as f64).sqrt() * dxm <= r {
                    used[(((z + dz).rem_euclid(ni) * ni + (y + dy).rem_euclid(ni)) * ni + (x + dxx).rem_euclid(ni)) as usize] = true;
                }
            }}}
        }
        r *= 0.88;
    }

    // world placement: box centre at (0, 0, D) Mpc; positions in cm
    let d0 = p.distance_mpc;
    let world = |c: [f64; 3]| -> [f64; 3] { [(c[0] * dxm - 0.5 * l) * MPC, (c[1] * dxm - 0.5 * l) * MPC, (c[2] * dxm - 0.5 * l + d0) * MPC] };
    let psi_at = |c: [f64; 3]| -> [f64; 3] {
        let i = ((((c[2] - 0.5) as usize) % n) * n + ((c[1] - 0.5) as usize) % n) * n + ((c[0] - 0.5) as usize) % n;
        [psi[0][i], psi[1][i], psi[2][i]]
    };
    let ztab = cosmo.z_table();
    let z_at = |chi: f64| { let c = chi.clamp(0.0, 1999.999); let i = c as usize; ztab[i] + (ztab[i + 1] - ztab[i]) * (c - i as f64) };
    let z_and_vlos = |w: [f64; 3], v: [f64; 3]| -> (f64, f64) {
        let rr = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
        let zc = z_at(rr / MPC);
        let vlos = (v[0] * w[0] + v[1] * w[1] + v[2] * w[2]) / rr;
        (zc, vlos)
    };
    let z_center = cosmo.z_of_chi(d0);
    let mut rng = Rng::keyed(&[p.seed, 0xC1_u64 << 8]);
    let rho_c = cosmo.rho_crit() * cosmo.e(z_center).powi(2);
    let mut halos: Vec<Halo> = Vec::new();
    let mut halo_vel: Vec<[f64; 3]> = Vec::new();
    halos_l.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (k, &(c, m)) in halos_l.iter().enumerate() {
        let ps = psi_at(c);
        let e = [0, 1, 2].map(|a| (c[a] + ps[a] / dxm).rem_euclid(n as f64));
        let w = world(e);
        let v = ps.map(|s| fgrow * h0 * s);
        let (zc, vlos) = z_and_vlos(w, v);
        let m200 = m;
        let m500 = 0.68 * m200;
        let r500 = (3.0 * m500 / (4.0 * PI * 500.0 * rho_c)).cbrt(); // Mpc
        let r200 = (3.0 * m200 / (4.0 * PI * 200.0 * rho_c)).cbrt();
        let c200 = 5.7 * (m200 * p.h / 2e12).powf(-0.084) / (1.0 + zc).powf(0.47);
        let kt = 5.0 * (m500 * p.h / 0.7 / 3.84e14).powf(1.0 / 1.71); // keV, Arnaud et al. 2005 (h₇₀ = 1)
        let t = kt * KEV / K;
        let beta = 2.0 / 3.0;
        let rc = 0.2 * r500;
        let x = r500 / rc;
        let mgas = 0.12 * m500 * M_SUN_G;
        let mue = 1.17;
        let ne0 = mgas / (4.0 * PI * mue * M_P * (rc * MPC).powi(3) * (x - x.atan()));
        let merger = k == 0;
        let big = m500 > 5e14;
        let radio_halo = if merger || (big && rng.f64() < 0.3) { 10f64.powf(24.5) * 1e7 * (m500 / 1e15).powf(3.3) } else { 0.0 };
        let relics = if merger { Some(((rng.f64() - 0.5) * 0.6, 0.9 * r500 * MPC * 1.4, 10f64.powf(24.3) * 1e7 * (m500 / 1e15).powf(2.0))) } else { None };
        halos.push(Halo {
            pos: w, m500, r500: r500 * MPC, rc: rc * MPC, beta, ne0, t,
            q: if merger { 0.62 } else { 0.75 + 0.22 * rng.f64() }, pa: rng.f64() * PI,
            z: (1.0 + zc) * (1.0 + vlos / 299_792.458) - 1.0, radio_halo, relics, c200, m200, r200: r200 * MPC,
        });
        halo_vel.push(v);
    }
    if let Some((ang, _, _)) = halos.first().and_then(|h| h.relics) {
        halos[0].pa = ang;
        // the merging subcluster: a quarter of the mass, 0.7 Mpc along the merger
        // axis in the sky plane, shock-heated by 30% (*statistically synthesized*)
        let mut sub = halos[0].clone();
        let off = 0.7 * MPC;
        sub.pos = [sub.pos[0] + off * ang.cos(), sub.pos[1] + off * ang.sin(), sub.pos[2]];
        sub.m500 *= 0.25; sub.m200 *= 0.25;
        sub.r500 *= 0.25f64.cbrt(); sub.r200 *= 0.25f64.cbrt(); sub.rc *= 0.25f64.cbrt();
        sub.ne0 *= 1.1; sub.t *= 0.25f64.powf(1.0 / 1.71) * 1.3;
        sub.radio_halo = 0.0; sub.relics = None; sub.q = 0.8;
        halos.insert(1, sub);
        halo_vel.insert(1, halo_vel[0]);
        halos[0].pos = [halos[0].pos[0] - 0.25 * off * ang.cos(), halos[0].pos[1] - 0.25 * off * ang.sin(), halos[0].pos[2]];
    }

    // 4. warm–hot gas on the grid -------------------------------------------
    log("supercluster: warm–hot gas, galaxies and radio sources");
    let center = [0.0, 0.0, d0 * MPC];
    let mut vol = MwVolume::new(n, dxm * MPC, center);
    vol.metallicity = 0.2;
    vol.fg_av = p.a_v_fg;
    vol.z = z_center;
    let sm = super::field::smooth_periodic(&dens, n, 1.0);
    let ne_mean = p.ob * cosmo.rho_crit() * M_SUN_G / MPC.powi(3) / (1.17 * M_P) * (1.0 + z_center).powi(3);
    for i in 0..n3 {
        let c = [(i % n) as f64 + 0.5, ((i / n) % n) as f64 + 0.5, (i / (n * n)) as f64 + 0.5];
        let w = world(c);
        let mut rho = sm[i] as f64;
        for h in halos.iter().take(64) {
            let d2 = (0..3).map(|a| (w[a] - h.pos[a]).powi(2)).sum::<f64>();
            if d2 < (1.5 * h.r500).powi(2) { rho = rho.min(1.0); }
        }
        let ne = ne_mean * rho;
        let t = (2e5 * rho.max(0.0).powf(0.7)).clamp(1e4, 5e6);
        vol.f[F_NH][i] = ne as f32;
        vol.f[F_XION][i] = 1.0;
        vol.f[F_TGAS][i] = t as f32;
        vol.f[F_NHOT][i] = ne as f32;
        vol.f[F_THOT][i] = t as f32;
    }
    // edge fade over 3 cells so the box never shows as a line
    for i in 0..n3 {
        let c = [i % n, (i / n) % n, i / (n * n)];
        let fade = c.iter().map(|&v| ((v.min(n - 1 - v) as f64 + 0.5) / 3.0).min(1.0)).product::<f64>();
        for f in [F_NH, F_NHOT] { vol.f[f][i] = (vol.f[f][i] as f64 * fade) as f32; }
    }

    // 5. galaxies -----------------------------------------------------------
    let mut sources: Vec<Source> = Vec::new();
    let schechter = |rng: &mut Rng, lo: f64, hi: f64| -> f64 {
        // sample log M* from Φ ∝ (M/M_c)^(α+1) e^(−M/M_c) d ln M, α = −1.2, M_c = 10^10.9
        loop {
            let lm = lo.log10() + rng.f64() * (hi.log10() - lo.log10());
            let x = 10f64.powf(lm) / 10f64.powf(10.9);
            let w = x.powf(-0.2) * (-x).exp() / (lo / 10f64.powf(10.9)).powf(-0.2);
            if rng.f64() < w { return 10f64.powf(lm); }
        }
    };
    let add_galaxy = |rng: &mut Rng, w: [f64; 3], v: [f64; 3], ms: f64, env_q: f64, central: bool, member: bool, sources: &mut Vec<Source>| {
        let (zc, vlos) = z_and_vlos(w, v);
        let chi = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
        let lm = ms.log10();
        let pq = (0.2 + 0.6 / (1.0 + (-(lm - 10.7) / 0.25).exp())).max(env_q);
        let quenched = rng.f64() < pq;
        let sfr_ms = 10f64.powf(0.75 * (lm - 10.0) + 0.1 + 0.3 * rng.normal());
        let sfr = if quenched { sfr_ms * 0.015 } else { sfr_ms };
        let mut l14 = 1.7e28 * sfr;
        let p_agn = if central && ms > 1e11 { 0.7 } else { (0.25 * (ms / 10f64.powf(11.5)).powi(2)).min(0.6) * if quenched { 1.0 } else { 0.2 } };
        let z_obs = (1.0 + zc) * (1.0 + vlos / 299_792.458) - 1.0;
        let d_l = (1.0 + zc) * chi;
        // size–mass relation (Shen et al. 2003 slopes): R_e ∝ M*^0.56 quenched, ^0.4 star-forming
        let re_kpc = if quenched { 3.5 * (ms / 5e10).powf(0.56) } else { 5.0 * (ms / 5e10).powf(0.4) };
        let size = re_kpc * 1e-3 * MPC * (1.0 + zc) / chi;
        let flags = 2 * quenched as u8 + 4 * member as u8;
        if ms > 3e10 && rng.f64() < p_agn {
            let lr = 10f64.powf(23.0 + 2.5 * rng.f64()) * 1e7;
            if lr > 1e31 {
                // FR-like double: core + two lobes along a random axis
                let s = (50.0 + 200.0 * rng.f64()) * 1e-3 * MPC;
                let (th, ph) = (rng.f64() * PI, rng.f64() * 2.0 * PI);
                let ax = [th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()];
                for sgn in [-1.0, 1.0] {
                    let lw = [0, 1, 2].map(|a| w[a] + sgn * s * ax[a]);
                    let lchi = (lw[0] * lw[0] + lw[1] * lw[1] + lw[2] * lw[2]).sqrt();
                    sources.push(Source { pos: lw, sed: Sed::Radio { l14: 0.42 * lr, alpha: 0.8 }, size: 0.35 * s / lchi, a_v_fg: 0.0, z: z_obs, d_l, flags: 8 });
                }
                l14 += 0.16 * lr;
            } else {
                l14 += lr;
            }
        }
        sources.push(Source { pos: w, sed: Sed::Galaxy { mstar: ms, sfr, l14, alpha: 0.75 }, size, a_v_fg: p.a_v_fg, z: z_obs, d_l, flags });
    };
    let mut n_members = 0usize;
    for (h, v) in halos.iter().zip(&halo_vel) {
        let m = h.m200;
        if m < 1e13 { continue; }
        // central from the Moster et al. 2013 SHMR
        let mm = m / 10f64.powf(11.59);
        let ms_c = 2.0 * 0.0351 * m / (mm.powf(-1.376) + mm.powf(0.608));
        let env = if m > 1e14 { 0.85 } else { 0.6 };
        add_galaxy(&mut rng, h.pos, *v, ms_c, env, true, true, &mut sources);
        let nsat = rng.poisson(m / 5e12) as usize;
        let sv = 1000.0 * (m / 1e15).powf(1.0 / 3.0) / 3f64.sqrt(); // km s⁻¹ per axis
        for _ in 0..nsat {
            // NFW radius by rejection within R200
            let c = h.c200;
            let mu = |x: f64| (1.0 + x).ln() - x / (1.0 + x);
            let u = rng.f64() * mu(c);
            let (mut a, mut b) = (0.0, c);
            for _ in 0..40 { let mid = 0.5 * (a + b); if mu(mid) < u { a = mid } else { b = mid } }
            let rr = 0.5 * (a + b) / c * h.r200;
            let (ct, ph) = (2.0 * rng.f64() - 1.0, 2.0 * PI * rng.f64());
            let st = (1.0 - ct * ct).sqrt();
            let w = [h.pos[0] + rr * st * ph.cos(), h.pos[1] + rr * st * ph.sin(), h.pos[2] + rr * ct];
            let vv = [v[0] + sv * rng.normal(), v[1] + sv * rng.normal(), v[2] + sv * rng.normal()];
            let ms = schechter(&mut rng, p.mstar_min, ms_c * 0.8);
            add_galaxy(&mut rng, w, vv, ms, env, false, true, &mut sources);
            n_members += 1;
        }
    }
    let n_field = ((p.gal_density * l * l * l) as usize).saturating_sub(n_members);
    // field galaxies on particles, accepted with probability ∝ (ρ/ρ̄)^0.6 so
    // they are biased tracers of the matter, emptier in voids (*empirical* bias)
    let mut placed = 0;
    while placed < n_field {
        let i = (rng.f64() * n3 as f64) as usize % n3;
        let cell = {
            let q = parts[i];
            ((q[2] as usize % n) * n + q[1] as usize % n) * n + q[0] as usize % n
        };
        if rng.f64() > ((sm[cell] as f64).max(0.0).powf(0.6) / 4.0).min(1.0) { continue; }
        placed += 1;
        let c = [parts[i][0] as f64, parts[i][1] as f64, parts[i][2] as f64];
        let w = world(c);
        let v = [psi[0][i], psi[1][i], psi[2][i]].map(|s| fgrow * h0 * s);
        let ms = schechter(&mut rng, p.mstar_min, 3e12);
        add_galaxy(&mut rng, w, v, ms, 0.0, false, false, &mut sources);
    }
    let n_gal = sources.iter().filter(|s| matches!(s.sed, Sed::Galaxy { .. })).count();
    // mean overdensity within the constraint radius of the centre (Eulerian)
    let delta_peak = {
        let (mut s, mut c) = (0.0, 0usize);
        for i in 0..n3 {
            let r2 = [i % n, (i / n) % n, i / (n * n)].iter().map(|&v| (v as f64 - (n / 2) as f64).powi(2)).sum::<f64>() * dxm * dxm;
            if r2 <= p.peak_r * p.peak_r { s += dens[i] as f64; c += 1; }
        }
        s / c.max(1) as f64 - 1.0
    };
    let stats = CosmicStats { haloes: halos.len(), m_max: halos.first().map(|h| h.m200).unwrap_or(0.0), galaxies: n_gal, delta_peak, k_nl };
    log(&format!(
        "supercluster: k_nl = {:.3} Mpc⁻¹, {} haloes (largest M₂₀₀ = {:.2e} M☉), {} galaxies, z_centre = {:.4}",
        k_nl, halos.len(), stats.m_max, n_gal, z_center
    ));
    let mut meta = p.to_json();
    meta.set("haloes", halos.len()).set("m200_max", stats.m_max).set("galaxies", n_gal).set("z_center", z_center).set("k_nl", k_nl);
    // particles in world Mpc for weak lensing
    let world_parts: Vec<[f32; 3]> = parts.iter().map(|q| { let w = world([q[0] as f64, q[1] as f64, q[2] as f64]); [(w[0] / MPC) as f32, (w[1] / MPC) as f32, (w[2] / MPC) as f32] }).collect();
    let scene = MwScene {
        kind: "supercluster".into(), vol, sources, halos, cluster: None, meta, seed: p.seed,
        particles: Some(world_parts), cosmo: Some((p.h, p.om, p.box_mpc)),
    };
    (scene, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_spectrum_is_normalised_to_sigma8() {
        let p = CosmicParams::from_json(&Json::obj());
        let pk = power(&p);
        let s8 = sigma2(&pk, 8.0 / p.h).sqrt();
        assert!((s8 - 0.81).abs() < 1e-3, "σ8 = {s8}");
    }

    #[test]
    fn comoving_distance_matches_a_known_value() {
        // flat ΛCDM, h = 0.7, Ω_m = 0.3: χ(z = 0.1) ≈ 418.4 Mpc
        let c = Cosmo { h: 0.7, om: 0.3 };
        assert!((c.chi(0.1) - 418.4).abs() < 1.5, "{}", c.chi(0.1));
        assert!((c.z_of_chi(c.chi(0.05)) - 0.05).abs() < 1e-6);
    }
}
