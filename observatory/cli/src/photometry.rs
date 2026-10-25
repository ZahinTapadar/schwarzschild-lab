//! `observatory photometry <scene.json>`: a photometric audit of a scene's
//! instrument. For each filter it reports the collecting area, pixel solid
//! angle, the unresolved sky background per pixel at the field centre, dark
//! current and read noise, and the peak-pixel signal and S/N of point sources
//! of given magnitudes for the scene's exposure plan, so a dark or noisy image
//! can be traced to physical counts rather than guessed at.

use observatory_core::json::Json;
use observatory_core::render::Ctx;
use observatory_core::scene::Scene;
use observatory_core::sensor::{self, Instrument};
use observatory_core::spectrum::{self, NB};
use std::path::Path;

pub fn run(args: &[String]) {
    let path = args.get(2).expect("scene path");
    let j = Json::parse(&std::fs::read_to_string(path).expect("read scene")).expect("scene JSON");
    let sc = Scene::from_json(&j);
    let (vol, _) = crate::render_cmd::load_volume(&sc, Path::new(path).parent().unwrap_or(Path::new(".")));
    let ctx = Ctx::new(sc.clone(), vol, &mut |s| eprintln!("{s}"));
    let ins = Instrument::from_ctx(&ctx);
    let omega = sc.pixel_scale().powi(2);
    let area = sc.collecting_area();
    let mut dif = [0f32; NB];
    if let Some(sky) = &ctx.sky { sky.diffuse_at(ctx.cam.3, &mut dif); }
    let arcsec = sc.pixel_scale().to_degrees() * 3600.0;
    println!("{}: {:.3}″ pixels (Ω = {:.3e} sr), aperture {:.3} m (A = {:.4e} m²), read noise {} e⁻, dark {} e⁻/s, full well {} e⁻",
        sc.name, arcsec, omega, sc.aperture(), area, sc.read_noise_e, sc.dark_e_per_s, sc.full_well_e);
    let plan = sc.exposure_list();
    for (fi, f) in ins.filters.iter().enumerate() {
        let sky_rate: f64 = (0..NB).map(|c| f.w[c] * dif[c] as f64).sum();
        let fwhm = sensor::psf_core_fwhm_px(&ins, fi);
        let sigma = fwhm / 2.3548;
        // fraction of a point source's light in its brightest pixel (Gaussian
        // core sampled by square pixels, minus the halo)
        let erf = |x: f64| { let t = 1.0 / (1.0 + 0.3275911 * x.abs()); let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp(); y * x.signum() };
        let peak = erf(0.5 / (std::f64::consts::SQRT_2 * sigma)).powi(2) * (1.0 - sc.halo_fraction);
        println!("\n[{}] sky background {:.3e} e⁻/s/pixel (unresolved starlight + DGL at the field centre); PSF core FWHM {:.2} px = {:.2}″; peak-pixel fraction {:.2}",
            f.name, sky_rate, fwhm, fwhm * arcsec, peak);
        println!("  {:>6} {:>12} {:>14} {:>12}   per exposure group: peak S, sky B, dark D, read²  →  S/N", "mag", "e⁻/s total", "e⁻/s peak px", "");
        for mag in [14.0, 18.0, 20.0, 21.0, 22.0] {
            let mut fl = [0f32; NB];
            spectrum::star_flux(mag, 5800.0, ctx.g_sky, 0.0, &mut fl);
            let rate: f64 = (0..NB).map(|c| f.w[c] / omega * fl[c] as f64).sum();
            let mut line = format!("  {mag:>6.1} {rate:>12.3e} {:>14.3e}             ", rate * peak);
            for e in &plan {
                let g = e.exposure_s * 10f64.powf(-e.neutral_density);
                let n = e.count as f64;
                let s = rate * peak * g * n;
                let b = sky_rate * g * n;
                let d = sc.dark_e_per_s * e.exposure_s * n;
                let r2 = sc.read_noise_e.powi(2) * n;
                let snr = s / (s + b + d + r2).sqrt();
                line += &format!(" [{}×{:.0}s: S {:.0}, B {:.1}, D {:.1}, R² {:.0} → {:.1}]", e.count, e.exposure_s, s, b, d, r2, snr);
            }
            println!("{line}");
        }
    }
    println!("\nThe sky term B is the only background: the observer is far from the Solar System (no zodiacal light) and in space (no airglow).");
}
