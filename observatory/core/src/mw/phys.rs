//! Emission and absorption physics shared by every band, in cgs units.
//!
//! Each function names its source and its label:
//! *physically derived* (a standard formula), *numerically approximated*
//! (a fit or table), or *empirical* (observed ratios, not a calculation).
//!
//! Emissivities are per steradian: j_ν in erg s⁻¹ cm⁻³ Hz⁻¹ sr⁻¹, so a ray
//! integrates dI_ν/ds = j_ν − α_ν I_ν to a specific intensity in
//! erg s⁻¹ cm⁻² Hz⁻¹ sr⁻¹ (1 Jy sr⁻¹ = 10⁻²³ of that).

use std::f64::consts::PI;

pub const C: f64 = 2.997_924_58e10;
pub const H: f64 = 6.626_070_15e-27;
pub const K: f64 = 1.380_649e-16;
pub const PC: f64 = 3.085_677_6e18;
pub const MPC: f64 = 3.085_677_6e24;
pub const KEV: f64 = 1.602_176_6e-9;
pub const M_P: f64 = 1.672_621_9e-24;
pub const ME_C2: f64 = 8.187_105_8e-7; // erg
pub const SIGMA_T: f64 = 6.652_458_7e-25;
pub const T_CMB: f64 = 2.7255;
pub const L_SUN: f64 = 3.828e33;
pub const M_SUN_G: f64 = 1.988_47e33;
pub const ARCSEC: f64 = PI / 648_000.0;
pub const JY: f64 = 1e-23;
/// Case-B recombination coefficient at 10⁴ K, cm³ s⁻¹.
pub const ALPHA_B: f64 = 2.59e-13;
/// Dust extinction cross-section at V per H nucleus (A_V/N_H = 5.3e-22 mag cm²), cm².
pub const SIGMA_V: f64 = 4.88e-22;

/// Planck function B_ν(T), erg s⁻¹ cm⁻² Hz⁻¹ sr⁻¹ (*physically derived*).
pub fn planck_nu(nu: f64, t: f64) -> f64 {
    if t <= 0.0 { return 0.0; }
    let x = H * nu / (K * t);
    if x > 700.0 { return 0.0; }
    2.0 * H * nu * nu * nu / (C * C) / x.exp_m1()
}

// ----------------------------------------------------------------- dust --
// Extinction A_λ/A_V for R_V = 3.1 diffuse-ISM dust: Cardelli–Clayton–Mathis
// in the optical, the Indebetouw/Weingartner–Draine near- and mid-infrared
// curve with the 9.7 µm silicate feature, and a β = 1.8 far-infrared power law
// pinned to Planck's τ₃₅₃/N_H = 1.2e-26 cm² at 850 µm (*numerically
// approximated*: a log–log table, not a grain model).
const EXT: [(f64, f64); 25] = [
    (0.10, 5.0), (0.15, 2.9), (0.2175, 3.2), (0.30, 1.8), (0.365, 1.56), (0.44, 1.32), (0.55, 1.0),
    (0.65, 0.81), (0.80, 0.60), (0.90, 0.48), (1.25, 0.28), (1.65, 0.17), (2.2, 0.11), (3.5, 0.055),
    (4.5, 0.043), (5.8, 0.039), (8.0, 0.040), (9.7, 0.075), (12.0, 0.045), (18.0, 0.040), (25.0, 0.025),
    (35.0, 0.013), (100.0, 1.15e-3), (250.0, 2.2e-4), (850.0, 2.45e-5),
];
// Albedo, falling from 0.6 in the optical to 0 beyond 8 µm.
const ALB: [(f64, f64); 7] = [(0.1, 0.4), (0.3, 0.6), (1.0, 0.55), (2.2, 0.4), (4.5, 0.2), (8.0, 0.0), (1e5, 0.0)];

fn loglog(t: &[(f64, f64)], x: f64) -> f64 {
    if x <= t[0].0 { return t[0].1; }
    for w in t.windows(2) {
        if x <= w[1].0 {
            let u = (x / w[0].0).ln() / (w[1].0 / w[0].0).ln();
            return (w[0].1.ln() * (1.0 - u) + w[1].1.max(1e-300).ln() * u).exp();
        }
    }
    let last = t[t.len() - 1];
    last.1 * (x / last.0).powf(-1.8)
}

fn linlog(t: &[(f64, f64)], x: f64) -> f64 {
    if x <= t[0].0 { return t[0].1; }
    for w in t.windows(2) {
        if x <= w[1].0 {
            let u = (x / w[0].0).ln() / (w[1].0 / w[0].0).ln();
            return w[0].1 * (1.0 - u) + w[1].1 * u;
        }
    }
    t[t.len() - 1].1
}

/// A_λ / A_V at wavelength λ (µm).
pub fn ext_ratio(lambda_um: f64) -> f64 { loglog(&EXT, lambda_um) }
/// Dust albedo at λ (µm).
pub fn albedo(lambda_um: f64) -> f64 { linlog(&ALB, lambda_um) }
/// Extinction cross-section per H nucleus at λ, cm².
pub fn sigma_ext(lambda_um: f64) -> f64 { SIGMA_V * ext_ratio(lambda_um) }
/// Absorption cross-section per H nucleus at λ, cm².
pub fn sigma_abs(lambda_um: f64) -> f64 { sigma_ext(lambda_um) * (1.0 - albedo(lambda_um)) }

/// Equilibrium temperature of big grains in a starlight field U (U = 1 is
/// the local interstellar radiation field). T ∝ U^(1/(4+β)), normalised to
/// 18 K at U = 1, which with the table above balances the 5e-24 erg s⁻¹ per H
/// absorbed in the ISRF (*physically derived* scaling, checked in a test).
pub fn dust_temperature(u: f64) -> f64 { 18.0 * u.max(1e-3).powf(1.0 / 5.8) }

/// Starlight power absorbed per H by big grains, erg s⁻¹, at U = 1.
pub const P_ABS_ISRF: f64 = 5.0e-24;

// PAH features (centre µm, fractional FWHM, share of PAH power), Drude
// profiles after Smith et al. 2007 (PAHFIT) and Draine & Li 2007
// (*numerically approximated*).
const PAH: [(f64, f64, f64); 8] = [
    (3.29, 0.013, 0.04), (5.70, 0.035, 0.03), (6.22, 0.030, 0.13), (7.70, 0.126, 0.36),
    (8.61, 0.039, 0.09), (11.27, 0.012, 0.11), (12.70, 0.045, 0.08), (17.0, 0.065, 0.06),
];
/// PAH power per H at U = 1 for a Milky Way PAH abundance (q_PAH ≈ 4.6%).
pub const P_PAH_ISRF: f64 = 1.2e-24;
/// Stochastically heated very small grains: power per H at U = 1.
pub const P_VSG_ISRF: f64 = 0.5e-24;

/// PAH emissivity per H per unit U, erg s⁻¹ Hz⁻¹ sr⁻¹ (single-photon heating,
/// so it scales linearly with U).
pub fn pah_per_h(lambda_um: f64) -> f64 {
    let nu = C / (lambda_um * 1e-4);
    let mut s = 0.0;
    for &(lc, g, w) in &PAH {
        let x = lambda_um / lc - lc / lambda_um;
        let nuc = C / (lc * 1e-4);
        // Drude profile normalised to unit integral over ν
        let prof = 2.0 / (PI * g * nuc) * g * g / (x * x + g * g);
        s += w * prof;
    }
    let _ = nu;
    P_PAH_ISRF * s / (4.0 * PI)
}

/// Very-small-grain continuum per H per unit U: a β = 1 grey body at 150 K
/// normalised to `P_VSG_ISRF` (*numerically approximated*; real VSGs have a
/// temperature distribution).
pub fn vsg_per_h(lambda_um: f64) -> f64 {
    let nu = C / (lambda_um * 1e-4);
    let t = 150.0;
    // ∫ ν B_ν(T) dν = (2h/c²)(kT/h)⁵ Γ(5)ζ(5) for β = 1, divided by ν₀ = 1e13 Hz
    let nu0 = 1e13;
    let norm = 4.0 * PI * (2.0 * H / (C * C)) * (K * t / H).powi(5) * 24.0 * 1.0369 / nu0;
    P_VSG_ISRF * (nu / nu0) * planck_nu(nu, t) / norm
}

// ------------------------------------------------------------- free–free --
/// Thermal free–free Gaunt factor for Z = 1, Draine (2011) eq. 10.9, valid from
/// radio to the near-infrared; tends to 1.2 at X-ray energies for hot gas.
pub fn gaunt_ff(nu: f64, t: f64) -> f64 {
    if H * nu > 0.1 * KEV { return 1.2; }
    let x = 5.960 - (3f64.sqrt() / PI) * (nu * 1e-9 * (t * 1e-4).powf(-1.5)).ln();
    (x.exp() + std::f64::consts::E).ln()
}

/// Free–free emissivity j_ν (Rybicki & Lightman 5.14b over 4π, *physically derived*).
pub fn j_ff(nu: f64, ne: f64, ni: f64, t: f64) -> f64 {
    if t <= 0.0 || ne <= 0.0 { return 0.0; }
    5.44e-39 * gaunt_ff(nu, t) * ne * ni / t.sqrt() * (-H * nu / (K * t)).exp()
}

/// Free–free absorption from Kirchhoff's law, α_ν = j_ν / B_ν(T).
pub fn alpha_ff(nu: f64, ne: f64, ni: f64, t: f64) -> f64 {
    let b = planck_nu(nu, t);
    if b <= 0.0 { 0.0 } else { j_ff(nu, ne, ni, t) / b }
}

// Collisional-ionisation-equilibrium cooling function for solar abundances,
// log₁₀ Λ (erg cm³ s⁻¹) against log₁₀ T, after Sutherland & Dopita (1993)
// (*numerically approximated*: a coarse table).
const COOL: [(f64, f64); 8] = [(5.0, -21.25), (5.5, -21.25), (6.0, -21.65), (6.5, -22.2), (7.0, -22.6), (7.5, -22.72), (8.0, -22.62), (8.5, -22.45)];

/// Total radiative cooling Λ(T) n_e n_H for metallicity `z` (in solar units);
/// the metal-line part scales with z.
pub fn cooling(t: f64, z: f64) -> f64 {
    let lt = t.max(1e5).log10();
    let mut l = COOL[COOL.len() - 1].1;
    for w in COOL.windows(2) {
        if lt <= w[1].0 { let u = (lt - w[0].0) / (w[1].0 - w[0].0); l = w[0].1 * (1.0 - u) + w[1].1 * u; break; }
    }
    let total = 10f64.powf(l);
    let ff = 1.42e-27 * 1.2 * t.sqrt();
    ff + (total - ff).max(0.0) * z
}

/// X-ray emissivity of hot plasma averaged over the photon-energy band
/// [e1, e2] keV, as a mean j_ν (erg s⁻¹ cm⁻³ Hz⁻¹ sr⁻¹). Bremsstrahlung is
/// exact in shape; the metal-line excess Λ − Λ_ff is spread above 0.3 keV with
/// the same e^(−E/kT) shape (*numerically approximated*: no individual lines).
pub fn xray_band(e1: f64, e2: f64, ne: f64, nh: f64, t: f64, z: f64) -> f64 {
    if t < 3e5 || ne <= 0.0 { return 0.0; }
    let kt = K * t / KEV;
    let (nu1, nu2) = (e1 * KEV / H, e2 * KEV / H);
    let dnu = nu2 - nu1;
    // bremsstrahlung: ∫ j_ν dν = 5.44e-39·1.2·n_e n_i T^-½ (kT/h)(e^(−E1/kT) − e^(−E2/kT))
    let ff = 5.44e-39 * 1.2 * ne * nh / t.sqrt() * (K * t / H) * ((-e1 / kt).exp() - (-e2 / kt).exp());
    let excess = (cooling(t, z) - 1.42e-27 * 1.2 * t.sqrt()).max(0.0) * ne * nh / (4.0 * PI);
    let lo = e1.max(0.3);
    let frac = if e2 <= 0.3 { 0.0 } else { ((-lo / kt).exp() - (-e2 / kt).exp()) / (-0.3 / kt).exp() };
    (ff + excess * frac) / dnu
}

/// Photoelectric absorption per H for solar-abundance gas, 0.3–10 keV, after
/// Morrison & McCammon (1983): σ ≈ 2.0e-22 (E/keV)^(-8/3) cm² (*numerically
/// approximated*: a single power law through the edges).
pub fn sigma_pe(e_kev: f64) -> f64 { 2.0e-22 * e_kev.max(0.1).powf(-8.0 / 3.0) }

// ------------------------------------------------------ recombination ----
/// Effective Hα recombination coefficient, case B (Osterbrock & Ferland),
/// cm³ s⁻¹ (*physically derived* fit).
pub fn alpha_ha(t: f64) -> f64 { 1.17e-13 * (t.max(1000.0) * 1e-4).powf(-0.942) }

/// Emission lines: (name, wavelength µm). Hydrogen line strengths are case-B
/// ratios to Hβ at 10⁴ K (Hummer & Storey 1987, *physically derived*); the
/// forbidden lines use ratios set by the local ionisation (*empirical*).
pub const LINES: [(&str, f64); 12] = [
    ("Hb", 0.4861), ("OIII4959", 0.4959), ("OIII5007", 0.5007), ("Ha", 0.6563), ("NII6584", 0.6584),
    ("SII6716", 0.6716), ("SII6731", 0.6731), ("Pab", 1.2818), ("Pag", 1.0938), ("Paa", 1.8751),
    ("Brg", 2.1661), ("Bra", 4.0523),
];

/// Line volume emissivities (erg s⁻¹ cm⁻³ sr⁻¹) of ionised gas, in `LINES`
/// order. `log_u` is the ionisation parameter log₁₀(Φ/(n c)); `front` ∈ [0, 1]
/// is the proximity of the ionisation front, where low-ionisation lines
/// ([S II], [N II]) are strong.
pub fn line_emissivities(ne: f64, np: f64, t: f64, log_u: f64, front: f64) -> [f64; 12] {
    let e = |l_um: f64| H * C / (l_um * 1e-4);
    let ha = ne * np * alpha_ha(t) * e(0.6563) / (4.0 * PI);
    let hb = ha / 2.86;
    let o3 = hb * 4.0 * ((log_u + 3.6) / 1.4).clamp(0.02, 1.0) * (1.0 - 0.7 * front);
    let n2 = ha * (0.12 + 0.35 * front);
    let s2 = ha * (0.04 + 0.45 * front);
    [hb, o3 / 2.98, o3, ha, n2, s2 * 0.55, s2 * 0.45, hb * 0.162, hb * 0.090, hb * 0.332, hb * 0.0275, hb * 0.078]
}

/// H₂ 1–0 S(1) at 2.122 µm from the photodissociation region: fluorescent
/// surface brightness I ≈ 1e-4 (U/10⁴)^½ erg s⁻¹ cm⁻² sr⁻¹ (Black & van
/// Dishoeck 1987 scale), spread over the first N = 2e21 cm⁻² of the PDR
/// (*numerically approximated*).
pub fn h2_emissivity(nh: f64, u_surface: f64, col_surf: f64) -> f64 {
    const N_PDR: f64 = 2e21;
    let is = 1e-4 * (u_surface / 1e4).max(0.0).sqrt();
    is * nh / N_PDR * (-col_surf / N_PDR).exp()
}

// ---------------------------------------------------------- synchrotron --
/// Optically thin synchrotron j_ν from its value at 1.4 GHz and spectral index
/// α (S ∝ ν^−α).
pub fn j_sync(j14: f64, nu: f64, alpha: f64) -> f64 { j14 * (nu / 1.4e9).powf(-alpha) }

// ---------------------------------------------------------------- SZ -----
/// Thermal Sunyaev–Zel'dovich spectral factor for the CMB temperature,
/// ΔT/T = y (x coth(x/2) − 4), x = hν/kT_CMB (*physically derived*,
/// non-relativistic).
pub fn sz_dt_factor(nu: f64) -> f64 {
    let x = H * nu / (K * T_CMB);
    x / (x / 2.0).tanh() - 4.0
}

/// Compton-y per unit path, cm⁻¹: σ_T n_e kT / m_e c².
pub fn dy_dl(ne: f64, t: f64) -> f64 { SIGMA_T * ne * K * t / ME_C2 }

// ------------------------------------------------------------- stars ----
/// Specific luminosity L_ν (erg s⁻¹ Hz⁻¹) of a blackbody star of bolometric
/// luminosity `l` (L☉) and temperature `t`.
pub fn star_lnu(nu: f64, l_sun: f64, t: f64) -> f64 {
    const SB: f64 = 5.670_374e-5;
    let r2 = l_sun * L_SUN / (4.0 * PI * SB * t.powi(4));
    4.0 * PI * PI * r2 * planck_nu(nu, t)
}

/// A grey body of total luminosity `l` (L☉) at temperature `t` with emissivity
/// index β, as L_ν.
pub fn greybody_lnu(nu: f64, l_sun: f64, t: f64, beta: f64) -> f64 {
    // ∫ (ν/ν₀)^β B_ν dν = ν₀^−β (2h/c²)(kT/h)^(4+β) Γ(4+β) ζ(4+β)
    let nu0: f64 = 1e12;
    let s = 4.0 + beta;
    let gz = gamma(s) * zeta(s);
    let integral = nu0.powf(-beta) * 2.0 * H / (C * C) * (K * t / H).powf(s) * gz;
    l_sun * L_SUN * (nu / nu0).powf(beta) * planck_nu(nu, t) / integral
}

pub fn gamma(x: f64) -> f64 { crate::rng::ln_gamma(x).exp() }
pub fn zeta(s: f64) -> f64 { (1..200).map(|n| (n as f64).powf(-s)).sum() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_grains_balance_the_isrf_at_18k() {
        // 4π ∫ σ_abs(ν) B_ν(18 K) dν over 5 µm–3 mm should match the 5e-24
        // erg s⁻¹ per H absorbed in the ISRF to within 25%.
        let mut p = 0.0;
        let n = 4000;
        let (l0, l1) = (5f64.ln(), 3000f64.ln());
        for i in 0..n {
            let l = (l0 + (l1 - l0) * (i as f64 + 0.5) / n as f64).exp();
            let nu = C / (l * 1e-4);
            let dnu = nu * (l1 - l0) / n as f64;
            p += 4.0 * PI * sigma_abs(l) * planck_nu(nu, 18.0) * dnu;
        }
        assert!((p / P_ABS_ISRF - 1.0).abs() < 0.25, "P = {p:e}");
    }

    #[test]
    fn pah_template_carries_its_power() {
        let mut p = 0.0;
        let n = 20000;
        let (l0, l1) = (2f64.ln(), 40f64.ln());
        for i in 0..n {
            let l = (l0 + (l1 - l0) * (i as f64 + 0.5) / n as f64).exp();
            let nu = C / (l * 1e-4);
            p += 4.0 * PI * pah_per_h(l) * nu * (l1 - l0) / n as f64;
        }
        assert!((p / P_PAH_ISRF - 1.0).abs() < 0.15, "P = {p:e}");
    }

    #[test]
    fn sz_decrement_at_150ghz() {
        // ΔT/T = −0.957 y at 150 GHz; null near 217 GHz.
        assert!((sz_dt_factor(150e9) + 0.957).abs() < 0.01);
        assert!(sz_dt_factor(217e9).abs() < 0.03);
    }

    #[test]
    fn halpha_of_a_uniform_slab_matches_emission_measure() {
        // I(Hα) = 1.67e-7 EM erg s⁻¹ cm⁻² sr⁻¹ for EM in pc cm⁻⁶ at 10⁴ K
        // (Osterbrock & Ferland; 1 R = 5.66e-18 erg s⁻¹ cm⁻² arcsec⁻²... in cgs/sr).
        let (ne, l_pc) = (100.0, 1.0);
        let j = line_emissivities(ne, ne, 1e4, -3.0, 0.0)[3];
        let i = j * l_pc * PC;
        let em = ne * ne * l_pc;
        let expect = 1.17e-13 * H * C / 0.6563e-4 / (4.0 * PI) * PC * em;
        assert!((i / expect - 1.0).abs() < 1e-6);
        // and in Rayleighs: 1 R ≈ 2.41e-7 erg s⁻¹ cm⁻² sr⁻¹ at Hα; EM = 2.75 T₄^0.9 I_R
        let rayleigh = i / 2.41e-7;
        assert!((em / (2.75 * rayleigh) - 1.0).abs() < 0.1, "EM/2.75R = {}", em / (2.75 * rayleigh));
    }

    #[test]
    fn free_free_radio_brightness_of_an_hii_region() {
        // Optically thin 5 GHz brightness temperature T_b ≈ 8.24e-2 T^-0.35 ν_GHz^-2.1 EM (Mezger & Henderson)
        let (ne, l_pc, t, nu) = (1000.0, 0.5, 8000.0, 5e9);
        let tau = alpha_ff(nu, ne, ne, t) * l_pc * PC;
        let tb = t * tau;
        let em = ne * ne * l_pc;
        let mh = 8.235e-2 * t.powf(-1.35) * 5f64.powf(-2.1) * em * t;
        assert!((tb / mh - 1.0).abs() < 0.12, "T_b {tb} vs {mh}");
    }

    #[test]
    fn hot_plasma_xray_emission_is_mostly_soft() {
        let soft = xray_band(0.5, 2.0, 1.0, 1.0, 6e6, 1.0) * (2.0 - 0.5);
        let hard = xray_band(2.0, 7.0, 1.0, 1.0, 6e6, 1.0) * (7.0 - 2.0);
        assert!(soft > 10.0 * hard, "{soft:e} {hard:e}");
        // a 5 keV cluster: still more soft than hard energy, but within 3×
        let s = xray_band(0.5, 2.0, 1.0, 1.0, 5.8e7, 0.3) * 1.5;
        let h = xray_band(2.0, 7.0, 1.0, 1.0, 5.8e7, 0.3) * 5.0;
        assert!(s > h * 0.5 && s < h * 3.0, "{s:e} {h:e}");
    }
}
