//! Multiwavelength Survey: the Observatory's instruments beyond visible light.
//!
//! The lensing scenes ask what one optical camera records near a black hole.
//! This module asks a different question about objects where no single band
//! tells the story: what does each instrument (infrared, sub-millimetre, radio,
//! X-ray, microwave, weak lensing) see, and why is it different?
//!
//! Two generated scenes, each described by physical state rather than colours:
//!
//! * `pillars`: a molecular cloud eroded by the ionising light of a nearby
//!   cluster into dense columns with photoevaporating skins, embedded young
//!   stars and a hot wind-blown cavity (Eagle-Nebula-like, *not* a map of it);
//! * `supercluster`: a constrained ΛCDM realisation evolved with the truncated
//!   Zel'dovich approximation, with clusters, a warm–hot intergalactic medium,
//!   galaxies, radio sources and the cosmic microwave background.
//!
//! Every band computes its emissivity j_ν and absorption α_ν from that state
//! (`phys`), the renderer integrates them along rays through the same volume
//! (`obs`), and each instrument applies its own beam, units and noise. No band
//! is painted: if the infrared shows stars the optical hides, it is because
//! the dust column along those rays is optically thin at 2 µm.
//!
//! Labels as elsewhere in the Observatory: *physically derived*, *numerically
//! approximated*, *statistically synthesized*, *empirical*.

pub mod bands;
pub mod cosmic;
pub mod field;
pub mod obs;
pub mod phys;
pub mod pillars;
pub mod show;
pub mod survey;

use crate::json::Json;

/// Per-voxel physical state, all in cgs.
pub const F_NH: usize = 0; // total hydrogen density (cm⁻³); electron density for cosmic gas
pub const F_XION: usize = 1; // ionised fraction
pub const F_TGAS: usize = 2; // gas temperature (K)
pub const F_U: usize = 3; // starlight intensity relative to the ISRF
pub const F_FDUST: usize = 4; // dust abundance relative to Galactic
pub const F_FPAH: usize = 5; // PAH abundance relative to Galactic
pub const F_NHOT: usize = 6; // hot-plasma electron density (cm⁻³)
pub const F_THOT: usize = 7; // hot-plasma temperature (K)
pub const F_COLSURF: usize = 8; // column to the nearest ionised gas (cm⁻²)
pub const F_LOGU: usize = 9; // log₁₀ ionisation parameter of ionised gas
pub const F_FRONT: usize = 10; // proximity of the ionisation front, 0–1
pub const F_USURF: usize = 11; // starlight intensity at the nearest PDR surface
pub const F_NSRC: usize = 12; // dust column (H-equivalent) to the illuminating cluster (cm⁻²)
pub const F_NE: usize = 13; // electron density of the warm ionised gas (cm⁻³)
pub const MF: usize = 14;
pub const FIELD_NAMES: [&str; MF] = ["n_H", "x_ion", "T_gas", "U", "f_dust", "f_PAH", "n_hot", "T_hot", "N_surf", "log_U_ion", "front", "U_surf", "N_src", "n_e"];

/// A generated volume: n³ cells of side `dx` cm, centred on `center` (cm,
/// observer at the origin looking along +z).
pub struct MwVolume {
    pub n: usize,
    pub dx: f64,
    pub center: [f64; 3],
    pub f: Vec<Vec<f32>>,
    /// Metallicity of hot gas (solar units) for X-ray lines.
    pub metallicity: f64,
    /// Cosmological redshift of the volume centre (0 for Galactic scenes).
    pub z: f64,
    /// Foreground (Galactic) visual extinction in front of the whole volume.
    pub fg_av: f64,
}

impl MwVolume {
    pub fn new(n: usize, dx: f64, center: [f64; 3]) -> MwVolume {
        MwVolume { n, dx, center, f: (0..MF).map(|_| vec![0f32; n * n * n]).collect(), metallicity: 1.0, z: 0.0, fg_av: 0.0 }
    }
    pub fn half(&self) -> f64 { 0.5 * self.n as f64 * self.dx }
}

/// Spectral energy distribution of a point or compact source.
#[derive(Clone, Debug)]
pub enum Sed {
    /// Blackbody star: bolometric luminosity (L☉), temperature (K).
    Star { l: f64, t: f64 },
    /// Young stellar object: photosphere plus warm envelope/disk (grey body
    /// carrying `f_env` of the luminosity at `t_env`), and coronal X-rays.
    Yso { l: f64, t: f64, f_env: f64, t_env: f64, lx: f64, kt_x: f64 },
    /// Galaxy: stellar mass (M☉), star-formation rate (M☉ yr⁻¹), radio
    /// luminosity at 1.4 GHz (erg s⁻¹ Hz⁻¹) from star formation and AGN.
    Galaxy { mstar: f64, sfr: f64, l14: f64, alpha: f64 },
    /// Pure synchrotron source (radio lobe or core).
    Radio { l14: f64, alpha: f64 },
}

#[derive(Clone, Debug)]
pub struct Source {
    /// Position in cm (observer at origin).
    pub pos: [f64; 3],
    pub sed: Sed,
    /// Angular Gaussian size (σ, radians) of an extended source; 0 = point.
    pub size: f64,
    /// Galactic foreground A_V not already counted by the volume.
    pub a_v_fg: f64,
    /// Cosmological redshift (including peculiar velocity) for cosmic sources.
    pub z: f64,
    /// Luminosity distance, cm (equals |pos| for Galactic sources).
    pub d_l: f64,
    /// Catalogue flags: bit 0 = embedded YSO, bit 1 = quenched galaxy, bit 2 = cluster member.
    pub flags: u8,
}

/// An analytic galaxy cluster: elliptical β-model gas with an NFW mass profile.
#[derive(Clone, Debug)]
pub struct Halo {
    pub pos: [f64; 3],
    pub m500: f64,
    pub r500: f64,
    pub rc: f64,
    pub beta: f64,
    pub ne0: f64,
    pub t: f64,
    pub q: f64,
    pub pa: f64,
    pub z: f64,
    /// Radio halo power at 1.4 GHz (erg s⁻¹ Hz⁻¹), 0 if none.
    pub radio_halo: f64,
    /// Relics: (angle of merger axis, distance of each relic from centre, power each).
    pub relics: Option<(f64, f64, f64)>,
    pub c200: f64,
    pub m200: f64,
    pub r200: f64,
}

/// A generated scene: a volume plus its compact and analytic components.
pub struct MwScene {
    pub kind: String,
    pub vol: MwVolume,
    pub sources: Vec<Source>,
    pub halos: Vec<Halo>,
    /// Matter tracer particles (comoving Mpc, observer at origin) for weak lensing.
    pub particles: Option<Vec<[f32; 3]>>,
    /// Cosmology (h, Ω_m, box side in Mpc) for cosmic scenes.
    pub cosmo: Option<(f64, f64, f64)>,
    /// Ionising-cluster position (pillars), cm, and its luminosities.
    pub cluster: Option<([f64; 3], f64, f64, f64)>, // pos, Q (s⁻¹), L_bol (L☉), T_eff
    pub meta: Json,
    pub seed: u64,
}

/// Map `f` over 0..n, in parallel threads natively and serially in the browser
/// build (WebAssembly here has no threads; the web app parallelises by worker).
pub fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    #[cfg(target_arch = "wasm32")]
    { (0..n).map(f).collect() }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let nt = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(4).min(16).max(1);
        let chunk = n.div_ceil(nt).max(1);
        let f = &f;
        let parts: Vec<Vec<T>> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..nt).map(|t| sc.spawn(move || (t * chunk..((t + 1) * chunk).min(n)).map(f).collect::<Vec<T>>())).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        parts.into_iter().flatten().collect()
    }
}
