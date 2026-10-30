# Findings log

Concrete, checkable results from Schwarzschild Lab and the Schwarzschild Observatory. The reasoning behind them is in the [research log](research-log.md).

Every number here comes from one of three places, marked on each section:

- **[code]** a constant or formula in the source;
- **[README]** a measurement recorded in [`README.md`](../README.md) or [`observatory/README.md`](../observatory/README.md), produced by `observatory/scripts/captures.sh` or the JavaScript calibration port;
- **[re-checked 2026-10-30]** reproduced again for this log, with the command given.

Nothing is estimated. Where a number does not exist, the gap is listed under [Not measured](#not-measured).

---

## 1. Geometry of the hole

| Quantity | Value | Source |
| --- | --- | --- |
| Event horizon | r = 2M | analytic |
| Photon sphere | r = 3M | analytic |
| Critical impact parameter (shadow edge) | b_c = 3√3 M = 5.19615 M | analytic |
| ISCO | r = 6M | analytic; the engine's timelike force law (README) |
| Radiative efficiency at the ISCO | η = 1 − √(8/9) = 0.0572 | [code], [re-checked] |
| Page–Thorne flux peak | r = 9.55 M | [README], unit-tested |
| Newtonian-limit (real-time) flux peak | r = (49/36) r_in | [code] `web/shaders.js` |

## 2. Geodesic validation (Observatory)

`cargo test --release` in `observatory/`. **[re-checked 2026-10-30]**: 60 unit tests passed, 2 ignored (`bench_trace`, `bench_dp_step`, which are benchmarks), plus the preset fixture test passed (all nine presets).

| Check | Result | Source |
| --- | --- | --- |
| Radial rays | escape along r̂ with error < 10⁻¹², r₀ = 2.5 to 10⁶ M | [README], tested |
| Distant-observer shadow | capture edge matches sin α_sh = 3√3√(1 − 2/r₀)/r₀ to 10⁻⁶ for r₀ = 50, 10³, 10⁴, 10⁶ | [README], tested |
| Photon orbit | shadow-edge impact parameter = 3√3 M to 10⁻⁶ at every distance | [README], tested |
| First integral | drift < 1e-8 for b from 5.3 to 150 | [README], tested |
| Weak field | deflection matches 4M/b + (15π/4)(M/b)²; with M → 10⁻¹², escape = launch direction to 10⁻⁹ | [README], tested |
| Strong field | matches Bozza's logarithmic limit at b/b_c − 1 = 1e-4, 1e-6, 1e-8 | [README], tested |
| Tolerance sensitivity | rtol 1e-12 → 1e-7 changes fates only for \|b/b_c − 1\| < 10⁻⁴, directions elsewhere by < 10⁻⁵ rad | [README], tested |
| Winding budget | failed-ray band always contains the analytic edge and narrows > 100× per extra turn | [README], tested |
| Einstein angle | ray mapped exactly behind the hole sits at 2/√r₀ within 2% (r₀ = 10⁴, 10⁶) | [README], tested |
| Extended source, exact alignment | ring at θ_E within 5% | [README], tested |
| Extended source, β = 0.5 θ_E | images at (β ± √(β² + 4))/2 within 5%; flux ratio 2.69 within 10% | [README], tested |
| Disk-crossing order | never decreases toward the critical curve and reaches ≥ 3 | [README], tested |
| **Real-time vs Observatory** | the Lab shader's Cartesian RK4 and the Dormand–Prince integrator agree on escape directions to **3 × 10⁻⁴ rad** | [README], tested |

## 3. Scene scales

All presets use a 4.3 × 10⁶ M☉ hole. **[re-checked 2026-10-30]** by recomputing from the formulas (G = 6.674 × 10⁻¹¹, c = 2.998 × 10⁸, M☉ = 1.989 × 10³⁰):

| Scene | r₀ (M) | Distance | Shadow radius | Einstein radius |
| --- | ---: | ---: | ---: | ---: |
| First Light | 2.0 × 10⁴ | 849 AU | 53.6″ | 48.6′ |
| Lensed H II / Einstein Ring / Narrowband | 1.18 × 10⁵ | 0.0243 pc | 9.08″ | 20.0′ |
| Weak-Lensing Deep Field | 10⁶ | 0.206 pc | 1.07″ | 6.88′ |
| Critical Curve Study (local) | 30 | — | 9.63° | — |

These agree with the README's scene table (850 AU, 0.024 pc, 0.21 pc; 53.6″, 9.1″, 1.1″, 9.6°; 48.6′, 20.0′, 6.9′).

## 4. The synthetic sky (Lab, carried into the Observatory)

**Star counts** [README / devlog, JavaScript port of the shader equations, N₀ = 4.2 × 10⁷ stars kpc⁻³]. All-sky counts brighter than V:

| V | Model | Real sky |
| ---: | ---: | ---: |
| 0 | 4.6 | ~4 |
| 1 | 16.5 | ~15 |
| 3 | 179 | ~170 |
| 6 | 4,640 | ~5,000 |
| 9 | 165,000 | ~120,000 |
| 12 | 2.36 million | ~2.3 million |
| 15 | 30.1 million | ~32 million |

Within about 30% over 15 magnitudes. The largest deviation is V = 9 (+38%). The first run, with a guessed normalisation, was ~2.5× high at every magnitude with the right shape. The Observatory's test suite checks N(<V) within 30% at V = 6 and 12.

The JavaScript calibration port is not in the repository, so the table cannot currently be regenerated from a checked-in script.

**Determinism** [devlog]. Seed 1977 rendered twice from cold starts: maximum difference 2/255 per pixel, which is the film-grain dither of the final pass. Seeds 42 and 314159 from the same camera gave visibly different skies with the same overall brightness.

**Calibration fixes found by checking:**
- Normalisation N₀: first guess ~2.5× too high.
- Nebula surface brightness: first setting ~3 mag too faint, invisible; recalibrated to 16.5–19.5 mag/arcsec².
- Diffuse galactic light uses the model's mean starlight, 2,250 flux units sr⁻¹, computed once in JavaScript.

## 5. The disk and First Light

[README; reported by the code for λ_Edd = 0.01]

| Quantity | Value |
| --- | --- |
| Accretion rate Ṁ | 1.7 × 10⁻³ M☉/yr |
| Peak T_eff | 86,700 K at 9.55 M |
| Thickness H/r | ≤ 0.0065 (thin-disk assumption holds) |
| Radiance at 550 nm | ~5 × 10²⁴ photons s⁻¹ m⁻² sr⁻¹ nm⁻¹, ~70× the Sun's surface |
| Exposure that keeps it below full well | ND 6, 20 µs |
| Background stars at that exposure | ~10¹² too faint to register |
| Sensor dynamic range (full well / read noise) | ≈ 1.7 × 10⁴ |
| HDR ladder | 6 frames, effective exposure stepping ~10³ |
| HDR merge test | rates across ten decades recovered within their noise |

## 6. Photometry (Weak-Lensing Deep Field)

[README, `observatory photometry presets/deep-field-stack.json`]

- Zero point: V = 0 ↔ 1.0 × 10⁸ photons s⁻¹ m⁻² nm⁻¹ at 550 nm, the AB 3631 Jy value within 5% (tested).
- In-field catalogue within 1°: 17,800 stars to V = 22 and 6,900 galaxies to m = 23.5.

| Filter | Sky (e⁻/s/px) | m = 20, 1 × 3600 s | m = 22, 1 × 3600 s | m = 22, 36 × 300 s |
| --- | ---: | ---: | ---: | ---: |
| L | 0.117 | 158 | 60 | 103 |
| R | 0.035 | 92 | 35 | 59 |
| G | 0.043 | 96 | 36 | 61 |
| B | 0.036 | 82 | 31 | 51 |

(peak-pixel S/N). The old 10 cm field already gave m = 22 a peak S/N of 18 in an hour: the dark, noisy first Deep Field was a display error, not a counting error.

## 7. Nebula volume

[README, `observatory inspect volumes/lensed-nebula-96.obsvol`]

| Check | Result |
| --- | --- |
| Strömgren radius, uniform gas | analytic within 8% |
| Grid-scale noise index before filter | Hα 0.42, [S II] 0.97 |
| After one [1, 2, 1]/4 pass | Hα 0.078, [O III] 0.014, [S II] 0.19 |
| Filter response | Nyquist mode removed exactly; 4-voxel wave halved; 16-voxel structure within 4% per axis |
| Uniform emitter–absorber slab | matches (j/α)(1 − e^−τ) to 10⁻³ |
| Step convergence | 1% at half a voxel, 0.3% at a quarter |
| Surface brightness | constant at 1 and 4 kpc while angular size scales as 1/D |
| Foreground extinction to the nebula | A_V ≈ 0.96 at 2.5 kpc, b = −4.4° |

## 8. Sensor and resolution invariance

[README, tested]

- PSF flux conserved to 0.2%.
- Stack noise ∝ 1/√N.
- Across 640 × 400, 1024 × 640 and 1600 × 1000: shadow solid angle within 3%, integrated electrons within 1%, PSF FWHM in arcseconds exact, mixed-fate boundary band shrinking by > 15% per step.

## 9. Performance (Observatory)

[README] Native CLI, 10 threads, **MacBook Air M4** (4P + 6E cores, 16 GB, fanless). The machine throttles; throughput varies up to 2× between runs of the same scene. Image data are deterministic.

| Render (1024 × 640) | spp | Samples/s | Geodesic steps/sample | Sampling time |
| --- | ---: | ---: | ---: | ---: |
| First Light, single exposure | 256 | 2.94 M | 36.4 | 57 s |
| First Light, HDR composite | 256 | 2.60 M | 36.4 | 65 s |
| Lensed H II, offset | 256 | 0.44 M | 30.3 | 380 s |
| … lensing off | 256 | 0.80 M | – | 210 s |
| Einstein Ring, structured | 256 | 0.29 M | 30.3 | 582 s |
| Einstein Ring, exact | 256 | 0.26 M | 30.3 | 651 s |
| Narrowband Nebula | 256 | 0.42 M | 30.3 | 396 s |
| Deep Field, physical frame | 512 | 3.98 M | 29.2 | 84 s |
| Deep Field, stacked | 512 | 3.92 M | 29.2 | 86 s |
| Critical Curve Study (1280 × 800, rtol 1e-10) | 64 | 1.70 M | 49.1 | 39 s |

The volume march dominates: the nebula scenes run 113–160 volume steps per ray in the volume and are about 7–11× slower per sample than First Light (2.94 M vs 0.44–0.26 M samples/s).

**Convergence** (First Light single exposure, 320 × 200):

| spp | Wall time | Renderer noise | Sensor noise |
| ---: | ---: | ---: | ---: |
| 256 | 8.0 s | 0.110% | 0.66% |
| 1,024 | 31.5 s | 0.055% | 0.66% |
| 4,096 | 143.2 s | 0.028% | 0.66% |

Renderer noise halves per 4× samples (1/√N); sensor noise is fixed once the exposure is complete.

**Resolution scaling** (First Light, 16 spp): 640 × 400 in 1.9 s, 1024 × 640 in 5.9 s, 1600 × 1000 in 13.4 s.

**Peak memory** (1024 × 640): First Light HDR 590 MiB, offset nebula 671 MiB, Deep Field stack 684 MiB, Critical Curve Study (1280 × 800) 763 MiB. Mostly per-pixel accumulators (26 spectral + 18 diagnostic channels). No GPU.

**Reproducibility.** A 160 × 100 render to 64 spp in one go and to 32 spp plus a checkpoint resume to 64 give bit-identical EXR files.

## 10. What is approximate

From both READMEs, unchanged in substance:

**Real-time Lab**
- Fixed-step-heuristic RK4 in the shader; no error control (3 × 10⁻⁴ rad from the reference).
- Disk is a Newtonian-limit flux shape with a free peak temperature and brightness; Doppler and beaming are switchable. Disk texture is noise.
- Sky: 10 shells, ≤ 3 stars per cell, single mass–luminosity law plus a fixed giant fraction, no 3D spiral arms, no isochrones, lognormal large-scale structure, no Local Group objects. Nebula shapes are noise. Galaxy redshift reddening and (1+z)⁻⁴ dimming are visual approximations.

**Observatory**
- Hypothetical vantage point: hundreds of AU to 0.2 pc from a Sgr A*-like hole.
- Static observer and stationary scene.
- Thin disk: colour-corrected blackbody, no atmosphere, corona or jet. First Light uses ideal optics with no scattered-light halo; a 1% Moffat halo would let the disk outshine every star within 2′.
- Display curves are presentation choices, recorded in every file. SHO is false colour.
- Asymptotic-sky lensing of the nebula (error ~10⁻⁵ of a voxel).
- Nebula: two-phase temperature, first-order advection, empirical line ratios, single scattering, 96³ voxels with a reconstruction filter.
- Sky: one realisation, faint sources tiled statistically outside the in-field square, 12 shells.
- Sensor: normal approximation above N = 30; no flat field, bias structure or dithering.
- No Kerr.

## Not measured

- **Real-time Lab frame rate.** No fps or GPU timings have been recorded for any device. The devlog and README say "real-time"; that has not been benchmarked.
- **Star-count calibration script.** The JavaScript port used for section 4 is not checked in.
- **Determinism check of the Lab sky** (2/255) was a manual comparison, not an automated test.
