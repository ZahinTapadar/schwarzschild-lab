//! Schwarzschild geodesic physics engine (Rust -> WebAssembly).
//!
//! Units are geometric (G = c = 1). Lengths and times are measured in the same
//! units as the black-hole mass `M`; the event horizon sits at r = 2M.
//!
//! Massive test bodies obey the exact Schwarzschild timelike geodesic equation,
//! written in Cartesian-like coordinates and parameterised by proper time tau:
//!
//!     d²x/dtau² = -M x / r³ · (1 + 3 L² / r²),      L = |x × dx/dtau|
//!
//! Its radial part reproduces  r'' = -M/r² + L²/r³ - 3ML²/r⁴  exactly, so
//! orbits precess, the ISCO lies at 6M, the photon sphere at 3M, and bodies
//! plunge once they lose enough angular momentum. The conserved energy
//!
//!     E² = (dr/dtau)² + (1 - 2M/r)(1 + L²/r²)
//!
//! gives the clock rate dt/dtau = E / (1 - 2M/r), letting the simulation run
//! in the far-away observer's time (bodies redden and freeze at the horizon)
//! or in each body's own proper time.
//!
//! Massive bodies additionally attract each other (and debris) with a softened
//! weak-field Newtonian term; debris has zero mass so tidal streams are pure
//! geodesic deviation.

use std::ptr;

const KIND_BODY: u32 = 0;
const KIND_DEBRIS: u32 = 1;
pub const STRIDE: usize = 8;

#[derive(Clone, Copy)]
struct Particle {
    x: [f64; 3],
    v: [f64; 3], // dx/dtau
    m: f64,
    radius: f64,
    tau: f64, // accumulated proper time
    kind: u32,
    id: u32,
    color: u32,
    alive: bool,
    rate: f64, // dtau/dt of last step
}

struct Sim {
    m: f64,
    ps: Vec<Particle>,
    out: Vec<f32>,
    proper_time: bool,
    nbody: bool,
    accrete: bool,
    absorbed: u32,
    absorbed_mass: f64,
    escape_r: f64,
    rng: u64,
    next_id: u32,
    precision: f64,
}

static mut SIM: *mut Sim = ptr::null_mut();

fn sim() -> &'static mut Sim {
    unsafe {
        if SIM.is_null() {
            SIM = Box::into_raw(Box::new(Sim {
                m: 1.0,
                ps: Vec::with_capacity(4096),
                out: Vec::with_capacity(4096 * STRIDE),
                proper_time: false,
                nbody: true,
                accrete: true,
                absorbed: 0,
                absorbed_mass: 0.0,
                escape_r: 4000.0,
                rng: 0x9E3779B97F4A7C15,
                next_id: 1,
                precision: 1.0,
            }));
        }
        &mut *SIM
    }
}

#[inline]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline]
fn add(a: [f64; 3], b: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

impl Sim {
    fn rand(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

/// Snapshot of a massive body used as a weak-field source during one step.
#[derive(Clone, Copy)]
struct Source {
    x: [f64; 3],
    m: f64,
    soft2: f64,
    id: u32,
}

#[inline]
fn accel(m: f64, x: [f64; 3], v: [f64; 3], sources: &[Source], self_id: u32) -> [f64; 3] {
    let r2 = dot(x, x);
    let r = r2.sqrt();
    let h = cross(x, v);
    let l2 = dot(h, h);
    let f = -m / (r2 * r) * (1.0 + 3.0 * l2 / r2);
    let mut a = [x[0] * f, x[1] * f, x[2] * f];
    for s in sources {
        if s.id == self_id {
            continue;
        }
        let d = [x[0] - s.x[0], x[1] - s.x[1], x[2] - s.x[2]];
        let d2 = dot(d, d) + s.soft2;
        let k = -s.m / (d2 * d2.sqrt());
        a = add(a, d, k);
    }
    a
}

/// Classical RK4 on (x, v) with proper-time step `h`.
#[inline]
fn rk4(m: f64, x: &mut [f64; 3], v: &mut [f64; 3], h: f64, src: &[Source], id: u32) {
    let a1 = accel(m, *x, *v, src, id);
    let x2 = add(*x, *v, 0.5 * h);
    let v2 = add(*v, a1, 0.5 * h);
    let a2 = accel(m, x2, v2, src, id);
    let x3 = add(*x, v2, 0.5 * h);
    let v3 = add(*v, a2, 0.5 * h);
    let a3 = accel(m, x3, v3, src, id);
    let x4 = add(*x, v3, h);
    let v4 = add(*v, a3, h);
    let a4 = accel(m, x4, v4, src, id);
    for i in 0..3 {
        x[i] += h / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]);
        v[i] += h / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]);
    }
}

/// Killing energy per unit mass for a geodesic at (x, v).
#[inline]
fn energy(m: f64, x: [f64; 3], v: [f64; 3]) -> f64 {
    let r = dot(x, x).sqrt();
    let rdot = dot(x, v) / r;
    let h = cross(x, v);
    let l2 = dot(h, h);
    let e2 = rdot * rdot + (1.0 - 2.0 * m / r) * (1.0 + l2 / (r * r));
    e2.max(1e-12).sqrt()
}

// ---------------------------------------------------------------- exports --

#[no_mangle]
pub extern "C" fn reset() {
    let s = sim();
    s.ps.clear();
    s.absorbed = 0;
    s.absorbed_mass = 0.0;
}

#[no_mangle]
pub extern "C" fn set_mass(m: f64) {
    sim().m = m.max(1e-4);
}

#[no_mangle]
pub extern "C" fn get_mass() -> f64 {
    sim().m
}

#[no_mangle]
pub extern "C" fn set_flags(proper_time: u32, nbody: u32, accrete: u32) {
    let s = sim();
    s.proper_time = proper_time != 0;
    s.nbody = nbody != 0;
    s.accrete = accrete != 0;
}

#[no_mangle]
pub extern "C" fn set_precision(p: f64) {
    sim().precision = p.clamp(0.1, 10.0);
}

#[no_mangle]
pub extern "C" fn absorbed_count() -> u32 {
    sim().absorbed
}

#[no_mangle]
pub extern "C" fn absorbed_mass() -> f64 {
    sim().absorbed_mass
}

/// Spawn a single particle. Velocity is the *coordinate-frame* velocity dx/dt
/// as a fraction of c; it is converted to dx/dtau with the Lorentz factor of a
/// local static observer so that |v| < 1 is always physical.
#[no_mangle]
pub extern "C" fn add_particle(
    x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64, mass: f64, radius: f64, kind: u32, color: u32,
) -> u32 {
    let s = sim();
    let mut vv = [vx, vy, vz];
    let sp2 = dot(vv, vv);
    if sp2 >= 0.995 {
        let k = (0.995 / sp2).sqrt();
        vv = [vx * k, vy * k, vz * k];
    }
    let gamma = 1.0 / (1.0 - dot(vv, vv)).sqrt();
    let id = s.next_id;
    s.next_id = s.next_id.wrapping_add(1).max(1);
    s.ps.push(Particle {
        x: [x, y, z],
        v: [vv[0] * gamma, vv[1] * gamma, vv[2] * gamma],
        m: mass.max(0.0),
        radius,
        tau: 0.0,
        kind,
        id,
        color,
        alive: true,
        rate: 1.0,
    });
    id
}

/// Exact circular-orbit speed (in proper-time velocity dx/dtau) at radius r.
#[no_mangle]
pub extern "C" fn circular_v_tau(r: f64) -> f64 {
    let m = sim().m;
    if r <= 3.0 * m {
        return 0.0;
    }
    (m / (r - 3.0 * m)).sqrt()
}

/// Spawn a particle with velocity given directly as dx/dtau (used for exact
/// circular / elliptical orbits).
#[no_mangle]
pub extern "C" fn add_particle_tau(
    x: f64, y: f64, z: f64, ux: f64, uy: f64, uz: f64, mass: f64, radius: f64, kind: u32, color: u32,
) -> u32 {
    let id = add_particle(x, y, z, 0.0, 0.0, 0.0, mass, radius, kind, color);
    let s = sim();
    let p = s.ps.last_mut().unwrap();
    p.v = [ux, uy, uz];
    id
}

/// A cloud of massless debris (e.g. a star about to be tidally disrupted):
/// `n` particles uniformly filling a sphere of radius `r` about the centre,
/// all sharing velocity (ux,uy,uz) = dx/dtau plus a small internal dispersion.
#[no_mangle]
pub extern "C" fn add_cloud(
    cx: f64, cy: f64, cz: f64, ux: f64, uy: f64, uz: f64, r: f64, n: u32, disp: f64, color: u32,
) {
    let s = sim();
    for _ in 0..n {
        let (mut px, mut py, mut pz);
        loop {
            px = s.rand() * 2.0 - 1.0;
            py = s.rand() * 2.0 - 1.0;
            pz = s.rand() * 2.0 - 1.0;
            if px * px + py * py + pz * pz <= 1.0 {
                break;
            }
        }
        let dv = [
            (s.rand() * 2.0 - 1.0) * disp,
            (s.rand() * 2.0 - 1.0) * disp,
            (s.rand() * 2.0 - 1.0) * disp,
        ];
        let id = s.next_id;
        s.next_id = s.next_id.wrapping_add(1).max(1);
        s.ps.push(Particle {
            x: [cx + px * r, cy + py * r, cz + pz * r],
            v: [ux + dv[0], uy + dv[1], uz + dv[2]],
            m: 0.0,
            radius: 0.0,
            tau: 0.0,
            kind: KIND_DEBRIS,
            id,
            color,
            alive: true,
            rate: 1.0,
        });
    }
}

/// Remove a particle by id (used by the UI).
#[no_mangle]
pub extern "C" fn remove_particle(id: u32) {
    let s = sim();
    s.ps.retain(|p| p.id != id);
}

/// Advance the simulation by `dt` units of the chosen clock (coordinate time t
/// of a distant observer, or each body's proper time tau).
#[no_mangle]
pub extern "C" fn step(dt: f64) {
    let s = sim();
    let m = s.m;
    let horizon = 2.0 * m;
    let capture_lapse = if s.proper_time { 0.0 } else { 0.003 };

    let sources: Vec<Source> = if s.nbody {
        s.ps
            .iter()
            .filter(|p| p.alive && p.kind == KIND_BODY && p.m > 0.0)
            .map(|p| Source {
                x: p.x,
                m: p.m,
                soft2: (p.radius * p.radius).max(0.01),
                id: p.id,
            })
            .collect()
    } else {
        Vec::new()
    };

    let prec = s.precision;
    let mut absorbed_mass = 0.0;
    let mut absorbed = 0u32;

    for p in s.ps.iter_mut() {
        if !p.alive {
            continue;
        }
        let mut t_left = dt;
        let mut iters = 0;
        while t_left > 1e-12 && iters < 4000 {
            iters += 1;
            let r = dot(p.x, p.x).sqrt();
            let lapse = 1.0 - horizon / r;
            if r <= horizon * 1.0005 || lapse <= capture_lapse {
                p.alive = false;
                break;
            }
            let e = energy(m, p.x, p.v);
            // dtau/dt for the distant observer's clock
            let rate = if s.proper_time { 1.0 } else { lapse / e };
            p.rate = lapse / e;
            let speed = dot(p.v, p.v).sqrt() + 1e-9;
            // step limited by orbital timescale and by distance to the horizon
            let h_orbit = 0.02 / prec * r * (r / m).sqrt();
            let h_cross = 0.02 / prec * r / speed;
            let h_hor = 0.05 / prec * (r - horizon) / speed;
            let h_max = h_orbit.min(h_cross).min(h_hor.max(1e-4 * m));
            let h = h_max.min(t_left * rate);
            rk4(m, &mut p.x, &mut p.v, h, &sources, p.id);
            p.tau += h;
            t_left -= h / rate;
        }
        let r = dot(p.x, p.x).sqrt();
        if r > s.escape_r {
            p.alive = false;
        } else if !p.alive || r <= horizon {
            p.alive = false;
            absorbed += 1;
            absorbed_mass += p.m;
        }
    }

    s.absorbed += absorbed;
    s.absorbed_mass += absorbed_mass;
    if s.accrete && absorbed_mass > 0.0 {
        s.m += absorbed_mass;
    }
    s.ps.retain(|p| p.alive);
}

/// Pack render data: [x, y, z, radius, kind, clock-rate dtau/dt, color, id].
#[no_mangle]
pub extern "C" fn pack() -> u32 {
    let s = sim();
    s.out.clear();
    let m = s.m;
    for p in &s.ps {
        let r = dot(p.x, p.x).sqrt();
        let lapse = (1.0 - 2.0 * m / r).max(0.0);
        let rate = if p.kind == KIND_DEBRIS { lapse.sqrt() } else { p.rate };
        s.out.extend_from_slice(&[
            p.x[0] as f32,
            p.x[1] as f32,
            p.x[2] as f32,
            p.radius as f32,
            p.kind as f32,
            rate as f32,
            p.color as f32,
            p.id as f32,
        ]);
    }
    s.ps.len() as u32
}

#[no_mangle]
pub extern "C" fn out_ptr() -> *const f32 {
    sim().out.as_ptr()
}

/// Detailed info for a single body: writes [r, speed_local, dtau/dt, tau, E, L]
/// into the output buffer start (call `pack` afterwards to restore).
#[no_mangle]
pub extern "C" fn body_info(id: u32, outp: *mut f64) -> u32 {
    let s = sim();
    let m = s.m;
    if let Some(p) = s.ps.iter().find(|p| p.id == id) {
        let r = dot(p.x, p.x).sqrt();
        let e = energy(m, p.x, p.v);
        let h = cross(p.x, p.v);
        let l = dot(h, h).sqrt();
        let lapse = 1.0 - 2.0 * m / r;
        // speed measured by a local static observer: gamma_local = E / sqrt(lapse)
        let gl = e / lapse.max(1e-9).sqrt();
        let vloc = (1.0 - 1.0 / (gl * gl)).max(0.0).sqrt();
        unsafe {
            *outp.add(0) = r;
            *outp.add(1) = vloc;
            *outp.add(2) = lapse / e;
            *outp.add(3) = p.tau;
            *outp.add(4) = e;
            *outp.add(5) = l;
        }
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn scratch_ptr() -> *mut f64 {
    static mut SCRATCH: [f64; 16] = [0.0; 16];
    ptr::addr_of_mut!(SCRATCH) as *mut f64
}
