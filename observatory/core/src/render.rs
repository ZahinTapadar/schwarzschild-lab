//! Progressive Monte-Carlo accumulation of the expected photon radiance.
//!
//! Each pixel estimates  L̄(x, y) = (1/N) Σᵢ L(ray through a random point of
//! the pixel)  for every spectral channel. Per sample, the stochastic choices
//! are the sub-pixel position (box filter over the pixel) and the start
//! offset of the volume ray-march (jittered stepping). Spectra are carried as
//! fixed bins, so there is no wavelength noise. The aperture is not sampled:
//! with apertures of millimetres to metres against black-hole scales of
//! kilometres or more, parallax across the aperture is negligible; the
//! aperture enters through the collecting area and the diffraction PSF
//! (sensor.rs). The camera is a static observer, so there is no exposure-time
//! dependence to sample either (the disk model is stationary).
//!
//! Every sample is keyed by (seed, pixel, sample index), so a render is
//! reproducible bit-for-bit whatever the thread count, and resuming a
//! checkpoint continues exactly where the previous run stopped.

use crate::disk::{DiskModel, DiskPhysics};
use crate::geodesic::{self, Crossing, Disk, Fate, Tolerance};
use crate::math::{basis, V3};
use crate::rng::Rng;
use crate::scene::Scene;
use crate::sky::{cell_index, DeepParams, Sky, SkyParams, INDEX_LEVEL, GAL_LEVEL};
use crate::spectrum::{self, bin_center, extinction_ratio, planck_photon, Filter, LINES, NB, NCH};
use crate::volume::{Volume, BRICK, F_ALPHA, F_SCA, NF};

pub const ND: usize = 18;
pub const D_STEPS: usize = 0; // geodesic steps
pub const D_WIND: usize = 1; // windings φ/2π
pub const D_G: usize = 2; // Σ g over disk hits
pub const D_GN: usize = 3; // disk-hit count
pub const D_TAU: usize = 4; // volume τ_V
pub const D_NEB: usize = 5; // nebula line radiance (photons s⁻¹ m⁻² sr⁻¹)
pub const D_ESC: usize = 6;
pub const D_CAPT: usize = 7;
pub const D_FAIL: usize = 8;
pub const D_ORDER: usize = 9; // Σ photon order of disk hits
pub const D_VSTEPS: usize = 10;
pub const D_UNRES: usize = 11; // disk hits beyond max order (left dark)
pub const D_DISK1: usize = 12; // opaque disk hit at photon order 1
pub const D_DISK2: usize = 13; // order 2
pub const D_DISK3: usize = 14; // resolved order ≥ 3
pub const D_INV: usize = 15; // Σ log₁₀ of the first-integral drift
pub const D_LMK: usize = 16; // Σ (landmark id + 1) of rays passing a volume landmark (debug)
pub const D_LMKN: usize = 17; // count of such rays

const PC_CM: f64 = 3.0857e18;

pub struct FilterInfo {
    pub name: String,
    /// e⁻ s⁻¹ per unit spectral channel radiance, per pixel (filter × QE ×
    /// bin width × collecting area × pixel solid angle).
    pub w: [f64; NCH],
    pub lambda_eff: f64,
}

pub struct VolumePlacement {
    pub vol: Volume,
    pub center_pc: V3,
    /// Rows of the world→volume rotation.
    pub rot: [V3; 3],
    /// Per-channel extinction ratio A_λ/A_V for the volume's dust.
    pub ext: [f32; NCH],
    /// Scattered-light spectral shape relative to 550 nm.
    pub sca_shape: [f32; NB],
    /// Foreground Galactic transmission per channel toward the volume.
    pub fg: [f32; NCH],
    pub brightness: f64,
    pub step: f64,
    /// Fields fade to zero over this many voxels at the box faces (0 = off).
    pub edge_fade: f64,
    /// Debug landmarks (voxel coordinates): the brightest separated H α knots.
    pub landmarks: Vec<[f64; 3]>,
}

pub struct Ctx {
    pub scene: Scene,
    pub sky: Option<Sky>,
    pub disk: Option<DiskModel>,
    pub vol: Option<VolumePlacement>,
    pub filters: Vec<FilterInfo>,
    pub cam: (V3, V3, V3, V3),
    pub tol: Tolerance,
    pub g_sky: f64,
    pub tan_half: f64,
    pub star_sigma: f64,
    /// Observed spectra (with the sky's uniform frequency shift g_sky applied
    /// once per scene, since every escaped ray of a static observer has the
    /// same g): per star, per galaxy (unit-flux profile multiplies it), and
    /// the shifted diffuse map.
    star_spec: Vec<[f32; NB]>,
    gal_spec: Vec<[f32; NB]>,
    deep_star_spec: Vec<[f32; NB]>,
    deep_gal_spec: Vec<[f32; NB]>,
    /// Calibration-sky frame: axis behind the hole, right, up.
    grid_frame: (V3, V3, V3),
    /// Distance (kpc) of the nebula's near face from the observer; catalogue
    /// stars closer than this are not attenuated by it.
    pub vol_front_kpc: f64,
    /// Precomputed channel extinction ratios (continuum bin centres, lines).
    pub ext_ratio: [f32; NCH],
}

pub struct Accum {
    pub w: usize,
    pub h: usize,
    pub spp: u32,
    pub sum: Vec<f32>,
    pub lsum: Vec<f64>,
    pub lsq: Vec<f64>,
    pub diag: Vec<f32>,
}

impl Accum {
    pub fn new(w: usize, h: usize) -> Accum {
        Accum { w, h, spp: 0, sum: vec![0.0; w * h * NCH], lsum: vec![0.0; w * h], lsq: vec![0.0; w * h], diag: vec![0.0; w * h * ND] }
    }
    pub fn reset(&mut self) {
        self.spp = 0;
        self.sum.fill(0.0);
        self.lsum.fill(0.0);
        self.lsq.fill(0.0);
        self.diag.fill(0.0);
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct PassStats {
    pub samples: u64,
    pub steps: u64,
    pub vol_steps: u64,
    pub vol_rays: u64,
    pub failed: u64,
    pub escaped: u64,
    pub captured: u64,
    pub disk: u64,
}

impl Ctx {
    pub fn new(scene: Scene, volume: Option<Volume>, log: &mut dyn FnMut(&str)) -> Ctx {
        Ctx::with_sky(scene, None, volume, log)
    }

    /// Like `new`, but reuse an already generated sky catalogue for this
    /// scene's seed and sky parameters (`None` generates it).
    pub fn with_sky(scene: Scene, prebuilt: Option<Sky>, volume: Option<Volume>, log: &mut dyn FnMut(&str)) -> Ctx {
        let cam = scene.camera();
        let tol = Tolerance { rtol: scene.rtol, atol: scene.rtol * 1e-3, max_step: 0.2, max_steps: 40_000, max_windings: scene.max_windings };
        let g_sky = 1.0 / (1.0 - 2.0 * scene.mass / scene.cam_r).sqrt();
        let px = scene.pixel_scale();
        let wants_sky = scene.sky_mode == "galaxy" && (scene.stars || scene.diffuse || scene.galaxies);
        let deep = (scene.deep_star_mag_limit > 0.0 || scene.deep_galaxy_mag_limit > 0.0).then(|| {
            let diag = (scene.fov_deg.to_radians() / 2.0) * (1.0 + (scene.height as f64 / scene.width as f64).powi(2)).sqrt();
            DeepParams {
                center: cam.3,
                radius: if scene.deep_radius_deg > 0.0 { scene.deep_radius_deg.to_radians() } else { 1.25 * diag },
                star_limit: scene.deep_star_mag_limit.max(scene.star_mag_limit),
                gal_limit: scene.deep_galaxy_mag_limit.max(scene.galaxy_mag_limit),
                cell: 6.0 * px,
            }
        });
        let sky = if !wants_sky { None } else if let Some(s) = prebuilt { Some(s) } else { Some({
            let t0 = now();
            let s = Sky::generate(&SkyParams {
                seed: scene.seed,
                mag_limit: scene.star_mag_limit,
                dust: scene.dust,
                density: scene.star_density,
                galaxy_mag_limit: scene.galaxy_mag_limit,
                galaxy_density: scene.galaxy_density,
                diffuse_w: 512,
                deep,
            });
            log(&format!("sky: {} stars to V={:.1}, {} galaxies to {:.1}, diffuse map {}×{} ({:.1} s)", s.stars.len(), scene.star_mag_limit, s.galaxies.len(), scene.galaxy_mag_limit, s.dw, s.dw / 2, now() - t0));
            if let Some(df) = &s.deep {
                log(&format!("deep field: {} stars to V={:.1}, {} galaxies to {:.1} within {:.1}′ ({}² index cells)", df.stars.len(), df.star_limit, df.galaxies.len(), df.gal_limit, df.radius.to_degrees() * 60.0, df.n));
            }
            s
        }) };
        let disk = scene.disk.then(|| {
            let d = DiskModel::new(scene.mass, scene.disk_r_in, scene.disk_r_out, DiskPhysics {
                mass_msun: scene.bh_mass_msun,
                eddington_ratio: scene.disk_eddington_ratio,
                efficiency: scene.disk_efficiency,
                color_correction: scene.disk_color_correction,
                brightness_scale: scene.disk_brightness_scale,
            });
            log(&format!("disk: M = {:.3e} M☉, L/L_Edd = {}, η = {:.4}, Ṁ = {:.3e} M☉/yr, T_eff,max = {:.0} K at r = {:.2} M, f_col = {}, max H/r = {:.4}",
                scene.bh_mass_msun, scene.disk_eddington_ratio, d.eta, d.mdot * 3.156e7 / 1.989e30, d.t_peak, d.r_peak, scene.disk_color_correction, d.h_over_r_max));
            d
        });

        let mut ext_ratio = [0f32; NCH];
        for i in 0..NB { ext_ratio[i] = extinction_ratio(bin_center(i)) as f32; }
        for (j, (_, l)) in LINES.iter().enumerate() { ext_ratio[NB + j] = extinction_ratio(l / g_sky) as f32; }

        let vol = volume.map(|v| {
            // offsets are measured from the camera→hole axis (not the aim point)
            let fwd = (-cam.0).norm();
            let r0 = fwd.cross(V3::new(0.0, 1.0, 0.0));
            let right = if r0.len() > 1e-9 { r0.norm() } else { V3::new(1.0, 0.0, 0.0) };
            let up = right.cross(fwd);
            let (az, el) = (scene.volume_offset_deg.0.to_radians(), scene.volume_offset_deg.1.to_radians());
            let dir = (fwd + right * az.tan() + up * el.tan()).norm();
            // the offset is the angle at the observer; the source sits D_ol + D_ls
            // from the observer along it (hole at the origin)
            let d_ol_pc = scene.observer_distance_m() / crate::scene::PC_M;
            let center = cam.0 * (d_ol_pc / scene.cam_r) + dir * (d_ol_pc + scene.volume_distance_pc);
            let (yaw, pitch) = (scene.volume_rotation_deg.0.to_radians(), scene.volume_rotation_deg.1.to_radians());
            // world→volume: rotate by −yaw about y, then −pitch about x
            let ry = |p: V3, a: f64| V3::new(p.x * a.cos() - p.z * a.sin(), p.y, p.x * a.sin() + p.z * a.cos());
            let rx = |p: V3, a: f64| V3::new(p.x, p.y * a.cos() - p.z * a.sin(), p.y * a.sin() + p.z * a.cos());
            let m = |e: V3| rx(ry(e, -yaw), -pitch);
            let (ex, ey, ez) = (m(V3::new(1.0, 0.0, 0.0)), m(V3::new(0.0, 1.0, 0.0)), m(V3::new(0.0, 0.0, 1.0)));
            let rot = [V3::new(ex.x, ey.x, ez.x), V3::new(ex.y, ey.y, ez.y), V3::new(ex.z, ey.z, ez.z)];
            let mut sca_shape = [0f32; NB];
            for i in 0..NB {
                let l = bin_center(i) * g_sky;
                sca_shape[i] = (planck_photon(l, v.sca_temp) / planck_photon(550.0, v.sca_temp) * extinction_ratio(l) * g_sky.powi(4)) as f32;
            }
            // foreground extinction from the Galactic model out to the volume distance
            let mut fg = [1f32; NCH];
            if let Some(s) = &sky {
                let g = s.to_gal(dir);
                let l = crate::sky::los(scene.seed, g, scene.dust, scene.star_density);
                let d_kpc = scene.volume_distance_pc / 1000.0;
                let mut av = 0.0;
                for k in 0..l.r.len() { if l.r[k] <= d_kpc { av = l.a_v[k]; } }
                for c in 0..NCH { fg[c] = 10f64.powf(-0.4 * av * ext_ratio[c] as f64) as f32; }
                log(&format!("volume: {} pc box at {:.0} pc, foreground A_V = {:.2}, occupied bricks {:.0}%", v.n as f64 * v.dx, scene.volume_distance_pc, av, v.occupied_fraction() * 100.0));
            }
            let landmarks = if scene.landmarks { find_landmarks(&v, 5) } else { Vec::new() };
            VolumePlacement { vol: v, center_pc: center, rot, ext: ext_ratio, sca_shape, fg, brightness: scene.volume_brightness, step: scene.volume_step, edge_fade: 3.0, landmarks }
        });

        let filters = make_filters(&scene, g_sky);

        let mut sky = sky;
        let mut star_spec = Vec::new();
        let mut gal_spec = Vec::new();
        let mut deep_star_spec = Vec::new();
        let mut deep_gal_spec = Vec::new();
        if let Some(sk) = sky.as_mut() {
            let sb = scene.sky_brightness as f32;
            let sspec = |s: &crate::sky::Star| {
                let mut sf = [0f32; NB];
                spectrum::star_flux((s.v - s.a_v) as f64, s.t as f64, g_sky, s.a_v as f64, &mut sf);
                for v in sf.iter_mut() { *v *= sb; }
                sf
            };
            let gspec = |gx: &crate::sky::Galaxy| {
                let temp = if gx.kind == 0 { 4600.0 } else { 5600.0 };
                let gz = g_sky / (1.0 + gx.z as f64);
                let norm = 1.0e8 * 10f64.powf(-0.4 * gx.mag as f64) / planck_photon(550.0, temp);
                let mut sf = [0f32; NB];
                for i in 0..NB { sf[i] = sb * (norm * planck_photon(bin_center(i) * gz, temp) * gz.powi(4)) as f32; }
                sf
            };
            star_spec = sk.stars.iter().map(sspec).collect();
            gal_spec = sk.galaxies.iter().map(gspec).collect();
            if let Some(df) = &sk.deep {
                deep_star_spec = df.stars.iter().map(sspec).collect();
                deep_gal_spec = df.galaxies.iter().map(gspec).collect();
            }
            // N_obs(λ) = g⁴ N(gλ) for the diffuse map, by interpolation between bins
            for cell in sk.diffuse.iter_mut() {
                let src = *cell;
                for i in 0..NB {
                    let x = (bin_center(i) * g_sky - spectrum::LAMBDA0) / spectrum::BIN_W - 0.5;
                    let k = (x.floor().max(0.0) as usize).min(NB - 2);
                    let t = (x - k as f64).clamp(0.0, 1.0) as f32;
                    cell[i] = sb * (src[k] * (1.0 - t) + src[k + 1] * t) * g_sky.powi(4) as f32;
                }
            }
            sk.refresh_poles();
        }
        if let Some(sk) = &sky {
            let g = sk.to_gal(cam.3);
            log(&format!("field centre: galactic l = {:.1}°, b = {:.1}°", g.x.atan2(g.z).to_degrees(), g.y.clamp(-1.0, 1.0).asin().to_degrees()));
        }
        log(&format!("scale: {} mode, r₀ = {:.4e} M, D = {:.4e} pc, shadow radius {:.3}″, Einstein radius (source at ∞) {:.2}′, field {:.2}′ × {:.2}′, {:.3}″/px",
            scene.mode, scene.cam_r, scene.observer_distance_m() / crate::scene::PC_M, scene.shadow_radius_rad().to_degrees() * 3600.0,
            scene.einstein_radius_rad(0.0).to_degrees() * 60.0, scene.fov_deg * 60.0, scene.fov_deg * 60.0 * scene.height as f64 / scene.width as f64, px.to_degrees() * 3600.0));
        let gfwd = (-cam.0).norm();
        let gr = gfwd.cross(V3::new(0.0, 1.0, 0.0));
        let gright = if gr.len() > 1e-9 { gr.norm() } else { V3::new(1.0, 0.0, 0.0) };
        let grid_frame = (gfwd, gright, gright.cross(gfwd));
        let vol_front_kpc = vol.as_ref().map_or(f64::INFINITY, |v| (scene.observer_distance_m() / crate::scene::PC_M + scene.volume_distance_pc - 0.5 * v.vol.n as f64 * v.vol.dx) / 1000.0);
        Ctx { tan_half: (scene.fov_deg.to_radians() / 2.0).tan(), star_sigma: 0.35 * px, star_spec, gal_spec, deep_star_spec, deep_gal_spec, grid_frame, vol_front_kpc, scene, sky, disk, vol, filters, cam, tol, g_sky, ext_ratio }
    }

    fn primary_rate(&self, spec: &[f32; NCH]) -> f64 {
        match self.filters.first() {
            Some(f) => (0..NCH).map(|c| spec[c] as f64 * f.w[c]).sum(),
            None => 0.0,
        }
    }

    /// World direction of the ray through image position (x, y) in pixels.
    pub fn pixel_dir(&self, x: f64, y: f64) -> V3 {
        let s = &self.scene;
        let (_, right, up, fwd) = self.cam;
        let aspect = s.width as f64 / s.height as f64;
        let sx = (x / s.width as f64 * 2.0 - 1.0) * self.tan_half;
        let sy = (1.0 - y / s.height as f64 * 2.0) * self.tan_half / aspect;
        (fwd + right * sx + up * sy).norm()
    }

    /// Image parity of the screen → sky mapping at each pixel centre, from
    /// finite differences of three rays (quarter-pixel apart): +1 where the
    /// sky appears with its unlensed orientation, −1 where it is mirrored
    /// (secondary images), 0 where the centre ray does not escape.
    pub fn parity_map(&self) -> Vec<i8> {
        let (w, h) = (self.scene.width, self.scene.height);
        let none = Disk { r_in: self.scene.disk_r_in, r_out: self.scene.disk_r_out, max_order: if self.disk.is_some() { u32::MAX } else { 0 } };
        let pos = self.cam.0;
        let orient = |a: V3, b: V3, c: V3| (b - a).cross(c - a).dot(a).signum();
        let mut out = vec![0i8; w * h];
        for py in 0..h {
            for px in 0..w {
                let (x, y) = (px as f64 + 0.5, py as f64 + 0.5);
                let ds = [self.pixel_dir(x, y), self.pixel_dir(x + 0.25, y), self.pixel_dir(x, y + 0.25)];
                let mut ss = [V3::ZERO; 3];
                let mut ok = true;
                for k in 0..3 {
                    let t = geodesic::trace(self.scene.mass, pos, ds[k], &none, &self.tol);
                    if t.fate != Fate::Escaped { ok = false; break; }
                    ss[k] = t.dir;
                }
                if ok { out[py * w + px] = (orient(ss[0], ss[1], ss[2]) * orient(ds[0], ds[1], ds[2])) as i8; }
            }
        }
        out
    }

    /// Trace one sample through pixel (px, py) at sub-pixel offset (u, v).
    pub fn sample(&self, px: usize, py: usize, u: f64, v: f64, rng: &mut Rng, spec: &mut [f32; NCH], diag: &mut [f32; ND], st: &mut PassStats) {
        spec.fill(0.0);
        let s = &self.scene;
        let (pos, right, up, fwd) = self.cam;
        let aspect = s.width as f64 / s.height as f64;
        let sx = ((px as f64 + u) / s.width as f64 * 2.0 - 1.0) * self.tan_half;
        let sy = (1.0 - (py as f64 + v) / s.height as f64 * 2.0) * self.tan_half / aspect;
        let d = (fwd + right * sx + up * sy).norm();
        if !s.lensing {
            // comparison image: straight rays, no hole, no disk
            st.samples += 1;
            st.escaped += 1;
            diag[D_ESC] += 1.0;
            self.sky_radiance(d, rng, spec, diag, st);
            return;
        }
        let disk = Disk { r_in: s.disk_r_in, r_out: s.disk_r_out, max_order: if self.disk.is_some() { u32::MAX } else { 0 } };
        // Disk crossings: an optically thick disk stops the ray; with a finite
        // vertical optical depth τ the slab emits S(1 − e^{−τ/μ}) and passes
        // e^{−τ/μ} of what lies behind it (experimental).
        let mut trans = 1f64;
        let mut first_order = 0u32;
        let mut emitted = [0f64; NB];
        let thick = s.disk_tau <= 0.0;
        let max_order = s.disk_max_order;
        let dm = self.disk.as_ref();
        let (cam_r, tau_z) = (s.cam_r, s.disk_tau);
        let mut g_hit = 0.0;
        // λ = L_z/E of this photon, from its initial direction (as in trace)
        let lambda0 = {
            let r0 = pos.len();
            let rhat = pos * (1.0 / r0);
            let td = d - rhat * d.dot(rhat);
            let sa = td.len();
            if sa < 1e-12 { 0.0 } else { -(r0 * sa / (1.0 - 2.0 * s.mass / r0).sqrt()) * rhat.cross(td * (1.0 / sa)).y }
        };
        let t = {
            let mut on_disk = |c: &Crossing| -> bool {
                if first_order == 0 { first_order = c.order; }
                let Some(dm) = dm else { return true };
                if c.order > max_order { return true; }
                if thick { return true; }
                let tau = tau_z / c.mu.max(1e-3);
                let a = 1.0 - (-tau).exp();
                let mut b = [0f32; NB];
                let g = dm.redshift(c.r, cam_r, lambda0);
                dm.observed(c.r, g, &mut b);
                for i in 0..NB { emitted[i] += trans * a * b[i] as f64; }
                if g_hit == 0.0 { g_hit = g; }
                trans *= 1.0 - a;
                trans < 1e-4
            };
            geodesic::trace_with(s.mass, pos, d, &disk, &self.tol, &mut on_disk)
        };
        st.samples += 1;
        st.steps += t.steps as u64;
        diag[D_STEPS] += t.steps as f32;
        diag[D_WIND] += (t.phi / (2.0 * std::f64::consts::PI)) as f32;
        diag[D_INV] += t.invariant_error.max(1e-17).log10() as f32;
        match t.fate {
            Fate::Escaped => {
                st.escaped += 1;
                diag[D_ESC] += 1.0;
                self.sky_radiance(t.dir, rng, spec, diag, st);
            }
            Fate::Disk => {
                st.disk += 1;
                if t.crossings > s.disk_max_order {
                    diag[D_UNRES] += 1.0;
                } else if let Some(dm) = &self.disk {
                    // photon angular momentum about +y (the photon travels toward the camera)
                    let lambda = -t.b * t.normal.y;
                    let g = dm.redshift(t.r_hit, s.cam_r, lambda);
                    let mut b = [0f32; NB];
                    dm.observed(t.r_hit, g, &mut b);
                    if thick {
                        spec[..NB].copy_from_slice(&b);
                    } else {
                        for i in 0..NB { spec[i] = (emitted[i] + trans * b[i] as f64) as f32; }
                    }
                    diag[D_G] += g as f32;
                    diag[D_GN] += 1.0;
                    diag[D_ORDER] += t.crossings as f32;
                    diag[[D_DISK1, D_DISK2, D_DISK3][(t.crossings.max(1) - 1).min(2) as usize]] += 1.0;
                }
            }
            Fate::Captured => {
                st.captured += 1;
                diag[D_CAPT] += 1.0;
            }
            Fate::Failed => {
                st.failed += 1;
                diag[D_FAIL] += 1.0;
            }
        }
        if !thick && t.fate != Fate::Disk && first_order > 0 && self.disk.is_some() {
            // partly transparent disk in front of an escaped or captured ray
            for c in 0..NCH { spec[c] *= trans as f32; }
            for i in 0..NB { spec[i] += emitted[i] as f32; }
            if g_hit > 0.0 { diag[D_G] += g_hit as f32; diag[D_GN] += 1.0; }
        }
    }

    /// Radiance arriving from sky direction d (world frame, at infinity).
    fn sky_radiance(&self, d: V3, rng: &mut Rng, spec: &mut [f32; NCH], diag: &mut [f32; ND], st: &mut PassStats) {
        // 1. nebula in front of the far sky: emission + per-channel transmission
        let mut trans = [1f32; NCH];
        let mut neb = [0f32; NCH];
        if let Some(vp) = &self.vol {
            let mut lmk = -1i32;
            let (tau_v, steps, hit) = march_landmarks(vp, d, rng.f64(), &mut neb, &mut trans, &mut lmk);
            if lmk >= 0 { diag[D_LMK] += (lmk + 1) as f32; diag[D_LMKN] += 1.0; }
            if hit {
                st.vol_rays += 1;
                st.vol_steps += steps as u64;
                diag[D_VSTEPS] += steps as f32;
                diag[D_TAU] += tau_v as f32;
                for c in 0..NCH { neb[c] *= vp.fg[c] * vp.brightness as f32; }
                diag[D_NEB] += neb[NB..].iter().sum::<f32>();
            }
        }
        match self.scene.sky_mode.as_str() {
            "grid" | "disc" => {
                let mut cal = [0f32; NB];
                self.calibration_sky(d, &mut cal);
                for i in 0..NB { spec[i] += cal[i] * trans[i]; }
            }
            _ => {}
        }
        let Some(sky) = &self.sky else {
            for c in 0..NCH { spec[c] += neb[c]; }
            return;
        };
        // 2. diffuse unresolved starlight + diffuse galactic light (behind the volume)
        if self.scene.diffuse {
            let mut dif = [0f32; NB];
            sky.diffuse_at(d, &mut dif);
            for i in 0..NB { spec[i] += dif[i] * trans[i]; }
        }
        // 3. resolved stars: Gaussian surface-brightness kernels (σ = 0.35 px on the sky)
        // Inside the deep layer's square: its catalogue. Outside: the all-sky
        // layer, plus the deep layer's *faint* sources tiled from the square
        // (a statistical stand-in, so demagnified images of distant directions
        // keep the faint-source density of the rest of the field).
        let deep_xy = sky.deep.as_ref().map(|df| match df.project(d) {
            Some((x, y)) => (df, x, y, d, false),
            None => { let (x, y, dq) = df.wrap(d); (df, x, y, dq, true) }
        });
        let shallow = deep_xy.map_or(true, |t| t.4);
        if let (true, Some((df, x, y, dq, wrapped))) = (self.scene.stars, deep_xy) {
            // deep field: tangent-plane grid
            let sig = self.star_sigma;
            let r_max = 4.0 * sig;
            let (r2_max, inv2s2) = (r_max * r_max, 1.0 / (2.0 * sig * sig));
            let knorm = inv2s2 / std::f64::consts::PI;
            let cw = 2.0 * df.half / df.n as f64;
            let (fx, fy) = ((x + df.half) / cw, (y + df.half) / cw);
            let k = (r_max / cw).ceil() as i64;
            for cy in (fy as i64 - k).max(0)..=(fy as i64 + k).min(df.n as i64 - 1) {
                for cx in (fx as i64 - k).max(0)..=(fx as i64 + k).min(df.n as i64 - 1) {
                    let c = cy as usize * df.n + cx as usize;
                    let (a, b) = (df.star_cells[c] as usize, df.star_cells[c + 1] as usize);
                    for (si, st_) in df.stars[a..b].iter().enumerate() {
                        if wrapped && (st_.v as f64) <= self.scene.star_mag_limit { continue; }
                        let r2 = star_chord2(st_.dir, dq);
                        if r2 > r2_max { continue; }
                        let kk = ((-r2 * inv2s2).exp() * knorm) as f32;
                        let sf = &self.deep_star_spec[a + si];
                        // stars in front of the nebula are not behind its dust
                        if (st_.d_kpc as f64) < self.vol_front_kpc { for i in 0..NB { spec[i] += sf[i] * kk; } } else { for i in 0..NB { spec[i] += sf[i] * kk * trans[i]; } }
                    }
                }
            }
        }
        if self.scene.stars && shallow {
            let sig = self.star_sigma;
            let r_max = 4.0 * sig;
            let (t1, t2) = basis(d);
            let mut cells = [usize::MAX; 4];
            let mut nc = 0;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let c = cell_index((d + t1 * (a * r_max) + t2 * (b * r_max)).norm(), INDEX_LEVEL);
                if !cells[..nc].contains(&c) { cells[nc] = c; nc += 1; }
            }
            let r2_max = r_max * r_max;
            let inv2s2 = 1.0 / (2.0 * sig * sig);
            let knorm = inv2s2 / std::f64::consts::PI;
            for &c in &cells[..nc] {
                let (a, b) = (sky.star_cells[c] as usize, sky.star_cells[c + 1] as usize);
                for (si, s) in sky.stars[a..b].iter().enumerate() {
                    let r2 = star_chord2(s.dir, d);
                    if r2 > r2_max { continue; }
                    let k = ((-r2 * inv2s2).exp() * knorm) as f32;
                    let sf = &self.star_spec[a + si];
                    if (s.d_kpc as f64) < self.vol_front_kpc { for i in 0..NB { spec[i] += sf[i] * k; } } else { for i in 0..NB { spec[i] += sf[i] * k * trans[i]; } }
                }
            }
        }
        // 4. galaxies: Sérsic n = 4 (ellipticals) or exponential discs
        if let (true, Some((df, x, y, dq, wrapped))) = (self.scene.galaxies, deep_xy) {
            if let Some(c) = df.cell_of(x, y) {
                for &gi in &df.gal_refs[df.gal_cells[c] as usize..df.gal_cells[c + 1] as usize] {
                    let gx = &df.galaxies[gi as usize];
                    if wrapped && (gx.mag as f64) <= self.scene.galaxy_mag_limit { continue; }
                    let p = self.galaxy_profile(gx, dq);
                    if p > 0.0 {
                        let sf = &self.deep_gal_spec[gi as usize];
                        for i in 0..NB { spec[i] += sf[i] * p * trans[i]; }
                    }
                }
            }
        }
        if self.scene.galaxies && shallow {
            let c = cell_index(d, GAL_LEVEL);
            let (t1, t2) = basis(d);
            let mut cells = [c, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
            let mut nc = 1;
            let r_q = 0.004; // neighbour search radius (rad); galaxies are < 1 arcmin here
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let cc = cell_index((d + t1 * (a * r_q) + t2 * (b * r_q)).norm(), GAL_LEVEL);
                if !cells[..nc].contains(&cc) { cells[nc] = cc; nc += 1; }
            }
            for &cc in &cells[..nc] {
                let base = sky.gal_cells[cc] as usize;
                for (gi, gx) in sky.galaxies[base..sky.gal_cells[cc + 1] as usize].iter().enumerate() {
                    let prof = self.galaxy_profile(gx, d);
                    if prof <= 0.0 { continue; }
                    let sf = &self.gal_spec[base + gi];
                    for i in 0..NB { spec[i] += sf[i] * prof * trans[i]; }
                }
            }
        }
        for c in 0..NCH { spec[c] += neb[c]; }
    }
}

impl Ctx {
    /// Unit-flux surface brightness (per sr) of a galaxy at direction d:
    /// de Vaucouleurs (n = 4) for ellipticals, exponential for discs, with the
    /// half-light radius floored at the star kernel width.
    fn galaxy_profile(&self, gx: &crate::sky::Galaxy, d: V3) -> f32 {
        let gd = V3::new(gx.dir[0] as f64, gx.dir[1] as f64, gx.dir[2] as f64).norm();
        let re = (gx.re as f64).max(self.star_sigma);
        let off = d - gd;
        if off.dot(off) > (8.0 * re).powi(2) { return 0.0; }
        let (a1, a2) = basis(gd);
        let (cp, sp) = ((gx.pa as f64).cos(), (gx.pa as f64).sin());
        let x = off.dot(a1) * cp + off.dot(a2) * sp;
        let y = -off.dot(a1) * sp + off.dot(a2) * cp;
        let q = gx.q as f64;
        let rr = (x * x + (y / q).powi(2)).sqrt() / re;
        (if gx.kind == 0 {
            (-7.669 * (rr.max(1e-3).sqrt().sqrt() - 1.0)).exp() / (22.67 * q * re * re)
        } else {
            let h = 1.0 / 1.678;
            (-rr / h).exp() / (2.0 * std::f64::consts::PI * h * h * q * re * re)
        }) as f32
    }

    /// Calibration skies (not physical): "grid" is a coordinate grid around the
    /// direction behind the hole, with quadrant tints so mirror images are
    /// recognisable and point markers at the intersections; "disc" is a uniform
    /// disc of radius `disc_radius_deg` at the source offset, its right half
    /// twice as bright (to show orientation and parity).
    fn calibration_sky(&self, d: V3, out: &mut [f32; NB]) {
        let s = &self.scene;
        let (ax, right, up) = self.grid_frame;
        let lon = d.dot(right).atan2(d.dot(ax));
        let lat = d.dot(up).clamp(-1.0, 1.0).asin();
        let r = s.grid_radiance as f32;
        if s.sky_mode == "disc" {
            let (ox, oy) = (s.volume_offset_deg.0.to_radians(), s.volume_offset_deg.1.to_radians());
            let c = (ax + right * ox.tan() + up * oy.tan()).norm();
            let ang = d.dot(c).clamp(-1.0, 1.0).acos();
            if ang <= s.disc_radius_deg.to_radians() {
                // the half on the observer's right (+x on the detector) is twice as bright
                let v = if d.dot(c.cross(up)) > 0.0 { 2.0 * r } else { r };
                for i in 0..NB { out[i] = v; }
            }
            return;
        }
        let sp = s.grid_spacing_deg.to_radians();
        let w = sp / 30.0;
        let near = |a: f64| { let k = (a / sp).round(); (a - k * sp).abs() };
        let (dl, db) = (near(lon) * lat.cos(), near(lat));
        let line = (-(dl * dl) / (2.0 * w * w)).exp().max((-(db * db) / (2.0 * w * w)).exp()) as f32;
        let mark = (-(dl * dl + db * db) / (2.0 * (2.5 * w).powi(2))).exp() as f32 * 3.0;
        // quadrant tints: (+,+) red, (−,+) blue, (−,−) green, (+,−) yellow
        let (lo, hi) = match (lon >= 0.0, lat >= 0.0) {
            (true, true) => (600.0, 960.0),
            (false, true) => (360.0, 500.0),
            (false, false) => (490.0, 570.0),
            (true, false) => (520.0, 650.0),
        };
        let back = if d.dot(ax) < 0.0 { 0.35 } else { 1.0 };
        for i in 0..NB {
            let l = bin_center(i);
            let tint = if l >= lo && l <= hi { 0.06 } else { 0.0 };
            out[i] = r * back * (line + mark + tint);
        }
    }
}

/// Debug landmarks: the `k` brightest H α voxels at least 3 pc apart.
pub fn find_landmarks(v: &Volume, k: usize) -> Vec<[f64; 3]> {
    let n = v.n;
    let mut f = [0f32; NF];
    let mut cand: Vec<(f32, [f64; 3])> = Vec::new();
    for z in 2..n - 2 { for y in 2..n - 2 { for x in 2..n - 2 {
        let p = [x as f64, y as f64, z as f64];
        v.sample(p, &mut f);
        cand.push((f[2], p));
    }}}
    cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let sep2 = (3.0 / v.dx).powi(2);
    let mut out: Vec<[f64; 3]> = Vec::new();
    for (_, p) in cand {
        if out.iter().all(|q| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2) > sep2) { out.push(p); }
        if out.len() == k { break; }
    }
    out
}

/// Squared chord between a stored (f32) star direction and d. Renormalising in
/// f64 first matters: 1 − cos from f32 components has ~6e-8 error, which at an
/// arcsecond kernel σ (1 − cos σ ~ 1e-10) turns exp(−…) into exp(+100).
fn star_chord2(dir: [f32; 3], d: V3) -> f64 {
    let off = V3::new(dir[0] as f64, dir[1] as f64, dir[2] as f64).norm() - d;
    off.dot(off)
}

/// Per-filter channel weights (e⁻ s⁻¹ per unit channel radiance per pixel).
pub fn make_filters(scene: &Scene, g_sky: f64) -> Vec<FilterInfo> {
    let area = scene.collecting_area();
    let px = scene.pixel_scale();
    let omega = px * px;
    scene
        .filters
        .iter()
        .filter_map(|n| spectrum::filter_by_name(n))
        .map(|f: &Filter| {
            let cw = spectrum::channel_weights(f, g_sky, true);
            let mut w = [0.0; NCH];
            // (neutral-density filters are applied per exposure in the readout)
            for c in 0..NCH { w[c] = cw[c] * area * omega; }
            FilterInfo { name: f.name.to_string(), w, lambda_eff: f.effective_wavelength() }
        })
        .collect()
}

/// e^−x for x ≥ 0 to ~2e-7 relative: 2^−x·log₂e split into 2ⁿ × a degree-6
/// polynomial on [−½, ½]. Uses only IEEE basic operations and floor, so native
/// and WebAssembly builds give bit-identical results (libm expf differs between
/// them and is several times slower in WebAssembly).
#[inline(always)]
pub fn exp_neg(x: f32) -> f32 {
    if !(x < 87.0) { return 0.0; }
    let y = -x * std::f32::consts::LOG2_E;
    let n = (y + 0.5).floor();
    let f = y - n;
    const C: [f32; 6] = [0.693_147_2, 0.240_226_5, 0.055_504_11, 0.009_618_129, 0.001_333_355_8, 0.000_154_035_3];
    let p = 1.0 + f * (C[0] + f * (C[1] + f * (C[2] + f * (C[3] + f * (C[4] + f * C[5])))));
    p * f32::from_bits(((n as i32 + 127) as u32) << 23)
}

/// Ray-march the volume along the ray from the origin in direction d
/// (world frame, parsecs). Front-to-back emission–absorption:
///   L += T · j · (1 − e^{−Δτ})/Δτ · Δs,   T ← T · e^{−Δτ}
/// per channel (exact for piecewise-constant j, α). `jitter` ∈ [0,1) offsets
/// the first sample (stochastic, converges over samples). Empty bricks are
/// skipped with a slab test to the brick boundary.
/// Returns (τ_V through the volume, steps, hit).
pub fn march(vp: &VolumePlacement, d: V3, jitter: f64, out: &mut [f32; NCH], trans: &mut [f32; NCH]) -> (f64, u32, bool) {
    let mut l = -1;
    march_landmarks(vp, d, jitter, out, trans, &mut l)
}

/// `march`, also reporting the first debug landmark (id) the ray passes within
/// 2 voxels of, or −1.
pub fn march_landmarks(vp: &VolumePlacement, d: V3, jitter: f64, out: &mut [f32; NCH], trans: &mut [f32; NCH], lmk: &mut i32) -> (f64, u32, bool) {
    let v = &vp.vol;
    let n = v.n as f64;
    // ray in voxel space: p(s) = o + s·dv, s in parsecs
    let rel = -vp.center_pc;
    let to_vox = |w: V3| V3::new(vp.rot[0].dot(w), vp.rot[1].dot(w), vp.rot[2].dot(w)) * (1.0 / v.dx);
    let half = 0.5 * (n - 1.0);
    let o = to_vox(rel) + V3::new(half, half, half);
    let dv = to_vox(d);
    // slab intersection with [−0.5, n − 0.5]³
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for (oo, dd) in [(o.x, dv.x), (o.y, dv.y), (o.z, dv.z)] {
        if dd.abs() < 1e-15 {
            if oo < -0.5 || oo > n - 0.5 { return (0.0, 0, false); }
            continue;
        }
        let (a, b) = ((-0.5 - oo) / dd, (n - 0.5 - oo) / dd);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    if t0 >= t1 { return (0.0, 0, false); }
    let step_vox = vp.step;
    let ds_pc = step_vox * v.dx; // dv has |dv| = 1/dx per pc
    let ds_cm = ds_pc * PC_CM;
    let mut s = t0 + jitter * ds_pc;
    let mut tau_v = 0.0;
    let mut steps = 0u32;
    let mut f = [0f32; NF];
    while s < t1 {
        let p = o + dv * s;
        let pa = [p.x, p.y, p.z];
        if v.brick_empty(pa) {
            // advance to the exit of this brick along the ray
            let mut exit = f64::INFINITY;
            for (k, dd) in [dv.x, dv.y, dv.z].iter().enumerate() {
                if dd.abs() < 1e-15 { continue; }
                let c = pa[k];
                let b0 = ((c.max(0.0) / BRICK as f64).floor()) * BRICK as f64;
                let edge = if *dd > 0.0 { b0 + BRICK as f64 } else { b0 - 1e-9 };
                exit = exit.min((edge - c) / dd);
            }
            let adv = exit.max(0.0) + 1e-6;
            // keep the jittered lattice so stepping stays unbiased
            s += (adv / ds_pc).ceil().max(1.0) * ds_pc;
            continue;
        }
        steps += 1;
        if *lmk < 0 && !vp.landmarks.is_empty() {
            for (k, q) in vp.landmarks.iter().enumerate() {
                let dd = (pa[0] - q[0]).powi(2) + (pa[1] - q[1]).powi(2) + (pa[2] - q[2]).powi(2);
                if dd < 4.0 { *lmk = k as i32; break; }
            }
        }
        v.sample(pa, &mut f);
        // fade every field to zero over the outer voxels, so the generator's
        // box (ambient gas up to its walls) never shows as an edge
        let edge = pa.iter().map(|&c| (c + 0.5).min(n - 0.5 - c)).fold(f64::INFINITY, f64::min);
        if edge < vp.edge_fade {
            let t = (edge / vp.edge_fade).clamp(0.0, 1.0) as f32;
            let wgt = t * t * (3.0 - 2.0 * t);
            for x in f.iter_mut() { *x *= wgt; }
        }
        let a_v = f[F_ALPHA] as f64;
        if a_v > 0.0 || f[..crate::spectrum::NL].iter().any(|&x| x > 0.0) || f[F_SCA] > 0.0 {
            let dtau_v = a_v * ds_cm;
            tau_v += dtau_v;
            for c in 0..NCH {
                let dtau = (dtau_v * vp.ext[c] as f64) as f32;
                let j = if c < NB { f[F_SCA] * vp.sca_shape[c] } else { f[c - NB] } as f64 * ds_cm * 1e4;
                let e = exp_neg(dtau);
                // (1 − e^−Δτ)/Δτ: series below 1e-2, where 1 − e loses f32 digits
                let w = if dtau > 1e-2 { (1.0 - e) / dtau } else { 1.0 - dtau * (0.5 - dtau * (1.0 / 6.0 - dtau / 24.0)) };
                out[c] += (trans[c] as f64 * j * w as f64) as f32;
                trans[c] *= e;
            }
        }
        s += ds_pc;
    }
    (tau_v, steps, true)
}

fn now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

/// Add `add` samples to every pixel in rows [y0, y1). `acc_rows` must be the
/// slice of the accumulators for exactly those rows.
pub fn render_rows(ctx: &Ctx, y0: usize, y1: usize, spp0: u32, add: u32, sum: &mut [f32], lsum: &mut [f64], lsq: &mut [f64], diag: &mut [f32]) -> PassStats {
    let w = ctx.scene.width;
    let mut st = PassStats::default();
    let mut spec = [0f32; NCH];
    let mut dg = [0f32; ND];
    for y in y0..y1 {
        for x in 0..w {
            let li = (y - y0) * w + x;
            let pix = (y * w + x) as u64;
            let acc = &mut sum[li * NCH..(li + 1) * NCH];
            let dacc = &mut diag[li * ND..(li + 1) * ND];
            for s in spp0..spp0 + add {
                let mut rng = Rng::keyed(&[ctx.scene.seed, pix, s as u64]);
                let (u, v) = (rng.f64(), rng.f64());
                dg.fill(0.0);
                ctx.sample(x, y, u, v, &mut rng, &mut spec, &mut dg, &mut st);
                for c in 0..NCH { acc[c] += spec[c]; }
                for k in 0..ND { dacc[k] += dg[k]; }
                let l = ctx.primary_rate(&spec);
                lsum[li] += l;
                lsq[li] += l * l;
            }
        }
    }
    st
}

/// Multi-threaded pass over the whole image (native).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_pass(ctx: &Ctx, acc: &mut Accum, add: u32, threads: usize) -> PassStats {
    let (w, h) = (acc.w, acc.h);
    let rows = 4usize;
    let chunks = h.div_ceil(rows);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let total = std::sync::Mutex::new(PassStats::default());
    let spp0 = acc.spp;
    // split buffers into row blocks; workers pull blocks from a shared counter
    let mut blocks: Vec<_> = acc
        .sum
        .chunks_mut(rows * w * NCH)
        .zip(acc.lsum.chunks_mut(rows * w))
        .zip(acc.lsq.chunks_mut(rows * w))
        .zip(acc.diag.chunks_mut(rows * w * ND))
        .map(|(((a, b), c), d)| std::sync::Mutex::new((a, b, c, d)))
        .collect();
    let blocks = &mut blocks;
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| {
                let mut local = PassStats::default();
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if k >= chunks { break; }
                    let mut g = blocks[k].lock().unwrap();
                    let (a, b, c, d) = &mut *g;
                    let y0 = k * rows;
                    let y1 = (y0 + rows).min(h);
                    let s = render_rows(ctx, y0, y1, spp0, add, a, b, c, d);
                    local.samples += s.samples; local.steps += s.steps; local.vol_steps += s.vol_steps; local.vol_rays += s.vol_rays;
                    local.failed += s.failed; local.escaped += s.escaped; local.captured += s.captured; local.disk += s.disk;
                }
                let mut t = total.lock().unwrap();
                t.samples += local.samples; t.steps += local.steps; t.vol_steps += local.vol_steps; t.vol_rays += local.vol_rays;
                t.failed += local.failed; t.escaped += local.escaped; t.captured += local.captured; t.disk += local.disk;
            });
        }
    });
    acc.spp += add;
    let t = *total.lock().unwrap();
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::Json;
    use crate::volume::{F_NH, F_XION};

    #[test]
    fn exp_neg_matches_libm() {
        let mut worst = 0f64;
        let mut x = 0f32;
        while x < 80.0 {
            let (a, b) = (exp_neg(x) as f64, (-(x as f64)).exp());
            worst = worst.max((a - b).abs() / b);
            x += 0.0137;
        }
        assert!(worst < 1e-5, "worst relative error {worst}");
        assert_eq!(exp_neg(0.0), 1.0);
        assert_eq!(exp_neg(200.0), 0.0);
    }

    #[test]
    fn star_kernel_normalised_at_arcsecond_sigma() {
        // a 1″ kernel, stored like the catalogue (f32 direction), must integrate
        // to 1 and never exceed its analytic peak
        let sig = 1f64.to_radians() / 3600.0;
        let c = V3::new(0.31, -0.62, 0.72).norm();
        let dir = [c.x as f32, c.y as f32, c.z as f32];
        let (t1, t2) = basis(c);
        let inv2s2 = 1.0 / (2.0 * sig * sig);
        let knorm = inv2s2 / std::f64::consts::PI;
        let (n, h) = (161, sig / 10.0);
        let (mut sum, mut peak) = (0.0, 0f64);
        for j in 0..n { for i in 0..n {
            let (x, y) = ((i as f64 - 80.0) * h, (j as f64 - 80.0) * h);
            let d = (c + t1 * x + t2 * y).norm();
            let k = (-star_chord2(dir, d) * inv2s2).exp() * knorm;
            sum += k * h * h;
            peak = peak.max(k);
        }}
        assert!((sum - 1.0).abs() < 0.01, "integral {sum}");
        assert!(peak <= knorm * 1.0001, "peak {peak} vs {knorm}");
    }

    fn uniform_volume(n: usize, alpha: f32, line: f32) -> VolumePlacement {
        let mut dense: [Vec<f32>; NF] = std::array::from_fn(|_| vec![0.0; n * n * n]);
        for i in 0..n * n * n {
            dense[F_ALPHA][i] = alpha;
            dense[2][i] = line; // H α
            dense[F_NH][i] = 1.0;
            dense[F_XION][i] = 1.0;
        }
        let vol = Volume::from_dense(n, 1.0, &dense, Json::obj(), 10000.0);
        let mut ext = [1f32; NCH];
        ext[0] = 1.5;
        VolumePlacement {
            vol,
            center_pc: V3::new(0.0, 0.0, 100.0),
            rot: [V3::new(1.0, 0.0, 0.0), V3::new(0.0, 1.0, 0.0), V3::new(0.0, 0.0, 1.0)],
            ext,
            sca_shape: [0.0; NB],
            fg: [1.0; NCH],
            brightness: 1.0,
            step: 0.5,
            edge_fade: 0.0,
            landmarks: Vec::new(),
        }
    }

    #[test]
    fn empty_volume_leaves_background_unchanged() {
        let vp = uniform_volume(16, 0.0, 0.0);
        let mut out = [0f32; NCH];
        let mut tr = [1f32; NCH];
        march(&vp, V3::new(0.0, 0.0, 1.0), 0.3, &mut out, &mut tr);
        assert!(out.iter().all(|&x| x == 0.0) && tr.iter().all(|&x| x == 1.0));
    }

    #[test]
    fn uniform_absorber_matches_beer_lambert() {
        // α = 1e-19 cm⁻¹ over 16 pc: τ_V = 1e-19 × 16 × 3.0857e18 = 4.937
        let vp = uniform_volume(16, 1.0e-19, 0.0);
        let mut out = [0f32; NCH];
        let mut tr = [1f32; NCH];
        let (tau, _, hit) = march(&vp, V3::new(0.0, 0.0, 1.0), 0.0, &mut out, &mut tr);
        assert!(hit);
        let expect = 1.0e-19 * 16.0 * PC_CM;
        assert!((tau / expect - 1.0).abs() < 0.04, "tau {tau} vs {expect}");
        assert!(((tr[3] as f64).ln() + tau).abs() < 1e-3);
        assert!(((tr[0] as f64).ln() + 1.5 * tau).abs() < 1e-3); // wavelength-dependent extinction
    }

    #[test]
    fn uniform_emitter_matches_analytic_solution() {
        // emission + absorption slab: L = (j/α)(1 − e^{−τ}) with τ = α L
        let (alpha, j) = (2.0e-20f32, 5.0e-12f32);
        let vp = uniform_volume(16, alpha, j);
        let mut out = [0f32; NCH];
        let mut tr = [1f32; NCH];
        let (tau, _, _) = march(&vp, V3::new(0.0, 0.0, 1.0), 0.5, &mut out, &mut tr);
        let expect = (j as f64 / alpha as f64) * (1.0 - (-tau).exp()) * 1e4;
        assert!((out[NB + 2] as f64 / expect - 1.0).abs() < 1e-3, "{} vs {}", out[NB + 2], expect);
        assert!(out.iter().all(|x| x.is_finite()));
    }

    fn smooth_volume(n: usize) -> VolumePlacement {
        // smoothly varying absorber and emitter (no brick is empty)
        let mut dense: [Vec<f32>; NF] = std::array::from_fn(|_| vec![0.0; n * n * n]);
        for z in 0..n { for y in 0..n { for x in 0..n {
            let i = (z * n + y) * n + x;
            let w = (2.0 * std::f64::consts::PI * z as f64 / n as f64).sin() * (2.0 * std::f64::consts::PI * (x + y) as f64 / n as f64).cos();
            dense[F_ALPHA][i] = (6.0e-20 * (1.0 + 0.8 * w)) as f32;
            dense[2][i] = (4.0e-12 * (1.0 - 0.7 * w)) as f32;
            dense[F_NH][i] = 1.0;
            dense[F_XION][i] = 1.0;
        }}}
        let mut vp = uniform_volume(n, 0.0, 0.0);
        vp.vol = Volume::from_dense(n, 1.0, &dense, Json::obj(), 10000.0);
        vp
    }

    #[test]
    fn volume_march_converges_with_step_size() {
        // the jittered march is an unbiased estimate of the emission integral;
        // its mean over jitters converges as the step shrinks
        let mut vp = smooth_volume(32);
        let d = V3::new(0.13, -0.07, 1.0).norm();
        let mut mean_at = |step: f64| {
            vp.step = step;
            let mut acc = 0.0;
            let k = 256;
            for i in 0..k {
                let (mut out, mut tr) = ([0f32; NCH], [1f32; NCH]);
                march(&vp, d, (i as f64 + 0.5) / k as f64, &mut out, &mut tr);
                acc += out[NB + 2] as f64;
            }
            acc / k as f64
        };
        let (l1, l2, l4, l8) = (mean_at(1.0), mean_at(0.5), mean_at(0.25), mean_at(0.125));
        let err = |l: f64| (l / l8 - 1.0).abs();
        assert!(err(l2) < 0.01 && err(l4) < 0.003, "{l1} {l2} {l4} {l8}");
        assert!(err(l4) <= err(l1) + 1e-6, "not converging: {l1} {l2} {l4} {l8}");
    }

    fn lens_scene(beta_frac: f64, fov_frac: f64) -> Scene {
        // telescope observer at r0 = 1e4 M: θ_E = 2/√r0 = 0.02 rad for a source
        // at infinity; a uniform test disc of radius 0.1 θ_E at β = beta_frac θ_E
        let te = 0.02f64.to_degrees();
        let mut sc = Scene {
            mode: "telescope".into(), bh_mass_msun: 4.3e6, width: 160, height: 160, fov_deg: fov_frac * te,
            disk: false, sky_mode: "disc".into(), disc_radius_deg: 0.1 * te, grid_radiance: 1e10,
            volume_offset_deg: (beta_frac * te, 0.0), filters: vec!["L".into()], sensor_noise: false, psf: false,
            rtol: 1e-9, ..Default::default()
        };
        sc.bh_distance_pc = 1e4 * sc.r_g_m() / crate::scene::PC_M;
        sc.finalize();
        sc
    }

    fn render_flux(sc: Scene, spp: u32) -> (usize, usize, Vec<f64>) {
        let ctx = Ctx::new(sc, None, &mut |_| {});
        let (w, h) = (ctx.scene.width, ctx.scene.height);
        let mut acc = Accum::new(w, h);
        render_rows(&ctx, 0, h, 0, spp, &mut acc.sum, &mut acc.lsum, &mut acc.lsq, &mut acc.diag);
        let lum = (0..w * h).map(|p| (0..NB).map(|c| acc.sum[p * NCH + c] as f64).sum::<f64>() / spp as f64).collect();
        (w, h, lum)
    }

    #[test]
    fn aligned_source_makes_an_einstein_ring() {
        let (w, h, lum) = render_flux(lens_scene(0.0, 4.0), 8);
        // flux-weighted mean radius ≈ θ_E (√(θ_E² + ρ²/2)-ish for a finite disc)
        let px = 4.0 / w as f64; // in units of θ_E
        let (mut f, mut fr, mut left, mut right) = (0.0, 0.0, 0.0, 0.0);
        for y in 0..h { for x in 0..w {
            let (dx, dy) = ((x as f64 + 0.5 - w as f64 / 2.0) * px, (y as f64 + 0.5 - h as f64 / 2.0) * px);
            let v = lum[y * w + x];
            f += v; fr += v * (dx * dx + dy * dy).sqrt();
            if dx < 0.0 { left += v } else { right += v }
        }}
        let r = fr / f;
        assert!((r - 1.0).abs() < 0.05, "ring radius {r} θ_E");
        assert!(left > 0.3 * f && right > 0.3 * f, "a ring, not an arc");
    }

    #[test]
    fn offset_source_makes_two_images_with_point_lens_flux_ratio() {
        // β = 0.5 θ_E: images at θ± = (β ± √(β² + 4))/2 = +1.28, −0.78 θ_E with
        // magnifications μ± = (u² + 2)/(2u√(u² + 4)) ± ½ → ratio 2.69
        let (w, h, lum) = render_flux(lens_scene(0.5, 4.0), 8);
        let px = 4.0 / w as f64;
        let (mut fp, mut fm, mut xp, mut xm) = (0.0, 0.0, 0.0, 0.0);
        for y in 0..h { for x in 0..w {
            let dx = (x as f64 + 0.5 - w as f64 / 2.0) * px;
            let v = lum[y * w + x];
            if dx > 0.0 { fp += v; xp += v * dx } else { fm += v; xm += v * dx }
        }}
        let (u, rt) = (0.5f64, (0.25f64 + 4.0).sqrt());
        let ratio = ((u * u + 2.0) / (2.0 * u * rt) + 0.5) / ((u * u + 2.0) / (2.0 * u * rt) - 0.5);
        assert!((fp / fm / ratio - 1.0).abs() < 0.1, "flux ratio {} vs {ratio}", fp / fm);
        assert!(((xp / fp) / ((u + rt) / 2.0) - 1.0).abs() < 0.05, "primary at {}", xp / fp);
        assert!(((xm / fm) / ((u - rt) / 2.0) - 1.0).abs() < 0.05, "secondary at {}", xm / fm);
    }

    #[test]
    fn unlensed_comparison_is_the_undeflected_source() {
        let mut sc = lens_scene(0.5, 4.0);
        sc.lensing = false;
        let (w, h, lum) = render_flux(sc, 4);
        let px = 4.0 / w as f64;
        let (mut f, mut fx) = (0.0, 0.0);
        for y in 0..h { for x in 0..w { let v = lum[y * w + x]; f += v; fx += v * (x as f64 + 0.5 - w as f64 / 2.0) * px; } }
        assert!((fx / f - 0.5).abs() < 0.02, "centroid {}", fx / f);
    }

    #[test]
    fn extended_source_surface_brightness_is_independent_of_distance() {
        // a uniform emitting volume, no dust: the radiance through its centre is
        // the same at 1 and 4 kpc; its angular size scales as 1/D
        let mk = |d_pc: f64| {
            let mut vp = uniform_volume(16, 0.0, 3.0e-12);
            vp.center_pc = V3::new(0.0, 0.0, d_pc);
            let mut out = [0f32; NCH];
            let mut tr = [1f32; NCH];
            march(&vp, V3::new(0.0, 0.0, 1.0), 0.5, &mut out, &mut tr);
            // angular half-size: the edge ray at 8 pc off-axis just grazes
            let edge = |a: f64| { let (mut o, mut t) = ([0f32; NCH], [1f32; NCH]); march(&vp, V3::new(a.sin(), 0.0, a.cos()), 0.5, &mut o, &mut t).2 };
            let (mut lo, mut hi) = (0.0, 0.1);
            for _ in 0..50 { let m = 0.5 * (lo + hi); if edge(m) { lo = m } else { hi = m } }
            (out[NB + 2] as f64, lo)
        };
        let ((l1, a1), (l4, a4)) = (mk(1000.0), mk(4000.0));
        assert!((l1 / l4 - 1.0).abs() < 1e-3, "{l1} vs {l4}");
        assert!((a1 / a4 / 4.0 - 1.0).abs() < 0.02, "sizes {a1} {a4}");
    }

    #[test]
    fn detector_rate_scales_with_aperture_area() {
        let base = Scene { filters: vec!["V".into()], aperture_m: 0.1, ..Default::default() };
        let f1 = make_filters(&base, 1.0);
        let f2 = make_filters(&Scene { aperture_m: 0.2, ..base.clone() }, 1.0);
        assert!((f2[0].w[7] / f1[0].w[7] - 4.0).abs() < 1e-9);
    }
}
