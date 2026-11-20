// Shared runtime for the Pillars essay demos. Every section script uses only this file and the DOM.
// Demos animate only while on screen (IntersectionObserver), redraw on input, and read colours from CSS tokens.
(function () {
  'use strict';

  // ---- physical constants (cgs unless noted) ----
  const C = {
    c: 2.99792458e10,          // cm/s
    G: 6.674e-8,               // cm^3 g^-1 s^-2
    kB: 1.380649e-16,          // erg/K
    h: 6.62607e-27,            // erg s
    mH: 1.6735e-24,            // g
    sigmaSB: 5.6704e-5,        // erg cm^-2 s^-1 K^-4
    Msun: 1.989e33,            // g
    Lsun: 3.828e33,            // erg/s
    Rsun: 6.957e10,            // cm
    pc: 3.0857e18,             // cm
    ly: 9.4607e17,             // cm
    au: 1.496e13,              // cm
    yr: 3.15576e7,             // s
    alphaB: 2.59e-13,          // case-B recombination coefficient at 1e4 K, cm^3/s
    g0: 980.665,               // cm/s^2 (1 g)
  };

  // ---- formatting ----
  const SUP = { '-': '⁻', '0': '⁰', '1': '¹', '2': '²', '3': '³', '4': '⁴', '5': '⁵', '6': '⁶', '7': '⁷', '8': '⁸', '9': '⁹' };
  function sci(x, digits = 2) {
    if (!isFinite(x)) return String(x);
    if (x === 0) return '0';
    const e = Math.floor(Math.log10(Math.abs(x)));
    if (e >= -2 && e <= 4) return fmt(x, digits);
    const m = x / Math.pow(10, e);
    return `${m.toFixed(digits - 1)} × 10${String(e).split('').map(ch => SUP[ch]).join('')}`;
  }
  function fmt(x, digits = 3) {
    if (Math.abs(x) >= 1000) return Math.round(x).toLocaleString('en-US');
    return Number(x.toPrecision(digits)).toString();
  }
  function duration(seconds) {
    const y = seconds / C.yr;
    if (y >= 1e9) return `${fmt(y / 1e9)} billion years`;
    if (y >= 1e6) return `${fmt(y / 1e6)} million years`;
    if (y >= 1) return `${fmt(y)} years`;
    const d = seconds / 86400;
    if (d >= 1) return `${fmt(d)} days`;
    return `${fmt(seconds)} s`;
  }

  const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const lerp = (a, b, t) => a + (b - a) * t;
  const clamp = (x, a, b) => Math.min(b, Math.max(a, x));
  const smooth = (t) => t * t * (3 - 2 * t);

  // Deterministic PRNG so demos look the same on every load.
  function rng(seed = 1) {
    let s = seed >>> 0;
    return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  }

  // ---- demos ----
  // demo(id, setup): setup(ctx) runs once when the figure first nears the viewport.
  // ctx = { fig, stage, canvas, g (2D context), w, h (CSS px), dpr, controls, slider(), toggle(), redraw() }
  // setup returns { draw(t, dt) , animate: bool }. Static demos set animate:false and are redrawn on input/resize.
  const registry = [];
  function demo(id, setup, opts = {}) { registry.push({ id, setup, opts }); }

  function boot() {
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        const d = e.target.__demo;
        if (!d) continue;
        d.visible = e.isIntersecting;
        if (e.isIntersecting && !d.started) start(d);
        if (d.visible && d.inst && d.inst.animate) loop(d);
      }
    }, { rootMargin: '200px 0px' });

    for (const r of registry) {
      const fig = document.getElementById(r.id);
      if (!fig) { console.warn('demo figure missing:', r.id); continue; }
      const d = { ...r, fig, started: false, visible: false, raf: 0, last: 0 };
      fig.__demo = d;
      io.observe(fig);
    }
  }

  function start(d) {
    d.started = true;
    const fig = d.fig;
    const stage = fig.querySelector('.stage');
    const aspect = parseFloat(stage.dataset.aspect || d.opts.aspect || 0.5);
    let canvas = stage.querySelector('canvas');
    if (!canvas) { canvas = document.createElement('canvas'); stage.appendChild(canvas); }
    let controls = fig.querySelector('.controls');
    if (!controls) { controls = document.createElement('div'); controls.className = 'controls'; stage.after(controls); }
    const g = canvas.getContext(d.opts.webgl ? 'webgl2' : '2d');
    const ctx = { fig, stage, canvas, g, controls, w: 0, h: 0, dpr: 1, state: {} };

    const size = () => {
      const w = stage.clientWidth;
      const h = Math.round(w * aspect);
      stage.style.height = h + 'px';
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr);
      ctx.w = w; ctx.h = h; ctx.dpr = dpr;
      if (!d.opts.webgl) g.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    size();
    ctx.redraw = () => { if (!d.inst) return; if (!d.inst.animate) requestAnimationFrame(() => paint(d, performance.now())); };
    ctx.slider = (o) => slider(controls, o, ctx.redraw);
    ctx.toggle = (o) => toggle(controls, o, ctx.redraw);
    ctx.choice = (o) => choice(controls, o, ctx.redraw);
    d.ctx = ctx;
    try { d.inst = d.setup(ctx) || {}; }
    catch (err) { console.error('demo setup failed:', d.id, err); return; }
    // size() resets canvas.width, which clears it, so always repaint after.
    new ResizeObserver(() => { const old = ctx.w; size(); if (old !== ctx.w && d.inst.resize) d.inst.resize(); ctx.redraw(); }).observe(stage);
    paint(d, performance.now());
    if (d.inst.animate && d.visible) loop(d);
    // Theme flips change token colours.
    matchMedia('(prefers-color-scheme: dark)').addEventListener('change', ctx.redraw);
  }

  function paint(d, now) {
    const dt = d.last ? Math.min(0.05, (now - d.last) / 1000) : 0;
    d.last = now;
    try { d.inst.draw && d.inst.draw(now / 1000, dt); }
    catch (err) { console.error('demo draw failed:', d.id, err); d.inst.animate = false; }
  }
  function loop(d) {
    if (d.raf) return;
    const step = (now) => {
      d.raf = 0;
      if (!d.visible || !d.inst.animate) { d.last = 0; return; }
      paint(d, now);
      d.raf = requestAnimationFrame(step);
    };
    d.raf = requestAnimationFrame(step);
  }

  let uid = 0;
  function slider(parent, o, redraw) {
    const id = 'ctl-' + (o.id || ++uid);
    const wrap = document.createElement('div'); wrap.className = 'ctl';
    const label = document.createElement('label'); label.htmlFor = id;
    const name = document.createElement('span'); name.textContent = o.label;
    const out = document.createElement('output');
    label.append(name, out);
    const input = document.createElement('input');
    Object.assign(input, { type: 'range', id, min: o.min ?? 0, max: o.max ?? 1, step: o.step ?? 'any', value: o.value ?? 0 });
    wrap.append(label, input); parent.appendChild(wrap);
    const s = { get value() { return +input.value; }, set value(v) { input.value = v; show(); }, input };
    const show = () => { out.textContent = o.format ? o.format(+input.value) : fmt(+input.value); };
    input.addEventListener('input', () => { show(); o.onInput && o.onInput(+input.value); redraw(); });
    show();
    return s;
  }
  function toggle(parent, o, redraw) {
    const b = document.createElement('button'); b.className = 'btn'; b.type = 'button'; b.textContent = o.label;
    b.setAttribute('aria-pressed', String(!!o.value));
    const t = { get value() { return b.getAttribute('aria-pressed') === 'true'; }, el: b };
    b.addEventListener('click', () => { b.setAttribute('aria-pressed', String(!t.value)); o.onInput && o.onInput(t.value); redraw(); });
    parent.appendChild(b); return t;
  }
  function choice(parent, o, redraw) {
    const seg = document.createElement('div'); seg.className = 'seg'; seg.setAttribute('role', 'group');
    if (o.label) seg.setAttribute('aria-label', o.label);
    const c = { value: o.value ?? o.options[0].value };
    const btns = o.options.map(opt => {
      const b = document.createElement('button'); b.className = 'btn'; b.type = 'button'; b.textContent = opt.label;
      b.setAttribute('aria-pressed', String(opt.value === c.value));
      b.addEventListener('click', () => { c.value = opt.value; btns.forEach((x, i) => x.setAttribute('aria-pressed', String(o.options[i].value === c.value))); o.onInput && o.onInput(c.value); redraw(); });
      seg.appendChild(b); return b;
    });
    parent.appendChild(seg); return c;
  }

  // Pointer drag helper: calls fn(dx, dy, x, y, phase) in CSS px relative to the canvas.
  function drag(el, fn) {
    let px = 0, py = 0, active = false;
    const pos = (e) => { const r = el.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
    el.addEventListener('pointerdown', (e) => { active = true; el.setPointerCapture(e.pointerId); [px, py] = pos(e); fn(0, 0, px, py, 'start'); });
    el.addEventListener('pointermove', (e) => { if (!active) return; const [x, y] = pos(e); fn(x - px, y - py, x, y, 'move'); px = x; py = y; });
    const end = (e) => { if (!active) return; active = false; const [x, y] = pos(e); fn(0, 0, x, y, 'end'); };
    el.addEventListener('pointerup', end); el.addEventListener('pointercancel', end);
  }

  // Before/after image swipe for <figure> containing .swipe with two <img> (bottom first, top second).
  function swipes() {
    document.querySelectorAll('.swipe').forEach((sw) => {
      const imgs = sw.querySelectorAll('img');
      if (imgs.length < 2) return;
      const top = document.createElement('div'); top.className = 'top';
      imgs[1].replaceWith(top); top.appendChild(imgs[1]);
      const bar = document.createElement('div'); bar.className = 'bar'; sw.appendChild(bar);
      const set = (f) => { f = clamp(f, 0, 1); top.style.width = (f * 100) + '%'; bar.style.left = (f * 100) + '%'; imgs[1].style.width = sw.clientWidth + 'px'; };
      const at = (e) => { const r = sw.getBoundingClientRect(); set((e.clientX - r.left) / r.width); };
      let down = false;
      sw.addEventListener('pointerdown', (e) => { down = true; sw.setPointerCapture(e.pointerId); at(e); });
      sw.addEventListener('pointermove', (e) => { if (down) at(e); });
      sw.addEventListener('pointerup', () => { down = false; });
      sw.tabIndex = 0; sw.setAttribute('role', 'slider'); sw.setAttribute('aria-label', sw.dataset.label || 'Compare images');
      let f = 0.5;
      sw.addEventListener('keydown', (e) => { if (e.key === 'ArrowLeft') set(f = clamp(f - 0.05, 0, 1)); if (e.key === 'ArrowRight') set(f = clamp(f + 0.05, 0, 1)); });
      new ResizeObserver(() => set(parseFloat(top.style.width) / 100 || 0.5)).observe(sw);
      set(0.5);
    });
  }

  // Blackbody: Planck B_lambda (cgs per cm) and a cheap sRGB approximation of a blackbody colour.
  function planck(lambdaCm, T) { const a = 2 * C.h * C.c * C.c / Math.pow(lambdaCm, 5); return a / (Math.exp(C.h * C.c / (lambdaCm * C.kB * T)) - 1); }
  function bbColor(T) { // Tanner Helland fit, fine for illustration
    const t = T / 100; let r, g, b;
    r = t <= 66 ? 255 : 329.698727446 * Math.pow(t - 60, -0.1332047592);
    g = t <= 66 ? 99.4708025861 * Math.log(t) - 161.1195681661 : 288.1221695283 * Math.pow(t - 60, -0.0755148492);
    b = t >= 66 ? 255 : (t <= 19 ? 0 : 138.5177312231 * Math.log(t - 10) - 305.0447927307);
    return [r, g, b].map(v => Math.round(clamp(v, 0, 255)));
  }
  // Wavelength (nm) to an approximate display colour for 380-780 nm.
  function nmColor(nm) {
    let r = 0, g = 0, b = 0;
    if (nm >= 380 && nm < 440) { r = -(nm - 440) / 60; b = 1; }
    else if (nm < 490) { g = (nm - 440) / 50; b = 1; }
    else if (nm < 510) { g = 1; b = -(nm - 510) / 20; }
    else if (nm < 580) { r = (nm - 510) / 70; g = 1; }
    else if (nm < 645) { r = 1; g = -(nm - 645) / 65; }
    else if (nm <= 780) { r = 1; }
    const f = nm < 420 ? 0.3 + 0.7 * (nm - 380) / 40 : nm > 700 ? 0.3 + 0.7 * (780 - nm) / 80 : 1;
    return [r, g, b].map(v => Math.round(255 * Math.pow(clamp(v * f, 0, 1), 0.8)));
  }

  window.P = { C, sci, fmt, duration, css, lerp, clamp, smooth, rng, demo, drag, planck, bbColor, nmColor };
  const go = () => { swipes(); boot(); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go); else go();
})();
