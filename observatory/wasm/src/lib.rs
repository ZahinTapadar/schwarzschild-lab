//! WebAssembly build of the Observatory core for the browser.
//!
//! Two roles, one module:
//! * **render worker** (`w_*`): owns every `nw`-th row starting at `k`,
//!   accumulates samples for them, and packs a reduced frame for its rows;
//! * **compositor** (`c_*`): holds the assembled full-image frame, runs the
//!   sensor readout, the display views and the exporters.
//!
//! Strings and byte buffers cross the boundary through `alloc`/`dealloc`;
//! results are left in an output buffer (`out_ptr`/`out_len`).

use observatory_core::export;
use observatory_core::json::Json;
use observatory_core::render::{render_rows, Accum, Ctx, PassStats, ND};
use observatory_core::scene::Scene;
use observatory_core::sensor::{self, DisplayParams, Frame, Instrument, Readout};
use observatory_core::spectrum::NCH;
use observatory_core::volume::Volume;

static mut OUT: Vec<u8> = Vec::new();
static mut LOG: String = String::new();

#[allow(static_mut_refs)]
fn set_out(b: Vec<u8>) { unsafe { OUT = b; } }

#[no_mangle]
pub extern "C" fn alloc(n: usize) -> *mut u8 {
    let mut v = vec![0u8; n.max(1)];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}
#[no_mangle]
pub unsafe extern "C" fn dealloc(p: *mut u8, n: usize) {
    drop(Vec::from_raw_parts(p, n.max(1), n.max(1)));
}
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn out_ptr() -> *const u8 { unsafe { OUT.as_ptr() } }
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn out_len() -> usize { unsafe { OUT.len() } }
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn log_to_out() { unsafe { set_out(LOG.as_bytes().to_vec()); LOG.clear(); } }

unsafe fn bytes<'a>(p: *const u8, n: usize) -> &'a [u8] { std::slice::from_raw_parts(p, n) }
unsafe fn string(p: *const u8, n: usize) -> String { String::from_utf8_lossy(bytes(p, n)).into_owned() }

// ------------------------------------------------------------- worker --
struct Worker {
    ctx: Ctx,
    ins: Instrument,
    k: usize,
    nw: usize,
    rows: Vec<usize>,
    acc: Accum, // width × rows.len()
    stats: PassStats,
}
static mut WORKER: Option<Worker> = None;

static mut SKY_IN: Vec<u8> = Vec::new();

/// Provide a serialized sky catalogue for the next w_init (skips generation).
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_give_sky(p: *const u8, n: usize) { SKY_IN = bytes(p, n).to_vec(); }

/// Serialized sky catalogue of the initialised worker, into the output buffer.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_sky_bytes() {
    if let Some(wk) = WORKER.as_ref() { if let Some(s) = wk.ctx.sky.as_ref() { set_out(s.to_bytes()); return; } }
    set_out(Vec::new());
}

/// Initialise a render worker. Returns the number of rows it owns.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_init(scene_p: *const u8, scene_n: usize, vol_p: *const u8, vol_n: usize, k: usize, nw: usize) -> i32 {
    let Ok(j) = Json::parse(&string(scene_p, scene_n)) else { return -1 };
    let scene = Scene::from_json(&j);
    let vol = if vol_n > 0 {
        match Volume::from_bytes(bytes(vol_p, vol_n)) { Ok(v) => Some(v), Err(e) => { LOG.push_str(&e); return -2; } }
    } else { None };
    let sky = if SKY_IN.is_empty() { None } else { observatory_core::sky::Sky::from_bytes(&SKY_IN).ok() };
    SKY_IN.clear();
    let ctx = Ctx::with_sky(scene.clone(), sky, vol, &mut |s| { LOG.push_str(s); LOG.push('\n'); });
    let ins = Instrument::from_ctx(&ctx);
    let rows: Vec<usize> = (k..scene.height).step_by(nw.max(1)).collect();
    let acc = Accum::new(scene.width, rows.len());
    let n = rows.len() as i32;
    WORKER = Some(Worker { ctx, ins, k, nw, rows, acc, stats: PassStats::default() });
    n
}

/// Add `add` samples per pixel to every owned row. Returns new spp.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_render(add: u32) -> u32 {
    let Some(wk) = WORKER.as_mut() else { return 0 };
    let w = wk.acc.w;
    let spp0 = wk.acc.spp;
    for (i, &y) in wk.rows.iter().enumerate() {
        let st = render_rows(
            &wk.ctx, y, y + 1, spp0, add,
            &mut wk.acc.sum[i * w * NCH..(i + 1) * w * NCH],
            &mut wk.acc.lsum[i * w..(i + 1) * w],
            &mut wk.acc.lsq[i * w..(i + 1) * w],
            &mut wk.acc.diag[i * w * ND..(i + 1) * w * ND],
        );
        let t = &mut wk.stats;
        t.samples += st.samples; t.steps += st.steps; t.vol_steps += st.vol_steps; t.vol_rays += st.vol_rays;
        t.failed += st.failed; t.escaped += st.escaped; t.captured += st.captured; t.disk += st.disk;
    }
    wk.acc.spp += add;
    wk.acc.spp
}

/// Stats as JSON in the output buffer.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_stats() {
    let Some(wk) = WORKER.as_ref() else { return };
    let t = &wk.stats;
    let mut j = Json::obj();
    j.set("spp", wk.acc.spp).set("samples", t.samples).set("steps", t.steps).set("vol_steps", t.vol_steps)
        .set("vol_rays", t.vol_rays).set("failed", t.failed).set("escaped", t.escaped).set("captured", t.captured).set("disk", t.disk);
    set_out(j.dump().into_bytes());
}

/// Packed reduced frame for the owned rows (f32 little-endian):
/// per pixel [rates (nf), truecol (3), lmean, rel_err, diag (ND)].
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_pack() -> usize {
    let Some(wk) = WORKER.as_ref() else { return 0 };
    let a = &wk.acc;
    let fr = sensor::frame_rows(&wk.ins, a.w, 0, a.h, a.spp, &a.sum, &a.lsum, &a.lsq, &a.diag);
    let nf = fr.rates.len();
    let stride = nf + 5 + ND;
    let mut out = Vec::with_capacity(a.w * a.h * stride * 4);
    for p in 0..a.w * a.h {
        for f in 0..nf { out.extend_from_slice(&fr.rates[f][p].to_le_bytes()); }
        for c in 0..3 { out.extend_from_slice(&fr.truecol[p][c].to_le_bytes()); }
        out.extend_from_slice(&fr.lmean[p].to_le_bytes());
        out.extend_from_slice(&fr.rel_err[p].to_le_bytes());
        for k in 0..ND { out.extend_from_slice(&fr.diag[p * ND + k].to_le_bytes()); }
    }
    set_out(out);
    stride
}

/// Raw accumulators for the owned rows (for checkpoints), row-major in
/// global row order: [sum f32 NCH][lsum f64][lsq f64][diag f32 ND] per pixel.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_get_accum() {
    let Some(wk) = WORKER.as_ref() else { return };
    let a = &wk.acc;
    let mut out = Vec::with_capacity(a.w * a.h * (NCH * 4 + 16 + ND * 4));
    for p in 0..a.w * a.h {
        for c in 0..NCH { out.extend_from_slice(&a.sum[p * NCH + c].to_le_bytes()); }
        out.extend_from_slice(&a.lsum[p].to_le_bytes());
        out.extend_from_slice(&a.lsq[p].to_le_bytes());
        for k in 0..ND { out.extend_from_slice(&a.diag[p * ND + k].to_le_bytes()); }
    }
    set_out(out);
}

#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_set_accum(p: *const u8, n: usize, spp: u32) -> i32 {
    let Some(wk) = WORKER.as_mut() else { return -1 };
    let a = &mut wk.acc;
    let per = NCH * 4 + 16 + ND * 4;
    if n != a.w * a.h * per { return -2; }
    let b = bytes(p, n);
    let mut i = 0;
    let f32_at = |i: &mut usize| { let v = f32::from_le_bytes(b[*i..*i + 4].try_into().unwrap()); *i += 4; v };
    let f64_at = |i: &mut usize| { let v = f64::from_le_bytes(b[*i..*i + 8].try_into().unwrap()); *i += 8; v };
    for q in 0..a.w * a.h {
        for c in 0..NCH { a.sum[q * NCH + c] = f32_at(&mut i); }
        a.lsum[q] = f64_at(&mut i);
        a.lsq[q] = f64_at(&mut i);
        for k in 0..ND { a.diag[q * ND + k] = f32_at(&mut i); }
    }
    a.spp = spp;
    0
}

/// Change filters / optics without re-rendering: the spectral sums are kept,
/// only the per-filter weights used by w_pack change.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_set_instrument(scene_p: *const u8, scene_n: usize) -> i32 {
    let Some(wk) = WORKER.as_mut() else { return -1 };
    let Ok(j) = Json::parse(&string(scene_p, scene_n)) else { return -2 };
    let mut sc = Scene::from_json(&j);
    // geometry stays what was rendered
    sc.width = wk.ctx.scene.width;
    sc.height = wk.ctx.scene.height;
    sc.fov_deg = wk.ctx.scene.fov_deg;
    sc.cam_r = wk.ctx.scene.cam_r;
    sc.mass = wk.ctx.scene.mass;
    wk.ins = Instrument::new(sc);
    wk.ins.filters.len() as i32
}

#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn w_reset() {
    if let Some(wk) = WORKER.as_mut() { wk.acc.reset(); wk.stats = PassStats::default(); }
    let _ = WORKER.as_ref().map(|w| (w.k, w.nw));
}

// --------------------------------------------------------- compositor --
struct Comp {
    ins: Instrument,
    frame: Frame,
    ro: Option<Readout>,
}
static mut COMP: Option<Comp> = None;

#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_init(scene_p: *const u8, scene_n: usize) -> i32 {
    let Ok(j) = Json::parse(&string(scene_p, scene_n)) else { return -1 };
    let scene = Scene::from_json(&j);
    let ins = Instrument::new(scene);
    let (w, h, nf) = (ins.scene.width, ins.scene.height, ins.filters.len());
    let frame = Frame { w, h, spp: 0, rates: vec![vec![0.0; w * h]; nf], truecol: vec![[0.0; 3]; w * h], lmean: vec![0.0; w * h], rel_err: vec![0.0; w * h], diag: vec![0.0; w * h * ND] };
    COMP = Some(Comp { ins, frame, ro: None });
    nf as i32
}

/// Scatter a worker's packed rows (k, nw) into the full frame.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_put(p: *const u8, n: usize, k: usize, nw: usize, spp: u32) -> i32 {
    let Some(c) = COMP.as_mut() else { return -1 };
    let fr = &mut c.frame;
    let nf = fr.rates.len();
    let stride = nf + 5 + ND;
    let b = bytes(p, n);
    let w = fr.w;
    let rows: Vec<usize> = (k..fr.h).step_by(nw.max(1)).collect();
    if n != rows.len() * w * stride * 4 { return -2; }
    let at = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
    for (ri, &y) in rows.iter().enumerate() {
        for x in 0..w {
            let base = (ri * w + x) * stride;
            let q = y * w + x;
            for f in 0..nf { fr.rates[f][q] = at(base + f); }
            fr.truecol[q] = [at(base + nf), at(base + nf + 1), at(base + nf + 2)];
            fr.lmean[q] = at(base + nf + 3);
            fr.rel_err[q] = at(base + nf + 4);
            for d in 0..ND { fr.diag[q * ND + d] = at(base + nf + 5 + d); }
        }
    }
    fr.spp = spp;
    0
}

/// Run the sensor readout; returns convergence stats JSON in the output buffer.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_readout() {
    let Some(c) = COMP.as_mut() else { return };
    let ro = sensor::readout(&c.ins, &c.frame);
    let cv = sensor::convergence(&c.ins, &c.frame, Some(&ro));
    let mut j = Json::obj();
    j.set("spp", c.frame.spp).set("exposure_s", ro.t).set("render_noise", cv.render_noise).set("sensor_noise", cv.sensor_noise)
        .set("failed_fraction", cv.failed_fraction).set("mean_steps", cv.mean_steps).set("mean_vol_steps", cv.mean_vol_steps)
        .set("disk_fraction", cv.disk_fraction).set("capture_fraction", cv.capture_fraction).set("saturated", ro.saturated)
        .set("cosmic_rays", ro.cosmic_rays)
        .set("filters", ro.filters.iter().map(|f| Json::from(f.as_str())).collect::<Vec<_>>());
    c.ro = Some(ro);
    set_out(j.dump().into_bytes());
}

/// RGBA8 for a view into the output buffer. `view` is a name from sensor::VIEWS.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_display(view_p: *const u8, view_n: usize, band: usize, white_pct: f64, soft: f64, gain: f64) {
    let Some(c) = COMP.as_ref() else { return };
    let Some(ro) = c.ro.as_ref() else { return };
    let dp = DisplayParams { view: string(view_p, view_n), band, white_pct, soft, gain, ..DisplayParams::from_scene(&c.ins.scene) };
    set_out(sensor::display(&c.ins, &c.frame, ro, &dp));
    LAST_DP = Some(dp);
}

static mut LAST_DP: Option<DisplayParams> = None;

/// Legend of a view as JSON ({} for presentation views): title, note, class,
/// and either categories [[name, [r, g, b]]] or a ramp {stops, ticks}.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_legend(view_p: *const u8, view_n: usize) {
    use observatory_core::annotate::Key;
    let Some(c) = COMP.as_ref() else { return };
    let view = string(view_p, view_n);
    let mut j = Json::obj();
    j.set("class", sensor::view_class(&view));
    if let Some(lg) = sensor::legend(&view, &c.ins) {
        j.set("title", lg.title.as_str()).set("note", lg.note.as_str());
        let rgb = |c: [u8; 3]| Json::from(vec![Json::from(c[0] as f64), (c[1] as f64).into(), (c[2] as f64).into()]);
        match lg.key {
            Key::Categories(cats) => { j.set("categories", cats.iter().map(|(n, c)| Json::from(vec![Json::from(n.as_str()), rgb(*c)])).collect::<Vec<_>>()); }
            Key::Ramp { stops, ticks } => {
                j.set("stops", stops.iter().map(|c| rgb(*c)).collect::<Vec<_>>());
                j.set("ticks", ticks.iter().map(|(t, l)| Json::from(vec![Json::from(*t), Json::from(l.as_str())])).collect::<Vec<_>>());
            }
        }
    }
    set_out(j.dump().into_bytes());
}

/// Scene scale and palette summary as JSON (for the page's info line).
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_info() {
    let Some(c) = COMP.as_ref() else { return };
    let s = &c.ins.scene;
    let (kind, keys) = sensor::palette_description(&c.ins);
    let mut j = Json::obj();
    j.set("mode", s.mode.as_str()).set("r0_m", s.cam_r).set("distance_pc", s.observer_distance_m() / observatory_core::scene::PC_M)
        .set("shadow_arcsec", s.shadow_radius_rad().to_degrees() * 3600.0)
        .set("einstein_arcmin", s.einstein_radius_rad(if s.volume.is_some() { s.volume_distance_pc * observatory_core::scene::PC_M } else { 0.0 }).to_degrees() * 60.0)
        .set("pixel_arcsec", s.pixel_scale().to_degrees() * 3600.0)
        .set("palette", kind.as_str())
        .set("palette_keys", keys.iter().map(|(n, _)| Json::from(n.as_str())).collect::<Vec<_>>())
        .set("lensing", s.lensing);
    set_out(j.dump().into_bytes());
}

/// Export: 0 = PNG (current view must be passed), 1 = EXR, 2 = FITS, 3 = JSON metadata.
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_export(kind: u32, view_p: *const u8, view_n: usize, date_p: *const u8, date_n: usize, vol_p: *const u8, vol_n: usize, elapsed: f64) {
    let Some(c) = COMP.as_ref() else { return };
    let Some(ro) = c.ro.as_ref() else { return };
    let date = string(date_p, date_n);
    let vol_id = string(vol_p, vol_n);
    let (w, h) = (c.frame.w, c.frame.h);
    let cv = sensor::convergence(&c.ins, &c.frame, Some(ro));
    let out = match kind {
        0 => {
            let view = string(view_p, view_n);
            let mut dp = LAST_DP.clone().unwrap_or_default();
            dp.view = view.clone();
            let rgba = sensor::display(&c.ins, &c.frame, ro, &dp);
            let class = sensor::view_class(&view);
            let (img, iw, ih) = match sensor::legend(&view, &c.ins) {
                Some(lg) => observatory_core::annotate::with_legend(&rgba, w, h, &lg),
                None => (rgba, w, h),
            };
            let rgb: Vec<u8> = img.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            let pal = sensor::palette_description(&c.ins).0;
            export::png(iw, ih, &rgb, &[("Class", class.into()), ("View", view.clone()), ("Palette", pal), ("Comment", "Synthetic instrument simulation; not a real observation.".into())])
        }
        1 => {
            let mut chans: Vec<(String, Vec<f32>)> = Vec::new();
            for (i, f) in ro.filters.iter().enumerate() {
                chans.push((format!("sensor.{f}.electrons"), ro.electrons[i].clone()));
                chans.push((format!("sensor.{f}.expected"), ro.expected[i].clone()));
                chans.push((format!("rate.{f}"), c.frame.rates[i].clone()));
            }
            for (k, n) in ["R", "G", "B"].iter().enumerate() {
                chans.push((format!("truecolor.{n}"), c.frame.truecol.iter().map(|t| t[k]).collect()));
            }
            let refs: Vec<(&str, &[f32])> = chans.iter().map(|(a, b)| (a.as_str(), b.as_slice())).collect();
            export::exr(w, h, &refs)
        }
        2 => {
            let planes: Vec<&[f32]> = ro.electrons.iter().map(|v| v.as_slice()).collect();
            export::fits(w, h, &planes, &sensor::fits_cards(&c.ins, &c.frame, ro, &cv, &date, &vol_id))
        }
        _ => {
            let mut m = sensor::metadata(&c.ins, &c.frame, ro, &cv, elapsed, &vol_id);
            m.set("render_date", date.as_str()).set("platform", "browser (WebAssembly)");
            m.dump().into_bytes()
        }
    };
    set_out(out);
}

/// Checkpoint header for the browser: scene JSON wrapper (the page assembles
/// the OBSCKPT2 file from this header and the workers' raw accumulators).
#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn c_scene_json() {
    let Some(c) = COMP.as_ref() else { return };
    let mut j = Json::obj();
    j.set("scene", c.ins.scene.to_json()).set("base", ".");
    set_out(j.dump().into_bytes());
}
