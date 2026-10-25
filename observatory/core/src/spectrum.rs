//! Spectral representation, filters, sensor QE, dust extinction and colour.
//!
//! Conventions (used everywhere in the renderer):
//! * The transported quantity is **photon spectral radiance per unit
//!   wavelength**, N_λ, in photons s⁻¹ m⁻² sr⁻¹ nm⁻¹.
//! * A frequency ratio g = ν_obs / ν_emit maps it as
//!   N_λ,obs(λ) = g⁴ N_λ,emit(g λ)  (physically derived: this is the Liouville
//!   invariant I_ν/ν³ rewritten for photon counts per unit wavelength).
//! * Emission lines are carried as integrated photon radiance (photons
//!   s⁻¹ m⁻² sr⁻¹) at an observed wavelength; they transform as g³ and move to
//!   λ_rest / g.
//! * Continuum lives in `NB` bins of `BIN_W` nm from `LAMBDA0`; lines in `NL`
//!   separate channels so narrow-band filters see them at their exact
//!   wavelength. RGB is only made at display/export time.

pub const NB: usize = 20;
pub const LAMBDA0: f64 = 360.0;
pub const BIN_W: f64 = 30.0;
pub const NL: usize = 6;
pub const NCH: usize = NB + NL;

/// Rest wavelengths (nm, air) of the carried emission lines.
pub const LINES: [(&str, f64); NL] = [
    ("H-beta", 486.13),
    ("[O III] 5007", 500.68),
    ("H-alpha", 656.28),
    ("[N II] 6583", 658.35),
    ("[S II] 6716", 671.64),
    ("[S II] 6731", 673.08),
];
pub const L_HB: usize = 0;
pub const L_OIII: usize = 1;
pub const L_HA: usize = 2;
pub const L_NII: usize = 3;
pub const L_SII_A: usize = 4;
pub const L_SII_B: usize = 5;

#[inline]
pub fn bin_center(i: usize) -> f64 {
    LAMBDA0 + BIN_W * (i as f64 + 0.5)
}

pub type Spectrum = [f32; NCH];

/// Photon spectral radiance of a blackbody (photons s⁻¹ m⁻² sr⁻¹ nm⁻¹).
pub fn planck_photon(lambda_nm: f64, t: f64) -> f64 {
    const C: f64 = 2.997_924_58e8;
    const HC_K: f64 = 1.438_776_877e-2; // hc/k (m·K)
    let l = lambda_nm * 1e-9;
    let x = HC_K / (l * t);
    if x > 700.0 {
        return 0.0;
    }
    2.0 * C / (l.powi(4) * x.exp_m1()) * 1e-9
}

/// V = 0 photon flux density at 550 nm: ~1000 photons s⁻¹ cm⁻² Å⁻¹ (Bessell 1998).
pub const V0_PHOTON_FLUX: f64 = 1.0e8; // photons s⁻¹ m⁻² nm⁻¹

/// Continuum of a star of V magnitude `v` and temperature `t`, as photon flux
/// density per bin (photons s⁻¹ m⁻² nm⁻¹), evaluated at g·λ and scaled by g⁴.
pub fn star_flux(v: f64, t: f64, g: f64, a_v: f64, out: &mut [f32; NB]) {
    let norm = V0_PHOTON_FLUX * 10f64.powf(-0.4 * v) / planck_photon(550.0, t);
    for (i, o) in out.iter_mut().enumerate() {
        let l = bin_center(i);
        let ext = 10f64.powf(-0.4 * a_v * extinction_ratio(g * l));
        *o = (norm * planck_photon(g * l, t) * g.powi(4) * ext) as f32;
    }
}

/// Cardelli, Clayton & Mathis (1989) A_λ / A_V for R_V = 3.1, valid
/// 0.3–3.3 µm⁻¹ (IR) and 1.1–3.3 µm⁻¹ (optical).
pub fn extinction_ratio(lambda_nm: f64) -> f64 {
    let x = 1000.0 / lambda_nm.clamp(303.0, 3333.0);
    const RV: f64 = 3.1;
    if x < 1.1 {
        let p = x.powf(1.61);
        0.574 * p - 0.527 * p / RV
    } else {
        let y = x - 1.82;
        let a = 1.0 + y * (0.17699 + y * (-0.50447 + y * (-0.02427 + y * (0.72085 + y * (0.01979 + y * (-0.77530 + y * 0.32999))))));
        let b = y * (1.41338 + y * (2.28305 + y * (1.07233 + y * (-5.38434 + y * (-0.62251 + y * (5.30260 + y * -2.09002))))));
        a + b / RV
    }
}

// ---------------------------------------------------------------- filters --

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Gaussian-like broadband (centre, FWHM).
    Gauss(f64, f64),
    /// Flat top with cosine edges (lo, hi, edge width), for luminance/RGB.
    Flat(f64, f64, f64),
}

#[derive(Clone, Debug)]
pub struct Filter {
    pub name: &'static str,
    pub shape: Shape,
    pub peak: f64,
}

pub const FILTERS: &[Filter] = &[
    // Bessell broadbands approximated as Gaussians (numerically approximated).
    Filter { name: "U", shape: Shape::Gauss(365.0, 66.0), peak: 0.70 },
    Filter { name: "B", shape: Shape::Gauss(445.0, 94.0), peak: 0.85 },
    Filter { name: "V", shape: Shape::Gauss(551.0, 88.0), peak: 0.90 },
    Filter { name: "R", shape: Shape::Gauss(658.0, 138.0), peak: 0.90 },
    Filter { name: "I", shape: Shape::Gauss(806.0, 149.0), peak: 0.90 },
    // Astro imaging set.
    Filter { name: "L", shape: Shape::Flat(400.0, 700.0, 10.0), peak: 0.97 },
    Filter { name: "Red", shape: Shape::Flat(600.0, 700.0, 8.0), peak: 0.95 },
    Filter { name: "Green", shape: Shape::Flat(500.0, 600.0, 8.0), peak: 0.95 },
    Filter { name: "Blue", shape: Shape::Flat(400.0, 500.0, 8.0), peak: 0.95 },
    // 5 nm narrowbands.
    Filter { name: "Ha", shape: Shape::Gauss(656.3, 5.0), peak: 0.90 },
    Filter { name: "OIII", shape: Shape::Gauss(500.7, 5.0), peak: 0.90 },
    Filter { name: "SII", shape: Shape::Gauss(672.0, 5.0), peak: 0.90 },
];

pub fn filter_by_name(name: &str) -> Option<&'static Filter> {
    FILTERS.iter().find(|f| f.name.eq_ignore_ascii_case(name))
}

impl Filter {
    pub fn t(&self, l: f64) -> f64 {
        self.peak
            * match self.shape {
                Shape::Gauss(c, fwhm) => {
                    // narrowbands: flatter top (super-Gaussian, order 4)
                    let s = fwhm / 2.0;
                    let x = (l - c) / s;
                    if fwhm < 20.0 { (-(0.693 * x.powi(4))).exp() } else { (-(0.693 * x * x)).exp() }
                }
                Shape::Flat(lo, hi, e) => {
                    let ramp = |d: f64| if d <= 0.0 { 0.0 } else if d >= e { 1.0 } else { 0.5 - 0.5 * (std::f64::consts::PI * d / e).cos() };
                    ramp(l - lo + e / 2.0).min(ramp(hi + e / 2.0 - l))
                }
            }
    }

    /// Effective wavelength (transmission-weighted), for the PSF.
    pub fn effective_wavelength(&self) -> f64 {
        let (mut s, mut w) = (0.0, 0.0);
        let mut l = 300.0;
        while l < 1100.0 {
            let t = self.t(l);
            s += t * l;
            w += t;
            l += 0.25;
        }
        s / w
    }
}

/// Typical back-illuminated CMOS quantum efficiency (numerically approximated
/// from published curves; peak ~0.8 near 550 nm).
pub fn qe(l: f64) -> f64 {
    const T: [(f64, f64); 9] = [
        (350.0, 0.25), (400.0, 0.55), (450.0, 0.72), (500.0, 0.80), (550.0, 0.82),
        (650.0, 0.75), (750.0, 0.58), (850.0, 0.38), (1000.0, 0.08),
    ];
    if l <= T[0].0 { return T[0].1; }
    for w in T.windows(2) {
        if l <= w[1].0 {
            let f = (l - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + f * (w[1].1 - w[0].1);
        }
    }
    0.02
}

/// Per-channel response weights of a filter × QE: continuum bins as
/// ∫_bin T·QE dλ (nm), lines as T·QE at the observed line wavelength.
pub fn channel_weights(f: &Filter, line_shift: f64, use_qe: bool) -> [f64; NCH] {
    let mut w = [0.0; NCH];
    for i in 0..NB {
        let (lo, hi) = (LAMBDA0 + BIN_W * i as f64, LAMBDA0 + BIN_W * (i as f64 + 1.0));
        let mut s = 0.0;
        let n = 120;
        for k in 0..n {
            let l = lo + (hi - lo) * (k as f64 + 0.5) / n as f64;
            s += f.t(l) * if use_qe { qe(l) } else { 1.0 };
        }
        w[i] = s * (hi - lo) / n as f64;
    }
    for (j, (_, l0)) in LINES.iter().enumerate() {
        let l = l0 / line_shift;
        w[NB + j] = f.t(l) * if use_qe { qe(l) } else { 1.0 };
    }
    w
}

// ----------------------------------------------------------------- colour --

/// CIE 1931 2° colour-matching functions, multi-lobe Gaussian fit
/// (Wyman, Sloan & Shirley 2013).
pub fn cie_xyz(l: f64) -> (f64, f64, f64) {
    let g = |x: f64, mu: f64, s1: f64, s2: f64| {
        let t = (x - mu) / if x < mu { s1 } else { s2 };
        (-0.5 * t * t).exp()
    };
    let x = 1.056 * g(l, 599.8, 37.9, 31.0) + 0.362 * g(l, 442.0, 16.0, 26.7) - 0.065 * g(l, 501.1, 20.4, 26.2);
    let y = 0.821 * g(l, 568.8, 46.9, 40.5) + 0.286 * g(l, 530.9, 16.3, 31.1);
    let z = 1.217 * g(l, 437.0, 11.8, 36.0) + 0.681 * g(l, 459.0, 26.0, 13.8);
    (x, y, z)
}

/// Channel → linear sRGB weights for "true colour" display. Photon counts
/// are weighted by photon energy ∝ 1/λ before the CIE projection.
pub fn true_color_weights(line_shift: f64) -> [[f64; 3]; NCH] {
    let to_rgb = |x: f64, y: f64, z: f64| {
        [
            3.2406 * x - 1.5372 * y - 0.4986 * z,
            -0.9689 * x + 1.8758 * y + 0.0415 * z,
            0.0557 * x - 0.2040 * y + 1.0570 * z,
        ]
    };
    let mut w = [[0.0; 3]; NCH];
    for i in 0..NB {
        let mut acc = [0.0; 3];
        for k in 0..30 {
            let l = LAMBDA0 + BIN_W * i as f64 + (k as f64 + 0.5);
            let (x, y, z) = cie_xyz(l);
            let c = to_rgb(x, y, z);
            for a in 0..3 { acc[a] += c[a] * 550.0 / l; }
        }
        w[i] = acc; // per nm of bin width (sum over 1 nm sub-steps)
    }
    for (j, (_, l0)) in LINES.iter().enumerate() {
        let l = l0 / line_shift;
        let (x, y, z) = cie_xyz(l);
        let c = to_rgb(x, y, z);
        w[NB + j] = [c[0] * 550.0 / l, c[1] * 550.0 / l, c[2] * 550.0 / l];
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extinction_is_one_at_v_and_rises_to_blue() {
        assert!((extinction_ratio(549.0) - 1.0).abs() < 0.02);
        assert!(extinction_ratio(440.0) > 1.25);
        assert!(extinction_ratio(800.0) < 0.65);
    }

    #[test]
    fn narrowband_sees_redshifted_line_only_when_in_band() {
        let ha = filter_by_name("Ha").unwrap();
        let w0 = channel_weights(ha, 1.0, false);
        let w_shift = channel_weights(ha, 1.0 / 1.02, false); // λ_obs = 1.02 λ_rest
        assert!(w0[NB + L_HA] > 0.85);
        assert!(w_shift[NB + L_HA] < 0.01);
    }

    #[test]
    fn star_flux_normalised_at_v() {
        let mut out = [0f32; NB];
        star_flux(0.0, 5800.0, 1.0, 0.0, &mut out);
        // bin containing 550 nm is [540,570): centre 555, within a few % of V0
        let i = ((550.0 - LAMBDA0) / BIN_W) as usize;
        assert!((out[i] as f64 / V0_PHOTON_FLUX - 1.0).abs() < 0.05);
        let mut hot = [0f32; NB];
        star_flux(0.0, 20000.0, 1.0, 0.0, &mut hot);
        assert!(hot[1] > out[1]); // hotter star is bluer at equal V
    }

    #[test]
    fn redshift_maps_g4() {
        let mut a = [0f32; NB];
        let mut b = [0f32; NB];
        star_flux(5.0, 6000.0, 1.0, 0.0, &mut a);
        star_flux(5.0, 6000.0, 1.1, 0.0, &mut b);
        // a hotter-looking (blueshifted) spectrum, larger by ~g⁴ near the peak
        assert!(b[5] > a[5]);
    }

    #[test]
    fn v_zero_point_matches_ab_flux_at_550nm() {
        // V = 0 ↔ ~3631 Jy at 550 nm: N_λ = F_ν / (h λ) ≈ 1.0 × 10⁸ photons s⁻¹ m⁻² nm⁻¹
        let ab = 3631e-26 / (6.626_070_15e-34 * 550e-9) * 1e-9;
        assert!((V0_PHOTON_FLUX / ab - 1.0).abs() < 0.05, "V0 {V0_PHOTON_FLUX} vs AB {ab}");
        // and a V = 0 star's spectrum is normalised to it at 550 nm
        let mut out = [0f32; NB];
        star_flux(0.0, 9600.0, 1.0, 0.0, &mut out);
        let k = ((550.0 - LAMBDA0) / BIN_W) as usize;
        let at550 = out[k] as f64 * planck_photon(550.0, 9600.0) / planck_photon(bin_center(k), 9600.0);
        assert!((at550 / V0_PHOTON_FLUX - 1.0).abs() < 1e-6);
    }
}
