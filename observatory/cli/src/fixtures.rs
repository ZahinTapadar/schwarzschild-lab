//! `observatory fixtures [--update]`: render every preset at 128 px width and
//! 8 samples per pixel, check it against per-preset limits, and compare it
//! with the stored fixture (linear rates, tonemapped PNG, one diagnostic PNG,
//! metadata JSON under fixtures/). Exits non-zero on any failure.
//!
//! Limits come from the preset's optional "checks" block:
//!   max_saturated, max_clipped, max_failed, disk_fraction [lo, hi],
//!   capture_fraction [lo, hi], max_star_density_deg2, min_volume_fraction.

use observatory_core::json::Json;
use observatory_core::render::{render_pass, Accum, Ctx, D_VSTEPS, ND};
use observatory_core::scene::Scene;
use observatory_core::sensor::{self, DisplayParams};
use observatory_core::{annotate, export};
use std::path::Path;

const W: usize = 128;
const SPP: u32 = 8;

fn png(path: &Path, w: usize, h: usize, rgba: &[u8], class: &str) {
    let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::write(path, export::png(w, h, &rgb, &[("Class", class.into())])).expect("write png");
}

fn linear_bytes(w: usize, h: usize, planes: &[Vec<f32>]) -> Vec<u8> {
    let mut b = b"OBSFIX01".to_vec();
    for v in [w as u32, h as u32, planes.len() as u32] { b.extend_from_slice(&v.to_le_bytes()); }
    for p in planes { for v in p { b.extend_from_slice(&v.to_le_bytes()); } }
    b
}

fn linear_load(b: &[u8]) -> Option<(usize, usize, Vec<Vec<f32>>)> {
    if b.len() < 20 || &b[..8] != b"OBSFIX01" { return None; }
    let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let (w, h, n) = (u(8), u(12), u(16));
    let mut planes = Vec::new();
    let mut i = 20;
    for _ in 0..n {
        planes.push((0..w * h).map(|k| f32::from_le_bytes(b[i + 4 * k..i + 4 * k + 4].try_into().unwrap())).collect());
        i += 4 * w * h;
    }
    Some((w, h, planes))
}

pub fn run(args: &[String]) {
    let update = args.iter().any(|a| a == "--update");
    let root = Path::new(".");
    let fix = root.join("fixtures");
    let cur = root.join("out/fixtures");
    std::fs::create_dir_all(&fix).ok();
    std::fs::create_dir_all(&cur).ok();
    let mut names: Vec<_> = std::fs::read_dir(root.join("presets")).expect("presets/").filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect();
    names.sort();
    let mut failures = Vec::new();
    for path in names {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let j = Json::parse(&std::fs::read_to_string(&path).unwrap()).expect("preset JSON");
        let checks = j.get("checks").cloned().unwrap_or(Json::obj());
        let mut sc = Scene::from_json(&j);
        // smaller frame, same field: keep electrons per pixel for extended
        // sources by scaling the exposure with the pixel solid angle
        let shrink = W as f64 / sc.width as f64;
        sc.height = (W as f64 * sc.height as f64 / sc.width as f64).round() as usize;
        sc.width = W;
        sc.exposure_s *= shrink * shrink;
        for e in sc.exposures.iter_mut() { e.exposure_s *= shrink * shrink; }
        sc.target_spp = SPP;
        sc.output_class = "fixture".into();
        let (vol, _) = crate::render_cmd::load_volume(&sc, path.parent().unwrap());
        let ctx = Ctx::new(sc.clone(), vol, &mut |_| {});
        let mut acc = Accum::new(sc.width, sc.height);
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let st = render_pass(&ctx, &mut acc, SPP, threads);
        let ins = sensor::Instrument::from_ctx(&ctx);
        let fr = sensor::frame(&ins, &acc);
        let ro = sensor::readout(&ins, &fr);
        let cv = sensor::convergence(&ins, &fr, Some(&ro));
        let mut fail = |what: String| { eprintln!("  FAIL {stem}: {what}"); failures.push(format!("{stem}: {what}")); };

        // --- physical sanity
        if !acc.sum.iter().all(|v| v.is_finite() && *v >= 0.0) { fail("NaN or negative radiance".into()); }
        if !ro.expected.iter().flatten().all(|v| v.is_finite() && *v >= 0.0) { fail("NaN or negative expected electrons".into()); }
        let num = |k: &str, d: f64| checks.num(k).unwrap_or(d);
        let range = |k: &str| checks.arr(k).filter(|a| a.len() == 2).map(|a| (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(1.0)));
        if ro.saturated > num("max_saturated", 0.005) { fail(format!("saturated fraction {:.4}", ro.saturated)); }
        if cv.failed_fraction > num("max_failed", 1e-3) { fail(format!("failed-ray fraction {:.2e}", cv.failed_fraction)); }
        let dp = DisplayParams::from_scene(&sc);
        let rgba = sensor::display(&ins, &fr, &ro, &dp);
        let npx = sc.width * sc.height;
        let clipped = rgba.chunks(4).filter(|p| p[0] == 255 && p[1] == 255 && p[2] == 255).count() as f64 / npx as f64;
        if clipped > num("max_clipped", 0.02) { fail(format!("display clipping {:.3}", clipped)); }
        let total = (st.samples as f64).max(1.0);
        let (disk_f, capt_f) = (st.disk as f64 / total, st.captured as f64 / total);
        if let Some((lo, hi)) = range("disk_fraction") { if disk_f < lo || disk_f > hi { fail(format!("disk occupancy {disk_f:.3} outside [{lo}, {hi}]")); } }
        if let Some((lo, hi)) = range("capture_fraction") { if capt_f < lo || capt_f > hi { fail(format!("shadow (capture) fraction {capt_f:.4} outside [{lo}, {hi}]")); } }
        let mut star_density = 0.0;
        if let Some(df) = ctx.sky.as_ref().and_then(|s| s.deep.as_ref()) {
            let area_deg2 = 2.0 * std::f64::consts::PI * (1.0 - df.radius.cos()) * (180.0 / std::f64::consts::PI).powi(2);
            star_density = df.stars.len() as f64 / area_deg2;
            if star_density > num("max_star_density_deg2", 2.0e4) { fail(format!("resolved-star density {star_density:.0} per deg²")); }
        }
        let mut vol_frac = 0.0;
        if sc.volume.is_some() {
            vol_frac = (0..npx).filter(|&p| acc.diag[p * ND + D_VSTEPS] > 0.0).count() as f64 / npx as f64;
            if vol_frac < num("min_volume_fraction", 0.01) { fail(format!("volume reached by only {:.3} of pixels", vol_frac)); }
        }

        // --- fixture files
        let mut planes = fr.rates.clone();
        for k in 0..3 { planes.push(fr.truecol.iter().map(|c| c[k]).collect()); }
        let lin = linear_bytes(sc.width, sc.height, &planes);
        let diag_view = if sc.volume.is_some() { "tau" } else if sc.disk || !sc.is_telescope() { "fate" } else { "variance" };
        let drgba = sensor::display(&ins, &fr, &ro, &DisplayParams { view: diag_view.into(), ..dp.clone() });
        let (dimg, dw, dh) = match sensor::legend(diag_view, &ins) { Some(lg) => annotate::with_legend(&drgba, sc.width, sc.height, &lg), None => (drgba, sc.width, sc.height) };
        let mut meta = Json::obj();
        meta.set("preset", stem.as_str()).set("width", sc.width).set("height", sc.height).set("spp", SPP)
            .set("saturated", ro.saturated).set("clipped", clipped).set("failed", cv.failed_fraction)
            .set("disk_fraction", disk_f).set("capture_fraction", capt_f).set("star_density_deg2", star_density)
            .set("volume_fraction", vol_frac).set("diagnostic", diag_view).set("mode", sc.mode.as_str());
        let dir = if update { &fix } else { &cur };
        std::fs::write(dir.join(format!("{stem}.lin")), &lin).unwrap();
        png(&dir.join(format!("{stem}-sensor.png")), sc.width, sc.height, &rgba, "presentation");
        png(&dir.join(format!("{stem}-{diag_view}.png")), dw, dh, &dimg, "diagnostic");
        std::fs::write(dir.join(format!("{stem}.json")), meta.dump()).unwrap();

        // --- regression against the stored fixture
        if !update {
            match std::fs::read(fix.join(format!("{stem}.lin"))).ok().and_then(|b| linear_load(&b)) {
                None => fail("no stored fixture (run: observatory fixtures --update)".into()),
                Some((fw, fh, old)) => {
                    if (fw, fh, old.len()) != (sc.width, sc.height, planes.len()) {
                        fail("fixture shape changed".into());
                    } else {
                        for (k, (a, b)) in planes.iter().zip(&old).enumerate() {
                            let (mut d, mut n) = (0.0f64, 0.0f64);
                            for (x, y) in a.iter().zip(b) { d += (*x as f64 - *y as f64).abs(); n += (*y as f64).abs(); }
                            let rel = if n > 0.0 { d / n } else { d };
                            if rel > num("max_regression", 0.02) { fail(format!("plane {k} differs from fixture by {:.2}% (L1)", rel * 100.0)); }
                        }
                        if let Some(om) = std::fs::read_to_string(fix.join(format!("{stem}.json"))).ok().and_then(|s| Json::parse(&s).ok()) {
                            let oc = om.num("capture_fraction").unwrap_or(capt_f);
                            if (oc - capt_f).abs() > 0.002 + 0.02 * oc { fail(format!("shadow size changed: capture fraction {oc:.4} → {capt_f:.4}")); }
                        }
                    }
                }
            }
        }
        eprintln!("{stem:<20} sat {:.4}  clip {:.3}  fail {:.1e}  disk {:.3}  capt {:.4}  stars/deg² {:>6.0}  vol {:.3}", ro.saturated, clipped, cv.failed_fraction, disk_f, capt_f, star_density, vol_frac);
    }
    if failures.is_empty() {
        eprintln!("fixtures: all presets {}", if update { "written" } else { "pass" });
    } else {
        eprintln!("fixtures: {} failure(s)", failures.len());
        std::process::exit(1);
    }
}
