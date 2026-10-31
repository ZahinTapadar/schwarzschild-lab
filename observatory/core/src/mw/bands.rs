//! Instruments and their bands.
//!
//! Each instrument is *modelled on* a real facility class (the `analog`
//! field says which) so its numbers are plausible: aperture, band edges,
//! beam or PSF, background and exposure. None of them claims to be that
//! facility's calibrated response; band edges are top-hats.

use super::phys::{ARCSEC, C, KEV, H};

#[derive(Clone, Debug)]
pub enum Spec {
    /// Wavelength band, µm.
    Wave { lo: f64, hi: f64 },
    /// Radio/mm band: centre and width, GHz.
    Freq { ghz: f64, bw: f64 },
    /// X-ray photon-energy band, keV.
    Energy { lo: f64, hi: f64 },
    /// CMB temperature at frequency (GHz): thermal SZ + primary anisotropy + sources.
    Sz { ghz: f64 },
    /// Weak-lensing convergence.
    Kappa,
}

impl Spec {
    /// Band edges in frequency (Hz), low to high.
    pub fn nu_range(&self) -> (f64, f64) {
        match *self {
            Spec::Wave { lo, hi } => (C / (hi * 1e-4), C / (lo * 1e-4)),
            Spec::Freq { ghz, bw } => ((ghz - 0.5 * bw) * 1e9, (ghz + 0.5 * bw) * 1e9),
            Spec::Energy { lo, hi } => (lo * KEV / H, hi * KEV / H),
            Spec::Sz { ghz } => (ghz * 0.85e9, ghz * 1.15e9),
            Spec::Kappa => (1.0, 1.0),
        }
    }
    pub fn nu_center(&self) -> f64 { let (a, b) = self.nu_range(); (a * b).sqrt() }
    pub fn lambda_um(&self) -> f64 { C / self.nu_center() * 1e4 }
    /// Does this band (rest-frame edges scaled by 1 + z) contain wavelength λ (µm)?
    pub fn contains_um(&self, l_um: f64, z: f64) -> bool {
        let (a, b) = self.nu_range();
        let nu = C / (l_um * 1e-4);
        nu >= a * (1.0 + z) && nu <= b * (1.0 + z)
    }
    pub fn is_xray(&self) -> bool { matches!(self, Spec::Energy { .. }) }
}

#[derive(Clone, Debug)]
pub struct Band {
    pub name: String,
    /// What the band traces, one phrase.
    pub traces: String,
    pub spec: Spec,
    /// PSF or beam FWHM, arcsec.
    pub fwhm: f64,
    /// Background in the band: MJy sr⁻¹ for photon-counting optical/IR bands,
    /// counts s⁻¹ arcsec⁻² for X-ray bands.
    pub bkg: f64,
    /// Effective collecting area × efficiency (cm²) for photon-counting bands.
    pub area: f64,
    /// Noise per beam (mJy/beam), per arcmin (µK·arcmin) or per galaxy (σ_e), by kind.
    pub sigma: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Photon-counting imager (optical, infrared): output MJy sr⁻¹.
    Imager,
    /// Photon-counting X-ray CCD: output counts.
    Xray,
    /// Bolometer or interferometer with Gaussian thermal noise: output mJy/beam.
    Radio,
    /// CMB receiver: output µK (CMB temperature).
    Cmb,
    /// Galaxy-shape survey: output convergence κ.
    Lensing,
}

#[derive(Clone, Debug)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub analog: String,
    pub kind: Kind,
    pub bands: Vec<Band>,
    pub exposure_s: f64,
    /// Native detector pixel (arcsec), for read noise per output pixel.
    pub native_px: f64,
    pub read_noise: f64,
    /// Lensing: source density (arcmin⁻²) and smoothing σ (arcmin).
    pub n_gal: f64,
    pub smooth_arcmin: f64,
}

impl Instrument {
    pub fn unit(&self) -> &'static str {
        match self.kind { Kind::Imager => "MJy/sr", Kind::Xray => "counts", Kind::Radio => "mJy/beam", Kind::Cmb => "uK", Kind::Lensing => "kappa" }
    }
}

fn band(name: &str, traces: &str, spec: Spec, fwhm: f64, bkg: f64, area: f64, sigma: f64) -> Band {
    Band { name: name.into(), traces: traces.into(), spec, fwhm, bkg, area, sigma }
}

/// Diffraction FWHM 1.03 λ/D (arcsec) combined in quadrature with a floor.
fn diff(l_um: f64, d_m: f64, floor: f64) -> f64 { ((1.03 * l_um * 1e-6 / d_m / ARCSEC).powi(2) + floor * floor).sqrt() }

/// The instrument catalogue. Exposure and noise can be overridden by presets.
pub fn instrument(id: &str) -> Option<Instrument> {
    let w = |lo, hi| Spec::Wave { lo, hi };
    let mut ins = Instrument { id: id.into(), name: String::new(), analog: String::new(), kind: Kind::Imager, bands: vec![], exposure_s: 1000.0, native_px: 0.1, read_noise: 5.0, n_gal: 0.0, smooth_arcmin: 0.0 };
    match id {
        "optical_narrow" => {
            let a = 4.0e4 * 0.22;
            ins.name = "Optical narrowband imager".into();
            ins.analog = "2.4 m space telescope, UV/visible camera (HST WFC3/UVIS class)".into();
            ins.exposure_s = 2000.0; ins.native_px = 0.04; ins.read_noise = 3.0;
            ins.bands = vec![
                band("O III 502", "doubly ionised oxygen in the hottest gas", w(0.4978, 0.5043), diff(0.50, 2.4, 0.07), 0.08, a, 0.0),
                band("H-alpha 657", "ionised hydrogen recombining", w(0.6506, 0.6627), diff(0.656, 2.4, 0.07), 0.08, a, 0.0),
                band("S II 673", "the ionisation front", w(0.6707, 0.6825), diff(0.673, 2.4, 0.07), 0.08, a, 0.0),
            ];
        }
        "nircam" => {
            let a = 2.54e5 * 0.35;
            ins.name = "Near-infrared camera".into();
            ins.analog = "6.5 m cold space telescope, near-infrared camera (JWST NIRCam class)".into();
            ins.exposure_s = 1500.0; ins.native_px = 0.031; ins.read_noise = 15.0;
            ins.bands = vec![
                band("F090W", "starlight and scattered light", w(0.795, 1.005), diff(0.90, 6.5, 0.03), 0.25, a, 0.0),
                band("F187N", "Paschen-alpha from ionised gas", w(1.863, 1.885), diff(1.87, 6.5, 0.03), 0.20, a, 0.0),
                band("F200W", "stars through the dust", w(1.755, 2.226), diff(2.0, 6.5, 0.03), 0.20, a, 0.0),
                band("F335M", "PAH 3.3 um on cloud surfaces", w(3.177, 3.537), diff(3.35, 6.5, 0.06), 0.15, a, 0.0),
                band("F444W", "embedded stars and warm dust", w(3.880, 4.986), diff(4.4, 6.5, 0.06), 0.18, a, 0.0),
            ];
        }
        "miri" => {
            let a = 2.54e5 * 0.25;
            ins.name = "Mid-infrared imager".into();
            ins.analog = "6.5 m cold space telescope, mid-infrared imager (JWST MIRI class)".into();
            ins.exposure_s = 1500.0; ins.native_px = 0.11; ins.read_noise = 14.0;
            ins.bands = vec![
                band("F770W", "PAH 7.7 um: photodissociation regions", w(6.581, 8.687), diff(7.7, 6.5, 0.05), 1.5, a, 0.0),
                band("F1130W", "PAH 11.3 um", w(10.953, 11.667), diff(11.3, 6.5, 0.05), 6.0, a, 0.0),
                band("F1500W", "warm dust continuum", w(13.527, 16.64), diff(15.0, 6.5, 0.05), 12.0, a, 0.0),
                band("F2100W", "hot small grains in ionised gas", w(17.989, 24.401), diff(21.0, 6.5, 0.05), 35.0, a, 0.0),
            ];
        }
        "submm" => {
            ins.name = "Sub-millimetre camera".into();
            ins.analog = "15 m single dish, 850 um bolometer array (JCMT SCUBA-2 class)".into();
            ins.kind = Kind::Radio;
            ins.bands = vec![band("850 um", "cold dust: the mass of the cloud", Spec::Freq { ghz: 352.7, bw: 35.0 }, 14.4, 0.0, 0.0, 2.0)];
        }
        "radio_cm" => {
            ins.name = "Centimetre radio interferometer".into();
            ins.analog = "27-dish array at 5 GHz, compact configuration (VLA C class)".into();
            ins.kind = Kind::Radio;
            ins.bands = vec![band("5 GHz", "free-free from ionised gas, unaffected by dust", Spec::Freq { ghz: 5.0, bw: 2.0 }, 4.0, 0.0, 0.0, 0.012)];
        }
        "xray_ccd" => {
            ins.name = "X-ray CCD imager".into();
            ins.analog = "1.2 m grazing-incidence X-ray telescope, CCD imager (Chandra ACIS class)".into();
            ins.kind = Kind::Xray;
            ins.exposure_s = 80_000.0;
            ins.bands = vec![
                band("0.5-1.2 keV", "soft X-rays: hot wind-blown gas", Spec::Energy { lo: 0.5, hi: 1.2 }, 0.8, 4e-7, 300.0, 0.0),
                band("1.2-2 keV", "medium X-rays", Spec::Energy { lo: 1.2, hi: 2.0 }, 0.8, 4e-7, 450.0, 0.0),
                band("2-7 keV", "hard X-rays: young-star coronae through the cloud", Spec::Energy { lo: 2.0, hi: 7.0 }, 1.0, 6e-7, 300.0, 0.0),
            ];
        }
        "xray_survey" => {
            ins.name = "X-ray survey telescope".into();
            ins.analog = "7-mirror wide-field X-ray survey telescope (eROSITA class)".into();
            ins.kind = Kind::Xray;
            ins.exposure_s = 2000.0;
            ins.bands = vec![band("0.5-2 keV", "hot intracluster gas", Spec::Energy { lo: 0.5, hi: 2.0 }, 20.0, 4.4e-7, 1000.0, 0.0)];
        }
        "xray_deep" => {
            ins.name = "X-ray CCD imager (deep)".into();
            ins.analog = "1.2 m grazing-incidence X-ray telescope, CCD imager (Chandra ACIS class)".into();
            ins.kind = Kind::Xray;
            ins.exposure_s = 100_000.0;
            ins.bands = vec![
                band("0.5-2 keV", "hot intracluster gas", Spec::Energy { lo: 0.5, hi: 2.0 }, 1.0, 4e-7, 450.0, 0.0),
                band("2-7 keV", "the hottest, shocked gas", Spec::Energy { lo: 2.0, hi: 7.0 }, 1.2, 6e-7, 300.0, 0.0),
            ];
        }
        "radio_survey" => {
            ins.name = "Radio continuum survey".into();
            ins.analog = "1.4 GHz all-sky interferometric survey (NVSS class)".into();
            ins.kind = Kind::Radio;
            ins.bands = vec![band("1.4 GHz", "synchrotron: radio galaxies, halos and relics", Spec::Freq { ghz: 1.4, bw: 0.1 }, 45.0, 0.0, 0.0, 0.45)];
        }
        "radio_deep" => {
            ins.name = "Radio continuum (deep)".into();
            ins.analog = "64-dish array at 1.28 GHz (MeerKAT class)".into();
            ins.kind = Kind::Radio;
            ins.bands = vec![band("1.28 GHz", "synchrotron: radio galaxies, halos and relics", Spec::Freq { ghz: 1.28, bw: 0.8 }, 7.5, 0.0, 0.0, 0.004)];
        }
        "cmb" => {
            ins.name = "CMB survey receiver".into();
            ins.analog = "6 m CMB telescope, 90/150/220 GHz arrays (ACT/SPT class)".into();
            ins.kind = Kind::Cmb;
            ins.bands = vec![
                band("90 GHz", "SZ decrement: hot electrons in the CMB shadow", Spec::Sz { ghz: 90.0 }, 2.1, 0.0, 0.0, 15.0),
                band("150 GHz", "SZ decrement", Spec::Sz { ghz: 150.0 }, 1.4, 0.0, 0.0, 15.0),
                band("220 GHz", "near the SZ null", Spec::Sz { ghz: 220.0 }, 1.0, 0.0, 0.0, 40.0),
            ];
        }
        "lensing" => {
            ins.name = "Weak-lensing survey".into();
            ins.analog = "ground-based galaxy-shape survey, sources at z = 1 (DES/HSC class)".into();
            ins.kind = Kind::Lensing;
            ins.n_gal = 10.0; ins.smooth_arcmin = 4.0;
            ins.bands = vec![band("convergence", "all mass, dark or not", Spec::Kappa, 0.0, 0.0, 0.0, 0.26)];
        }
        "optical_gri" => {
            let a = 5.3e5 * 0.3;
            ins.name = "Optical survey camera".into();
            ins.analog = "8 m ground telescope, wide-field g/r/i camera (Subaru HSC class)".into();
            ins.exposure_s = 600.0; ins.native_px = 0.17; ins.read_noise = 5.0;
            ins.bands = vec![
                band("g", "young stars", w(0.40, 0.55), 0.75, 0.25, a, 0.0),
                band("r", "stellar mass", w(0.55, 0.69), 0.7, 0.62, a, 0.0),
                band("i", "old stars", w(0.69, 0.82), 0.65, 1.2, a, 0.0),
            ];
        }
        "wise" => {
            let a = 1.1e3 * 0.4;
            ins.name = "Mid-infrared survey".into();
            ins.analog = "40 cm all-sky infrared survey telescope (WISE class)".into();
            ins.exposure_s = 200.0; ins.native_px = 2.75; ins.read_noise = 5.0;
            ins.bands = vec![
                band("W1 3.4", "old stars: stellar mass", w(2.8, 3.8), 6.1, 0.15, a, 0.0),
                band("W2 4.6", "stars and hot dust", w(4.0, 5.2), 6.4, 0.6, a, 0.0),
                band("W3 12", "PAHs: star formation", w(7.5, 16.5), 6.5, 25.0, a, 0.0),
            ];
        }
        _ => return None,
    }
    Some(ins)
}

pub const INSTRUMENTS: &[&str] = &[
    "optical_narrow", "nircam", "miri", "submm", "radio_cm", "xray_ccd", "xray_survey", "xray_deep",
    "radio_survey", "radio_deep", "cmb", "lensing", "optical_gri", "wise",
];
