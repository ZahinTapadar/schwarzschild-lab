//! Pillars: a molecular cloud sculpted by an ionising cluster.
//!
//! What it computes, on a uniform grid (*numerically approximated* unless noted):
//!
//! 1. **Initial cloud.** A lognormal turbulent density field from a Gaussian
//!    random field with P(k) ∝ (k² + k₀²)^(−s/2) (s = 3.7, close to the
//!    Kolmogorov-like slopes measured in molecular clouds), filling the box
//!    below a rough upper surface, plus a few dense Plummer cores
//!    (*statistically synthesized*).
//! 2. **Photoevaporation front.** An ionisation front facing a source of Q
//!    ionising photons per second is shielded by its own photoevaporation
//!    flow: recombinations in the flow balance the flux when the flow's base
//!    density is n_IF = √(F/(α_B ℓ)) (Bertoldi 1989), and the front then eats
//!    atoms at n_IF c_s per unit area. Over an age t it has removed a column
//!    N_erode = n_IF c_s t. A cell is ionised if the cloud's column between it
//!    and the source is smaller than that (*physically derived* scaling). Dense
//!    cores stop the front and shadow the gas behind them, which is how pillars
//!    pointing at the cluster form.
//! 3. **Diffuse field.** Recombinations to the ground state make ionising
//!    photons in all directions; they erode a further column (a fixed fraction
//!    of N_erode) from every surface, which narrows the pillars' sides.
//! 4. **Compressed layer.** The neutral gas just beyond a lit front is
//!    compressed by the front's shock (a factor up to 3 within ~4e21 cm⁻²;
//!    *not mass-conserving*).
//! 5. **Ionised gas.** A tenuous cavity plus photoevaporation flows off every
//!    surface, with density n_IF (ℓ/(ℓ + d))² at distance d from the surface;
//!    shadowed surfaces see only the diffuse field. A hot (7 × 10⁶ K),
//!    wind-shocked plasma fills the cavity away from the surfaces (*prescribed*).
//! 6. **Dust and starlight.** FUV intensity U from the cluster through the dust
//!    column, plus a diffuse part from the nearest surface; dust and PAHs are
//!    partly destroyed in ionised gas.
//! 7. **Stars.** Young stellar objects in the dense heads, and field stars
//!    along the line of sight from the Observatory's Galactic model.

use super::field::{column_from, distance_to, gaussian_field};
use super::phys::*;
use super::*;
use crate::json::Json;
use crate::math::V3;
use crate::rng::Rng;

pub struct PillarParams {
    pub seed: u64,
    pub n: usize,
    pub box_pc: f64,
    pub distance_pc: f64,
    pub gal_l: f64,
    pub gal_b: f64,
    pub a_v_fg: f64,
    /// Cluster position relative to the box centre (pc; x right, y up, z away from the observer).
    pub cluster_pc: [f64; 3],
    pub q_ion: f64,
    pub l_bol: f64,
    pub t_eff: f64,
    pub n_mean: f64,
    pub sigma_ln: f64,
    pub slope: f64,
    pub outer_pc: f64,
    pub top: f64,
    pub cores: usize,
    pub core_density: f64,
    pub age_myr: f64,
    pub flow_pc: f64,
    pub cs_kms: f64,
    pub diffuse: f64,
    pub n_cavity: f64,
    pub ysos: usize,
    pub star_limit_v: f64,
    pub n_hot: f64,
    pub roughness: f64,
}

impl PillarParams {
    pub fn from_json(j: &Json) -> PillarParams {
        let g = |k: &str, d: f64| j.num(k).unwrap_or(d);
        let cl = j.arr("cluster_pc").map(|a| [0, 1, 2].map(|i| a.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0))).unwrap_or([1.6, 5.4, -1.0]);
        PillarParams {
            seed: g("seed", 16.0) as u64,
            // FFT-based turbulence needs a power-of-two grid
            n: (g("grid", 128.0) as usize).next_power_of_two(),
            box_pc: g("box_pc", 6.4),
            distance_pc: g("distance_pc", 1740.0),
            gal_l: g("gal_l_deg", 17.0),
            gal_b: g("gal_b_deg", 0.8),
            a_v_fg: g("foreground_av", 2.6),
            cluster_pc: cl,
            q_ion: g("cluster_q", 2.0e50),
            l_bol: g("cluster_lbol", 8.0e5),
            t_eff: g("cluster_teff", 40000.0),
            n_mean: g("cloud_density", 2500.0),
            sigma_ln: g("turbulence", 1.3),
            slope: g("turbulence_slope", 3.7),
            outer_pc: g("turbulence_outer_pc", 2.5),
            top: g("cloud_top", 0.25),
            cores: g("cores", 7.0) as usize,
            core_density: g("core_density", 1.2e5),
            age_myr: g("age_myr", 1.5),
            flow_pc: g("flow_scale_pc", 0.12),
            cs_kms: g("flow_speed_kms", 11.0),
            diffuse: g("diffuse_fraction", 0.12),
            n_cavity: g("cavity_density", 40.0),
            ysos: g("ysos", 34.0) as usize,
            star_limit_v: g("star_limit_v", 29.0),
            n_hot: g("hot_density", 0.4),
            roughness: g("surface_roughness", 0.12),
        }
    }

    pub fn to_json(&self) -> Json {
        let mut j = Json::obj();
        j.set("generator", "pillars").set("seed", self.seed).set("grid", self.n).set("box_pc", self.box_pc)
            .set("distance_pc", self.distance_pc).set("cluster_q", self.q_ion).set("cluster_lbol", self.l_bol)
            .set("cloud_density", self.n_mean).set("turbulence", self.sigma_ln).set("cores", self.cores)
            .set("core_density", self.core_density).set("age_myr", self.age_myr).set("flow_scale_pc", self.flow_pc)
            .set("diffuse_fraction", self.diffuse).set("ysos", self.ysos);
        j
    }
}

/// Generator statistics, reported by the CLI and recorded in metadata.
pub struct PillarStats {
    pub ionised_volume: f64,
    pub neutral_mass_msun: f64,
    pub erode_column_center: f64,
    pub n_stars: usize,
    pub n_ysos: usize,
}

/// Galactic-frame direction (x along rotation, y north, z to the centre).
pub fn gal_dir(l_deg: f64, b_deg: f64) -> V3 {
    let (l, b) = (l_deg.to_radians(), b_deg.to_radians());
    V3::new(b.cos() * l.sin(), b.sin(), b.cos() * l.cos())
}

pub fn generate(p: &PillarParams, log: &mut dyn FnMut(&str)) -> (MwScene, PillarStats) {
    let n = p.n;
    let n3 = n * n * n;
    let dx_pc = p.box_pc / n as f64;
    let dx = dx_pc * PC;
    let center = [0.0, 0.0, p.distance_pc * PC];
    let mut vol = MwVolume::new(n, dx, center);
    vol.fg_av = p.a_v_fg;
    let h = 0.5 * (n as f64 - 1.0);
    let cpos = |i: usize| -> [f64; 3] {
        let (x, y, z) = (i % n, (i / n) % n, i / (n * n));
        [(x as f64 - h) * dx_pc, (y as f64 - h) * dx_pc, (z as f64 - h) * dx_pc]
    };
    let src = p.cluster_pc;
    let r_src = |c: [f64; 3]| ((c[0] - src[0]).powi(2) + (c[1] - src[1]).powi(2) + (c[2] - src[2]).powi(2)).sqrt().max(0.05);

    // 1. initial cloud ---------------------------------------------------
    log("pillars: turbulent cloud");
    let k0 = 2.0 * std::f64::consts::PI / p.outer_pc;
    let pk = |k: f64| (k * k + k0 * k0).powf(-p.slope / 2.0);
    let mut g = gaussian_field(n, p.box_pc, p.seed, 1, &pk);
    normalise(&mut g);
    let mut gs = gaussian_field(n, p.box_pc, p.seed, 2, &|k: f64| (k * k + k0 * k0 * 0.25).powf(-2.6));
    normalise(&mut gs);
    let mut rng = Rng::keyed(&[p.seed, 0x91_11A5]);
    let cores: Vec<([f64; 3], f64, f64)> = (0..p.cores)
        .map(|k| {
            let fx = (k as f64 + 0.5) / p.cores as f64 - 0.5;
            let x = fx * 0.78 * p.box_pc + 0.06 * p.box_pc * (rng.f64() - 0.5);
            let y = (-0.02 + 0.22 * rng.f64()) * p.box_pc;
            let z = (rng.f64() - 0.5) * 0.3 * p.box_pc;
            let a = (0.22 + 0.2 * rng.f64()) * p.box_pc / 6.4;
            (([x, y, z]), a, p.core_density * (0.5 + rng.f64()))
        })
        .collect();
    let edge = |c: f64| -> f64 { let m = 0.5 * p.box_pc - c.abs(); (m / (6.0 * dx_pc)).clamp(0.0, 1.0) };
    let mut n0 = vec![0f32; n3];
    for i in 0..n3 {
        let c = cpos(i);
        let ytop = p.top * p.box_pc * 0.5 + p.roughness * p.box_pc * gs[i];
        let env = 1.0 / (1.0 + ((c[1] - ytop) / 0.12).exp());
        let mut v = p.n_mean * (p.sigma_ln * g[i] - 0.5 * p.sigma_ln * p.sigma_ln).exp() * env;
        for &(cc, a, nc) in &cores {
            let r2 = (c[0] - cc[0]).powi(2) + (c[1] - cc[1]).powi(2) + (c[2] - cc[2]).powi(2);
            v += nc * (1.0 + r2 / (a * a)).powf(-2.0) * (0.6 * p.sigma_ln * g[i]).exp() * 0.7;
        }
        v *= edge(c[0]) * edge(c[2]);
        n0[i] = v as f32;
    }

    // 2. photoevaporation front ------------------------------------------
    log("pillars: photoevaporation front");
    let flow = p.flow_pc * PC;
    let t_age = p.age_myr * 3.156e13;
    let cs = p.cs_kms * 1e5;
    let flux = |r_pc: f64| p.q_ion / (4.0 * std::f64::consts::PI * (r_pc * PC).powi(2));
    let n_if = |f: f64| (f / (ALPHA_B * flow)).sqrt();
    let n_erode = |r_pc: f64| n_if(flux(r_pc)) * cs * t_age;
    // column from each cell to the source through a density field
    let to_source = |dens: &[f32], i: usize| -> f64 {
        let c = cpos(i);
        let d = [src[0] - c[0], src[1] - c[1], src[2] - c[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let u = [d[0] / len, d[1] / len, d[2] / len];
        let ds = 0.5 * dx_pc;
        let half = 0.5 * p.box_pc;
        let mut s = 0.5 * ds;
        let mut col = 0.0;
        while s < len {
            let q = [c[0] + u[0] * s, c[1] + u[1] * s, c[2] + u[2] * s];
            if q[0].abs() >= half || q[1].abs() >= half || q[2].abs() >= half { break; }
            let ix = [0, 1, 2].map(|k| (((q[k] + half) / dx_pc) as usize).min(n - 1));
            col += dens[(ix[2] * n + ix[1]) * n + ix[0]] as f64;
            s += ds;
        }
        col * ds * PC
    };
    let col0: Vec<f32> = par_map(n3, |i| to_source(&n0, i) as f32);
    let mut ion: Vec<bool> = (0..n3).map(|i| (col0[i] as f64) < n_erode(r_src(cpos(i)))).collect();

    // 3. diffuse-field erosion
    let colw = column_from(&ion, &n0, n, dx);
    for i in 0..n3 {
        if !ion[i] && (colw[i] as f64) < p.diffuse * n_erode(r_src(cpos(i))) { ion[i] = true; }
    }

    // 4. compressed layer behind lit fronts
    let mut nn = n0.clone();
    for i in 0..n3 {
        if ion[i] { continue; }
        let depth = (col0[i] as f64 - n_erode(r_src(cpos(i)))).max(0.0);
        nn[i] = (nn[i] as f64 * (1.0 + 2.0 * (-depth / 4e21).exp())) as f32;
    }
    let neutral: Vec<f32> = (0..n3).map(|i| if ion[i] { 0.0 } else { nn[i] }).collect();

    // 5. ionised gas -------------------------------------------------------
    log("pillars: ionised flows, starlight and dust");
    let is_neutral: Vec<bool> = ion.iter().map(|&b| !b).collect();
    let dist = distance_to(&is_neutral, n); // cells to the nearest neutral cell
    let col_surf = column_from(&ion, &neutral, n, dx); // neutral column to the nearest ionised cell
    let nsrc_neutral: Vec<f32> = par_map(n3, |i| to_source(&neutral, i) as f32);
    let mut dens = vec![0f32; n3];
    let mut lit_flux = vec![0f32; n3];
    for i in 0..n3 {
        let c = cpos(i);
        let r = r_src(c);
        if ion[i] {
            // direct flux through the neutral column (FUV-like e-folding at 1e21 cm⁻²,
            // so a shadow edge is a gradient rather than a voxel step), plus the diffuse field
            let lit = (-(nsrc_neutral[i] as f64) / 1e21).exp();
            let f = flux(r) * (lit + p.diffuse * (1.0 - lit));
            let d = dist[i] as f64 * dx;
            let nf = n_if(f) * (flow / (flow + d)).powi(2);
            let cav = p.n_cavity * (0.5 * g[i] - 0.125).exp();
            dens[i] = (nf + cav) as f32;
            lit_flux[i] = f as f32;
        } else {
            dens[i] = neutral[i];
        }
    }
    // electron density of the warm ionised gas: the ionised cells' density,
    // zero in neutral cells (the renderer's trilinear sampling makes the edge a
    // one-cell ramp instead of mixing neutral density into the plasma)
    let ne_field: Vec<f32> = (0..n3).map(|i| if ion[i] { 1.1 * dens[i] } else { 0.0 }).collect();
    // one [1, 2, 1] pass removes the voxel-scale steps of the binary front
    // (the Observatory's nebula uses the same reconstruction filter)
    let mut dl: Vec<f32> = dens.iter().map(|&v| v.max(1e-3).ln()).collect();
    smooth1(&mut dl, n);
    for i in 0..n3 { dens[i] = dl[i].exp(); }
    smooth1(&mut lit_flux, n);
    // dust column (neutral + half-depleted ionised) to the cluster, for FUV and scattered light
    let dust_h: Vec<f32> = (0..n3).map(|i| if ion[i] { 0.5 * dens[i] } else { dens[i] }).collect();
    let nsrc_dust: Vec<f32> = par_map(n3, |i| to_source(&dust_h, i) as f32);

    // FUV: L_FUV ≈ 0.45 L_bol for an O-star cluster; U = F_FUV / 1.8e-3 erg cm⁻² s⁻¹
    let g0 = |r_pc: f64| 0.45 * p.l_bol * L_SUN / (4.0 * std::f64::consts::PI * (r_pc * PC).powi(2)) / 1.8e-3;
    let sigma_fuv = 2.6 * SIGMA_V / 1.086 * 1.086; // A_FUV/A_V ≈ 2.6
    let mut xion = vec![0f32; n3];
    for i in 0..n3 { xion[i] = if ion[i] { 1.0 } else { 0.0 }; }
    smooth1(&mut xion, n);
    let vf = &mut vol.f;
    let mut neutral_mass = 0.0;
    let mut n_ion = 0usize;
    for i in 0..n3 {
        let c = cpos(i);
        let r = r_src(c);
        let u_dir = g0(r) * (-sigma_fuv * nsrc_dust[i] as f64).exp();
        let lit_surface = (nsrc_neutral[i] as f64 - col_surf[i] as f64) < 3e21; // surface toward the cluster
        let u_surf = g0(r) * if lit_surface { 1.0 } else { 0.05 };
        let u = u_dir + 0.05 * g0(r) * (-sigma_fuv * col_surf[i] as f64).exp() + 3.0;
        vf[F_NH][i] = dens[i];
        vf[F_XION][i] = xion[i];
        vf[F_U][i] = u as f32;
        vf[F_NSRC][i] = nsrc_dust[i];
        vf[F_NE][i] = ne_field[i];
        if ion[i] {
            n_ion += 1;
            vf[F_TGAS][i] = 8500.0;
            vf[F_FDUST][i] = 0.5;
            vf[F_FPAH][i] = 0.02;
            let d_pc = dist[i] as f64 * dx_pc;
            let hot = ((d_pc - 0.35) / 0.45).clamp(0.0, 1.0);
            vf[F_NHOT][i] = (p.n_hot * hot * hot * (3.0 - 2.0 * hot)) as f32;
            vf[F_THOT][i] = 7.0e6;
            vf[F_LOGU][i] = (lit_flux[i] as f64 / (dens[i] as f64 * C)).max(1e-12).log10() as f32;
            vf[F_FRONT][i] = (-(dist[i] as f64 - 1.0).max(0.0) / 1.5).exp() as f32;
        } else {
            neutral_mass += dens[i] as f64 * dx * dx * dx * 1.4 * M_P / M_SUN_G;
            let cs_ = col_surf[i] as f64;
            vf[F_TGAS][i] = (20.0 + 280.0 * (-cs_ / 1e21).exp()) as f32;
            vf[F_FDUST][i] = 1.0;
            vf[F_FPAH][i] = 1.0;
            vf[F_COLSURF][i] = col_surf[i];
            vf[F_USURF][i] = u_surf as f32;
        }
    }

    // 7. stars -------------------------------------------------------------
    log("pillars: young stellar objects and field stars");
    let mut sources = Vec::new();
    let to_world = |c: [f64; 3]| [c[0] * PC + center[0], c[1] * PC + center[1], c[2] * PC + center[2]];
    // embedded young stars, weighted to dense, shielded gas near the heads
    let cand: Vec<(usize, f64)> = (0..n3)
        .filter(|&i| !ion[i] && dens[i] > 2e4 && (2e21..8e22).contains(&(col_surf[i] as f64)))
        .map(|i| (i, (dens[i] as f64).powf(1.5)))
        .collect();
    let wsum: f64 = cand.iter().map(|c| c.1).sum();
    let mut rng = Rng::keyed(&[p.seed, 0x7_5050]);
    let mut n_ysos = 0;
    if wsum > 0.0 {
        for _ in 0..p.ysos {
            let mut u = rng.f64() * wsum;
            let mut pick = cand[0].0;
            for &(i, w) in &cand { u -= w; if u <= 0.0 { pick = i; break; } }
            let c = cpos(pick);
            let jit = [0, 1, 2].map(|k| c[k] + (rng.f64() - 0.5) * dx_pc);
            let m = 0.2 * (1.0 - rng.f64() * 0.93).powf(-1.0 / 1.3); // Salpeter-like 0.2–~3 M☉
            let m = m.min(4.0);
            let class1 = rng.f64() < 0.4;
            let l = 1.5 * m.powf(1.8) * if class1 { 3.0 } else { 1.0 };
            let t = 3400.0 + 1600.0 * (m / 2.0).sqrt().min(1.5);
            let lx = 10f64.powf(29.6 + 0.5 * rng.normal()) * m.powf(0.8);
            sources.push(Source {
                pos: to_world(jit),
                sed: Sed::Yso { l, t, f_env: if class1 { 0.8 } else { 0.3 }, t_env: if class1 { 260.0 } else { 900.0 }, lx, kt_x: 2.0 },
                size: 0.0, a_v_fg: p.a_v_fg, z: 0.0, d_l: 0.0, flags: 1,
            });
            n_ysos += 1;
        }
    }
    // field stars: the Observatory's Galactic model along this sightline
    let half_ang = 0.5 * p.box_pc / p.distance_pc;
    let stars = field_stars(p.seed, p.gal_l, p.gal_b, [0.0, 0.0, 1.0], 2.0 * half_ang, 2.0 * half_ang, p.star_limit_v, 120_000);
    let count = stars.len();
    sources.extend(stars);
    let stats = PillarStats {
        ionised_volume: n_ion as f64 / n3 as f64,
        neutral_mass_msun: neutral_mass,
        erode_column_center: n_erode(r_src([0.0, 0.0, 0.0])),
        n_stars: count,
        n_ysos,
    };
    log(&format!(
        "pillars: ionised volume {:.0}%, neutral mass {:.0} M☉, erosion column at centre {:.2e} cm⁻², {} YSOs, {} field stars",
        stats.ionised_volume * 100.0, stats.neutral_mass_msun, stats.erode_column_center, n_ysos, count
    ));
    let mut meta = p.to_json();
    meta.set("ionised_volume", stats.ionised_volume).set("neutral_mass_msun", stats.neutral_mass_msun).set("field_stars", count).set("ysos_placed", n_ysos);
    let cl = to_world(src);
    let scene = MwScene { kind: "pillars".into(), vol, sources, halos: vec![], particles: None, cosmo: None, cluster: Some((cl, p.q_ion, p.l_bol, p.t_eff)), meta, seed: p.seed };
    (scene, stats)
}

fn normalise(g: &mut [f64]) {
    let m = g.iter().sum::<f64>() / g.len() as f64;
    let v = g.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / g.len() as f64;
    let s = v.sqrt().max(1e-30);
    for x in g.iter_mut() { *x = (*x - m) / s; }
}

/// One [1, 2, 1]/4 pass on each axis (softens the binary ionisation mask to a
/// one-cell-wide front).
fn smooth1(f: &mut [f32], n: usize) {
    for axis in 0..3 {
        let off = [1usize, n, n * n][axis];
        let src = f.to_vec();
        for i in 0..src.len() {
            let c = (i / off) % n;
            let a = if c > 0 { src[i - off] } else { src[i] };
            let b = if c + 1 < n { src[i + off] } else { src[i] };
            f[i] = 0.25 * a + 0.5 * src[i] + 0.25 * b;
        }
    }
}

/// Field stars from the Observatory's Galactic model toward (l, b), drawn in a
/// gnomonic field of `w` × `h` radians centred on direction `look`, to
/// magnitude `v_lim` (*statistically synthesized*: the same luminosity
/// function, density model and clumpy dust as the Deep Field).
pub fn field_stars(seed: u64, l: f64, b: f64, look: [f64; 3], w: f64, h: f64, v_lim: f64, max: usize) -> Vec<Source> {
    let gd = gal_dir(l, b);
    let los = crate::sky::los(seed, gd, 1.0, 1.0);
    let omega = w * h;
    let grid: Vec<f64> = (0..=((v_lim + 3.0) / 0.05) as usize).map(|k| -3.0 + 0.05 * k as f64).collect();
    let cum: Vec<f64> = grid.iter().map(|&m| los.count_brighter(m) * omega).collect();
    let total = *cum.last().unwrap();
    let mut rng = Rng::keyed(&[seed, 0xF1E1D]);
    let count = (rng.poisson(total) as usize).min(max);
    let f = V3::new(look[0], look[1], look[2]).norm();
    let r = V3::new(0.0, 1.0, 0.0).cross(f).norm();
    let u = f.cross(r).norm();
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let x = rng.f64() * total;
        let k = cum.partition_point(|&c| c < x).min(grid.len() - 1);
        let m = grid[k] - 0.05 * rng.f64();
        let (t, a_v, d_kpc) = crate::sky::draw_star(&los, m, &mut rng);
        let d = d_kpc * 1e3 * PC;
        // V flux → L via the blackbody at T (V zero point 3640 Jy at 550 nm)
        let fnu_v = 3640.0 * JY * 10f64.powf(-0.4 * (m - a_v));
        let lum = 4.0 * std::f64::consts::PI * d * d * fnu_v / star_lnu(C / 0.55e-4, 1.0, t);
        let (ax, ay) = ((rng.f64() - 0.5) * w, (rng.f64() - 0.5) * h);
        let dir = (f + r * ax + u * ay).norm();
        out.push(Source { pos: [dir.x * d, dir.y * d, dir.z * d], sed: Sed::Star { l: lum, t }, size: 0.0, a_v_fg: a_v, z: 0.0, d_l: 0.0, flags: 0 });
    }
    out
}
