# Reference: Bartosz Ciechanowski, "Moon" (ciechanow.ski/moon, Dec 17 2024)

Purpose: a style and craft reference for writing an interactive essay about the Pillars of Creation in the same manner.
Sources read: the Moon page HTML, `/js/base.js` (34 KB, shared across all essays), `/js/moon.js` (551 KB, essay-specific), `/css/base.css`, `/css/moon.css`. For recurring patterns I also skimmed `/gps/` (2022) and `/lights-and-shadows/` (2020).
Everything below is paraphrased. There is one short quote, in section 3.

Size of the Moon essay: 305 paragraphs, 120 demo containers (`drawer_container`), 115 sliders, about a dozen inline equations, 3 photographs, and 9 H2 sections plus "Further Reading" and "Final Words". GPS has 62 demos and 53 sliders across 16 sections. Lights and Shadows has 44 demos across 15 sections.

---

## 1. Content outline of the Moon essay

### Intro: "Moon" (untitled opening, 3 demos)
The opening is a short poetic pair of sentences about the Moon as a nearby, dependable presence. After that the essay goes straight to the most impressive demo. **moon1** is a photoreal WebGL Moon: you drag to rotate it (arcball) and a long slider scrubs the date and time, which moves the terminator across the surface. **sky1 + hemi8** form a split pair. The big panel is a draggable sky panorama for your actual local time, and the small one is a globe with a draggable "figurine" observer plus a locate-me button that uses geolocation. Two colour-coded sliders control **days** and **hours**. **sky2 + hemi2** repeat the same setup with the camera zoomed in and locked on the Moon, so you can see its apparent rotation, its wobble and its phases. The intro ends by listing the effects the essay will explain (gravity, tides, eclipses). It shows the full mystery up front and promises to explain all of it.

### Motion in Space (≈16 demos)
This section builds orbital mechanics from nothing in a 2D "cosmic playground."
- **space_sim1**: drag one body and its velocity arrow, then let go. It moves in a straight line and leaves a ghost trail.
- **space_sim2**: two bodies. They pass each other, orbit, or collide, and colliding bodies merge.
- **gravity**: drag the bodies and use 2 sliders for the masses. Force arrows show equal and opposite pulls.
- **gravity2**: a plot of force against distance that shows inverse-square falloff, with 2 sliders.
- The first math appears here: F = G·m1·m2/r², written with colour-coded variables, then F = m·a, then the two acceleration equations.
- **space_sim3**: unequal masses. The light body swerves while the heavy one barely moves.
- **space_sim5/6**: a split view of a fixed camera and a camera that follows the pair. A slider places the camera on the line joining the bodies, and one special position makes the camera move in a straight line. This introduces the barycenter.
- **com**: drag the bodies and use mass sliders, with a live equation r1·m1 = r2·m2 underneath.
- **space_sim6a/7/7a**: a drawing plane attached to the camera shows that relative orbits are ellipses.
- **ellipse1**: sliders for eccentricity and semi-major axis. **ellipse2** lets a slider choose which body is the "centre." **ellipse3** compares a constant-rate line with the real orbital angle, which shows that angular velocity is not constant.
- **angular_momentum**: two masses on a spinning bar, where a slider changes their separation and the spin speeds up or slows down. A real-life analogy follows: spinning on a chair holding weights.
The section ends by tying the bar back to gravity and pointing outward to a recommended video for the full proof.

### Moon and Earth (≈8 demos)
The section moves from the toy model to the real bodies.
- **table_image0**: a stats table with small rotatable globes inside the cells and a metric/imperial unit toggle.
- **space_sim8**: the playground with true Earth/Moon sizes and masses.
- **earth_scale**: a slider zooms out until the Moon's real distance appears, with perigee, mean and apogee marked.
- **m_orbit1**: a 3D draggable orbit with a time slider. A button switches between true size and 10x planet size. It has a reference line for the sidereal month.
- **m_orbit2**: a best-fit osculating ellipse with live eccentricity and semi-major axis labels.
- **m_orbit3**: apsidal precession, where markers dropped at perigee measure the anomalistic month.
- **m_orbit4/4a**: the spins and axial tilts of both bodies, with an arrow glued to the Moon that shows tidal locking and a slight wobble, and the orbital inclination.
Each new lunar "month" is defined at the moment a demo makes it visible.

### Eyes on the Heavens (≈13 demos)
This section links the view from space to the view from the ground.
- **sky5**: a draggable ground-level sky with three coloured stand-in bodies.
- **sky6 + hemi1**: a split view with the ground on the left and space on the right. Visibility cones show how the horizon cuts the sky and why apparent size depends on distance.
- **fov**: a flat side view of the visibility cone. Drag the body and the angular diameter updates.
- **sky6a/6b/6c**: time sliders. 6b keeps the camera tracking one body so the others appear to rotate around it (compared to watching a plane fly overhead). 6c makes the Earth transparent so you can follow the full closed paths.
- **sky8, sky10, sky11** paired with hemi globes: the real Moon over 24 hours, then zoomed in and locked, then over 30 days. These reproduce the intro demos, which can now be explained.
- **m_orbit5/8/9**: views from space of why the Moon rises later each day, why its apparent size changes, and the arrow that shows libration.
- **sky13/13a**: a side-by-side comparison of the Moon's apparent size at perigee and apogee.
The section ends with a number: libration lets us see about 59% of the surface over time.

### Gravity at Scale (≈17 demos)
- **sph_sim1/2/3**: about 1200 bodies collapsing into a planet. The first starts at rest. The second has random velocities and ends up spinning (angular momentum). The third has dense blue particles that sink to the core (differentiation and buoyancy). The demos auto-run, and only the one most visible on screen is active.
- **space_sim1a**: straight-line motion still carries angular momentum relative to a point.
- **sph_sim5**: a giant-impact sandbox where you drag the impactor and its velocity.
- **tides1, tides3, tides4, tides4a**: three test particles falling toward Earth drift apart. Subtracting the central body's force produces tidal-force arrows.
- **tides5/5a/6/7**: tidal force fields over the Moon. A second slider shows deliberately exaggerated stretching, and a lag slider introduces the off-axis bulge.
- **twist**: a bar pulled by two ropes. The off-axis pull creates a torque, which explains tidal locking and the Moon receding.
- **tides3d1/2/5/7**: an ocean planet. The water bulge, an observer's tide-height plot over time, and an inclined orbit that produces unequal tides.
The section ends with tangible numbers: the day lengthens about 2 ms per century and the Moon recedes 3.8 cm/yr. It then hands off to the Sun.

### Moon, Earth, and Sun (≈13 demos)
- **table_image1**: a three-body stats table with a zoom slider, because the Sun dwarfs the other two.
- **sun_scale**: a slider zooms out until the Sun appears.
- **em_orbit1/2**: a 3D three-body view with two sliders (time and speed). The barycenter path defines the ecliptic.
- **em_orbit5/6/7/7a**: inclination of 5.145°, then the line of nodes, then the longitude of the ascending node, then nodal precession every 18.61 years (the draconic month).
- **em_orbit8/9/10**: axial tilts and axial precession, then Cassini's third law shown as three coplanar lines.
- **em_orbit12 + sky14**: the angle between the Moon's orbit and Earth's equator over 18.6 years, and how wide the Moon's band across the sky becomes.
The section ends by noting that real lunar theory goes much deeper (numerical ephemerides) and then turns to the next section with a light-and-gravity pun.

### Sunlight (≈16 demos)
- **em_orbit20/22**: lit bodies in space and the solar day on each body.
- **sky20/23 + hemi**: an atmospheric sky with separate date and time sliders. The Moon is visible in daylight, and a locked zoom shows the phases.
- **sky26**: named phases selected with a segmented control.
- **em_orbit25**: the geometry behind phases and the synodic month (the fourth "month").
- **em_orbit30**: space filled with light so the shadow cones become visible.
- **shadow**: drag a blocker between the Sun and an observer to see the umbra, penumbra and antumbra zones, with the observer's view through filters on the right.
- **sky28**: new moons over successive months, which rarely line up with the Sun.
- **em_orbit6a**: scrub to the alignment behind the eclipse of April 8, 2024.
- **sky30 + hemi30**: the 2024 eclipse shadow crossing the globe. **eclipse** is a totality re-enactment with the corona. **sky33** shows an annular eclipse. **sky36** shows the lunar eclipse of November 8, 2022 with a red Moon.
- **sky40**: earthshine.
- **moon3**: back to the free-rotating Moon, now lit realistically.

### Lunar Surface (2 demos)
- **moon5**: elevation coloured by a palette lookup table plus exaggerated relief. It gives the highest and lowest points in local units.
- **moon7**: a slider for cratering and ejecta.
The prose covers simple versus complex craters, why craters are round (the shockwave), maria, and regolith, with one Apollo footprint photo. The section ends by setting up the last section ("regolith explains the full moon's brightness").

### Lunar Brightness (≈15 demos)
- **brdf1**: a matte (Lambertian) sphere with a light-direction slider.
- **brdf2**: the same sphere covered in lunar soil. It gets brighter as you look along the light direction.
- **brdf4 + brdf4a**: a split view with a camera on the left and light and view vectors on the right. This defines the phase angle.
- **brdf3**: matte compared with lunar soil, plotted against phase angle. This shows the opposition surge.
- **em_orbit50**: the real Sun-Earth-Moon geometry sets the phase angle. An Apollo photo shows a bright halo around the astronaut's shadow.
- **shadow_hiding/2**: grains with a light slider, where the shadows disappear when you view from the light's direction.
- **em_mesh, em1, em2, em3, em4, em4a, em5**: a wave-optics build-up from a pulsing source, to a plane wave, to one scattering grain, to two grains interfering, to double scattering (a segmented control selects incoming, once-scattered or twice-scattered light), to path tracing. This explains coherent backscattering.
The section ends with the current scientific consensus, then the Earthrise photo, styled as a full-bleed image with vignetted edges.

### Further Reading / Final Words
This part is three annotated book recommendations, each with a sentence on who it suits. Final Words is an emotional close in three short paragraphs about the Moon as a companion, ending with an invitation to see it differently next time. It contains no summary of the physics.

---

## 2. How it was made

### Stack
- **No frameworks or libraries.** It is hand-written vanilla JS (`"use strict"`) and hand-written GLSL. `base.js` is a small personal toolkit shared by every essay. It contains vec/mat3/mat4 math (`vec_add`, `mat3_mul`, `rot_aa_mat3`, `lerp`, `smooth_step`, `hash` ...), `CanvasRenderingContext2D` prototype helpers (`roundRect`, `fillEllipse`, `strokeLine`), `TouchHandler`, `Dragger`, `Slider`, `SegmentedControl`, a `Shader` wrapper, `ArcBall`, and `TwoAxis`.
- Each essay has its own large script (`moon.js`, `gps.js`, `light.js`). In it, a `Drawer(container, mode, args)` constructor holds a giant per-demo `mode` switch, and a list of `make_drawer("name", args, slider_count, slider_defaults)` calls wires up all 120 demos at `DOMContentLoaded`.
- **Rendering is hybrid.** A **single shared, hidden WebGL1 context** (`GLDrawer`, created with `experimental-webgl`, antialias and stencil on) renders the 3D content: textured Moon and Earth, atmosphere, shadows, particles. Each demo has its **own visible 2D canvas**, which `drawImage`s the GL result (`gl.finish()`) and then draws the 2D overlays on top: arrows, dashed lines, labels in IBM Plex Sans, trails. This avoids browser limits on the number of WebGL contexts, and the overlays stay crisp. GL extensions requested include `OES_texture_float`, half float, `OES_standard_derivatives`, `EXT_shader_texture_lod`, and `OES_element_index_uint`. A second "mini" GL drawer handles the small globes.
- **Data assets**: textures load as PNGs from `/images/moon/<name>.png` (colour and elevation maps), with mipmaps. A binary **ephemeris** file (`/models/moon_eph.dat`, fetched as an XHR arraybuffer) drives the real Sun, Earth and Moon positions for the current date. Demos that need it re-render when it arrives, and sky and moon demos show a flickering "Loading..." text until then. The intro demos use `new Date()` and the visitor's location, so the reader sees *their own sky right now*.
- Canvas sizes are multiplied by `devicePixelRatio`. Label font size steps down at viewport widths of 500, 400 and 350 px.

### Lazy start / performance
- An `IntersectionObserver` with `rootMargin: "300px"` calls `drawer.set_visible(isIntersecting)`, and `repaint()` returns early when a demo is not visible. Demos start drawing a little before they scroll into view.
- Drawing is **on demand**. `request_repaint()` merges requests into one `requestAnimationFrame`. Animated demos run a `tick` loop only while unpaused.
- Expensive simulations (the N-body `sph` sims) register as "active containers." A scroll listener scores each one by the fraction of its height that is on screen and activates only the best one, so only one heavy sim runs at a time. The others get `.inactive_container` (opacity 0.5, 0.2s transition).
- Animated demos have a **play/pause button** in the bottom-left corner: a 44×44 sprite (`.play_pause_button`, with a `.playing` state and a `.white` variant for dark backgrounds). Most start paused. Simulated ones auto-run.

### Interaction primitives
- **Slider** (base.js): it is not an `<input type=range>`. The HTML has an empty placeholder `<div class="long_slider" id="moon1_sl0">` after each demo, and JS builds a zero-height `.slider_container` that contains a left gutter (filled, `#777`), a right gutter (same colour at 0.2 opacity), and a 40 px round knob (`.slider_knob`, `cursor: grab`, `touch-action: none`). Values are normalised to 0..1, and the callback runs on every move. Clicking the gutter jumps the knob to that point. Widths: 90% up to a 380 px maximum, with `.long_slider` allowing 600 px. Breakpoints reduce this to 320, 300 and 260 px. Sliders can carry colour variants (for example `slider_c2`) whose colours match the variable they control, so the "days" slider uses the same blue as the word **days** in the text.
- **ArcBall** (free 3D rotation of the Moon): it maps the drag to a rotation on a virtual sphere (radius = 0.65 × min(width, height)) and records angular velocity. On release, it keeps spinning with **inertia**, decaying by ×0.995 per ms, but only if the pointer moved within the last 40 ms.
- **TwoAxis** (orbit and sky views): yaw and pitch with optional angle limits, also with momentum. It is used for orbital views where "up" should stay up, and for the tables (which lock the vertical axis).
- Direct manipulation **inside** the canvas: draggable bodies, velocity-arrow tips, the observer figurine, and grains. The cursor changes to `move`/`grab`. Touch is handled by `TouchHandler`, which suppresses page scroll while dragging.
- **SegmentedControl**: a rounded grey pill (`#ddd`, radius 10 px, height 40 px) whose selected segment is light. Used for discrete choices such as phase names or scattering order.
- **Unit switch**: an inline pill (`.unit_switch`) with a small "↑↓" badge drawn by `::after`. Clicking it toggles `body.show_imperial`, which shows or hides paired `.metric_word`/`.imperial_word` spans. Time formats are toggled the same way (`.short_time`/`.long_time`, for example "27.322 days" against "27 days, 7 hours, and 44 minutes"). The choice persists in `localStorage("global.metric")`, and the default comes from the browser language (en-US means imperial).
- Touch and mouse wording: `.click_word`/`.tap_word` spans swap via `@media (pointer:coarse)`, so the text says "tapping" on phones.

### Layout and typography
- Page background `#F8F8F8`, body text `#444`. Body font **IBM Plex Sans** (400, 500, italic). Headings in **Inter** 600, colour `#535353`, with `font-optical-sizing: auto`. `strong` is weight 500, not bold.
- Text column: `.article > *` uses `max-width: 44rem` with 3rem side padding (1.2rem under 450 px). Paragraphs are 1.2em with line-height 1.6em, and `p` padding is 0.7em vertically.
- Links `#0181eb`, visited `#356082`, with a thin underline (0.06em) and offset 0.1em.
- **Demo containers** keep their aspect ratio with `.drawer_container::after { padding-top: N% }` and the classes `ratio_20 ... ratio_200`. The Moon CSS caps them at `max-width: 500px`. `.full_width` removes the cap so wide 2D sims span the column. The canvas fills an absolutely positioned `.canvas_container`. Margins are 2.3em above and 2em below.
- **Split demos**: `.split_container` (ratio 50%) holds two `drawer_container`s at 50% width each. A `big_small` variant is used for the sky panorama plus the small globe.
- **Moon-specific theme**: almost the whole article sits inside `.dark_light_bg`, a full-width **black band** with text `#ddd`, entered through gradient strips at the top and bottom (`dark_light_bg_grad_top/bottom`). The essay feels like space while the site shell stays light. GPS and Lights and Shadows stay on the light background.
- **Colour-coded vocabulary**: a fixed palette (`#EFC540` yellow, `#EF7F39` orange, `#31C0B1` teal, `#256BB9` blue, `#DA7BA3` pink, `#A41247` crimson, plus `#7290AF` for date and `#CDB57B` for time). The same hex values are used in the canvas drawing, in `<span class="c0"><strong>word</strong></span>` within the prose, and inside equations. The reader matches words to marks by colour alone.
- **Equations** are hand-built HTML, not MathJax or KaTeX: `.equation` is centred at 1.42em with weight 500 and coloured spans, and fractions use `.equation_frac` with a 2 px `.equation_div_symbol` bar. On the dark Moon page the equation colour is `#888`.
- **Figures and photos**: `.img_container > .img_border` (white 15 px border, 10 px radius, soft 3 px `#eaeaea` ring) around `<img loading="lazy">`, with an optional caption in a `span` (0.8em, `#555`, centred). Photos are rare (3 in the whole essay) and are used as "this is real" evidence. **Demos have no captions**: the paragraph before a demo introduces it and the paragraph after it interprets it.
- Headings have hover anchors (`.hanchor`). The footer and the "Further Reading" section are simple.

---

## 3. Rhetorical and pedagogical techniques

1. **Open with the finished phenomenon, then rebuild it from zero.** The intro shows the real Moon in the reader's own sky, with all its puzzling behaviour (wobble, rotation, phases). The essay then admits this may feel overwhelming, promises to explain it, and drops back to a single dot in empty space. Every later section climbs back toward the opening view. The intro demos come back in the middle of the essay, and this time the reader understands them.
2. **Strict ladder, one variable at a time.** One body, then two bodies, then mass sliders, then a force plot, then a moving camera, then a camera fixed at the barycenter, then a drawing plane on the camera, then ellipses. The same playground is reused about 10 times, and each version adds exactly one new thing (a slider, a trail, a camera mode, a marker).
3. **"Let me show you" framing.** Almost every demo is introduced by a sentence ending in a colon that names what to do: "In the demonstration below, you can drag ... and use the slider to ...:" The author speaks as a guide who openly builds the apparatus. The Motion in Space section opens with "Let me introduce a little cosmic playground" and the essay keeps narrating its own construction: I put, I'm marking, I'll zoom the camera in, I made the Earth transparent.
4. **"Notice that ..." after each demo.** The paragraph after a demo directs attention to the one thing to see: notice that, you may have noticed, if you paid close attention. The interpretation comes only after the reader has had a chance to see the effect.
5. **Voice: "we" for the journey, "you" for the hands, "I" for the author's choices.** "We'll learn", "let's see". "You can drag". "I'm drawing a ghost trail". The tone is warm and plain, with no jokes at the reader's expense and only an occasional mild pun at section transitions.
6. **Honest simplification.** The essay states when a model is crude or exaggerated (bodies that merge on collision, tidal stretching "vastly exaggerated", a water planet that is "not super realistic") and then says what reality adds. It points to outside resources for full derivations instead of doing them.
7. **Math is sparse, inline, and colour-matched.** The essay contains about 12 equations in total, all short, introduced as a "brief mathematical interlude" only after the demo has built the intuition, and immediately read out in words ("notice that a1 depends on m2"). Every symbol is coloured like its counterpart in the demo. Some equations are live and update with the demo (the barycenter r1·m1 = r2·m2).
8. **Numbers are made tangible.** Real scale arrives through a zoom-out slider rather than a stated figure (Earth to Moon, then to the Sun). Planet-size toggles switch between true and 10x scale. Stats tables have rotatable globes in them. Units switch inline between metric and imperial. Durations switch between decimal days and "days, hours, minutes". Comparisons are relative ("only about 3.67 times larger", "332,950 times the Earth"). Rates are given per human-scale period (2 ms per century, 3.8 cm per year). Named real events are scrubbed to directly (the April 8, 2024 eclipse).
9. **Real-world anchors.** Analogies come from the body and from daily life (spinning on a chair holding weights, watching an airplane pass overhead, lifting objects despite the Earth's mass), plus a few photographs as evidence that the simulation matches reality.
10. **Vocabulary is earned.** A technical term (barycenter, apsides, sidereal/anomalistic/draconic/synodic month, libration, nodal precession, opposition surge, coherent backscattering) is named only once a demo has made it visible. Terms appear in italics or as colour-coded bold, often with a Wikipedia link.
11. **Section endings.** Sections end with a one-sentence bridge that names the gap the next section fills ("we're ready to cast some light on ...", "the regolith also explains the full moon's brightness, so let's explore it"). Sometimes a satisfying number closes a section first (59% of the surface).
12. **Ending.** The essay finishes on an image (Earthrise), then annotated further reading, then a brief, sincere, almost lyrical close that changes how the reader will see the object. It does not recap the content.

---

## 4. Style contract (rules for imitating it on another topic)

1. Open with 2-3 sentences of quiet wonder, then show the real object as an interactive demo within the first screen. Do not start with a definition.
2. In the intro, show the full puzzling phenomenon and list the effects you will explain. Promise that by the end the reader will understand every one of them.
3. Then reset to the simplest possible model (one particle, one ray, one star) and build upward. Every section depends on the one before.
4. Make demos the backbone, at roughly one demo per 2-3 paragraphs. Each demo teaches one idea, and its prose must not make sense without it.
5. Reuse the same playground repeatedly. Each new version adds exactly one variable, overlay, or camera trick. Keep visual identity (colours, body shapes) constant across versions.
6. Introduce each demo with a sentence that tells the reader what to drag or slide, ending with a colon. Never caption demos. The prose is the caption.
7. After each demo, write "Notice that ..." and name the single most important thing to see, then explain why it happens.
8. Use "we/let's" for the journey, "you" for actions, and "I" for your visual choices ("I'm marking ... with a thin line", "I made X transparent").
9. Give every key concept a fixed colour and use it everywhere: in canvas strokes, in coloured bold words in the text, in equation symbols, and in the matching slider. Keep to 6-8 hues.
10. Hold math back until the intuition exists. Present it as a short "interlude", keep each equation to one line, read it aloud in plain words right after, and let it update live with the demo where possible.
11. Name a technical term only after the reader has seen the thing. Italicise it or colour it, and link it once.
12. Show scale with motion, not just numbers: zoom-out sliders, true-scale against enlarged toggles, and side-by-side tables containing rotatable objects.
13. When you do give numbers, make them relatable: ratios, rates per human lifetime or century, metric and imperial units (with a toggle), and named real dates and events the reader can jump to.
14. Say plainly when a demo exaggerates or simplifies, and in one sentence describe what reality adds.
15. Include 1-3 real photographs (for the Pillars: Hubble 1995/2014 and JWST 2022) as proof that the simulation matches reality. Place them where a demo has just predicted what the photo shows.
16. Use everyday or bodily analogies sparingly, one per hard concept.
17. Controls: draggable objects inside the canvas, custom round-knob sliders (0..1) under the demo, segmented controls for discrete modes, play/pause for animations, and inertial drag-to-rotate for 3D. Leave out dropdowns and text inputs.
18. Demos start paused or idle, render only when near the viewport, and only the most visible heavy simulation runs.
19. Typography: one calm sans for body (Plex Sans at 1.2em/1.6), a geometric sans for headings (Inter 600), a 44rem column, generous whitespace, and medium weight (not bold) for emphasis.
20. Choose one background world for the topic. The Moon used a full-bleed black band for space. A nebula essay can do the same, with light text at about `#ddd` and gradient entry and exit strips.
21. Section titles are short nouns or noun phrases (two to four words). No questions or clickbait in the Moon essay (GPS allows the playful exception).
22. End each section with one sentence that names the open question the next section answers.
23. Cover more than seems necessary. Follow one thread from basic physics all the way to the subtle, recent-science explanation, and state the current consensus honestly.
24. Close with a striking image, a short annotated "Further Reading" list (a sentence per item on who it suits), and 2-3 sincere sentences that change how the reader will see the object next time. Do not summarise.
25. The reader should never feel lectured. Every claim should be something they could have discovered with the slider in front of them.
