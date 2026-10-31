//! A survey preset: one generated scene, one field of view, several
//! instruments, and the composites built from them. Shared by the CLI and the
//! browser build so both produce the same images.

use super::bands::{instrument, Instrument, Kind};
use super::obs::{observe, ObsOpts, Observation, View};
use super::show::{band_image, composite, instrument_composite, montage, shrink, Layer, Stretch};
use super::*;
use crate::json::Json;

pub struct Preset {
    pub json: Json,
    pub title: String,
    pub w: usize,
    pub h: usize,
    pub spp: u32,
    pub instruments: Vec<String>,
}

impl Preset {
    pub fn from_json(j: &Json) -> Preset {
        let v = j.get("view").cloned().unwrap_or(Json::obj());
        Preset {
            json: j.clone(),
            title: j.str("title").unwrap_or("Survey").to_string(),
            w: v.num("width").unwrap_or(1024.0) as usize,
            h: v.num("height").unwrap_or(640.0) as usize,
            spp: j.num("spp").unwrap_or(4.0) as u32,
            instruments: j.arr("instruments").map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        }
    }
    pub fn generator(&self) -> &str { self.json.get("scene").and_then(|s| s.str("generator")).unwrap_or("pillars") }
    pub fn stretch(&self, id: &str) -> Stretch {
        let mut st = Stretch::default();
        for key in ["stretch".to_string(), format!("stretch_{id}")] {
            if let Some(s) = self.json.get(&key) {
                st.soft = s.num("soft").unwrap_or(st.soft);
                st.lo_pct = s.num("lo").unwrap_or(st.lo_pct);
                st.hi_pct = s.num("hi").unwrap_or(st.hi_pct);
                st.smooth = s.num("smooth").unwrap_or(st.smooth);
                st.shared = s.bool("shared").unwrap_or(st.shared);
            }
        }
        st
    }
}

pub fn build_scene(p: &Preset, grid: Option<usize>, log: &mut dyn FnMut(&str)) -> MwScene {
    let mut sj = p.json.get("scene").cloned().unwrap_or(Json::obj());
    if let Some(n) = grid { sj.set("grid", n); }
    match p.generator() {
        "supercluster" => cosmic::generate(&cosmic::CosmicParams::from_json(&sj), log).0,
        _ => pillars::generate(&pillars::PillarParams::from_json(&sj), log).0,
    }
}

/// The preset's view: `fov_arcmin` or `fov_deg` wide, aimed at the volume
/// centre offset by `look_pc` (pillars) or at a halo (`target: "halo0"`).
pub fn view(p: &Preset, sc: &MwScene) -> View {
    let v = p.json.get("view").cloned().unwrap_or(Json::obj());
    let fov = v.num("fov_arcmin").map(|a| a / 60.0).or(v.num("fov_deg")).unwrap_or(10.0).to_radians();
    let mut look = sc.vol.center;
    if let Some(off) = v.arr("look_pc") {
        for k in 0..2 { look[k] += off.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) * phys::PC; }
    }
    if let Some(t) = v.str("target") {
        if let Some(k) = t.strip_prefix("halo").and_then(|s| s.parse::<usize>().ok()) {
            if let Some(h) = sc.halos.get(k) { look = h.pos; }
        }
    }
    View::new(p.w, p.h, fov, look)
}

/// Add foreground stars inside a view when the preset asks for them
/// (`foreground_stars: {gal_l_deg, gal_b_deg, v_limit}`); Galactic scenes
/// already carry their own.
pub fn add_foreground_stars(p: &Preset, sc: &mut MwScene, v: &View) {
    let Some(fs) = p.json.get("foreground_stars") else { return };
    let w = v.pix * v.w as f64 * 1.02;
    let h = v.pix * v.h as f64 * 1.02;
    let look = [v.fwd.x, v.fwd.y, v.fwd.z];
    let stars = pillars::field_stars(sc.seed ^ 0x57A2, fs.num("gal_l_deg").unwrap_or(310.0), fs.num("gal_b_deg").unwrap_or(30.0), look, w, h, fs.num("v_limit").unwrap_or(24.0), 200_000);
    sc.sources.extend(stars);
}

pub fn instrument_for(p: &Preset, id: &str) -> Option<Instrument> {
    let mut ins = instrument(id)?;
    if let Some(o) = p.json.get("overrides").and_then(|o| o.get(id)) {
        ins.exposure_s = o.num("exposure_s").unwrap_or(ins.exposure_s);
        ins.smooth_arcmin = o.num("smooth_arcmin").unwrap_or(ins.smooth_arcmin);
        ins.n_gal = o.num("n_gal").unwrap_or(ins.n_gal);
        if let Some(s) = o.num("sigma") { for b in ins.bands.iter_mut() { b.sigma = s; } }
        if let Some(f) = o.num("fwhm_arcsec") { for b in ins.bands.iter_mut() { b.fwhm = f; } }
    }
    Some(ins)
}

pub fn observe_one(p: &Preset, sc: &MwScene, v: &View, id: &str, spp: u32, noise: bool, log: &mut dyn FnMut(&str)) -> Option<Observation> {
    let ins = instrument_for(p, id)?;
    Some(observe(sc, &ins, v, &ObsOpts { spp, seed: sc.seed ^ 0x5u64, noise }, log))
}

/// An image the survey produces: (file stem, rgba, w, h, class, presentation mode).
pub struct Image {
    pub stem: String,
    pub rgba: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub class: &'static str,
    pub mode: String,
}

/// Single-band images and the instrument composite for one observation.
pub fn instrument_images(p: &Preset, obs: &Observation) -> Vec<Image> {
    let st = p.stretch(&obs.ins.id);
    let class = if obs.w >= 1024 && obs.h >= 640 { "presentation" } else if obs.w >= 512 { "validation" } else { "fixture" };
    let mut out = Vec::new();
    let signed = matches!(obs.ins.kind, Kind::Cmb | Kind::Lensing);
    if obs.maps.len() > 1 && !signed {
        let (rgba, w, h) = instrument_composite(obs, st);
        out.push(Image { stem: format!("{}-composite", obs.ins.id), rgba, w, h, class, mode: "false_colour_composite".into() });
    }
    for b in 0..obs.maps.len() {
        let (rgba, w, h) = band_image(obs, b, st);
        let slug: String = obs.maps[b].name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        let mode = match obs.ins.kind { Kind::Lensing => "reconstructed_map", Kind::Cmb => "calibrated_map", _ => "single_band" };
        out.push(Image { stem: format!("{}-{}", obs.ins.id, slug.trim_matches('-')), rgba, w, h, class, mode: mode.into() });
    }
    out
}

/// Cross-instrument composites declared in the preset (`cross`), each layer
/// naming an instrument, a band index, a colour and optionally log scaling.
pub fn cross_images(p: &Preset, obs: &[Observation]) -> Vec<Image> {
    let mut out = Vec::new();
    let Some(list) = p.json.arr("cross") else { return out };
    for c in list {
        let mut layers = Vec::new();
        let mut w = 0;
        let mut h = 0;
        for l in c.arr("layers").unwrap_or(&[]) {
            let id = l.str("instrument").unwrap_or("");
            let Some(o) = obs.iter().find(|o| o.ins.id == id) else { continue };
            let b = l.num("band").unwrap_or(0.0) as usize;
            let Some(m) = o.maps.get(b) else { continue };
            let col = l.arr("colour").map(|a| [0, 1, 2].map(|k| a.get(k).and_then(|v| v.as_f64()).unwrap_or(1.0))).unwrap_or([1.0; 3]);
            let mut st = p.stretch(id);
            st.soft = l.num("soft").unwrap_or(st.soft);
            st.lo_pct = l.num("lo").unwrap_or(st.lo_pct);
            st.hi_pct = l.num("hi").unwrap_or(st.hi_pct);
            st.smooth = l.num("smooth").unwrap_or(st.smooth);
            st.shared = false;
            layers.push(Layer { data: &m.data, colour: col, label: format!("{} {}", o.ins.name, m.name), stretch: st, log: l.bool("log").unwrap_or(false),
                negate: l.bool("negate").unwrap_or(false), highpass: l.num("highpass_px").unwrap_or(0.0) });
            w = o.w;
            h = o.h;
        }
        if layers.is_empty() { continue; }
        let title = format!("FALSE COLOUR: {}", c.str("title").unwrap_or("multiwavelength composite").to_uppercase());
        let note = c.str("note").unwrap_or("Each layer stretched on its own and added in the colour shown.").to_string();
        let (rgba, iw, ih) = composite(&layers, w, h, &title, &note, false);
        out.push(Image { stem: c.str("name").unwrap_or("cross").to_string(), rgba, w: iw, h: ih, class: "presentation", mode: "false_colour_composite".into() });
    }
    out
}

/// A montage of every instrument's composite (or its single band), at half size.
pub fn montage_image(p: &Preset, obs: &[Observation]) -> Option<Image> {
    if obs.is_empty() { return None; }
    let mut tiles = Vec::new();
    for o in obs {
        let st = p.stretch(&o.ins.id);
        let signed = matches!(o.ins.kind, Kind::Cmb | Kind::Lensing);
        let pick = if o.ins.kind == Kind::Cmb { o.maps.len().min(2) - 1 } else { 0 };
        let (rgba, w, h) = if o.maps.len() > 1 && !signed {
            let layers: Vec<Layer> = {
                let n = o.maps.len();
                let mut idx: Vec<usize> = (0..n).collect();
                idx.sort_by(|&a, &b| o.maps[a].lambda_um.partial_cmp(&o.maps[b].lambda_um).unwrap());
                idx.iter().enumerate().map(|(k, &b)| Layer { data: &o.maps[b].data, colour: super::show::chromatic(k, n), label: String::new(), stretch: st, log: o.ins.kind == Kind::Xray, negate: false, highpass: 0.0 }).collect()
            };
            composite_raw(&layers, o.w, o.h)
        } else {
            let (img, w, h) = band_image(o, pick, st);
            (img[..w * o.h * 4].to_vec(), w, o.h.min(h))
        };
        let f = if w >= 1024 { 2 } else { 1 };
        let (s, sw, sh) = shrink(&rgba, w, h, f);
        let label = if o.maps.len() > 1 && !signed { format!("{} ({} bands)", o.ins.name, o.maps.len()) } else { format!("{} {}", o.ins.name, o.maps[pick].name) };
        tiles.push((s, sw, sh, label));
    }
    let cols = if tiles.len() > 4 { 3 } else { 2 };
    let (rgba, w, h) = montage(&tiles, cols, &format!("{}: THE SAME FIELD, EVERY INSTRUMENT", p.title.to_uppercase()),
        "Tiles are the presentation renders shrunk 2x for comparison. Multi-band tiles are false-colour composites in chromatic order; single bands are greyscale (X-ray log viridis, CMB and lensing diverging).");
    Some(Image { stem: "montage".into(), rgba, w, h, class: "presentation", mode: "comparison_montage".into() })
}

/// A composite without the legend band (for montage tiles).
fn composite_raw(layers: &[Layer], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (img, iw, _) = composite(layers, w, h, "", "", true);
    (img[..iw * h * 4].to_vec(), iw, h)
}

pub fn metadata(p: &Preset, sc: &MwScene, v: &View, obs: &Observation, secs: f64) -> Json {
    let mut j = Json::obj();
    j.set("preset", p.title.as_str()).set("generator", sc.meta.clone()).set("instrument", obs.ins.id.as_str())
        .set("instrument_name", obs.ins.name.as_str()).set("modelled_on", obs.ins.analog.as_str()).set("unit", obs.ins.unit())
        .set("exposure_s", obs.ins.exposure_s).set("width", v.w).set("height", v.h).set("pixel_arcsec", obs.pix_arcsec)
        .set("spp", p.spp).set("seconds", secs);
    let bands: Vec<Json> = obs.maps.iter().zip(&obs.ins.bands).map(|(m, b)| {
        let mut bj = Json::obj();
        bj.set("name", m.name.as_str()).set("traces", m.traces.as_str()).set("lambda_um", m.lambda_um).set("fwhm_arcsec", b.fwhm)
            .set("noise_per_pixel", m.sigma).set("max", m.ideal.iter().fold(f32::MIN, |a, &x| a.max(x)) as f64)
            .set("median", super::show::percentile(&m.ideal, 50.0));
        bj
    }).collect();
    j.set("bands", bands);
    j
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mw::obs::{band_grids, sky_maps, View};
    use crate::mw::phys::{ext_ratio, PC};

    fn small_pillars(n: usize, extra: &[(&str, f64)]) -> (pillars::PillarParams, MwScene) {
        let mut j = Json::obj();
        j.set("grid", n).set("seed", 3u64).set("ysos", 6u64).set("star_limit_v", 18.0);
        for (k, v) in extra { j.set(k, *v); }
        let p = pillars::PillarParams::from_json(&j);
        let (sc, _) = pillars::generate(&p, &mut |_| {});
        (p, sc)
    }

    #[test]
    fn front_depth_in_a_uniform_cloud_matches_the_erosion_column() {
        // no turbulence, no cores, no diffuse erosion: under the cluster the
        // front sits where n · depth = n_IF c_s t at that distance
        let (p, sc) = small_pillars(64, &[("turbulence", 0.0), ("cores", 0.0), ("diffuse_fraction", 0.0), ("cloud_top", 0.6), ("age_myr", 0.6), ("surface_roughness", 0.0)]);
        let n = p.n;
        let dx = p.box_pc / n as f64;
        let h = 0.5 * (n as f64 - 1.0);
        let ix = ((p.cluster_pc[0] / dx + h).round() as usize).min(n - 1);
        let iz = ((p.cluster_pc[2] / dx + h).round() as usize).min(n - 1);
        // walk down from the top: first neutral cell
        let mut y_front = None;
        for y in (0..n).rev() {
            let i = (iz * n + y) * n + ix;
            if sc.vol.f[F_XION][i] < 0.5 { y_front = Some((y as f64 - h) * dx); break; }
        }
        let y_front = y_front.expect("a neutral cloud");
        // predicted: cloud surface at y_top (env 1/(1+e^((y−top)/0.12))), then depth N_erode/n
        let top = p.top * p.box_pc * 0.5;
        let flow = p.flow_pc * PC;
        let r = (p.cluster_pc[1] - y_front).abs();
        let f = p.q_ion / (4.0 * std::f64::consts::PI * (r * PC).powi(2));
        let n_erode = (f / (super::phys::ALPHA_B * flow)).sqrt() * p.cs_kms * 1e5 * p.age_myr * 3.156e13;
        let depth = n_erode / p.n_mean / PC;
        let predicted = top - depth;
        assert!((y_front - predicted).abs() < 2.5 * dx, "front at {y_front:.2} pc, predicted {predicted:.2} pc (cell {dx:.2})");
    }

    #[test]
    fn every_band_sees_the_same_dust_column() {
        // a young star's optical depth in each band scales with the dust law
        let (_, sc) = small_pillars(32, &[]);
        let ins_o = bands::instrument("optical_narrow").unwrap();
        let ins_n = bands::instrument("nircam").unwrap();
        let (go, gn) = (band_grids(&sc, &ins_o), band_grids(&sc, &ins_n));
        let yso = sc.sources.iter().find(|s| s.flags & 1 != 0).unwrap();
        let tau = |g: &obs::Grids, b: usize| {
            let p = crate::math::V3::new(yso.pos[0], yso.pos[1], yso.pos[2]);
            let d = p.len();
            let mut i = vec![0.0; g.j.len()];
            let mut t = vec![0.0; g.j.len()];
            obs::march_public(&g.geom, g, d, p * (1.0 / d), &mut i, &mut t);
            t[b]
        };
        let (t_ha, t_f200) = (tau(&go, 1), tau(&gn, 2));
        assert!(t_ha > 1.0, "the star should be embedded, τ(Hα) = {t_ha}");
        let expect = ext_ratio(2.0) / ext_ratio(0.6566);
        assert!((t_f200 / t_ha / expect - 1.0).abs() < 0.15, "τ ratio {} vs dust law {expect}", t_f200 / t_ha);
    }

    #[test]
    fn radio_flux_does_not_depend_on_resolution() {
        let (_, sc) = small_pillars(32, &[]);
        let ins = bands::instrument("radio_cm").unwrap();
        let fov = 0.9 * 6.4 / 1740.0;
        let tot = |w: usize, h: usize| {
            let v = View::new(w, h, fov, sc.vol.center);
            let m = sky_maps(&sc, &ins, &v, &obs::ObsOpts { spp: 2, seed: 1, noise: false }, &mut |_| {});
            m[0].iter().sum::<f64>() * v.omega_pix()
        };
        let (a, b) = (tot(64, 40), tot(160, 100));
        assert!((a / b - 1.0).abs() < 0.02, "{a:e} vs {b:e}");
    }

    #[test]
    fn a_small_supercluster_has_haloes_galaxies_and_mass() {
        let mut j = Json::obj();
        j.set("grid", 32u64).set("box_mpc", 64.0).set("distance_mpc", 120.0).set("seed", 5u64).set("peak_r_mpc", 6.0);
        let p = cosmic::CosmicParams::from_json(&j);
        let (sc, st) = cosmic::generate(&p, &mut |_| {});
        assert!(st.haloes >= 1 && st.m_max > 1e14, "{} haloes, max {:e}", st.haloes, st.m_max);
        assert!(st.delta_peak > 0.5, "constrained peak δ = {}", st.delta_peak);
        let target = p.gal_density * 64f64.powi(3);
        assert!((st.galaxies as f64 / target - 1.0).abs() < 0.15, "{} galaxies vs {target}", st.galaxies);
        let v = View::new(48, 30, 30f64.to_radians(), sc.halos[0].pos);
        let k = obs::kappa_map(&sc, &v, true);
        let c = k[15 * 48 + 24];
        let mean = k.iter().sum::<f64>() / k.len() as f64;
        assert!(c > 0.0 && c > mean + 0.01, "κ at the largest halo {c} vs mean {mean}");
    }
}
