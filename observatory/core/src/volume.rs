//! Volumetric deep-sky objects: storage, sampling and the cache format.
//!
//! A volume is a cube of `n³` voxels of side `dx` parsecs holding the fields
//! the renderer needs, all per voxel:
//!
//! * `j[line]`: line emissivity, photons s⁻¹ cm⁻³ sr⁻¹ (H β, [O III], H α,
//!   [N II], [S II] ×2, in `spectrum::LINES` order);
//! * `alpha_v`: dust extinction coefficient at V, cm⁻¹ (scaled to other
//!   wavelengths with the CCM curve);
//! * `sca`: dust-scattered continuum at 550 nm, photons s⁻¹ cm⁻³ sr⁻¹ nm⁻¹;
//! * `n_h`, `x_ion`: gas density (cm⁻³) and ionized fraction, for diagnostics.
//!
//! Storage is sparse: 8³ bricks, with empty bricks (no emission, no dust)
//! dropped, and an occupancy table used for empty-space skipping. The cache
//! file is a little-endian binary with a JSON header carrying the generator
//! parameters and seed, so a render can record exactly which volume it used.

use crate::json::Json;
use crate::spectrum::NL;

pub const BRICK: usize = 8;
pub const NF: usize = NL + 4; // lines, alpha_v, sca, n_h, x_ion
pub const F_ALPHA: usize = NL;
pub const F_SCA: usize = NL + 1;
pub const F_NH: usize = NL + 2;
pub const F_XION: usize = NL + 3;

pub struct Volume {
    pub n: usize,
    /// Voxel size in parsecs.
    pub dx: f64,
    pub nb: usize,
    /// For each brick: index into `bricks`, or u32::MAX if empty.
    pub table: Vec<u32>,
    /// Brick data: [brick][voxel z,y,x][field], f32.
    pub bricks: Vec<f32>,
    /// Per-brick maxima of total line emissivity and alpha (for skipping).
    pub brick_max: Vec<[f32; 2]>,
    pub meta: Json,
    /// Spectral shape temperature of the scattered (illuminating) starlight.
    pub sca_temp: f64,
}

impl Volume {
    /// Build sparse storage from dense fields `dense[field][z][y][x]`.
    pub fn from_dense(n: usize, dx: f64, dense: &[Vec<f32>; NF], meta: Json, sca_temp: f64) -> Volume {
        assert!(n % BRICK == 0);
        let nb = n / BRICK;
        let mut table = vec![u32::MAX; nb * nb * nb];
        let mut bricks = Vec::new();
        let mut brick_max = Vec::new();
        let j_peak = (0..dense[0].len()).map(|i| (0..NL).map(|f| dense[f][i]).sum::<f32>() + dense[F_SCA][i] * 50.0).fold(0f32, f32::max);
        let j_floor = j_peak * 1e-5;
        for bz in 0..nb {
            for by in 0..nb {
                for bx in 0..nb {
                    let (mut ej, mut ea) = (0f32, 0f32);
                    for z in 0..BRICK { for y in 0..BRICK { for x in 0..BRICK {
                        let i = ((bz * BRICK + z) * n + by * BRICK + y) * n + bx * BRICK + x;
                        let jt: f32 = (0..NL).map(|f| dense[f][i]).sum::<f32>() + dense[F_SCA][i] * 50.0;
                        ej = ej.max(jt);
                        ea = ea.max(dense[F_ALPHA][i]);
                    }}}
                    // Empty if its emission is below 1e-5 of the peak and it is
                    // optically thin across a brick (τ_V < 3e-3, i.e. < 0.3%
                    // attenuation): such bricks are dropped and skipped.
                    let brick_cm = (BRICK as f64 * dx * 3.0857e18) as f32;
                    if ej > j_floor || ea * brick_cm > 3e-3 {
                        table[(bz * nb + by) * nb + bx] = brick_max.len() as u32;
                        brick_max.push([ej, ea]);
                        for z in 0..BRICK { for y in 0..BRICK { for x in 0..BRICK {
                            let i = ((bz * BRICK + z) * n + by * BRICK + y) * n + bx * BRICK + x;
                            for f in 0..NF { bricks.push(dense[f][i]); }
                        }}}
                    }
                }
            }
        }
        Volume { n, dx, nb, table, bricks, brick_max, meta, sca_temp }
    }

    #[inline]
    fn voxel(&self, x: i64, y: i64, z: i64, out: &mut [f32; NF]) -> bool {
        let n = self.n as i64;
        if x < 0 || y < 0 || z < 0 || x >= n || y >= n || z >= n {
            return false;
        }
        let (bx, by, bz) = (x as usize / BRICK, y as usize / BRICK, z as usize / BRICK);
        let b = self.table[(bz * self.nb + by) * self.nb + bx];
        if b == u32::MAX {
            return false;
        }
        let (lx, ly, lz) = (x as usize % BRICK, y as usize % BRICK, z as usize % BRICK);
        let o = ((b as usize * BRICK * BRICK * BRICK) + (lz * BRICK + ly) * BRICK + lx) * NF;
        out.copy_from_slice(&self.bricks[o..o + NF]);
        true
    }

    /// Is the brick containing voxel-space point p empty? (empty-space skipping)
    #[inline]
    pub fn brick_empty(&self, p: [f64; 3]) -> bool {
        let n = self.n as f64;
        if p.iter().any(|&c| c < -1.0 || c > n) {
            return true;
        }
        let b = |c: f64| ((c.max(0.0) as usize) / BRICK).min(self.nb - 1);
        // conservative: also check the brick of the opposite trilinear corner
        let (bx, by, bz) = (b(p[0]), b(p[1]), b(p[2]));
        let (cx, cy, cz) = (b(p[0] + 1.0), b(p[1] + 1.0), b(p[2] + 1.0));
        for z in [bz, cz] { for y in [by, cy] { for x in [bx, cx] {
            if self.table[(z * self.nb + y) * self.nb + x] != u32::MAX { return false; }
        }}}
        true
    }

    /// Trilinear sample at voxel-space point p (voxel centres at integers).
    pub fn sample(&self, p: [f64; 3], out: &mut [f32; NF]) {
        out.fill(0.0);
        let f = [p[0].floor(), p[1].floor(), p[2].floor()];
        let t = [(p[0] - f[0]) as f32, (p[1] - f[1]) as f32, (p[2] - f[2]) as f32];
        let mut v = [0f32; NF];
        for c in 0..8 {
            let o = [(c & 1) as i64, ((c >> 1) & 1) as i64, ((c >> 2) & 1) as i64];
            if !self.voxel(f[0] as i64 + o[0], f[1] as i64 + o[1], f[2] as i64 + o[2], &mut v) {
                continue;
            }
            let w = (if o[0] == 1 { t[0] } else { 1.0 - t[0] }) * (if o[1] == 1 { t[1] } else { 1.0 - t[1] }) * (if o[2] == 1 { t[2] } else { 1.0 - t[2] });
            for k in 0..NF { out[k] += w * v[k]; }
        }
    }

    pub fn occupied_fraction(&self) -> f64 {
        self.brick_max.len() as f64 / self.table.len() as f64
    }

    // ------------------------------------------------------------ cache --
    // Layout: "OBSVOL01" | u32 json_len | json | u32 n | f64 dx | f64 sca_temp |
    // table (u32 × nb³) | bricks (f16 × count) | brick_max (f32 × 2 × count)
    pub fn to_bytes(&self) -> Vec<u8> {
        let header = self.meta.dump();
        let mut b = Vec::with_capacity(64 + header.len() + self.table.len() * 4 + self.bricks.len() * 2);
        b.extend_from_slice(b"OBSVOL01");
        b.extend_from_slice(&(header.len() as u32).to_le_bytes());
        b.extend_from_slice(header.as_bytes());
        b.extend_from_slice(&(self.n as u32).to_le_bytes());
        b.extend_from_slice(&self.dx.to_le_bytes());
        b.extend_from_slice(&self.sca_temp.to_le_bytes());
        for t in &self.table { b.extend_from_slice(&t.to_le_bytes()); }
        b.extend_from_slice(&(self.brick_max.len() as u32).to_le_bytes());
        // Per-field power-of-two scale so each field's maximum lands near 2¹⁴
        // in half-float range (physical values span ~1e-22..1e3).
        let mut scale = [1f32; NF];
        for f in 0..NF {
            let mx = self.bricks.iter().skip(f).step_by(NF).fold(0f32, |a, &v| a.max(v.abs()));
            if mx > 0.0 { scale[f] = 2f32.powi(14 - mx.log2().ceil() as i32); }
            b.extend_from_slice(&scale[f].to_le_bytes());
        }
        for (k, v) in self.bricks.iter().enumerate() { b.extend_from_slice(&f32_to_f16(*v * scale[k % NF]).to_le_bytes()); }
        for m in &self.brick_max { b.extend_from_slice(&m[0].to_le_bytes()); b.extend_from_slice(&m[1].to_le_bytes()); }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Volume, String> {
        if b.len() < 12 || &b[..8] != b"OBSVOL01" {
            return Err("not an OBSVOL01 volume cache".into());
        }
        let mut i = 8;
        let rd_u32 = |i: &mut usize| { let v = u32::from_le_bytes(b[*i..*i + 4].try_into().unwrap()); *i += 4; v };
        let rd_f64 = |i: &mut usize| { let v = f64::from_le_bytes(b[*i..*i + 8].try_into().unwrap()); *i += 8; v };
        let hl = rd_u32(&mut i) as usize;
        let meta = Json::parse(std::str::from_utf8(&b[i..i + hl]).map_err(|_| "header utf8")?)?;
        i += hl;
        let n = rd_u32(&mut i) as usize;
        let dx = rd_f64(&mut i);
        let sca_temp = rd_f64(&mut i);
        let nb = n / BRICK;
        let table: Vec<u32> = (0..nb * nb * nb).map(|_| rd_u32(&mut i)).collect();
        let count = rd_u32(&mut i) as usize;
        let scale: Vec<f32> = (0..NF).map(|f| f32::from_le_bytes(b[i + 4 * f..i + 4 * f + 4].try_into().unwrap())).collect();
        i += 4 * NF;
        let nvals = count * BRICK * BRICK * BRICK * NF;
        if b.len() < i + nvals * 2 + count * 8 {
            return Err("truncated volume cache".into());
        }
        let bricks: Vec<f32> = (0..nvals).map(|k| f16_to_f32(u16::from_le_bytes([b[i + 2 * k], b[i + 2 * k + 1]])) / scale[k % NF]).collect();
        i += nvals * 2;
        let brick_max: Vec<[f32; 2]> = (0..count)
            .map(|k| {
                let o = i + 8 * k;
                [f32::from_le_bytes(b[o..o + 4].try_into().unwrap()), f32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap())]
            })
            .collect();
        Ok(Volume { n, dx, nb, table, bricks, brick_max, meta, sca_temp })
    }
}

/// IEEE 754 binary16 conversion (round to nearest even), used for the cache.
pub fn f32_to_f16(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xFF) as i32;
    let mut man = x & 0x007F_FFFF;
    if exp == 0xFF {
        return sign | 0x7C00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1F {
        return sign | 0x7C00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        man |= 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = man >> shift;
        let rem = man & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let r = if rem > mid || (rem == mid && half & 1 == 1) { half + 1 } else { half };
        return sign | r as u16;
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1FFF;
    let r = if rem > 0x1000 || (rem == 0x1000 && half & 1 == 1) { half + 1 } else { half };
    sign | r as u16
}

pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1F) as u32;
    let man = (h & 0x3FF) as u32;
    let bits = if exp == 0 {
        if man == 0 {
            sign
        } else {
            let mut e = 127 - 15 + 1;
            let mut m = man;
            while m & 0x400 == 0 { m <<= 1; e -= 1; }
            sign | (e << 23) | ((m & 0x3FF) << 13)
        }
    } else if exp == 0x1F {
        sign | 0x7F80_0000 | (man << 13)
    } else {
        sign | ((exp + 127 - 15) << 23) | (man << 13)
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_float_roundtrip() {
        for &v in &[0.0f32, 1.0, -2.5, 65504.0, 6.1e-5, 3.0e-7, 1234.5] {
            let r = f16_to_f32(f32_to_f16(v));
            assert!((r - v).abs() <= v.abs() * 1e-3 + 1e-7, "{v} -> {r}");
        }
    }

    #[test]
    fn cache_roundtrip_preserves_meta_and_dims() {
        let n = 16;
        let mut dense: [Vec<f32>; NF] = std::array::from_fn(|_| vec![0.0; n * n * n]);
        dense[F_ALPHA][(5 * n + 5) * n + 5] = 2.0e-21;
        dense[2][(12 * n + 3) * n + 9] = 7.0e-11;
        let mut meta = Json::obj();
        meta.set("seed", 99u64).set("generator", "test");
        let v = Volume::from_dense(n, 0.5, &dense, meta.clone(), 30000.0);
        let w = Volume::from_bytes(&v.to_bytes()).unwrap();
        assert_eq!(w.n, n);
        assert_eq!(w.dx, 0.5);
        assert_eq!(w.meta, meta);
        assert_eq!(w.brick_max.len(), 2);
        let mut o = [0f32; NF];
        w.sample([5.0, 5.0, 5.0], &mut o);
        assert!((o[F_ALPHA] / 2.0e-21 - 1.0).abs() < 1e-3);
        w.sample([9.0, 3.0, 12.0], &mut o);
        assert!((o[2] / 7.0e-11 - 1.0).abs() < 1e-3);
        assert!(w.brick_empty([1.0, 14.0, 1.0]));
    }
}
