//! Scene description: everything that defines an observation. Read from and
//! written to JSON so a render, its checkpoint and its exported metadata all
//! carry the exact parameters that produced it.
//!
//! Two scene modes, never mixed:
//!
//! * **telescope** (default): a distant observer. The hole has a physical mass
//!   (solar masses) and distance (parsecs); the observer's radius in
//!   gravitational radii follows from them, r₀ = D / r_g with r_g = GM/c².
//!   Shadow, disk and Einstein radius then have definite angular sizes on the
//!   detector, and the field of view is an angle on the sky.
//! * **local**: a theoretical observer a few tens of M from the hole (`cam_r`
//!   in M). Used for geodesic and critical-curve studies; never described as a
//!   telescope observation.
//!
//! Geometric quantities inside the renderer stay in units of M (G = c = 1,
//! `mass` = 1); the physical mass only enters radiometry and angular scales.

use crate::json::Json;
use crate::math::V3;

/// GM_⊙/c² (m).
pub const R_G_SUN_M: f64 = 1476.625;
/// Parsec (m).
pub const PC_M: f64 = 3.085_677_581e16;

/// One registered exposure of a multi-exposure observation (`count` frames).
#[derive(Clone, Debug, PartialEq)]
pub struct Exposure {
    pub exposure_s: f64,
    pub neutral_density: f64,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub name: String,
    pub seed: u64,
    /// "telescope" (distant observer) or "local" (observer at `cam_r` M).
    pub mode: String,
    // black hole
    /// Geometric mass of the integrator (always 1: lengths are in M).
    pub mass: f64,
    pub bh_mass_msun: f64,
    /// Observer–hole distance (telescope mode).
    pub bh_distance_pc: f64,
    /// false: straight rays (an unlensed comparison image of the same scene).
    pub lensing: bool,
    // camera: static observer at radius `cam_r` (in M), looking at the hole.
    // In telescope mode `cam_r` is derived from the distance.
    pub cam_r: f64,
    pub cam_incl_deg: f64,
    pub cam_azimuth_deg: f64,
    pub fov_deg: f64,
    pub roll_deg: f64,
    /// Aim point offset from the hole (degrees right, up), for off-centre framing.
    pub aim_offset_deg: (f64, f64),
    pub width: usize,
    pub height: usize,
    // disk
    pub disk: bool,
    pub disk_r_in: f64,
    pub disk_r_out: f64,
    /// Accretion rate as a fraction of the Eddington rate, L/L_Edd.
    pub disk_eddington_ratio: f64,
    /// Radiative efficiency η = L/(Ṁc²); ≤ 0: 1 − E(r_in) of the thin disk.
    pub disk_efficiency: f64,
    /// Colour-correction factor f_col (T_col = f_col T_eff, I = f_col⁻⁴ B(T_col)).
    pub disk_color_correction: f64,
    /// Vertical optical depth of the disk; ≤ 0 means optically thick (opaque).
    pub disk_tau: f64,
    /// Artistic multiplier on the disk's physical radiance (1 = physical).
    pub disk_brightness_scale: f64,
    /// Highest photon order shaded (1 = direct image, 2 = + secondary, ...).
    /// Higher orders are traced but left dark: never extrapolated.
    pub disk_max_order: u32,
    // geodesic integration
    pub rtol: f64,
    pub max_windings: f64,
    // sky
    /// "galaxy" (population model), "grid" (calibration grid) or "none".
    pub sky_mode: String,
    pub grid_spacing_deg: f64,
    /// Photon radiance of grid lines and markers (calibration; not physical).
    pub grid_radiance: f64,
    /// Radius of the uniform test disc ("disc" sky mode), centred at the
    /// source offset.
    pub disc_radius_deg: f64,
    pub stars: bool,
    pub star_mag_limit: f64,
    pub star_density: f64,
    pub dust: f64,
    pub galaxies: bool,
    pub galaxy_mag_limit: f64,
    pub galaxy_density: f64,
    pub diffuse: bool,
    pub sky_brightness: f64,
    /// Deeper catalogue inside the field (0 = off): stars and galaxies to these
    /// magnitudes within `deep_radius_deg` of the field centre (auto if ≤ 0).
    pub deep_star_mag_limit: f64,
    pub deep_galaxy_mag_limit: f64,
    pub deep_radius_deg: f64,
    // nebula volume behind the hole (finite-distance source)
    pub volume: Option<String>,
    /// Source centre offset from the observer→hole axis, as seen by the
    /// observer (right, up), in degrees (JSON: `source_offset_arcmin`).
    pub volume_offset_deg: (f64, f64),
    /// Lens→source distance (JSON: `source_distance_pc`).
    pub volume_distance_pc: f64,
    pub volume_rotation_deg: (f64, f64),
    pub volume_brightness: f64,
    pub volume_step: f64,
    // telescope + sensor
    /// Aperture diameter (m); if ≤ 0, focal length / f_ratio.
    pub aperture_m: f64,
    pub f_ratio: f64,
    pub central_obstruction: f64,
    pub spider_vanes: u32,
    /// Optics/seeing blur FWHM on the sky (arcsec), added in quadrature to the
    /// Airy core; converted to pixels at the render's own plate scale.
    pub optics_fwhm_arcsec: f64,
    pub halo_fraction: f64,
    /// Neutral-density filter optical density (transmission 10^−OD).
    pub neutral_density: f64,
    pub exposure_s: f64,
    pub pixel_um: f64,
    pub filters: Vec<String>,
    pub gain_e_per_adu: f64,
    pub read_noise_e: f64,
    pub dark_e_per_s: f64,
    pub full_well_e: f64,
    pub cosmic_rays_per_cm2_s: f64,
    pub psf: bool,
    pub sensor_noise: bool,
    pub shutter_open: bool,
    /// Samples per pixel at which the simulated exposure is complete: the
    /// shutter is notionally open while the renderer accumulates.
    pub target_spp: u32,
    pub notes: String,
    // --- output quality and presentation (recorded in every export)
    /// fixture | validation | presentation | high_quality
    pub output_class: String,
    /// physical_single_exposure | registered_hdr_composite | stacked_survey | diagnostic
    pub presentation_mode: String,
    /// Registered exposures for composites and stacks (empty: one exposure of
    /// `exposure_s` through `neutral_density`).
    pub exposures: Vec<Exposure>,
    /// linear | asinh | filmic | narrowband_sho | hdr_log
    pub display_transform: String,
    pub black_point_percentile: f64,
    pub white_point_percentile: f64,
    pub display_softness: f64,
    /// Highlight shoulder: values above this fraction of white roll off
    /// smoothly instead of clipping (0 = hard clip).
    pub highlight_rolloff: f64,
    /// Per-channel black point at the sky level (neutral dark background).
    pub background_neutralization: bool,
    /// Decades shown below the knee by the hdr_log transform.
    pub hdr_decades: f64,
    /// Debug only: record which volume landmarks each ray passes.
    pub landmarks: bool,
}

impl Default for Scene {
    fn default() -> Self {
        Scene {
            name: "untitled".into(),
            seed: 1977,
            mode: "local".into(),
            mass: 1.0,
            bh_mass_msun: 4.3e6,
            bh_distance_pc: 0.01,
            lensing: true,
            cam_r: 60.0,
            cam_incl_deg: 85.0,
            cam_azimuth_deg: 0.0,
            fov_deg: 32.0,
            roll_deg: 0.0,
            aim_offset_deg: (0.0, 0.0),
            width: 640,
            height: 400,
            disk: true,
            disk_r_in: 6.0,
            disk_r_out: 28.0,
            disk_eddington_ratio: 0.01,
            disk_efficiency: 0.0,
            disk_color_correction: 1.7,
            disk_tau: 0.0,
            disk_brightness_scale: 1.0,
            disk_max_order: 3,
            rtol: 1e-9,
            max_windings: 6.0,
            sky_mode: "galaxy".into(),
            grid_spacing_deg: 10.0,
            grid_radiance: 1e12,
            disc_radius_deg: 1.0,
            stars: true,
            star_mag_limit: 10.5,
            star_density: 1.0,
            dust: 1.0,
            galaxies: true,
            galaxy_mag_limit: 13.0,
            galaxy_density: 1.0,
            diffuse: true,
            sky_brightness: 1.0,
            deep_star_mag_limit: 0.0,
            deep_galaxy_mag_limit: 0.0,
            deep_radius_deg: 0.0,
            volume: None,
            volume_offset_deg: (0.0, 0.0),
            volume_distance_pc: 800.0,
            volume_rotation_deg: (20.0, 35.0),
            volume_brightness: 1.0,
            volume_step: 0.6,
            aperture_m: 0.0,
            f_ratio: 2.8,
            central_obstruction: 0.0,
            spider_vanes: 0,
            optics_fwhm_arcsec: 2.0,
            halo_fraction: 0.01,
            neutral_density: 0.0,
            exposure_s: 120.0,
            pixel_um: 3.76,
            filters: vec!["L".into()],
            gain_e_per_adu: 1.0,
            read_noise_e: 3.0,
            dark_e_per_s: 0.002,
            full_well_e: 50_000.0,
            cosmic_rays_per_cm2_s: 0.0,
            psf: true,
            sensor_noise: true,
            shutter_open: true,
            target_spp: 1024,
            notes: String::new(),
            output_class: "presentation".into(),
            presentation_mode: "physical_single_exposure".into(),
            exposures: Vec::new(),
            display_transform: "asinh".into(),
            black_point_percentile: 0.02,
            white_point_percentile: 0.99995,
            display_softness: 0.08,
            highlight_rolloff: 0.0,
            background_neutralization: false,
            hdr_decades: 8.0,
            landmarks: false,
        }
    }
}

macro_rules! rd {
    ($j:ident, $s:ident, $field:ident, num) => { if let Some(v) = $j.num(stringify!($field)) { $s.$field = v as _; } };
    ($j:ident, $s:ident, $field:ident, bool) => { if let Some(v) = $j.bool(stringify!($field)) { $s.$field = v; } };
    ($j:ident, $s:ident, $field:ident, str) => { if let Some(v) = $j.str(stringify!($field)) { $s.$field = v.to_string(); } };
}

impl Scene {
    pub fn from_json(j: &Json) -> Scene {
        let mut s = Scene::default();
        rd!(j, s, name, str);
        rd!(j, s, seed, num);
        rd!(j, s, mode, str);
        rd!(j, s, bh_mass_msun, num);
        rd!(j, s, bh_distance_pc, num);
        rd!(j, s, lensing, bool);
        rd!(j, s, cam_r, num);
        rd!(j, s, cam_incl_deg, num);
        rd!(j, s, cam_azimuth_deg, num);
        rd!(j, s, fov_deg, num);
        if let Some(v) = j.num("fov_arcmin") { s.fov_deg = v / 60.0; }
        rd!(j, s, roll_deg, num);
        rd!(j, s, f_ratio, num);
        rd!(j, s, spider_vanes, num);
        rd!(j, s, optics_fwhm_arcsec, num);
        rd!(j, s, halo_fraction, num);
        rd!(j, s, target_spp, num);
        if let Some(v) = j.num("samples_per_pixel_target") { s.target_spp = v as u32; }
        rd!(j, s, width, num);
        rd!(j, s, height, num);
        if let Some(v) = j.num("render_width") { s.width = v as usize; }
        if let Some(v) = j.num("render_height") { s.height = v as usize; }
        rd!(j, s, disk, bool);
        rd!(j, s, disk_r_in, num);
        rd!(j, s, disk_r_out, num);
        rd!(j, s, disk_eddington_ratio, num);
        rd!(j, s, disk_efficiency, num);
        rd!(j, s, disk_color_correction, num);
        rd!(j, s, disk_tau, num);
        rd!(j, s, disk_brightness_scale, num);
        rd!(j, s, disk_max_order, num);
        rd!(j, s, rtol, num);
        rd!(j, s, max_windings, num);
        rd!(j, s, sky_mode, str);
        rd!(j, s, grid_spacing_deg, num);
        rd!(j, s, grid_radiance, num);
        rd!(j, s, disc_radius_deg, num);
        rd!(j, s, stars, bool);
        rd!(j, s, star_mag_limit, num);
        rd!(j, s, star_density, num);
        if let Some(v) = j.num("star_density_scale") { s.star_density = v; }
        rd!(j, s, dust, num);
        rd!(j, s, galaxies, bool);
        rd!(j, s, galaxy_mag_limit, num);
        rd!(j, s, galaxy_density, num);
        if let Some(v) = j.num("galaxy_density_scale") { s.galaxy_density = v; }
        rd!(j, s, diffuse, bool);
        rd!(j, s, sky_brightness, num);
        rd!(j, s, deep_star_mag_limit, num);
        rd!(j, s, deep_galaxy_mag_limit, num);
        rd!(j, s, deep_radius_deg, num);
        if let Some(v) = j.str("volume") { s.volume = if v.is_empty() { None } else { Some(v.to_string()) }; }
        let pair = |k: &str, d: (f64, f64)| j.arr(k).filter(|a| a.len() == 2).map(|a| (a[0].as_f64().unwrap_or(d.0), a[1].as_f64().unwrap_or(d.1))).unwrap_or(d);
        let am = pair("source_offset_arcmin", (s.volume_offset_deg.0 * 60.0, s.volume_offset_deg.1 * 60.0));
        s.volume_offset_deg = (am.0 / 60.0, am.1 / 60.0);
        s.volume_rotation_deg = pair("volume_rotation_deg", s.volume_rotation_deg);
        s.aim_offset_deg = pair("aim_offset_deg", s.aim_offset_deg);
        if j.arr("aim_offset_arcmin").is_some() {
            let a = pair("aim_offset_arcmin", (0.0, 0.0));
            s.aim_offset_deg = (a.0 / 60.0, a.1 / 60.0);
        }
        if let Some(v) = j.num("source_distance_pc") { s.volume_distance_pc = v; }
        rd!(j, s, volume_brightness, num);
        rd!(j, s, volume_step, num);
        rd!(j, s, aperture_m, num);
        rd!(j, s, central_obstruction, num);
        rd!(j, s, neutral_density, num);
        rd!(j, s, exposure_s, num);
        rd!(j, s, pixel_um, num);
        if let Some(a) = j.arr("filters") { s.filters = a.iter().filter_map(|f| f.as_str().map(String::from)).collect(); }
        rd!(j, s, gain_e_per_adu, num);
        rd!(j, s, read_noise_e, num);
        rd!(j, s, dark_e_per_s, num);
        rd!(j, s, full_well_e, num);
        rd!(j, s, cosmic_rays_per_cm2_s, num);
        rd!(j, s, psf, bool);
        rd!(j, s, sensor_noise, bool);
        rd!(j, s, shutter_open, bool);
        rd!(j, s, notes, str);
        rd!(j, s, output_class, str);
        rd!(j, s, presentation_mode, str);
        rd!(j, s, display_transform, str);
        rd!(j, s, black_point_percentile, num);
        rd!(j, s, white_point_percentile, num);
        rd!(j, s, display_softness, num);
        rd!(j, s, highlight_rolloff, num);
        rd!(j, s, background_neutralization, bool);
        rd!(j, s, hdr_decades, num);
        rd!(j, s, landmarks, bool);
        if let Some(a) = j.arr("exposures") {
            s.exposures = a.iter().map(|e| Exposure {
                exposure_s: e.num("exposure_s").unwrap_or(s.exposure_s),
                neutral_density: e.num("neutral_density").unwrap_or(0.0),
                count: e.num("count").unwrap_or(1.0).max(1.0) as u32,
            }).collect();
        }
        s.finalize();
        s
    }

    /// Derive dependent quantities: in telescope mode the observer radius (M)
    /// from the physical mass and distance.
    pub fn finalize(&mut self) {
        self.mass = 1.0;
        if self.is_telescope() {
            self.cam_r = self.bh_distance_pc * PC_M / self.r_g_m();
        }
    }

    pub fn is_telescope(&self) -> bool { self.mode != "local" }

    /// Gravitational radius GM/c² (m).
    pub fn r_g_m(&self) -> f64 { R_G_SUN_M * self.bh_mass_msun }

    /// Observer–hole distance (m): r₀ r_g in both modes.
    pub fn observer_distance_m(&self) -> f64 { self.cam_r * self.r_g_m() }

    /// Angular radius of the shadow seen by the static observer at r₀ (exact
    /// for Schwarzschild: sin α = 3√3 √(1 − 2/r₀) / r₀); → 3√3 r_g / D far away.
    pub fn shadow_radius_rad(&self) -> f64 {
        let r0 = self.cam_r;
        (3.0 * 3f64.sqrt() * (1.0 - 2.0 / r0).sqrt() / r0).min(1.0).asin()
    }

    /// Weak-field Einstein radius for a point source a distance D_ls behind the
    /// hole: θ_E = √(4 r_g D_ls / (D_ol D_os)).
    pub fn einstein_radius_rad(&self, d_ls_m: f64) -> f64 {
        let d_ol = self.observer_distance_m();
        if d_ls_m <= 0.0 { return (4.0 / self.cam_r).sqrt(); }
        (4.0 * self.r_g_m() * d_ls_m / (d_ol * (d_ol + d_ls_m))).sqrt()
    }

    pub fn to_json(&self) -> Json {
        let mut j = Json::obj();
        j.set("name", self.name.as_str()).set("seed", self.seed).set("mode", self.mode.as_str())
            .set("bh_mass_msun", self.bh_mass_msun).set("bh_distance_pc", self.bh_distance_pc).set("lensing", self.lensing)
            .set("cam_r", self.cam_r).set("cam_incl_deg", self.cam_incl_deg).set("cam_azimuth_deg", self.cam_azimuth_deg)
            .set("fov_deg", self.fov_deg).set("roll_deg", self.roll_deg).set("width", self.width).set("height", self.height)
            .set("aim_offset_deg", vec![Json::from(self.aim_offset_deg.0), self.aim_offset_deg.1.into()])
            .set("f_ratio", self.f_ratio).set("spider_vanes", self.spider_vanes).set("optics_fwhm_arcsec", self.optics_fwhm_arcsec)
            .set("halo_fraction", self.halo_fraction).set("target_spp", self.target_spp)
            .set("disk", self.disk).set("disk_r_in", self.disk_r_in).set("disk_r_out", self.disk_r_out)
            .set("disk_eddington_ratio", self.disk_eddington_ratio).set("disk_efficiency", self.disk_efficiency)
            .set("disk_color_correction", self.disk_color_correction).set("disk_tau", self.disk_tau)
            .set("disk_brightness_scale", self.disk_brightness_scale).set("disk_max_order", self.disk_max_order)
            .set("rtol", self.rtol).set("max_windings", self.max_windings)
            .set("sky_mode", self.sky_mode.as_str()).set("grid_spacing_deg", self.grid_spacing_deg).set("grid_radiance", self.grid_radiance).set("disc_radius_deg", self.disc_radius_deg)
            .set("stars", self.stars).set("star_mag_limit", self.star_mag_limit).set("star_density", self.star_density)
            .set("dust", self.dust).set("galaxies", self.galaxies).set("galaxy_mag_limit", self.galaxy_mag_limit)
            .set("galaxy_density", self.galaxy_density).set("diffuse", self.diffuse).set("sky_brightness", self.sky_brightness)
            .set("deep_star_mag_limit", self.deep_star_mag_limit).set("deep_galaxy_mag_limit", self.deep_galaxy_mag_limit)
            .set("deep_radius_deg", self.deep_radius_deg)
            .set("volume", self.volume.clone().unwrap_or_default())
            .set("source_offset_arcmin", vec![Json::from(self.volume_offset_deg.0 * 60.0), (self.volume_offset_deg.1 * 60.0).into()])
            .set("volume_rotation_deg", vec![Json::from(self.volume_rotation_deg.0), self.volume_rotation_deg.1.into()])
            .set("source_distance_pc", self.volume_distance_pc).set("volume_brightness", self.volume_brightness)
            .set("volume_step", self.volume_step)
            .set("aperture_m", self.aperture_m).set("central_obstruction", self.central_obstruction)
            .set("neutral_density", self.neutral_density)
            .set("exposure_s", self.exposure_s).set("pixel_um", self.pixel_um)
            .set("filters", self.filters.iter().map(|f| Json::from(f.as_str())).collect::<Vec<_>>())
            .set("gain_e_per_adu", self.gain_e_per_adu).set("read_noise_e", self.read_noise_e)
            .set("dark_e_per_s", self.dark_e_per_s).set("full_well_e", self.full_well_e)
            .set("cosmic_rays_per_cm2_s", self.cosmic_rays_per_cm2_s)
            .set("psf", self.psf).set("sensor_noise", self.sensor_noise).set("shutter_open", self.shutter_open)
            .set("notes", self.notes.as_str())
            .set("output_class", self.output_class.as_str()).set("presentation_mode", self.presentation_mode.as_str())
            .set("exposures", self.exposures.iter().map(|e| { let mut o = Json::obj(); o.set("exposure_s", e.exposure_s).set("neutral_density", e.neutral_density).set("count", e.count); o }).collect::<Vec<_>>())
            .set("display_transform", self.display_transform.as_str()).set("black_point_percentile", self.black_point_percentile)
            .set("white_point_percentile", self.white_point_percentile).set("display_softness", self.display_softness)
            .set("highlight_rolloff", self.highlight_rolloff).set("background_neutralization", self.background_neutralization)
            .set("hdr_decades", self.hdr_decades).set("landmarks", self.landmarks)
            .set("star_density_scale", self.star_density).set("galaxy_density_scale", self.galaxy_density);
        j
    }

    /// Camera position (world) and orthonormal frame (right, up, forward).
    pub fn camera(&self) -> (V3, V3, V3, V3) {
        let i = self.cam_incl_deg.to_radians();
        let a = self.cam_azimuth_deg.to_radians();
        // inclination measured from the disk normal (+y)
        let pos = V3::new(i.sin() * a.sin(), i.cos(), i.sin() * a.cos()) * self.cam_r;
        let fwd = (-pos).norm();
        let mut right = fwd.cross(V3::new(0.0, 1.0, 0.0));
        if right.len() < 1e-9 { right = V3::new(1.0, 0.0, 0.0); }
        let right = right.norm();
        let up = right.cross(fwd);
        let r = self.roll_deg.to_radians();
        let (right, up) = (right * r.cos() + up * r.sin(), up * r.cos() - right * r.sin());
        let (ox, oy) = (self.aim_offset_deg.0.to_radians().tan(), self.aim_offset_deg.1.to_radians().tan());
        let fwd2 = (fwd + right * ox + up * oy).norm();
        let right2 = fwd2.cross(up).norm();
        let up2 = right2.cross(fwd2);
        (pos, right2, up2, fwd2)
    }

    /// Angular size of one pixel (radians), from the field of view.
    pub fn pixel_scale(&self) -> f64 {
        2.0 * (self.fov_deg.to_radians() / 2.0).tan() / self.width as f64
    }

    /// Focal length implied by the field of view and pixel pitch (mm).
    pub fn focal_length_mm(&self) -> f64 {
        self.pixel_um * 1e-3 / self.pixel_scale()
    }

    pub fn aperture(&self) -> f64 {
        if self.aperture_m > 0.0 { self.aperture_m } else { self.focal_length_mm() * 1e-3 / self.f_ratio }
    }

    /// Effective collecting area (m²).
    /// The exposure schedule: the explicit list, or the single exposure.
    pub fn exposure_list(&self) -> Vec<Exposure> {
        if self.exposures.is_empty() {
            vec![Exposure { exposure_s: self.exposure_s, neutral_density: self.neutral_density, count: 1 }]
        } else {
            self.exposures.clone()
        }
    }

    pub fn collecting_area(&self) -> f64 {
        let d = self.aperture();
        std::f64::consts::PI * d * d / 4.0 * (1.0 - self.central_obstruction.powi(2))
    }
}
