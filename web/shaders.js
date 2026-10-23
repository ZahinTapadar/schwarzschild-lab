// GLSL ES 3.0 shaders: Schwarzschild null-geodesic ray tracer + HDR post.
// Geometric units (G = c = 1); every length is in the same unit as uM.

export const FULLSCREEN_VS = `#version 300 es
in vec2 aPos;
out vec2 vUv;
void main() { vUv = aPos * 0.5 + 0.5; gl_Position = vec4(aPos, 0.0, 1.0); }`;

const NOISE = `
float hash13(vec3 p) {
  p = fract(p * 0.1031);
  p += dot(p, p.zyx + 31.32);
  return fract((p.x + p.y) * p.z);
}
vec3 hash33(vec3 p) {
  p = fract(p * vec3(0.1031, 0.1030, 0.0973));
  p += dot(p, p.yxz + 33.33);
  return fract((p.xxy + p.yxx) * p.zyx);
}
float vnoise(vec3 p) {
  vec3 i = floor(p), f = fract(p);
  vec3 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash13(i), hash13(i + vec3(1,0,0)), u.x),
                 mix(hash13(i + vec3(0,1,0)), hash13(i + vec3(1,1,0)), u.x), u.y),
             mix(mix(hash13(i + vec3(0,0,1)), hash13(i + vec3(1,0,1)), u.x),
                 mix(hash13(i + vec3(0,1,1)), hash13(i + vec3(1,1,1)), u.x), u.y), u.z);
}
float fbm(vec3 p, int oct) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 6; i++) {
    if (i >= oct) break;
    s += a * vnoise(p);
    p = p * 2.03 + vec3(1.7, 9.2, 3.1);
    a *= 0.5;
  }
  return s;
}`;

// --------------------------------------------------- calibrated synthetic sky ---
// A population-synthesis sky in the spirit of the Gaia Universe Model /
// Besancon models, cut down to what a fragment shader can afford. It is one
// stochastic realisation of a physically motivated Milky Way and galaxy field,
// not a map of the real sky and not an N-body or cosmological simulation.
//
//   model      3D stellar densities (thin/thick disc, bar, halo), dust disc,
//              Kroupa IMF luminosity functions, galaxy number counts
//   sampling   Poisson draws in hierarchical cube-sphere cells, keyed by
//              (spaceSeed, level, cell, index): the seed picks the realisation
//   radiance   apparent magnitudes -> flux, extinction exp(-tau), blackbody
//              colours, unresolved remainder as integrated diffuse light
//
// Everything is evaluated on the escaped geodesic direction in sky(), so
// lensing and observer aberration come from the ray tracer. The star counts
// were calibrated against observed all-sky counts N(<V) from V = 0 to 15
// (within ~30%).
const nrm3 = (v) => { const l = Math.hypot(...v); return v.map((x) => x / l); };
const cross3 = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot3 = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const glv = (v) => `vec3(${v.map((x) => x.toFixed(6)).join(', ')})`;

// Fixed galactic frame (part of the model, not the seed): north pole N, the
// direction from the Sun to the Galactic centre C, and E = N x C.
export const GALACTIC_POLE = nrm3([0.18, 1.0, -0.42]);
export const GALACTIC_CENTRE = (() => {
  const c0 = [-0.9, 0.0, 0.35];
  const k = dot3(c0, GALACTIC_POLE);
  return nrm3(c0.map((x, i) => x - k * GALACTIC_POLE[i]));
})();
export const GALACTIC_EAST = cross3(GALACTIC_POLE, GALACTIC_CENTRE);

// Kroupa-like present-day mass functions: dN/dm = m^-1.3 (0.08-0.5),
// 0.5 m^-2.3 (0.5-1), and above 1 Msun either 0.5 m^-4.8 (thin disc, constant
// star formation for 10 Gyr with t_MS = 10 Gyr m^-2.5) or nothing above the
// 0.95 Msun turn-off (old populations).
const segN = (a, m1, m2) => (Math.pow(m1, 1 - a) - Math.pow(m2, 1 - a)) / (a - 1);
const TOT_Y = segN(1.3, 0.08, 0.5) + 0.5 * segN(2.3, 0.5, 1) + 0.5 * segN(4.8, 1, 60);
const TOT_O = segN(1.3, 0.08, 0.5) + 0.5 * segN(2.3, 0.5, 0.95);

const SKY = `
uniform float uSpaceSeed;      // stochastic realisation
uniform float uMagLimit;       // faintest individually rendered star (V)
uniform float uDust;           // extinction scale
uniform float uNebulae;        // nebular emission / scattering
uniform float uGalaxies;       // extragalactic density
uniform int   uSkyDebug;       // 0 radiance, 1 populations, 2 A_V, 3 star counts, 4 nebula conditions, 5 galaxy field

const vec3 GAL_N = ${glv(GALACTIC_POLE)};
const vec3 GAL_C = ${glv(GALACTIC_CENTRE)};
const vec3 GAL_E = ${glv(GALACTIC_EAST)};

// ---- model parameters (kpc, V magnitudes) ----
const float R0 = 8.2, ZSUN = 0.02;                                // Sun's galactocentric position
const float HR_THIN = 2.6, HZ_THIN = 0.30, HOLE = 3.0;             // thin disc (with central hole)
const float N_THICK = 0.12, HR_THICK = 2.0, HZ_THICK = 0.9;        // thick disc, relative to local thin
const float N_BULGE = 120.0, BAR = 0.47, BX = 1.2, BY = 0.45, BZ = 0.35;   // Gaussian triaxial bar, 27 deg
const float N_HALO = 0.0015, RC_HALO = 0.5, Q_HALO = 0.7, P_HALO = 2.5;   // power-law spheroid
const float N0 = 4.2e7;                                           // local thin-disc stars per kpc^3 (calibrated)
const float FG_Y = 0.003, FG_O = 0.006;                           // giant fractions, M_V uniform in [-1.5, 1.5]
const float HR_DUST = 3.2, HZ_DUST = 0.12, KAPPA_V = 0.74;        // tau_V per kpc at the Sun (A_V ~ 0.8 mag/kpc)
const float TOT_Y = ${TOT_Y.toFixed(6)}, TOT_O = ${TOT_O.toFixed(6)};
const float M_ZERO = 4.5;
const float J_ISRF = 2250.0;                                      // all-sky mean starlight, F per sr (from this model)                                        // apparent V with flux F = 1
const int   NSHELL = 10;                                          // log shells 5 pc .. 40 kpc
const float SHELL_R0 = 0.005, SHELL_DLN = ${(Math.log(8000) / 10).toFixed(6)};

// ---- hashing: PCG3D keyed by (seed, level, cell, index) ----
uvec3 pcg3d(uvec3 v) {
  v = v * 1664525u + 1013904223u;
  v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
  v ^= v >> 16u;
  v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
  return v;
}
vec3 rnd3(uvec3 key) {
  return vec3(pcg3d(key + uint(uSpaceSeed) * uvec3(73856093u, 19349663u, 83492791u))) * (1.0 / 4294967295.0);
}

// ---- equi-angular cube-sphere tessellation ----
// face 0..5 = +x -x +y -y +z -z; a in [-1,1]^2 is the equi-angular coordinate
// (a = 4/pi atan u), so cells at level L (2^L per face side) have nearly equal area.
void cubeCoords(vec3 d, out int face, out vec2 a) {
  vec3 ad = abs(d);
  vec2 uv;
  if (ad.x >= ad.y && ad.x >= ad.z) { face = d.x > 0.0 ? 0 : 1; uv = vec2(d.x > 0.0 ? -d.z : d.z, d.y) / ad.x; }
  else if (ad.y >= ad.z) { face = d.y > 0.0 ? 2 : 3; uv = vec2(d.x, d.y > 0.0 ? -d.z : d.z) / ad.y; }
  else { face = d.z > 0.0 ? 4 : 5; uv = vec2(d.z > 0.0 ? d.x : -d.x, d.y) / ad.z; }
  a = atan(uv) * (4.0 / 3.14159265);
}
vec3 cubeDir(int face, vec2 a) {
  vec2 u = tan(a * 0.78539816);
  vec3 d = face == 0 ? vec3(1.0, u.y, -u.x) : face == 1 ? vec3(-1.0, u.y, u.x)
         : face == 2 ? vec3(u.x, 1.0, -u.y) : face == 3 ? vec3(u.x, -1.0, u.y)
         : face == 4 ? vec3(u.x, u.y, 1.0) : vec3(-u.x, u.y, -1.0);
  return normalize(d);
}
float cellSolidAngle(vec2 a, float n) {          // dOmega of the cell centred at a
  vec2 u = tan(a * 0.78539816);
  float s = 1.0 + dot(u, u);
  return 0.61685 * (1.0 + u.x * u.x) * (1.0 + u.y * u.y) / (s * sqrt(s)) * 4.0 / (n * n);
}
uvec3 cellKey(int face, int level, ivec2 ij, int salt) {
  return uvec3(uint(ij.x), uint(ij.y), uint(face) + 8u * uint(level) + 256u * uint(salt));
}

// ---- gradient noise (lattice gradients from PCG, rotated octaves) ----
vec3 gradAt(vec3 c, uint salt) {
  return vec3(pcg3d(uvec3(ivec3(c)) + uvec3(salt, salt * 7u, salt * 13u) + uint(uSpaceSeed) * uvec3(2654435761u, 40503u, 9973u))) * (2.0 / 4294967295.0) - 1.0;
}
float gnoise(vec3 p, uint salt) {
  vec3 i = floor(p), f = fract(p);
  vec3 w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  float n000 = dot(gradAt(i, salt), f);
  float n100 = dot(gradAt(i + vec3(1, 0, 0), salt), f - vec3(1, 0, 0));
  float n010 = dot(gradAt(i + vec3(0, 1, 0), salt), f - vec3(0, 1, 0));
  float n110 = dot(gradAt(i + vec3(1, 1, 0), salt), f - vec3(1, 1, 0));
  float n001 = dot(gradAt(i + vec3(0, 0, 1), salt), f - vec3(0, 0, 1));
  float n101 = dot(gradAt(i + vec3(1, 0, 1), salt), f - vec3(1, 0, 1));
  float n011 = dot(gradAt(i + vec3(0, 1, 1), salt), f - vec3(0, 1, 1));
  float n111 = dot(gradAt(i + vec3(1, 1, 1), salt), f - vec3(1, 1, 1));
  return mix(mix(mix(n000, n100, w.x), mix(n010, n110, w.x), w.y),
             mix(mix(n001, n101, w.x), mix(n011, n111, w.x), w.y), w.z);
}
const mat3 OCT_ROT = mat3(0.00, 0.80, 0.60, -0.80, 0.36, -0.48, -0.60, -0.48, 0.64);

// Unit-variance-ish Gaussian field on the sphere (domain-warped, rotated octaves,
// part plain and part ridged so it forms filaments). Used only to perturb the
// physical dust density and the far-field galaxy density, never as visible fog.
float gaussField(vec3 g, float scale, int octaves, float ridge, uint salt) {
  vec3 q = g * scale;
  q += 0.45 * vec3(gnoise(q * 0.5, salt + 11u), gnoise(q * 0.5, salt + 12u), gnoise(q * 0.5, salt + 13u)) * 2.0;
  float sp = 0.0, sr = 0.0, a = 1.0, an = 0.0;
  for (int i = 0; i < 4; i++) {
    if (i >= octaves) break;
    float n = gnoise(q, salt + uint(i));
    sp += a * n; sr += a * (0.2 - abs(n)); an += a * a;
    q = OCT_ROT * q * 2.03; a *= 0.55;
  }
  return ((1.0 - ridge) * sp / 0.25 + ridge * sr / 0.15) / sqrt(an);
}

// ---- line-of-sight model ----
// d (galactic frame: x = E, y = N, z = C) -> galactocentric X, Y, Z at distance r
vec3 galPos(vec3 g, float r) { return vec3(R0 - r * g.z, r * g.x, ZSUN + r * g.y); }
float rhoThin(vec3 P) {
  float R = length(P.xy);
  float hole = (1.0 - exp(-(R * R) / (HOLE * HOLE))) / (1.0 - exp(-(R0 * R0) / (HOLE * HOLE)));
  return exp(-(R - R0) / HR_THIN - (abs(P.z) - ZSUN) / HZ_THIN) * hole;
}
float rhoThick(vec3 P) { return N_THICK * exp(-(length(P.xy) - R0) / HR_THICK - (abs(P.z) - ZSUN) / HZ_THICK); }
float rhoBulge(vec3 P) {
  float c = cos(BAR), s = sin(BAR);
  vec3 b = vec3((P.x * c + P.y * s) / BX, (-P.x * s + P.y * c) / BY, P.z / BZ);
  return N_BULGE * exp(-0.5 * dot(b, b));
}
float rhoHalo(vec3 P) {
  float rh2 = dot(P.xy, P.xy) + (P.z / Q_HALO) * (P.z / Q_HALO);
  return N_HALO * pow((rh2 + RC_HALO * RC_HALO) / (R0 * R0 + RC_HALO * RC_HALO), -0.5 * P_HALO);
}
float rhoDust(vec3 P) { return exp(-(length(P.xy) - R0) / HR_DUST - (abs(P.z) - ZSUN) / HZ_DUST); }
float rhoYoung(vec3 P) { return rhoThin(P) * exp(-abs(P.z) / 0.06 + abs(P.z) / HZ_THIN); }   // young disc, h_z = 60 pc

// Per-pixel line-of-sight state: for each shell its radius, distance modulus,
// extinction A_V to its centre, and thin/old number weights (stars per sr).
float shR[NSHELL];
float shMu[NSHELL];
float shA[NSHELL];
float shWy[NSHELL];
float shWo[NSHELL];
float losTau;                  // total tau_V to infinity
float cloudC;                  // lognormal molecular-cloud factor in this direction

// Kroupa PDMF tail fraction above mass mt (young = constant-SFR thin disc).
float segN(float a, float m1, float m2) { return (pow(m1, 1.0 - a) - pow(m2, 1.0 - a)) / (a - 1.0); }
float pdmfTail(float mt, bool young) {
  float t = 0.0;
  if (mt < 0.5) t += segN(1.3, max(mt, 0.08), 0.5);
  float top = young ? 1.0 : 0.95;
  if (mt < top) t += 0.5 * segN(2.3, max(mt, 0.5), top);
  if (young && mt < 60.0) t += 0.5 * segN(4.8, max(mt, 1.0), 60.0);
  return t / (young ? TOT_Y : TOT_O);
}
float massOfMV(float M) { return pow(10.0, (4.83 - M) / 8.75); }   // main sequence L ~ m^3.5
// Fraction of stars brighter than absolute magnitude M.
float lfCum(float M, bool young) {
  float fg = young ? FG_Y : FG_O;
  return (1.0 - fg) * pdmfTail(massOfMV(M), young) + fg * clamp((M + 1.5) / 3.0, 0.0, 1.0);
}
// Mean luminosity (Lsun) per star contributed by stars fainter than M.
float segL(float a, float m1, float m2) { float e = 4.5 - a; return (pow(m2, e) - pow(m1, e)) / e; }
float lfLumFainter(float M, bool young) {
  float mt = massOfMV(M);
  float l = segL(1.3, 0.08, clamp(mt, 0.08, 0.5));
  float top = young ? 1.0 : 0.95;
  if (mt > 0.5) l += 0.5 * segL(2.3, 0.5, min(mt, top));
  if (young && mt > 1.0) l += 0.5 * segL(4.8, 1.0, min(mt, 60.0));
  float fg = young ? FG_Y : FG_O;
  // giants: M uniform on [-1.5, 1.5]; luminosity of those fainter than M
  float lo = clamp(M, -1.5, 1.5);
  float lg = (pow(10.0, -0.4 * (lo - 4.83)) - pow(10.0, -0.4 * (1.5 - 4.83))) / (0.921034 * 3.0);
  return (1.0 - fg) * l / (young ? TOT_Y : TOT_O) + fg * lg;
}

void buildLOS(vec3 g) {
  // clumpy molecular clouds: lognormal perturbation of the dust density, with
  // a contrast that is high in the plane and small at high latitude
  float sig = 0.35 + 0.65 * exp(-abs(g.y) / 0.15);
  cloudC = min(exp(sig * gaussField(g, 4.5, 4, 0.6, 100u) - 0.5 * sig * sig), 12.0);
  float tau = 0.0;
  for (int k = 0; k < NSHELL; k++) {
    float r = SHELL_R0 * exp((float(k) + 0.5) * SHELL_DLN);
    float dr = r * SHELL_DLN;
    vec3 P = galPos(g, r);
    float dtau = KAPPA_V * uDust * rhoDust(P) * cloudC * dr;
    shR[k] = r;
    shMu[k] = 5.0 * log(r) / log(10.0) + 10.0;
    shA[k] = 1.086 * (tau + 0.5 * dtau);
    tau += dtau;
    float w = N0 * uStarDensity * r * r * dr;
    shWy[k] = w * rhoThin(P);
    shWo[k] = w * (rhoThick(P) + rhoBulge(P) + rhoHalo(P));
  }
  losTau = tau;
}
float extinctionTo(float r) {                    // A_V to distance r (interpolated between shells)
  float x = log(r / SHELL_R0) / SHELL_DLN - 0.5;
  int k = int(clamp(floor(x), 0.0, float(NSHELL - 2)));
  return mix(shA[k], shA[k + 1], clamp(x - float(k), 0.0, 1.0));
}
// Expected stars per steradian brighter than apparent magnitude m.
float countBrighter(float m) {
  float n = 0.0;
  for (int k = 0; k < NSHELL; k++) {
    float off = shMu[k] + shA[k];
    if (shWy[k] > 1e-3) n += shWy[k] * lfCum(m - off, true);
    if (shWo[k] > 1e-3) n += shWo[k] * lfCum(m - off, false);
  }
  return n;
}
// Expected stars per steradian with apparent magnitude in [ma, mb).
float countBetween(float ma, float mb) {
  float n = 0.0;
  for (int k = 0; k < NSHELL; k++) {
    float off = shMu[k] + shA[k];
    if (shWy[k] > 1e-3) n += shWy[k] * (lfCum(mb - off, true) - lfCum(ma - off, true));
    if (shWo[k] > 1e-3) n += shWo[k] * (lfCum(mb - off, false) - lfCum(ma - off, false));
  }
  return n;
}

// ---- footprint / point-spread function ----
// Reference angular footprint (rad): a point source of flux F renders with
// peak radiance F when one pixel subtends W0. Diffuse flux S per steradian
// renders as S * pi * W0^2.
const float W0 = 0.0007;
const float DIFFUSE_K = 3.14159265 * W0 * W0;
vec3 gJx, gJy;
float gA11, gA12, gA22, gDet, gArea, gFw;
void setFootprint(vec3 jx, vec3 jy, float basePx) {
  gJx = jx; gJy = jy;
  gA11 = dot(jx, jx); gA12 = dot(jx, jy); gA22 = dot(jy, jy);
  gDet = max(gA11 * gA22 - gA12 * gA12, 1e-30);
  gArea = max(sqrt(gDet), basePx * basePx * 0.02);         // sky solid angle per pixel (<= 50x magnification)
  gFw = clamp(sqrt(max(gA11, gA22)), basePx * 0.02, basePx * 60.0);
}
// Round ~1.6 px spot in screen space (through the inverse Jacobian), unit flux.
float psf(vec3 dqr) {
  float r1 = dot(gJx, dqr), r2 = dot(gJy, dqr);
  float sx = (gA22 * r1 - gA12 * r2) / gDet, sy = (gA11 * r2 - gA12 * r1) / gDet;
  vec3 res = dqr - sx * gJx - sy * gJy;
  float pix2 = sx * sx + sy * sy + dot(res, res) / sqrt(gDet);
  return W0 * W0 / (0.64 * gArea) * exp(-pix2 / 0.64);
}
float fluxOfMag(float m) { return pow(10.0, -0.4 * (m - M_ZERO)); }

// Main-sequence effective temperature from mass (rough fit).
float teffMS(float m) { return 5778.0 * (m < 1.0 ? pow(m, 0.35) : pow(m, 0.6)); }
// Interstellar reddening as a shift of colour temperature (visual approximation):
// 1/T_app = 1/T + 1.2e-4 E(B-V), E(B-V) = A_V / 3.1.
float redden(float T, float Av) { return 1.0 / (1.0 / T + 3.9e-5 * Av); }

// ---- resolved stars ----
// Magnitude bins, each sampled on its own cube-sphere level so the expected
// count per cell stays O(1). Within a bin, magnitudes follow dN/dm ~ 10^(0.3 m).
const int NBIN = 6;
const float BIN_EDGE[7] = float[7](-3.0, 1.5, 4.0, 6.5, 8.5, 10.5, 12.5);
const int BIN_LEVEL[6] = int[6](2, 4, 6, 7, 9, 10);

float sampleMag(float ma, float mb, float u) {
  float b = 0.3 * 2.302585;
  return ma + log(1.0 + u * (exp(b * (mb - ma)) - 1.0)) / b;
}
float meanFluxInBin(float ma, float mb) {           // mean F under the sampling density
  float b = 0.3 * 2.302585, c = b - 0.4 * 2.302585;
  float num = (exp(c * mb) - exp(c * ma)) / c, den = (exp(b * mb) - exp(b * ma)) / b;
  return pow(10.0, 0.4 * M_ZERO) * num / den;
}

// Attribute a star of apparent magnitude m (drawn in bin [ma, mb)) to a shell
// and population, then derive its absolute magnitude, mass/class and colour.
vec3 starColour(float m, float ma, float mb, vec3 h) {
  float tot = 0.0;
  for (int k = 0; k < NSHELL; k++) {
    float off = shMu[k] + shA[k];
    tot += shWy[k] * (lfCum(mb - off, true) - lfCum(ma - off, true))
         + shWo[k] * (lfCum(mb - off, false) - lfCum(ma - off, false));
  }
  float pick = h.x * tot, acc = 0.0;
  int ks = NSHELL - 1; bool young = true;
  for (int k = 0; k < NSHELL; k++) {
    float off = shMu[k] + shA[k];
    float cy = shWy[k] * (lfCum(mb - off, true) - lfCum(ma - off, true));
    float co = shWo[k] * (lfCum(mb - off, false) - lfCum(ma - off, false));
    if (pick < acc + cy) { ks = k; young = true; break; }
    acc += cy;
    if (pick < acc + co) { ks = k; young = false; break; }
    acc += co;
  }
  float Av = shA[ks];
  float M = m - shMu[ks] - Av;
  float fg = young ? FG_Y : FG_O;
  float pMS = (1.0 - fg) * max(pdmfTail(massOfMV(M - 0.05), young) - pdmfTail(massOfMV(M + 0.05), young), 0.0) / 0.1;
  float pG = (M > -1.5 && M < 1.5) ? fg / 3.0 : 0.0;
  float T;
  if (h.y * (pMS + pG) < pG) T = mix(3900.0, 5100.0, h.z);            // red giant / clump
  else T = min(teffMS(massOfMV(M)), 45000.0);
  if (!young) T *= 1.03;                                              // metal-poor populations run slightly bluer
  return blackbody(redden(T, Av) * gObsD);
}

vec3 stellarBackground(vec3 d, float magLim, out float unresolvedFlux) {
  int face; vec2 a;
  cubeCoords(d, face, a);
  vec3 col = vec3(0.0);
  unresolvedFlux = 0.0;
  // cumulative counts at the bin edges (each edge shared by two bins)
  float edgeN[7];
  for (int j = 0; j < 7; j++) {
    float e = min(BIN_EDGE[j], magLim);
    edgeN[j] = (j > 0 && BIN_EDGE[j - 1] >= magLim) ? edgeN[j - 1] : countBrighter(e);
  }
  for (int b = 0; b < NBIN; b++) {
    float ma = BIN_EDGE[b], mb = min(BIN_EDGE[b + 1], magLim);
    if (ma >= mb) break;
    float nSr = edgeN[b + 1] - edgeN[b];               // expected stars per sr in this bin
    if (nSr <= 0.0) continue;
    int L = BIN_LEVEL[b];
    float n = exp2(float(L));
    float cellAng = 1.5708 / n;
    float Fbar = meanFluxInBin(ma, mb);
    // footprint larger than a cell: the bin becomes its mean radiance (flux conserved)
    float resolved = 1.0 - smoothstep(0.35, 0.8, gFw / cellAng);
    if (resolved < 1.0) unresolvedFlux += nSr * Fbar * (1.0 - resolved);
    if (resolved <= 0.0) continue;
    ivec2 ij = ivec2(clamp(floor((a * 0.5 + 0.5) * n), 0.0, n - 1.0));
    float lost = 0.0;
    for (int di = -1; di <= 1; di++) for (int dj = -1; dj <= 1; dj++) {
      ivec2 c = ij + ivec2(di, dj);
      if (c.x < 0 || c.y < 0 || float(c.x) >= n || float(c.y) >= n) continue;
      vec2 ac = (vec2(c) + 0.5) / n * 2.0 - 1.0;
      float lam = nSr * cellSolidAngle(ac, n);
      vec3 h = rnd3(cellKey(face, L, c, 1));
      // Poisson count, truncated at 3 candidates (inverse CDF)
      float e = exp(-lam);
      int cnt = int(h.x > e) + int(h.x > e * (1.0 + lam)) + int(h.x > e * (1.0 + lam + 0.5 * lam * lam));
      if (di == 0 && dj == 0) lost = max(lam - (1.0 - e) - (1.0 - e * (1.0 + lam)) - (1.0 - e * (1.0 + lam + 0.5 * lam * lam)), 0.0);
      for (int i = 0; i < 3; i++) {
        if (i >= cnt) break;
        vec3 s = rnd3(cellKey(face, L, c, 2 + i));
        vec3 sd = cubeDir(face, (vec2(c) + s.xy) / n * 2.0 - 1.0);
        vec3 dq = sd - d;
        if (dot(dq, dq) > 49.0 * gFw * gFw + 1e-12) continue;    // too far to touch this pixel
        float m = sampleMag(ma, mb, s.z);
        vec3 h2 = rnd3(cellKey(face, L, c, 5 + i));
        col += starColour(m, ma, mb, h2) * fluxOfMag(m) * psf(dq) * resolved;
      }
    }
    // stars beyond the 3-candidate cap still contribute their mean flux
    unresolvedFlux += lost / cellSolidAngle(a, n) * Fbar * resolved;
  }
  return col;
}

// Integrated light of all stars fainter than the magnitude limit, per shell,
// attenuated by exp(-tau) (flux per steradian, F units).
float diffuseStarlight(float magLim, out float unextinguished) {
  float s = 0.0;
  unextinguished = 0.0;
  float zp = pow(10.0, -0.4 * (14.83 - M_ZERO));     // 10 pc -> kpc and the M_ZERO flux scale
  for (int k = 0; k < NSHELL; k++) {
    float M = magLim - shMu[k] - shA[k];
    float lum = shWy[k] * lfLumFainter(M, true) + shWo[k] * lfLumFainter(M, false);
    float dr2 = shR[k] * shR[k];
    float sh = lum / dr2 * zp;
    s += sh * pow(10.0, -0.4 * shA[k]);
    unextinguished += sh;
  }
  return s;
}

// ---- open clusters and the nebulae they light ----
// Associations are drawn from the young-disc density, boosted where molecular
// clouds are (cloudC). Emission only appears with an O star (m > 15 Msun) in a
// cluster younger than its lifetime and inside a cloud; reflection needs a B
// star, a cloud and no ionising source; otherwise only the stars are seen.
vec3 gE1, gE2;
vec2 tangentCoords(vec3 g, vec3 cd, float pa) {
  vec3 up = abs(cd.y) > 0.9 ? vec3(1.0, 0.0, 0.0) : vec3(0.0, 1.0, 0.0);
  vec3 a = normalize(cross(up, cd)), b = cross(cd, a);
  gE1 = cos(pa) * a + sin(pa) * b;
  gE2 = -sin(pa) * a + cos(pa) * b;
  return vec2(dot(g, gE1), dot(g, gE2)) / dot(g, cd);
}

float kroupaTot(float mmax) { return segN(1.3, 0.08, 0.5) + 0.5 * segN(2.3, 0.5, mmax); }
float massAtTail(float q, float mmax) {            // i-th brightest member: mass with tail fraction q
  float t = q * kroupaTot(mmax) / 0.5 * 1.3 + pow(mmax, -1.3);
  return clamp(pow(t, -1.0 / 1.3), 0.5, mmax);
}

vec3 hiiRegion(vec2 p, float seed, float hot) {      // p in units of the Stromgren radius
  vec2 w = p + 0.55 * vec2(gnoise(vec3(p * 1.3, seed), 30u), gnoise(vec3(p * 1.3, seed + 7.1), 30u)) * 2.0;
  w += 0.25 * vec2(gnoise(vec3(w * 3.1, seed + 3.3), 31u), gnoise(vec3(w * 3.1, seed + 9.4), 31u)) * 2.0;
  float rr = length(w);
  float shape = smoothstep(1.0, 0.3, rr + 0.5 * gnoise(vec3(w * 2.2, seed + 1.0), 32u));
  if (shape <= 0.0) return vec3(0.0);
  float ln = 1.2 * gnoise(vec3(w * 4.0, seed + 2.0), 33u) + 0.6 * gnoise(vec3(w * 9.0, seed + 4.0), 34u);
  float em = shape * exp(2.0 * ln - 0.4);                       // emission measure ~ n_e^2 (lognormal clumps)
  float front = smoothstep(0.45, 0.85, rr) * shape;             // brighter ionisation front
  float globules = smoothstep(0.15, 0.4, gnoise(vec3(w * 5.5, seed + 6.0), 35u)) * 2.0;
  vec3 col = vec3(1.0, 0.25, 0.32) * em * (0.6 + 0.8 * front)                   // H-alpha / [N II]
           + vec3(0.30, 0.68, 1.0) * em * exp(-rr * rr * 4.0) * 0.5 * hot;     // [O III] near the hottest stars
  return col * exp(-globules);
}
vec3 reflectionNebula(vec2 p, float seed, vec3 starCol) {   // p in units of the nebula radius
  vec2 w = p + 0.5 * vec2(gnoise(vec3(p * 1.5, seed), 40u), gnoise(vec3(p * 1.5, seed + 5.0), 40u)) * 2.0;
  float dust = smoothstep(-0.05, 0.35, gnoise(vec3(w.x * 2.0, w.y * 4.0, seed + 1.0), 41u)) * smoothstep(1.0, 0.2, length(w));
  float illum = 1.0 / (1.0 + 8.0 * dot(p, p));                  // inverse-square from the star
  return starCol * vec3(0.55, 0.78, 1.3) * dust * illum;         // blue-favouring scattering
}

void clusterField(vec3 g, vec3 d, inout vec3 stars, inout vec3 neb, inout vec3 debugNeb) {
  int face; vec2 a;
  cubeCoords(d, face, a);
  for (int lev = 0; lev < 2; lev++) {
    int L = lev == 0 ? 3 : 5;
    float rA = lev == 0 ? 0.08 : 0.8, rB = lev == 0 ? 0.8 : 5.0;
    float nDens = lev == 0 ? 600.0 : 6.0;            // associations per kpc^3 (near: all ages; far: rich young only)
    // expected associations per sr along this line of sight (4-point log integral)
    float lamSr = 0.0;
    float wq[4];
    for (int q = 0; q < 4; q++) {
      float r = rA * pow(rB / rA, (float(q) + 0.5) / 4.0);
      float dr = r * log(rB / rA) / 4.0;
      wq[q] = nDens * rhoYoung(galPos(g, r)) * r * r * dr;
      lamSr += wq[q];
    }
    lamSr *= 0.3 + 0.7 * min(cloudC, 3.0);           // star formation follows the clouds
    float n = exp2(float(L));
    ivec2 ij = ivec2(clamp(floor((a * 0.5 + 0.5) * n), 0.0, n - 1.0));
    for (int di = -1; di <= 1; di++) for (int dj = -1; dj <= 1; dj++) {
      ivec2 c = ij + ivec2(di, dj);
      if (c.x < 0 || c.y < 0 || float(c.x) >= n || float(c.y) >= n) continue;
      vec2 ac = (vec2(c) + 0.5) / n * 2.0 - 1.0;
      vec3 h = rnd3(cellKey(face, L, c, 20 + lev));
      if (h.x > 1.0 - exp(-lamSr * cellSolidAngle(ac, n))) continue;
      vec3 h2 = rnd3(cellKey(face, L, c, 22 + lev)), h3 = rnd3(cellKey(face, L, c, 24 + lev));
      vec3 cd = cubeDir(face, (vec2(c) + 0.2 + 0.6 * h2.xy) / n * 2.0 - 1.0);
      // distance ~ young density * r^2
      float pick = h2.z * (wq[0] + wq[1] + wq[2] + wq[3]);
      int qi = pick < wq[0] ? 0 : pick < wq[0] + wq[1] ? 1 : pick < wq[0] + wq[1] + wq[2] ? 2 : 3;
      float r = rA * pow(rB / rA, (float(qi) + h3.x) / 4.0);
      float ageMyr = lev == 0 ? pow(10.0, h3.y * 3.0) : 1.0 + 7.0 * h3.y;
      float nStar = lev == 0 ? pow(10.0, 1.5 + 1.8 * h3.z) : pow(10.0, 3.0 + 1.3 * h3.z);
      float mmax = min(pow(10000.0 / ageMyr, 0.4), 80.0);        // main-sequence turn-off
      float m1 = massAtTail(0.5 / nStar, mmax);                    // expected most massive member
      float rc = 0.0015 * pow(nStar / 300.0, 0.3) * (0.6 + 0.8 * h.y);   // Plummer radius (kpc)
      float ac_ = rc / r;                                          // angular (rad)
      float cloudHere = cloudC;                                    // smooth field: same cloud as this pixel
      bool hii = m1 > 15.0 && ageMyr < 8.0 && cloudHere > 1.3;
      bool refl = !hii && m1 > 2.5 && ageMyr < 60.0 && cloudHere > 1.1;
      // Stromgren radius R_s ~ Q^(1/3): ionising output Q grows with the number of
      // O stars (~ nStar / 1000) and steeply with the top mass; ~10-30 pc for rich clusters
      float rs = hii ? 0.010 * pow(m1 / 40.0, 1.2) * pow(nStar / 3000.0, 0.33) * (0.6 + 0.8 * h.z)
               : refl ? 0.0015 * (0.6 + 0.8 * h.z) : 0.0;
      float thNeb = rs / r;
      float bound = max(5.0 * ac_, 1.6 * thNeb) + 3.0 * gFw;
      vec3 dq = d - cd;
      if (dot(dq, dq) > bound * bound) continue;
      float Av = extinctionTo(r);
      float mu = 5.0 * log(r) / log(10.0) + 10.0;
      vec2 p = tangentCoords(d, cd, h2.x * 6.2832);               // world frame, like psf()
      float lSum = 0.0;
      // brightest members, mass-segregated Plummer positions
      for (int i = 0; i < 8; i++) {
        vec3 hm = rnd3(cellKey(face, L, c, 30 + 8 * lev + i));
        float mi = massAtTail((float(i) + hm.x) / nStar, mmax);
        float app = 4.83 - 8.75 * log(mi) / log(10.0) + mu + Av;
        if (app > uMagLimit + 1.0) continue;
        float rad = ac_ * (0.4 + 0.6 * float(i) / 8.0) / sqrt(max(pow(max(hm.y, 0.02), -0.6667) - 1.0, 0.04));
        float ang = hm.z * 6.2832;
        vec3 sd = normalize(cd + (cos(ang) * gE1 + sin(ang) * gE2) * rad);
        lSum += pow(mi, 3.5);
        stars += blackbody(redden(min(teffMS(mi), 45000.0), Av) * gObsD) * fluxOfMag(app) * psf(sd - d);
      }
      // unresolved remainder of the cluster: Plummer surface brightness
      float meanL = (segL(1.3, 0.08, 0.5) + 0.5 * segL(2.3, 0.5, mmax)) / kroupaTot(mmax);
      float Fglow = fluxOfMag(4.83 + mu + Av) * max(nStar * meanL - lSum, 0.0);   // members not drawn individually
      float th = sqrt(ac_ * ac_ + 0.36 * gFw * gFw);
      float x2 = dot(p, p) / (th * th);
      stars += vec3(1.0, 0.93, 0.85) * Fglow * DIFFUSE_K / (3.14159265 * th * th) / ((1.0 + x2) * (1.0 + x2));
      if (uNebulae > 0.0 && (hii || refl)) {
        // surface brightness is distance independent; only foreground dust dims it
        float sbMag = hii ? 16.5 + 3.0 * h.z : 19.0 + 2.0 * h.z;     // V mag / arcsec^2 (HII 16.5-19.5, reflection 19-21)
        float R = 0.0165 * pow(10.0, -0.4 * (sbMag - 21.0));        // radiance of that surface brightness (21 mag/arcsec^2 = 0.0165)
        float seed = floor(h3.x * 997.0);
        vec3 e = hii ? hiiRegion(p / thNeb, seed, smoothstep(25.0, 50.0, m1)) * pow(10.0, -0.4 * 0.81 * Av)
                     : reflectionNebula(p / thNeb, seed, blackbody(teffMS(m1))) * pow(10.0, -0.4 * Av);
        neb += e * R;
        debugNeb.r += length(e) * 0.5;
      }
    }
  }
}

// ---- extragalactic field ----
// Euclidean number counts N(<m) = 1500 sr^-1 10^(0.6 (m - 15)), modulated by a
// lognormal far-field density (a fast statistical stand-in for large-scale
// structure, not an N-body result) and dimmed/hidden by Galactic extinction
// (A_B = 1.32 A_V shifts the counts by 10^(-0.6 A_B)).
float farField(vec3 g) {
  float G = 0.75 * gaussField(g, 2.2, 3, 0.0, 200u) + 0.66 * gaussField(g, 9.0, 2, 0.3, 210u);
  return exp(G - 0.5);
}
vec3 galaxyProfile(vec2 x, float typeU, vec3 hB, vec3 hC) {
  // x in units of the (PSF-broadened) half-light radius; returns profile with unit integral
  if (typeU < 0.3) {                                         // elliptical: Sersic n = 4
    float q = mix(1.0, 0.5, hB.z);
    float R = length(vec2(x.x, x.y / q));
    float I = exp(-7.669 * (pow(max(R, 0.02), 0.25) - 1.0));
    return vec3(1.0, 0.84, 0.66) * I / (22.67 * q);
  }
  float ci = hB.z;                                             // cos(inclination), isotropic orientations
  float q = sqrt(ci * ci * (1.0 - 0.0225) + 0.0225);           // intrinsic thickness 0.15
  float h = 1.0 / 1.678;                                       // exponential scale length / R_e
  vec2 xd = vec2(x.x, x.y / q);
  float R = length(xd);
  float disc = exp(-R / h) / (6.2832 * h * h * q);
  float bt = typeU < 0.45 ? 0.5 : 0.15 + 0.2 * hC.y;           // bulge-to-total
  float hb = 0.12;
  float bulge = exp(-length(vec2(x.x, x.y / max(q, 0.7))) / hb) / (6.2832 * hb * hb * max(q, 0.7));
  vec3 col = vec3(1.0, 0.82, 0.62) * bulge * bt;
  if (typeU > 0.45) {                                          // spiral: logarithmic arms, patchy star formation
    float s = floor(hC.x * 997.0);
    float arms = 2.0 + floor(hC.z * 2.99), pitch = mix(0.2, 0.5, hC.y);
    float ph = atan(xd.y, xd.x) - log(R + 0.05) / pitch;
    float pat = pow(0.5 + 0.5 * cos(arms * ph + 1.2 * gnoise(vec3(xd * 3.0, s), 50u)), 3.0);
    float knots = smoothstep(0.25, 0.6, gnoise(vec3(xd * 9.0, s + 1.0), 51u)) * pat;
    float armMod = mix(1.0, 0.35 + 2.0 * pat, ci);               // arms wash out edge-on
    float lane = (1.0 - ci) * exp(-pow(x.y / (0.04 + 0.1 * q), 2.0)) * smoothstep(2.5, 0.5, abs(x.x));
    col += (vec3(0.62, 0.75, 1.0) * armMod + vec3(1.0, 0.45, 0.6) * knots * 0.8) * disc * (1.0 - bt);
    col *= exp(-2.5 * lane);
  } else {
    col += vec3(0.85, 0.85, 0.9) * disc * (1.0 - bt);          // lenticular / smooth disc
  }
  return col;
}

vec3 extragalacticField(vec3 g, vec3 d, float AvTot, out float density) {
  density = farField(g);
  if (uGalaxies <= 0.0) return vec3(0.0);
  int face; vec2 a;
  cubeCoords(d, face, a);
  vec3 col = vec3(0.0);
  float hide = pow(10.0, -0.6 * 1.32 * AvTot);
  for (int b = 0; b < 3; b++) {
    float ma = b == 0 ? 3.0 : b == 1 ? 10.5 : 13.0;
    float mb = b == 0 ? 10.5 : b == 1 ? 13.0 : 15.5;
    int L = b == 0 ? 3 : b == 1 ? 5 : 7;
    float nSr = 1500.0 * uGalaxies * (pow(10.0, 0.6 * (mb - 15.0)) - pow(10.0, 0.6 * (ma - 15.0))) * density * hide;
    float n = exp2(float(L));
    ivec2 ij = ivec2(clamp(floor((a * 0.5 + 0.5) * n), 0.0, n - 1.0));
    for (int di = -1; di <= 1; di++) for (int dj = -1; dj <= 1; dj++) {
      ivec2 c = ij + ivec2(di, dj);
      if (c.x < 0 || c.y < 0 || float(c.x) >= n || float(c.y) >= n) continue;
      vec2 ac = (vec2(c) + 0.5) / n * 2.0 - 1.0;
      float lam = nSr * cellSolidAngle(ac, n);
      vec3 h = rnd3(cellKey(face, L, c, 60));
      float e = exp(-lam);
      int cnt = int(h.x > e) + int(h.x > e * (1.0 + lam));
      for (int i = 0; i < 2; i++) {
        if (i >= cnt) break;
        vec3 s = rnd3(cellKey(face, L, c, 61 + i));
        vec3 hB = rnd3(cellKey(face, L, c, 63 + i)), hC = rnd3(cellKey(face, L, c, 65 + i));
        vec3 cd = cubeDir(face, (vec2(c) + s.xy) / n * 2.0 - 1.0);
        float m = ma + log(1.0 + s.z * (exp(1.3816 * (mb - ma)) - 1.0)) / 1.3816;   // dN/dm ~ 10^(0.6 m)
        float M = -20.5 + 1.6 * (hB.x + hB.y - 1.0);               // around M*, roughly Gaussian
        float rMpc = pow(10.0, (m - M - 25.0) / 5.0);
        float z = 70.0 * rMpc / 299792.0;
        float Re = 0.004 * pow(10.0, -0.2 * (M + 20.5)) * (0.6 + 0.8 * h.y);   // Mpc, R ~ L^0.5
        float th = sqrt(pow(Re / rMpc, 2.0) + 0.36 * gFw * gFw);  // PSF-broadened angular scale
        vec3 dq = d - cd;
        if (dot(dq, dq) > 100.0 * th * th) continue;
        vec2 p = tangentCoords(d, cd, hC.x * 6.2832) / th;
        vec3 prof = galaxyProfile(p, h.z, hB, hC);
        // redshift-inspired colour attenuation and (1+z)^-4 surface dimming (visual approximation)
        prof *= mix(vec3(1.0), vec3(1.0, 0.7, 0.45), min(z * 3.0, 1.0)) / pow(1.0 + z, 4.0);
        col += prof * fluxOfMag(m) * DIFFUSE_K / (th * th);
      }
    }
  }
  return col;
}

vec3 heat(float x) { x = clamp(x, 0.0, 1.0); return vec3(smoothstep(0.0, 0.6, x), smoothstep(0.3, 0.9, x), smoothstep(0.7, 1.0, x)) + vec3(0.05, 0.0, 0.15) * (1.0 - x); }

// Radiance from world direction d (an escaped geodesic):
//   I = I_stars,resolved + (I_unresolved) e^-tau + I_emission + I_scattered + I_galaxies
vec3 sky(vec3 d, float fw) {
  if (uDeepSky <= 0.0 && uSkyDebug == 0) return vec3(0.0);
  vec3 g = vec3(dot(d, GAL_E), dot(d, GAL_N), dot(d, GAL_C));
  buildLOS(g);
  float AvTot = 1.086 * losTau;

  if (uSkyDebug == 1) {                                       // population column densities
    vec4 col4 = vec4(0.0);
    for (int k = 0; k < NSHELL; k++) {
      vec3 P = galPos(g, shR[k]); float dr = shR[k] * SHELL_DLN;
      col4 += vec4(rhoThin(P), rhoThick(P), rhoBulge(P), rhoHalo(P) * 30.0) * dr;
    }
    // each population's column, log-normalised to its own range
    // hue = which population dominates the column, brightness = log total
    vec4 v = log(1.0 + col4) / log(1.0 + vec4(6.0, 1.5, 250.0, 1.0));
    vec3 hue = (vec3(0.2, 0.4, 1.0) * v.x + vec3(0.15, 1.0, 0.3) * v.y + vec3(1.0, 0.35, 0.1) * v.z + vec3(0.85, 0.2, 1.0) * v.w) / max(v.x + v.y + v.z + v.w, 1e-4);
    return hue * max(max(v.x, v.y), max(v.z, v.w));
  }
  if (uSkyDebug == 2) return heat(AvTot / 6.0);
  if (uSkyDebug == 3) return heat((log(max(countBetween(-3.0, uMagLimit), 1.0)) / 2.302585 - 3.0) / 4.0);   // log10 N/sr, 1e3 .. 1e7
  if (uSkyDebug == 5) { float dens; extragalacticField(g, d, AvTot, dens); return heat(dens * pow(10.0, -0.6 * 1.32 * AvTot) / 4.0); }

  float unres;
  vec3 col = vec3(0.0);
  vec3 stars = uStars > 0.0 ? stellarBackground(d, uMagLimit, unres) : vec3(0.0);
  if (uStars <= 0.0) unres = 0.0;
  float unext;
  float S = diffuseStarlight(uMagLimit, unext) + unres;
  // integrated starlight (Milky Way) and dust-scattered diffuse galactic light
  col += vec3(1.0, 0.93, 0.84) * S * DIFFUSE_K * uNebula;
  // diffuse galactic light: single scattering of the local mean starlight
  // J_ISRF by the dust column, I = albedo * phase * J (1 - e^-tau)
  col += vec3(0.62, 0.74, 1.0) * 0.3 * J_ISRF * (1.0 - exp(-losTau)) * DIFFUSE_K * uNebula;
  col += stars * uStars;
  vec3 neb = vec3(0.0), dbg = vec3(0.0), cl = vec3(0.0);
  clusterField(g, d, cl, neb, dbg);
  col += cl * uStars + neb * uNebulae;
  if (uSkyDebug == 4) {
    float young = 0.0;
    for (int k = 0; k < 6; k++) young += rhoYoung(galPos(g, shR[k])) * shR[k] * SHELL_DLN;
    return vec3(min(dbg.r * 20.0, 1.0), min(cloudC / 4.0, 1.0) * 0.6, min(young * 3.0, 1.0) * 0.6);
  }
  float dens;
  col += extragalacticField(g, d, AvTot, dens);
  return col * uDeepSky;
}`;


export const RAYTRACE_FS = `#version 300 es
precision highp float;
precision highp int;
in vec2 vUv;
out vec4 fragColor;

uniform vec2  uRes;
uniform vec3  uCamPos;
uniform mat3  uCamRot;
uniform float uTanFov;
uniform float uM;            // black-hole mass
uniform float uTime;         // distant-observer time, units of M
uniform int   uSteps;
uniform float uStepScale;

uniform float uDiskIn, uDiskOut;  // in units of M
uniform float uDiskTemp;          // peak local temperature (K)
uniform float uDiskBright;
uniform float uDiskOpacity;
uniform float uDoppler;           // 0 = Interstellar look, 1 = physical
uniform float uBeaming;           // exponent on g (4 = bolometric)
uniform float uDiskSpeed;
uniform float uShowDisk;
uniform float uCamLapse;          // sqrt(1 - 2M/r_cam)
uniform float uStars, uNebula;     // star brightness, Milky Way intensity
uniform float uDeepSky, uStarDensity;
uniform vec3  uCamVel;            // observer 3-velocity (fraction of c) seen by a local static observer
uniform float uCamGamma;
float gObsD = 1.0;                // observer Doppler factor for the current ray

uniform sampler2D uBlackbody;

uniform int  uNB;
uniform vec4 uBody[16];     // xyz, radius
uniform vec4 uBodyCol[16];  // rgb, emissive
uniform vec4 uBodyAux[16];  // clock rate dtau/dt, id, -, -

${NOISE}

vec3 blackbody(float T) {
  float u = clamp(log(T / 800.0) / log(40000.0 / 800.0), 0.0, 1.0);
  return texture(uBlackbody, vec2(u, 0.5)).rgb;
}

${SKY}

// ---------------------------------------------------------- accretion ---
// Thin Keplerian disk, Novikov-Thorne temperature profile, relativistic
// Doppler + gravitational shift computed from the photon's conserved L_z.
vec4 diskSample(vec3 p, vec3 v) {
  float r = length(p.xz);
  float x = r / uM;
  if (x < uDiskIn || x > uDiskOut) return vec4(0.0);

  // Novikov-Thorne flux F ~ (1 - sqrt(rin/r)) / r^3, peak at r = 49/36 rin
  float xp = uDiskIn * 49.0 / 36.0;
  float Fmax = (1.0 - sqrt(uDiskIn / xp)) / (xp * xp * xp);
  float F = (1.0 - sqrt(uDiskIn / x)) / (x * x * x) / Fmax;
  float Tloc = uDiskTemp * pow(max(F, 1e-4), 0.25);

  // Keplerian angular velocity, prograde about +y
  float omega = sqrt(uM / (r * r * r));
  // photon momentum toward the camera (E normalised to 1 at infinity)
  vec3 n = -v / uCamLapse;
  float Lz = p.z * n.x - p.x * n.z;
  float dop = 1.0 / max(1.0 - omega * Lz, 0.05);
  float grav = sqrt(max(1.0 - 3.0 * uM / r, 1e-4)) / uCamLapse;
  float g = grav * mix(1.0, dop * gObsD, uDoppler);

  // turbulent, differentially rotating texture
  float phi = atan(p.z, p.x);
  float ang = phi + mod(omega * uTime * uDiskSpeed, 6.2831853);
  vec3 q = vec3(cos(ang) * 2.2, sin(ang) * 2.2, x * 0.9 + uTime * 0.0035);
  float turb = fbm(q * vec3(1.0, 1.0, 2.6), 5);
  float fine = fbm(vec3(cos(ang) * 6.0, sin(ang) * 6.0, x * 7.0), 4);
  float rings = 0.8 + 0.2 * sin(x * 7.0 + turb * 9.0);
  float dens = clamp(0.25 + 1.1 * turb * rings + 0.35 * fine - 0.2, 0.0, 1.4);

  float edge = smoothstep(uDiskIn, uDiskIn * 1.06, x) * (1.0 - smoothstep(uDiskOut * 0.55, uDiskOut, x));
  float alpha = clamp(dens * edge * uDiskOpacity, 0.0, 1.0);

  float Tobs = Tloc * g;
  float I = uDiskBright * pow(g, uBeaming) * F * (0.5 + 0.8 * dens);
  vec3 c = blackbody(Tobs) * I * edge;
  return vec4(c * alpha, alpha);
}

vec3 accel(vec3 x, float h2) {
  float r2 = dot(x, x);
  float r = sqrt(r2);
  return -3.0 * uM * h2 * x / (r2 * r2 * r);
}

void main() {
  vec2 uv = (gl_FragCoord.xy / uRes) * 2.0 - 1.0;
  float aspect = uRes.x / uRes.y;
  vec3 dir = normalize(uCamRot * vec3(uv.x * uTanFov * aspect, uv.y * uTanFov, 1.0));

  // Relativistic aberration: the camera direction is measured in the moving
  // observer's frame; boost it into the local static frame the geodesics use.
  float b2 = dot(uCamVel, uCamVel);
  if (b2 > 1e-10) {
    float bm = sqrt(b2);
    vec3 bh = uCamVel / bm;
    gObsD = 1.0 / (uCamGamma * (1.0 - dot(uCamVel, dir)));
    dir = normalize(dir + ((uCamGamma - 1.0) * dot(dir, bh) - uCamGamma * bm) * bh);
  }

  vec3 x = uCamPos;
  vec3 v = dir;
  vec3 hv = cross(x, v);
  float h2 = dot(hv, hv);

  vec3 col = vec3(0.0);
  float alpha = 0.0;
  bool escaped = false;
  float rh = 2.0 * uM;
  float rEsc = max(90.0 * uM, length(uCamPos) * 1.25);

  for (int i = 0; i < 2000; i++) {
    if (i >= uSteps || alpha > 0.995) break;
    float r = length(x);
    if (r < rh * 1.0002) break;
    if (r > rEsc && dot(x, v) > 0.0) { escaped = true; break; }

    // adaptive affine step: coarse far away, fine near the photon sphere
    float dl = uStepScale * 0.07 * r * clamp((r - rh) / (1.5 * uM), 0.08, 1.0);

    // RK4
    vec3 a1 = accel(x, h2);
    vec3 x2 = x + 0.5 * dl * v,  v2 = v + 0.5 * dl * a1;
    vec3 a2 = accel(x2, h2);
    vec3 x3 = x + 0.5 * dl * v2, v3 = v + 0.5 * dl * a2;
    vec3 a3 = accel(x3, h2);
    vec3 x4 = x + dl * v3,       v4 = v + dl * a3;
    vec3 a4 = accel(x4, h2);
    vec3 xn = x + dl / 6.0 * (v + 2.0 * v2 + 2.0 * v3 + v4);
    vec3 vn = v + dl / 6.0 * (a1 + 2.0 * a2 + 2.0 * a3 + a4);

    // body intersection along the segment
    vec3 seg = xn - x;
    float segL = length(seg);
    vec3 sd = seg / segL;
    float tBody = 2.0; int hit = -1;
    for (int b = 0; b < 16; b++) {
      if (b >= uNB) break;
      vec3 oc = x - uBody[b].xyz;
      float bq = dot(oc, sd);
      float cq = dot(oc, oc) - uBody[b].w * uBody[b].w;
      float disc = bq * bq - cq;
      if (disc < 0.0) continue;
      float t = (-bq - sqrt(disc)) / segL;
      if (t >= 0.0 && t <= 1.0 && t < tBody) { tBody = t; hit = b; }
    }

    // disk plane crossing
    float tDisk = 2.0;
    if (uShowDisk > 0.5 && x.y * xn.y < 0.0) tDisk = x.y / (x.y - xn.y);

    if (tDisk < tBody) {
      vec3 p = mix(x, xn, tDisk);
      vec4 d = diskSample(p, normalize(mix(v, vn, tDisk)) * length(v));
      col += (1.0 - alpha) * d.rgb;
      alpha += (1.0 - alpha) * d.a;
    }
    if (hit >= 0) {
      vec3 p = x + seg * tBody;
      vec3 c = uBody[hit].xyz;
      vec3 nrm = normalize(p - c);
      vec3 L = normalize(-c);
      float diff = max(dot(nrm, L), 0.0);
      float tex = 0.75 + 0.25 * fbm(nrm * 3.0 + uBodyAux[hit].y, 4);
      float bands = 0.85 + 0.15 * sin(nrm.y * 14.0 + tex * 6.0);
      vec3 base = uBodyCol[hit].rgb * tex * bands;
      float rate = uBodyAux[hit].x;                  // gravitational dimming
      vec3 lit = base * (diff * 1.4 + 0.05) * 40.0 / max(dot(c, c), 16.0);
      vec3 emi = base * uBodyCol[hit].a * (0.6 + 0.4 * pow(1.0 - abs(dot(nrm, -sd)), 0.5) );
      vec3 bc = (lit + emi * 3.0) * pow(rate, 3.0);
      col += (1.0 - alpha) * bc;
      alpha = 1.0;
      break;
    }
    x = xn; v = vn;
  }

  // Sky footprint from the screen-space derivative of the escaped direction:
  // this includes lensing (de)magnification, so stars stay anti-aliased even
  // where the image is squeezed against the photon ring. Computed outside any
  // branch so the derivatives are well defined.
  vec3 vEsc = normalize(v);
  float basePx = 2.0 * uTanFov / uRes.y;
  vec3 jx = dFdx(vEsc), jy = dFdy(vEsc);
  setFootprint(jx, jy, basePx);
  float fw = clamp(sqrt(max(dot(jx, jx), dot(jy, jy))), basePx * 0.02, basePx * 60.0);
  // intensity ~ D^3 for the observer's motion; star colours shift via T * D
  vec3 bg = escaped ? sky(vEsc, fw) * gObsD * gObsD * gObsD : vec3(0.0);
  col += (1.0 - alpha) * bg;
  fragColor = vec4(col, 1.0);
}`;

// ------------------------------------------------------------- post FX ---
export const DOWNSAMPLE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uTex; uniform vec2 uTexel; uniform float uThreshold; uniform int uFirst;
void main() {
  vec2 t = uTexel;
  vec3 a = texture(uTex, vUv + t * vec2(-2, 2)).rgb, b = texture(uTex, vUv + t * vec2(0, 2)).rgb, c = texture(uTex, vUv + t * vec2(2, 2)).rgb;
  vec3 d = texture(uTex, vUv + t * vec2(-2, 0)).rgb, e = texture(uTex, vUv).rgb, f = texture(uTex, vUv + t * vec2(2, 0)).rgb;
  vec3 g = texture(uTex, vUv + t * vec2(-2, -2)).rgb, h = texture(uTex, vUv + t * vec2(0, -2)).rgb, i = texture(uTex, vUv + t * vec2(2, -2)).rgb;
  vec3 j = texture(uTex, vUv + t * vec2(-1, 1)).rgb, k = texture(uTex, vUv + t * vec2(1, 1)).rgb;
  vec3 l = texture(uTex, vUv + t * vec2(-1, -1)).rgb, m = texture(uTex, vUv + t * vec2(1, -1)).rgb;
  vec3 s = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
  if (uFirst == 1) {
    float br = max(s.r, max(s.g, s.b));
    float soft = clamp(br - uThreshold + 0.5, 0.0, 1.0);
    soft = soft * soft * 0.5;
    float w = max(soft, br - uThreshold) / max(br, 1e-4);
    s *= w;
  }
  o = vec4(min(s, vec3(500.0)), 1.0);
}`;

export const UPSAMPLE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uTex; uniform vec2 uTexel; uniform float uRadius;
void main() {
  vec2 t = uTexel * uRadius;
  vec3 s = texture(uTex, vUv).rgb * 4.0;
  s += (texture(uTex, vUv + vec2(t.x, 0)).rgb + texture(uTex, vUv - vec2(t.x, 0)).rgb
      + texture(uTex, vUv + vec2(0, t.y)).rgb + texture(uTex, vUv - vec2(0, t.y)).rgb) * 2.0;
  s += texture(uTex, vUv + t).rgb + texture(uTex, vUv - t).rgb
     + texture(uTex, vUv + vec2(t.x, -t.y)).rgb + texture(uTex, vUv + vec2(-t.x, t.y)).rgb;
  o = vec4(s / 16.0, 1.0);
}`;

export const COMPOSITE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uHdr, uBloom;
uniform float uExposure, uBloomStrength, uTime, uDim;
vec3 aces(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
float h12(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
void main() {
  vec3 hdr = texture(uHdr, vUv).rgb;
  vec3 bloom = texture(uBloom, vUv).rgb;
  vec3 c = (hdr + bloom * uBloomStrength) * uExposure * uDim;
  c = aces(c);
  vec2 q = vUv - 0.5;
  c *= 1.0 - dot(q, q) * 0.55;
  c = pow(c, vec3(1.0 / 2.2));
  c += (h12(gl_FragCoord.xy + fract(uTime) * 917.0) - 0.5) / 255.0 * 1.5;
  o = vec4(c, 1.0);
}`;

// Debris particles, drawn additively into the HDR buffer.
export const POINTS_VS = `#version 300 es
in vec3 aPos; in float aRadius; in float aKind; in float aRate; in float aColor;
uniform mat4 uViewProj; uniform vec3 uCam; uniform float uM; uniform float uPx; uniform float uShadowSin;
out vec3 vCol; out float vA;
uniform vec3 uCamVel; uniform float uCamGamma;
vec3 aberrate(vec3 p, vec3 cam) {
  vec3 rel = p - cam; float dist = length(rel); vec3 d = rel / dist;
  float b = length(uCamVel);
  if (b < 1e-6) return p;
  vec3 bh = uCamVel / b;
  d = normalize(d + ((uCamGamma - 1.0) * dot(d, bh) + uCamGamma * b) * bh);
  return cam + d * dist;
}
void main() {
  vec3 d = aPos - uCam;
  float dist = length(d);
  // cull debris hidden behind the shadow
  vec3 toBH = -uCam; float camR = length(uCam);
  float s = length(cross(normalize(d), toBH / camR));
  bool hidden = dot(d, toBH) > 0.0 && dist > camR && s < uShadowSin;
  if (aKind < 0.5 || hidden || length(aPos) < 2.0 * uM) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; return; }
  gl_Position = uViewProj * vec4(aberrate(aPos, uCam), 1.0);
  gl_PointSize = clamp(uPx * 0.1 / dist, 1.2, 10.0);
  if (aColor > 1.5) gl_PointSize = min(gl_PointSize, 3.0);
  float rate = clamp(aRate, 0.0, 1.0);
  float hot = clamp(4.0 * uM / length(aPos), 0.0, 1.0);
  vec3 base = aColor < 0.5 ? vec3(1.0, 0.55, 0.25) : aColor < 1.5 ? vec3(0.55, 0.75, 1.0) : vec3(0.8, 0.62, 0.45);
  float lum = aColor > 1.5 ? 0.22 : 1.0;   // ambient gas is faint
  vCol = mix(base, vec3(1.0, 0.92, 0.8), hot) * (1.4 + 4.0 * hot) * rate * rate * lum;
  vA = 1.0;
}`;
export const POINTS_FS = `#version 300 es
precision highp float;
in vec3 vCol; in float vA; out vec4 o;
void main() {
  vec2 p = gl_PointCoord * 2.0 - 1.0;
  float f = exp(-dot(p, p) * 3.0);
  o = vec4(vCol * f, 1.0);
}`;

// Lines for trails and the spacetime-fabric embedding grid (screen pass).
export const LINES_VS = `#version 300 es
in vec3 aPos; in vec4 aCol;
uniform mat4 uViewProj; uniform vec3 uCam;
out vec4 vCol;
uniform vec3 uCamVel; uniform float uCamGamma;
vec3 aberrate(vec3 p, vec3 cam) {
  vec3 rel = p - cam; float dist = length(rel); vec3 d = rel / dist;
  float b = length(uCamVel);
  if (b < 1e-6) return p;
  vec3 bh = uCamVel / b;
  d = normalize(d + ((uCamGamma - 1.0) * dot(d, bh) + uCamGamma * b) * bh);
  return cam + d * dist;
}
void main() { gl_Position = uViewProj * vec4(aberrate(aPos, uCam), 1.0); vCol = aCol; }`;
export const LINES_FS = `#version 300 es
precision highp float;
in vec4 vCol; out vec4 o;
void main() { o = vCol; }`;
