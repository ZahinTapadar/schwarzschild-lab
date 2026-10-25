//! `observatory render` / `observatory resume`.
//!
//!   render <scene.json> [--spp N] [--pass K] [--threads T] [--w W --h H]
//!          [--tree DIR --name NAME | --out DIR/NAME] [--snap 16,64,256]
//!          [--views radiance,fate,...] [--set key=json]... [--unlensed]
//!   resume <NAME.ckpt>  [--spp N] ...
//!
//! The output class follows the resolution actually rendered: presentation
//! (≥ 1024 px wide; high_quality ≥ 1600 when requested), validation (≥ 512)
//! or fixture. With --tree, presentation renders write
//!   DIR/presentation/NAME-W.png (+ -annotated, -visible, band planes, frames/, progress/)
//!   DIR/diagnostics/NAME-VIEW-W.png
//!   DIR/data/NAME-W.{exr,fits,json,ckpt}
//! and validation or fixture renders write everything flat in DIR.

use observatory_core::json::Json;
use observatory_core::render::{render_pass, Accum, Ctx, PassStats};
use observatory_core::scene::Scene;
use observatory_core::sensor::{self, DisplayParams};
use observatory_core::spectrum::{bin_center, LINES, NB, NCH};
use observatory_core::volume::Volume;
use observatory_core::export;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
}

fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y + (m <= 2) as i64, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn load_volume(scene: &Scene, base: &Path) -> (Option<Volume>, String) {
    let Some(p) = &scene.volume else { return (None, "none".into()) };
    let candidates = [base.join(p), PathBuf::from(p)];
    for c in &candidates {
        if let Ok(b) = std::fs::read(c) {
            let v = Volume::from_bytes(&b).unwrap_or_else(|e| panic!("{}: {e}", c.display()));
            let seed = v.meta.get("params").and_then(|q| q.num("seed")).unwrap_or(0.0) as u64;
            let id = format!("{} (n={}, seed={}, {} bytes)", c.file_name().unwrap().to_string_lossy(), v.n, seed, b.len());
            return (Some(v), id);
        }
    }
    panic!("volume cache '{p}' not found (generate it with: observatory volume)");
}

pub fn run(args: &[String]) {
    let resume = args[1] == "resume";
    let input = args.get(2).expect("scene or checkpoint path");
    let t_start = Instant::now();
    let input_json: Json;
    let (mut scene, base, mut acc0): (Scene, PathBuf, Option<Accum>) = if resume {
        let b = std::fs::read(input).expect("read checkpoint");
        let (sj, acc) = sensor::checkpoint_load(&b).expect("checkpoint");
        input_json = sj.get("scene").cloned().unwrap_or(Json::obj());
        (Scene::from_json(sj.get("scene").unwrap_or(&sj)), PathBuf::from(sj.str("base").unwrap_or(".")), Some(acc))
    } else {
        let src = std::fs::read_to_string(input).expect("read scene");
        let mut j = Json::parse(&src).expect("scene JSON");
        // --set key=value (value as JSON) overrides, repeatable
        for (i, a) in args.iter().enumerate() {
            if a == "--set" {
                let kv = args.get(i + 1).expect("--set key=value");
                let (k, v) = kv.split_once('=').expect("--set key=value");
                let val = Json::parse(v).unwrap_or_else(|_| Json::from(v));
                j.set(k, val);
            }
        }
        input_json = j.clone();
        (Scene::from_json(&j), Path::new(input).parent().unwrap_or(Path::new(".")).to_path_buf(), None)
    };
    if args.iter().any(|a| a == "--unlensed") { scene.lensing = false; }
    if let Some(w) = arg::<usize>(args, "--w") { scene.width = w; }
    if let Some(h) = arg::<usize>(args, "--h") { scene.height = h; }
    // the class is earned by the resolution rendered, never just declared
    let requested = scene.output_class.clone();
    scene.output_class = match () {
        _ if scene.width >= 1600 && requested == "high_quality" => "high_quality",
        _ if scene.width >= 1024 && (requested == "presentation" || requested == "high_quality") => "presentation",
        _ if scene.width >= 512 && requested != "fixture" => "validation",
        _ => "fixture",
    }.to_string();
    if scene.output_class != requested {
        eprintln!("output class: {requested} requested, {} at {}×{} (presentation needs ≥ 1024 px, validation ≥ 512)", scene.output_class, scene.width, scene.height);
    }
    let target: u32 = arg(args, "--spp").unwrap_or(scene.target_spp);
    let pass: u32 = arg(args, "--pass").unwrap_or(4);
    let threads: usize = arg(args, "--threads").unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let slug: String = scene.name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>()
        .split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-");
    let name: String = args.iter().position(|a| a == "--name").and_then(|i| args.get(i + 1)).cloned().unwrap_or(slug);
    let tree: Option<PathBuf> = args.iter().position(|a| a == "--tree").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let presentation_class = scene.output_class == "presentation" || scene.output_class == "high_quality";
    let paths = Paths::new(tree.as_deref(), &name, scene.width, presentation_class,
        args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| format!("out/{name}")));
    let mut paths = paths;
    if scene.presentation_mode == "diagnostic" && tree.is_some() && presentation_class {
        // a validation scene's own image is a check, filed with the diagnostics
        paths.main = paths.diag("radiance-check");
        paths.annotated = paths.diag("radiance-check-annotated");
        paths.visible = paths.diag("visible-check");
    }
    let out = paths.data.clone();
    let snaps: Vec<u32> = args.iter().position(|a| a == "--snap").and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let views: Vec<String> = args.iter().position(|a| a == "--views").and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(String::from).collect()).unwrap_or_else(|| vec!["radiance".into(), "fate".into()]);
    paths.create();
    let _ = &input_json;

    // display transform: the scene's recorded parameters, overridable by flags
    let mut dp = DisplayParams::from_scene(&scene);
    if let Some(v) = arg::<f64>(args, "--white") { dp.white_pct = v; }
    if let Some(v) = arg::<f64>(args, "--soft") { dp.soft = v; }
    if let Some(v) = arg::<f64>(args, "--gain") { dp.gain = v; }
    let (vol, vol_id) = load_volume(&scene, &base);
    let t_setup = Instant::now();
    let ctx = Ctx::new(scene.clone(), vol, &mut |s| eprintln!("{s}"));
    let setup_s = t_setup.elapsed().as_secs_f64();
    let ins = sensor::Instrument::from_ctx(&ctx);
    let mut acc = acc0.take().unwrap_or_else(|| Accum::new(scene.width, scene.height));
    assert_eq!((acc.w, acc.h), (scene.width, scene.height), "checkpoint size mismatch");
    eprintln!(
        "{}: {}×{}, {} filter(s) [{}], {} threads, target {} spp (from {}), f = {:.1} mm, D = {:.2} mm, {:.1}\"/px",
        scene.name, acc.w, acc.h, ctx.filters.len(), scene.filters.join(","), threads, target, acc.spp,
        scene.focal_length_mm(), scene.aperture() * 1e3, scene.pixel_scale().to_degrees() * 3600.0
    );

    let mut tot = PassStats::default();
    let t_render = Instant::now();
    let mut pass_s = 0.0;
    let mut timeline: Vec<Json> = Vec::new();
    let ckpt_path = format!("{out}.ckpt");
    let mut last_ckpt = Instant::now();
    let mut scene_ck = Json::obj();
    scene_ck.set("scene", scene.to_json()).set("base", base.to_string_lossy().to_string());
    while acc.spp < target {
        let next_snap = snaps.iter().copied().filter(|&s| s > acc.spp).min().unwrap_or(u32::MAX);
        let add = pass.min(target - acc.spp).min(next_snap - acc.spp);
        let tp = Instant::now();
        let st = render_pass(&ctx, &mut acc, add, threads);
        pass_s += tp.elapsed().as_secs_f64();
        tot.samples += st.samples; tot.steps += st.steps; tot.vol_steps += st.vol_steps; tot.vol_rays += st.vol_rays;
        tot.failed += st.failed; tot.escaped += st.escaped; tot.captured += st.captured; tot.disk += st.disk;
        let el = t_render.elapsed().as_secs_f64();
        if acc.spp.is_power_of_two() || acc.spp == target || snaps.contains(&acc.spp) {
            let fr = sensor::frame(&ins, &acc);
            let ro = sensor::readout(&ins, &fr);
            let cv = sensor::convergence(&ins, &fr, Some(&ro));
            eprintln!(
                "  spp {:>5}  {:>7.1} s  {:>6.2} Msamples/s  MC noise {:>6.2}%  shot+read {:>6.2}%  t_exp {:>6.1} s  fails {:.4}%  steps {:.0}  vol steps {:.0}",
                acc.spp, el, tot.samples as f64 / pass_s / 1e6, cv.render_noise * 100.0, cv.sensor_noise * 100.0, ro.t,
                cv.failed_fraction * 100.0, cv.mean_steps, cv.mean_vol_steps
            );
            let mut e = Json::obj();
            e.set("spp", acc.spp).set("wall_s", el).set("render_noise", cv.render_noise).set("sensor_noise", cv.sensor_noise).set("exposure_s", ro.t);
            timeline.push(e);
            if snaps.contains(&acc.spp) {
                let rgba = sensor::display(&ins, &fr, &ro, &dp);
                write_png(&paths.snapshot(acc.spp), acc.w, acc.h, &rgba, &[("Class", scene.output_class.clone()), ("View", format!("progress: sensor at {} spp, {:.3e} s", acc.spp, ro.t)), ("Display", dp.describe())]);
            }
        }
        if last_ckpt.elapsed().as_secs() > 60 {
            std::fs::write(&ckpt_path, sensor::checkpoint_bytes(&scene_ck, &acc)).ok();
            last_ckpt = Instant::now();
        }
    }
    let render_s = t_render.elapsed().as_secs_f64();
    eprintln!("pure sampling time {pass_s:.1} s ({:.2} Msamples/s); readouts/snapshots {:.1} s", tot.samples as f64 / pass_s / 1e6, render_s - pass_s);

    // ---- outputs
    let t_exp = Instant::now();
    let fr = sensor::frame(&ins, &acc);
    let ro = sensor::readout(&ins, &fr);
    let cv = sensor::convergence(&ins, &fr, Some(&ro));
    let date = utc_now();
    let (w, h) = (acc.w, acc.h);
    let mut outputs = Json::obj();
    let mut put = |path: &str, what: &str| { outputs.set(&file_name(path), what); };
    let synthetic = "Synthetic instrument simulation of a statistically synthesized scene; not a real observation.".to_string();
    let display = dp.describe();
    let (pal_kind, pal_keys) = sensor::palette_description(&ins);
    let class = if scene.presentation_mode == "diagnostic" { "diagnostic".to_string() } else { scene.output_class.clone() };
    let mode_text = match scene.presentation_mode.as_str() {
        "registered_hdr_composite" => "SYNTHETIC HDR COMPOSITE OF REGISTERED VIRTUAL EXPOSURES. NOT A SINGLE DETECTOR FRAME".to_string(),
        "stacked_survey" => format!("STACKED SURVEY COMPOSITE OF {} REGISTERED FRAMES, {:.0} S TOTAL", ro.frames.iter().map(|f| f.2).sum::<u32>(), ro.t),
        "diagnostic" => "VALIDATION SCENE (LOCAL OBSERVER): A CHECK, NOT A TELESCOPE IMAGE".to_string(),
        _ => format!("PHYSICAL SINGLE EXPOSURE, {:.3E} S{}", ro.t, if scene.neutral_density > 0.0 { format!(" THROUGH ND {}", scene.neutral_density) } else { String::new() }),
    };
    let exposures_text = ro.frames.iter().map(|(t, nd, n)| format!("{n} x {t:.3e} s ND {nd}")).collect::<Vec<_>>().join("; ");
    let pres = |what: &str| vec![("Class", class.clone()), ("PresentationMode", scene.presentation_mode.clone()), ("View", what.to_string()), ("Palette", pal_kind.clone()), ("Display", display.clone()), ("Exposures", exposures_text.clone()), ("Comment", synthetic.clone())];
    let rgba = sensor::display(&ins, &fr, &ro, &dp);
    write_png(&paths.main, w, h, &rgba, &pres("sensor"));
    put(&paths.main, &format!("{class}: {} — {}", scene.presentation_mode, pal_kind.to_lowercase()));
    // annotated copy: a caption naming the class, palette, exposures and stretch
    let cap = observatory_core::annotate::Legend {
        title: format!("{}: {}. {}", class.to_uppercase(), mode_text, pal_kind),
        key: observatory_core::annotate::Key::Categories(pal_keys.clone()),
        note: format!("{} SPP. {}. SYNTHETIC SCENE, NOT A REAL OBSERVATION", acc.spp, display.to_uppercase()),
    };
    let (ra, aw, ah) = observatory_core::annotate::with_legend(&rgba, w, h, &cap);
    write_png(&paths.annotated, aw, ah, &ra, &pres("sensor (annotated)"));
    put(&paths.annotated, &format!("{class}: annotated sensor image"));
    // a composite or stack also writes each registered exposure as the
    // physical frame it is
    if ro.composite {
        for (k, e) in scene.exposure_list().iter().enumerate() {
            let mut one = scene.clone();
            one.exposures = vec![observatory_core::scene::Exposure { count: 1, ..e.clone() }];
            one.neutral_density = e.neutral_density;
            one.exposure_s = e.exposure_s;
            let ins1 = sensor::Instrument::new(one);
            let ro1 = sensor::readout(&ins1, &fr);
            let img = sensor::display(&ins1, &fr, &ro1, &dp);
            let path = paths.frame(k);
            write_png(&path, w, h, &img, &[("Class", class.clone()), ("PresentationMode", "physical_single_exposure".into()), ("View", format!("registered frame {k}: {:.3e} s, ND {}", e.exposure_s, e.neutral_density)), ("Display", display.clone())]);
            put(&path, &format!("{class}: physical single exposure {k} of the composite ({:.3e} s, ND {})", e.exposure_s, e.neutral_density));
        }
    }
    for v in &views {
        if v == "parity" {
            let t0 = Instant::now();
            let par = ctx.parity_map();
            let (img, iw, ih) = sensor::parity_image(&par, w, h);
            let path = paths.diag("parity");
            write_png(&path, iw, ih, &img, &[("Class", "diagnostic".into()), ("View", "image parity".into())]);
            put(&path, "diagnostic: image parity of the screen-to-sky mapping");
            eprintln!("parity map: {:.1} s", t0.elapsed().as_secs_f64());
            continue;
        }
        if v == "band" {
            // raw filter planes (expected electrons), one greyscale image each
            for (fi, f) in ro.filters.iter().enumerate() {
                let rgba = sensor::display(&ins, &fr, &ro, &DisplayParams { view: "band".into(), band: fi, ..dp.clone() });
                let path = paths.band(f);
                write_png(&path, w, h, &rgba, &[("Class", class.clone()), ("View", format!("single filter plane {f}, noise-free expected electrons")), ("Display", display.clone()), ("Comment", synthetic.clone())]);
                put(&path, &format!("{class}: single filter plane {f}"));
            }
            continue;
        }
        let rgba = sensor::display(&ins, &fr, &ro, &DisplayParams { view: v.clone(), ..dp.clone() });
        let vclass = if sensor::view_class(v) == "diagnostic" { "diagnostic".to_string() } else { class.clone() };
        let (img, iw, ih) = match sensor::legend(v, &ins) {
            Some(lg) => observatory_core::annotate::with_legend(&rgba, w, h, &lg),
            None => (rgba, w, h),
        };
        let what = if v == "radiance" { "visible-light rendering: CIE 1931 weighting of the noise-free spectrum".to_string() } else { format!("{v} diagnostic, fixed scale") };
        let path = if v == "radiance" { paths.visible.clone() } else { paths.diag(v) };
        write_png(&path, iw, ih, &img, &[("Class", vclass.clone()), ("View", what.clone()), ("Display", display.clone()), ("Comment", synthetic.clone())]);
        put(&path, &format!("{vclass}: {what}"));
    }
    put(&format!("{out}.exr"), "linear: spectral radiance (26 channels) and per-filter electrons and rates");
    put(&format!("{out}.fits"), if ro.composite { "linear: calibrated composite rate per filter (e-/s)" } else { "linear: realised electrons per filter" });
    // EXR: linear e⁻/s per filter (noise-free), realised electrons, and the full spectral radiance
    let n = acc.spp.max(1) as f32;
    let mut chans: Vec<(String, Vec<f32>)> = Vec::new();
    for (i, f) in ro.filters.iter().enumerate() {
        chans.push((format!("sensor.{f}.electrons"), ro.electrons[i].clone()));
        chans.push((format!("sensor.{f}.expected"), ro.expected[i].clone()));
        chans.push((format!("sensor.{f}.rate"), ro.rate[i].clone()));
    }
    for c in 0..NCH {
        let name = if c < NB { format!("radiance.{:04.0}nm", bin_center(c)) } else { format!("radiance.line.{}", LINES[c - NB].0.replace(' ', "_")) };
        chans.push((name, (0..w * h).map(|p| acc.sum[p * NCH + c] / n).collect()));
    }
    let refs: Vec<(&str, &[f32])> = chans.iter().map(|(a, b)| (a.as_str(), b.as_slice())).collect();
    std::fs::write(format!("{out}.exr"), export::exr(w, h, &refs)).expect("write exr");
    let planes: Vec<&[f32]> = if ro.composite { ro.rate.iter().map(|v| v.as_slice()).collect() } else { ro.electrons.iter().map(|v| v.as_slice()).collect() };
    std::fs::write(format!("{out}.fits"), export::fits(w, h, &planes, &sensor::fits_cards(&ins, &fr, &ro, &cv, &date, &vol_id))).expect("write fits");
    let mut meta = sensor::metadata(&ins, &fr, &ro, &cv, render_s, &vol_id);
    meta.set("render_date", date.as_str()).set("setup_s", setup_s).set("timeline", timeline)
        .set("samples", tot.samples).set("samples_per_s", tot.samples as f64 / pass_s).set("sampling_s", pass_s)
        .set("pixels_per_s_per_spp", (w * h) as f64 * (acc.spp as f64 - 0.0) / render_s / acc.spp.max(1) as f64)
        .set("threads", threads).set("vol_rays", tot.vol_rays)
        .set("outputs", outputs)
        .set("output_class", class.as_str()).set("presentation_mode", scene.presentation_mode.as_str())
        .set("display", display.as_str()).set("exposures_taken", exposures_text.as_str())
        .set("export_s", t_exp.elapsed().as_secs_f64());
    std::fs::write(format!("{out}.json"), meta.dump()).expect("write json");
    std::fs::write(&ckpt_path, sensor::checkpoint_bytes(&scene_ck, &acc)).expect("write checkpoint");
    eprintln!(
        "done: {} spp in {:.1} s ({:.2} Msamples/s), export {:.2} s, total {:.1} s → {out}.{{png,exr,fits,json,ckpt}}",
        acc.spp, render_s, tot.samples as f64 / render_s / 1e6, t_exp.elapsed().as_secs_f64(), t_start.elapsed().as_secs_f64()
    );
}

fn file_name(out: &str) -> String {
    Path::new(out).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
}

/// Where each output goes (see the module header).
struct Paths {
    main: String,
    annotated: String,
    visible: String,
    data: String,
    pres_dir: PathBuf,
    diag_dir: PathBuf,
    name: String,
    w: usize,
    flat_prefix: Option<String>,
}

impl Paths {
    fn new(tree: Option<&Path>, name: &str, w: usize, presentation: bool, legacy: String) -> Paths {
        match tree {
            Some(t) if presentation => Paths {
                main: t.join("presentation").join(format!("{name}-{w}.png")).to_string_lossy().into(),
                annotated: t.join("presentation").join(format!("{name}-{w}-annotated.png")).to_string_lossy().into(),
                visible: t.join("presentation").join(format!("{name}-visible-{w}.png")).to_string_lossy().into(),
                data: t.join("data").join(format!("{name}-{w}")).to_string_lossy().into(),
                pres_dir: t.join("presentation"),
                diag_dir: t.join("diagnostics"),
                name: name.into(),
                w,
                flat_prefix: None,
            },
            Some(t) => {
                let pre = t.join(format!("{name}-{w}")).to_string_lossy().to_string();
                Paths::flat(pre, t.to_path_buf(), name, w)
            }
            None => {
                let dir = Path::new(&legacy).parent().unwrap_or(Path::new(".")).to_path_buf();
                Paths::flat(legacy, dir, name, w)
            }
        }
    }
    fn flat(pre: String, dir: PathBuf, name: &str, w: usize) -> Paths {
        Paths {
            main: format!("{pre}.png"),
            annotated: format!("{pre}-annotated.png"),
            visible: format!("{pre}-radiance.png"),
            data: pre.clone(),
            pres_dir: dir.clone(),
            diag_dir: dir,
            name: name.into(),
            w,
            flat_prefix: Some(pre),
        }
    }
    fn create(&self) {
        for d in [&self.pres_dir, &self.diag_dir] { let _ = std::fs::create_dir_all(d); }
        if let Some(p) = Path::new(&self.data).parent() { let _ = std::fs::create_dir_all(p); }
    }
    fn diag(&self, view: &str) -> String {
        match &self.flat_prefix {
            Some(p) => format!("{p}-{view}.png"),
            None => self.diag_dir.join(format!("{}-{view}-{}.png", self.name, self.w)).to_string_lossy().into(),
        }
    }
    fn band(&self, f: &str) -> String {
        match &self.flat_prefix {
            Some(p) => format!("{p}-band-{}.png", f.to_lowercase()),
            None => self.pres_dir.join(format!("{}-band-{}-{}.png", self.name, f.to_lowercase(), self.w)).to_string_lossy().into(),
        }
    }
    fn frame(&self, k: usize) -> String {
        match &self.flat_prefix {
            Some(p) => format!("{p}-frame{k}.png"),
            None => { let d = self.pres_dir.join("frames"); let _ = std::fs::create_dir_all(&d); d.join(format!("{}-{}-frame{k}.png", self.name, self.w)).to_string_lossy().into() }
        }
    }
    fn snapshot(&self, spp: u32) -> String {
        match &self.flat_prefix {
            Some(p) => format!("{p}-spp{spp:05}.png"),
            None => { let d = self.pres_dir.join("progress"); let _ = std::fs::create_dir_all(&d); d.join(format!("{}-{}-spp{spp:05}.png", self.name, self.w)).to_string_lossy().into() }
        }
    }
}

fn write_png(path: &str, w: usize, h: usize, rgba: &[u8], text: &[(&str, String)]) {
    let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::write(path, export::png(w, h, &rgb, text)).expect("write png");
}
