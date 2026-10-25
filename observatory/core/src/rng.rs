//! Deterministic random numbers.
//!
//! Every stochastic choice in a render is keyed by what it belongs to (seed,
//! pixel, sample index, purpose), never by thread scheduling, so a render is
//! bit-reproducible for a given seed whatever the thread count.

/// SplitMix64 finaliser: a good 64-bit avalanche hash.
#[inline]
pub fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
pub fn hash(parts: &[u64]) -> u64 {
    let mut h = 0x5851_F42D_4C95_7F2D_u64;
    for &p in parts {
        h = mix64(h ^ p);
    }
    h
}

/// PCG32 (XSH-RR) stream.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut r = Rng { state: 0, inc: (stream << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(mix64(seed));
        r.next_u32();
        r
    }

    pub fn keyed(parts: &[u64]) -> Self {
        let h = hash(parts);
        Rng::new(h, mix64(h ^ 0xDA3E_39CB_94B9_5BDB))
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn f64(&mut self) -> f64 {
        let hi = (self.next_u32() as u64) << 21;
        let lo = (self.next_u32() >> 11) as u64;
        ((hi | lo) as f64) * (1.0 / (1u64 << 53) as f64)
    }

    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(1e-300);
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Poisson deviate. Exact inversion for small means, PTRS (Hörmann 1993)
    /// transformed rejection for large ones.
    pub fn poisson(&mut self, mean: f64) -> f64 {
        if !(mean > 0.0) {
            return 0.0;
        }
        if mean < 30.0 {
            let l = (-mean).exp();
            let mut k = 0.0;
            let mut p = self.f64();
            while p > l {
                k += 1.0;
                p *= self.f64();
            }
            return k;
        }
        let slam = mean.sqrt();
        let loglam = mean.ln();
        let b = 0.931 + 2.53 * slam;
        let a = -0.059 + 0.02483 * b;
        let invalpha = 1.1239 + 1.1328 / (b - 3.4);
        let vr = 0.9277 - 3.6224 / (b - 2.0);
        loop {
            let u = self.f64() - 0.5;
            let v = self.f64();
            let us = 0.5 - u.abs();
            let k = ((2.0 * a / us + b) * u + mean + 0.43).floor();
            if us >= 0.07 && v <= vr {
                return k;
            }
            if k < 0.0 || (us < 0.013 && v > us) {
                continue;
            }
            if (v * invalpha / (a / (us * us) + b)).ln() <= -mean + k * loglam - ln_gamma(k + 1.0) {
                return k;
            }
        }
    }
}

/// ln Γ(x) by Lanczos (g = 7, n = 9); ~1e-15 relative accuracy for x > 0.
pub fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_mean_and_variance() {
        for &mean in &[0.3, 4.0, 29.0, 31.0, 400.0, 1.0e5] {
            let mut r = Rng::new(7, 3);
            let n = 200_000;
            let (mut s, mut s2) = (0.0, 0.0);
            for _ in 0..n {
                let k = r.poisson(mean);
                s += k;
                s2 += k * k;
            }
            let m = s / n as f64;
            let var = s2 / n as f64 - m * m;
            let se = (mean / n as f64).sqrt();
            assert!((m - mean).abs() < 5.0 * se, "mean {mean}: got {m}");
            assert!((var / mean - 1.0).abs() < 0.03, "variance {mean}: got {var}");
        }
    }

    #[test]
    fn keyed_streams_are_reproducible() {
        let a: Vec<u32> = { let mut r = Rng::keyed(&[1, 2, 3]); (0..8).map(|_| r.next_u32()).collect() };
        let b: Vec<u32> = { let mut r = Rng::keyed(&[1, 2, 3]); (0..8).map(|_| r.next_u32()).collect() };
        let c: Vec<u32> = { let mut r = Rng::keyed(&[1, 2, 4]); (0..8).map(|_| r.next_u32()).collect() };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
