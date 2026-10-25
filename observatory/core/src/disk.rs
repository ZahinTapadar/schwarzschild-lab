//! Thin accretion disk around a Schwarzschild hole.
//!
//! *Physically derived:* the radial flux profile is the Page–Thorne (1974)
//! solution for a geometrically thin, optically thick disk with zero torque at
//! the ISCO, integrated numerically for a = 0:
//!
//! ```text
//! F(r) = Ṁ/(4π r) · f(r),   f(r) = −Ω_,r / (E − ΩL)² ∫_{r_in}^{r} (E − ΩL) L_,r dr
//! E = (1 − 2m/r)/√(1 − 3m/r),  L = √(m r)/√(1 − 3m/r),  Ω = √(m/r³)
//! ```
//!
//! *Physically derived:* the absolute scale. With the hole's mass M and the
//! accretion rate Ṁ = λ_Edd L_Edd / (η c²) (L_Edd = 1.257 × 10³¹ W per M_⊙,
//! η = 1 − E(r_in) = 0.0572 for r_in = 6M), the flux in SI units is
//! F = (Ṁ c² / r_g²) · F̃(r/M), and T_eff = (F/σ)^¼. In the Newtonian limit this
//! is the familiar 3GMṀ/(8πR³)(1 − √(R_in/R)) (tested).
//!
//! *Numerically approximated:* the spectrum. Each element radiates a
//! colour-corrected ("diluted") blackbody, I_ν = f_col⁻⁴ B_ν(f_col T_eff) with
//! f_col ≈ 1.7, the usual stand-in for electron scattering in the disk
//! atmosphere. Each element moves on a circular geodesic, so the frequency
//! ratio to a static observer at r_obs is
//!
//! ```text
//! g = [1/√(1 − 2m/r_obs)] / [u^t (1 − Ω λ)],   u^t = 1/√(1 − 3m/r),  λ = L_z/E of the photon.
//! ```
//!
//! Model limits: steady, geometrically thin, optically thick (unless a finite
//! `disk_tau` is set, which is *experimental*), zero torque at the ISCO,
//! Schwarzschild only. Not GRMHD, no disk atmosphere, no corona, no jet.
//! `brightness_scale` is an *artistically directed* multiplier, 1 by default.

use crate::spectrum::{bin_center, planck_photon, NB};

/// Eddington luminosity per solar mass (W), κ_es = 0.34 cm² g⁻¹ (solar).
pub const L_EDD_SUN_W: f64 = 1.257e31;
const C_M_S: f64 = 2.997_924_58e8;
const SIGMA_SB: f64 = 5.670_374e-8;

#[derive(Clone, Copy, Debug)]
pub struct DiskPhysics {
    pub mass_msun: f64,
    pub eddington_ratio: f64,
    /// ≤ 0: 1 − E(r_in).
    pub efficiency: f64,
    pub color_correction: f64,
    pub brightness_scale: f64,
}

pub struct DiskModel {
    pub m: f64,
    pub r_in: f64,
    pub r_out: f64,
    pub phys: DiskPhysics,
    /// Radiative efficiency actually used.
    pub eta: f64,
    /// Accretion rate (kg s⁻¹) and luminosity (W).
    pub mdot: f64,
    pub lum: f64,
    /// Peak effective temperature (K) and where it occurs (M).
    pub t_peak: f64,
    pub r_peak: f64,
    /// Thin-disk scale height check: max H/r of the radiation-pressure
    /// dominated solution, H ≈ (3/2η) λ_Edd r_g (1 − √(r_in/r)).
    pub h_over_r_max: f64,
    table_r: Vec<f64>,
    table_f: Vec<f64>,
    f_max: f64,
    /// SI flux per unit table value: Ṁ c² / (4π r_g²).
    f_si: f64,
}

/// Specific energy of a circular geodesic, E = (1 − 2m/r)/√(1 − 3m/r).
pub fn circular_energy(m: f64, r: f64) -> f64 { (1.0 - 2.0 * m / r) / (1.0 - 3.0 * m / r).sqrt() }

impl DiskModel {
    pub fn new(m: f64, r_in: f64, r_out: f64, phys: DiskPhysics) -> Self {
        // tabulate F(r) on a log grid; Simpson for the inner integral
        let n = 2048;
        let r_in = r_in.max(6.0 * m); // no stable circular orbits inside the ISCO
        let (lo, hi) = (r_in.ln(), r_out.max(r_in * 1.001).ln());
        let e = |r: f64| (1.0 - 2.0 * m / r) / (1.0 - 3.0 * m / r).sqrt();
        let l = |r: f64| (m * r).sqrt() / (1.0 - 3.0 * m / r).sqrt();
        let om = |r: f64| (m / (r * r * r)).sqrt();
        let d = |fun: &dyn Fn(f64) -> f64, r: f64| {
            let h = r * 1e-5;
            (fun(r + h) - fun(r - h)) / (2.0 * h)
        };
        let integrand = |r: f64| (e(r) - om(r) * l(r)) * d(&l, r);
        let mut table_r = Vec::with_capacity(n);
        let mut table_f = Vec::with_capacity(n);
        let mut integral = 0.0;
        let mut prev = r_in;
        for k in 0..n {
            let r = (lo + (hi - lo) * k as f64 / (n - 1) as f64).exp();
            if k > 0 {
                let mid = 0.5 * (prev + r);
                integral += (r - prev) / 6.0 * (integrand(prev) + 4.0 * integrand(mid) + integrand(r));
            }
            let f = -d(&om, r) / (e(r) - om(r) * l(r)).powi(2) * integral / r;
            table_r.push(r);
            table_f.push(f.max(0.0));
            prev = r;
        }
        let (mut f_max, mut r_peak) = (0.0, r_in);
        for (r, f) in table_r.iter().zip(&table_f) { if *f > f_max { f_max = *f; r_peak = *r; } }
        let eta = if phys.efficiency > 0.0 { phys.efficiency } else { 1.0 - circular_energy(m, r_in) };
        let lum = phys.eddington_ratio * L_EDD_SUN_W * phys.mass_msun;
        let mdot = lum / (eta * C_M_S * C_M_S);
        let r_g = crate::scene::R_G_SUN_M * phys.mass_msun * m;
        let f_si = mdot * C_M_S * C_M_S / (4.0 * std::f64::consts::PI * r_g * r_g);
        let t_peak = (f_si * f_max / SIGMA_SB).powf(0.25);
        let mut h_over_r_max: f64 = 0.0;
        for &r in &table_r {
            h_over_r_max = h_over_r_max.max(1.5 / eta * phys.eddington_ratio * (1.0 - (r_in / r).sqrt()) / (r / m));
        }
        DiskModel { m, r_in, r_out, phys, eta, mdot, lum, t_peak, r_peak, h_over_r_max, table_r, table_f, f_max, f_si }
    }

    /// Local flux (W m⁻²) emitted per face at r.
    pub fn flux_si(&self, r: f64) -> f64 { self.flux(r) * self.f_max * self.f_si }

    /// F(r)/F_max.
    pub fn flux(&self, r: f64) -> f64 {
        if r < self.r_in || r > self.r_out {
            return 0.0;
        }
        let n = self.table_r.len();
        let lo = self.table_r[0].ln();
        let hi = self.table_r[n - 1].ln();
        let x = ((r.ln() - lo) / (hi - lo) * (n - 1) as f64).clamp(0.0, (n - 1) as f64 - 1e-9);
        let i = x as usize;
        let t = x - i as f64;
        (self.table_f[i] * (1.0 - t) + self.table_f[(i + 1).min(n - 1)] * t) / self.f_max
    }

    /// Effective temperature (K).
    pub fn temperature(&self, r: f64) -> f64 {
        self.t_peak * self.flux(r).max(0.0).powf(0.25)
    }

    /// Frequency ratio g for a disk element at radius r seen by a static camera
    /// at r_obs, for a photon with λ = L_z/E (sign: + if co-rotating).
    pub fn redshift(&self, r: f64, r_obs: f64, lambda: f64) -> f64 {
        let ut = 1.0 / (1.0 - 3.0 * self.m / r).sqrt();
        let om = (self.m / (r * r * r)).sqrt();
        1.0 / (1.0 - 2.0 * self.m / r_obs).sqrt() / (ut * (1.0 - om * lambda))
    }

    /// Observed photon spectral radiance per bin for an element at r seen with
    /// frequency ratio g: N_obs(λ) = g⁴ N_emit(gλ).
    pub fn observed(&self, r: f64, g: f64, out: &mut [f32; NB]) {
        let t = self.temperature(r);
        if t <= 0.0 {
            out.fill(0.0);
            return;
        }
        let fc = self.phys.color_correction.max(1.0);
        let norm = self.phys.brightness_scale / fc.powi(4);
        for (i, o) in out.iter_mut().enumerate() {
            *o = (norm * planck_photon(g * bin_center(i), fc * t) * g.powi(4)) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phys() -> DiskPhysics {
        DiskPhysics { mass_msun: 10.0, eddington_ratio: 0.1, efficiency: 0.0, color_correction: 1.0, brightness_scale: 1.0 }
    }

    #[test]
    fn absolute_flux_matches_newtonian_far_out_and_luminosity() {
        let d = DiskModel::new(1.0, 6.0, 4000.0, phys());
        assert!((d.eta - (1.0 - (8.0f64 / 9.0).sqrt())).abs() < 1e-12);
        // far out: F → 3GMṀ/(8πR³)(1 − C/√R) with GM = c² r_g. The constant C
        // differs from the Newtonian √R_in (the relativistic zero-torque term),
        // so the ratio approaches 1 as r^(−1/2), and the leading 1/r³ law is exact
        let r_g = crate::scene::R_G_SUN_M * 10.0;
        let rel = |r: f64| {
            let newton = 3.0 * C_M_S * C_M_S * r_g * d.mdot / (8.0 * std::f64::consts::PI * (r * r_g).powi(3)) * (1.0 - (6.0f64 / r).sqrt());
            d.flux_si(r) / newton - 1.0
        };
        let (e500, e2000) = (rel(500.0), rel(2000.0));
        assert!(e2000.abs() < 0.04 && (e2000 / e500 - 0.5).abs() < 0.12, "{e500} {e2000}");
        // both faces of the disk radiate the luminosity at infinity η Ṁ c²; the
        // locally emitted power ∫2F dA exceeds it by the redshift/time-dilation
        // factors, so compare within a generous bound
        let mut p = 0.0;
        let n = 20000;
        for k in 0..n {
            let (r0, r1) = (6.0 * (4000.0f64 / 6.0).powf(k as f64 / n as f64), 6.0 * (4000.0f64 / 6.0).powf((k + 1) as f64 / n as f64));
            let r = 0.5 * (r0 + r1);
            p += 2.0 * d.flux_si(r) * 2.0 * std::f64::consts::PI * r * (r1 - r0) * r_g * r_g;
        }
        assert!(p / d.lum > 0.95 && p / d.lum < 1.4, "emitted/L = {}", p / d.lum);
    }

    #[test]
    fn page_thorne_peaks_outside_isco() {
        let d = DiskModel::new(1.0, 6.0, 60.0, phys());
        assert_eq!(d.flux(6.0), 0.0); // zero-torque inner edge
        let mut best = (0.0, 0.0);
        let mut r = 6.0;
        while r < 60.0 {
            if d.flux(r) > best.1 { best = (r, d.flux(r)); }
            r += 0.01;
        }
        // Page–Thorne Schwarzschild flux peaks near r ≈ 9.55 m
        assert!((best.0 - 9.55).abs() < 0.15, "peak at {}", best.0);
        assert!((best.1 - 1.0).abs() < 1e-3);
        assert!((d.r_peak - 9.55).abs() < 0.15);
    }

    #[test]
    fn redshift_limits() {
        let d = DiskModel::new(1.0, 6.0, 60.0, phys());
        // far observer, λ = 0: g = √(1 − 3/r) (transverse Doppler + gravity)
        let g = d.redshift(10.0, 1e12, 0.0);
        assert!((g - (1.0 - 0.3f64).sqrt()).abs() < 1e-9);
        // co-rotating photons are blueshifted relative to counter-rotating ones
        assert!(d.redshift(10.0, 1e12, 5.0) > d.redshift(10.0, 1e12, -5.0));
    }
}
