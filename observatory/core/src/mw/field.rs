//! Grid tools for the generators: 3D FFTs, Gaussian random fields with a
//! chosen power spectrum, cloud-in-cell deposits, and column-density sweeps.

use crate::rng::Rng;
use crate::sensor::fft;

/// A cube of n³ cells, x fastest.
#[derive(Clone)]
pub struct Cube {
    pub n: usize,
    pub v: Vec<f32>,
}

impl Cube {
    pub fn new(n: usize) -> Cube { Cube { n, v: vec![0.0; n * n * n] } }
    #[inline]
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize { (z * self.n + y) * self.n + x }
    /// Periodic trilinear sample at cell coordinates (cell centres at integers).
    pub fn sample_periodic(&self, p: [f64; 3]) -> f64 {
        let n = self.n as i64;
        let f = p.map(|c| c.floor());
        let t = [p[0] - f[0], p[1] - f[1], p[2] - f[2]];
        let mut s = 0.0;
        for c in 0..8 {
            let o = [(c & 1) as i64, ((c >> 1) & 1) as i64, ((c >> 2) & 1) as i64];
            let ix = [0, 1, 2].map(|k| ((f[k] as i64 + o[k]).rem_euclid(n)) as usize);
            let w = (0..3).map(|k| if o[k] == 1 { t[k] } else { 1.0 - t[k] }).product::<f64>();
            s += w * self.v[self.idx(ix[0], ix[1], ix[2])] as f64;
        }
        s
    }
}

/// In-place 3D FFT of a complex cube (forward: e^(−ikx); inverse divides by n³).
pub fn fft3(re: &mut [f64], im: &mut [f64], n: usize, inverse: bool) {
    let mut r = vec![0.0; n];
    let mut i = vec![0.0; n];
    for axis in 0..3 {
        let stride = [1, n, n * n][axis];
        for a in 0..n {
            for b in 0..n {
                let base = match axis { 0 => (a * n + b) * n, 1 => a * n * n + b, _ => a * n + b };
                for k in 0..n { r[k] = re[base + k * stride]; i[k] = im[base + k * stride]; }
                fft(&mut r, &mut i, inverse);
                for k in 0..n { re[base + k * stride] = r[k]; im[base + k * stride] = i[k]; }
            }
        }
    }
}

/// Integer wavenumber of FFT index k.
#[inline]
pub fn kint(k: usize, n: usize) -> f64 { if k <= n / 2 { k as f64 } else { k as f64 - n as f64 } }

/// White Gaussian noise of unit variance per cell, transformed to Fourier space.
pub fn white_k(n: usize, seed: u64, stream: u64) -> (Vec<f64>, Vec<f64>) {
    let mut rng = Rng::new(seed, stream);
    let mut re: Vec<f64> = (0..n * n * n).map(|_| rng.normal()).collect();
    let mut im = vec![0.0; n * n * n];
    fft3(&mut re, &mut im, n, false);
    (re, im)
}

/// A Gaussian random field with power spectrum P(k) (k in rad per unit
/// length, box side `l`), returned in real space. The discrete amplitude is
/// √(P(k)/Δx³), so that the cell variance is Σ_k P(k)/L³ (see the
/// supercluster notes).
pub fn gaussian_field(n: usize, l: f64, seed: u64, stream: u64, p: &dyn Fn(f64) -> f64) -> Vec<f64> {
    let (mut re, mut im) = white_k(n, seed, stream);
    let dx3 = (l / n as f64).powi(3);
    let kf = 2.0 * std::f64::consts::PI / l;
    for z in 0..n { for y in 0..n { for x in 0..n {
        let i = (z * n + y) * n + x;
        let k = kf * (kint(x, n).powi(2) + kint(y, n).powi(2) + kint(z, n).powi(2)).sqrt();
        let a = if k == 0.0 { 0.0 } else { (p(k) / dx3).sqrt() };
        re[i] *= a;
        im[i] *= a;
    }}}
    fft3(&mut re, &mut im, n, true);
    re
}

/// Measured spherically averaged power spectrum of a real field (for tests),
/// in bins of integer |k|: returns (k, P(k)).
pub fn measure_power(f: &[f64], n: usize, l: f64) -> Vec<(f64, f64)> {
    let mut re = f.to_vec();
    let mut im = vec![0.0; f.len()];
    fft3(&mut re, &mut im, n, false);
    let kf = 2.0 * std::f64::consts::PI / l;
    let nb = n / 2;
    let mut s = vec![0.0; nb];
    let mut c = vec![0usize; nb];
    let v = l.powi(3);
    let n3 = (n * n * n) as f64;
    for z in 0..n { for y in 0..n { for x in 0..n {
        let kk = (kint(x, n).powi(2) + kint(y, n).powi(2) + kint(z, n).powi(2)).sqrt();
        let b = kk.round() as usize;
        if b == 0 || b >= nb { continue; }
        let i = (z * n + y) * n + x;
        s[b] += (re[i] * re[i] + im[i] * im[i]) * v / (n3 * n3);
        c[b] += 1;
    }}}
    (1..nb).filter(|&b| c[b] > 0).map(|b| (b as f64 * kf, s[b] / c[b] as f64)).collect()
}

/// Cloud-in-cell deposit of unit-mass particles at cell coordinates onto a
/// periodic grid; returns counts per cell.
pub fn cic(n: usize, pos: &[[f32; 3]]) -> Vec<f32> {
    let mut g = vec![0f32; n * n * n];
    let ni = n as i64;
    for p in pos {
        let f = p.map(|c| (c as f64 - 0.5).floor());
        let t = [p[0] as f64 - 0.5 - f[0], p[1] as f64 - 0.5 - f[1], p[2] as f64 - 0.5 - f[2]];
        for c in 0..8 {
            let o = [(c & 1) as i64, ((c >> 1) & 1) as i64, ((c >> 2) & 1) as i64];
            let ix = [0, 1, 2].map(|k| ((f[k] as i64 + o[k]).rem_euclid(ni)) as usize);
            let w = (0..3).map(|k| if o[k] == 1 { t[k] } else { 1.0 - t[k] }).product::<f64>();
            g[(ix[2] * n + ix[1]) * n + ix[0]] += w as f32;
        }
    }
    g
}

/// Separable Gaussian smoothing of a periodic cube, σ in cells.
pub fn smooth_periodic(f: &[f32], n: usize, sigma: f64) -> Vec<f32> {
    let r = (3.0 * sigma).ceil() as i64;
    let k: Vec<f64> = (-r..=r).map(|d| (-(d * d) as f64 / (2.0 * sigma * sigma)).exp()).collect();
    let ks: f64 = k.iter().sum();
    let mut a = f.to_vec();
    for axis in 0..3 {
        let stride = [1usize, n, n * n][axis];
        let src = a.clone();
        for i in 0..src.len() {
            let c = ((i / stride) % n) as i64;
            let base = i - (c as usize) * stride;
            let mut s = 0.0;
            for (j, d) in (-r..=r).enumerate() {
                let cc = (c + d).rem_euclid(n as i64) as usize;
                s += k[j] * src[base + cc * stride] as f64;
            }
            a[i] = (s / ks) as f32;
        }
    }
    a
}

/// Minimum column density from a set of source cells: for every cell, the
/// least ∫ n ds over 26-connected paths to any cell with `seed[i]`, by
/// repeated forward and backward chamfer sweeps (*numerically approximated*:
/// a 3-4-5-style metric, within ~4% of Euclidean). `n_h` in cm⁻³, cell size
/// `dx_cm`. Returns cm⁻².
pub fn column_from(seed: &[bool], n_h: &[f32], n: usize, dx_cm: f64) -> Vec<f32> {
    let mut c: Vec<f32> = seed.iter().map(|&s| if s { 0.0 } else { f32::INFINITY }).collect();
    let offs: Vec<(i64, i64, i64, f64)> = {
        let mut v = Vec::new();
        for dz in -1i64..=1 { for dy in -1i64..=1 { for dx in -1i64..=1 {
            if (dz, dy, dx) < (0, 0, 0) {
                v.push((dx, dy, dz, ((dx * dx + dy * dy + dz * dz) as f64).sqrt()));
            }
        }}}
        v
    };
    let ni = n as i64;
    for _ in 0..3 {
        for dir in [1i64, -1] {
            let range: Box<dyn Iterator<Item = usize>> = if dir > 0 { Box::new(0..n * n * n) } else { Box::new((0..n * n * n).rev()) };
            for i in range {
                if seed[i] { continue; }
                let (x, y, z) = ((i % n) as i64, ((i / n) % n) as i64, (i / (n * n)) as i64);
                let mut best = c[i];
                for &(ox, oy, oz, d) in &offs {
                    let (xx, yy, zz) = (x + ox * dir, y + oy * dir, z + oz * dir);
                    if xx < 0 || yy < 0 || zz < 0 || xx >= ni || yy >= ni || zz >= ni { continue; }
                    let j = ((zz * ni + yy) * ni + xx) as usize;
                    let v = c[j] + (0.5 * (n_h[i] + n_h[j]) as f64 * d * dx_cm) as f32;
                    if v < best { best = v; }
                }
                c[i] = best;
            }
        }
    }
    c
}

/// Euclidean-ish distance (in cells) to the nearest cell with `seed`, via the
/// same sweeps with unit density.
pub fn distance_to(seed: &[bool], n: usize) -> Vec<f32> {
    let ones = vec![1f32; seed.len()];
    column_from(seed, &ones, n, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_field_has_the_requested_power() {
        let (n, l) = (32, 100.0);
        let p = |k: f64| 50.0 * k.powf(-2.0);
        let f = gaussian_field(n, l, 3, 1, &p);
        let m = measure_power(&f, n, l);
        // average ratio over the well-sampled shells
        let r: Vec<f64> = m.iter().skip(3).take(8).map(|&(k, pk)| pk / p(k)).collect();
        let mean = r.iter().sum::<f64>() / r.len() as f64;
        assert!((mean - 1.0).abs() < 0.12, "P ratio {mean}");
    }

    #[test]
    fn column_sweep_matches_a_straight_slab() {
        let n = 16;
        let mut seed = vec![false; n * n * n];
        for z in 0..n { for y in 0..n { seed[(z * n + y) * n] = true; } }
        let nh = vec![10f32; n * n * n];
        let c = column_from(&seed, &nh, n, 1.0);
        let i = (5 * n + 7) * n + 9;
        assert!((c[i] - 90.0).abs() < 1e-3, "{}", c[i]);
    }

    #[test]
    fn cic_conserves_mass() {
        let pos = vec![[0.3f32, 5.7, 7.99], [15.9, 0.1, 3.5], [8.0, 8.0, 8.0]];
        let g = cic(16, &pos);
        let s: f32 = g.iter().sum();
        assert!((s - 3.0).abs() < 1e-5);
    }
}
