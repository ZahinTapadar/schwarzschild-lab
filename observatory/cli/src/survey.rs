//! `observatory survey`: observe a generated scene with several instruments.
//!
//!   survey <preset.json> [--tree DIR] [--name NAME] [--grid N] [--spp S]
//!          [--size WxH] [--only id,id] [--no-noise]
//!
//! Writes, under DIR (default `out`):
//!   presentation/NAME-INSTRUMENT-composite.png, NAME-INSTRUMENT-BAND.png,
//!   presentation/NAME-CROSS.png, NAME-montage.png
//!   diagnostics/NAME-state.png (and NAME-wedge.png for cosmic scenes)
//!   data/NAME-INSTRUMENT-BAND.fits (calibrated map, then noise-free map), NAME-INSTRUMENT.json

use observatory_core::export::{self, Card};
use observatory_core::json::Json;
use observatory_core::mw::show::{redshift_wedge, rgba_to_rgb, state_slices};
use observatory_core::mw::survey::{self, Image, Preset};
use std::time::Instant;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn write_png(path: &str, img: &Image, extra: &[(&str, String)]) {
    let mut text: Vec<(&str, String)> = vec![("Class", img.class.to_string()), ("PresentationMode", img.mode.clone()), ("Software", "Schwarzschild Observatory: Multiwavelength Survey".into())];
    text.extend(extra.iter().cloned());
    std::fs::write(path, export::png(img.w, img.h, &rgba_to_rgb(&img.rgba), &text)).expect("write png");
}

pub fn run(args: &[String]) {
    let path = args.get(2).expect("survey preset path");
    let src = std::fs::read_to_string(path).expect("read preset");
    let mut pj = Json::parse(&src).expect("preset JSON");
    if let Some(sz) = arg(args, "--size") {
        let (w, h) = sz.split_once('x').expect("--size WxH");
        let mut v = pj.get("view").cloned().unwrap_or(Json::obj());
        v.set("width", w.parse::<usize>().unwrap()).set("height", h.parse::<usize>().unwrap());
        pj.set("view", v);
    }
    if let Some(s) = arg(args, "--spp") { pj.set("spp", s.parse::<u32>().unwrap()); }
    let p = Preset::from_json(&pj);
    let tree = arg(args, "--tree").unwrap_or_else(|| "out".into());
    let name = arg(args, "--name").unwrap_or_else(|| std::path::Path::new(path).file_stem().unwrap().to_string_lossy().into_owned());
    let grid = arg(args, "--grid").map(|g| g.parse::<usize>().unwrap());
    let noise = !args.iter().any(|a| a == "--no-noise");
    let only: Option<Vec<String>> = arg(args, "--only").map(|s| s.split(',').map(String::from).collect());
    for d in ["presentation", "diagnostics", "data"] { std::fs::create_dir_all(format!("{tree}/{d}")).unwrap(); }

    let t0 = Instant::now();
    let mut sc = survey::build_scene(&p, grid, &mut |s| eprintln!("{s}"));
    let view = survey::view(&p, &sc);
    survey::add_foreground_stars(&p, &mut sc, &view);
    let t_gen = t0.elapsed().as_secs_f64();
    eprintln!("generated in {t_gen:.1} s");
    eprintln!("view {}x{}, {:.3}\"/px", view.w, view.h, view.pix_arcsec());

    let (rgba, w, h) = state_slices(&sc);
    write_png(&format!("{tree}/diagnostics/{name}-state.png"), &Image { stem: "state".into(), rgba, w, h, class: "diagnostic", mode: "diagnostic".into() }, &[]);
    if let Some(wd) = pj.get("wedge") {
        let fov = pj.get("view").and_then(|v| v.num("fov_deg")).unwrap_or(40.0).to_radians();
        let (rgba, w, h) = redshift_wedge(&sc, wd.num("half_dec_deg").unwrap_or(3.0).to_radians(), fov, wd.num("mstar_min").unwrap_or(1e10));
        write_png(&format!("{tree}/diagnostics/{name}-wedge.png"), &Image { stem: "wedge".into(), rgba, w, h, class: "diagnostic", mode: "catalogue_plot".into() }, &[]);
    }

    let mut done = Vec::new();
    let mut timing = Vec::new();
    for id in &p.instruments {
        if let Some(o) = &only { if !o.contains(id) { continue; } }
        let t = Instant::now();
        let Some(obs) = survey::observe_one(&p, &sc, &view, id, p.spp, noise, &mut |s| eprintln!("  {s}")) else { eprintln!("unknown instrument {id}"); continue };
        let secs = t.elapsed().as_secs_f64();
        eprintln!("{id}: {secs:.1} s");
        timing.push((id.clone(), secs));
        let meta = survey::metadata(&p, &sc, &view, &obs, secs);
        std::fs::write(format!("{tree}/data/{name}-{id}.json"), meta.dump()).unwrap();
        for img in survey::instrument_images(&p, &obs) {
            let dir = if img.class == "presentation" { "presentation" } else { "diagnostics" };
            write_png(&format!("{tree}/{dir}/{name}-{}.png", img.stem), &img, &[("Instrument", obs.ins.analog.clone())]);
        }
        for (b, m) in obs.maps.iter().enumerate() {
            let slug: String = m.name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
            let cards = vec![
                Card::Str("TELESCOP", "Schwarzschild Observatory (synthetic)".into(), ""),
                Card::Str("INSTRUME", obs.ins.name.clone(), ""),
                Card::Str("FILTER", m.name.clone(), ""),
                Card::Str("BUNIT", obs.ins.unit().into(), "plane 1: observed, plane 2: noise-free"),
                Card::Float("EXPTIME", obs.ins.exposure_s, "s"),
                Card::Float("PIXSCALE", obs.pix_arcsec, "arcsec per pixel"),
                Card::Float("BEAMFWHM", obs.ins.bands[b].fwhm, "arcsec"),
                Card::Float("NOISE", m.sigma, "rms per pixel, BUNIT"),
                Card::Str("OUTCLASS", if obs.w >= 1024 { "presentation" } else { "validation" }.into(), ""),
                Card::Str("PRESMODE", "single_band".into(), ""),
                Card::Comment(format!("Modelled on {}. Generated scene: {}.", obs.ins.analog, sc.kind)),
            ];
            let fits = export::fits(obs.w, obs.h, &[&m.data, &m.ideal], &cards);
            std::fs::write(format!("{tree}/data/{name}-{id}-{}.fits", slug.trim_matches('-')), fits).unwrap();
        }
        done.push(obs);
    }
    for img in survey::cross_images(&p, &done) {
        write_png(&format!("{tree}/presentation/{name}-{}.png", img.stem), &img, &[]);
    }
    if let Some(img) = survey::montage_image(&p, &done) {
        write_png(&format!("{tree}/presentation/{name}-montage.png", ), &img, &[]);
    }
    eprintln!("total {:.1} s (generation {t_gen:.1} s; {})", t0.elapsed().as_secs_f64(),
        timing.iter().map(|(i, s)| format!("{i} {s:.1} s")).collect::<Vec<_>>().join(", "));
}
