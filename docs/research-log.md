# Research log

What was investigated in Schwarzschild Lab and the Schwarzschild Observatory, why, and what it rests on. It runs in order of the git history. Results and numbers are in the [findings log](findings.md); the story is in the [devlog](https://zahin-tapadar.vercel.app/blog/building-a-black-hole-simulator/).

The history is coarse: the Observatory landed as a single commit (`2f93eef`, 2026-10-25). The order of its internal investigations is not recorded, so its entries below are grouped by topic, not by date.

Units are geometric (G = c = 1) with lengths in black-hole masses M, unless a physical unit is given.

---

## 2026-10-18 — A ray-traced Schwarzschild hole (`7a1c38c`)

**Question.** Can a browser show a black hole whose image *comes from* traced light paths, rather than a dark disc with a ring painted round it, at interactive frame rates?

**Physics it rests on.**

- **Schwarzschild metric** (Schwarzschild 1916): the static, spherically symmetric vacuum solution. Event horizon at r = 2M, photon sphere at r = 3M, innermost stable circular orbit (ISCO) at r = 6M.
- **Null geodesics as a force law.** A photon's path, written in Cartesian coordinates with an affine parameter, obeys ẍ = −3Mh²x / r⁵, where h = |x × ẋ| is conserved along the ray. This is the form used in `web/shaders.js` (`accel`). It is cheap: one vector, one constant per ray.
- **Critical impact parameter** b_c = 3√3 M ≈ 5.196 M. Rays aimed inside it are captured, so the shadow is larger than the horizon (2M) and the photon sphere (3M).
- **Timelike geodesics** for massive bodies: d²x/dτ² = −(Mx/r³)(1 + 3L²/r²), the Newtonian term plus the relativistic correction that makes orbits precess and sets the ISCO. Integrated with classical RK4 in proper time in `engine/src/lib.rs`, with the step limited by the orbital timescale, the crossing time and the distance to the horizon. The engine also computes the Killing energy to convert proper time to a distant observer's time (dτ/dt = (1 − 2M/r)/E), which is what makes bodies redden and freeze at the horizon.
- **Thin accretion disk.** A Novikov–Thorne-inspired flux, F(r) ∝ r⁻³(1 − √(r_in/r)), with T ∝ F^¼. This is the Newtonian-limit shape, used in the real-time renderer for its look; the fully relativistic Page–Thorne form came later, in the Observatory.
- **Doppler and gravitational shift.** The observed-to-emitted frequency ratio is g = √(1 − 3M/r) / (1 − ΩL_z), the redshift of a circular Keplerian orbit (Ω = √(M/r³)) times its Doppler factor. In the shader L_z comes from the photon's direction at the disk crossing, so no extra state is needed per ray. Observed temperature is T·g and intensity is scaled by g⁴ (the beaming exponent is a control, default 4). The Doppler term is off in the Gargantua preset and on in Physical (Doppler).
- **Blackbody colour.** A Planck spectrum integrated against the CIE 1931 colour-matching functions, using the multi-lobe Gaussian fit of Wyman, Sloan & Shirley (2013) in `web/main.js`, baked into a lookup texture.

**Approach.** One ray per pixel, adaptive affine step `dl = 0.07 r · clamp((r − 2M)/1.5M, 0.08, 1)`: coarse far away, fine near the horizon and photon sphere. Default cap of 500 steps per ray. Bodies from the WASM engine are tested as sphere intersections along each RK4 segment (up to 16), and thousands of small gas particles are drawn in a separate additive pass.

**Why RK4 and not something better.** In a fragment shader, a fixed-order explicit method with a heuristic step is the only practical choice: no error estimate, no branch-heavy step rejection. The cost of that choice was measured later, against the Observatory's integrator (see findings: 3 × 10⁻⁴ rad).

## 2026-10-18 — The observer moves (`340cb54`)

**Question.** A static camera looked like a poster. What does a moving observer actually see?

- The camera follows a circular free-fall orbit. Ray directions are transformed by **relativistic aberration** from the observer's frame into the local static frame the geodesics use, and the sky and disk get the observer's Doppler factor.
- Overlays and picking use the same aberration map, so clicks land where the eye says they should.
- A halo of massless gas on inclined geodesics, replenished as it is swallowed.

## 2026-10-23 — The sky (`c9998b0`)

**Question.** Why does every procedural deep-space background look procedural, and what would make one look astronomical?

### Abandoned approaches

1. **A catalogue of landmarks.** Five hand-placed objects (emission nebula, reflection nebula, edge-on galaxy, face-on spiral, compact galaxy with cluster), each drawn with noise at a fixed sky direction. Lensing worked, since they were looked up on the escaped ray direction, but they were obviously authored. *Abandoned: authored, not generated.*
2. **Seeded noise.** A seed tilted the galaxy; hashed sphere cells decided whether they held a nebula or galaxy. Every nebula was a puff the size of its cell, star density was uniform, the galactic band was grey fog painted on top. The seed changed *where*, never *why*. It also exposed two bugs:
   - A cheap float hash correlates on an integer lattice and lined stars up in diagonal rows. Replaced by an integer hash (PCG3D; Jarzynski & Olano 2020).
   - Background galaxies drawn as sky-space ellipses were stretched into streaks by lensing. A galaxy that small should be a point of light, so it now goes through the same PSF as a star.

   *Abandoned: noise drew things instead of modelling why they exist.*
3. **Population synthesis.** Kept. Describe the Milky Way in 3D, count what a telescope would see. The idea comes from the Besançon model (Robin et al. 2003) and the Gaia Universe Model (Robin et al. 2012), shrunk to fit a fragment shader.

### What the sky model rests on

- **Galactic structure.** Four stellar populations in galactocentric kpc (Sun at R₀ = 8.2, z☉ = 0.02): exponential thin and thick discs, a rotated triaxial Gaussian bar/bulge, and a flattened power-law halo. Normalised to N₀ = 4.2 × 10⁷ stars kpc⁻³ locally.
- **Initial mass function.** Kroupa (2001): m⁻¹·³ for 0.08–0.5 M☉, m⁻²·³ for 0.5–1 M☉. For the thin disc's *present-day* mass function above 1 M☉, a steep m⁻⁴·⁸ tail to 60 M☉ (constant star formation, short lifetimes). Old populations stop at a 0.95 M☉ turn-off.
- **Mass–luminosity.** L ∝ m³·⁵, so M_V = 4.83 − 8.75 log₁₀ m, plus 0.3–0.6% giants at M_V ∈ [−1.5, 1.5].
- **Extinction.** An exponential dust disc (τ_V = 0.74 kpc⁻¹ locally, scale height 0.12 kpc) multiplied by a lognormal cloud field C = exp(σG − σ²/2), which keeps the mean dust fixed while letting clouds get thick. Contrast σ is high in the plane, low at high latitude. Reddening E(B−V) = A_V / 3.1. This is the only place noise is allowed in the sky.
- **Line-of-sight integration.** 10 logarithmic shells from 5 pc to 40 kpc. Each shell gets its population weights (∝ ρ r² dr), distance modulus and extinction to its centre.
- **Sampling.** An equal-angle cube-sphere grid, one level per magnitude bin, chosen so each cell expects about one star. Counts are Poisson draws by inverse CDF, capped at 3 per cell; the excess is kept as mean flux. Each star then draws its own shell, population, mass or giant class, temperature and reddening.
- **Diffuse galactic light.** Single scattering of the model's mean starlight by the dust column, I ∝ 0.3·J·(1 − e^−τ).
- **Clusters and nebulae** from the young disc (60 pc scale height). Turn-off mass m_max ≈ (10,000 Myr / age)^0.4. HII regions require a star above 15 M☉, age under 8 Myr and a dense cloud (C > 1.3), sized by a Strömgren-like scaling (Strömgren 1939). Reflection nebulae require a B star, a cloud (C > 1.1) and no ionising source.
- **Galaxies.** Euclidean number counts N(<m) = 1500 sr⁻¹ · 10^0.6(m−15), modulated by a lognormal large-scale-structure field, hidden behind 10^−0.6·A_B. Sérsic n = 4 ellipticals, exponential discs with logarithmic arms. R ∝ L^0.5.
- **Lensing for free.** The sky is evaluated only on the escaped ray direction. Stars are drawn as ~1.6 px screen-space spots through the inverse of the screen-to-sky Jacobian, divided by the pixel's solid angle, so lensing magnification brightens them.

### How the model was checked

The equations were ported to JavaScript and integrated over the whole sky to compare N(<V) with observed counts. The first run used a guessed normalisation and was about 2.5× too high at every magnitude, with the right shape, so only N₀ was changed.

The first nebula pass rendered nothing. The debug view showed clusters passing every condition with invisible nebulae: their surface brightness had been set about 3 magnitudes too faint, from the dim end of real nebulae. Recalibrated to 16.5–19.5 mag/arcsec².

Debug views (population density, A_V, expected star counts, nebula conditions, galaxy density) were added because without them it was not possible to tell which part of the model was wrong.

A side fix in the same commit: ambient gas is fed before packing, so WebAssembly memory growth cannot detach the debris upload buffer.

## 2026-10-25 — The Observatory (`2f93eef`)

**Question.** The real-time renderer answers "what does it look like now?". The Observatory asks "what would a camera record if it pointed at this, and how much of that can we justify?" That question changes almost every design decision: physical scale, an integrator with error control, light as a spectrum, a sensor model, and output labels that say what kind of image each one is.

The devlog's original closing said a fully accurate observatory simulator would be "an enormous project". The Observatory is a first, bounded attempt at one, with its approximations listed.

### Scene scale

A hole of 4.3 × 10⁶ M☉ (Sgr A*-like) at a stated distance, seen by a **static observer**. The observer's distance in M, r₀ = D/r_g, sets the shadow (sin α_sh = 3√3 √(1 − 2/r₀)/r₀, exact for Schwarzschild) and the Einstein radius (θ_E = √(4 r_g D_ls/(D_ol D_os)) → 2/√r₀ for a distant source). A separate **local mode** (r₀ = 30) exists only for geodesic and critical-curve validation and is labelled as such everywhere.

Why these distances: Sgr A* at 8 kpc has a 26 µas shadow. A telescope parked hundreds of AU to 0.2 pc away makes the shadow and lensing resolvable alongside a star field. This is stated as a hypothetical vantage point.

### Ray integration

- **Orbit equation.** In the photon's orbital plane, with u = 1/r: d²u/dφ² = 3Mu² − u, with first integral (du/dφ)² + u²(1 − 2Mu) = 1/b².
- **Integrator.** Embedded Dormand–Prince 5(4) (Dormand & Prince 1980) with adaptive step control, relative tolerance 1e-7 to 1e-11 per scene. Integrating in u makes escape exact: the ray reaches r = ∞ where u = 0, found by Newton iteration. Disk crossings sit at known angles (φ_c + kπ) and steps are clipped to land on them.
- **Why the critical-curve scene uses rtol 1e-10.** Near-critical rays amplify integration error roughly as e^φ over their windings. Image presets use 1e-8.
- **Validation approach.** Each piece is checked against an analytic limit: radial rays, the shadow formula, the photon orbit, the first integral, weak-field deflection 4M/b + (15π/4)(M/b)², the strong-field logarithmic limit of Bozza (2002), the Einstein angle, image parity, disk-crossing order, and point-lens theory for extended sources (image positions (β ± √(β² + 4))/2 and flux ratio). The real-time shader's RK4 is compared with this integrator on escape directions.

### The disk

- **Page–Thorne (1974)** zero-torque thin disk around a Schwarzschild hole, tabulated numerically, with absolute scale from the accretion rate: Ṁ = λ_Edd L_Edd/(ηc²), L_Edd = 1.257 × 10³¹ W (M/M☉), η = 1 − E(r_in) = 1 − √(8/9) ≈ 0.0572. Checked against its Newtonian limit 3GMṀ/(8πR³)(1 − C/√R).
- **Colour correction.** I_ν = f_col⁻⁴ B_ν(f_col T_eff) with f_col = 1.7, the usual hardening factor for accretion-disk atmospheres (Shimura & Takahara 1995).
- **Consequence.** At λ_Edd = 0.01 the disk is about 70 times brighter than the Sun's surface at 550 nm. A real camera would need ND 6 and 20 µs to keep it under full well, and the stars would then be ~10¹² too faint. That is why First Light ships as two labelled products: a physical single exposure and a registered HDR composite.

### HDR merging and display

- Six registered frames, effective exposure stepping ~10³ (the sensor's dynamic range, full well / read noise ≈ 1.7 × 10⁴). Merged per pixel and per filter in linear units, as the maximum-likelihood rate over unsaturated frames, with dark current subtracted.
- One global, monotone display curve (`hdr_log`) with no local tone mapping, bloom or sharpening. The empty intensity range between the brightest star and the faintest disk pixel is found from the histogram and squeezed.
- Broadband colour uses the colour-preserving asinh stretch of Lupton et al. (2004). Narrowband uses an SHO mapping with a *shared* stretch so hue follows line ratios.
- **Considered and set aside:** a 1% Moffat scattered-light halo. With it, the glare of a 10¹⁵ e⁻/s source outshines every star within two arcminutes. A real telescope would show that, so First Light's halo-free optics are recorded as an idealisation rather than hidden.

### Lensing a nebula at a finite distance

- A 24 pc box placed 2.5 kpc behind the hole. Escaped rays continue as straight lines along their asymptotic direction (the thin-lens, asymptotic-sky approximation).
- **Volume generation** (`observatory volume`). A lognormal turbulent cloud (the only noise), ionised by three massive stars against case-B recombination (Osterbrock & Ferland 2006): a ray-cast Strömgren balance, so dense clumps cast neutral shadows. Gas flow from the Euler equations in conservative form with a first-order donor-cell scheme; pressure from the ionisation state. No energy equation, self-gravity or magnetic field. Hα/Hβ from case B; [O III], [N II], [S II] from line ratios tied to the ionisation parameter within observed H II-region ranges.
- **The grid-noise investigation.** `observatory inspect` measures a grid-scale noise index (mean |f − mean of 6 neighbours| / f in the ionised gas). It found an odd–even checkerboard density mode at the Nyquist scale, amplified by n² line emission.
  - *Tried and abandoned:* Rusanov (local Lax–Friedrichs) fluxes diffused the neutral shell into the H II region until it recombined.
  - *Tried and abandoned:* fourth-order hyperdiffusion erased the turbulent structure.
  - *Kept:* a documented reconstruction filter, one separable [1, 2, 1]/4 binomial pass on the cached fields, which removes the Nyquist mode exactly and leaves 16-voxel structure within 4% per axis.
- **Radiative transfer.** Front to back per spectral channel, exact for piecewise-constant emission and absorption, with a series form below Δτ = 10⁻² where single precision would lose 1 − e^−Δτ.

### The deep sky and the Weak-Lensing Deep Field

- The Lab's population model, with two depths: an all-sky layer to V ≈ 9 (rays near the hole can map anywhere) and an in-field deep catalogue on a tangent-plane index.
- **First Deep Field placed exactly on the galactic pole** showed a fan-shaped artefact from the old equirectangular lookup of unresolved light. Replaced by one all-sky map blended to a single value at each pole.
- **First Deep Field was dark and noisy.** `observatory photometry` was written to trace every term from magnitude to electrons (zero point, filter × QE × bin width, collecting area, pixel solid angle, sky background, PSF peak fraction). The audit showed the counts were never the problem — the old 10 cm field already gave m = 22 a peak S/N of 18 in an hour. The display was: its white point sat on saturating stars and its black point far below the sky, so the sky's shot noise filled the frame. Fixed with a 30 cm telescope at 0.94″/px and a display with per-channel black points at the sky level and a highlight shoulder.

### The sensor

Rate per filter from the spectrum, a Gaussian PSF core with FWHM² = FWHM_optics² + (1.03 λ/D)², an optional Moffat halo (FFT-convolved), Poisson shot noise by CDF inversion at a frozen per-pixel uniform, Gaussian read noise, dark current and full-well saturation. Stacking is drawn exactly as one Poisson(nμ) plus √n read noise.

### Output classes

Every render belongs to one class (fixture, validation, presentation, high quality), **earned by the resolution actually rendered**, never declared. Images are never upscaled. Diagnostics carry their legends and fixed scales and are never used as pictures. This was a response to a specific failure mode: a debug render being mistaken for a result.

---

## Open questions

- **Kerr.** A spinning hole needs its own validated integrator. Nothing is called Kerr until it has one.
- **Observer motion in the Observatory.** It uses a static observer and a stationary scene. The real-time Lab has an orbiting observer with aberration; the Observatory does not.
- **Disk physics.** Thin disk with a colour-corrected blackbody: no atmosphere, corona, jet or thick-flow regime. A finite vertical optical depth exists but is experimental.
- **Scattered light.** First Light's halo-free optics are an idealisation. What does an honest First Light look like with a realistic halo?
- **Nebula physics.** No energy equation, first-order advection, empirical line ratios, single scattering, 96³ voxels. The Nyquist filter treats a symptom of the first-order collocated scheme; a better solver is still open.
- **Sky model.** 10 shells (12 in the Observatory), no 3D spiral arms, no isochrones or metallicity, a lognormal stand-in for large-scale structure, no Local Group objects.
- **Real-time vs Observatory agreement.** 3 × 10⁻⁴ rad on escape directions. Is that dominated by the heuristic step near the photon sphere? Not yet investigated.
- **Real-time performance.** No frame-rate or GPU timing measurements of the Lab have been recorded. The step cap (60–2000, default 500), step scale and auto-resolution are exposed as controls, but their cost has not been benchmarked.

## References

- Bozza, V. (2002). Gravitational lensing in the strong field limit. *Phys. Rev. D* 66, 103001.
- Dormand, J. R. & Prince, P. J. (1980). A family of embedded Runge–Kutta formulae. *J. Comput. Appl. Math.* 6, 19–26.
- Jarzynski, M. & Olano, M. (2020). Hash functions for GPU rendering. *JCGT* 9(3).
- Kroupa, P. (2001). On the variation of the initial mass function. *MNRAS* 322, 231–246.
- Lupton, R. et al. (2004). Preparing red-green-blue images from CCD data. *PASP* 116, 133–137.
- Novikov, I. D. & Thorne, K. S. (1973). Astrophysics of black holes. In *Black Holes (Les Astres Occlus)*.
- Osterbrock, D. E. & Ferland, G. J. (2006). *Astrophysics of Gaseous Nebulae and Active Galactic Nuclei*.
- Page, D. N. & Thorne, K. S. (1974). Disk-accretion onto a black hole. *ApJ* 191, 499–506.
- Robin, A. C. et al. (2003). A synthetic view on structure and evolution of the Milky Way. *A&A* 409, 523–540.
- Robin, A. C. et al. (2012). Gaia Universe Model Snapshot. *A&A* 543, A100.
- Schwarzschild, K. (1916). Über das Gravitationsfeld eines Massenpunktes nach der Einsteinschen Theorie.
- Shimura, T. & Takahara, F. (1995). On the spectral hardening factor of the X-ray emission from accretion disks. *ApJ* 445, 780.
- Strömgren, B. (1939). The physical state of interstellar hydrogen. *ApJ* 89, 526.
- Wyman, C., Sloan, P.-P. & Shirley, P. (2013). Simple analytic approximations to the CIE XYZ color matching functions. *JCGT* 2(2).
