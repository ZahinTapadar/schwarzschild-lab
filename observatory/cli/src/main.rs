//! `observatory`: native renderer and volume generator.
//!
//!   observatory volume [--n 128] [--seed 7] [--out volumes/lensed-nebula.obsvol]
//!   observatory render <scene.json> [--spp 256] [--out out/name] [--threads N]
//!   observatory resume <checkpoint.ckpt> [--spp 1024]
//!   observatory inspect <volume.obsvol> [--out prefix]   (orthographic field previews)
//!   observatory fixtures [--update]                      (preset checks + image regression)
//!   observatory photometry <scene.json>                  (photometric audit of the instrument)

use observatory_core::export;
use observatory_core::volgen::{generate, NebulaParams};
use observatory_core::volume::{Volume, F_ALPHA, NF};
use std::time::Instant;

mod fixtures;
mod photometry;
mod survey;
mod render_cmd;

fn arg<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn arg_str(args: &[String], name: &str, default: &str) -> String {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("volume") => cmd_volume(&args),
        Some("render") | Some("resume") => render_cmd::run(&args),
        Some("inspect") => cmd_inspect(&args),
        Some("fixtures") => fixtures::run(&args),
        Some("photometry") => photometry::run(&args),
        Some("survey") => survey::run(&args),
        _ => {
            eprintln!("usage: observatory volume|render|resume ...  (see source header)");
            std::process::exit(2);
        }
    }
}

fn cmd_volume(args: &[String]) {
    let n: usize = arg(args, "--n", 128);
    let seed: u64 = arg(args, "--seed", 7);
    let out = arg_str(args, "--out", "volumes/lensed-nebula.obsvol");
    let mut p = NebulaParams::lensed_nebula(seed, n);
    p.t_end = arg(args, "--myr", p.t_end);
    eprintln!("generating {n}³ nebula volume (seed {seed}, {:.2} Myr)...", p.t_end);
    let t0 = Instant::now();
    let (vol, st) = generate(&p, &mut |s| eprintln!("{s}"));
    let secs = t0.elapsed().as_secs_f64();
    eprintln!(
        "done in {secs:.1} s: {} steps, ionized volume {:.1}%, equivalent front radius {:.2} pc, mass retained {:.1}%, occupied bricks {:.0}%",
        st.steps, st.ionized_fraction * 100.0, st.mean_front_radius_pc, st.mass_final / st.mass_initial * 100.0, vol.occupied_fraction() * 100.0
    );
    let bytes = vol.to_bytes();
    if let Some(dir) = std::path::Path::new(&out).parent() { let _ = std::fs::create_dir_all(dir); }
    std::fs::write(&out, &bytes).expect("write volume");
    eprintln!("wrote {out} ({:.1} MB)", bytes.len() as f64 / 1e6);
    let preview = out.replace(".obsvol", "-preview.png");
    std::fs::write(&preview, projection_png(&vol)).expect("write preview");
    let slices = out.replace(".obsvol", "-slices.png");
    std::fs::write(&slices, slices_png(&vol)).expect("write slices");
    eprintln!("wrote {preview} and {slices}");
}

/// Mid-plane slices: log n_H (left), ionized fraction (middle), log H α (right).
fn slices_png(v: &Volume) -> Vec<u8> {
    use observatory_core::volume::{F_NH, F_XION};
    let n = v.n;
    let w = 3 * n;
    let mut rgb = vec![0u8; w * n * 3];
    let mut s = [0f32; NF];
    for y in 0..n {
        for x in 0..n {
            v.sample([x as f64, (n - 1 - y) as f64, (n / 2) as f64], &mut s);
            let vals = [
                ((s[F_NH].max(0.01) as f64).log10() + 1.0) / 4.5,
                s[F_XION] as f64,
                ((s[2].max(1e-20) as f64).log10() + 14.0) / 6.0,
            ];
            for (k, val) in vals.iter().enumerate() {
                let o = (y * w + k * n + x) * 3;
                let c = (val.clamp(0.0, 1.0) * 255.0) as u8;
                rgb[o] = c; rgb[o + 1] = c; rgb[o + 2] = c;
            }
        }
    }
    export::png(w, n, &rgb, &[])
}

/// Orthographic emission/absorption projection along z, false colour:
/// [S II] red, H α green, [O III] blue (the "Hubble palette"), dust darkening.
fn projection_png(v: &Volume) -> Vec<u8> {
    let n = v.n;
    let mut img = vec![[0f64; 3]; n * n];
    let mut s = [0f32; NF];
    let ds = v.dx * 3.0857e18;
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0f64; 3];
            let mut tr = 1.0;
            for z in 0..n {
                v.sample([x as f64, (n - 1 - y) as f64, z as f64], &mut s);
                let a = s[F_ALPHA] as f64 * ds;
                let em = [(s[4] + s[5]) as f64, s[2] as f64, s[1] as f64];
                for c in 0..3 { acc[c] += em[c] * ds * tr; }
                tr *= (-a).exp();
            }
            img[y * n + x] = acc;
        }
    }
    let mut mx = [1e-30f64; 3];
    for p in &img { for c in 0..3 { mx[c] = mx[c].max(p[c]); } }
    let mut rgb = vec![0u8; n * n * 3];
    for (i, p) in img.iter().enumerate() {
        for c in 0..3 {
            let t = (p[c] / mx[c] * 3.0).asinh() / 3f64.asinh();
            rgb[i * 3 + c] = (t.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
        }
    }
    export::png(n, n, &rgb, &[("Software", "Schwarzschild Observatory".into())])
}

/// Orthographic previews of the generated volume before any lensing: eight
/// fields projected along the volume's z axis, each with its own legend.
fn cmd_inspect(args: &[String]) {
    use observatory_core::annotate::{stops, with_legend, Key, Legend};
    use observatory_core::sensor::viridis;
    use observatory_core::volume::{F_NH, F_SCA, F_XION};
    let path = args.get(2).expect("volume path");
    let out = arg_str(args, "--out", &path.replace(".obsvol", "-inspect"));
    let v = Volume::from_bytes(&std::fs::read(path).expect("read volume")).expect("volume");
    let n = v.n;
    let ds = v.dx * 3.0857e18;
    let mut s = [0f32; NF];
    // per pixel: [N_H, Σ x n, Σ T n, τ_V, Hα, [O III], [S II], scattered]
    let mut acc = vec![[0f64; 8]; n * n];
    for y in 0..n {
        for x in 0..n {
            let a = &mut acc[y * n + x];
            for z in 0..n {
                v.sample([x as f64, (n - 1 - y) as f64, z as f64], &mut s);
                let nh = s[F_NH] as f64;
                let xi = s[F_XION] as f64;
                a[0] += nh * ds;
                a[1] += xi * nh * ds;
                a[2] += (30.0 + 7970.0 * xi) * nh * ds;
                a[3] += s[F_ALPHA] as f64 * ds;
                a[4] += s[2] as f64 * ds * 1e4;
                a[5] += s[1] as f64 * ds * 1e4;
                a[6] += (s[4] + s[5]) as f64 * ds * 1e4;
                a[7] += s[F_SCA] as f64 * ds * 1e4;
            }
        }
    }
    let up = 3;
    let pw = n * up;
    let panel = |vals: &dyn Fn(&[f64; 8]) -> Option<f64>, lo: f64, hi: f64, log: bool, title: &str, unit: &str| -> (Vec<u8>, usize, usize) {
        let mut rgba = vec![255u8; pw * pw * 4];
        for y in 0..pw {
            for x in 0..pw {
                let c = match vals(&acc[(y / up) * n + x / up]) {
                    Some(v) => {
                        let t = if log { (v.max(1e-300).log10() - lo) / (hi - lo) } else { (v - lo) / (hi - lo) };
                        viridis(t).map(|k| (k * 255.0) as u8)
                    }
                    None => [0, 0, 0],
                };
                rgba[(y * pw + x) * 4..(y * pw + x) * 4 + 3].copy_from_slice(&c);
            }
        }
        let ticks: Vec<(f64, String)> = if log {
            let (a, b) = (lo.ceil() as i64, hi.floor() as i64);
            (a..=b).map(|k| ((k as f64 - lo) / (hi - lo), format!("1E{k}"))).collect()
        } else {
            (0..=4).map(|k| { let v = lo + (hi - lo) * k as f64 / 4.0; (k as f64 / 4.0, if hi <= 1.0 { format!("{v:.2}") } else { format!("{v:.0}") }) }).collect()
        };
        with_legend(&rgba, pw, pw, &Legend { title: title.into(), key: Key::Ramp { stops: stops(&viridis, 64), ticks }, note: unit.into() })
    };
    let dec = |f: &dyn Fn(&[f64; 8]) -> f64| -> (f64, f64) {
        let mx = acc.iter().map(|a| f(a)).fold(0f64, f64::max).max(1e-300).log10().ceil();
        (mx - 4.0, mx)
    };
    let f_nh = |a: &[f64; 8]| a[0];
    let (l0, h0) = dec(&f_nh);
    let (l3, h3) = dec(&|a| a[3]);
    let (l4, h4) = dec(&|a| a[4]);
    let (l5, h5) = dec(&|a| a[5]);
    let (l6, h6) = dec(&|a| a[6]);
    let (l7, h7) = dec(&|a| a[7]);
    let panels = vec![
        panel(&|a| Some(a[0]), l0, h0, true, "COLUMN DENSITY N_H", "CM-2, LOG"),
        panel(&|a| (a[0] > 0.0).then(|| a[1] / a[0]), 0.0, 1.0, false, "IONIZED FRACTION", "N_H WEIGHTED MEAN"),
        panel(&|a| (a[0] > 0.0).then(|| a[2] / a[0]), 0.0, 8000.0, false, "TEMPERATURE (TWO-PHASE)", "K, N_H WEIGHTED; FROM X"),
        panel(&|a| (a[3] > 0.0).then(|| a[3]), l3.max(-3.0), h3.max(-2.0), true, "DUST OPTICAL DEPTH TAU_V", "THROUGH THE BOX, LOG"),
        panel(&|a| (a[4] > 0.0).then(|| a[4]), l4, h4, true, "H-ALPHA (NO DUST)", "PHOT S-1 M-2 SR-1, LOG"),
        panel(&|a| (a[5] > 0.0).then(|| a[5]), l5, h5, true, "[O III] 5007 (NO DUST)", "PHOT S-1 M-2 SR-1, LOG"),
        panel(&|a| (a[6] > 0.0).then(|| a[6]), l6, h6, true, "[S II] 6716+6731 (NO DUST)", "PHOT S-1 M-2 SR-1, LOG"),
        panel(&|a| (a[7] > 0.0).then(|| a[7]), l7, h7, true, "SCATTERED STARLIGHT", "RELATIVE, LOG"),
    ];
    // 4 × 2 montage
    let (tw, th) = (panels[0].1, panels[0].2);
    let gap = 6;
    let (mw, mh) = (4 * tw + 3 * gap, 2 * th + gap);
    let mut rgb = vec![7u8; mw * mh * 3];
    for (i, (img, w, h)) in panels.iter().enumerate() {
        let (ox, oy) = ((i % 4) * (tw + gap), (i / 4) * (th + gap));
        for y in 0..*h { for x in 0..*w {
            let src = (y * w + x) * 4;
            let dst = ((oy + y) * mw + ox + x) * 3;
            rgb[dst..dst + 3].copy_from_slice(&img[src..src + 3]);
        }}
    }
    // grid-scale noise index: mean |f − mean of 6 neighbours| / mean f inside
    // the ionized gas, per field (a smooth field gives ≲ 0.05; odd–even
    // decoupling or ray-sampling noise shows up as ≳ 0.2)
    let names = [(F_NH, "n_H"), (F_XION, "x"), (2usize, "H-alpha"), (1usize, "[O III]"), (4usize, "[S II]")];
    let mut f = [0f32; NF];
    let mut fb = [0f32; NF];
    for (k, name) in names {
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for z in 1..n - 1 { for y in 1..n - 1 { for x in 1..n - 1 {
            v.sample([x as f64, y as f64, z as f64], &mut f);
            if f[F_XION] < 0.5 { continue; }
            let c = f[k] as f64;
            let mut m = 0.0;
            for (dx, dy, dz) in [(1i64, 0i64, 0i64), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                v.sample([(x as i64 + dx) as f64, (y as i64 + dy) as f64, (z as i64 + dz) as f64], &mut fb);
                m += fb[k] as f64 / 6.0;
            }
            num += (c - m).abs();
            den += c.abs();
        }}}
        eprintln!("grid-scale noise index {name:>8}: {:.3}", if den > 0.0 { num / den } else { 0.0 });
    }
    let file = format!("{out}.png");
    std::fs::write(&file, export::png(mw, mh, &rgb, &[("Class", "diagnostic".into()), ("View", "orthographic volume fields, unlensed".into())])).expect("write");
    eprintln!("wrote {file} ({} voxels of {:.3} pc; projections along z)", n * n * n, v.dx);
}
