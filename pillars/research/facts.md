# Pillars of Creation: physics fact sheet

The reference sheet for the essay on the Pillars of Creation in the Eagle Nebula (M16 / NGC 6611 / IC 4703).
Writers: **use the numbers in the canonical table at the end.** The sections above it show where each number comes from.

Conventions: cgs units unless stated. 1 pc = 3.086×10¹⁸ cm = 3.262 ly. All arithmetic was checked with `facts_check.py` in this folder (stdlib python3, run `python3 facts_check.py`).
Confidence tags: **[P]** = peer-reviewed value, read from the abstract or full text. **[S]** = secondary or press source. **[C]** = our own calculation from the inputs shown. **[E]** = rough estimate, not from the literature.

Distance caveat: many older papers and every NASA/STScI press release use **2 kpc (6,500 ly)**. The modern Gaia value is **1.74 kpc**. Sizes scale as d, masses as d², and mass-loss rates (McLeod 2015 method) as d. Where a source used 2 kpc, both values are shown.

---

## A. Location & distance

| Quantity | Value | Source |
|---|---|---|
| RA, Dec (J2000, M16/NGC 6611 centre) | 18ʰ18ᵐ48ˢ, −13°49′ | [S] https://en.wikipedia.org/wiki/Eagle_Nebula ; https://www.messier-objects.com/messier-16-eagle-nebula/ |
| Galactic l, b | l = 16.95°, b = +0.79° | [C] IAU J2000→Galactic rotation of the RA/Dec above |
| Constellation | Serpens (Serpens Cauda) | [S] Wikipedia, above |
| Apparent magnitude (cluster + nebula) | V ≈ 6.0–6.4 | [S] 6.0 messier-objects.com ; 6.4 Wikipedia |
| Brightest star HD 168076 | V = 8.24 | [S] messier-objects.com ; Wikipedia |
| Angular size, nebula | ~30′ (bright part) up to 70′×50′ (full emission; Wikipedia flags this "citation needed") | [S] |
| Angular size, cluster NGC 6611 | ~7′ | [S] messier-objects.com |
| Angular size, pillars | ~2.5–3′ tall; MUSE mosaic covering them is 3′×3′ | [P] McLeod et al. 2015 https://arxiv.org/abs/1504.03323 ; [C] 5 ly at 2 kpc → 2.6′ |
| Scale | 1′ = 0.506 pc at 1.74 kpc (0.582 pc at 2 kpc) | [C] |

**Distance estimates**

| Estimate | Method | Source |
|---|---|---|
| 2.0 ± 0.1 kpc | spectroscopic/photometric, anomalous R_V ≈ 3.75 | [P, via citing papers] Hillenbrand et al. 1993, AJ 106, 1906 https://ui.adsabs.harvard.edu/abs/1993AJ....106.1906H |
| 1.8 ± 0.1 kpc | spectroscopic parallaxes of 24 OB stars | [P] Dufton et al. 2006, A&A 457, 265 (quoted in Stoop+2023) |
| 3.2 ± 0.3 kpc | old photometry (obsolete) | Walker 1961; Sagar & Joshi 1979 (quoted in Stoop+2023) |
| **1.74 (+0.13/−0.12) kpc** | Gaia DR2 mean parallax 0.57 ± 0.04 mas | [P] Kuhn et al. 2019, ApJ 870, 32 https://arxiv.org/abs/1807.02115 ; adopted by Karim et al. 2023 (SOFIA) |
| 1.706 ± 0.007 kpc (statistical only) | Gaia EDR3, 137 members | [P] Stoop et al. 2023, A&A 670, A108 https://arxiv.org/abs/2207.08452 |

**Recommendation:** use **1.74 kpc ≈ 5,700 ly**. It is the Gaia value most widely adopted (Kuhn 2019, SOFIA papers), and EDR3 agrees with it within errors. The 1.706 ± 0.007 kpc error bar is statistical only; Gaia's systematic parallax error alone is larger. If a writer quotes NASA's "6,500 light-years", they should note that it is the older 2 kpc value.

**Light travel time** [C]: 1.74 kpc × 3,262 ly/kpc = **5,680 ly**, so the light we see left about **5,700 years ago**. (1.706 kpc → 5,560 yr; 2 kpc → 6,520 yr.)

**Extinction toward NGC 6611**

- Mean A_V ≈ **2.6 mag** for cluster members, with A_V/E(B−V) ≈ 3.27 [P] Guarcello et al. 2007, A&A 462, 245 https://arxiv.org/abs/astro-ph/0610401 (value as quoted in later papers).
- E(B−V) varies from 0.5 to 1.1 across the field; R_V reported 3.5–4.8, typical 3.75 [P, via review] Oliveira 2008 https://arxiv.org/abs/0809.3735 ; Hillenbrand 1993.
- Stoop et al. 2023 find two populations: the 1.3 Myr population has A_V ≈ 3.5 and the 7.5 Myr population A_V ≈ 2.0 (as quoted; https://arxiv.org/abs/2207.08452).
- On the pillar skins, the Balmer decrement gives A_V ≈ 1.5–3.2 (highest at P1a, lowest at P3) [P] McLeod et al. 2015.
- **Recommend: A_V ≈ 2.5–3.5 mag toward the cluster; say "about 3 magnitudes".**

---

## B. NGC 6611, the cluster

| Quantity | Value | Source |
|---|---|---|
| Age, main (young) population | 1.3 ± 0.2 Myr | [P] Stoop et al. 2023 |
| Age, older population | 7.5 ± 0.4 Myr | [P] Stoop et al. 2023 |
| Age, commonly quoted | ~2 Myr (literature range 1–3 Myr) | [P] Hillenbrand 1993 (2 Myr); Bonatto et al. 2006 (1.3 ± 0.3 Myr) https://arxiv.org/abs/astro-ph/0509805 ; Oliveira 2008 (2–3 Myr); Karim et al. 2025 (~2 Myr) |
| Number of O stars | ~19 | [P] Stoop et al. 2023 |
| Observed stellar mass | ~1,600 M☉ (observed MS + PMS, a lower limit) | [P] Bonatto et al. 2006 |
| Total mass (IMF-extrapolated) | ~10⁴ M☉ (Karim et al. 2025) to ~2×10⁴ M☉ (Karim et al. 2023) | [P] https://arxiv.org/abs/2511.03978 ; https://arxiv.org/abs/2309.14637 |
| X-ray sources | >1,700; 1,183 identified as young stars | [S] Chandra 2018 release https://chandra.si.edu/photo/2018/m16/ ; [P] Guarcello et al. 2012, ApJ 753, 117 |

**Recommend: "about 2 million years old", "~10⁴ solar masses", "about 19 O stars".**

**Dominant O stars.** Types are from Stoop et al. 2023 (S) and Hillenbrand et al. 1993 (H), as tabulated in Karim et al. 2025, Appendix B, https://arxiv.org/html/2511.03978. Teff, L and Q are **[C] from the Martins, Schaerer & Hillier 2005 observational class-V calibration** (A&A 436, 1049; https://arxiv.org/abs/astro-ph/0503346, Table 4). They assume dwarfs, so they are approximate for giants and binaries.

| Star (W# = Walker 1961) | Type (S) | Type (H) | Teff (K) | log L/L☉ | Q (s⁻¹) |
|---|---|---|---|---|---|
| **W205 = HD 168076** (binary) | O3.5 V((f+)) + O7.5 V | O5 V((f*)) | 43,900 + 35,900 | 5.76 + 5.05 | 3.9×10⁴⁹ (system) |
| W175 | O4 V((f)) + O7.5 V | O5.5 V((f)) | 42,900 + 35,900 | 5.67 + 5.05 | 3.2×10⁴⁹ |
| W197 | O6.5 V((f)) + B0–1 V | O7 V((f)) | 37,900 | 5.23 | 7.6×10⁴⁸ |
| W401 | O7 V + O8 V | O8.5 V | 36,900 + 34,900 | 5.14 + 4.96 | 8.4×10⁴⁸ |
| W222 | O7 V((f)) | O7 III((f)) | 36,900 | 5.14 | 5.6×10⁴⁸ |
| W246 | O7 II(f) | O7 II(f) | (giant; dwarf values are a lower bound) | ≥5.14 | ≥5.6×10⁴⁸ |
| W314 | O7 V + B0.5 V + B0.5 V | B0 V | 36,900 | 5.14 | 5.6×10⁴⁸ |
| W161 | O8.5 V | O8.5 V | 33,900 | 4.86 | 1.9×10⁴⁸ |
| W166 | O9 V | O8.5 V | 32,900 | 4.77 | 1.2×10⁴⁸ |

- The HD 168076 spectral classification is [P] Sana, Gosset & Evans 2009, MNRAS 400, 1479, https://arxiv.org/abs/0909.0486. Its mass is "~80 M☉" per [S] Wikipedia; Martins' spectroscopic mass for O3.5 V is only ~52 M☉. **Say "tens of solar masses (~50–80 M☉)".**
- **Cluster O-star bolometric luminosity** [C]: ~2.2×10⁶ L☉ (sum of the rows above).

**Total ionizing photon rate Q(H)**

| Value | Source |
|---|---|
| log Q₀ ≈ 49.87 (7.4×10⁴⁹ s⁻¹) | [P] McLeod et al. 2016, MNRAS 455, 4057, as adopted by Flagey et al. 2020, A&A 635, A111 https://arxiv.org/abs/2002.05313 |
| 8.43×10⁴⁹ s⁻¹ (Stoop types) / 6.25×10⁴⁹ s⁻¹ (Hillenbrand types) | [P] Karim et al. 2025, Table 3 https://arxiv.org/abs/2511.03978 |
| L_FUV = 1.04×10⁶ / 9.1×10⁵ L☉ ; L_mech (winds) = 4.1×10³ / 1.9×10³ L☉ ; wind energy over 2 Myr ≈ 10⁵¹ erg | [P] Karim et al. 2025 |
| ~1.1×10⁵⁰ s⁻¹ (upper end: sums Martins dwarf rates for every component, including W314 at O7 V) | [C] |

**Recommend Q = 7×10⁴⁹ s⁻¹ (≈10⁵⁰ to order of magnitude).** HD 168076 alone supplies about 35–40% of it [C].

**Cluster–pillar separation**

- W205 lies "just under 2 pc" north-east of the pillars, in projection at 2 kpc [P] McLeod 2015. At 1.74 kpc that is **~1.7 pc, about 4′** [C].
- Karim et al. 2023: the main pillars are lit by O5–7 stars **~3 pc away in projection**; the brightest members are ~2.5 pc from the P1 "Cap" [P].
- The true 3D distance is larger than the projected one. MUSE places P1a behind the stars and P2/P3 in front (see C).
- **Recommend "about 2 pc (6–7 light-years) in projection, ~2–3 pc true".**

---

## C. Pillar geometry & physical state

Naming (McLeod 2015; Karim 2023): **P1** is the tallest, left pillar, split into **P1a** (behind) and **P1b** (front). **P2** is the middle pillar and **P3** the small right one. **P4** is the "Spire" further away.

| Quantity | Value | Source |
|---|---|---|
| Tallest pillar (P1), incl. faint base | "about 5 light-years" at 2 kpc → **1.3 pc ≈ 4.3 ly at 1.74 kpc** (projected) | [S] NASA/STScI 2015 https://science.nasa.gov/asset/hubble/hubble-revisits-the-famous-pillars-of-creation-to-celebrate-25th-anniversary ; [C] rescaling |
| P2 length | ~1 pc | [P] Karim et al. 2023 |
| P3 body | ~0.38 pc long, ~0.1 pc wide (14″); "wishbone" tails ~0.34 pc each | [P] Karim et al. 2023 |
| P2 head clump | ~0.08 pc diameter | [P] Karim et al. 2023 |
| P1 width | ~0.2–0.3 pc | [E] scaled from images |
| Line-of-sight separation of P1a and P2 heads | ~1 pc | [P] Karim et al. 2023 |
| Inclinations | P1a ≈ 47°, P2 ≈ 40°, P3 ≈ 40° (magnitudes only) | [P] Schneider et al., cited in Karim et al. 2023 |
| **Masses** (H₂ from ¹³CO + atomic from [C II]) | **P1a 103, P1b 78, P2 103, P3 18 M☉; total ≈ 300 M☉** | [P] Karim et al. 2023 (AJ), https://arxiv.org/abs/2309.14637 , Table 4 (at 1.74 kpc) |
| Older total mass | ~200 M☉ | [P] White et al. 1999, A&A 342, 233 (at 2 kpc), as used by McLeod 2015 |
| Molecular density | n(H₂) ≈ 1.3×10⁵ cm⁻³ (average) | [P] Karim et al. 2023 |
| Atomic (PDR) density | n(H) ≈ 1.8×10⁴ cm⁻³ | [P] Karim et al. 2023 |
| Peak column density | N_H up to 2.9×10²³ cm⁻² (P1a centre, CO) | [P] Karim et al. 2023, Table 3 |
| Molecular gas temperature | ~10–20 K in the interiors; warmer at the surfaces (¹²CO peak T_mb ~80 K) | [P] Karim et al. 2023 |
| PDR (atomic surface) temperature | ~100 K | [P] Karim et al. 2023 (background) |
| FUV field at the heads | G₀ ≈ 2,500 (stellar upper limit) or 300–2,000 (from dust) Habing | [P] Karim et al. 2023 |
| **Ionized skin n_e** | >2,000 cm⁻³ at tips, ~2,800 at the interface, <250 in the surrounding H II gas ([S II]) | [P] McLeod 2015 |
| | 1,120–1,800 cm⁻³ | [P] Karim et al. 2023 |
| | 706 cm⁻³ at the Spire (P4) tip | [P] Flagey et al. 2020 |
| **Ionized skin T_e** | 8,000–10,000 K (one interface value ~11,900 K) | [P] McLeod 2015 ; Karim 2023 |
| Pressures (P/k, K cm⁻³) | ionized flow 1.8–3.6×10⁷; molecular total 4.8×10⁷ (thermal 3.2×10⁶, turbulent 1.5×10⁷, magnetic 3.0×10⁷) → roughly in pressure balance | [P] Karim et al. 2023 |
| Ionization stratification | [O III], Hα/[N II] and [S II] peaks offset by ~0.02 pc (~4,000 AU) at the P1 tip | [P] McLeod 2015 ; Hester et al. 1996, AJ 111, 2349 https://ui.adsabs.harvard.edu/abs/1996AJ....111.2349H |
| Ionization front thickness (neutral→ionized transition) | ~1/(n_H σ_ν): σ = 6.3×10⁻¹⁸ cm² at threshold, ~2×10⁻¹⁸ spectrum-averaged → **~1–30 AU** for n_H = 10³–10⁴ | [C] |
| Photoevaporation flow speed | ~8 km/s ([S II] flow) | [P] McLeod 2015 |
| | expected ~c_s(10⁴ K) ≈ 10–12 km/s | [C] |

The ionization front is razor-thin compared with the pillars. What Hubble resolves is the ~0.02 pc *ionized flow* above it.

**3D orientation (McLeod et al. 2015, MUSE).** Line-of-sight order from the observer is P3 (closest), then P2, then P1.

- P1a lies **behind** the ionizing O stars, with its **tip pointing toward us**.
- P1b lies in front of the stars, tip pointing away. (The paper's conclusions say P1b's tip faces the observer, which contradicts its §4.2; treat P1b as uncertain.)
- P2 lies in front of the stars, tip pointing away, tail toward us.
- P3 is closest to us, tip toward us.
- Karim 2023 agrees: P1b and P2 are foreground and point toward the stars, P1a is background, and P3 is back-lit.
- **Safe phrasing: "the pillars are not side by side; they are staggered by about a parsec along our line of sight, some in front of the cluster and one behind it."**

**Velocities**

- Mean ionized-gas velocities relative to the H II region [P] McLeod 2015: P1a −3.4, P1b −5.3, P2 −7.1, P3 −6.6 km/s.
- Bipolar outflow at the P2 tip: lobes at about −20/+10 km/s (relative jet speed ~20 km/s), carrying ~0.29 M☉ Myr⁻¹ [P] McLeod 2015.
- Molecular line widths are ~1–3 km/s, with gradients of −1 to −3 km/s/pc along the pillars [P] Karim 2023.

---

## D. Erosion & fate

**Mass-loss rate**

- McLeod et al. 2015 [P]: Ṁ ≈ M_flow·v/ℓ with L([S II]6731) ≈ 130 L☉, ℓ ≈ 3×10¹⁶ cm and v ≈ 8 km/s, giving **Ṁ ≈ 70 M☉ Myr⁻¹ total**. With M ≈ 200 M☉ this gives a lifetime of **≈ 3 Myr**, which the authors call an upper limit. At 1.74 kpc [C]: Ṁ ≈ 61 M☉/Myr and the lifetime is unchanged to first order.
- Karim et al. 2023 [P]: PDR-flow mass loss of 54 (P1a head), 103 (P1a neck), 14 (P2 head), 38 (P2 neck) and 7 (P3 head) M☉/Myr, giving evaporation times of **~1–2 Myr** for the heads.
- Earlier estimates were ~10⁷ yr for photoevaporation and ~10⁵ yr for dynamical dissipation [P] Pound 1998, ApJ 493, L113, as quoted by Karim 2023.
- Our order-of-magnitude check [C] (section F) gives ~60 M☉/Myr for four ~0.1 pc-radius heads, consistent with McLeod.
- **Recommend "losing roughly 70 solar masses per million years; they will last another ~1–3 million years".**

**The 2007 "already destroyed by a supernova" story**

- In January 2007, NASA/JPL released Spitzer images (N. Flagey, A. Noriega-Crespo) showing a shell of hot dust. The release speculated that a supernova ~6,000–9,000 years ago had sent a shock that toppled the pillars ~6,000 years ago, and that we would keep seeing them for ~1,000 more years. [S] https://www.jpl.nasa.gov/news/famous-space-pillars-feel-the-heat-of-stars-explosion/
- The refereed paper (Flagey et al. 2011, A&A 531, A51, https://arxiv.org/abs/1103.2495) is more cautious. Its first interpretation is a shell fed by photoevaporation flows and pushed by **stellar winds**, with dust shattered in wind-driven shocks. It calls the supernova remnant interpretation "more speculative".

**Why it is disfavoured:**

1. No supernova-remnant signature has been reported. Chandra (Linsky et al. 2007, ApJ 654, 347, https://arxiv.org/abs/astro-ph/0610279 ; Guarcello et al. 2012) catalogued >1,700 point sources around M16 and reported no remnant. A remnant only a few thousand years old should be bright in X-rays and non-thermal radio. [S] Wikipedia summarises a contemporary critique along these lines.
2. Timing. The dominant population is 1.3–2 Myr old. The shortest-lived massive star (120 M☉) lives ~3.0–3.5 Myr (Ekström et al. 2012). So the main cluster should not have produced a supernova yet [C, P]. Stoop et al. 2023 did find an older 7.5 Myr population, so a supernova from that population cannot be strictly excluded on age grounds.
3. Kinematics. The 2014 HST re-imaging and the 2015 MUSE kinematics show ordinary, ongoing photoevaporation with flows of ~8 km/s and no shock disruption. [S] https://science.nasa.gov/asset/hubble/hubble-revisits-the-famous-pillars-of-creation-to-celebrate-25th-anniversary ; [P] McLeod 2015.

**Consensus phrasing:** "A 2007 suggestion that a supernova had already destroyed the pillars has not held up. Later infrared, X-ray and spectroscopic work attributes the hot dust to stellar winds. The pillars are eroding slowly through photoevaporation and should survive another ~1–3 million years. That is roughly when the first supernovae in NGC 6611 are expected."

---

## E. Star formation inside the pillars

- **EGGs.** Hester et al. 1996 coined "evaporating gaseous globules" (EGGs). In their picture, photoevaporation uncovers dense globules and may freeze the masses of protostars still accreting inside them [P] https://ui.adsabs.harvard.edu/abs/1996AJ....111.2349H.
- **EGGs with stars.** McCaughrean & Andersen 2002 (A&A 389, 513, https://arxiv.org/abs/astro-ph/0202025) imaged 73 EGGs with the VLT in the near-infrared. **About 15%**, roughly 11 EGGs, show associated young low-mass stars or brown dwarfs; most are empty. Two of the three HST pillars have "relatively massive" YSOs at their tips [P].
- **X-rays.** None of the X-ray sources coincide with EGGs [P] Linsky et al. 2007. Absorption analysis puts almost all of Chandra's sources in the wider nebula, not inside the pillars [S] Chandra 2018.
- **YSOs.** Water masers and YSOs (e.g. M16ES-2 at the P2 tip) sit in the compressed layers within a few tenths of a pc of the ionization fronts [P] Healy et al. 2004; Linsky 2007. MUSE found a bipolar outflow from an embedded protostar at the P2 tip and a candidate outflow on P1 [P] McLeod 2015.
- **HH 216.** A bipolar Herbig–Haro flow driven by a Class I protostar at the tip of **Pillar IV (the Spire)**, not one of the three famous pillars.
  - JWST shows the driver as a single object (<1000 AU) with an ionized jet extending ~0.16 pc to the south; the data support episodic accretion [P] Dewangan et al. 2024, MNRAS https://arxiv.org/abs/2401.06016 ; Andersen et al. 2004, A&A 414, 969.
- **HST 1995→2014 jet.** A jet in the left pillar grew by about 60 billion miles in ~19 years, "about 450,000 mph" (~200 km/s) [S] NASA release via https://www.sciencedaily.com/releases/2015/01/150105182521.htm.
- **JWST NIRCam (19 Oct 2022).** The wavy crimson features at the pillar edges are **jets and bow shocks from forming stars, glowing in molecular hydrogen (H₂)**. NASA estimates those stars at "a few hundred thousand years old" [S] https://science.nasa.gov/missions/webb/nasas-webb-takes-star-filled-portrait-of-pillars-of-creation/

**Jeans length, Jeans mass and free-fall time [C]**

Formulae, with μ = 2.33 (molecular gas incl. He) and ρ = μ m_H n:

- c_s = √(kT/μm_H)
- λ_J = √(π c_s² / Gρ)
- M_J = (4π/3) ρ (λ_J/2)³ (sphere of diameter λ_J)
- Alternative form: M_J = (5kT/Gμm_H)^{3/2} (3/4πρ)^{1/2}, which is ~1.9× larger. Quote "a few M☉" and never more precision than that.
- t_ff = √(3π/32Gρ)

| T (K) | n (cm⁻³) | λ_J | M_J (sphere / 5kT form) | t_ff |
|---|---|---|---|---|
| 20 | 10⁴ | 0.30 pc (62,000 AU) | 8 / 15 M☉ | 3.4×10⁵ yr |
| 20 | 10⁵ | 0.095 pc (20,000 AU) | 2.6 / 4.8 M☉ | 1.1×10⁵ yr |
| 20 | 1.3×10⁵ (Karim mean) | 0.083 pc | 2.3 / 4.2 M☉ | 0.9×10⁵ yr |
| 10 | 10⁵ | 0.067 pc | 0.9 / 1.7 M☉ | 1.1×10⁵ yr |

The pillar heads are 0.1–0.3 pc across, so they are a few Jeans lengths in size. Each holds tens of Jeans masses at 10⁵ cm⁻³. Their free-fall times (~10⁵ yr) are ≥10× shorter than their remaining photoevaporation lifetime (1–3 Myr), so stars have time to form before the pillars are eroded away.

---

## F. Key physics with worked numbers [C]

Inputs: Q = 7×10⁴⁹ s⁻¹, α_B = 2.6×10⁻¹³ cm³ s⁻¹ (case B, 10⁴ K), d = 2–3 pc.

**1. Strömgren radius.** R_S = (3Q / 4π n² α_B)^{1/3}

| n (cm⁻³) | R_S | recombination time 1/(nα_B) |
|---|---|---|
| 10 | 8.6×10¹⁹ cm = **28 pc** | 12,000 yr |
| 100 | 1.9×10¹⁹ cm = **6.0 pc** (5.8–6.4 pc for Q = 6.25–8.43×10⁴⁹) | 1,200 yr |
| 1000 | 4.0×10¹⁸ cm = **1.3 pc** | 120 yr |

M16's H II region is ~15–35 pc across (30′–70′ at 1.74 kpc), which is consistent with mean n ~ 10–100 cm⁻³. Only ~4% of the ionizing photons are used up within 2 pc at n = 100, so the pillars receive essentially the unattenuated flux.

**2. Ionizing flux at the pillar tips.** F = Q / 4πd²

- d = 2 pc (6.17×10¹⁸ cm): F = 7×10⁴⁹ / (4π × 3.81×10³⁷) = **1.5×10¹¹ photons cm⁻² s⁻¹**
- d = 3 pc: **6.5×10¹⁰ photons cm⁻² s⁻¹**
- For comparison, McLeod 2015's simulations used 5×10⁹, so they under-illuminate M16 by >10×.

**3. Ionization-front base density** (Bertoldi 1989, ApJ 346, 735, https://ui.adsabs.harvard.edu/abs/1989ApJ...346..735B). Ionization balance in a flow of thickness ~r_c gives n_IF ≈ √(F / α_B r_c).

| d | r_c = 0.05 pc | 0.1 pc | 0.2 pc |
|---|---|---|---|
| 2 pc | 1,900 | 1,350 | 950 cm⁻³ |
| 3 pc | 1,270 | 900 | 640 cm⁻³ |

These match the measured 1,100–2,800 cm⁻³ (McLeod 2015; Karim 2023). The simple Strömgren-plus-curvature picture works.

**4. Photoevaporation mass flux.** ṁ ≈ 1.4 m_H n_IF c_s, with the 1.4 accounting for helium and c_s ≈ 10 km/s.

- n_IF = 1,400: 1.4 × 1.67×10⁻²⁴ × 1,400 × 10⁶ = **3.3×10⁻¹⁵ g cm⁻² s⁻¹ ≈ 500 M☉ Myr⁻¹ pc⁻²**
- One head of radius 0.1 pc (π r² = 0.031 pc²) loses ≈ 16 M☉/Myr. Four heads lose ≈ **60 M☉/Myr**, which agrees with McLeod's 70 M☉/Myr.
- Head radii of 0.15 pc would give ~140 M☉/Myr, so the estimate is good to about a factor of 2.

**5. Pressures (P/k, K cm⁻³).**

| Component | Calculation | P/k (K cm⁻³) |
|---|---|---|
| Ionized skin, thermal | 2 n_e k T = 2 × 1,400 × 10⁴ | **2.8×10⁷** (measured: 1.8–3.6×10⁷) |
| Radiation (all frequencies, fully absorbed), d = 2 pc | L/(4πd²c), with L_O-stars ≈ 2.2×10⁶ L☉ | **4.3×10⁶** |
| Radiation, d = 3 pc | | 1.9×10⁶ |
| Molecular, thermal only (n = 1.3×10⁵, T = 20 K) | | 2.6×10⁶ |

- The ionized gas pushes **~7–15× harder** than starlight does directly. The pillars are sculpted by the hot gas boiling off them (the "rocket effect"), not by radiation pressure.
- Thermal gas pressure alone cannot hold up the molecular interior. Turbulent and magnetic support bring the total to ~4.8×10⁷ (Karim 2023), roughly balancing the ionized skin.

**6. Sound speeds** (isothermal; ×1.29 for adiabatic γ = 5/3).

| Gas | T, μ | c_s |
|---|---|---|
| Molecular | 10 K, μ = 2.33 | **0.19 km/s** |
| Molecular | 20 K, μ = 2.33 | 0.27 km/s |
| Atomic PDR | 100 K, μ = 1.27 | 0.8 km/s |
| Ionized | 10⁴ K, μ = 0.61 | **11.6 km/s** |

The ratio is ~60× in speed and ~1,000× in temperature. Heating at the ionization front turns sluggish cold gas into a fast outflow. The ionized gas escapes at ~10 km/s, which is about 1 pc per 100,000 years.

**7. How pillars form** (qualitative, with citations).

- **Shadowing.** A pre-existing dense clump shields the gas behind it from ionizing photons. The ionization front sweeps past on either side, and the shadowed column is left as a tail pointing away from the stars, with the dense head toward them.
  - Bertoldi 1989 gives the photoevaporating-globule theory.
  - Mackey & Lim 2010, MNRAS 403, 714 (https://arxiv.org/abs/0912.1499) show with simulations that shadowing by an inhomogeneous medium forms elephant trunks.
  - Mackey & Lim 2011 (https://arxiv.org/abs/1104.1073) add magnetic fields, which get swept into alignment with the pillar.
- **Rayleigh–Taylor-like instability.** Hot, low-density ionized gas pushes on (effectively "supports") cold, dense gas. The ionization front is then unstable to fingering (Frieman 1954; Spitzer 1954, ApJ 120, 1), and the denser fingers lag behind as pillars.
- **Turbulence.** Gritschneder et al. 2010 (ApJ 723, 971) show that pillars also emerge from turbulent clouds.
- The modern view combines these: pre-existing density structure plus shadowing does most of the work.

**8. O-star lifetimes and when the supernovae come** (Ekström et al. 2012, A&A 537, A146, https://arxiv.org/abs/1110.5049). Lifetimes are total H+He+C burning; the range runs from non-rotating to rotating models.

| Initial mass | Lifetime | Time left after the 1.3–2 Myr cluster age |
|---|---|---|
| 85 M☉ | 3.4–4.0 Myr | 1.4–2.7 Myr |
| 60 M☉ | 3.9–4.8 Myr | 1.9–3.5 Myr |
| 40 M☉ | 4.9–6.1 Myr | 2.9–4.8 Myr |
| 20 M☉ | 8.6–10.4 Myr | 6.6–9.0 Myr |

HD 168076's primary, at ~50–80 M☉, should explode (or collapse) in **~1.5–3.5 Myr**. That is comparable to the pillars' 1–3 Myr evaporation time. **Phrase it as "the pillars may well be gone, or nearly gone, by the time the first supernova goes off".**

---

## G. Observation physics

**Dominant optical lines in M16**

| Line | λ (nm, air) | Origin | Where it peaks |
|---|---|---|---|
| Hα | 656.3 | H recombination | the main H II glow and the ionized skins |
| [O III] | 495.9 / 500.7 | O²⁺, needs >35 eV photons | high-ionization gas, farthest from the pillar surface |
| [N II] | 654.8 / 658.3 | N⁺ | next to Hα |
| [S II] | 671.6 / 673.1 | S⁺ (10.4 eV ionization) | hugs the ionization front; the 6716/6731 ratio gives n_e (critical density ~4×10³ cm⁻³ for 6731) |
| Hβ | 486.1 | H recombination | with Hα; the Hα/Hβ ratio gives extinction (M16 mean 5.93 vs intrinsic 2.86) |

Sources: McLeod 2015 ; Hester 1996 ; Flagey 2020. In the "Hubble palette", [S II] is mapped to red, Hα to green and [O III] to blue.

**Extinction law A_λ/A_V** [C] (Cardelli, Clayton & Mathis 1989, ApJ 345, 245, https://ui.adsabs.harvard.edu/abs/1989ApJ...345..245C ; R_V = 3.1)

| λ | 0.50 µm ([O III]) | 0.55 µm (V) | 0.656 µm (Hα) | 0.672 µm ([S II]) | 1.25 µm (J) | 2.2 µm (K) | 4.4 µm |
|---|---|---|---|---|---|---|---|
| A_λ/A_V | 1.12 | 1.00 | 0.82 | 0.79 | 0.28 | 0.11 | 0.037* / 0.049† |

\* CCM89 is formally valid only to 3.3 µm; the 4.4 µm value is a power-law extrapolation.
† Measured mid-IR law: A_4.5/A_K = 0.43 [P] Indebetouw et al. 2005, ApJ 619, 931 (https://arxiv.org/abs/astro-ph/0406403), giving 0.43 × 0.114 = 0.049. **Use 0.05.**
With R_V = 3.75 (the M16 value), A_Hα/A_V changes only to 0.83.

**What this means for the pillar interiors** [C]

- Karim 2023 column densities, with N_H ≈ 2.2×10²¹ A_V (Güver & Özel 2009, MNRAS 400, 2050, https://arxiv.org/abs/0903.2057), give:
  - P3 head (N_H = 6.7×10²²): A_V ≈ 30
  - P2 head (1.1×10²³): A_V ≈ 50
  - P1a centre (2.9×10²³): A_V ≈ 130
- Through the middle of the pillars, A_V ≈ 30–130 mag (half that to a star buried at the centre).
- At A_V = 50, the extinction is A_Hα ≈ 41 mag (totally opaque), A_K ≈ 5.7 and A_4.5 ≈ 2.4 mag.
- **This is why JWST's NIRCam (1–5 µm) sees stars inside and behind the pillars, while Hubble's visible light sees an opaque silhouette.** The densest cores stay dark even at 4.4 µm.

**Surface brightness**

- Unit conversion [C]: 1 Rayleigh = 10⁶/4π photons cm⁻² s⁻¹ sr⁻¹ = **5.7×10⁻¹⁸ erg s⁻¹ cm⁻² arcsec⁻²** at Hα.
- Hα intensity in Rayleighs ≈ EM / 2.75 at T = 10⁴ K, with EM = ∫n_e² dl in pc cm⁻⁶ (Haffner et al. 2003, ApJS 149, 405).
- Model [C]:
  - Diffuse H II region (n_e ≈ 100, path ≈ 5 pc): EM ≈ 5×10⁴ → ~2×10⁴ R
  - Pillar skins (n_e ≈ 1,400–2,000, path ≈ 0.02–0.05 pc): EM ≈ 10⁵ → ~3×10⁴ R
- Measured: Flagey et al. 2020 (SITELLE, Table 2) give an observed integrated Hα flux of 8.2×10⁻¹⁰ erg s⁻¹ cm⁻² in a 3′11″ aperture.
  - Read as a diameter: **2.9×10⁻¹⁴ erg s⁻¹ cm⁻² arcsec⁻² ≈ 5,000 R observed**, or ≈35,000 R after removing A_Hα ≈ 2.1 mag. That matches the model.
  - Read as a radius: 4× lower.
- Spreading that line flux across an R band gives **≈ 20–22 mag arcsec⁻²** [C]. That is comparable to, or fainter than, a dark rural sky (~21.5–22 mag arcsec⁻² in V).

**Would it look brighter if you flew there? No.**

- Surface brightness (radiance) is independent of distance in empty space. Getting closer makes the nebula bigger by d², but the flux per unit solid angle stays the same.
- No telescope can raise surface brightness above the naked-eye value either; it only enlarges the image. That is conservation of étendue (radiance).
- The one real gain from flying in is leaving behind the ~2–3 mag of foreground dust at Hα (A_V ≈ 2.6–3.5). That makes it up to ~10× brighter, still a dim glow.
- Inside the nebula you would be surrounded by a faint sky-wide glow, not a vivid sculpture.

**Human eye**

- The luminance equivalent of ~20–22 mag arcsec⁻² is ~10⁻³–10⁻⁴ cd/m² [C] (L ≈ 10.8×10⁴ × 10^(−0.4μ) cd/m²).
- That is in the **scotopic (rod) range**. Mesopic vision, where cones start to help, spans ~0.005–5 cd/m² (CIE 191:2010), and full colour vision needs ≳3 cd/m².
- Rods are colour-blind and almost blind to Hα. The scotopic sensitivity V′(656 nm) ≈ 5×10⁻⁴ of its peak, against V′(500 nm) ≈ 0.98 (CIE 1951 scotopic and 1924 photopic tables, http://www.cvrl.org/). Photopic V(656) ≈ 0.08.
- So **through an amateur telescope M16 looks like a grey-green haze around a star cluster.** What the eye picks up is mostly [O III] and Hβ, not the red Hα that dominates photographs.
- The pillars themselves "may be visible in 30 cm and larger telescopes under good conditions" as dark intrusions [S] messier-objects.com.
- The red/gold/blue of famous images comes from long exposures and narrowband false-colour mapping.

---

## Recommended canonical numbers (writers must use these)

| Quantity | Canonical value | Say it as | Range / note | Key source |
|---|---|---|---|---|
| Distance | **1.74 kpc** | "about 5,700 light-years" | 1.71–2.0 kpc; NASA PR uses 6,500 ly (2 kpc) | Kuhn+2019 https://arxiv.org/abs/1807.02115 ; Stoop+2023 |
| Light travel time | **~5,700 yr** | "light that left ~5,700 years ago" | | [C] |
| Position | 18ʰ18.8ᵐ, −13°49′; l = 17.0°, b = +0.8°; Serpens | | | Wikipedia ; [C] |
| Cluster brightness | V ≈ 6.0–6.4 | "just below naked-eye visibility, easy in binoculars" | | messier-objects.com |
| Cluster age | **~2 Myr** | "about 2 million years" | 1.3–3 Myr; an older 7.5 Myr subpopulation exists | Stoop+2023 ; Hillenbrand 1993 |
| Cluster mass | **~10⁴ M☉** | "about ten thousand Suns" | observed ≥1,600; 1–2×10⁴ extrapolated | Bonatto+2006 ; Karim+2023/2025 |
| O stars | **~19** | | | Stoop+2023 |
| Brightest star | HD 168076 = W205, O3.5 V + O7.5 V binary, ~50–80 M☉ primary, ~44,000 K | | | Sana+2009 ; Martins+2005 |
| Ionizing photon rate Q | **7×10⁴⁹ s⁻¹** | "~10⁵⁰ ionizing photons per second" | 6.3–8.4×10⁴⁹ | McLeod+2016 ; Karim+2025 |
| Cluster O-star luminosity | ~2×10⁶ L☉ | "two million Suns" | | [C] Martins+2005 |
| Cluster → pillars | **~2 pc projected** | "about 6–7 light-years" | 1.7–3 pc | McLeod+2015 ; Karim+2023 |
| Tallest pillar (P1) | **~1.3 pc** | "about 4 light-years tall" | NASA "5 ly" at 2 kpc incl. base; projected | NASA 2015 ; [C] |
| Pillar masses | P1 ≈ 180 (P1a 103 + P1b 78), P2 ≈ 100, P3 ≈ 20 M☉ | "the biggest holds ~100 Suns' worth of gas" | older total ~200 M☉ (2 kpc) | Karim+2023 |
| Total pillar mass | **~300 M☉** | | 150–300 | Karim+2023 ; White+1999 |
| Interior density | **n(H₂) ~ 10⁵ cm⁻³** | | 10⁴–10⁵ | Karim+2023 |
| Interior temperature | **10–20 K** | | | Karim+2023 |
| Ionized skin | **n_e ~ 2×10³ cm⁻³, T ~ 10⁴ K** | | 700–2,800 cm⁻³; 8,000–12,000 K | McLeod+2015 ; Karim+2023 |
| Flux at pillar tips | **~10¹¹ ionizing photons cm⁻² s⁻¹** | | 0.65–1.5×10¹¹ | [C] |
| Photoevaporation flow speed | **~10 km/s** | | 8 km/s measured | McLeod+2015 |
| Mass-loss rate | **~70 M☉/Myr** | "about 70 Suns per million years" | 60–140 (Karim per-feature) | McLeod+2015 |
| Remaining lifetime | **~1–3 Myr** | "a few million years at most" | | McLeod+2015 ; Karim+2023 |
| Supernova-destruction claim | **disfavoured** | "an early claim that has not held up" | | Flagey+2011 ; see §D |
| First supernova in NGC 6611 | **~1.5–3.5 Myr from now** | | | Ekström+2012 ; [C] |
| EGGs | **73 studied, ~15% (~11) host young stars** | | | McCaughrean & Andersen 2002 |
| Jeans mass / length (20 K, 10⁵ cm⁻³) | **~3–5 M☉, ~0.1 pc** | | | [C] |
| Free-fall time (10⁵ cm⁻³) | **~10⁵ yr** | | | [C] |
| Sound speed cold → ionized | **0.2–0.3 → ~12 km/s** | | | [C] |
| Strömgren radius (n = 100) | **~6 pc** | | 1.3 pc (n = 1000) to 28 pc (n = 10) | [C] |
| A_V to cluster | **~3 mag** | | 2.6–3.5 | Guarcello+2007 ; Stoop+2023 |
| A_V through pillar interiors | **~30–130 mag** | "opaque even to much infrared" | | [C] from Karim+2023 N_H |
| Hα surface brightness | **~10⁴ R intrinsic** | "~20–22 mag/arcsec², about as faint as a dark sky" | ~5,000 R observed | Flagey+2020 ; [C] |
| Brighter if you flew there? | **No** (surface brightness is conserved; only ~2–3 mag of foreground dust is lost) | | | physics |
| Through an eyepiece | **grey-green haze**; pillars barely visible in ≥30 cm scopes | | | CIE scotopic curve ; messier-objects.com |
| JWST red edge features | **H₂ emission from jets and bow shocks of protostars a few ×10⁵ yr old** | | | NASA 2022 |
