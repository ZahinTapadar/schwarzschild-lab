# Pillars of Creation: imagery catalogue

Machine-readable version: `imagery.json` (26 entries: 21 images, 2 videos, 3 data sets). Local files are in `pillars/assets/img/` (19 JPEGs, about 24 MB). Each file was checked with `file` and measured with `sips`. Anything over 3 MB was downscaled to 2400 px wide (2000 px for ISAAC) at JPEG quality 85.

**Licences.** ESA/Hubble, ESA/Webb and ESO images are CC BY 4.0. NASA/STScI/JPL/Chandra images claim no copyright but ask for credit. The Herschel image is an ESA image, used with credit. Use the credit line exactly as written below.

Target: M16 / Eagle Nebula / NGC 6611 (the cluster). RA 18h18m48s, Dec −13°49′, in Serpens Cauda. Distance is about 5,700–7,000 ly, depending on the source.

---

## A. Locating it

| File | What it shows | Credit |
|---|---|---|
| `finder-chart-serpens-m16.jpg` (3338×4470) | Naked-eye chart of Serpens Cauda, with M16 circled in red. Use it for an opening "where in the sky" frame. | ESO, IAU and Sky & Telescope |
| `dss-eagle-nebula-visible.jpg` (1280×1096) | Digitized Sky Survey 2 photographic-plate colour view of the whole nebula. Good as the first frame of a zoom. | ESO/Digitized Sky Survey 2. Acknowledgment: Davide De Martin. |
| `eso-wfi-eagle-nebula-wide.jpg` (2400×2400) | MPG/ESO 2.2 m WFI mosaic in B (451 nm, blue), V (539 nm, green) and R (651 nm, red). The Pillars sit at the centre. The **NGC 6611** cluster is upper right: its massive O stars ionise and sculpt the pillars. **"The Spire"** (Fairy of Eagle Nebula) is middle left. | ESO |
| `wise-eagle-nebula-wide-infrared.jpg` (3400×3400) | WISE mid-IR: 3.4 µm blue, 4.6 µm cyan, 12 µm green, 22 µm red. The whole dust cavity glows. The pillars are only a faint yellow-green feature inside the white circle, which shows how small they are. | NASA/JPL-Caltech |

## B. Hubble

### `hubble-1995-pillars-wfpc2.jpg` (1518×1497): the original, 1 April 1995 data, released 2 Nov 1995
- WFPC2 narrowband filters: **F502N [O III] = blue, F656N Hα = green, F673N [S II] = red**.
- The stair-step shape with the missing black corner comes from the WFPC2 chip layout. The PC chip is at higher scale and is masked.
- Things to point out:
  - The three columns. The left one is about 4 ly tall.
  - The bright rims along the pillar tops, where UV from NGC 6611 boils gas off (**photoevaporation**).
  - The streamers leaving the surfaces.
  - The tiny "fingers" on the top of the left pillar: these are **EGGs** (Evaporating Gaseous Globules).
- Paper: Hester et al. 1996, AJ 111, 2349.
- Credit: *Jeff Hester and Paul Scowen (Arizona State University), and NASA/ESA*

### `hubble-1995-eggs-closeup.jpg` (1280×1447)
Close-up of the tip of the tallest pillar. Each finger-tip is a denser globule that resists erosion, and some contain protostars. ESO's ISAAC image (below) later showed that only about 15% of EGGs contain young stars.
Credit: *Jeff Hester and Paul Scowen (Arizona State University), and NASA/ESA*

### `hubble-2014-pillars-visible.jpg` (2400×2503): WFC3/UVIS, observed Sep 2014, released Jan 2015
- **[O III] 502 nm = blue, Hα + [N II] 657 nm = green, [S II] 673 nm = orange/red.**
- Wider and sharper than the 1995 image. It shows the base of the pillars and the rust-coloured "elephant trunks".
- The blue haze is hot oxygen gas, closest to the ionising stars.
- **HH 216**: a Herbig-Haro jet from a young star buried near the top of the tallest (left) pillar. Check its exact position against the ESA caption before you annotate it.
- Credit: *NASA, ESA/Hubble and the Hubble Heritage Team*

### `hubble-2014-pillars-infrared.jpg` (2400×2249): WFC3/IR
- **F110W (~1.1 µm) and F160W (~1.6 µm).** Near-IR passes through most of the dust, so the pillars become ghostly silhouettes against a blue haze.
- Many background and embedded stars appear. A young star shows at the tip of the left pillar.
- Pairs with the visible image as a fade or slider.
- Credit: *NASA, ESA/Hubble and the Hubble Heritage Team*

### `hubble-1995-vs-2014-comparison.jpg` (4320×2234)
Side by side across 20 years. Small changes are visible at the pillar edges, and a jet-like feature at the top of the left pillar has moved.
Credit: *WFC3: NASA, ESA/Hubble and the Hubble Heritage Team WFPC2: NASA, ESA/Hubble, STScI, J. Hester and P. Scowen (Arizona State University)*

## C. Webb (2022; data from JWST program 2739, observed 14 Aug 2022)

### `webb-2022-pillars-nircam.jpg` (2400×2503): NIRCam, released 19 Oct 2022
- Filter-to-colour mapping:

  | Filter | Wavelength / tracer | Colour |
  |---|---|---|
  | F090W | 0.9 µm | purple |
  | F187N | Paschen-α, 1.87 µm | blue |
  | F200W | 2.0 µm | cyan |
  | F335M | 3.35 µm, PAH | yellow |
  | F444W | 4.44 µm | orange |
  | F470N | 4.7 µm, H₂ | red |

- The pillars look semi-transparent and brown-orange, with thousands of foreground and background stars.
- **The wavy red, lava-like lines at the pillar edges** are shocked molecular hydrogen from **outflows and jets of newly forming stars** (Herbig-Haro objects), which show up in F470N.
- **Bright red-orange orbs** with diffraction spikes at the pillar tips are young stars, a few hundred thousand years old, still embedded.
- The blue haze is ionised hydrogen.
- Credit: *NASA, ESA, CSA, STScI; J. DePasquale, A. Koekemoer, A. Pagan (STScI).*

### `webb-2022-pillars-miri.jpg` (1987×1817): MIRI, released 28 Oct 2022
- **F770W (7.7 µm) = blue, F1130W (11.3 µm) = green, F1500W (15 µm) = red.**
- Most stars disappear because they are faint in the mid-IR, so the view is mostly dust: grey-blue, ashen pillars against a dark backdrop.
- Red tinges at the pillar tops mark the densest, coolest dust, where stars are still forming.
- Credit: *NASA, ESA, CSA, STScI, J. DePasquale (STScI), A. Pagan (STScI)*

### `webb-2022-pillars-nircam-miri.jpg` (2000×1872): NIRCam + MIRI composite, released 30 Nov 2022
- Colour mapping: purple F090W; blue F187N + F770W; cyan F200W; green F1130W; yellow F335M; orange F444W; red F470N + F1500W.
- It combines the NIRCam starfield with the MIRI dust glow.
- This file is the official 2000 px PNG rendition converted to JPEG. The full image is 7130×6675 (58 MB) and was not downloaded.
- Credit: *NASA, ESA, CSA, STScI; Image Processing: Joseph DePasquale (STScI), Alyssa Pagan (STScI), Anton Koekemoer (STScI)*

### `webb-2022-hubble-webb-side-by-side.jpg` (2400×1159)
Hubble WFC3 (2014) on the left and Webb NIRCam on the right, at the same framing. For an interactive slider, it is better to build one from `hubble-2014-pillars-visible.jpg` and `webb-2022-pillars-nircam.jpg`: they are the same size (3835×4000 originals) and appear to share the same framing.
Credit: *NASA, ESA, CSA, STScI; J. DePasquale, A. Koekemoer, A. Pagan (STScI).*

## D. Other wavelengths

### `herschel-2012-eagle-far-infrared.jpg` (1352×1353): Herschel PACS/SPIRE, Jan 2012
- **70 µm (PACS) = blue, 160 µm (PACS) = green, 250 µm (SPIRE) = red.**
- Each colour traces a different dust temperature, from about 40 K (blue) to about 10 K (red). This is the cold dust's own emission.
- The pillars look translucent, inside a cave-like system of tendrils and cavities.
- ESA released it together with an XMM-Newton X-ray image, which shows the hot young stars that sculpt the pillars.
- Credit: *ESA/Herschel/PACS/SPIRE/Hill, Motte, HOBYS Key Programme Consortium*

### `chandra-2018-eagle-xray-optical.jpg` (864×860): Chandra ACIS + Hubble
- X-ray point sources: **red 0.5–1.5 keV, green 1.5–2.5 keV, blue 2.5–7 keV.** The diffuse light is Hubble optical.
- The X-ray dots are young, magnetically active stars of NGC 6611. Very few sit inside the pillars (Guarcello et al. 2012; Linsky et al. 2007). This suggests that star formation in the pillars is winding down or is hidden.
- The 4k version (m16_4k.jpg) returned HTTP 503, so this is the small version.
- Credit: *X-ray: NASA/CXC/INAF/M.Guarcello et al.; Optical: NASA/STScI*

### `spitzer-2007-eagle-infrared.jpg` (2100×2400): Spitzer IRAC + MIPS, Jan 2007
- **4.5 µm = blue, 8 µm = green, 24 µm = red.** Green is cooler dust, including the pillars. The red shell is hot dust.
- This is the "**supernova toppled the pillars**" story (Flagey et al.). The hot dust was interpreted as heated by a supernova about 8,000–9,000 years ago, whose shock may have destroyed the pillars about 6,000 years ago. If so, we would not see the result for about another 1,000 years.
- Present this as an interpretation, not settled fact. Later work suggests stellar winds could heat the dust instead.
- Credit: *NASA/JPL-Caltech/STScI/ Institut d'Astrophysique Spatiale*

### `eso-isaac-pillars-infrared.jpg` (2000×2000): VLT ISAAC, 2001
- **J 1.24 µm = blue, H 1.65 µm = green, Ks 2.16 µm = red.** It covers 9×9 arcmin.
- McCaughrean & Andersen 2002 found that most EGGs are empty: only about 15% host young stars. They also found a massive young star in the head of column 1.
- Credit: *ESO/M.McCaughrean & M.Andersen (AIP)*

### `eso-muse-pillars-colour.jpg` (902×909) and `eso-muse-pillars-3d-view.jpg` (4500×2813): VLT MUSE, 2015 (eso1518)
- The colour view is built from emission-line slices of the MUSE data cube. The 3D view diagram explains how a data cube works: each pixel is a full spectrum.
- Results from McLeod et al. 2015 (MNRAS 450, 1057):
  - The pillars are **separate pieces on either side of NGC 6611**. The left pillar's tip points towards us, and the others point away.
  - The photoevaporation rate is about **70 solar masses per million years**, so the pillars have about **3 million years** left.
  - A new jet was found in the middle pillar.
- Credit for both: *ESO*

---

## Videos

| Title | Source | Credit |
|---|---|---|
| **Pillars of Creation Star in New Visualization from NASA's Hubble and Webb Telescopes** (3D multiwavelength flythrough, 26 Jun 2024 — not 2022) | https://svs.gsfc.nasa.gov/14616 ; 1080p60 WebM: https://svs.gsfc.nasa.gov/vis/a010000/a014600/a014616/Pillars_3D_Exploration-1920x1080p60.webm | NASA, ESA, CSA, STScI; Greg Bacon & Frank Summers (STScI), NASA's Universe of Learning; science advisor Anna McLeod (Durham) |
| The Pillars of Creation and the Interplay of Stars and Dust (narrated companion) | https://science.nasa.gov/asset/hubble/the-pillars-of-creation-and-the-interplay-of-stars-and-dust | NASA's Universe of Learning / STScI |
| Hubblecast 82: New view of the Pillars of Creation | https://esahubble.org/videos/heic1501a/ | ESA/Hubble (full credit on page; images NASA, ESA/Hubble, Hubble Heritage Team, Hester & Scowen, ESO, DSS2, McCaughrean & Andersen) |
| Zoom into Pillars of Creation | https://esahubble.org/videos/heic1501f/ | NASA, ESA/Hubble and the Hubble Heritage Team |
| 3D exploration of Pillars of Creation (2015) | https://esahubble.org/videos/heic1501g/ | NASA, ESA/Hubble and the Hubble Heritage Team |
| Fade from visible to infrared | https://esahubble.org/videos/heic1501d/ | NASA, ESA/Hubble and the Hubble Heritage Team |
| Pan over visible / infrared | https://esahubble.org/videos/heic1501b/ , /heic1501c/ | NASA, ESA/Hubble and the Hubble Heritage Team |
| Tour of Eagle Nebula | https://esahubble.org/videos/heic1501e/ | ESA/Hubble; images NASA, Hubble Heritage Team, ESO, M. McCaughrean & M. Andersen (AIP) |
| Zoom into Webb's View of the Pillars of Creation | https://esawebb.org/videos/weic2216d/ (YouTube B-wCGoAoIzQ) | NASA, ESA, CSA, STScI, ESO, NOIRLab/NSF/AURA, T.A. Rector, B.A. Wolpa, ESA/Hubble, J. DePasquale, A. Koekemoer, A. Pagan, N. Bartmann, M. Zamani |
| Hubble and Webb Showcase the Pillars of Creation | https://esawebb.org/videos/weic2216b/ (YouTube zw27saHH9PQ) | ESA/Webb, NASA, ESA, CSA, STScI, J. DePasquale, A. Koekemoer, A. Pagan, N. Bartmann |
| Pan of Webb's NIRCam portrait | https://esawebb.org/videos/weic2216c/ (YouTube 5T5JhTLunTQ) | NASA, ESA, CSA, STScI, J. DePasquale, A. Koekemoer, A. Pagan, N. Bartmann |
| Space Sparks Ep. 7 (Webb Pillars) | https://esawebb.org/videos/weic2216a/ (YouTube 1_g8KkquX4o) | ESA/Webb; footage NASA, ESA, CSA, STScI |
| 3D data visualisation of the Pillars (MUSE) | https://www.eso.org/public/videos/eso1518a/ | ESO/M. Kornmesser |
| Chandra "Different Views of the Eagle Nebula" (16 s) | https://chandra.harvard.edu/photo/2007/m16/ (animation link) | NASA/CXC |

## Open data

- **JWST program 2739.** A Cycle 1 Director's Discretionary outreach program (it is *not* listed as an ERO). PI: K. Pontoppidan.
  - Instruments and filters: NIRCam F090W/F187N/F200W/F335M/F444W/F470N and MIRI F770W/F1130W/F1500W.
  - Get the data from MAST, filtering by Proposal ID 2739. Program info: https://www.stsci.edu/jwst-program-info/download/jwst/pdf/2739/
- **HST program 13926** (PI Z. Levay). This is the 2014 WFC3 data behind heic1501. It is on MAST.
- **VLT MUSE, ESO programme 60.A-9309(A).** Science verification data, Aug 2014: a 3×3 arcmin mosaic used by McLeod et al. 2015.
  - Reduced cubes are in the ESO Science Archive Phase 3 products: https://archive.eso.org/scienceportal/ . See https://www.eso.org/sci/activities/vltsv/musesv2014.html
  - Paper DOI: 10.1093/mnras/stv680. arXiv: 1504.03323.
