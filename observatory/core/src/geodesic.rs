//! Schwarzschild null geodesics: the Observatory integrator.
//!
//! Units: G = c = M = 1 (the mass enters only through `m`). The photon orbit
//! lies in a plane through the hole; in that plane, with u = 1/r,
//!
//! ```text
//! d²u/dφ² = 3 m u² − u                                       (physically derived)
//! ```
//!
//! with the first integral  (du/dφ)² + u²(1 − 2mu) = 1/b²,  b = L/E. We integrate
//! (u, du/dφ) in φ with an embedded Dormand–Prince 5(4) scheme and error control
//! (numerically approximated, tolerance set per render). Using u instead of r
//! makes escape exact: the ray reaches r = ∞ at the finite angle where u = 0,
//! and the radial direction there is the asymptotic direction of the ray.
//!
//! The camera is a static observer at radius r₀. A ray leaving it at angle α
//! from the outward radial direction, *as measured in the static frame*, has
//! b = r₀ sin α / √(1 − 2m/r₀) and du/dφ = −u √(1 − 2mu) cot α.
//!
//! The disk is the plane y = 0. Within the photon's orbital plane the crossings
//! of that plane happen at angles known in advance (φ_c + kπ), so steps are
//! clipped to land on them exactly rather than detected after the fact.

use crate::math::V3;

#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    pub rtol: f64,
    pub atol: f64,
    /// Largest allowed step in φ (radians).
    pub max_step: f64,
    /// Integration fails after this many accepted + rejected steps.
    pub max_steps: u32,
    /// Fails if the photon winds more than this many times around the hole.
    pub max_windings: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance { rtol: 1e-9, atol: 1e-12, max_step: 0.2, max_steps: 20_000, max_windings: 8.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fate {
    /// Reached r = ∞; `dir` is the asymptotic (sky) direction.
    Escaped,
    /// Crossed r = 2m.
    Captured,
    /// Hit the disk plane inside [r_in, r_out] (opaque, optically thick disk,
    /// at every crossing; `crossings` is the photon order of the image).
    Disk,
    /// Integration failed (step budget, winding budget, step-size underflow).
    Failed,
}

#[derive(Clone, Copy, Debug)]
pub struct Trace {
    pub fate: Fate,
    /// Asymptotic direction (escape) or hit point (disk), world frame.
    pub dir: V3,
    pub hit: V3,
    /// Radius of the disk hit.
    pub r_hit: f64,
    /// Total in-plane angle swept, radians (winding = phi / 2π).
    pub phi: f64,
    /// Asymptotic angle from the camera's radial direction (escape only).
    pub psi_inf: f64,
    /// Impact parameter b = L/E and the unit normal of the orbital plane.
    pub b: f64,
    pub normal: V3,
    /// Number of disk-plane crossings before the fate was decided (photon order).
    pub crossings: u32,
    pub steps: u32,
    /// Largest relative drift of the first integral seen along the ray.
    pub invariant_error: f64,
}

pub struct Disk {
    pub r_in: f64,
    pub r_out: f64,
    /// 0 = no disk; otherwise the disk is opaque at every crossing.
    pub max_order: u32,
}

// Dormand–Prince 5(4) tableau.
const A21: f64 = 1.0 / 5.0;
const A31: f64 = 3.0 / 40.0;
const A32: f64 = 9.0 / 40.0;
const A41: f64 = 44.0 / 45.0;
const A42: f64 = -56.0 / 15.0;
const A43: f64 = 32.0 / 9.0;
const A51: f64 = 19372.0 / 6561.0;
const A52: f64 = -25360.0 / 2187.0;
const A53: f64 = 64448.0 / 6561.0;
const A54: f64 = -212.0 / 729.0;
const A61: f64 = 9017.0 / 3168.0;
const A62: f64 = -355.0 / 33.0;
const A63: f64 = 46732.0 / 5247.0;
const A64: f64 = 49.0 / 176.0;
const A65: f64 = -5103.0 / 18656.0;
const B1: f64 = 35.0 / 384.0;
const B3: f64 = 500.0 / 1113.0;
const B4: f64 = 125.0 / 192.0;
const B5: f64 = -2187.0 / 6784.0;
const B6: f64 = 11.0 / 84.0;
const E1: f64 = 71.0 / 57600.0;
const E3: f64 = -71.0 / 16695.0;
const E4: f64 = 71.0 / 1920.0;
const E5: f64 = -17253.0 / 339200.0;
const E6: f64 = 22.0 / 525.0;
const E7: f64 = -1.0 / 40.0;

#[inline]
fn f(m: f64, y: [f64; 2]) -> [f64; 2] {
    [y[1], 3.0 * m * y[0] * y[0] - y[0]]
}

/// One Dormand–Prince step of size h. Returns (y5, error estimate, f at end).
#[inline(always)]
fn dp_step(m: f64, y: [f64; 2], k1: [f64; 2], h: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let st = |c: [f64; 2]| [y[0] + h * c[0], y[1] + h * c[1]];
    let k2 = f(m, st([A21 * k1[0], A21 * k1[1]]));
    let k3 = f(m, st([A31 * k1[0] + A32 * k2[0], A31 * k1[1] + A32 * k2[1]]));
    let k4 = f(m, st([A41 * k1[0] + A42 * k2[0] + A43 * k3[0], A41 * k1[1] + A42 * k2[1] + A43 * k3[1]]));
    let k5 = f(m, st([
        A51 * k1[0] + A52 * k2[0] + A53 * k3[0] + A54 * k4[0],
        A51 * k1[1] + A52 * k2[1] + A53 * k3[1] + A54 * k4[1],
    ]));
    let k6 = f(m, st([
        A61 * k1[0] + A62 * k2[0] + A63 * k3[0] + A64 * k4[0] + A65 * k5[0],
        A61 * k1[1] + A62 * k2[1] + A63 * k3[1] + A64 * k4[1] + A65 * k5[1],
    ]));
    let y5 = st([
        B1 * k1[0] + B3 * k3[0] + B4 * k4[0] + B5 * k5[0] + B6 * k6[0],
        B1 * k1[1] + B3 * k3[1] + B4 * k4[1] + B5 * k5[1] + B6 * k6[1],
    ]);
    let k7 = f(m, y5);
    let e = |i: usize| h * (E1 * k1[i] + E3 * k3[i] + E4 * k4[i] + E5 * k5[i] + E6 * k6[i] + E7 * k7[i]);
    (y5, [e(0), e(1)], k7)
}

/// Step-size factor (1/err)^(1/5) for a 5th-order method, via exp/ln-free
/// x^(-0.2) = 1/√√(x^0.8): approximated by x^(-1/4) (two square roots). The
/// controller only needs a monotone, roughly scaled factor; accuracy is set
/// by the error test, not by this.
#[inline(always)]
fn step_factor(e: f64) -> f64 {
    1.0 / e.max(1e-12).sqrt().sqrt()
}

/// A disk-plane crossing inside [r_in, r_out].
#[derive(Clone, Copy, Debug)]
pub struct Crossing {
    /// Photon order (1 = first crossing).
    pub order: u32,
    pub r: f64,
    pub hit: V3,
    /// |cos| of the angle between the photon's spatial direction and the disk
    /// normal at the crossing (static-frame, coordinate basis).
    pub mu: f64,
}

/// Integrate a photon leaving a static camera at `cam` (|cam| = r₀) in local
/// static-frame direction `d` (unit vector, world-aligned axes). The disk is
/// opaque at every crossing.
pub fn trace(m: f64, cam: V3, d: V3, disk: &Disk, tol: &Tolerance) -> Trace {
    trace_with(m, cam, d, disk, tol, &mut |_| true)
}

/// Like `trace`, but every crossing inside [r_in, r_out] calls `on_disk`, which
/// returns true to stop the ray there (fate `Disk`) or false to let it continue
/// (a partly transparent disk).
pub fn trace_with(m: f64, cam: V3, d: V3, disk: &Disk, tol: &Tolerance, on_disk: &mut dyn FnMut(&Crossing) -> bool) -> Trace {
    let r0 = cam.len();
    let rhat = cam * (1.0 / r0);
    let cos_a = d.dot(rhat).clamp(-1.0, 1.0);
    let mut tdir = d - rhat * cos_a;
    let sin_a = tdir.len();
    let mut out = Trace {
        fate: Fate::Failed,
        dir: d,
        hit: V3::ZERO,
        r_hit: 0.0,
        phi: 0.0,
        psi_inf: 0.0,
        b: 0.0,
        normal: V3::ZERO,
        crossings: 0,
        steps: 0,
        invariant_error: 0.0,
    };
    let horizon_u = 1.0 / (2.0 * m);
    let u0 = 1.0 / r0;
    if u0 >= horizon_u {
        out.fate = Fate::Captured;
        return out;
    }
    if sin_a < 1e-12 {
        // Radial ray: straight in or out (the disk plane is crossed only if the
        // camera sits in it, which a radial ray cannot leave).
        out.fate = if cos_a > 0.0 { Fate::Escaped } else { Fate::Captured };
        out.dir = rhat * cos_a.signum();
        return out;
    }
    tdir = tdir * (1.0 / sin_a);
    let lapse = (1.0 - 2.0 * m * u0).sqrt();
    out.b = r0 * sin_a / lapse;
    out.normal = rhat.cross(tdir);
    let inv_b2 = 1.0 / (out.b * out.b);

    // Disk-plane crossings: y(φ) ∝ cos φ r̂_y + sin φ t̂_y = 0.
    let (ry, ty) = (rhat.y, tdir.y);
    let in_plane = ry.abs() < 1e-14 && ty.abs() < 1e-14; // orbit lies in the disk plane
    let mut next_cross = if in_plane || disk.max_order == 0 {
        f64::INFINITY
    } else {
        let mut p = (-ry).atan2(ty);
        while p <= 1e-12 {
            p += std::f64::consts::PI;
        }
        p
    };

    let mut y = [u0, -u0 * lapse * cos_a / sin_a];
    let mut phi = 0.0;
    let mut k1 = f(m, y);
    let mut h = tol.max_step.min(0.05);
    let phi_max = tol.max_windings * 2.0 * std::f64::consts::PI;
    let mut steps = 0u32;
    let mut inv_err: f64 = 0.0;

    loop {
        if steps >= tol.max_steps || phi > phi_max {
            out.fate = Fate::Failed;
            break;
        }
        steps += 1;
        let mut hh = h.min(tol.max_step);
        let mut lands_on_cross = false;
        if phi + hh >= next_cross {
            hh = next_cross - phi;
            lands_on_cross = true;
        }
        if hh < 1e-14 {
            out.fate = Fate::Failed;
            break;
        }
        let (yn, err, kn) = dp_step(m, y, k1, hh);
        let sc0 = tol.atol + tol.rtol * y[0].abs().max(yn[0].abs());
        let sc1 = tol.atol + tol.rtol * y[1].abs().max(yn[1].abs());
        let e = ((err[0] / sc0).powi(2) + (err[1] / sc1).powi(2)).sqrt() / std::f64::consts::SQRT_2;
        if !e.is_finite() || e > 1.0 {
            h = hh * (0.9 * step_factor(e)).clamp(0.1, 0.9);
            if !e.is_finite() {
                h = hh * 0.1;
            }
            continue;
        }

        // Accepted step. Check events inside [phi, phi + hh].
        if yn[0] <= 0.0 {
            // Escaped: root of u(φ) = 0 inside this step. Start from the linear
            // interpolant, then Newton on re-integrated substeps (each substep
            // returns u and du/dφ); converges in ≤ 4 iterations to ~1e-15.
            let mut x = (y[0] / (y[0] - yn[0])) * hh;
            for _ in 0..8 {
                let (ym, _, _) = dp_step(m, y, k1, x);
                if ym[1].abs() < 1e-300 { break; }
                let dx = ym[0] / ym[1];
                x = (x - dx).clamp(0.0, hh);
                if dx.abs() < 1e-14 * (1.0 + x) { break; }
            }
            let ps = phi + x;
            out.psi_inf = ps;
            out.phi = ps;
            out.dir = rhat * ps.cos() + tdir * ps.sin();
            out.fate = Fate::Escaped;
            break;
        }
        if yn[0] >= horizon_u {
            out.phi = phi + hh;
            out.fate = Fate::Captured;
            break;
        }
        phi += hh;
        y = yn;
        k1 = kn;
        let inv = (y[1] * y[1] + y[0] * y[0] * (1.0 - 2.0 * m * y[0])) * out.b * out.b - 1.0;
        inv_err = inv_err.max(inv.abs());
        let _ = inv_b2;

        if lands_on_cross {
            out.crossings += 1;
            let r = 1.0 / y[0];
            if r >= disk.r_in && r <= disk.r_out {
                let (rp, tp) = (rhat * phi.cos() + tdir * phi.sin(), tdir * phi.cos() - rhat * phi.sin());
                let v = rp * (-y[1] / (y[0] * y[0])) + tp * r;
                let c = Crossing { order: out.crossings, r, hit: rp * r, mu: (v.y / v.len()).abs() };
                if on_disk(&c) {
                    out.fate = Fate::Disk;
                    out.r_hit = r;
                    out.hit = c.hit;
                    out.phi = phi;
                    break;
                }
            }
            next_cross += std::f64::consts::PI;
        }
        h = hh * (0.9 * step_factor(e)).clamp(0.2, 5.0);
    }
    if out.fate == Fate::Failed {
        out.phi = phi;
    }
    out.steps = steps;
    out.invariant_error = inv_err;
    out
}

/// Asymptotic angle ψ∞(α) for a camera at r₀, used for the sky-footprint
/// Jacobian. Returns None if the ray does not escape.
pub fn psi_inf(m: f64, r0: f64, alpha: f64, tol: &Tolerance) -> Option<f64> {
    let cam = V3::new(r0, 0.0, 0.0);
    let d = V3::new(alpha.cos(), alpha.sin(), 0.0);
    let t = trace(m, cam, d, &Disk { r_in: 0.0, r_out: 0.0, max_order: 0 }, tol);
    (t.fate == Fate::Escaped).then_some(t.psi_inf)
}

/// Critical impact parameter 3√3 m.
pub fn b_crit(m: f64) -> f64 {
    3.0 * 3f64.sqrt() * m
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn ray_with_b(m: f64, r0: f64, b: f64) -> Trace {
        // Inward ray with impact parameter b from radius r0. Built from sin α
        // directly: going through α = π − asin(s) loses s to rounding when s ~ 1e-7.
        let lapse = (1.0 - 2.0 * m / r0).sqrt();
        let s = b * lapse / r0;
        let cam = V3::new(r0, 0.0, 0.0);
        let d = V3::new(-(1.0 - s * s).sqrt(), s, 0.0);
        trace(m, cam, d, &Disk { r_in: 0.0, r_out: 0.0, max_order: 0 }, &Tolerance::default())
    }

    /// Total deflection for a ray from (effectively) infinity: ψ∞ − π measured
    /// against the incoming direction.
    fn deflection(m: f64, b: f64) -> f64 {
        let r0 = 1.0e7;
        let t = ray_with_b(m, r0, b);
        assert_eq!(t.fate, Fate::Escaped, "b={b}");
        // Without gravity the position vector sweeps exactly α = π − asin(sin α)
        // before lining up with the ray direction at infinity.
        let sin_a = b * (1.0 - 2.0 * m / r0).sqrt() / r0;
        t.psi_inf - (PI - sin_a.asin())
    }

    #[test]
    fn first_integral_is_conserved() {
        for &b in &[5.3, 6.0, 8.0, 20.0, 150.0] {
            let t = ray_with_b(1.0, 50.0, b);
            assert!(t.invariant_error < 1e-8, "b={b} drift {}", t.invariant_error);
        }
    }

    #[test]
    fn weak_field_deflection() {
        // Second-order PPN result: δ = 4m/b + (15π/4)(m/b)² + O((m/b)³).
        for &b in &[200.0, 1000.0, 5000.0] {
            let d = deflection(1.0, b);
            let x = 1.0 / b;
            let expect = 4.0 * x + 15.0 * PI / 4.0 * x * x;
            let third = 128.0 / 3.0 * x * x * x;
            assert!((d - expect).abs() < 2.0 * third + 1e-8, "b={b}: {d} vs {expect}");
        }
    }

    #[test]
    fn capture_boundary_is_critical_impact_parameter() {
        let bc = b_crit(1.0);
        assert_eq!(ray_with_b(1.0, 1.0e4, bc * (1.0 - 1e-6)).fate, Fate::Captured);
        assert_eq!(ray_with_b(1.0, 1.0e4, bc * (1.0 + 1e-6)).fate, Fate::Escaped);
    }

    fn deflection_tol(m: f64, b: f64, rtol: f64) -> f64 {
        let r0 = 1.0e7;
        let s = b * (1.0 - 2.0 * m / r0).sqrt() / r0;
        let d = V3::new(-(1.0 - s * s).sqrt(), s, 0.0);
        let tol = Tolerance { rtol, atol: rtol * 1e-3, ..Default::default() };
        let t = trace(m, V3::new(r0, 0.0, 0.0), d, &Disk { r_in: 0.0, r_out: 0.0, max_order: 0 }, &tol);
        assert_eq!(t.fate, Fate::Escaped);
        t.psi_inf - (PI - s.asin())
    }

    #[test]
    fn strong_deflection_limit() {
        // Bozza (2002): δ ≈ −ln(b/b_c − 1) + ln[216(7 − 4√3)] − π near b_c,
        // with corrections of order (b/b_c − 1). Near-critical rays amplify
        // integration error by ~e^φ over their windings, so this needs a tight
        // tolerance (the Photon-ring study preset uses 1e-13).
        let bc = b_crit(1.0);
        for &e in &[1e-4, 1e-6, 1e-8] {
            let d = deflection_tol(1.0, bc * (1.0 + e), 1e-13);
            let expect = -(e.ln()) + (216.0 * (7.0 - 4.0 * 3f64.sqrt())).ln() - PI;
            assert!((d - expect).abs() < 5.0 * e + 1e-5, "e={e}: {d} vs {expect}");
        }
    }

    #[test]
    fn near_critical_rays_converge_with_tolerance() {
        // b = b_c(1 + 1e-8): about three windings. Error falls as rtol tightens.
        let b = b_crit(1.0) * (1.0 + 1e-8);
        let expect = -(1e-8f64.ln()) + (216.0 * (7.0 - 4.0 * 3f64.sqrt())).ln() - PI;
        let errs: Vec<f64> = [1e-9, 1e-11, 1e-13].iter().map(|&t| (deflection_tol(1.0, b, t) - expect).abs()).collect();
        assert!(errs[0] > errs[1] && errs[1] > errs[2], "{errs:?}");
        assert!(errs[2] < 1e-5, "{errs:?}");
    }

    #[test]
    fn tolerance_convergence() {
        let mut loose = Tolerance::default();
        loose.rtol = 1e-6;
        loose.atol = 1e-9;
        let cam = V3::new(30.0, 2.0, 0.0);
        let d = V3::new(-0.97, 0.05, 0.2).norm();
        let disk = Disk { r_in: 0.0, r_out: 0.0, max_order: 0 };
        let a = trace(1.0, cam, d, &disk, &Tolerance::default());
        let b = trace(1.0, cam, d, &disk, &loose);
        assert_eq!(a.fate, b.fate);
        assert!((a.psi_inf - b.psi_inf).abs() < 1e-5);
        assert!(a.dir.dot(b.dir) > 1.0 - 1e-10);
    }

    #[test]
    fn matches_realtime_cartesian_integrator() {
        // The real-time shader integrates x'' = −3 m h² x / r⁵ in Cartesian
        // coordinates (h = |x × v|, |v| = 1) with RK4. Its orbits obey the same
        // u'' = 3mu² − u, but its first integral is 1/h² − 2m/r₀³, so its true
        // impact parameter is b with 1/b² = 1/h² − 2m/r₀³, not h. At equal b
        // the two must agree on the escape direction.
        let m = 1.0;
        for &b in &[5.4, 6.5, 10.0, 30.0] {
            let r0 = 60.0;
            // realtime: coordinate direction with |x × v| = b
            let x0 = V3::new(r0, 0.0, 0.0);
            let hh = 1.0 / (1.0 / (b * b) + 2.0 * m / (r0 * r0 * r0)).sqrt();
            let sin_c = hh / r0;
            let v0 = V3::new(-(1.0 - sin_c * sin_c).sqrt(), sin_c, 0.0);
            let h2 = hh * hh;
            let (mut x, mut v) = (x0, v0);
            let acc = |x: V3| x * (-3.0 * m * h2 / x.len().powi(5));
            for _ in 0..2_000_000 {
                let r = x.len();
                if r > 5.0e4 || r < 2.0 * m { break; }
                let dl = 0.002 * r.max(1.0);
                let a1 = acc(x);
                let (x2, v2) = (x + v * (0.5 * dl), v + a1 * (0.5 * dl));
                let a2 = acc(x2);
                let (x3, v3) = (x + v2 * (0.5 * dl), v + a2 * (0.5 * dl));
                let a3 = acc(x3);
                let (x4, v4) = (x + v3 * dl, v + a3 * dl);
                let a4 = acc(x4);
                x = x + (v + v2 * 2.0 + v3 * 2.0 + v4) * (dl / 6.0);
                v = v + (a1 + a2 * 2.0 + a3 * 2.0 + a4) * (dl / 6.0);
            }
            // remaining far-field bending beyond 5e4 m is < 1e-4 rad
            let rt_dir = v.norm();
            let t = ray_with_b(m, r0, b);
            assert_eq!(t.fate, Fate::Escaped);
            let ang = rt_dir.dot(t.dir).clamp(-1.0, 1.0).acos();
            assert!(ang < 3e-4, "b={b}: angle {ang}");
        }
    }

    #[test]
    fn circular_photon_orbit_is_unstable_equilibrium() {
        // tangential ray from the static frame at r = 3m: b = b_c exactly, u'' = 0
        let tol = Tolerance { max_windings: 3.0, ..Default::default() };
        let t = trace(1.0, V3::new(3.0, 0.0, 0.0), V3::new(0.0, 1.0, 0.0), &Disk { r_in: 0.0, r_out: 0.0, max_order: 0 }, &tol);
        assert_eq!(t.fate, Fate::Failed, "should stay on the photon sphere until the winding budget");
        assert!(t.phi >= 3.0 * 2.0 * PI);
        // perturbed outward by 1e-6: leaves after ~ln(1e6)/(2π)·… windings, then escapes
        let t2 = trace(1.0, V3::new(3.0 * (1.0 + 1e-6), 0.0, 0.0), V3::new(0.0, 1.0, 0.0), &Disk { r_in: 0.0, r_out: 0.0, max_order: 0 }, &Tolerance { max_windings: 8.0, ..Default::default() });
        assert_eq!(t2.fate, Fate::Escaped);
        assert!(t2.phi > 2.0 * PI, "should wind at least once, wound {}", t2.phi / (2.0 * PI));
    }

    #[test]
    fn disk_hit_on_first_crossing() {
        let disk = Disk { r_in: 6.0, r_out: 20.0, max_order: 3 };
        let cam = V3::new(0.0, 3.0, 40.0);
        let d = (V3::new(0.0, 0.0, 10.0) - cam).norm();
        let t = trace(1.0, cam, d, &disk, &Tolerance::default());
        assert_eq!(t.fate, Fate::Disk);
        assert!(t.hit.y.abs() < 1e-9);
        assert!((t.hit.len() - t.r_hit).abs() < 1e-9);
        assert_eq!(t.crossings, 1);
    }
}


#[cfg(test)]
mod bench {
    use super::*;
    #[test]
    #[ignore]
    fn bench_trace() {
        let cam = V3::new(0.0, 6.27, 59.67);
        let disk = Disk { r_in: 6.0, r_out: 26.0, max_order: u32::MAX };
        let tol = Tolerance::default();
        let n = 200_000;
        let t0 = std::time::Instant::now();
        let mut acc = 0.0;
        let mut steps = 0u64;
        for i in 0..n {
            let a = (i as f64 / n as f64 - 0.5) * 0.6;
            let b = ((i * 7919) % n) as f64 / n as f64 * 0.4 - 0.2;
            let d = (V3::new(a, b, -1.0)).norm();
            let t = trace(1.0, cam, d, &disk, &tol);
            acc += t.phi;
            steps += t.steps as u64;
        }
        let dt = t0.elapsed().as_secs_f64();
        println!("trace: {:.2} µs/ray, {:.1} steps/ray, {:.1} ns/step ({acc})", dt / n as f64 * 1e6, steps as f64 / n as f64, dt / steps as f64 * 1e9);
    }
}

#[cfg(test)]
mod bench2 {
    use super::*;
    #[test]
    #[ignore]
    fn bench_dp_step() {
        let n = 5_000_000;
        let t0 = std::time::Instant::now();
        let mut y = [0.02, -0.01];
        let mut k = f(1.0, y);
        for _ in 0..n {
            let (yn, e, kn) = dp_step(1.0, y, k, 1e-4);
            y = [yn[0] + e[0] * 1e-30, yn[1]];
            k = kn;
        }
        println!("dp_step: {:.1} ns ({:?})", t0.elapsed().as_secs_f64() / n as f64 * 1e9, y);
    }
}

/// Validation suite for the observer geometry (distant and local observers).
#[cfg(test)]
mod validation {
    use super::*;
    use std::f64::consts::PI;

    const NO_DISK: Disk = Disk { r_in: 0.0, r_out: 0.0, max_order: 0 };

    /// Camera on +x at r0; ray at angle α from the inward direction (toward the hole), in the x–z plane.
    fn ray(r0: f64, alpha: f64, tol: &Tolerance) -> Trace {
        trace(1.0, V3::new(r0, 0.0, 0.0), V3::new(-alpha.cos(), 0.0, alpha.sin()), &NO_DISK, tol)
    }

    /// Edge of the shadow seen from r0: bisect on capture vs escape.
    fn shadow_edge(r0: f64, tol: &Tolerance) -> f64 {
        let (mut lo, mut hi) = (0.0, PI / 2.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if ray(r0, mid, tol).fate == Fate::Captured { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }

    fn tight() -> Tolerance { Tolerance { rtol: 1e-12, atol: 1e-15, max_windings: 12.0, max_steps: 200_000, ..Default::default() } }

    #[test]
    fn radial_rays_escape_outward_and_fall_inward() {
        let tol = Tolerance::default();
        for r0 in [2.5, 3.0, 30.0, 1e6] {
            let cam = V3::new(r0, 0.0, 0.0);
            let out = trace(1.0, cam, V3::new(1.0, 0.0, 0.0), &NO_DISK, &tol);
            assert_eq!(out.fate, Fate::Escaped, "r0={r0}");
            assert!((out.dir - V3::new(1.0, 0.0, 0.0)).len() < 1e-12);
            assert_eq!(trace(1.0, cam, V3::new(-1.0, 0.0, 0.0), &NO_DISK, &tol).fate, Fate::Captured);
        }
    }

    #[test]
    fn horizon_is_at_two_m() {
        // inside the photon sphere every non-outgoing ray falls through r = 2M,
        // and the integrator reports the capture at u = 1/(2M)
        let tol = Tolerance::default();
        let t = trace(1.0, V3::new(2.5, 0.0, 0.0), V3::new(0.0, 0.0, 1.0), &NO_DISK, &tol);
        assert_eq!(t.fate, Fate::Captured);
        assert_eq!(trace(1.0, V3::new(1.999, 0.0, 0.0), V3::new(1.0, 0.0, 0.0), &NO_DISK, &tol).fate, Fate::Captured);
    }

    #[test]
    fn distant_observer_shadow_radius() {
        // exact: sin α_sh = 3√3 √(1 − 2/r0) / r0 (Synge); far away → 3√3 M / D
        let tol = tight();
        for r0 in [50.0, 1e3, 1e4, 1e6] {
            let edge = shadow_edge(r0, &tol);
            let exact = (3.0 * 3f64.sqrt() * (1.0 - 2.0 / r0).sqrt() / r0).asin();
            assert!((edge / exact - 1.0).abs() < 1e-6, "r0={r0}: {edge} vs {exact}");
            let far = 3.0 * 3f64.sqrt() / r0;
            assert!((edge / far - 1.0).abs() < 1.5 / r0, "r0={r0}: far-field limit");
        }
    }

    #[test]
    fn photon_orbit_impact_parameter_from_the_shadow() {
        // the shadow edge's impact parameter is b_c = 3√3 M at every distance
        let tol = tight();
        for r0 in [10.0, 100.0, 1e4] {
            let a = shadow_edge(r0, &tol);
            let b = r0 * a.sin() / (1.0 - 2.0 / r0).sqrt();
            assert!((b / b_crit(1.0) - 1.0).abs() < 1e-6, "r0={r0}: b={b}");
        }
    }

    #[test]
    fn weak_deflection_vanishes_with_mass() {
        let tol = Tolerance::default();
        let cam = V3::new(1e4, 0.0, 0.0);
        for a in [0.01f64, 0.3, 1.0] {
            let d = V3::new(-a.cos(), 0.0, a.sin());
            let t = trace(1e-12, cam, d, &NO_DISK, &tol);
            assert_eq!(t.fate, Fate::Escaped);
            assert!((t.dir - d).len() < 1e-9, "α={a}: {:?}", t.dir);
        }
    }

    #[test]
    fn tolerance_changes_only_near_critical_rays() {
        // fate topology is stable: loosening rtol from 1e-12 to 1e-7 changes
        // fates only for |b/b_c − 1| < 1e-4, and escape directions elsewhere
        // by less than 1e-5 rad
        let r0: f64 = 30.0;
        let (loose, fine) = (Tolerance { rtol: 1e-7, atol: 1e-10, ..Default::default() }, tight());
        let lapse = (1.0 - 2.0 / r0).sqrt();
        for i in 0..400 {
            let a = 0.05 + 0.45 * i as f64 / 400.0;
            let (p, q) = (ray(r0, a, &loose), ray(r0, a, &fine));
            let b = r0 * a.sin() / lapse;
            let near = (b / b_crit(1.0) - 1.0).abs() < 1e-4;
            if !near {
                assert_eq!(p.fate, q.fate, "α={a}");
                if p.fate == Fate::Escaped { assert!((p.dir - q.dir).len() < 1e-5, "α={a}"); }
            }
        }
    }

    /// Bisect for the boundary of a monotone predicate on α ∈ [lo, hi].
    fn boundary(mut lo: f64, mut hi: f64, inside: &dyn Fn(f64) -> bool) -> f64 {
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if inside(mid) { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }

    #[test]
    fn more_windings_do_not_move_the_shadow() {
        // with a winding budget, near-critical rays on both sides of the edge
        // fail instead of resolving; the failure band [last captured, first
        // escaped] always contains the analytic edge and narrows ~e^{−2π}
        // per extra turn
        let r0: f64 = 30.0;
        let exact = (3.0 * 3f64.sqrt() * (1.0 - 2.0 / r0).sqrt() / r0).asin();
        let mut prev_width = f64::INFINITY;
        for w in [1.0, 2.0, 4.0] {
            let tol = Tolerance { max_windings: w, ..tight() };
            let capt = boundary(0.01, 1.0, &|a| ray(r0, a, &tol).fate == Fate::Captured);
            let esc = boundary(0.01, 1.0, &|a| ray(r0, a, &tol).fate != Fate::Escaped);
            assert!(capt <= exact * (1.0 + 1e-9) && esc >= exact * (1.0 - 1e-9), "w={w}: [{capt}, {esc}] vs {exact}");
            let width = esc - capt;
            assert!(width < prev_width * 0.01 || prev_width.is_infinite(), "w={w}: band {width} after {prev_width}");
            prev_width = width;
        }
    }

    #[test]
    fn einstein_angle_for_a_distant_observer() {
        // the direction straight behind the hole (ψ∞ = π) is imaged on a ring
        // of radius ≈ 2/√r0 (weak field, source at infinity)
        let tol = tight();
        for r0 in [1e4, 1e6] {
            let (mut lo, mut hi) = (shadow_edge(r0, &tol) * 1.01, 0.5);
            // inward-ray convention: ψ is measured from the outward radial
            // direction, so "straight behind" is ψ∞ = π (−x direction)
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                let t = ray(r0, mid, &tol);
                // deflection past the axis: the escape direction has z < 0
                if t.dir.z < 0.0 { lo = mid } else { hi = mid }
            }
            let ae = 0.5 * (lo + hi);
            let weak = 2.0 / r0.sqrt();
            assert!((ae / weak - 1.0).abs() < 0.02, "r0={r0}: {ae} vs {weak}");
        }
    }

    #[test]
    fn parity_flips_at_the_einstein_ring() {
        // tangential magnification sin ψ∞ / sin α changes sign where ψ∞ = π:
        // secondary images (inside the ring) are mirrored
        let tol = tight();
        let r0 = 30.0;
        let psi = |a: f64| { let t = ray(r0, a, &tol); assert_eq!(t.fate, Fate::Escaped); t.dir.z.atan2(t.dir.x) };
        let edge = shadow_edge(r0, &tol);
        // just outside the shadow the ray wraps behind the hole (z < 0 side)
        let inner = psi(edge * 1.2);
        let outer = psi(1.2);
        assert!(inner < 0.0, "inner image is on the far side: ψ = {inner}");
        assert!(outer > 0.0, "outer image is direct: ψ = {outer}");
    }

    #[test]
    fn disk_crossing_order_grows_toward_the_critical_curve() {
        // edge-on observer: as b → b_c from above, the photon order of the
        // first disk hit is non-decreasing and reaches ≥ 3
        let tol = tight();
        // camera 20° above the disk plane; rays in the plane through the camera
        // and +z, so the disk plane is crossed at φ = π/2, 3π/2, ...; a disk with
        // r_in = 6M lets near-critical rays pass inside it and hit later
        let r0: f64 = 100.0;
        let disk = Disk { r_in: 6.0, r_out: 1e9, max_order: u32::MAX };
        let lapse = (1.0 - 2.0 / r0).sqrt();
        let e = 20f64.to_radians();
        let (rh, cam) = (V3::new(e.cos(), e.sin(), 0.0), V3::new(e.cos(), e.sin(), 0.0) * r0);
        let mut prev = 0;
        let mut max = 0;
        for k in 0..70 {
            let b = b_crit(1.0) * (1.0 + 10f64.powf(-0.2 - 0.1 * k as f64));
            let s = b * lapse / r0;
            let d = rh * -(1.0 - s * s).sqrt() + V3::new(0.0, 0.0, s);
            let t = trace(1.0, cam, d, &disk, &tol);
            if t.fate == Fate::Disk {
                assert!(t.crossings >= prev, "b={b}: order {} after {prev}", t.crossings);
                prev = t.crossings;
                max = max.max(t.crossings);
            }
        }
        assert!(max >= 3, "max order {max}");
    }
}
