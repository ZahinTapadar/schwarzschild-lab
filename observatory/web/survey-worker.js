// Multiwavelength Survey worker. One instance is the generator/compositor
// (s_* exports); the others ray-march rows of the volume (m_* exports).

let E;
const enc = new TextEncoder();
const dec = new TextDecoder();

function put(bytes) {
  const p = E.alloc(bytes.length);
  new Uint8Array(E.memory.buffer, p, bytes.length).set(bytes);
  return [p, bytes.length];
}
const out = () => new Uint8Array(E.memory.buffer, E.out_ptr(), E.out_len()).slice();
function call(fn, bytes, ...rest) {
  const [p, n] = put(bytes);
  const r = fn(p, n, ...rest);
  E.dealloc(p, n);
  return r;
}

onmessage = async ({ data: m }) => {
  const t0 = performance.now();
  const reply = (msg, transfer = []) => postMessage({ ...msg, ms: performance.now() - t0 }, transfer);
  if (m.cmd === 'load') {
    E = (await WebAssembly.instantiate(m.wasm)).instance.exports;
    reply({ cmd: 'loaded' });
  } else if (m.cmd === 'init') {
    const r = call(E.s_init, enc.encode(m.preset), m.grid, m.w, m.h);
    reply({ cmd: 'inited', ok: r === 0, info: r === 0 ? JSON.parse(dec.decode(out())) : null, seed: E.s_seed() });
  } else if (m.cmd === 'diag') {
    E.s_diagnostics();
    const b = out().buffer;
    reply({ cmd: 'diagged', buf: b }, [b]);
  } else if (m.cmd === 'prepare') {
    const nb = call(E.s_prepare, enc.encode(m.id));
    const b = out().buffer;
    reply({ cmd: 'prepared', nb, grids: b }, [b]);
  } else if (m.cmd === 'finish') {
    const r = call(E.s_finish, new Uint8Array(m.rows), m.spp, m.noise ? 1 : 0);
    const b = out().buffer;
    reply({ cmd: 'finished', ok: r === 0, buf: b }, [b]);
  } else if (m.cmd === 'cross') {
    E.s_cross();
    const b = out().buffer;
    reply({ cmd: 'crossed', buf: b }, [b]);
  } else if (m.cmd === 'fits') {
    call(E.s_fits, enc.encode(m.id), m.band);
    const b = out().buffer;
    reply({ cmd: 'fitsed', buf: b }, [b]);
  } else if (m.cmd === 'minit') {
    const [gp, gn] = put(new Uint8Array(m.grids));
    const [vp, vn] = put(enc.encode(m.view));
    const r = E.m_init(gp, gn, vp, vn);
    E.dealloc(gp, gn);
    E.dealloc(vp, vn);
    reply({ cmd: 'minited', ok: r === 0 });
  } else if (m.cmd === 'mrows') {
    E.m_rows(m.k, m.nw, m.spp, m.seed % 4294967296, Math.floor(m.seed / 4294967296));
    const b = out().buffer;
    E.m_free();
    reply({ cmd: 'mrowed', buf: b }, [b]);
  }
};
