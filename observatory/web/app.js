// Schwarzschild Observatory: browser orchestration.
// N render workers each own interleaved rows; a compositor instance on the
// main thread runs the sensor, display views and exporters.

const $ = (s) => document.querySelector(s);
const enc = new TextEncoder();
const dec = new TextDecoder();
const PRESETS = ['first-light-hdr', 'first-light-single', 'lensed-hii-offset', 'einstein-ring-structured', 'narrowband-nebula', 'deep-field-stack', 'deep-field-frame', 'einstein-ring', 'critical-curve'];
const PRESET_TITLES = {
  'first-light-hdr': 'First Light — HDR composite (registered virtual exposures)', 'first-light-single': 'First Light — physical single exposure (calibration)',
  'lensed-hii-offset': 'Lensed H II region — offset source', 'einstein-ring-structured': 'Einstein ring — structured alignment',
  'narrowband-nebula': 'Narrowband nebula (SHO false colour)', 'deep-field-stack': 'Weak-lensing deep field — stacked survey',
  'deep-field-frame': 'Weak-lensing deep field — physical detector frame', 'einstein-ring': 'Einstein ring — exact alignment (symmetry test)',
  'critical-curve': 'Critical curve study — local observer (validation)',
};
const FILTERS = ['L', 'Red', 'Green', 'Blue', 'U', 'B', 'V', 'R', 'I', 'Ha', 'OIII', 'SII'];
const VIEWS = [
  ['sensor', 'Presentation · simulated sensor image'], ['radiance', 'Presentation · visible-light rendering (CIE)'], ['band', 'Presentation · single filter plane'],
  ['fate', 'Diagnostic · ray fate'], ['winding', 'Diagnostic · orbital angle (turns)'], ['redshift', 'Diagnostic · disk frequency ratio g'],
  ['invariant', 'Diagnostic · first-integral drift'], ['steps', 'Diagnostic · geodesic steps'], ['tau', 'Diagnostic · nebula dust τ_V'],
  ['nebula', 'Diagnostic · nebula line radiance'], ['variance', 'Diagnostic · renderer (MC) error'], ['noise', 'Diagnostic · sensor noise / σ'],
];

let wasmBytes, C; // compositor exports
let workers = [], nw = 0;
let scene = null, presetName = 'first-light', volumeBytes = null, volumeId = 'none';
let running = false, busy = false, wallStart = 0, wallAcc = 0, spp = 0, lastShow = 0, passAdd = 1;
let totals = { samples: 0 };
const disp = { view: 'sensor', band: 0, white: 0.99995, soft: 0.08, gain: 1 };

// ------------------------------------------------------------ wasm glue --
function cPut(bytes) {
  const p = C.alloc(bytes.length);
  new Uint8Array(C.memory.buffer, p, bytes.length).set(bytes);
  return [p, bytes.length];
}
const cOut = () => new Uint8Array(C.memory.buffer, C.out_ptr(), C.out_len()).slice();
function cCall(fn, ...strs) {
  const ptrs = strs.map((s) => cPut(typeof s === 'string' ? enc.encode(s) : s));
  const r = fn(...ptrs.flat());
  ptrs.forEach(([p, n]) => C.dealloc(p, n));
  return r;
}

function ask(w, msg, reply, transfer = []) {
  return new Promise((res) => {
    const h = ({ data }) => { if (data.cmd === reply) { w.removeEventListener('message', h); res(data); } };
    w.addEventListener('message', h);
    w.postMessage(msg, transfer);
  });
}
const all = (fn) => Promise.all(workers.map((w, k) => fn(w, k)));

// ----------------------------------------------------------------- setup --
async function boot() {
  wasmBytes = await (await fetch('observatory.wasm')).arrayBuffer();
  C = (await WebAssembly.instantiate(wasmBytes)).instance.exports;
  nw = Math.max(1, Math.min(8, (navigator.hardwareConcurrency || 4) - 1));
  workers = Array.from({ length: nw }, () => new Worker('worker.js'));
  await all((w) => ask(w, { cmd: 'load', wasm: wasmBytes }, 'loaded'));
  for (const p of PRESETS) {
    const o = document.createElement('option');
    o.value = p;
    o.textContent = PRESET_TITLES[p] || p;
    $('#preset').appendChild(o);
  }
  const q = new URLSearchParams(location.search).get('preset');
  if (q && PRESETS.includes(q)) $('#preset').value = q;
  $('#preset').onchange = () => loadPreset($('#preset').value);
  bindButtons();
  await loadPreset($('#preset').value);
}

async function loadPreset(name) {
  presetName = name;
  const j = await (await fetch(`presets/${name}.json`)).json();
  // the sliders edit degrees; fold arcminute keys into them once
  if (j.fov_arcmin != null) { j.fov_deg = j.fov_arcmin / 60; delete j.fov_arcmin; }
  if (j.aim_offset_arcmin) { j.aim_offset_deg = j.aim_offset_arcmin.map((v) => v / 60); delete j.aim_offset_arcmin; }
  // fold the explicit quality keys into the fields the controls edit
  const alias = { render_width: 'width', render_height: 'height', samples_per_pixel_target: 'target_spp', star_density_scale: 'star_density', galaxy_density_scale: 'galaxy_density' };
  for (const [a, k] of Object.entries(alias)) if (j[a] != null) { j[k] = j[a]; delete j[a]; }
  scene = j;
  disp.view = 'sensor'; disp.band = 0; disp.gain = 1;
  $('#preset-desc').textContent = j.notes || '';
  await initScene();
}

async function initScene(accumFromCheckpoint) {
  running = false; busy = true;
  setState('Preparing sky and volume…', 'busy');
  $('#c').width = scene.width; $('#c').height = scene.height;
  volumeBytes = null; volumeId = 'none';
  if (scene.volume) {
    const name = scene.volume.split('/').pop();
    const r = await fetch(`volumes/${name}`);
    volumeBytes = await r.arrayBuffer();
    volumeId = `${name} (${volumeBytes.byteLength} bytes)`;
  }
  const sj = JSON.stringify(scene);
  cCall(C.c_init, sj);
  const t0 = performance.now();
  // generate the sky once (worker 0), then hand the catalogue to the others
  const first = await ask(workers[0], { cmd: 'init', scene: sj, volume: volumeBytes ? volumeBytes.slice(0) : null, k: 0, nw, wantSky: true }, 'ready');
  setState('Preparing workers…', 'busy');
  await Promise.all(workers.slice(1).map((w, i) => ask(w, { cmd: 'init', scene: sj, volume: volumeBytes ? volumeBytes.slice(0) : null, k: i + 1, nw, sky: first.sky && first.sky.byteLength ? first.sky.slice(0) : null }, 'ready')));
  console.log(first.log, `workers ready in ${(performance.now() - t0).toFixed(0)} ms`);
  spp = 0; wallAcc = 0; totals = { samples: 0, steps: 0, vol_steps: 0, vol_rays: 0, failed: 0 }; passAdd = 1;
  if (accumFromCheckpoint) await accumFromCheckpoint();
  buildControls();
  busy = false;
  await show(true);
  setState(spp ? 'Paused' : 'Ready', 'paused');
  $('#run').textContent = 'Start';
}

// ------------------------------------------------------------- render loop --
async function loop() {
  while (running) {
    const t0 = performance.now();
    const res = await all((w) => ask(w, { cmd: 'render', add: passAdd }, 'rendered'));
    spp = res[0].spp;
    const dt = performance.now() - t0;
    totals = res.reduce((a, r) => { for (const k of Object.keys(r.stats)) if (k !== 'spp') a[k] = (a[k] || 0) + r.stats[k]; return a; }, {});
    // aim for ~400 ms passes
    passAdd = Math.max(1, Math.min(64, Math.round(passAdd * 400 / Math.max(dt, 1))));
    if (performance.now() - lastShow > 1500) await show();
  }
}

async function show(force) {
  if (busy && !force) return;
  const packs = await all((w) => ask(w, { cmd: 'pack' }, 'packed'));
  packs.forEach((p, k) => {
    const b = new Uint8Array(p.buf);
    const [ptr, n] = cPut(b);
    C.c_put(ptr, n, k, nw, spp);
    C.dealloc(ptr, n);
  });
  C.c_readout();
  const st = JSON.parse(dec.decode(cOut()));
  draw();
  updateStatus(st);
  lastShow = performance.now();
}

function draw() {
  cCall((vp, vn) => C.c_display(vp, vn, disp.band, scene.white_point_percentile ?? 0.99995, scene.display_softness ?? 0.08, disp.gain), disp.view);
  const px = cOut();
  const cv = $('#c');
  cv.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(px.buffer), cv.width, cv.height), 0, 0);
  drawLegend();
}

// legend and output-class badge under the image (diagnostics are never
// presented as pictures)
function drawLegend() {
  cCall(C.c_legend, disp.view);
  const lg = JSON.parse(dec.decode(cOut()) || '{}');
  C.c_info();
  const info = JSON.parse(dec.decode(cOut()) || '{}');
  const el = $('#legend');
  const esc = (t) => String(t).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  let html = `<span class="badge ${lg.class}">${lg.class === 'diagnostic' ? 'Diagnostic — not an image of light' : 'Presentation'}</span>`;
  const MODE = { registered_hdr_composite: 'Synthetic HDR composite of registered frames — not a single detector frame', stacked_survey: 'Stacked survey composite of registered frames', diagnostic: 'Validation scene (local observer) — not a telescope image' };
  if (MODE[scene.presentation_mode]) html += ` <span class="badge false">${MODE[scene.presentation_mode]}</span>`;
  if (lg.class !== 'diagnostic' && disp.view === 'sensor') {
    const fc = /FALSE COLOUR/.test(info.palette || '');
    html += ` <span class="badge ${fc ? 'false' : ''}">${esc((info.palette || '').toLowerCase())}</span> <span class="keys">${(info.palette_keys || []).map(esc).join(' · ')}</span>`;
  }
  if (lg.title) {
    html += `<div class="lt">${esc(lg.title.toLowerCase())}</div>`;
    if (lg.categories) html += `<div class="cats">${lg.categories.map(([n, c]) => `<span><i style="background:rgb(${c.join(',')})"></i>${esc(n)}</span>`).join('')}</div>`;
    if (lg.stops) {
      const grad = lg.stops.map((c, i) => `rgb(${c.join(',')}) ${(i / (lg.stops.length - 1)) * 100}%`).join(',');
      html += `<div class="ramp" style="background:linear-gradient(90deg,${grad})"></div><div class="ticks">${lg.ticks.map(([t, l]) => `<span style="left:${t * 100}%">${esc(l)}</span>`).join('')}</div>`;
    }
    html += `<div class="ln">${esc(lg.note.toLowerCase())}</div>`;
  }
  el.innerHTML = html;
  const m = info.mode === 'local'
    ? `<b>Local observer</b> at r₀ = ${(+info.r0_m).toFixed(1)} M — a theoretical visualization, not a telescope observation.`
    : `<b>Distant telescope</b>: r₀ = ${(+info.r0_m).toExponential(2)} M (${(+info.distance_pc).toExponential(2)} pc) · shadow radius ${(+info.shadow_arcsec).toPrecision(3)}″ · Einstein radius ${(+info.einstein_arcmin).toPrecision(3)}′ · ${(+info.pixel_arcsec).toPrecision(3)}″/px${info.lensing ? '' : ' · <b>lensing off (comparison)</b>'}`;
  $('#scale').innerHTML = m;
}

function wall() { return wallAcc + (running ? (performance.now() - wallStart) / 1000 : 0); }
function fmtTime(s) { const m = Math.floor(s / 60); return `${String(m).padStart(2, '0')}:${String(Math.floor(s % 60)).padStart(2, '0')}`; }
function pct(x, d = 2) { const v = x * 100; return v > 0 && v < 0.01 ? `${v.toExponential(1)}%` : `${v.toFixed(d)}%`; }

function updateStatus(st) {
  $('#s-spp').textContent = spp.toLocaleString();
  const total = scene.exposures && scene.exposures.length ? scene.exposures.reduce((a, e) => a + (e.count || 1) * e.exposure_s, 0) : scene.exposure_s;
  const fmtS = (v) => (v < 0.1 ? v.toExponential(1) : v.toFixed(1));
  $('#s-exp').textContent = `${fmtS(st.exposure_s)} / ${fmtS(total)} s${scene.exposures && scene.exposures.length ? ' (all frames)' : ''}`;
  $('#s-mc').textContent = spp > 1 ? pct(st.render_noise) : '–';
  $('#s-sn').textContent = st.sensor_noise ? pct(st.sensor_noise) : '–';
  $('#s-wall').textContent = fmtTime(wall());
  const w = wall();
  $('#s-rate').textContent = w > 0 && totals.samples ? `${(totals.samples / w / 1e6).toFixed(2)} M samples/s` : '–';
  $('#s-fail').textContent = pct(st.failed_fraction, 3);
  $('#s-steps').textContent = st.mean_steps.toFixed(0);
  $('#s-vol').textContent = st.mean_vol_steps ? st.mean_vol_steps.toFixed(0) : '–';
  if (totals.mem_bytes) $('#s-mem').textContent = `${((totals.mem_bytes + C.memory.buffer.byteLength) / 2 ** 20).toFixed(0)} MiB`;
  if (running) setState('Accumulating', '');
}

function setState(t, cls) { $('#state b').textContent = t; $('#state').className = cls; }

// --------------------------------------------------------------- controls --
function bindButtons() {
  $('#run').onclick = () => {
    if (busy) return;
    running = !running;
    if (running) { wallStart = performance.now(); $('#run').textContent = 'Pause'; setState('Accumulating', ''); loop(); }
    else { wallAcc += (performance.now() - wallStart) / 1000; $('#run').textContent = 'Resume'; setState('Paused', 'paused'); show(); }
  };
  $('#reset').onclick = async () => {
    const was = running; running = false; busy = true;
    await all((w) => ask(w, { cmd: 'reset' }, 'reset_done'));
    spp = 0; wallAcc = 0; totals = { samples: 0 }; busy = false;
    await show(true);
    $('#run').textContent = 'Start'; setState('Ready', 'paused');
    if (was) $('#run').click();
  };
  $('#save').onclick = saveCheckpoint;
  $('#loadbtn').onclick = () => $('#load').click();
  $('#load').onchange = (e) => e.target.files[0] && loadCheckpoint(e.target.files[0]);
  document.querySelectorAll('[data-exp]').forEach((b) => (b.onclick = () => exportFile(b.dataset.exp)));
}

function groupsFor(sc) {
  const local = sc.mode === 'local';
  return {
    'g-geom': [
      ...(local
        ? [['cam_r', 'Observer radius (M)', 8, 400, 1]]
        : [['bh_mass_msun', 'Mass (M☉)', 1e1, 1e10, 0, 'log'], ['bh_distance_pc', 'Distance (pc)', 1e-5, 10, 0, 'log']]),
      ['cam_incl_deg', 'Inclination (°)', 0, 90, 0.5], ['cam_azimuth_deg', 'Azimuth (°)', 0, 360, 1],
      ['fov_deg', 'Field of view (°)', 0.01, 90, 0, 'log'], ['lensing', 'Lensing (off = comparison)', 0, 1, 1, 'bool'],
      ['rtol', 'Geodesic rtol', 1e-12, 1e-6, 0, 'log'], ['max_windings', 'Max windings', 1, 12, 0.5],
      ['width', 'Width (px)', 160, 1280, 16], ['height', 'Height (px)', 100, 800, 16],
    ],
    'g-disk': [
      ['disk', 'Accretion disk', 0, 1, 1, 'bool'], ['disk_r_out', 'Outer radius (M)', 8, 200, 0.5],
      ['disk_eddington_ratio', 'L / L_Edd', 1e-3, 1, 0, 'log'], ['disk_color_correction', 'Colour correction f_col', 1, 2.5, 0.05],
      ['disk_tau', 'Vertical τ (0 = opaque)', 0, 10, 0.05], ['disk_brightness_scale', 'Artistic multiplier (1 = physical)', 1e-3, 1e3, 0, 'log'],
      ['disk_max_order', 'Max photon order', 1, 6, 1],
    ],
    'g-sky': [
      ['seed', 'Universe seed', 0, 99999, 1], ['star_mag_limit', 'All-sky star limit (V)', 4, 11, 0.1],
      ['star_density', 'Star density scale (1 = model)', 0.1, 3, 0.05], ['galaxy_density', 'Galaxy density scale (1 = model)', 0.1, 3, 0.05],
      ['deep_star_mag_limit', 'In-field star limit (V, 0 = off)', 0, 22, 0.5], ['deep_galaxy_mag_limit', 'In-field galaxy limit', 0, 23, 0.5],
      ['dust', 'Galactic dust', 0, 3, 0.05], ['galaxies', 'Galaxies', 0, 1, 1, 'bool'], ['galaxy_mag_limit', 'All-sky galaxy limit', 8, 15, 0.1],
      ['diffuse', 'Unresolved starlight', 0, 1, 1, 'bool'], ['source_distance_pc', 'Nebula distance behind hole (pc)', 200, 10000, 0, 'log'],
      ['volume_brightness', 'Nebula multiplier (1 = physical)', 0, 5, 0.05],
    ],
    'g-filt': [
      ['aperture_m', 'Aperture (m, 0 = auto)', 0, 0.5, 0.001], ['f_ratio', 'f-ratio (auto aperture)', 1, 16, 0.1], ['pixel_um', 'Pixel pitch (µm)', 1, 15, 0.01],
      ['neutral_density', 'ND filter (optical density)', 0, 8, 0.1],
    ],
    'g-sens': [
      ['exposure_s', 'Exposure (s)', 1e-6, 1e4, 0, 'log'], ['read_noise_e', 'Read noise (e⁻)', 0, 20, 0.1], ['dark_e_per_s', 'Dark (e⁻/s)', 0, 1, 0.001],
      ['gain_e_per_adu', 'Gain (e⁻/ADU)', 0.1, 10, 0.05], ['full_well_e', 'Full well (e⁻)', 1000, 200000, 1000],
      ['optics_fwhm_arcsec', 'Optics / seeing FWHM (″)', 0, 6, 0.05], ['halo_fraction', 'Halo fraction', 0, 0.1, 0.002], ['spider_vanes', 'Spider vanes', 0, 4, 1],
      ['cosmic_rays_per_cm2_s', 'Cosmic rays (/cm²/s)', 0, 0.2, 0.001], ['sensor_noise', 'Sensor noise', 0, 1, 1, 'bool'],
      ['psf', 'PSF', 0, 1, 1, 'bool'], ['shutter_open', 'Shutter open', 0, 1, 1, 'bool'],
    ],
    'g-budget': [['target_spp', 'Samples per pixel target (exposure complete)', 16, 16384, 16, 'log']],
  };
}
const RERENDER = new Set(['g-geom', 'g-disk', 'g-sky']);

function fmt(v, kind) {
  if (kind === 'bool') return v ? 'on' : 'off';
  if (kind === 'log') return Math.abs(v) >= 1e4 || Math.abs(v) < 1e-2 ? Number(v).toExponential(1) : String(+(+v).toFixed(2));
  return String(+(+v).toFixed(3));
}

function buildControls() {
  // view
  const gv = $('#g-view');
  gv.innerHTML = '';
  const sel = document.createElement('select');
  sel.innerHTML = VIEWS.map(([v, t]) => `<option value="${v}">${t}</option>`).join('');
  sel.value = disp.view;
  sel.onchange = () => { disp.view = sel.value; draw(); };
  gv.appendChild(sel);
  const bandRow = document.createElement('div');
  bandRow.className = 'row wide';
  bandRow.innerHTML = `<label>Filter band</label><select>${(scene.filters || []).map((f, i) => `<option value="${i}">${f}</option>`).join('')}</select>`;
  bandRow.querySelector('select').onchange = (e) => { disp.band = +e.target.value; draw(); };
  gv.appendChild(bandRow);
  // output class: sets the native render size (never an upscale)
  const QUAL = { fixture: [128, 80], validation: [512, 320], presentation: [1024, 640], high_quality: [1600, 1000] };
  const q = document.createElement('div');
  q.className = 'row wide';
  q.innerHTML = `<label>Output class</label><select>${Object.entries(QUAL).map(([k, [w, h]]) => `<option value="${k}">${k.replace('_', ' ')} · ${w}×${h}</option>`).join('')}</select>`;
  const qs = q.querySelector('select');
  qs.value = scene.output_class || 'presentation';
  qs.onchange = async () => { const [w, h] = QUAL[qs.value]; scene.output_class = qs.value; scene.width = w; scene.height = h; await initScene(); };
  gv.appendChild(q);
  const pm = document.createElement('p');
  pm.className = 'help';
  pm.textContent = `Presentation mode: ${(scene.presentation_mode || 'physical_single_exposure').replace(/_/g, ' ')}${scene.exposures && scene.exposures.length ? ` · ${scene.exposures.map((e) => `${e.count || 1}×${e.exposure_s}s${e.neutral_density ? ` ND${e.neutral_density}` : ''}`).join(', ')}` : ''}`;
  gv.appendChild(pm);
  const tr = document.createElement('div');
  tr.className = 'row wide';
  tr.innerHTML = `<label>Display transform</label><select>${['asinh', 'linear', 'filmic', 'narrowband_sho', 'hdr_log'].map((t) => `<option>${t}</option>`).join('')}</select>`;
  const ts = tr.querySelector('select');
  ts.value = scene.display_transform || 'asinh';
  ts.onchange = () => change('g-disp', 'display_transform', ts.value);
  gv.appendChild(tr);
  const DISP = [
    ['white_point_percentile', 'White point (quantile)', 0.99, 1, 0.00005, 0.99995], ['black_point_percentile', 'Black point (quantile)', 0, 0.9, 0.01, 0.02],
    ['display_softness', 'Stretch softness', 0.005, 1.5, 0.005, 0.08], ['highlight_rolloff', 'Highlight shoulder (0 = clip)', 0, 0.98, 0.01, 0],
    ['hdr_decades', 'HDR log decades', 2, 16, 0.5, 8],
  ];
  for (const [k, label, min, max, step, def] of DISP) {
    const r = document.createElement('div');
    r.className = 'row';
    const v = scene[k] ?? def;
    r.innerHTML = `<label>${label}</label><input type="range" min="${min}" max="${max}" step="${step}" value="${v}"><output>${v}</output>`;
    const inp = r.querySelector('input');
    inp.oninput = () => (r.querySelector('output').textContent = inp.value);
    inp.onchange = () => change('g-disp', k, +inp.value);
    gv.appendChild(r);
  }
  const bn = document.createElement('div');
  bn.className = 'chk';
  bn.innerHTML = `<input type="checkbox" ${scene.background_neutralization ? 'checked' : ''}> Background neutralization (per-channel black)`;
  bn.querySelector('input').onchange = (e) => change('g-disp', 'background_neutralization', e.target.checked);
  gv.appendChild(bn);
  // scene groups
  for (const [g, list] of Object.entries(groupsFor(scene))) {
    const el = document.getElementById(g);
    el.innerHTML = '';
    for (const [key, label, min, max, step, kind] of list) {
      const r = document.createElement('div');
      if (kind === 'bool') {
        r.className = 'chk';
        r.innerHTML = `<input type="checkbox" ${(scene[key] ?? key === 'lensing') ? 'checked' : ''}> ${label}`;
        r.querySelector('input').onchange = (e) => change(g, key, e.target.checked);
      } else {
        r.className = 'row';
        const v = scene[key] ?? (key === 'lensing' ? true : 0);
        const toSlider = (x) => (kind === 'log' ? Math.log10(Math.max(x, min)) : x);
        const fromSlider = (x) => (kind === 'log' ? 10 ** x : x);
        const [smin, smax, sstep] = kind === 'log' ? [Math.log10(min), Math.log10(max), 0.01] : [min, max, step];
        r.innerHTML = `<label title="${label}">${label}</label><input type="range" min="${smin}" max="${smax}" step="${sstep}" value="${toSlider(v)}"><output>${fmt(v, kind)}</output>`;
        const inp = r.querySelector('input');
        inp.oninput = () => (r.querySelector('output').textContent = fmt(fromSlider(+inp.value), kind));
        inp.onchange = () => { let x = fromSlider(+inp.value); if (step >= 1) x = Math.round(x); change(g, key, x); };
      }
      el.appendChild(r);
    }
    if (g === 'g-filt') {
      const chips = document.createElement('div');
      chips.className = 'chips';
      chips.style.margin = '8px 0';
      for (const f of FILTERS) {
        const b = document.createElement('button');
        b.textContent = f;
        b.className = (scene.filters || []).includes(f) ? 'on' : '';
        b.onclick = () => {
          const set = new Set(scene.filters || []);
          set.has(f) ? set.delete(f) : set.add(f);
          if (!set.size) return;
          change(g, 'filters', FILTERS.filter((x) => set.has(x)));
        };
        chips.appendChild(b);
      }
      el.prepend(chips);
      const help = document.createElement('p');
      help.className = 'help';
      help.textContent = 'Filters × QE act on the stored spectrum, so switching filters re-reads the same render.';
      el.appendChild(help);
    }
  }
}

async function change(group, key, value) {
  scene[key] = value;
  if (RERENDER.has(group)) {
    await initScene();
  } else if (group === 'g-filt') {
    busy = true;
    const sj = JSON.stringify(scene);
    await all((w) => ask(w, { cmd: 'instrument', scene: sj }, 'instrumented'));
    cCall(C.c_init, sj);
    busy = false;
    buildControls();
    await show(true);
  } else {
    cCall(C.c_init, JSON.stringify(scene));
    await show(true);
  }
}

// ---------------------------------------------------------- checkpoints --
const NCH = 26, ND = 18, PER = NCH * 4 + 16 + ND * 4;

async function saveCheckpoint() {
  const was = running; running = false;
  await new Promise((r) => setTimeout(r, 50));
  const parts = await all((w) => ask(w, { cmd: 'get_accum' }, 'accum'));
  C.c_scene_json();
  const head = cOut();
  const W = scene.width, H = scene.height, npx = W * H;
  const sum = new Float32Array(npx * NCH), lsum = new Float64Array(npx), lsq = new Float64Array(npx), diag = new Float32Array(npx * ND);
  parts.forEach((p, k) => {
    const dv = new DataView(p.buf);
    let i = 0, r = 0;
    for (let y = k; y < H; y += nw, r++) {
      for (let x = 0; x < W; x++) {
        const q = y * W + x;
        for (let c = 0; c < NCH; c++) { sum[q * NCH + c] = dv.getFloat32(i, true); i += 4; }
        lsum[q] = dv.getFloat64(i, true); i += 8;
        lsq[q] = dv.getFloat64(i, true); i += 8;
        for (let d = 0; d < ND; d++) { diag[q * ND + d] = dv.getFloat32(i, true); i += 4; }
      }
    }
  });
  const hdr = new Uint8Array(8 + 4 + head.length + 12);
  const hv = new DataView(hdr.buffer);
  hdr.set(enc.encode('OBSCKPT2'), 0);
  hv.setUint32(8, head.length, true);
  hdr.set(head, 12);
  hv.setUint32(12 + head.length, W, true); hv.setUint32(16 + head.length, H, true); hv.setUint32(20 + head.length, spp, true);
  download(new Blob([hdr, sum, lsum, lsq, diag]), `${presetName}-spp${spp}.ckpt`);
  if (was) $('#run').click();
}

async function loadCheckpoint(file) {
  running = false;
  const b = new Uint8Array(await file.arrayBuffer());
  if (dec.decode(b.slice(0, 8)) !== 'OBSCKPT2') return alert('Not an Observatory checkpoint.');
  const dv = new DataView(b.buffer);
  const hl = dv.getUint32(8, true);
  const head = JSON.parse(dec.decode(b.slice(12, 12 + hl)));
  let o = 12 + hl;
  const W = dv.getUint32(o, true), H = dv.getUint32(o + 4, true), cspp = dv.getUint32(o + 8, true);
  o += 12;
  scene = head.scene;
  const npx = W * H;
  const sumOff = o, lsumOff = sumOff + npx * NCH * 4, lsqOff = lsumOff + npx * 8, diagOff = lsqOff + npx * 8;
  await initScene(async () => {
    await all((w, k) => {
      const rows = Math.ceil((H - k) / nw);
      const buf = new ArrayBuffer(rows * W * PER);
      const out = new DataView(buf);
      let i = 0;
      for (let y = k; y < H; y += nw) for (let x = 0; x < W; x++) {
        const q = y * W + x;
        for (let c = 0; c < NCH; c++) { out.setFloat32(i, dv.getFloat32(sumOff + (q * NCH + c) * 4, true), true); i += 4; }
        out.setFloat64(i, dv.getFloat64(lsumOff + q * 8, true), true); i += 8;
        out.setFloat64(i, dv.getFloat64(lsqOff + q * 8, true), true); i += 8;
        for (let d = 0; d < ND; d++) { out.setFloat32(i, dv.getFloat32(diagOff + (q * ND + d) * 4, true), true); i += 4; }
      }
      return ask(w, { cmd: 'set_accum', buf, spp: cspp }, 'accum_set', [buf]);
    });
    spp = cspp;
  });
}

// --------------------------------------------------------------- export --
function download(blob, name) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}

function exportFile(kind) {
  const k = { png: 0, exr: 1, fits: 2, json: 3 }[kind];
  const date = new Date().toISOString().slice(0, 19);
  cCall((vp, vn, dp, dn, ip, inn) => C.c_export(k, vp, vn, dp, dn, ip, inn, wall()), disp.view, date, volumeId);
  const types = { png: 'image/png', exr: 'image/x-exr', fits: 'application/fits', json: 'application/json' };
  download(new Blob([cOut()], { type: types[kind] }), `${presetName}-spp${spp}.${kind}`);
}

boot().catch((e) => { setState('Error', 'paused'); $('#notice').textContent = `Could not start: ${e.message}`; console.error(e); });
