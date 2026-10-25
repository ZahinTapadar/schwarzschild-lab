// Render worker: owns every nw-th image row starting at k and accumulates
// samples for them in its own WebAssembly instance.

let E, mem;
const enc = new TextEncoder();
const dec = new TextDecoder();

function put(bytes) {
  const p = E.alloc(bytes.length);
  new Uint8Array(E.memory.buffer, p, bytes.length).set(bytes);
  return [p, bytes.length];
}
function out() {
  return new Uint8Array(E.memory.buffer, E.out_ptr(), E.out_len()).slice();
}
function log() {
  E.log_to_out();
  return dec.decode(out());
}

onmessage = async ({ data: m }) => {
  if (m.cmd === 'load') {
    const { instance } = await WebAssembly.instantiate(m.wasm);
    E = instance.exports;
    postMessage({ cmd: 'loaded' });
  } else if (m.cmd === 'init') {
    const t0 = performance.now();
    if (m.sky) {
      const [kp, kn] = put(new Uint8Array(m.sky));
      E.w_give_sky(kp, kn);
      E.dealloc(kp, kn);
    }
    const [sp, sn] = put(enc.encode(m.scene));
    let vp = 0, vn = 0;
    if (m.volume) [vp, vn] = put(new Uint8Array(m.volume));
    const rows = E.w_init(sp, sn, vp, vn, m.k, m.nw);
    E.dealloc(sp, sn);
    if (vn) E.dealloc(vp, vn);
    let sky = null;
    if (m.wantSky) { E.w_sky_bytes(); sky = out().buffer; }
    postMessage({ cmd: 'ready', rows, log: log(), ms: performance.now() - t0, sky }, sky ? [sky] : []);
  } else if (m.cmd === 'render') {
    const t0 = performance.now();
    const spp = E.w_render(m.add);
    E.w_stats();
    const stats = JSON.parse(dec.decode(out()));
    stats.mem_bytes = E.memory.buffer.byteLength;
    postMessage({ cmd: 'rendered', spp, ms: performance.now() - t0, stats });
  } else if (m.cmd === 'pack') {
    const stride = E.w_pack();
    const buf = out().buffer;
    postMessage({ cmd: 'packed', buf, stride }, [buf]);
  } else if (m.cmd === 'instrument') {
    const [sp, sn] = put(enc.encode(m.scene));
    E.w_set_instrument(sp, sn);
    E.dealloc(sp, sn);
    postMessage({ cmd: 'instrumented' });
  } else if (m.cmd === 'get_accum') {
    E.w_get_accum();
    const buf = out().buffer;
    postMessage({ cmd: 'accum', buf }, [buf]);
  } else if (m.cmd === 'set_accum') {
    const [p, n] = put(new Uint8Array(m.buf));
    const r = E.w_set_accum(p, n, m.spp);
    E.dealloc(p, n);
    postMessage({ cmd: 'accum_set', ok: r === 0 });
  } else if (m.cmd === 'reset') {
    E.w_reset();
    postMessage({ cmd: 'reset_done' });
  }
};
