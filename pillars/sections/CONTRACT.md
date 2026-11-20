# Section contract: "A Journey to the Pillars of Creation"

One long interactive essay in the spirit of Bartosz Ciechanowski (ciechanow.ski). Its frame is a voyage: we start at the naked-eye sky, fly 5,700 light-years to the Eagle Nebula, and learn on the way what the Pillars are, how they were made, how we see them and what they will become. Several writers build it in parallel. Each owns one or more sections and must follow this contract exactly so the pieces assemble into one page.

## Files you own
- `sections/NN-slug.html`: an HTML **fragment** (no `<html>`, `<head>`, `<body>`, `<style>`, `<script>` tags). It starts with `<h2 id="slug">Title</h2>` and contains prose plus figures.
- `sections/NN-slug.js`: plain browser JS (no modules, no imports, no libraries). It registers demos with `P.demo(...)`. Wrap it in an IIFE: `(function(){ 'use strict'; ... })();`.
- Never touch `lib/`, other writers' files, or `index.html`. If you need something that `lib/core.js` lacks, implement it locally inside your own IIFE.

## The runtime (`lib/core.js`, global `P`; read the file before you start)
- `P.C`: cgs constants (c, G, kB, h, mH, sigmaSB, Msun, Lsun, Rsun, pc, ly, au, yr, alphaB, g0).
- `P.sci(x, digits)` returns e.g. "2.6 × 10⁴⁹". `P.fmt`, `P.duration(seconds)`, `P.lerp`, `P.clamp`, `P.smooth`, `P.rng(seed)` (deterministic), `P.css('--ha')` (read a colour token at draw time, never cache it across draws, so theme changes work), `P.planck(lambdaCm, T)`, `P.bbColor(T)`, `P.nmColor(nm)`, `P.drag(el, fn)`.
- `P.demo('fig-id', (ctx) => {...; return { draw(t, dt){...}, animate: true|false, resize(){...} } })`.
  - The figure markup is `<figure id="fig-id"><div class="stage" data-aspect="0.5"></div><figcaption>…</figcaption></figure>`. The runtime creates the canvas inside `.stage`, sizes it (height = width × aspect, DPR-scaled, 2D context already transformed so you draw in CSS px), and creates a `.controls` row under it.
  - `ctx.g` is the 2D context; `ctx.w`, `ctx.h` are CSS px; `ctx.canvas` for pointer events (use `P.drag(ctx.canvas, fn)`).
  - Controls: `ctx.slider({label, min, max, step, value, format})` returns an object with `.value`; `ctx.toggle({label, value})`; `ctx.choice({label, options:[{label,value}], value})`. All of them trigger a redraw. Static demos (`animate:false`) are redrawn only on input or resize; animated ones run requestAnimationFrame only while visible.
  - For WebGL pass `P.demo(id, setup, { webgl: true })`; `ctx.g` is then a WebGL2 context. Use WebGL only where it is really needed.
- Image comparison: `<figure class="narrow"><div class="swipe" data-label="Hubble vs Webb"><img src="assets/img/a.jpg" alt="…"><img src="assets/img/b.jpg" alt="…"><span class="tag" style="left:10px">Hubble 2014</span><span class="tag" style="right:10px">Webb 2022</span></div><figcaption>…</figcaption></figure>`. The second image is revealed from the left as the divider moves right. The two images must have the same framing and aspect ratio.
- Plain image: `<figure><img src="assets/img/x.jpg" alt="…" loading="lazy"><figcaption>… <span class="credit">Credit line exactly as in research/imagery.json</span></figcaption></figure>`.

## Styling
Use only the classes in `lib/core.css`: `.eq` (display equation, a `<div>` with plain text, `<sub>` and `<sup>`, no LaTeX library), `.num`, `.aside`, `.ha .oiii .sii .ir` (coloured words for spectral lines), `.eyebrow`, `figure.narrow`, `.controls`. No inline colours in HTML. In canvas code take every colour from tokens: `--bg --bg-2 --fg --fg-dim --rule --accent --ha --oiii --sii --ir --dust --star`. Physical colours (a blackbody, a wavelength, a real photo) may be computed. Each demo must read well on both the dark and the light theme.

## Writing voice
- Second person, calm and curious, plain words. Build each idea from the simplest version and add one variable at a time, reusing the same demo where possible ("let's turn up…").
- Every number is tangible: compare it to something the reader knows (the Solar System, a human lifetime, a stadium).
- Introduce math sparingly. Show a formula only when the reader then uses it in a demo, and walk through one worked number with units.
- Avoid em-dash asides, "not X but Y" framing, and stock phrases ("worth noting", "it turns out", "fascinatingly"). Use short, direct sentences.
- No emoji. No bullet-point lists in the prose, except a short list of data where a list really is the clearest form.
- Aim for 1,800 to 3,000 words per section and 3 to 5 interactive demos per section. Each demo must teach something the prose alone could not.
- Never reproduce text from ciechanow.ski, the reference video, or press releases. Paraphrase and cite.

- Follow the 25 rules in `research/ref-ciechanowski.md` section 4. In particular: introduce each demo with a sentence that tells the reader what to drag, ending with a colon; after it write what to notice and why. Keep figcaptions short (credits, or one sentence saying a demo is a toy model). Reuse one playground and add one variable at a time. End each section with one sentence naming the question the next section answers.
- For travel and scale, follow the presentation ideas in `research/ref-video.md` (readouts in a corner, ship-time against Earth-time bars, mass shown as a grid of familiar objects, redrawing a map when true scale stops being readable).

## Physics and data rules
- All canonical numbers (distance, ionising photon rate Q, pillar sizes, masses, lifetimes) come from `research/facts.md` ("Recommended canonical numbers"). If you need a number that is not there, look it up from a primary source, cite it in the section's reference list, and stay consistent with facts.md.
- Every formula used in a demo must be dimensionally correct and the code must compute in cgs (or say clearly which units it uses). Add a short comment above each physics function naming the formula and its source.
- When a demo is a toy model (not a quantitative simulation), say so in the caption in one sentence.
- Clearly separate what is measured, what is modelled, and what is still debated (for example the 2007 supernova hypothesis).
- End your fragment with `<h3>Sources for this section</h3><ol class="refs">…</ol>` listing papers and pages (author, year, title, URL).

## Images
Downloaded agency images are in `assets/img/` and catalogued in `research/imagery.json` (exact credit lines and licenses). Use only those files and reference them as `assets/img/<file>`. The synthetic survey images from this repo's Observatory may be copied from `../observatory/docs/survey-pillars-*.webp` into `assets/img/` under the same name if your section uses them. Do not hotlink external images, because the published page cannot load them.

## Self-check before you report
1. Run `node --check sections/NN-slug.js`.
2. Make sure every `<figure id>` that has a `.stage` in your HTML has a matching `P.demo` and vice versa, and all ids are prefixed `fig-<slug>-` so they never collide with another section.
3. Check your physics numbers against facts.md with a short python3 script and correct any mismatch.
4. Render-check your demos headlessly if you can (e.g. stub a canvas in node, or just reason carefully); at minimum make sure draw() cannot throw on first call with default control values.
5. Report: the files written, the demos (id plus one line each), the word count, and anything you could not verify.
