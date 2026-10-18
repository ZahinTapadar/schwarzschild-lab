import {
  FULLSCREEN_VS, RAYTRACE_FS, DOWNSAMPLE_FS, UPSAMPLE_FS, COMPOSITE_FS,
  POINTS_VS, POINTS_FS, LINES_VS, LINES_FS,
} from './shaders.js';

const $ = (s) => document.querySelector(s);
const fail = (msg) => { const e = $('#err'); e.style.display = 'grid'; e.textContent = msg; throw new Error(msg); };

// ------------------------------------------------------------ physics ---
const wasm = await WebAssembly.instantiateStreaming(fetch('engine.wasm'))
  .catch(async () => WebAssembly.instantiate(await (await fetch('engine.wasm')).arrayBuffer()));
const E = wasm.instance.exports;
const STRIDE = 8;

// ---------------------------------------------------------------- GL ---
const canvas = $('#c');
const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, powerPreference: 'high-performance' });
if (!gl) fail('WebGL2 is required.');
const floatRT = !!(gl.getExtension('EXT_color_buffer_float') || gl.getExtension('EXT_color_buffer_half_float'));
gl.getExtension('OES_texture_float_linear');

function compile(vs, fs) {
  const p = gl.createProgram();
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) fail(gl.getShaderInfoLog(s));
    gl.attachShader(p, s);
  }
  gl.bindAttribLocation(p, 0, 'aPos');
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) fail(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const info = gl.getActiveUniform(p, i);
    u[info.name.replace('[0]', '')] = gl.getUniformLocation(p, info.name);
  }
  return { p, u };
}
const P = {
  ray: compile(FULLSCREEN_VS, RAYTRACE_FS),
  down: compile(FULLSCREEN_VS, DOWNSAMPLE_FS),
  up: compile(FULLSCREEN_VS, UPSAMPLE_FS),
  comp: compile(FULLSCREEN_VS, COMPOSITE_FS),
  pts: compile(POINTS_VS, POINTS_FS),
  lines: compile(LINES_VS, LINES_FS),
};

const quad = gl.createVertexArray();
gl.bindVertexArray(quad);
gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
gl.enableVertexAttribArray(0);
gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

// debris VAO, reading the engine's packed buffer directly
const ptsBuf = gl.createBuffer();
const ptsVao = gl.createVertexArray();
gl.bindVertexArray(ptsVao);
gl.bindBuffer(gl.ARRAY_BUFFER, ptsBuf);
{
  const p = P.pts.p;
  const attr = (name, size, off) => {
    const loc = gl.getAttribLocation(p, name);
    if (loc < 0) return;
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, size, gl.FLOAT, false, STRIDE * 4, off * 4);
  };
  attr('aPos', 3, 0); attr('aRadius', 1, 3); attr('aKind', 1, 4); attr('aRate', 1, 5); attr('aColor', 1, 6);
}

const lineBuf = gl.createBuffer();
const lineVao = gl.createVertexArray();
gl.bindVertexArray(lineVao);
gl.bindBuffer(gl.ARRAY_BUFFER, lineBuf);
{
  const p = P.lines.p;
  const lp = gl.getAttribLocation(p, 'aPos'), lc = gl.getAttribLocation(p, 'aCol');
  gl.enableVertexAttribArray(lp); gl.vertexAttribPointer(lp, 3, gl.FLOAT, false, 28, 0);
  gl.enableVertexAttribArray(lc); gl.vertexAttribPointer(lc, 4, gl.FLOAT, false, 28, 12);
}
gl.bindVertexArray(null);

// --------------------------------------------------- blackbody table ---
// Planck spectrum integrated against CIE 1931 colour-matching functions
// (Wyman-Sloan-Shirley analytic fit) -> linear sRGB, normalised to Y = 1.
function blackbodyTable(n = 256) {
  const g = (x, mu, s1, s2) => { const t = (x - mu) / (x < mu ? s1 : s2); return Math.exp(-0.5 * t * t); };
  const data = new Float32Array(n * 4);
  for (let i = 0; i < n; i++) {
    const T = 800 * Math.pow(40000 / 800, i / (n - 1));
    let X = 0, Y = 0, Z = 0;
    for (let l = 380; l <= 780; l += 5) {
      const lm = l * 1e-9;
      const B = 1 / (Math.pow(lm, 5) * (Math.exp(1.4388e-2 / (lm * T)) - 1));
      X += B * (1.056 * g(l, 599.8, 37.9, 31.0) + 0.362 * g(l, 442.0, 16.0, 26.7) - 0.065 * g(l, 501.1, 20.4, 26.2));
      Y += B * (0.821 * g(l, 568.8, 46.9, 40.5) + 0.286 * g(l, 530.9, 16.3, 31.1));
      Z += B * (1.217 * g(l, 437.0, 11.8, 36.0) + 0.681 * g(l, 459.0, 26.0, 13.8));
    }
    X /= Y; Z /= Y; Y = 1;
    data[i * 4 + 0] = Math.max(0, 3.2406 * X - 1.5372 * Y - 0.4986 * Z);
    data[i * 4 + 1] = Math.max(0, -0.9689 * X + 1.8758 * Y + 0.0415 * Z);
    data[i * 4 + 2] = Math.max(0, 0.0557 * X - 0.2040 * Y + 1.0570 * Z);
    data[i * 4 + 3] = 1;
  }
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, n, 1, 0, gl.RGBA, gl.FLOAT, data);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  return tex;
}
const bbTex = blackbodyTable();

// ----------------------------------------------------- render targets ---
function target(w, h) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  if (floatRT) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, w, h, 0, gl.RGBA, gl.HALF_FLOAT, null);
  else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  return { tex, fb, w, h };
}
const freeTarget = (t) => { gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fb); };
let hdr = null, bloom = [];
function buildTargets() {
  if (hdr) { freeTarget(hdr); bloom.forEach(freeTarget); }
  const w = Math.max(64, Math.round(canvas.width * params.renderScale));
  const h = Math.max(64, Math.round(canvas.height * params.renderScale));
  hdr = target(w, h);
  bloom = [];
  let bw = w, bh = h;
  for (let i = 0; i < 6; i++) {
    bw = Math.max(2, bw >> 1); bh = Math.max(2, bh >> 1);
    bloom.push(target(bw, bh));
  }
}

// ------------------------------------------------------------ params ---
const dpr = Math.min(window.devicePixelRatio || 1, 2);
const params = {
  M: 1, diskIn: 6, diskOut: 22, diskTemp: 4300, diskBright: 2.6, diskOpacity: 0.95,
  doppler: 0, beaming: 4, diskSpeed: 1, showDisk: true,
  steps: 500, stepScale: 1, renderScale: dpr > 1 ? 0.6 : 1, autoRes: true,
  exposure: 1.15, bloom: 0.55, bloomThreshold: 1.2, stars: 1, nebula: 1, fov: 42,
  timeScale: 12, properTime: false, nbody: true, accrete: true, paused: false,
  trails: true, fabric: false, fabricDepth: 0.55, fabricOffset: 4, fabricMassExag: 250,
  spawnType: 'planet', spawnMass: 0.002, vFactor: 1, incl: 0, cloudN: 3000,
  camMotion: 'orbit', orbitSpeed: 1, bob: true, ambientGas: true, gasCount: 2500,
};
const cam = { yaw: 0, pitch: 0.055, dist: 36, tyaw: 0, tpitch: 0.055, tdist: 36, bob: 0 };

// ---------------------------------------------------------------- UI ---
const ctl = $('#controls');
const ui = {};
function section(title, open = false) {
  const d = document.createElement('details'); d.open = open;
  d.innerHTML = `<summary>${title}</summary>`;
  ctl.appendChild(d);
  return d;
}
function slider(sec, key, label, min, max, step, onChange, fmt = (v) => (+v).toFixed(step < 0.01 ? 4 : step < 1 ? 2 : 0)) {
  const row = document.createElement('div'); row.className = 'row';
  row.innerHTML = `<label title="${label}">${label}</label><input type="range" min="${min}" max="${max}" step="${step}"><output></output>`;
  const inp = row.querySelector('input'), out = row.querySelector('output');
  const sync = () => { inp.value = params[key]; out.textContent = fmt(params[key]); };
  inp.addEventListener('input', () => { params[key] = +inp.value; out.textContent = fmt(params[key]); onChange?.(params[key]); });
  sec.appendChild(row); sync(); ui[key] = sync;
}
function check(sec, key, label, onChange) {
  const row = document.createElement('label'); row.className = 'chk';
  row.innerHTML = `<input type="checkbox"> ${label}`;
  const inp = row.querySelector('input');
  const sync = () => { inp.checked = params[key]; };
  inp.addEventListener('change', () => { params[key] = inp.checked; onChange?.(inp.checked); });
  sec.appendChild(row); sync(); ui[key] = sync;
}
function select(sec, key, label, opts) {
  const row = document.createElement('div'); row.className = 'row';
  row.innerHTML = `<label>${label}</label><select style="grid-column: span 2">${opts.map(([v, t]) => `<option value="${v}">${t}</option>`).join('')}</select>`;
  const s = row.querySelector('select');
  const sync = () => { s.value = params[key]; };
  s.addEventListener('change', () => { params[key] = s.value; });
  sec.appendChild(row); sync(); ui[key] = sync;
}
function buttons(sec, list) {
  const b = document.createElement('div'); b.className = 'btns';
  for (const [t, fn] of list) { const e = document.createElement('button'); e.textContent = t; e.onclick = fn; b.appendChild(e); }
  sec.appendChild(b);
}
const syncAll = () => Object.values(ui).forEach((f) => f());
const syncFlags = () => E.set_flags(+params.properTime, +params.nbody, +params.accrete);

let s = section('Black hole', true);
slider(s, 'M', 'Mass M', 0.2, 4, 0.01, (v) => E.set_mass(v));
s = section('Accretion disk', true);
check(s, 'showDisk', 'Show disk');
slider(s, 'diskIn', 'Inner edge (M)', 2.5, 12, 0.1);
slider(s, 'diskOut', 'Outer edge (M)', 8, 60, 0.5);
slider(s, 'diskTemp', 'Peak temp (K)', 1500, 30000, 50);
slider(s, 'diskBright', 'Brightness', 0.1, 10, 0.05);
slider(s, 'diskOpacity', 'Opacity', 0.05, 1, 0.01);
slider(s, 'doppler', 'Doppler shift', 0, 1, 0.01);
slider(s, 'beaming', 'Beaming exp.', 0, 4, 0.1);
slider(s, 'diskSpeed', 'Spin anim.', 0, 5, 0.05);
s = section('Observer motion', true);
select(s, 'camMotion', 'Camera', [['orbit', 'Orbiting (aberration + Doppler)'], ['drift', 'Slow drift'], ['static', 'Static']]);
slider(s, 'orbitSpeed', 'Orbit speed', 0, 3, 0.01, null, (v) => (v === 1 ? 'free fall' : '×' + (+v).toFixed(2)));
check(s, 'bob', 'Gentle vertical drift');
check(s, 'ambientGas', 'Ambient infalling gas');
slider(s, 'gasCount', 'Gas particles', 0, 10000, 100);
s = section('Rendering');
slider(s, 'fov', 'FOV (deg)', 10, 100, 1);
slider(s, 'steps', 'Max ray steps', 60, 2000, 10);
slider(s, 'stepScale', 'Step size', 0.25, 3, 0.05);
slider(s, 'renderScale', 'Resolution', 0.2, 1, 0.05, () => buildTargets());
check(s, 'autoRes', 'Adaptive resolution (target 50 fps)');
slider(s, 'exposure', 'Exposure', 0.1, 5, 0.05);
slider(s, 'bloom', 'Bloom', 0, 2, 0.01);
slider(s, 'bloomThreshold', 'Bloom thresh.', 0, 5, 0.05);
slider(s, 'stars', 'Stars', 0, 3, 0.05);
slider(s, 'nebula', 'Galaxy', 0, 3, 0.05);
s = section('Physics', true);
slider(s, 'timeScale', 'Speed (M/s)', 0, 200, 0.5);
check(s, 'paused', 'Paused (space)');
check(s, 'properTime', "Integrate in proper time (off: distant observer's clock)", syncFlags);
check(s, 'nbody', 'Bodies attract each other', syncFlags);
check(s, 'accrete', 'Swallowed mass grows the hole', syncFlags);
check(s, 'trails', 'Orbit trails', () => trails.clear());
check(s, 'fabric', 'Spacetime fabric (Flamm embedding)');
slider(s, 'fabricDepth', 'Fabric depth', 0.1, 1.5, 0.01);
slider(s, 'fabricMassExag', 'Body well exag.', 0, 1000, 5);
s = section('Spawn (click the disk plane)', true);
select(s, 'spawnType', 'Object', [['planet', 'Planet (lensed sphere)'], ['star', 'Star (emissive)'], ['cloud', 'Gas cloud (tidal debris)'], ['swarm', 'Swarm of 12 planets']]);
slider(s, 'spawnMass', 'Mass (M)', 0, 0.1, 0.0005);
slider(s, 'vFactor', 'Speed × v_circ', 0, 1.6, 0.01);
slider(s, 'incl', 'Inclination°', -90, 90, 1);
slider(s, 'cloudN', 'Cloud particles', 200, 20000, 100);
buttons(s, [['Clear objects', () => { E.reset(); trails.clear(); selected = 0; }]]);

$('#toggle').onclick = () => $('#panel').classList.toggle('hidden');

// ------------------------------------------------------------- spawn ---
const PALETTE = [[0.55, 0.7, 1.0], [0.9, 0.75, 0.55], [0.6, 0.9, 0.7], [0.95, 0.55, 0.45],
  [0.8, 0.8, 0.85], [0.7, 0.55, 0.95], [1.0, 0.9, 0.5], [0.45, 0.85, 0.95]];
let colorCounter = 0;

function spawnAt(px, pz, o = {}) {
  const type = o.type ?? params.spawnType;
  const M = E.get_mass();
  const r = Math.hypot(px, pz);
  if (r < 3.2 * M) return false;
  const vf = o.vFactor ?? params.vFactor;
  const inc = ((o.incl ?? params.incl) * Math.PI) / 180;
  const vc = E.circular_v_tau(r);
  // prograde tangent (same sense as the disk), tilted by the inclination
  const tx = pz / r, tz = -px / r;
  const ux = tx * Math.cos(inc) * vc * vf, uy = Math.sin(inc) * vc * vf, uz = tz * Math.cos(inc) * vc * vf;
  const mass = o.mass ?? params.spawnMass;
  if (type === 'cloud') {
    E.add_cloud(px, 0, pz, ux, uy, uz, o.radius ?? 1.5 * M, o.n ?? params.cloudN, 0.0015, o.color ?? 0);
  } else if (type === 'swarm') {
    for (let i = 0; i < 12; i++) {
      const a = Math.random() * Math.PI * 2, rr = r * (0.75 + Math.random() * 0.6);
      spawnAt(Math.cos(a) * rr, Math.sin(a) * rr, { type: 'planet', vFactor: vf * (0.85 + Math.random() * 0.3), incl: (Math.random() - 0.5) * 50 });
    }
  } else {
    const star = type === 'star';
    const col = o.color ?? (star ? 6 : colorCounter++ % PALETTE.length);
    const radius = o.radius ?? (star ? 1.0 : 0.45 + Math.random() * 0.25);
    const id = E.add_particle_tau(px, 0, pz, ux, uy, uz, mass, radius, 0, col);
    bodyMeta.set(id, { star, col, mass });
  }
  return true;
}
const spawnPolar = (r, deg, o) => spawnAt(r * Math.cos((deg * Math.PI) / 180), r * Math.sin((deg * Math.PI) / 180), o);
const bodyMeta = new Map();

// ----------------------------------------------------------- presets ---
const PRESETS = {
  'Gargantua': () => {
    Object.assign(params, { M: 1, diskIn: 6, diskOut: 22, diskTemp: 4300, diskBright: 2.6, doppler: 0, beaming: 4, fabric: false, showDisk: true, exposure: 1.15, fov: 42 });
    Object.assign(cam, { tyaw: 0, tpitch: 0.055, tdist: 36 });
    E.reset(); trails.clear();
  },
  'Physical (Doppler)': () => {
    Object.assign(params, { M: 1, diskIn: 6, diskOut: 22, diskTemp: 6500, diskBright: 1.4, doppler: 1, beaming: 4, fabric: false, showDisk: true });
    Object.assign(cam, { tyaw: 0, tpitch: 0.12, tdist: 36 });
  },
  'Orbit zoo': () => {
    Object.assign(params, { M: 1, fabric: false, trails: true, nbody: true, doppler: 0, diskOpacity: 0.95, timeScale: 25 });
    Object.assign(cam, { tyaw: 0.4, tpitch: 0.62, tdist: 62 });
    E.reset(); trails.clear();
    spawnPolar(9, 0, { type: 'planet', vFactor: 1, incl: 0, mass: 0.001 });
    spawnPolar(16, 90, { type: 'planet', vFactor: 0.82, incl: 0, mass: 0.001 });
    spawnPolar(24, 200, { type: 'planet', vFactor: 1, incl: 35, mass: 0.001 });
    spawnPolar(30, 300, { type: 'planet', vFactor: 0.62, incl: -10, mass: 0.001 });
    spawnPolar(38, 150, { type: 'star', vFactor: 1, incl: 12, mass: 0.002 });
  },
  'Tidal disruption': () => {
    Object.assign(params, { M: 1, fabric: false, trails: false, timeScale: 30, doppler: 0 });
    Object.assign(cam, { tyaw: -0.3, tpitch: 0.75, tdist: 70 });
    E.reset(); trails.clear();
    spawnPolar(55, 40, { type: 'cloud', vFactor: 0.5, incl: 6, radius: 2.5, n: 8000 });
  },
  'Plunge & freeze': () => {
    Object.assign(params, { M: 1, fabric: false, trails: true, properTime: false, timeScale: 15 });
    Object.assign(cam, { tyaw: 0.2, tpitch: 0.5, tdist: 45 });
    E.reset(); trails.clear();
    for (let i = 0; i < 5; i++) spawnPolar(22 + i * 3, i * 72, { type: 'planet', vFactor: 0.25 + i * 0.05, incl: 0, mass: 0.0 });
  },
  'Spacetime fabric': () => {
    Object.assign(params, { M: 1, fabric: true, trails: true, nbody: true, timeScale: 25, fabricDepth: 0.55 });
    Object.assign(cam, { tyaw: 0.3, tpitch: 0.5, tdist: 75 });
    E.reset(); trails.clear();
    spawnPolar(14, 0, { type: 'star', vFactor: 1, incl: 0, mass: 0.03 });
    spawnPolar(26, 120, { type: 'planet', vFactor: 0.9, incl: 0, mass: 0.02 });
    spawnPolar(28, 125, { type: 'planet', vFactor: 0.9, incl: 0, mass: 0.002 });
    spawnPolar(36, 250, { type: 'planet', vFactor: 1.0, incl: 0, mass: 0.01 });
  },
};
{
  const box = $('#presets');
  for (const [name, fn] of Object.entries(PRESETS)) {
    const b = document.createElement('button'); b.textContent = name;
    b.onclick = () => { fn(); E.set_mass(params.M); syncFlags(); syncAll(); };
    box.appendChild(b);
  }
}

// ------------------------------------------------------------- math ---
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const crossV = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };
const dotV = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
function mul4(a, b) {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    let s = 0; for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k]; o[c * 4 + r] = s;
  }
  return o;
}

// Observer velocity measured by a local static observer, and the two
// directions of the aberration map (static frame <-> observer frame).
let obsVel = [0, 0, 0], obsGamma = 1;
function toObserver(d) {
  const b = Math.hypot(...obsVel);
  if (b < 1e-6) return d;
  const bh = obsVel.map((x) => x / b);
  const k = (obsGamma - 1) * dotV(d, bh) + obsGamma * b;
  return norm([d[0] + k * bh[0], d[1] + k * bh[1], d[2] + k * bh[2]]);
}
function toStatic(d) {
  const b = Math.hypot(...obsVel);
  if (b < 1e-6) return d;
  const bh = obsVel.map((x) => x / b);
  const k = (obsGamma - 1) * dotV(d, bh) - obsGamma * b;
  return norm([d[0] + k * bh[0], d[1] + k * bh[1], d[2] + k * bh[2]]);
}

function cameraFrame() {
  const pitch = cam.pitch + cam.bob;
  const pos = [cam.dist * Math.cos(pitch) * Math.sin(cam.yaw), cam.dist * Math.sin(pitch), cam.dist * Math.cos(pitch) * Math.cos(cam.yaw)];
  // aim at where the hole *appears* after aberration, keeping it centred
  const fwd = toObserver(norm([-pos[0], -pos[1], -pos[2]]));
  const right = norm(crossV(fwd, [0, 1, 0]));
  const up = crossV(right, fwd);
  const tanFov = Math.tan((params.fov * Math.PI) / 360);
  const aspect = canvas.width / canvas.height;
  const view = new Float32Array([
    right[0], up[0], -fwd[0], 0, right[1], up[1], -fwd[1], 0, right[2], up[2], -fwd[2], 0,
    -dotV(right, pos), -dotV(up, pos), dotV(fwd, pos), 1]);
  const n = 0.05, f = 10000;
  const proj = new Float32Array([1 / (tanFov * aspect), 0, 0, 0, 0, 1 / tanFov, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, (2 * f * n) / (n - f), 0]);
  return { pos, fwd, right, up, tanFov, aspect, viewProj: mul4(proj, view) };
}

function project(vp, p0) {
  const rel = sub(p0, frame.pos), dist = Math.hypot(...rel);
  const d = toObserver(rel.map((x) => x / dist));
  const p = [frame.pos[0] + d[0] * dist, frame.pos[1] + d[1] * dist, frame.pos[2] + d[2] * dist];
  const x = vp[0] * p[0] + vp[4] * p[1] + vp[8] * p[2] + vp[12];
  const y = vp[1] * p[0] + vp[5] * p[1] + vp[9] * p[2] + vp[13];
  const w = vp[3] * p[0] + vp[7] * p[1] + vp[11] * p[2] + vp[15];
  if (w <= 0) return null;
  return [((x / w) * 0.5 + 0.5) * canvas.clientWidth, (1 - ((y / w) * 0.5 + 0.5)) * canvas.clientHeight];
}

// ------------------------------------------------------------- input ---
let drag = null, selected = 0, frame = null;
canvas.addEventListener('pointerdown', (e) => { drag = { x: e.clientX, y: e.clientY, sx: e.clientX, sy: e.clientY }; canvas.setPointerCapture(e.pointerId); });
canvas.addEventListener('pointermove', (e) => {
  if (!drag) return;
  cam.tyaw -= (e.clientX - drag.x) * 0.005;
  cam.tpitch = Math.max(-1.5, Math.min(1.5, cam.tpitch + (e.clientY - drag.y) * 0.005));
  drag.x = e.clientX; drag.y = e.clientY;
});
canvas.addEventListener('pointerup', (e) => {
  if (drag && Math.hypot(e.clientX - drag.sx, e.clientY - drag.sy) < 4) click(e.clientX, e.clientY);
  drag = null;
});
canvas.addEventListener('wheel', (e) => {
  e.preventDefault();
  cam.tdist = Math.max(3.5 * E.get_mass(), Math.min(600, cam.tdist * Math.exp(e.deltaY * 0.001)));
}, { passive: false });
addEventListener('keydown', (e) => {
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
  if (e.key === 'h' || e.key === 'H') $('#panel').classList.toggle('hidden');
  if (e.key === ' ') { params.paused = !params.paused; ui.paused(); e.preventDefault(); }
});

function click(cx, cy) {
  if (!frame) return;
  // pick a body first
  let best = 0, bd = 18;
  for (const b of bodies) {
    const s = project(frame.viewProj, b.p);
    if (!s) continue;
    const d = Math.hypot(s[0] - cx, s[1] - cy);
    if (d < bd) { bd = d; best = b.id; }
  }
  if (best) { selected = best; return; }
  selected = 0;
  // straight-line ray to the equatorial plane (spawn placement only)
  const u = (cx / canvas.clientWidth) * 2 - 1, v = 1 - (cy / canvas.clientHeight) * 2;
  const d = toStatic(norm([0, 1, 2].map((i) => frame.fwd[i] + u * frame.tanFov * frame.aspect * frame.right[i] + v * frame.tanFov * frame.up[i])));
  if (Math.abs(d[1]) < 1e-5) return;
  const t = -frame.pos[1] / d[1];
  if (t <= 0) return;
  spawnAt(frame.pos[0] + d[0] * t, frame.pos[2] + d[2] * t);
}

// ------------------------------------------------------------ trails ---
const trails = new Map();
function updateTrails() {
  if (!params.trails) return;
  const alive = new Set();
  for (const b of bodies) {
    alive.add(b.id);
    let t = trails.get(b.id);
    if (!t) trails.set(b.id, (t = { pts: [], col: PALETTE[b.col % PALETTE.length] }));
    const last = t.pts[t.pts.length - 1];
    if (!last || Math.hypot(last[0] - b.p[0], last[1] - b.p[1], last[2] - b.p[2]) > 0.08) t.pts.push(b.p.slice());
    if (t.pts.length > 1500) t.pts.shift();
  }
  for (const id of trails.keys()) if (!alive.has(id)) trails.delete(id);
}

// ------------------------------------------------------------ fabric ---
function fabricLines(out) {
  const M = E.get_mass();
  const ext = Math.max(45, 45 * M);
  const N = 48, S = 160;
  const flamm = (r) => 2 * Math.sqrt(2 * M * Math.max(r - 2 * M, 0));
  const top = flamm(ext * 1.42);
  const depth = params.fabricDepth;
  const wells = bodies.filter((b) => b.m > 0).map((b) => [b.p[0], b.p[2], b.m * params.fabricMassExag, b.radius]);
  const height = (x, z) => {
    const r = Math.max(Math.hypot(x, z), 2 * M);
    let y = -params.fabricOffset * M - depth * (top - flamm(r));
    for (const [wx, wz, k, rad] of wells) {
      const d2 = (x - wx) ** 2 + (z - wz) ** 2;
      y -= (depth * k) / Math.sqrt(d2 + (1.5 + rad) ** 2);
    }
    return y;
  };
  const pushV = (x, z) => {
    const r = Math.hypot(x, z);
    const a = 0.5 * (1 - Math.min(1, Math.max(0, (r - ext * 0.6) / (ext * 0.4)))) * (r < 2 * M ? 0 : 1);
    const glow = Math.min(1, (6 * M) / r);
    out.push(x, height(x, z), z, 0.3 + 0.7 * glow, 0.62 + 0.25 * glow, 1.0, a);
  };
  for (let axis = 0; axis < 2; axis++) {
    for (let i = 0; i <= N; i++) {
      const c = -ext + (2 * ext * i) / N;
      for (let j = 0; j < S; j++) {
        const a = -ext + (2 * ext * j) / S, b = -ext + (2 * ext * (j + 1)) / S;
        if (axis === 0) { pushV(a, c); pushV(b, c); } else { pushV(c, a); pushV(c, b); }
      }
    }
  }
  // drop lines from each body to its well on the fabric
  for (const bd of bodies) {
    const col = PALETTE[bd.col % PALETTE.length];
    out.push(bd.p[0], bd.p[1], bd.p[2], col[0], col[1], col[2], 0.5);
    out.push(bd.p[0], height(bd.p[0], bd.p[2]), bd.p[2], col[0], col[1], col[2], 0.0);
  }
}

// ------------------------------------------------------- ambient gas ---
// A sparse, thick halo of massless gas on randomly inclined geodesics. Some
// orbits are bound, some plunge; swallowed gas is replenished from far out.
function spawnGasParticle(rMin, rMax, M) {
  const r = rMin + Math.pow(Math.random(), 0.8) * (rMax - rMin);
  const a = Math.random() * Math.PI * 2;
  const inc = (Math.random() + Math.random() + Math.random() - 1.5) * 0.45;
  const vc = E.circular_v_tau(r) * (0.72 + Math.random() * 0.32);
  const x = r * Math.cos(a), z = r * Math.sin(a), y = (Math.random() - 0.5) * 0.8 * M;
  const tx = Math.sin(a), tz = -Math.cos(a);
  E.add_particle_tau(x, y, z, tx * Math.cos(inc) * vc, Math.sin(inc) * vc, tz * Math.cos(inc) * vc, 0, 0, 1, 2);
}
function feedGas(have, M) {
  const want = params.gasCount;
  if (have >= want * 0.97) return;
  if (have < want * 0.3) { for (let i = have; i < want; i++) spawnGasParticle(8 * M, 70 * M, M); return; }
  for (let i = 0; i < Math.min(25, want - have); i++) spawnGasParticle(50 * M, 75 * M, M);
}

// -------------------------------------------------------------- loop ---
let bodies = [];
let simT = 0, last = performance.now(), fpsAcc = 0, fpsN = 0, fps = 60, resTimer = 0;
const bodyU = new Float32Array(64), bodyC = new Float32Array(64), bodyA = new Float32Array(64);

function resize() {
  const w = Math.round(innerWidth * dpr), h = Math.round(innerHeight * dpr);
  if (canvas.width !== w || canvas.height !== h || !hdr) { canvas.width = w; canvas.height = h; buildTargets(); }
}

function pass(prog, fb, w, h) {
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.viewport(0, 0, w, h);
  gl.useProgram(prog.p);
  gl.bindVertexArray(quad);
}

function loop(now) {
  const dt = Math.max(0, Math.min(0.05, (now - last) / 1000));
  last = now;
  fpsAcc += dt; fpsN++;
  if (fpsAcc > 0.5) { fps = fpsN / fpsAcc; fpsAcc = 0; fpsN = 0; }
  resize();

  // adaptive resolution
  resTimer += dt;
  if (params.autoRes && resTimer > 1.2) {
    resTimer = 0;
    const old = params.renderScale;
    if (fps < 42) params.renderScale = Math.max(0.25, old - 0.05);
    else if (fps > 57 && old < (dpr > 1 ? 0.75 : 1)) params.renderScale = Math.min(1, old + 0.05);
    if (old !== params.renderScale) { buildTargets(); ui.renderScale(); }
  }

  // physics
  if (!params.paused && params.timeScale > 0) {
    const h = dt * params.timeScale;
    E.step(h);
    simT += h;
  }
  const n = E.pack();
  const buf = new Float32Array(E.memory.buffer, E.out_ptr(), n * STRIDE);
  bodies = [];
  let debris = 0, debrisGas = 0;
  for (let i = 0; i < n; i++) {
    const o = i * STRIDE;
    if (buf[o + 4] < 0.5) {
      const id = buf[o + 7];
      bodies.push({ id, p: [buf[o], buf[o + 1], buf[o + 2]], radius: buf[o + 3], rate: buf[o + 5], col: buf[o + 6], m: 0, star: bodyMeta.get(id)?.star });
    } else { debris++; if (buf[o + 6] > 1.5) debrisGas++; }
  }
  // masses (for fabric wells) come from the UI metadata stored at spawn
  for (const b of bodies) b.m = bodyMeta.get(b.id)?.mass ?? 0;
  const M = E.get_mass();
  if (Math.abs(M - params.M) > 1e-6) { params.M = M; ui.M(); }

  // observer motion: a circular orbit about the hole (free fall at speed 1)
  const running = !params.paused && params.timeScale > 0;
  obsVel = [0, 0, 0]; obsGamma = 1;
  if (params.camMotion === 'orbit') {
    const r = cam.dist, lapse = Math.max(1e-3, 1 - (2 * M) / r);
    const omega = params.orbitSpeed * Math.sqrt(M / (r * r * r));      // dphi/dt, coordinate time
    if (running) { const dy = omega * dt * params.timeScale; cam.yaw += dy; cam.tyaw += dy; }
    const rc = r * Math.cos(cam.pitch);
    const vloc = Math.min(0.95, (omega * rc) / Math.sqrt(lapse));     // speed seen by a static observer
    obsVel = [Math.cos(cam.yaw) * vloc, 0, -Math.sin(cam.yaw) * vloc];
    obsGamma = 1 / Math.sqrt(1 - vloc * vloc);
  } else if (params.camMotion === 'drift') {
    cam.yaw += 0.03 * dt; cam.tyaw += 0.03 * dt;
  }
  cam.bob = params.bob && params.camMotion !== 'static' ? 0.035 * Math.sin(now * 0.00012) + 0.015 * Math.sin(now * 0.00031) : 0;
  if (params.ambientGas) feedGas(debrisGas, M);

  cam.yaw += (cam.tyaw - cam.yaw) * Math.min(1, dt * 8);
  cam.pitch += (cam.tpitch - cam.pitch) * Math.min(1, dt * 8);
  cam.dist += (cam.tdist - cam.dist) * Math.min(1, dt * 6);
  cam.dist = Math.max(cam.dist, 2.6 * M);
  frame = cameraFrame();
  updateTrails();

  // ---- ray trace
  const nb = Math.min(16, bodies.length);
  for (let i = 0; i < nb; i++) {
    const b = bodies[i], c = PALETTE[b.col % PALETTE.length];
    bodyU.set([b.p[0], b.p[1], b.p[2], b.radius], i * 4);
    bodyC.set([c[0], c[1], c[2], b.star ? 1 : 0], i * 4);
    bodyA.set([Math.max(0, Math.min(1, b.rate)), (b.id % 97) * 1.37, 0, 0], i * 4);
  }
  const camLapse = Math.sqrt(Math.max(1e-4, 1 - (2 * M) / cam.dist));
  const R = P.ray;
  pass(R, hdr.fb, hdr.w, hdr.h);
  gl.disable(gl.BLEND);
  gl.uniform2f(R.u.uRes, hdr.w, hdr.h);
  gl.uniform3fv(R.u.uCamPos, frame.pos);
  gl.uniformMatrix3fv(R.u.uCamRot, false, [...frame.right, ...frame.up, ...frame.fwd]);
  gl.uniform1f(R.u.uTanFov, frame.tanFov);
  gl.uniform1f(R.u.uM, M);
  gl.uniform1f(R.u.uTime, simT);
  gl.uniform1i(R.u.uSteps, params.steps);
  gl.uniform1f(R.u.uStepScale, params.stepScale);
  gl.uniform1f(R.u.uDiskIn, params.diskIn);
  gl.uniform1f(R.u.uDiskOut, params.diskOut);
  gl.uniform1f(R.u.uDiskTemp, params.diskTemp);
  gl.uniform1f(R.u.uDiskBright, params.diskBright);
  gl.uniform1f(R.u.uDiskOpacity, params.diskOpacity);
  gl.uniform1f(R.u.uDoppler, params.doppler);
  gl.uniform1f(R.u.uBeaming, params.beaming);
  gl.uniform1f(R.u.uDiskSpeed, params.diskSpeed);
  gl.uniform1f(R.u.uShowDisk, params.showDisk ? 1 : 0);
  gl.uniform1f(R.u.uCamLapse, camLapse);
  gl.uniform3fv(R.u.uCamVel, obsVel);
  gl.uniform1f(R.u.uCamGamma, obsGamma);
  gl.uniform1f(R.u.uStars, params.stars);
  gl.uniform1f(R.u.uNebula, params.nebula);
  gl.uniform1i(R.u.uNB, nb);
  if (R.u.uBody) gl.uniform4fv(R.u.uBody, bodyU);
  if (R.u.uBodyCol) gl.uniform4fv(R.u.uBodyCol, bodyC);
  if (R.u.uBodyAux) gl.uniform4fv(R.u.uBodyAux, bodyA);
  gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, bbTex);
  gl.uniform1i(R.u.uBlackbody, 0);
  gl.drawArrays(gl.TRIANGLES, 0, 3);

  // ---- debris (additive into HDR)
  if (debris > 0) {
    gl.bindBuffer(gl.ARRAY_BUFFER, ptsBuf);
    gl.bufferData(gl.ARRAY_BUFFER, buf, gl.DYNAMIC_DRAW);
    gl.useProgram(P.pts.p);
    gl.bindVertexArray(ptsVao);
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE);
    gl.uniformMatrix4fv(P.pts.u.uViewProj, false, frame.viewProj);
    gl.uniform3fv(P.pts.u.uCam, frame.pos);
    gl.uniform3fv(P.pts.u.uCamVel, obsVel);
    gl.uniform1f(P.pts.u.uCamGamma, obsGamma);
    gl.uniform1f(P.pts.u.uM, M);
    gl.uniform1f(P.pts.u.uPx, hdr.h);
    gl.uniform1f(P.pts.u.uShadowSin, Math.min(1, ((Math.sqrt(27) * M) / cam.dist) * camLapse));
    gl.drawArrays(gl.POINTS, 0, n);
    gl.disable(gl.BLEND);
  }

  // ---- bloom chain
  let src = hdr;
  for (let i = 0; i < bloom.length; i++) {
    const t = bloom[i];
    pass(P.down, t.fb, t.w, t.h);
    gl.bindTexture(gl.TEXTURE_2D, src.tex);
    gl.uniform1i(P.down.u.uTex, 0);
    gl.uniform2f(P.down.u.uTexel, 1 / src.w, 1 / src.h);
    gl.uniform1f(P.down.u.uThreshold, params.bloomThreshold);
    gl.uniform1i(P.down.u.uFirst, i === 0 ? 1 : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    src = t;
  }
  gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE);
  for (let i = bloom.length - 2; i >= 0; i--) {
    const t = bloom[i], s2 = bloom[i + 1];
    pass(P.up, t.fb, t.w, t.h);
    gl.bindTexture(gl.TEXTURE_2D, s2.tex);
    gl.uniform1i(P.up.u.uTex, 0);
    gl.uniform2f(P.up.u.uTexel, 1 / s2.w, 1 / s2.h);
    gl.uniform1f(P.up.u.uRadius, 1.0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
  gl.disable(gl.BLEND);

  // ---- composite
  pass(P.comp, null, canvas.width, canvas.height);
  gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, hdr.tex);
  gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, bloom[0].tex);
  gl.uniform1i(P.comp.u.uHdr, 0); gl.uniform1i(P.comp.u.uBloom, 1);
  gl.uniform1f(P.comp.u.uExposure, params.exposure);
  gl.uniform1f(P.comp.u.uBloomStrength, params.bloom);
  gl.uniform1f(P.comp.u.uTime, now / 1000);
  gl.uniform1f(P.comp.u.uDim, params.fabric ? 0.45 : 1);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  gl.activeTexture(gl.TEXTURE0);

  // ---- overlay lines (trails + fabric)
  const lines = [];
  if (params.fabric) fabricLines(lines);
  if (params.trails) {
    for (const t of trails.values()) {
      const p = t.pts, L = p.length;
      for (let i = 1; i < L; i++) {
        const a0 = (0.55 * (i - 1)) / L, a1 = (0.55 * i) / L;
        lines.push(p[i - 1][0], p[i - 1][1], p[i - 1][2], t.col[0], t.col[1], t.col[2], a0);
        lines.push(p[i][0], p[i][1], p[i][2], t.col[0], t.col[1], t.col[2], a1);
      }
    }
  }
  if (lines.length) {
    gl.bindBuffer(gl.ARRAY_BUFFER, lineBuf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(lines), gl.DYNAMIC_DRAW);
    gl.useProgram(P.lines.p);
    gl.bindVertexArray(lineVao);
    gl.uniformMatrix4fv(P.lines.u.uViewProj, false, frame.viewProj);
    gl.uniform3fv(P.lines.u.uCam, frame.pos);
    gl.uniform3fv(P.lines.u.uCamVel, obsVel);
    gl.uniform1f(P.lines.u.uCamGamma, obsGamma);
    gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE);
    gl.drawArrays(gl.LINES, 0, lines.length / 7);
    gl.disable(gl.BLEND);
  }
  gl.bindVertexArray(null);

  hud(M, n - debris, debris, camLapse);
  requestAnimationFrame(loop);
}

// --------------------------------------------------------------- HUD ---
const hudEl = $('#hud'), infoEl = $('#info');
function hud(M, nb, nd, camLapse) {
  const f = (x, d = 2) => x.toFixed(d);
  hudEl.textContent =
    `${f(fps, 0)} fps · ${Math.round(hdr.w)}×${Math.round(hdr.h)} rays${floatRT ? '' : ' (8-bit)'}\n` +
    `M = ${f(M, 3)}   r_s = ${f(2 * M)}   r_ph = ${f(3 * M)}   r_isco = ${f(6 * M)}\n` +
    `shadow b_c = 3√3 M = ${f(Math.sqrt(27) * M)}\n` +
    `camera r = ${f(cam.dist / M, 1)} M   clock rate ${f(camLapse, 4)}\n` +
    `t = ${f(simT, 0)} M   bodies ${nb}   debris ${nd}   swallowed ${E.absorbed_count()}`;
  const b = selected && bodies.find((x) => x.id === selected);
  if (!b) { infoEl.style.display = 'none'; selected = 0; return; }
  const sp = E.scratch_ptr();
  E.body_info(selected, sp);
  const v = new Float64Array(E.memory.buffer, sp, 6);
  const [r, vloc, rate, tau, En, L] = v;
  const fate = L / M < Math.sqrt(12) ? 'no stable orbit possible: will plunge' : En >= 1 ? 'unbound: may escape' : 'bound orbit';
  infoEl.style.display = 'block';
  infoEl.textContent =
    `body #${selected}\n` +
    `r            ${f(r / M, 3)} M\n` +
    `local speed  ${f(vloc, 4)} c\n` +
    `dτ/dt        ${f(rate, 5)}  (its clock vs. distant observer)\n` +
    `proper time  ${f(tau, 1)} M\n` +
    `energy E     ${f(En, 5)}\n` +
    `ang. mom L   ${f(L / M, 3)} M   ${fate}`;
}

syncFlags();
E.set_mass(params.M);
{
  const q = new URLSearchParams(location.search).get('preset');
  if (q && PRESETS[q]) { PRESETS[q](); E.set_mass(params.M); syncFlags(); syncAll(); cam.yaw = cam.tyaw; cam.pitch = cam.tpitch; cam.dist = cam.tdist; }
}
requestAnimationFrame(loop);
