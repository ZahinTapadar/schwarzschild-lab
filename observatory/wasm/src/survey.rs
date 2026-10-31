//! Multiwavelength Survey in the browser.
//!
//! * **generator / compositor** (`s_*`, one worker): builds the scene, the
//!   per-instrument band grids and the non-volume maps, then applies the
//!   instrument and renders the images (PNG bytes) once the rows are back;
//! * **march workers** (`m_*`): hold one instrument's band grids and ray-march
//!   every `nw`-th image row.
//!
//! Same seeds and the same code as `observatory survey`, so a browser render
//! matches the CLI render of the same preset, grid, size and samples.

use crate::{bytes, set_out, string};
use observatory_core::export;
use observatory_core::json::Json;
use observatory_core::math::V3;
use observatory_core::mw::bands::Kind;
use observatory_core::mw::obs::{apply_instrument, band_grids, extra_maps, march_row, Grids, ObsOpts, Observation, View};
use observatory_core::mw::show::{redshift_wedge, rgba_to_rgb, state_slices};
use observatory_core::mw::survey::{self, Image, Preset};
use observatory_core::mw::MwScene;

struct Gen {
    preset: Preset,
    scene: MwScene,
    view: View,
    current: Option<(String, Vec<Vec<f64>>)>,
    done: Vec<Observation>,
}
static mut GEN: Option<Gen> = None;

struct March {
    grids: Grids,
    view: View,
}
static mut MARCH: Option<March> = None;

fn view_json(v: &View) -> Json {
    let mut j = Json::obj();
    j.set("w", v.w).set("h", v.h).set("pix", v.pix)
        .set("fwd", vec![v.fwd.x.into(), v.fwd.y.into(), v.fwd.z.into()])
        .set("right", vec![v.right.x.into(), v.right.y.into(), v.right.z.into()])
        .set("up", vec![v.up.x.into(), v.up.y.into(), v.up.z.into()]);
    j
}

fn view_from(j: &Json) -> View {
    let v3 = |k: &str| { let a = j.arr(k).unwrap(); V3::new(a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()) };
    View { w: j.num("w").unwrap() as usize, h: j.num("h").unwrap() as usize, pix: j.num("pix").unwrap(), fwd: v3("fwd"), right: v3("right"), up: v3("up") }
}

/// Pack images as: u32 count, then per image u32 json length, json, u32 png length, png.
fn pack(images: &[Image]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&(images.len() as u32).to_le_bytes());
    for im in images {
        let mut j = Json::obj();
        j.set("stem", im.stem.as_str()).set("class", im.class).set("mode", im.mode.as_str()).set("w", im.w).set("h", im.h);
        let js = j.dump();
        let png = export::png(im.w, im.h, &rgba_to_rgb(&im.rgba), &[("Class", im.class.to_string()), ("PresentationMode", im.mode.clone()), ("Software", "Schwarzschild Observatory: Multiwavelength Survey (browser)".into())]);
        b.extend_from_slice(&(js.len() as u32).to_le_bytes());
        b.extend_from_slice(js.as_bytes());
        b.extend_from_slice(&(png.len() as u32).to_le_bytes());
        b.extend_from_slice(&png);
    }
    b
}

/// Generate the scene of a preset at `grid`³ and `w` × `h`. Leaves an info JSON
/// (view, generator summary, instruments) in the output buffer.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_init(p: *const u8, n: usize, grid: usize, w: usize, h: usize) -> i32 {
    let Ok(mut j) = Json::parse(&string(p, n)) else { return -1 };
    let mut v = j.get("view").cloned().unwrap_or(Json::obj());
    v.set("width", w).set("height", h);
    j.set("view", v);
    let preset = Preset::from_json(&j);
    let mut sc = survey::build_scene(&preset, Some(grid), &mut |_| {});
    let view = survey::view(&preset, &sc);
    survey::add_foreground_stars(&preset, &mut sc, &view);
    let mut info = Json::obj();
    let ins: Vec<Json> = preset.instruments.iter().filter_map(|id| survey::instrument_for(&preset, id)).map(|i| {
        let mut o = Json::obj();
        o.set("id", i.id.as_str()).set("name", i.name.as_str()).set("analog", i.analog.as_str()).set("unit", i.unit())
            .set("bands", i.bands.iter().map(|b| { let mut bj = Json::obj(); bj.set("name", b.name.as_str()).set("traces", b.traces.as_str()); bj }).collect::<Vec<Json>>());
        o
    }).collect();
    info.set("view", view_json(&view)).set("generator", sc.meta.clone()).set("instruments", ins).set("sources", sc.sources.len()).set("pix_arcsec", view.pix_arcsec());
    set_out(info.dump().into_bytes());
    GEN = Some(Gen { preset, scene: sc, view, current: None, done: vec![] });
    0
}

/// Diagnostics of the generated scene (state slices, and the redshift wedge
/// for cosmic scenes), as packed images.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_diagnostics() {
    let Some(g) = GEN.as_ref() else { set_out(vec![]); return };
    let (rgba, w, h) = state_slices(&g.scene);
    let mut imgs = vec![Image { stem: "state".into(), rgba, w, h, class: "diagnostic", mode: "diagnostic".into() }];
    if let Some(wd) = g.preset.json.get("wedge") {
        let fov = g.preset.json.get("view").and_then(|v| v.num("fov_deg")).unwrap_or(40.0).to_radians();
        let (rgba, w, h) = redshift_wedge(&g.scene, wd.num("half_dec_deg").unwrap_or(3.0).to_radians(), fov, wd.num("mstar_min").unwrap_or(1e10));
        imgs.push(Image { stem: "wedge".into(), rgba, w, h, class: "diagnostic", mode: "catalogue_plot".into() });
    }
    set_out(pack(&imgs));
}

/// Prepare one instrument: computes the non-volume maps and leaves the band
/// grids (for the march workers) in the output buffer; empty if there is no
/// volume to march (lensing). Returns the number of bands, or −1.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_prepare(p: *const u8, n: usize) -> i32 {
    let Some(g) = GEN.as_mut() else { return -1 };
    let id = string(p, n);
    let Some(ins) = survey::instrument_for(&g.preset, &id) else { return -1 };
    if ins.kind == Kind::Lensing {
        let ex = extra_maps(&g.scene, &ins, &g.view, None, &mut |_| {});
        g.current = Some((id, ex));
        set_out(vec![]);
    } else {
        let grids = band_grids(&g.scene, &ins);
        let ex = extra_maps(&g.scene, &ins, &g.view, Some(&grids), &mut |_| {});
        g.current = Some((id, ex));
        set_out(grids.to_bytes());
    }
    ins.bands.len() as i32
}

/// Finish the prepared instrument with the marched volume (f32, w × h × nb,
/// band fastest; empty if none): instrument, images. Leaves packed images.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_finish(p: *const u8, n: usize, spp: u32, noise: u32) -> i32 {
    let Some(g) = GEN.as_mut() else { return -1 };
    let Some((id, mut maps)) = g.current.take() else { return -1 };
    let Some(ins) = survey::instrument_for(&g.preset, &id) else { return -1 };
    let nb = ins.bands.len();
    let px = g.view.w * g.view.h;
    if n >= px * nb * 4 {
        let b = bytes(p, n);
        for i in 0..px { for k in 0..nb {
            let o = 4 * (i * nb + k);
            maps[k][i] += f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
        }}
    }
    let obs = apply_instrument(&ins, &g.view, maps, &ObsOpts { spp, seed: g.scene.seed ^ 0x5u64, noise: noise != 0 });
    let imgs = survey::instrument_images(&g.preset, &obs);
    set_out(pack(&imgs));
    g.done.retain(|o| o.ins.id != id);
    g.done.push(obs);
    0
}

/// Cross-instrument composites and the montage from every finished instrument.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_cross() {
    let Some(g) = GEN.as_ref() else { set_out(vec![]); return };
    let mut imgs = survey::cross_images(&g.preset, &g.done);
    if let Some(m) = survey::montage_image(&g.preset, &g.done) { imgs.push(m); }
    set_out(pack(&imgs));
}

/// FITS file of one band (observed and noise-free planes) of a finished instrument.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_fits(p: *const u8, n: usize, band: usize) {
    let Some(g) = GEN.as_ref() else { set_out(vec![]); return };
    let id = string(p, n);
    let Some(o) = g.done.iter().find(|o| o.ins.id == id) else { set_out(vec![]); return };
    let Some(m) = o.maps.get(band) else { set_out(vec![]); return };
    use export::Card;
    let cards = vec![
        Card::Str("TELESCOP", "Schwarzschild Observatory (synthetic)".into(), ""),
        Card::Str("INSTRUME", o.ins.name.clone(), ""),
        Card::Str("FILTER", m.name.clone(), ""),
        Card::Str("BUNIT", o.ins.unit().into(), "plane 1: observed, plane 2: noise-free"),
        Card::Float("EXPTIME", o.ins.exposure_s, "s"),
        Card::Float("PIXSCALE", o.pix_arcsec, "arcsec per pixel"),
        Card::Float("NOISE", m.sigma, "rms per pixel, BUNIT"),
        Card::Comment(format!("Modelled on {}.", o.ins.analog)),
    ];
    set_out(export::fits(o.w, o.h, &[&m.data, &m.ideal], &cards));
}

/// March worker: take band grids and a view.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn m_init(gp: *const u8, gn: usize, vp: *const u8, vn: usize) -> i32 {
    let Some(grids) = Grids::from_bytes(bytes(gp, gn)) else { return -1 };
    let Ok(vj) = Json::parse(&string(vp, vn)) else { return -1 };
    MARCH = Some(March { grids, view: view_from(&vj) });
    0
}

/// March rows y ≡ k (mod nw); leaves f32 rows (w × nb each, in order) in the output.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn m_rows(k: usize, nw: usize, spp: u32, seed_lo: u32, seed_hi: u32) -> i32 {
    let Some(m) = MARCH.as_ref() else { return -1 };
    let seed = (seed_hi as u64) << 32 | seed_lo as u64;
    let mut out = Vec::new();
    let mut y = k;
    while y < m.view.h {
        for v in march_row(&m.grids, &m.view, y, spp, seed) { out.extend_from_slice(&(v as f32).to_le_bytes()); }
        y += nw;
    }
    set_out(out);
    0
}

/// Free the march worker's grids.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn m_free() { MARCH = None; }

/// The march seed the generator uses (so workers match the CLI).
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn s_seed() -> f64 { GEN.as_ref().map(|g| (g.scene.seed ^ 0x5u64) as f64).unwrap_or(0.0) }
