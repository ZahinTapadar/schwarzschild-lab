// Multiwavelength Survey: browser orchestration.
// One worker generates the scene and composes images; the others ray-march
// rows of each instrument's band grids.

const $ = (s) => document.querySelector(s);
const dec = new TextDecoder();
const PRESETS = [
  ['pillars', 'Pillars: one cloud, six instruments'],
  ['cluster-core', 'Cluster core: a merging cluster, galaxies to mass'],
  ['supercluster', 'Supercluster: 40° of sky and a redshift survey'],
];

let wasm, gen, marchers = [];
let presets = {}, info = null, current = null, images = [], busy = false;
const chosen = new Set();

function ask(w, msg, reply, transfer = []) {
  return new Promise((res) => {
    const h = ({ data }) => { if (data.cmd === reply) { w.removeEventListener('message', h); res(data); } };
    w.addEventListener('message', h);
    w.postMessage(msg, transfer);
  });
}

function log(html, cls) {
  const el = $('#log');
  el.insertAdjacentHTML('beforeend', `<div${cls ? ` class="${cls}"` : ''}>${html}</div>`);
  el.scrollTop = el.scrollHeight;
}

// packed images: u32 count, then (u32 json length, json, u32 png length, png)
function unpack(buf) {
  const v = new DataView(buf);
  const u8 = new Uint8Array(buf);
  let o = 0;
  const n = v.getUint32(o, true); o += 4;
  const out = [];
  for (let k = 0; k < n; k++) {
    const jl = v.getUint32(o, true); o += 4;
    const meta = JSON.parse(dec.decode(u8.subarray(o, o + jl))); o += jl;
    const pl = v.getUint32(o, true); o += 4;
    const blob = new Blob([u8.slice(o, o + pl)], { type: 'image/png' }); o += pl;
    out.push({ ...meta, url: URL.createObjectURL(blob), blob });
  }
  return out;
}

function addImages(list, group) {
  for (const im of list) {
    im.group = group;
    images.push(im);
    const f = document.createElement('figure');
    f.innerHTML = `<img src="${im.url}" alt=""><figcaption>${group} · ${im.stem.replace(/^[a-z_]+-/, '')}</figcaption>`;
    f.onclick = () => show(im, f);
    $('#strip').appendChild(f);
    im.fig = f;
  }
}

function show(im, fig) {
  current = im;
  $('#stage').innerHTML = '';
  const img = document.createElement('img');
  img.src = im.url;
  img.alt = `${im.group} ${im.stem}`;
  $('#stage').appendChild(img);
  document.querySelectorAll('#strip figure').forEach((f) => f.classList.toggle('on', f === (fig || im.fig)));
  $('#png').disabled = false;
  $('#fits').disabled = !(im.band !== undefined && im.ins);
}

function renderInstruments() {
  const p = presets[$('#preset').value];
  $('#pdesc').textContent = p.description || '';
  const box = $('#ins');
  box.innerHTML = '';
  chosen.clear();
  for (const id of p.instruments) {
    chosen.add(id);
    const b = document.createElement('button');
    b.textContent = id.replace(/_/g, ' ');
    b.className = 'on';
    b.onclick = () => { if (chosen.has(id)) chosen.delete(id); else chosen.add(id); b.classList.toggle('on', chosen.has(id)); };
    box.appendChild(b);
  }
}

async function boot() {
  wasm = await (await fetch('observatory.wasm')).arrayBuffer();
  for (const [name] of PRESETS) presets[name] = await (await fetch(`presets/survey/${name}.json`)).json();
  const sel = $('#preset');
  for (const [name, title] of PRESETS) sel.insertAdjacentHTML('beforeend', `<option value="${name}">${title}</option>`);
  sel.onchange = renderInstruments;
  renderInstruments();
  const n = Math.max(1, Math.min(6, (navigator.hardwareConcurrency || 4) - 1));
  gen = new Worker('survey-worker.js');
  marchers = Array.from({ length: n }, () => new Worker('survey-worker.js'));
  await Promise.all([gen, ...marchers].map((w) => ask(w, { cmd: 'load', wasm }, 'loaded')));
  $('#log').innerHTML = '';
  log(`Ready: 1 generator and ${n} ray-march workers.`);
  $('#run').disabled = false;
}

async function observe() {
  if (busy) return;
  busy = true;
  $('#run').disabled = true;
  images.forEach((im) => URL.revokeObjectURL(im.url));
  images = [];
  $('#strip').innerHTML = '';
  $('#log').innerHTML = '';
  const name = $('#preset').value;
  const [grid, w, h, spp] = $('#quality').value.split(',').map(Number);
  const noise = $('#noise').checked;
  const t0 = performance.now();
  log(`Generating <b>${name}</b> on a ${grid}³ grid…`);
  const r = await ask(gen, { cmd: 'init', preset: JSON.stringify(presets[name]), grid, w, h }, 'inited');
  if (!r.ok) { log('Generation failed.', 'err'); busy = false; $('#run').disabled = false; return; }
  info = r.info;
  const g = info.generator;
  log(`Generated in ${(r.ms / 1000).toFixed(1)} s · ${info.sources.toLocaleString()} sources · ${info.pix_arcsec.toFixed(2)}″/px` +
      (g.haloes ? ` · ${g.haloes} haloes, z = ${g.z_center.toFixed(3)}` : g.ionised_volume !== undefined ? ` · ${(g.ionised_volume * 100).toFixed(0)}% ionised` : ''));
  const about = $('#about');
  about.innerHTML = '';
  for (const ins of info.instruments) {
    about.insertAdjacentHTML('beforeend', `<dt>${ins.name} <span style="color:var(--dim)">(${ins.unit})</span></dt><dd>${ins.analog}</dd>` +
      ins.bands.map((b) => `<dd>· <b style="font-weight:500;color:var(--text)">${b.name}</b>: ${b.traces}</dd>`).join(''));
  }
  const d = await ask(gen, { cmd: 'diag' }, 'diagged');
  addImages(unpack(d.buf), 'scene');
  const viewJson = JSON.stringify(info.view);
  for (const ins of info.instruments) {
    if (!chosen.has(ins.id)) continue;
    const ti = performance.now();
    log(`<b>${ins.name}</b>: emissivity grids…`);
    const pr = await ask(gen, { cmd: 'prepare', id: ins.id }, 'prepared');
    const nb = pr.nb;
    let rows = new ArrayBuffer(0);
    if (pr.grids.byteLength > 0) {
      log(`  ${(pr.grids.byteLength / 1e6).toFixed(0)} MB of band grids → ${marchers.length} workers`);
      await Promise.all(marchers.map((m) => ask(m, { cmd: 'minit', grids: pr.grids, view: viewJson }, 'minited')));
      const nw = marchers.length;
      const parts = await Promise.all(marchers.map((m, k) => ask(m, { cmd: 'mrows', k, nw, spp, seed: r.seed }, 'mrowed')));
      const all = new Float32Array(w * h * nb);
      parts.forEach((p, k) => {
        const f = new Float32Array(p.buf);
        let o = 0;
        for (let y = k; y < h; y += nw) { all.set(f.subarray(o, o + w * nb), y * w * nb); o += w * nb; }
      });
      rows = all.buffer;
    }
    const fin = await ask(gen, { cmd: 'finish', rows, spp, noise }, 'finished', [rows]);
    const list = unpack(fin.buf);
    list.forEach((im, k) => { im.ins = ins.id; const bandIdx = list.length > nb ? k - 1 : k; if (bandIdx >= 0) im.band = bandIdx; });
    addImages(list, ins.name);
    if (!current || current.group === 'scene') show(list[0]);
    log(`  done in ${((performance.now() - ti) / 1000).toFixed(1)} s`);
  }
  const c = await ask(gen, { cmd: 'cross' }, 'crossed');
  const cross = unpack(c.buf);
  addImages(cross, 'composite');
  if (cross.length) show(cross[cross.length - 1]);
  log(`All instruments in ${((performance.now() - t0) / 1000).toFixed(1)} s.`);
  busy = false;
  $('#run').disabled = false;
}

function download(blob, name) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}

$('#run').onclick = observe;
$('#png').onclick = () => current && download(current.blob, `${$('#preset').value}-${current.stem}.png`);
$('#fits').onclick = async () => {
  if (!current || current.band === undefined) return;
  const r = await ask(gen, { cmd: 'fits', id: current.ins, band: current.band }, 'fitsed');
  download(new Blob([r.buf], { type: 'application/fits' }), `${$('#preset').value}-${current.stem}.fits`);
};
boot().catch((e) => log(String(e), 'err'));
